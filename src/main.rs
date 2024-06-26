// Parse the NVIDIA ctrl header files and generate a complete json database of defines and structs

extern crate clang;

use clang::*;
use std::env;
use serde::{Deserialize, Serialize};

use std::fs::File;
use std::collections::BTreeMap;
use std::io::{BufWriter, Write};
use walkdir::{DirEntry, WalkDir};

// start/end are in bits
#[derive(Serialize, Deserialize)]
struct HWStructField {
    name: String,
    start: u32,
    size: u32,
    group_len: u32,
    val_type: String,
}

#[derive(Serialize, Deserialize)]
struct HWStruct {
    total_size: u32,
    fields: Vec<HWStructField>,
}

#[derive(Serialize, Deserialize, Default)]
enum HWDefineType {
    #[default]
    Unknown,
    Value,
}

#[derive(Serialize, Deserialize, Default)]
struct HWDefine {
    hwtype: HWDefineType,
    vals: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct HWJson {
    version: String,
    defines: BTreeMap<String, HWDefine>,
    structs: BTreeMap<String, HWStruct>,
}

#[derive(Serialize, Deserialize)]
struct CStructField {
    ftype: String,
    name: String,
    is_array: bool,
    size: u32,
    is_aligned: bool,
    alignment: u32,
}

#[derive(Serialize, Deserialize, Default)]
enum CType {
    #[default]
    Unknown,
    Value,
    Struct,
    Typedef,
}

#[derive(Serialize, Deserialize, Default)]
struct CTypes {
    ctype: CType,
    vals: Vec<String>,
    is_anon_struct: bool,
    // for structs
    fields: Vec<CStructField>,
}

#[derive(Serialize, Deserialize, Default)]
struct CJson {
    version: String,
    types: BTreeMap<String, CTypes>,
}

fn get_type_size(fld_type: Type) -> usize {
    let mut size = 0;
    if fld_type.is_elaborated().unwrap() {
	let field_elab_type = fld_type.get_elaborated_type().unwrap();
	if field_elab_type.get_kind() == TypeKind::Typedef {
	    let field_elab_typedef = field_elab_type.get_declaration().unwrap().get_typedef_underlying_type();
	    size = field_elab_typedef.unwrap().get_sizeof().unwrap() * 8;

	}
    }
    size
}

// recursive function that handles records inside records.
// used for handling union/struct nesting
fn handle_record(base_offset: usize, newfields: &mut Vec<HWStructField>, record: Entity) -> usize {
    let mut end_offset = base_offset;
    for fld in record.get_type().unwrap().get_fields().unwrap() {
	let fld_type = fld.get_type().unwrap();
	let this_base_offset = base_offset + fld.get_offset_of_field().unwrap();

	if fld_type.get_kind() == TypeKind::Record {
	    end_offset = handle_record(this_base_offset, newfields, fld);
	    continue;
	}

	let size: usize;
	let mut group_size: usize = 0;
	let mut valname = "".to_string();
	if fld_type.get_kind() == TypeKind::ConstantArray {
	    group_size = fld_type.get_size().unwrap();
	    size = get_type_size(fld_type.get_element_type().unwrap());
	    valname = fld_type.get_element_type().unwrap().get_display_name();
	} else {
	    size = get_type_size(fld_type);
	}

	end_offset = this_base_offset + size;

	newfields.push(HWStructField {
	    name: fld.get_display_name().unwrap(),
	    val_type: valname,
	    start: this_base_offset as u32,
	    size: size as u32,
	    group_len: group_size as u32,
	})
    }
    end_offset
}

fn setup_parser<'a>(index: &'a Index, path: &str, prefix: &String) -> std::io::Result<TranslationUnit<'a>> {
    // Parse a source file into a translation unit
    let mut parser = index.parser(path);

    let mut args : Vec<String> = Default::default();

    for define in DEFINES {
	args.push(define.to_string());
    }

    for incpath in INCPATHS {
	args.push("-I".to_string() + prefix + incpath);
    }

    for incfile in INCFILES {
	args.push("-include".to_string());
	args.push(incfile.to_string());
    }

    // turn on detailed preprocessing to get defines
    parser.detailed_preprocessing_record(true);
    parser.arguments(&args);
    Ok(parser.parse().unwrap())
}

fn add_file_to_hwjson<'a>(tu: &TranslationUnit<'a>, json_output: &mut HWJson) -> std::io::Result<()> {
    // Get the declearations?
    let defines = tu.get_entity().get_children().into_iter().filter(|e| {
	e.get_kind() == EntityKind::MacroDefinition &&
        !e.is_builtin_macro() &&
        !e.is_function_like_macro()
    }).collect::<Vec<_>>();

    for define_ in defines {
	let name = define_.get_display_name().unwrap();
	let mut hwtype : HWDefineType = HWDefineType::Unknown;
	// filter out the __ ones
	if name.as_bytes()[0] == b'_' && name.as_bytes()[1] == b'_' {
	    continue;
	}
	let tokens = define_.get_range().unwrap().tokenize();
	// All the interesting ones have 4 tokens.
	if tokens.len() != 2 && tokens.len() != 4 {
	    continue;
	}

	let mut vals: Vec<String> = Default::default();
	if tokens.len() == 2 {
	    hwtype = HWDefineType::Value;
	    vals.push(tokens[1].get_spelling());
	} else if tokens[1].get_spelling() == "(" && tokens[3].get_spelling() == ")" {
	    hwtype = HWDefineType::Value;
	    vals.push(tokens[2].get_spelling());
	} else if tokens[2].get_spelling() == ":" {
	    hwtype = HWDefineType::Value;
	    vals.push(tokens[1].get_spelling());
	    vals.push(tokens[3].get_spelling());
	}

	json_output.defines.insert(name, HWDefine {
	    hwtype,
	    vals,
	});
    }

    let enums = tu.get_entity().get_children().into_iter().filter(|e| {
	e.get_kind() == EntityKind::EnumDecl
    }).collect::<Vec<_>>();

    for tenum in enums {
	for child in tenum.get_children() {

	    println!("{:?} {:?}", child.get_display_name(), child.get_enum_constant_value());
	    json_output.defines.insert(child.get_display_name().unwrap(), HWDefine {
		hwtype: HWDefineType::Value,
		vals: vec!(child.get_enum_constant_value().unwrap().1.to_string()),
	    });
	}
    }
    let typedefs = tu.get_entity().get_children().into_iter().filter(|e| {
        e.get_kind() == EntityKind::TypedefDecl
    }).collect::<Vec<_>>();

    for typedef in typedefs {
	let under_type = typedef.get_typedef_underlying_type().unwrap();

	if !under_type.is_elaborated().unwrap() {
	    continue
	}

	let elab_type = under_type.get_elaborated_type().unwrap();
	if elab_type.get_kind() != TypeKind::Record {
	    continue
	}

	let mut newfields : Vec<HWStructField> = Default::default();

	let base_offset = 0;
	let total_size = handle_record(base_offset, &mut newfields, elab_type.get_declaration().unwrap());

	let thisname = typedef.get_display_name().unwrap();
	json_output.structs.insert(thisname,
				   HWStruct {
				       total_size: total_size as u32,
				       fields: newfields,
				   });
    }
    Ok(())
}

fn add_file_to_cjson<'a>(tu: &TranslationUnit<'a>, json_output: &mut CJson) -> std::io::Result<()> {
    // Get the declearations?
    let defines = tu.get_entity().get_children().into_iter().filter(|e| {
	e.get_kind() == EntityKind::MacroDefinition &&
        !e.is_builtin_macro() &&
        !e.is_function_like_macro()
    }).collect::<Vec<_>>();

    for define_ in defines {
	let name = define_.get_display_name().unwrap();
	let mut ctype : CType = CType::Unknown;

	// filter out the __ ones
	if name.as_bytes()[0] == b'_' && name.as_bytes()[1] == b'_' {
	    continue;
	}
	let tokens = define_.get_range().unwrap().tokenize();
	// All the interesting ones have 4 tokens.
	if tokens.len() != 4 {
	    continue;
	}

	let mut vals: Vec<String> = Default::default();
	if tokens[1].get_spelling() == "(" && tokens[3].get_spelling() == ")" {
	    ctype = CType::Value;
	    vals.push(tokens[2].get_spelling());
	} else if tokens[2].get_spelling() == ":" {
	    ctype = CType::Value;
	    vals.push(tokens[1].get_spelling());
	    vals.push(tokens[3].get_spelling());
	}

	json_output.types.insert(name, CTypes {
	    ctype,
	    vals,
	    is_anon_struct: false,
	    fields: Default::default(),
	});
    }

    // Get the structs in this translation unit
    let structs = tu.get_entity().get_children().into_iter().filter(|e| {
        e.get_kind() == EntityKind::StructDecl
    }).collect::<Vec<_>>();

    // Print information about the structs
    for struct_ in structs {
	let mut newfields : Vec<CStructField> = Default::default();

        for field in struct_.get_children() {
	    let mut is_array = false;
	    let mut this_size = 0;
	    let mut is_aligned = false;
	    let mut alignment : usize = 0;

	    if field.has_attributes() {
		let attr = field.get_child(0).unwrap();
		if attr.get_kind() == EntityKind::AlignedAttr {
		    is_aligned = true;
		    alignment = field.get_type().unwrap().get_alignof().unwrap();
		}
	    }
	    let wrapped_size = field.get_type().unwrap().get_size();
	    if wrapped_size.is_some() {
		this_size = wrapped_size.unwrap();
		if this_size > 0 {
		    is_array = true;
		}
	    }
	    newfields.push(CStructField {
		name: field.get_name().unwrap(),
		is_array,
		size : this_size as u32,
		ftype: field.get_type().unwrap().get_display_name(),
		is_aligned,
		alignment: alignment as u32,
	    });
        }

	json_output.types.insert(
	    struct_.get_name().unwrap(),
	    CTypes {
		ctype: CType::Struct,
		fields: newfields,
		is_anon_struct: struct_.is_anonymous(),
		vals: vec!("".to_string()),
	    }
	    );
    }

    let typedefs = tu.get_entity().get_children().into_iter().filter(|e| {
        e.get_kind() == EntityKind::TypedefDecl
    }).collect::<Vec<_>>();

    // output typedef info to json
    for typedef in typedefs {
	json_output.types.insert(
	    typedef.get_name().unwrap(),
	    CTypes {
		ctype: CType::Typedef,
		vals: vec!(typedef.get_typedef_underlying_type().unwrap().get_display_name()),
		is_anon_struct: false,
		fields: Default::default(),
	    });
    }

    Ok(())
}

fn just_headers(entry: &DirEntry) -> bool {
    entry.file_name()
         .to_str()
         .map(|s| s.ends_with(".h"))
         .unwrap_or(false)
}

const INCFILES: &'static [&'static str] = &[
    "stddef.h",
    "cpuopsys.h",
    "gpu/mem_mgr/mem_desc.h",
    "g_rpc-structures.h",
    "g_rpc-message-header.h",
    "objrpc.h",
];

const INCPATHS: &'static [&'static str] = &[
    "src/common/inc",
    "src/common/inc/swref/published",
    "src/common/nvlink/inbound/interface",
    "src/common/shared/msgq/inc",
    "src/common/sdk/nvidia/inc",
    "src/common/shared/msgq/inc/msgq",
    "src/nvidia/",
    "src/nvidia/generated/",
    "src/nvidia/inc/",
    "src/nvidia/inc/kernel/",
    "src/nvidia/inc/libraries/",
    "src/nvidia/interface/",
    "src/nvidia/kernel/inc",
    "src/nvidia/arch/nvalloc/common/inc",
];

const DEFINES: &'static [&'static str] = &[
    "-DRPC_MESSAGE_GENERIC_UNION",
    "-DRPC_MESSAGE_STRUCTURES",
    "-DRPC_STRUCTURES",
    "-DRPC_GENERIC_UNION",
    "-DPORT_MODULE_memory=1",
    "-DPORT_MODULE_cpu=1",
    "-DPORT_MODULE_core=1",
    "-DPORT_MODULE_debug=1",
    "-DPORT_MODULE_util=1",
    "-DPORT_MODULE_safe=1",
    "-DPORT_MODULE_thread=1",
    "-DNVRM",
    "-D_LANGUAGE_C",
    "-D__NO_CTYPE",
    "-DRS_STANDALONE=0",
    "-DPORT_IS_CHECKED_BUILD=1",
    "-DPORT_IS_KERNEL_BUILD=1",
];

const PATHS: &'static [&'static str] = &[
    "src/common/sdk/nvidia/inc",
    "src/common/shared/msgq/inc/msgq",
    "src/nvidia/inc/kernel/gpu/gsp/",
    "src/nvidia/arch/nvalloc/common/inc/gsp/",
    "src/nvidia/kernel/inc/vgpu/",
];

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    // Acquire an instance of `Clang`
    let clang = Clang::new().unwrap();

    // Create a new `Index`
    let index = Index::new(&clang, false, false);

    let mut cjson_output : CJson = Default::default();
    let mut hwjson_output : HWJson = Default::default();

    cjson_output.version = args[1].clone();
    hwjson_output.version = args[1].clone();

    for path in PATHS {
	let newpath = args[2].clone() + "/" + path;
	for entry in WalkDir::new(&newpath).into_iter() {
	    let ent = entry.unwrap();
	    if !just_headers(&ent) {
		continue
	    }
	    let path = ent.path().to_str().unwrap();
	    println!("parsing {:?}", path);

	    let tu = setup_parser(&index, path, &args[2])?;
	    add_file_to_cjson(&tu, &mut cjson_output)?;
	    add_file_to_hwjson(&tu, &mut hwjson_output)?;
	}
    }

    let cjsonname = args[1].clone() + ".json";
    let file = File::create(cjsonname)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, &cjson_output)?;
    writer.flush()?;

    let hwjsonname = args[1].clone() + ".hw.json";
    let file = File::create(hwjsonname)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, &hwjson_output)?;
    writer.flush()?;
    Ok(())
}

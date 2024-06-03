// Parse the NVIDIA ctrl header files and generate a complete json database of defines and structs

extern crate clang;

use clang::*;
use std::env;
use serde::{Deserialize, Serialize};

use std::fs::File;
use std::collections::HashMap;
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
    fields: Vec<HWStructField>,
}

#[derive(Serialize, Deserialize, Default)]
enum HWDefineType {
    #[default]
    UNKNOWN,
    VALUE,
    VALUE2,
}

#[derive(Serialize, Deserialize, Default)]
struct HWDefine {
    hwtype: HWDefineType,
    val: String,
    // for : sepearated values
    val2: String,
}

#[derive(Serialize, Deserialize, Default)]
struct HWJson {
    version: String,
    defines: HashMap<String, HWDefine>,
    structs: HashMap<String, HWStruct>,
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
    UNKNOWN,
    VALUE,
    VALUE2,
    STRUCT,
    TYPEDEF,
}

#[derive(Serialize, Deserialize, Default)]
struct CTypes {
    ctype: CType,
    val: String,
    // for : sepearated values
    val2: String,
    is_anon_struct: bool,
    // for structs
    fields: Vec<CStructField>,
}

#[derive(Serialize, Deserialize, Default)]
struct CJson {
    version: String,
    types: HashMap<String, CTypes>,
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
fn handle_record(base_offset: usize, newfields: &mut Vec<HWStructField>, record: Entity) {
    for fld in record.get_type().unwrap().get_fields().unwrap() {
	let fld_type = fld.get_type().unwrap();
	let this_base_offset = base_offset + fld.get_offset_of_field().unwrap();

	if fld_type.get_kind() == TypeKind::Record {
	    handle_record(this_base_offset, newfields, fld);
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

	newfields.push(HWStructField {
	    name: fld.get_display_name().unwrap(),
	    val_type: valname,
	    start: this_base_offset as u32,
	    size: size as u32,
	    group_len: group_size as u32,
	})
    }
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
	let mut hwtype : HWDefineType = HWDefineType::UNKNOWN;
	// filter out the __ ones
	if name.as_bytes()[0] == '_' as u8 && name.as_bytes()[1] == '_' as u8 {
	    continue;
	}
	let tokens = define_.get_range().unwrap().tokenize();
	// All the interesting ones have 4 tokens.
	if tokens.len() != 4 {
	    continue;
	}

	let mut vstring : String = "".to_string();
	let mut vstring2 : String = "".to_string();
	if tokens[1].get_spelling() == "(" && tokens[3].get_spelling() == ")" {
	    hwtype = HWDefineType::VALUE;
	    vstring = tokens[2].get_spelling();
	} else if tokens[2].get_spelling() == ":" {
	    hwtype = HWDefineType::VALUE2;
	    vstring = tokens[1].get_spelling();
	    vstring2 = tokens[3].get_spelling();
	}

	json_output.defines.insert(name, HWDefine {
	    hwtype : hwtype,
	    val: vstring,
	    val2: vstring2,
	});
    }

    let typedefs = tu.get_entity().get_children().into_iter().filter(|e| {
        e.get_kind() == EntityKind::TypedefDecl
    }).collect::<Vec<_>>();

    for typedef in typedefs {
	let under_type = typedef.get_typedef_underlying_type().unwrap();
	if under_type.is_elaborated().unwrap() == false {
	    continue
	}

	let elab_type = under_type.get_elaborated_type().unwrap();
	if elab_type.get_kind() != TypeKind::Record {
	    continue
	}

	let mut newfields : Vec<HWStructField> = Default::default();

	let base_offset = 0;
	handle_record(base_offset, &mut newfields, elab_type.get_declaration().unwrap());

	let thisname = typedef.get_display_name().unwrap();
	json_output.structs.insert(thisname,
				   HWStruct {
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

//  println!("parsing defines");
    for define_ in defines {
	let name = define_.get_display_name().unwrap();
	let mut ctype : CType = CType::UNKNOWN;
//	println!(" {:?}", name);
	// filter out the __ ones
	if name.as_bytes()[0] == '_' as u8 && name.as_bytes()[1] == '_' as u8 {
	    continue;
	}
	let tokens = define_.get_range().unwrap().tokenize();
	// All the interesting ones have 4 tokens.
	if tokens.len() != 4 {
	    continue;
	}
//	for token in &tokens {
//	    println!("{:?}", token.get_spelling());
//	}

	let mut vstring : String = "".to_string();
	let mut vstring2 : String = "".to_string();
	if tokens[1].get_spelling() == "(" && tokens[3].get_spelling() == ")" {
	    ctype = CType::VALUE;
	    vstring = tokens[2].get_spelling();
	} else if tokens[2].get_spelling() == ":" {
	    ctype = CType::VALUE2;
	    vstring = tokens[1].get_spelling();
	    vstring2 = tokens[3].get_spelling();
	}

	json_output.types.insert(name, CTypes {
	    ctype: ctype,
	    val: vstring,
	    val2: vstring2,
	    is_anon_struct: false,
	    fields: Default::default(),
	});
    }

//  println!("parsing structs");
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
	    if wrapped_size != None {
		this_size = wrapped_size.unwrap();
		if this_size > 0 {
		    is_array = true;
		}
	    }
	    newfields.push(CStructField {
		name: field.get_name().unwrap(),
		is_array : is_array,
		size : this_size as u32,
		ftype: field.get_type().unwrap().get_display_name(),
		is_aligned: is_aligned,
		alignment: alignment as u32,
	    });
        }

	json_output.types.insert(
	    struct_.get_name().unwrap(),
	    CTypes {
		ctype: CType::STRUCT,
		fields: newfields,
		is_anon_struct: struct_.is_anonymous(),
		val: "".to_string(),
		val2: "".to_string(),
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
		ctype: CType::TYPEDEF,
		val: typedef.get_typedef_underlying_type().unwrap().get_display_name(),
		val2: "".to_string(),
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
    "src/nvidia/arch/nvalloc/common/inc/gsp/"
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
	println!("{:?}", newpath);
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

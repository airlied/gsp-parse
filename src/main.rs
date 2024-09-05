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
    isint: u32,
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
enum FieldType {
    Member,
    UnionStart,
    UnionEnd,
    StructStart,
    StructEnd,
}

#[derive(Serialize, Deserialize)]
struct CStructField {
    fldtype: FieldType,
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
    } else if fld_type.is_integer() {
	size = fld_type.get_sizeof().unwrap() * 8;
    }
    size
}

// recursive function that handles records inside records.
// used for handling union/struct nesting
fn handle_record(base_offset: usize,
		 newfields: &mut Vec<HWStructField>,
		 record_fields: Vec<Entity>,
		 name_prefix: &str) -> usize {
    let mut end_offset = base_offset;
    for fld in record_fields {
	let fld_type = fld.get_type().unwrap();
	let this_base_offset = base_offset + fld.get_offset_of_field().unwrap();

	if fld_type.is_elaborated().unwrap() {
	    if fld_type.get_elaborated_type().unwrap().get_kind() == TypeKind::Record {
		handle_record(this_base_offset, newfields,
			      fld_type.get_elaborated_type().unwrap().get_fields().unwrap(), &(fld.get_display_name().unwrap() + "_"));
		end_offset += fld_type.get_elaborated_type().unwrap().get_sizeof().unwrap();
		continue;
	    }
	}
	if fld_type.get_kind() == TypeKind::Record {
	    handle_record(this_base_offset, newfields, fld.get_type().unwrap().get_fields().unwrap(), "");//fld_type.get_display_name());
	    end_offset += fld.get_type().unwrap().get_sizeof().unwrap();
	    continue;
	}

	let mut size: usize;
	let mut group_size: usize = 0xffffffff;
	let mut valname = "".to_string();
	let mut isint = 1;
	if fld_type.get_kind() == TypeKind::ConstantArray {
	    let elem_type = fld_type.get_element_type().unwrap();
	    group_size = fld_type.get_size().unwrap();
	    size = get_type_size(elem_type);
	    valname = elem_type.get_display_name();
	    if elem_type.is_integer() == false {
		if elem_type.is_elaborated().unwrap() {
		    let elab_type = elem_type.get_elaborated_type().unwrap();
		    if elab_type.get_kind() == TypeKind::Typedef {
			if elab_type.get_canonical_type().get_kind() == TypeKind::Record {
			    let canon_type = elab_type.get_canonical_type().get_declaration().unwrap();

			    if elab_type.is_integer() == false {
				isint = 0;
			    }
			    valname = canon_type.get_display_name().unwrap();
			} else {
			    valname = elem_type.get_display_name();
			}
		    } else {
			valname = elem_type.get_display_name();
		    }
		} else {
		    valname = elem_type.get_display_name();
		}
	    }
	} else {
	    size = get_type_size(fld_type);
	    if fld_type.is_integer() == false {
		if fld_type.is_elaborated().unwrap() {
		    let elab_type = fld_type.get_elaborated_type().unwrap();

		    if elab_type.get_kind() == TypeKind::Typedef {
			if elab_type.get_canonical_type().get_kind() == TypeKind::ConstantArray {
			    let canon_type = elab_type.get_canonical_type();
			    let elem_type = canon_type.get_element_type().unwrap().get_canonical_type();
			    group_size = canon_type.get_size().unwrap();
			    size = get_type_size(elem_type);
			    valname = elem_type.get_display_name();
			} else if elab_type.get_canonical_type().get_kind() == TypeKind::Record {
			    let canon_type = elab_type.get_canonical_type().get_declaration().unwrap();

			    if elab_type.is_integer() == false {
				isint = 0;
			    }
			    valname = canon_type.get_display_name().unwrap();
			} else {
			    valname = fld_type.get_display_name();
			}
		    } else {
			valname = fld_type.get_display_name();
		    }
		} else {
		    valname = fld_type.get_display_name();
		}
	    }
	}

	end_offset = this_base_offset + size;

	newfields.push(HWStructField {
	    name: name_prefix.to_owned() + &fld.get_display_name().unwrap(),
	    val_type: valname,
	    start: this_base_offset as u32,
	    size: size as u32,
	    group_len: group_size as u32,
	    isint,
	})
    }
    end_offset
}

// recursive function that handles records inside records.
// used for handling union/struct nesting
fn handle_c_parser_record(newfields: &mut Vec<CStructField>,
			  record_fields: Vec<Entity>,
			  name_prefix: &str) -> usize {
//    println!("{:?}", record_fields);
    for fld in record_fields {
	let mut fld_type = fld.get_type().unwrap();
	let mut incomplete_array = false;
	let mut array_size = 0xffffffff;
	let mut valname = "".to_string();
	let mut is_aligned = false;
	let mut aligned_val = 0;
	
//	println!("{:?} {:?}", fld_type, fld_type.get_declaration());
	if fld_type.get_kind() == TypeKind::IncompleteArray {
	    //	    println!("{:?}", fld_type.get_element_type());
	    
	    fld_type = fld_type.get_element_type().unwrap();
	    incomplete_array = true;
	    array_size = 0;
	    
	}

	if fld_type.get_kind() == TypeKind::ConstantArray {
	    array_size = fld_type.get_size().unwrap();
	    fld_type = fld_type.get_element_type().unwrap();
	    valname = fld_type.get_display_name();
	}

	if fld_type.is_elaborated().unwrap() {
	    if fld_type.get_elaborated_type().unwrap().get_kind() == TypeKind::Record {
		if fld_type.get_elaborated_type().unwrap().get_declaration().unwrap().get_kind() == EntityKind::UnionDecl {
		    newfields.push(CStructField {
			fldtype: FieldType::UnionStart,
			ftype: "".to_string(),
			name: fld.get_display_name().unwrap(),
			is_array: false,
			size: 0,
			is_aligned: false,
			alignment: 0
		    });
		}
		if fld_type.get_elaborated_type().unwrap().get_declaration().unwrap().get_kind() == EntityKind::StructDecl {
		    newfields.push(CStructField {
			fldtype: FieldType::StructStart,
			ftype: "".to_string(),
			name: fld.get_display_name().unwrap(),
			is_array: false,
			size: 0,
			is_aligned: false,
			alignment: 0
		    });
		}

		handle_c_parser_record(newfields,
				       fld_type.get_elaborated_type().unwrap().get_fields().unwrap(), "");
		if fld_type.get_elaborated_type().unwrap().get_declaration().unwrap().get_kind() == EntityKind::StructDecl {
		    newfields.push(CStructField {
			fldtype: FieldType::StructEnd,
			ftype: "".to_string(),
			name: fld.get_display_name().unwrap(),			
			is_array: incomplete_array || array_size != 0xffffffff,
			size: array_size as u32,
			is_aligned: false,
			alignment: 0
		    });
		}
		if fld_type.get_elaborated_type().unwrap().get_declaration().unwrap().get_kind() == EntityKind::UnionDecl {
		    newfields.push(CStructField {
			fldtype: FieldType::UnionEnd,
			ftype: "".to_string(),
			name: fld.get_display_name().unwrap(),
			is_array: incomplete_array || array_size != 0xffffffff,
			size: array_size as u32,
			is_aligned: false,
			alignment: 0
		    });
		}
		continue;
	    }
	}
	if fld_type.get_kind() == TypeKind::Record {
	    if fld_type.get_declaration().unwrap().get_kind() == EntityKind::UnionDecl {
		newfields.push(CStructField {
		    fldtype: FieldType::UnionStart,
		    ftype: "".to_string(),
		    name: fld.get_display_name().unwrap(),
		    is_array: false,
		    size: 0,
		    is_aligned: false,
		    alignment: 0
		});
	    }
	    if fld_type.get_declaration().unwrap().get_kind() == EntityKind::StructDecl {
		newfields.push(CStructField {
		    fldtype: FieldType::StructStart,
		    ftype: "".to_string(),
		    name: fld.get_display_name().unwrap(),					    
		    is_array: false,
		    size: 0,
		    is_aligned: false,
		    alignment: 0
		});
	    }
	    handle_c_parser_record(newfields, fld.get_type().unwrap().get_fields().unwrap(), "");//fld_type.get_display_name());
	    if fld_type.get_declaration().unwrap().get_kind() == EntityKind::StructDecl {
		newfields.push(CStructField {
		    fldtype: FieldType::StructEnd,
		    ftype: "".to_string(),
		    name: "".to_string(),
		    is_array: incomplete_array || array_size != 0xffffffff,
		    size: array_size as u32,		    
		    is_aligned: false,
		    alignment: 0
		});
	    }
	    if fld_type.get_declaration().unwrap().get_kind() == EntityKind::UnionDecl {
		newfields.push(CStructField {
		    fldtype: FieldType::UnionEnd,
		    ftype: "".to_string(),
		    name: "".to_string(),
		    is_array: incomplete_array || array_size != 0xffffffff,
		    size: array_size as u32,		    		    
		    is_aligned: false,
		    alignment: 0
		});
	    }
	    continue;
	}

	let mut size: usize;
	let mut isint = 1;

	size = get_type_size(fld_type);
	if fld.has_attributes() {
	    let aligned = fld.get_children().into_iter().filter(|attr| {
		attr.get_kind() == EntityKind::AlignedAttr
	    }).collect::<Vec<_>>();

	    if (aligned.len() > 0) {
		// Finding alignment with libclang is difficult, I tried tokenise
		// but failed.
		// Assume 8 here
		is_aligned = true;
		aligned_val = 8;
	    }
	}
	if fld_type.is_integer() == false {
	    if fld_type.is_elaborated().unwrap() {
		let elab_type = fld_type.get_elaborated_type().unwrap();

		if elab_type.get_kind() == TypeKind::Typedef {
		    if elab_type.get_canonical_type().get_kind() == TypeKind::ConstantArray {
			let canon_type = elab_type.get_canonical_type();
			let elem_type = canon_type.get_element_type().unwrap().get_canonical_type();
			array_size = canon_type.get_size().unwrap();
			size = get_type_size(elem_type);
			valname = elem_type.get_display_name();
		    } else if elab_type.get_canonical_type().get_kind() == TypeKind::Record {
			let canon_type = elab_type.get_canonical_type().get_declaration().unwrap();

			if elab_type.is_integer() == false {
			    isint = 0;
			}
//			valname = canon_type.get_display_name().unwrap();
			valname = elab_type.get_display_name();
		    } else {
			valname = fld_type.get_display_name();
		    }
		} else {
		    valname = fld_type.get_display_name();
		}
	    } else {
		valname = fld_type.get_display_name();
	    }
	}
    
	newfields.push(CStructField {
	    fldtype: FieldType::Member,
	    name: name_prefix.to_owned() + &fld.get_display_name().unwrap(),
	    ftype: valname,
	    is_array: array_size != 0xffffffff,
	    size: array_size as u32,
	    is_aligned: is_aligned,
	    alignment: aligned_val,
	})
    }
    0
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
	if tokens.len() != 2 && tokens.len() != 4 && tokens.len() != 6 && tokens.len() != 10 {
	    continue;
	}

	let mut vals: Vec<String> = Default::default();
	if tokens.len() == 10 &&
	    tokens[1].get_spelling() == "(" && tokens[9].get_spelling() == ")" &&
	    tokens[4].get_spelling() == "<<" && tokens[7].get_spelling() == "*" {
		hwtype = HWDefineType::Value;
		vals.push("(".to_owned() + &tokens[3].get_spelling() + "<<" + &tokens[5].get_spelling() + ") * " + &tokens[8].get_spelling());
	    }
	else if tokens.len() == 6 &&
	    tokens[1].get_spelling() == "(" && tokens[5].get_spelling() == ")" &&
	    tokens[3].get_spelling() == "<<" {
		hwtype = HWDefineType::Value;
		vals.push(tokens[2].get_spelling() + "<<" + &tokens[4].get_spelling());
	    }
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

	    //	    println!("{:?} {:?}", child.get_display_name(), child.get_enum_constant_value());
	    if child.get_display_name().unwrap() == "packed" {
		continue;
	    }
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
	let decl = elab_type.get_declaration().unwrap();
	handle_record(base_offset, &mut newfields, decl.get_type().unwrap().get_fields().unwrap(), "");
	let total_size = match decl.get_type().unwrap().get_sizeof() {
	    Ok(x) => { x * 8 }
	    Err(_) => { 0 }
	};

	if total_size == 0 {
	    continue;
	}
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
	if tokens.len() != 2 && tokens.len() != 4 && tokens.len() != 6 && tokens.len() != 10 {
	    continue;
	}

	let mut vals: Vec<String> = Default::default();
	if tokens.len() == 10 &&
	    tokens[1].get_spelling() == "(" && tokens[9].get_spelling() == ")" &&
	    tokens[4].get_spelling() == "<<" && tokens[7].get_spelling() == "*" {
		ctype = CType::Value;
		vals.push("(".to_owned() + &tokens[3].get_spelling() + "<<" + &tokens[5].get_spelling() + ") * " + &tokens[8].get_spelling());
	    }
	else if tokens.len() == 6 &&
	    tokens[1].get_spelling() == "(" && tokens[5].get_spelling() == ")" &&
	    tokens[3].get_spelling() == "<<" {
		ctype = CType::Value;
		vals.push(tokens[2].get_spelling() + "<<" + &tokens[4].get_spelling());
	    }	
	if tokens.len() == 2 {
	    ctype = CType::Value;
	    vals.push(tokens[1].get_spelling());
	} else if tokens[1].get_spelling() == "(" && tokens[3].get_spelling() == ")" {
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

    let enums = tu.get_entity().get_children().into_iter().filter(|e| {
	e.get_kind() == EntityKind::EnumDecl
    }).collect::<Vec<_>>();

    for tenum in enums {
	for child in tenum.get_children() {
	    if child.get_display_name().unwrap() == "packed" {
		continue;
	    }
	    json_output.types.insert(child.get_display_name().unwrap(), CTypes {
		ctype: CType::Value,
		vals: vec!(child.get_enum_constant_value().unwrap().1.to_string()),
		is_anon_struct: false,
		fields: Default::default(),
	    });
	}
	json_output.types.insert(tenum.get_display_name().unwrap(), CTypes {
	    ctype: CType::Typedef,
	    vals: vec!("u32".to_string()),
	    is_anon_struct: false,
	    fields: Default::default(),
	});
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

	let mut newfields : Vec<CStructField> = Default::default();

	let decl = elab_type.get_declaration().unwrap();
	handle_c_parser_record(&mut newfields, decl.get_type().unwrap().get_fields().unwrap(), "");
	let total_size = match decl.get_type().unwrap().get_sizeof() {
	    Ok(x) => { x * 8 }
	    Err(_) => { 0 }
	};

	if total_size == 0 {
	    continue;
	}
	let thisname = typedef.get_display_name().unwrap();
	json_output.types.insert(thisname,
				 CTypes {
				     ctype: CType::Struct,
				     vals: vec!("".to_string()),
				     is_anon_struct: false,
				     fields: newfields,
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
    "vgpu/sdk-structures.h",
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
    "src/common/inc/swref/published/",
    "src/common/shared/msgq/inc/msgq",
    "src/nvidia/inc/kernel/gpu/gsp/",
    "src/nvidia/arch/nvalloc/common/inc/",
    "src/nvidia/kernel/inc/vgpu/",
    "src/common/uproc/os/common/include/",
    "src/nvidia/generated/",
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

	    let ent = match entry {
		Err(_) => { continue; }
		Ok(x) => { x }
	    };

	    if !just_headers(&ent) {
		continue
	    }
	    let path = ent.path().to_str().unwrap();
	    println!("parsing {:?}", path);

	    if path != "/home/airlied/devel/open-gpu-kernel-modules//src/nvidia/inc/kernel/gpu/gsp/message_queue_priv.h" && path != "/home/airlied/devel/open-gpu-kernel-modules//src/nvidia/arch/nvalloc/common/inc/gsp/gsp_fw_wpr_meta.h" {
//		continue
	    }

	    if path != "/home/airlied/devel/open-gpu-kernel-modules//src/nvidia/inc/kernel/gpu/gsp/gsp_fw_heap.h" {
//		continue
	    }


	    let tu = setup_parser(&index, path, &args[2])?;
	    add_file_to_cjson(&tu, &mut cjson_output)?;
	    add_file_to_hwjson(&tu, &mut hwjson_output)?;
	}
    }

    let cjsonname = args[3].clone() + "/" + &args[1].clone() + ".json";
    let file = File::create(cjsonname)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, &cjson_output)?;
    writer.flush()?;

    let hwjsonname = args[3].clone() + "/" + &args[1].clone() + ".hw.json";
    let file = File::create(hwjsonname)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, &hwjson_output)?;
    writer.flush()?;
    Ok(())
}

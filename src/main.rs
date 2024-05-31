// Parse the NVIDIA ctrl header files and generate a complete json database of defines and structs

extern crate clang;

use clang::*;
use std::env;
use serde::{Deserialize, Serialize};

use std::fs::File;
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
    name: String,
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
    name: String,
    hw_def_type: HWDefineType,
    value: String,
    // for : sepearated values
    value2: String,
}

#[derive(Serialize, Deserialize, Default)]
struct HWJson {
    version: String,
    defines: Vec<HWDefine>,
    structs: Vec<HWStruct>,
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
    name: String,
    ctype: CType,
    value: String,
    // for : sepearated values
    value2: String,
    is_anon_struct: bool,
    // for structs
    fields: Vec<CStructField>,
}

#[derive(Serialize, Deserialize, Default)]
struct CJson {
    version: String,
    types: Vec<CTypes>,
}

fn get_type_size(fld_type: Type) -> usize {
    let mut size = 0;
    if fld_type.is_elaborated().unwrap() {
	let field_elab_type = fld_type.get_elaborated_type().unwrap();
	if field_elab_type.get_kind() == TypeKind::Typedef {
	    let field_elab_typedef = field_elab_type.get_declaration().unwrap().get_typedef_underlying_type();
	    println!("elaborated typedef {:?} {:?} {:?}", field_elab_type, field_elab_typedef, field_elab_typedef.unwrap().get_sizeof().unwrap() * 8);
	    size = field_elab_typedef.unwrap().get_sizeof().unwrap() * 8;

	}
    }
    size
}

// recursive function that handles records inside records.
// used for handling union/struct nesting
fn handle_record(base_offset: usize, newfields: &mut Vec<HWStructField>, record: Entity) {
    for fld in record.get_type().unwrap().get_fields().unwrap() {
	let this_base_offset = base_offset + fld.get_offset_of_field().unwrap();

	let fld_type = fld.get_type().unwrap();
	println!("RECFLD {:?} {:?}", fld, fld_type);

	if fld_type.get_kind() == TypeKind::Record {
	    handle_record(this_base_offset, newfields, fld);
	    continue;
	}

	let mut size: usize = 0;
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

fn add_file_to_hwjson(index: &Index, path: &str, inc_path: &String, json_output: &mut HWJson) -> std::io::Result<()> {
    // Parse a source file into a translation unit
    let mut parser = index.parser(path);
    // turn on detailed preprocessing to get defines
    let args = ["-I".to_string().to_owned() + inc_path, "-include".to_string(), "nvtypes.h".to_string()];
    parser.detailed_preprocessing_record(true);
    parser.arguments(&args);
    let tu = parser.parse().unwrap();

    // Get the declearations?
    let defines = tu.get_entity().get_children().into_iter().filter(|e| {
	e.get_kind() == EntityKind::MacroDefinition &&
        !e.is_builtin_macro() &&
        !e.is_function_like_macro()
    }).collect::<Vec<_>>();

    for define_ in defines {
	let name = define_.get_display_name().unwrap();
	let mut hwtype : HWDefineType = HWDefineType::UNKNOWN;
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

	json_output.defines.push(HWDefine {
	    name: name,
	    hw_def_type : hwtype,
	    value: vstring,
	    value2: vstring2,
	})
    }

    let structs = tu.get_entity().get_children().into_iter().filter(|e| {
        e.get_kind() == EntityKind::StructDecl
    }).collect::<Vec<_>>();
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

	println!("{:?} {:?}", typedef.get_display_name(), under_type);
	let understruct = elab_type.get_declaration().unwrap().get_canonical_entity();

	let mut newfields : Vec<HWStructField> = Default::default();

	let base_offset = 0;
	handle_record(base_offset, &mut newfields, elab_type.get_declaration().unwrap());

	let name = typedef.get_display_name();
	json_output.structs.push(HWStruct {
	    name: typedef.get_display_name().unwrap(),
	    fields: newfields,
	})
    }
    Ok(())
}

fn add_file_to_cjson(index: &Index, path: &str, inc_path: &String, json_output: &mut CJson) -> std::io::Result<()> {
    // Parse a source file into a translation unit
    let mut parser = index.parser(path);
    // turn on detailed preprocessing to get defines
    let args = ["-I".to_string().to_owned() + inc_path, "-include \"nvtypes.h\"".to_string()];
    parser.detailed_preprocessing_record(true);
    parser.arguments(&args);
    let tu = parser.parse().unwrap();

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

	json_output.types.push(CTypes {
	    name: name,
	    ctype: ctype,
	    value: vstring,
	    value2: vstring2,
	    is_anon_struct: false,
	    fields: Default::default(),
	})
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
	json_output.types.push(CTypes {
	    name: struct_.get_name().unwrap(),
	    ctype: CType::STRUCT,
	    fields: newfields,
	    is_anon_struct: struct_.is_anonymous(),
	    value: "".to_string(),
	    value2: "".to_string(),
	});
    }

    let typedefs = tu.get_entity().get_children().into_iter().filter(|e| {
        e.get_kind() == EntityKind::TypedefDecl
    }).collect::<Vec<_>>();

    // output typedef info to json
    for typedef in typedefs {
	json_output.types.push(CTypes {
	    name: typedef.get_name().unwrap(),
	    ctype: CType::TYPEDEF,
	    value: typedef.get_typedef_underlying_type().unwrap().get_display_name(),
	    value2: "".to_string(),
	    is_anon_struct: false,
	    fields: Default::default(),
	})
    }

    Ok(())
}

fn just_headers(entry: &DirEntry) -> bool {
    entry.file_name()
         .to_str()
         .map(|s| s.ends_with(".h"))
         .unwrap_or(false)
}

const PATHS: &'static [&'static str] = &[
    "src/common/sdk/nvidia/inc",
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

    let incpath = args[2].clone() + PATHS[0];
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

	    add_file_to_cjson(&index, path, &incpath, &mut cjson_output)?;
	    add_file_to_hwjson(&index, path, &incpath, &mut hwjson_output)?;
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

// Parse the NVIDIA ctrl header files and generate a complete json database of defines and structs

extern crate clang;

use clang::*;
use std::env;
use serde::{Deserialize, Serialize};

use std::fs::File;
use std::io::{BufWriter, Write};
use walkdir::{DirEntry, WalkDir};

#[derive(Serialize, Deserialize)]
struct CStructField {
    ftype: String,
    name: String,
    is_array: bool,
    size: u32,
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

fn add_file_to_json(index: &Index, path: &str, inc_path: &String, json_output: &mut CJson) -> std::io::Result<()> {

    // Parse a source file into a translation unit
    let mut parser = index.parser(path);
    // turn on detailed preprocessing to get defines
    let args = ["-I".to_string().to_owned() + inc_path];
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

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    // Acquire an instance of `Clang`
    let clang = Clang::new().unwrap();

    // Create a new `Index`
    let index = Index::new(&clang, false, false);

    let mut json_output : CJson = Default::default();

    json_output.version = args[1].clone();

    println!("{:?}", args[2]);
    for entry in WalkDir::new(&args[2]).into_iter() {
	let ent = entry.unwrap();
	if !just_headers(&ent) {
	    continue
	}
	let path = ent.path().to_str().unwrap();
	println!("parsing {:?}", path);
	add_file_to_json(&index, path, &args[2], &mut json_output)?;
    }

    let jsonname = args[1].clone() + ".json";
    let file = File::create(jsonname)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, &json_output)?;
    writer.flush()?;
    Ok(())
}

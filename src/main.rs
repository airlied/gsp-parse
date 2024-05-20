// Parse the NVIDIA ctrl header files and generate a complete json database of defines and structs

extern crate clang;

use clang::*;
use std::env;
use serde::{Deserialize, Serialize};

use std::fs::File;
use std::io::{BufWriter, Write};

#[derive(Serialize, Deserialize)]
struct CStructField {
    ftype: String,
    name: String,
}

#[derive(Serialize, Deserialize)]
struct CStruct {
    name: String,
    fields: Vec<CStructField>,
}

#[derive(Serialize, Deserialize, Default)]
enum CDefineType {
    #[default]
    UNKNOWN,
    VALUE,
    VALUE2,
}

#[derive(Serialize, Deserialize, Default)]
struct CDefine {
    name: String,
    define_type: CDefineType,
    value: String,
    // for : sepearated values
    value2: String,
}

#[derive(Serialize, Deserialize, Default)]
struct CJson {
    defines: Vec<CDefine>,
    structs: Vec<CStruct>,
}

fn add_file_to_json(index: &Index, path: &String, json_output: &mut CJson) -> std::io::Result<()> {

    // Parse a source file into a translation unit
    let mut parser = index.parser(path);
    // turn on detailed preprocessing to get defines
    parser.detailed_preprocessing_record(true);
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
	let mut define_type : CDefineType = CDefineType::UNKNOWN;
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
	    define_type = CDefineType::VALUE;
	    vstring = tokens[2].get_spelling();
	} else if tokens[2].get_spelling() == ":" {
	    define_type = CDefineType::VALUE2;
	    vstring = tokens[1].get_spelling();
	    vstring2 = tokens[3].get_spelling();
	}

	json_output.defines.push(CDefine {
	    name: name,
	    define_type: define_type,
	    value: vstring,
	    value2: vstring2,
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
	    newfields.push(CStructField {
		ftype: field.get_type().unwrap().get_display_name(),
		name: field.get_name().unwrap(),
	    });
        }
	json_output.structs.push(CStruct {
	    name: struct_.get_name().unwrap(),
	    fields: newfields,
	});
    }
    Ok(())
}
fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    // Acquire an instance of `Clang`
    let clang = Clang::new().unwrap();

    // Create a new `Index`
    let index = Index::new(&clang, false, false);

    let mut json_output : CJson = Default::default();

    for arg in &args[1..] {
	println!("parsing {:?}", arg);
	add_file_to_json(&index, arg, &mut json_output)?;
    }

    let file = File::create("out.json")?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, &json_output)?;
    writer.flush()?;
    Ok(())
}

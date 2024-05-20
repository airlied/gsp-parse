// Parse the NVIDIA ctrl header files and generate a complete json database of defines and structs

extern crate clang;

use clang::*;

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
    RANGE,
    VALUE,
}

#[derive(Serialize, Deserialize, Default)]
struct CDefine {
    name: String,
    define_type: CDefineType,
    value: String,
    range: [u32; 2],
}

#[derive(Serialize, Deserialize, Default)]
struct CJson {
    defines: Vec<CDefine>,
    structs: Vec<CStruct>,
}

fn main() -> std::io::Result<()> {
    // Acquire an instance of `Clang`
    let clang = Clang::new().unwrap();

    // Create a new `Index`
    let index = Index::new(&clang, false, false);

    let mut json_output : CJson = Default::default();
    // Parse a source file into a translation unit
    let mut parser = index.parser("examples/ctrl0073dp.h");
    // turn on detailed preprocessing to get defines
    parser.detailed_preprocessing_record(true);
    let tu = parser.parse().unwrap();

    // Get the declearations?
    let defines = tu.get_entity().get_children().into_iter().filter(|e| {
	e.get_kind() == EntityKind::MacroDefinition &&
        !e.is_builtin_macro() &&
        !e.is_function_like_macro()
    }).collect::<Vec<_>>();
    
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
	let mut range : u32 = 0;
	let mut range_end : u32 = 0;
	let mut vstring : String = "".to_string();
	if tokens[1].get_spelling() == "(" && tokens[3].get_spelling() == ")" {
	    define_type = CDefineType::VALUE;
	    vstring = tokens[2].get_spelling();
	} else if tokens[2].get_spelling() == ":" {
	    define_type = CDefineType::RANGE;
	    range = tokens[1].get_spelling().parse().unwrap();
	    range_end = tokens[3].get_spelling().parse().unwrap();	    
	}
//	for token in tokens {
//	    println!("{:?}", token.get_spelling());
//	}

	json_output.defines.push(CDefine {
	    name: name,
	    define_type: define_type,
	    value: vstring,
	    range: [range, range_end],
	})
    }

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

    let file = File::create("out.json")?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, &json_output)?;
    writer.flush()?;
    Ok(())
}

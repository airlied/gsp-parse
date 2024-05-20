use std::env;
use std::fs::File;
use std::io::{BufReader, BufRead};
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Clone)]
struct CStructField {
    ftype: String,
    name: String,
    is_array: bool,
    size: u32,
}

#[derive(Serialize, Deserialize, Default, Clone)]
enum CType {
    #[default]
    UNKNOWN,
    VALUE,
    VALUE2,
    STRUCT,
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct CTypes {
    name: String,
    ctype: CType,
    value: String,
    // for : sepearated values
    value2: String,
    // for structs
    fields: Vec<CStructField>,
}

#[derive(Serialize, Deserialize, Default)]
struct CJson {
    version: String,
    types: Vec<CTypes>,
}

fn generate_define(define: &CTypes) {
    println!("#define {} {}", define.name, define.value);
}

fn generate_define2(define: &CTypes) {
    println!("#define {} {}:{}", define.name, define.value, define.value2);
}

fn generate_struct(cstruct: &CTypes) {
    println!("typedef struct {} {{", cstruct.name);
    for field in &cstruct.fields {
	if field.is_array {
	    let fname = field.ftype.split("[").collect::<Vec<_>>()[0];
	    println!("    {}    {}[{}];", fname, field.name, field.size);
	} else {
	    println!("    {}    {};", field.ftype, field.name);
	}
    }
    println!("}} {};", cstruct.name);
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let file = File::open(args[1].clone())?;
    let reader = BufReader::new(file);
    let json_input: CJson = serde_json::from_reader(reader)?;

    let sym_list = File::open(args[2].clone())?;
    let sym_reader = BufReader::new(sym_list);

    for sym_name in sym_reader.lines() {
	let name = sym_name.unwrap();
	for ctype in &json_input.types {
	    if ctype.name == name {
		let c_def_type = ctype.clone().ctype;

		match c_def_type {
		    CType::STRUCT => generate_struct(&ctype),
		    CType::VALUE => generate_define(&ctype),
		    CType::VALUE2 => generate_define2(&ctype),
		    CType::UNKNOWN => todo!(),
		}
		println!();
		break;
	    }
	}
    }

    Ok(())
}

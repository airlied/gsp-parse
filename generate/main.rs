use std::env;
use std::fs::File;
use std::collections::HashMap;
use std::io::{BufReader, BufRead, Write};
use serde::{Deserialize, Serialize};

const SPECIAL_TYPES:  [&str;8] = ["NvU32", "NvU64", "NvU16", "NvU8", "NvBool", "char", "NvHandle", "int"];

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
    Unknown,
    Value,
    Struct,
    Typedef,
}

#[derive(Serialize, Deserialize, Default, Clone)]
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
    types: HashMap<String, CTypes>,
}

fn generate_define(out_writer: &mut File, verstr: &str, defname: &String, define: &CTypes) -> std::io::Result<()> {
    if define.vals.len() == 2 {
	writeln!(out_writer, "#define {}_{} {}:{}", defname, verstr, define.vals[0], define.vals[1])?;
    } else {
	writeln!(out_writer, "#define {}_{} {}", defname, verstr, define.vals[0])?;
    }
    Ok(())
}

fn generate_struct(out_writer: &mut File, verstr: &str, strname: &String, cstruct: &CTypes) -> std::io::Result<()> {
    writeln!(out_writer, "typedef struct {}_{} {{", strname, verstr)?;

    for field in &cstruct.fields {
	if field.is_array {
	    let fname = field.ftype.split('[').collect::<Vec<_>>()[0];
	    writeln!(out_writer, "    {}_{}    {}[{}];", fname, verstr, field.name, field.size)?;
	} else {
	    writeln!(out_writer, "    {}_{}    {};", field.ftype, verstr, field.name)?;
	}
    }
    writeln!(out_writer, "}} {}_{};", strname, verstr)?;
    Ok(())
}

fn generate_typedef(out_writer: &mut File, verstr: &str, tdname: &String, ctypedef: &CTypes) -> std::io::Result<()> {
    writeln!(out_writer, "typedef struct {}_{} {}", tdname, verstr, ctypedef.vals[0])?;
    Ok(())
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let file = File::open(args[1].clone())?;
    let reader = BufReader::new(file);
    let json_input: CJson = serde_json::from_reader(reader)?;

    let sym_list = File::open(args[2].clone())?;
    let sym_reader = BufReader::new(sym_list);

    let mut out_file = File::create(args[3].clone())?;

    let ver_str = json_input.version.replace('.', "_");
    let def_ver_str = "__NV_HEADER_".to_owned() + ver_str.as_str() + "__";
    writeln!(out_file, "/* This file is autogenerated */")?;
    writeln!(out_file, "#ifndef {}", def_ver_str)?;
    writeln!(out_file, "#define {} 1", def_ver_str)?;
    writeln!(out_file, "#define __NV_VERSION__ {}", json_input.version)?;
    writeln!(out_file)?;

    for base_type in SPECIAL_TYPES {
	writeln!(out_file, "#define {}_{} {}", base_type, ver_str, base_type)?;
    }
    writeln!(out_file)?;
    for sym_name in sym_reader.lines() {
	let name = sym_name.unwrap();
	for (cname, ctype) in &json_input.types {
	    if *cname == name {
		let c_def_type = ctype.clone().ctype;

		match c_def_type {
		    CType::Struct => generate_struct(&mut out_file, &ver_str.as_str(), &cname, &ctype),
		    CType::Value => generate_define(&mut out_file, &ver_str.as_str(), &cname, &ctype),
		    CType::Typedef => generate_typedef(&mut out_file, &ver_str.as_str(), &cname, &ctype),
		    CType::Unknown => todo!(),
		}?;
		writeln!(&out_file).unwrap();
		break;
	    }
	}
    }
    writeln!(out_file, "#endif")?;
    Ok(())
}

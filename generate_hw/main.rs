use std::env;
use std::fs::File;
use std::collections::BTreeMap;
use std::io::{BufReader, BufRead, Write};
use serde::{Deserialize, Serialize};

const SPECIAL_TYPES:  [&str;8] = ["NvU32", "NvU64", "NvU16", "NvU8", "NvBool", "char", "NvHandle", "int"];


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


#[derive(Serialize, Deserialize, Default)]
struct WantedJson {
    structs: Vec<String>,
    cmds: BTreeMap<String, Vec<String>>,
    defines: Vec<String>,
}

fn generate_define(out_writer: &mut File, verstr: &str, defname: &String, define: &HWDefine) -> std::io::Result<()> {
    if define.vals.len() == 2 {
	writeln!(out_writer, "#define {} {}:{}", defname, define.vals[0], define.vals[1])?;
    } else {
	writeln!(out_writer, "#define {} {}", defname, define.vals[0])?;
    }
    Ok(())
}

fn generate_hw_struct(out_writer: &mut File, verstr: &str, strname: &String, hwstruct: &HWStruct) -> std::io::Result<()> {
    writeln!(out_writer, "struct {} {{", strname)?;

    for field in &hwstruct.fields {
	let typestr = match field.size {
	    64 => "u64",
	    32 => "u32",
	    16 => "u16",
	    8 => "u8",
	    _ => "u32",
	};
	if field.group_len != 0xffffffff && field.group_len > 0 {
	    writeln!(out_writer, "    {} {}[{}];", typestr, field.name, field.group_len)?;
	} else {
	    writeln!(out_writer, "    {} {};", typestr, field.name)?;
	}
    }
    writeln!(out_writer, "}};")
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let file = File::open(args[1].clone())?;
    let reader = BufReader::new(file);
    let json_input: HWJson = serde_json::from_reader(reader)?;

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
//	writeln!(out_file, "#define {}_{} {}", base_type, ver_str, base_type)?;
    }
    writeln!(out_file)?;
    for sym_name in sym_reader.lines() {
	let name = sym_name.unwrap();

	for (defname, define) in &json_input.defines {
	    if *defname == name {
		match define.hwtype {
		    HWDefineType::Value => generate_define(&mut out_file, &ver_str.as_str(), &defname, define),
		    HWDefineType::Unknown => todo!(),
		}?;
		writeln!(&out_file).unwrap();
		break;
	    }
	}

	for (strname, structinfo) in &json_input.structs {
	    if *strname == name {
		generate_hw_struct(&mut out_file, &ver_str.as_str(), &strname, structinfo)?;
	    }
	}
    }
    writeln!(out_file, "#endif")?;
    Ok(())
}

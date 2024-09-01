
use std::env;
use std::fs::File;
use std::collections::BTreeMap;
use std::io::{BufReader, Write};
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

fn get_val_info(val: String) -> (u32, String) {
    if val.ends_with("ULL") {
	let mut newstr = val.clone();
	newstr.pop();
	newstr.pop();
	newstr.pop();
	newstr += "_u64";
	return (64, newstr);
    }
    if val.ends_with("U") || val.ends_with("u") {
	let mut newstr = val.clone();
	newstr.pop();
	return (32, newstr);
    }
    (32, val)
}

fn generate_define(out_writer: &mut File, defname: &String, define: &HWDefine) -> std::io::Result<()> {
    let (valsize, valstr) = get_val_info(define.vals[0].clone());
    if define.vals.len() == 2 {
	if define.vals[0] == define.vals[1] {
	    writeln!(out_writer, "pub(crate) const {}: u{} = {};", defname, valsize, valstr)?;
	} else {
	    let (valsize1, valstr1) = get_val_info(define.vals[1].clone());
	    writeln!(out_writer, "pub(crate) const {}_A: u{} = {};", defname, valsize, valstr)?;
	    writeln!(out_writer, "pub(crate) const {}_B: u{} = {};", defname, valsize1, valstr1)?;
	}
    } else {
	writeln!(out_writer, "pub(crate) const {}: u{} = {};", defname, valsize, valstr)?;
    }
    Ok(())
}

fn generate_hw_struct(out_writer: &mut File, strname: &String, hwstruct: &HWStruct) -> std::io::Result<()> {
    writeln!(out_writer, "struct {} {{", strname)?;

    for field in &hwstruct.fields {
	let typestr = match field.size {
	    64 => "u64",
	    32 => "u32",
	    16 => "u16",
	    8 => "u8",
	    _ => "u32",
	};
	if field.group_len > 0 {
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
    let sym_json: WantedJson = serde_json::from_reader(sym_reader)?;

    let mut out_file = File::create(args[3].clone())?;

    writeln!(out_file, "// AUTO GENERATED")?;
    writeln!(out_file, "#![allow(non_snake_case)]")?;
    writeln!(out_file, "#![allow(dead_code)]")?;
    writeln!(out_file, "#![allow(non_camel_case_types)]")?;
    writeln!(out_file)?;

    for sym_define in sym_json.defines {
	for (defname, define) in &json_input.defines {
	    if *defname == sym_define {
		match define.hwtype {
		    HWDefineType::Value => generate_define(&mut out_file, &defname, define),
		    HWDefineType::Unknown => todo!(),
		}?;
		break;
	    }
	}
    }

    writeln!(&mut out_file, "")?;
    for sym_struct in sym_json.structs {
	println!("{}", sym_struct);

	for (strname, structinfo) in &json_input.structs {
	    if *strname == sym_struct {
		writeln!(&mut out_file, "pub(crate) struct s_{}<'s> {{", sym_struct)?;
		writeln!(&mut out_file, "    ptr: *mut u8,")?;
		writeln!(&mut out_file, "    store: &'s mut[u8],")?;
		writeln!(&mut out_file, "}}")?;
		writeln!(&mut out_file, "")?;
		writeln!(&mut out_file, "impl<'s> s_{}<'s> {{", sym_struct)?;
		writeln!(&mut out_file, "    pub const fn str_size() -> usize {{")?;
		writeln!(&mut out_file, "        {}", structinfo.total_size / 8)?;
		writeln!(&mut out_file, "    }}")?;
		writeln!(&mut out_file, "    pub fn new(ptr: *mut u8) -> Self {{ Self {{")?;
		writeln!(&mut out_file, "        ptr,")?;
		writeln!(&mut out_file, "        store: unsafe {{ std::slice::from_raw_parts_mut(ptr, {}) }},", structinfo.total_size / 8)?;
		writeln!(&mut out_file, "    }} }}")?;
		writeln!(&mut out_file, "")?;
		for fld in &structinfo.fields {
		    let mut fld_type_name = format!("u{}", fld.size);
		    let mut fld_is_struct: bool = false;

		    if fld.size == 0 {
			continue;
		    }

		    if fld.isint == 0 {
			fld_type_name = "s_".to_owned() + &fld.val_type.clone();
			fld_is_struct = true;
		    }

		    if fld_is_struct {
			writeln!(&mut out_file, "")?;

			if fld.group_len != 0xffffffff {
			    writeln!(&mut out_file, "    pub fn new_S_{}(&mut self, idx: isize) -> s_{}<'s> {{", fld.name, fld.val_type)?;
			    writeln!(&mut out_file, "        s_{}::new(unsafe {{ self.ptr.byte_offset(idx * {} + {}) }})", fld.val_type, fld.size / 8, fld.start / 8)?;
			} else {
			    writeln!(&mut out_file, "    pub fn new_S_{}(&mut self) -> s_{}<'s> {{", fld.name, fld.val_type)?;
			    writeln!(&mut out_file, "        s_{}::new(unsafe {{ self.ptr.byte_offset({}) }})", fld.val_type, fld.start / 8)?;
			}
			writeln!(&mut out_file, "    }}")?;
			writeln!(&mut out_file, "")?;
			continue;
		    }

		    let mut fld_name = fld.name.clone();
		    if fld.name == "type" {
			fld_name = "r".to_string() + &fld.name;
		    }
		    if fld.group_len != 0xffffffff {
			writeln!(&mut out_file, "    pub fn {}(self, fld: [{}; {}]) -> Self {{", fld_name, fld_type_name, fld.group_len)?;

			writeln!(&mut out_file, "        let byte_data: Vec<u8> = fld.iter().flat_map(|&x| x.to_le_bytes()).collect();")?;
			writeln!(&mut out_file, "        self.store[{}..{}].copy_from_slice(&byte_data);", fld.start / 8, (fld.start + (fld.size * fld.group_len)) / 8)?;
			writeln!(&mut out_file, "    self }}")?;

			writeln!(&mut out_file, "    pub fn set_{}(&mut self, fld: [{}; {}]) {{", fld_name, fld_type_name, fld.group_len)?;

			writeln!(&mut out_file, "        let byte_data: Vec<u8> = fld.iter().flat_map(|&x| x.to_le_bytes()).collect();")?;
			writeln!(&mut out_file, "        self.store[{}..{}].copy_from_slice(&byte_data);", fld.start / 8, (fld.start + (fld.size * fld.group_len)) / 8)?;
			writeln!(&mut out_file, "    }}")?;

			writeln!(&mut out_file, "    pub fn get_{}(&mut self) -> [{}; {}] {{", fld_name, fld_type_name, fld.group_len)?;
			writeln!(&mut out_file, "        let mut array = [0{}; {}];", fld_type_name, fld.group_len)?;
			writeln!(&mut out_file, "        for (i, chunk) in self.store[{}..{}].chunks_exact({}).enumerate() {{", fld.start / 8, (fld.start + (fld.size * fld.group_len)) / 8, fld.size / 8)?;
			writeln!(&mut out_file, "            array[i] = {}::from_le_bytes(chunk.try_into().unwrap());", fld_type_name)?;
			writeln!(&mut out_file, "        }}")?;
			writeln!(&mut out_file, "        array")?;
			writeln!(&mut out_file, "    }}")?;

		    } else {
			writeln!(&mut out_file, "    pub fn {}(self, fld: {}) -> Self {{", fld_name, fld_type_name)?;
			writeln!(&mut out_file, "        self.store[{}..{}].copy_from_slice(&u{}::to_le_bytes(fld));", fld.start / 8, (fld.start + fld.size) / 8, fld.size)?;
			writeln!(&mut out_file, "    self }}")?;

			writeln!(&mut out_file, "")?;
			writeln!(&mut out_file, "    pub fn get_{}(&self) -> {} {{", fld_name, fld_type_name)?;
			writeln!(&mut out_file, "        u{}::from_le_bytes(self.store[{}..{}].try_into().unwrap())", fld.size, fld.start / 8, (fld.start + fld.size) / 8)?;
			writeln!(&mut out_file, "    }}")?;

			writeln!(&mut out_file, "    pub fn set_{}(&mut self, fld: {}) {{", fld_name, fld_type_name)?;
			writeln!(&mut out_file, "        self.store[{}..{}].copy_from_slice(&u{}::to_le_bytes(fld));", fld.start / 8, (fld.start + fld.size) / 8, fld.size)?;
			writeln!(&mut out_file, "    }}")?;
		    }
		}
		writeln!(&mut out_file, "}}")?;
		writeln!(&mut out_file, "")?;
	    }
	}

    }

    for cmdgroup in sym_json.cmds {
	// cmd have a general structure
	let basename : String = "NV".to_owned() + &cmdgroup.0;
	for cmd in cmdgroup.1 {
	    let cmdname = basename.clone() + "_CTRL_CMD_" + &cmd;
	    let ctrlname = basename.clone() + "_CTRL_" + &cmd;

	    for (defname, define) in &json_input.defines {
		if defname.starts_with(&cmdname) || defname.starts_with(&ctrlname) {
		    match define.hwtype {
			HWDefineType::Value => generate_define(&mut out_file, &defname, define),
			HWDefineType::Unknown => todo!(),
		    }?;
		}
	    }

	    /* find the params for this command */
	    for (strname, hwstruct) in &json_input.structs {
		if *strname == ctrlname.clone() + "_PARAMS" {
		    println!("{:?} {:?}", strname, hwstruct.total_size);
		    for fld in &hwstruct.fields {
			if fld.val_type != "" {
			    for (fldname, fldhwstruct) in &json_input.structs {
				if *fldname == fld.val_type {
				    println!("struct field {} {} {}", fldname, fld.val_type, fldhwstruct.total_size);
				}
			    }
			}
		    }
		}
	    }
	}
    }
    Ok(())
}

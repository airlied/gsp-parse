
This program has two ways to run:

1) Generate the JSON file from the NVIDIA driver:

This can be run by using:

sh src/runit.sh 535.113.01 ~/devel/open-gpu-kernel-modules/src/common/sdk/nvidia/inc/

This will replace 535.113.01.json which is a complete json representing the headers for the version you have checked out.

defines have a name, one of two types, VALUE is just a single value define,
VALUE2 is where NVIDIA use STR:STR and seprates that into two pieces which might make it easier to use

structs have a name and list of fields.

2) Take a list of type names and generate and output C header from them - these should eventually be versioned.

cargo run --bin generate -- ./535.113.01.json ./examples/nouveau_want_list.txt 535.113.01.h

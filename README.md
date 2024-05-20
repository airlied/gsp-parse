
This can be run by using:

sh src/runit.sh ~/devel/open-gpu-kernel-modules/src/common/sdk/nvidia/inc/ctrl/

This will replace out.json which is a complete json representing the headers.

defines have a name, one of two types, VALUE is just a single value define,
VALUE2 is where NVIDIA use STR:STR and seprates that into two pieces which might make it easier to use

structs have a name and list of fields.


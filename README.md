
This is a set of software to parse and generate things from the NVIDIA open gpu header files, particularly to implement interfaces to the GSP processor firmware, which has an unstable ABI.

This has two stages of software:

1. JSON generator. This parses the NVIDIA includes and generates a set of json files with the information needed for all structs/defines/etc.

2. rust module generator. This parses the json databases with a list of needed symbols and generates a set of module for use in the nova GPU driver. This might be adapted to nouveau.


The list of fw versions we care about is stored in fw_list.
The recreate_hw_json.sh will checkout the NVIDIA repo and run the parser over all of it to pull out the json files and put them in jsondb/

The recreate_rust.sh will generate a set of files in _out for use in nova eventually.

examples/nouveau_want_list.json is the lists of symbols needed to be generated.
mkdir _out/
for i in `cat fw_list`
do
	dirname=fwr`echo $i | sed -e 's/\./_/g'`
	mkdir _out/$dirname
	cargo run --bin generate_rust jsondb/$i.hw.json examples/nova_want_list.json _out/$dirname/gen.rs;
done

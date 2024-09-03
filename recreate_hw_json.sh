rm -rf jsondb
mkdir -p jsondb

mkdir -p gitrepo
cd gitrepo
if [ ! -d open-gpu-kernel-modules ]; then
    git clone https://github.com/NVIDIA/open-gpu-kernel-modules
fi
cd open-gpu-kernel-modules
git remote update
cd ..
cd ..

for i in `cat fw_list`
do
	cd gitrepo/open-gpu-kernel-modules
	git checkout -f $i
	cd ../..
	cargo run --bin json -- $i $PWD/gitrepo/open-gpu-kernel-modules/ jsondb
done

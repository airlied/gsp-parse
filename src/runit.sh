#! /bin/sh

# pass the version and the nvidia repo
# 535.113.03 ~/devel/open-gpu-kernel-modules/ <outdir>

cargo run --bin json -- $1 $2 $3


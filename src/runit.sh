#! /bin/sh

# pass the nvidia repo
# ~/devel/open-gpu-kernel-modules/src/common/sdk/nvidia/inc/ctrl/

find $1 -name "*.h" | xargs cargo run

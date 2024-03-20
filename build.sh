#!/usr/bin/env bash
set -o errexit -o nounset -o pipefail

# set current working directory to script directory to run script from everywhere
cd "$(dirname "$0")"

# This script builds all subprojects and puts all created Wasm modules in one dir
fluence module build ./effector --no-input

# To be able to publish the cid crate, we need to move wasms to the cid crate scope
mkdir -p cid/artifacts/
cp target/wasm32-wasi/release/curl_effector.wasm cid/artifacts/

cargo build --release

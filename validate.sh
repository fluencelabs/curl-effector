#!/usr/bin/env bash
set -o errexit -o nounset -o pipefail

# set current working directory to script directory to run script from everywhere
cd "$(dirname "$0")"


echo "Building the project"
# Build the project
# This script builds all subprojects and puts all created Wasm modules in one dir
fluence module build ./effector --no-input

echo "Generating CID"
mkdir -p cid/artifacts
ipfs add --offline -Q --only-hash --cid-version 1 --hash sha2-256 --chunker=size-262144 target/wasm32-wasi/release/ls_effector.wasm > cid/artifacts/cidv1

echo "Building the cid crate"
cd cid
cargo build --release
cd ..

echo "Packaging the effector"
# Pack the module
fluence module pack ./effector/ --binding-crate=./imports/ --no-input -d .

echo "Extracting the CID from the package and the crate"
cid_package="$(tar -axf ls_effector.tar.gz module.yaml -O | grep cid | cut -d' ' -f2)"
cid_crate="$(cut -d' ' -f2 < cid/artifacts/cidv1)"

if [[ "$cid_package" = "$cid_crate" ]]
then
   echo "Validated"
else
   echo "Not validated" >&2
   echo "Fluence Package CID: '${cid_package}'" >&2
   echo "Rust Crate CID: '${cid_crate}'" >&2
   exit 1
fi

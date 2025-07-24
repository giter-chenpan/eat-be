#!/bin/bash

# check if version_history.txt exists
if [ -f "version_history.txt" ]; then
    # get the last line of the file
    LAST_VERSION=$(tail -n 1 version_history.txt)
    echo "The last version was: $LAST_VERSION"
else
    echo "version_history.txt does not exist"
fi

echo "What's the new version?" 
read VERSION

echo -e "\nversion is : $VERSION"

# configure these for your environment
PKG="eatbe"                                    # cargo package name
TARGET="x86_64-unknown-linux-gnu"            # remote target
ASSETS=("Rocket.toml")  # list of assets to bundle
BUILD_DIR="target/${TARGET}/release"         # cargo build directory

## ensure target toolchain is present
rustup target add $TARGET

## solve the problem of too many open files
ulimit -n 4096


## cross-compile，require installed ziglang
cargo zigbuild --target $TARGET --release

## bundle
tar -cvzf "${VERSION}.tar.gz" "${ASSETS[@]}" -C "${BUILD_DIR}" "${PKG}"

echo "$VERSION" >> version_history.txt
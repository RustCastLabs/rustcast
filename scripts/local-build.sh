#!/bin/bash

cargo build --release
mv target/release/rustcast assets/macos/RustCast.app/Contents/MacOS/rustcast
rm -rf ~/Applications/Rustcast.app
cp -r assets/macos/Rustcast.app ~/Applications/Rustcast.app
codesign --force --deep --sign - ~/Applications/RustCast.app

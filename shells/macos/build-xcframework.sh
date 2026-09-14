#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
export MACOSX_DEPLOYMENT_TARGET=26.0
cargo build -p llamp-ffi --release --locked --manifest-path "$root/Cargo.toml"
rm -rf "$root/shells/macos/LlampFFI.xcframework"
xcodebuild -create-xcframework \
  -library "$root/target/release/libllamp_ffi.a" \
  -headers "$root/crates/llamp-ffi/include" \
  -output "$root/shells/macos/LlampFFI.xcframework"

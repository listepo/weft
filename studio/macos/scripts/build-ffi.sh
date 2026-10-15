#!/bin/sh
# Xcode's pre-build phase: builds weft-studio-ffi as a static library for Apple Silicon and puts
# it in the products folder, where the app links it. One Rust library carries both the `ws_*`
# editor and slint-bindings' `sb_*` surface, so the app holds one copy of Slint.
set -eu

# Xcode runs scripts with a minimal PATH; cargo comes from mise or rustup.
PATH="$HOME/.local/share/mise/shims:$HOME/.cargo/bin:$PATH"
repo=$(cd "$SRCROOT/../.." && pwd)

if [ "$CONFIGURATION" = Release ]; then
  profile=release
  set -- --release
else
  profile=debug
  set --
fi

cd "$repo"
cargo rustc -p weft-studio-ffi --lib --crate-type staticlib --target aarch64-apple-darwin "$@"
target=$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
# A link, not a copy: the debug library is about 280 MB.
ln -sf "$target/aarch64-apple-darwin/$profile/libweft_studio_ffi.a" "$BUILT_PRODUCTS_DIR/libweft_studio_ffi.a"

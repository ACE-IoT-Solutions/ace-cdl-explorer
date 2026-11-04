#!/usr/bin/env sh
set -eu

# Use rustup's compiler when Homebrew's rustc precedes it on PATH.
if command -v rustup >/dev/null 2>&1; then
  cdl_rustc_bin=$(rustup which rustc)
  cdl_toolchain_dir=$(dirname "$(dirname "$cdl_rustc_bin")")
  PATH="$cdl_toolchain_dir/bin:$PATH"
  export PATH
  if [ "$(uname)" = Darwin ]; then
    DYLD_LIBRARY_PATH="$cdl_toolchain_dir/lib${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
    export DYLD_LIBRARY_PATH
  fi
fi

wasm-pack build runtime --target web --out-dir ../web/src/wasm

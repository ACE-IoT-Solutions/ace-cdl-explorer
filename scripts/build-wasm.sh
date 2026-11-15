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

wasm-pack build crates/ace-cdl-wasm --target web --out-dir ../../web/src/wasm --no-opt
python3 scripts/verify-wasm-table.py web/src/wasm/ace_cdl_wasm_bg.wasm

# Remove obsolete generated module names after the runtime crate rename.
python3 - <<'PY_CLEAN'
from pathlib import Path
for name in ("ace_cdl_runtime.js", "ace_cdl_runtime.d.ts", "ace_cdl_runtime_bg.wasm", "ace_cdl_runtime_bg.wasm.d.ts"):
    Path("web/src/wasm", name).unlink(missing_ok=True)
PY_CLEAN

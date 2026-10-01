#!/usr/bin/env sh
set -eu
command -v wasm-pack >/dev/null 2>&1 || { echo "Instale wasm-pack: https://rustwasm.github.io/wasm-pack/installer/"; exit 1; }
wasm-pack build crates/phx-sql-core --release --target web --features wasm --out-dir ../../pkg
printf '\nWASM gerado em ./pkg\n'

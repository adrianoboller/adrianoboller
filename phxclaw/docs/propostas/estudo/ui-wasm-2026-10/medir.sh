#!/bin/bash
# uso: medir.sh nome  -> bindgen + wasm-opt -Oz, imprime bytes crus, gzip -9, brotli
set -e
n=$1; B=$(dirname $0); WB=$B/../ferr/wasm-bindgen-0.2.129-x86_64-unknown-linux-musl/wasm-bindgen
rm -rf $B/out/$n; mkdir -p $B/out/$n
$WB --target web --no-typescript --out-dir $B/out/$n $B/target/wasm32-unknown-unknown/release/tela-$n.wasm
w=$(ls $B/out/$n/*_bg.wasm); $B/node_modules/.bin/wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals --enable-reference-types --enable-multivalue $w -o $w.opt 2>&1 | tail -2; mv $w.opt $w
for f in $B/out/$n/*; do printf "%s cru=%d gz=%d br=%s\n" $(basename $f) $(stat -c%s $f) $(gzip -9c $f|wc -c) $( (command -v brotli >/dev/null && brotli -cZ $f|wc -c) || echo NA); done

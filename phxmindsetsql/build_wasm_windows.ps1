$ErrorActionPreference = "Stop"
if (-not (Get-Command wasm-pack -ErrorAction SilentlyContinue)) {
  Write-Error "wasm-pack não encontrado. Instale Rust e wasm-pack antes de continuar."
}
wasm-pack build crates/phx-sql-core --release --target web --features wasm --out-dir ../../pkg
Write-Host "WASM gerado em .\pkg"

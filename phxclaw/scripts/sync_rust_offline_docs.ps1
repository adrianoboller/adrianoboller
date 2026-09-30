param(
  [string]$Toolchain = "1.98.1"
)
$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Out = Join-Path $Root "knowledge\offline\rust"

if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
  throw "rustup not found. Install it from https://rustup.rs/ on a connected host."
}
if (-not (Get-Command python -ErrorAction SilentlyContinue)) {
  throw "Python is required to build the offline search index."
}

rustup toolchain install $Toolchain --profile default
rustup component add rust-docs --toolchain $Toolchain
$Sysroot = (& rustc "+$Toolchain" --print sysroot).Trim()
$DocSrc = Join-Path $Sysroot "share\doc\rust\html"
if (-not (Test-Path $DocSrc)) {
  throw "rust-docs component did not produce $DocSrc"
}

$HtmlOut = Join-Path $Out "html"
$IndexOut = Join-Path $Out "index\documents.jsonl"
if (Test-Path $HtmlOut) { Remove-Item -Recurse -Force $HtmlOut }
New-Item -ItemType Directory -Force $HtmlOut | Out-Null
New-Item -ItemType Directory -Force (Split-Path $IndexOut) | Out-Null
Copy-Item -Recurse -Force (Join-Path $DocSrc "*") $HtmlOut
python (Join-Path $Root "scripts\index_offline_docs.py") --root $HtmlOut --out $IndexOut

$Version = (& rustc "+$Toolchain" --version).Trim()
$Snapshot = @{
  toolchain = $Toolchain
  rustc = $Version
  synced_at = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
  source = "rustup component rust-docs"
  local_root = "knowledge/offline/rust/html"
} | ConvertTo-Json -Depth 4
$Snapshot | Set-Content -Encoding UTF8 (Join-Path $Out "snapshot.json")
Write-Host "Rust offline docs synchronized at: $Out"

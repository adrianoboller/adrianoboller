$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
if (-not $env:PHX_WEB_ROOT) { $env:PHX_WEB_ROOT = $PSScriptRoot }
if (-not $env:PHX_PROFILES_FILE) { $env:PHX_PROFILES_FILE = Join-Path $PSScriptRoot "gateway\profiles.json" }
Write-Host "PhxMindSetSQL v0.7 — http://127.0.0.1:8787"
Write-Host "Profiles: $env:PHX_PROFILES_FILE"
cargo run --release -p phx-sql-gateway

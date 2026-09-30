param([string]$Ref = "main")
$ErrorActionPreference = "Stop"
$Root = (Resolve-Path "$PSScriptRoot\..").Path
$Dest = Join-Path $Root "private\vendor\openclaw-rs\upstream"
$Tmp = Join-Path $env:TEMP ("phoenix-openclaw-rs-" + [guid]::NewGuid())
git clone --depth 1 --branch $Ref https://github.com/neul-labs/openclaw-rs.git $Tmp
if (-not (Test-Path (Join-Path $Tmp "LICENSE"))) { throw "REFUSED: upstream LICENSE missing" }
if (-not (Select-String -Path (Join-Path $Tmp "LICENSE") -Pattern "MIT License" -Quiet)) { throw "REFUSED: expected MIT license not found" }
Remove-Item $Dest -Recurse -Force -ErrorAction SilentlyContinue
New-Item (Split-Path $Dest) -ItemType Directory -Force | Out-Null
Move-Item $Tmp $Dest
$Commit = (git -C $Dest rev-parse HEAD).Trim()
@{source="git_clone";upstream_repository="https://github.com/neul-labs/openclaw-rs";commit=$Commit;license="MIT";license_file_present=$true;vendored_path="private/vendor/openclaw-rs/upstream"} | ConvertTo-Json | Set-Content (Join-Path (Split-Path $Dest) "UPSTREAM.lock.json")
Write-Host "VENDORED openclaw-rs commit=$Commit"

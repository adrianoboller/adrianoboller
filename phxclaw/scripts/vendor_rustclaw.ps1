param([string]$Ref = "main")
$ErrorActionPreference = "Stop"
$Root = (Resolve-Path "$PSScriptRoot\..").Path
$Dest = Join-Path $Root "private\vendor\rustclaw\upstream"
$Tmp = Join-Path $env:TEMP ("phoenix-rustclaw-" + [guid]::NewGuid())
git clone --depth 1 --branch $Ref https://github.com/Adaimade/RustClaw.git $Tmp
$License = @("LICENSE","LICENSE-MIT","LICENSE.txt","LICENSE.md") | Where-Object { Test-Path (Join-Path $Tmp $_) } | Select-Object -First 1
if (-not $License) { throw "REFUSED: RustClaw clone has no license text; keep source quarantined" }
if (-not (Select-String -Path (Join-Path $Tmp $License) -Pattern "MIT License" -Quiet)) { throw "REFUSED: RustClaw license is not verified MIT" }
Remove-Item $Dest -Recurse -Force -ErrorAction SilentlyContinue
New-Item (Split-Path $Dest) -ItemType Directory -Force | Out-Null
Move-Item $Tmp $Dest
$Commit=(git -C $Dest rev-parse HEAD).Trim()
@{source="git_clone";upstream_repository="https://github.com/Adaimade/RustClaw";commit=$Commit;license="MIT";license_file_present_in_archive=$true;license_file=$License;reuse_state="verified_mit_source";compiled_or_linked_into_phxclaw=$true;vendored_path="private/vendor/rustclaw/upstream"} | ConvertTo-Json | Set-Content (Join-Path (Split-Path $Dest) "LICENSE_STATUS.json")
Write-Host "VERIFIED RustClaw MIT source commit=$Commit; PhxClaw native compatibility layer may reuse MIT portions with notice"

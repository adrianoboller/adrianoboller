param(
  [string]$Ref = $(if ($env:CLAW_CODE_REF) { $env:CLAW_CODE_REF } else { "main" })
)
$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Dest = if ($env:PHXCLAW_CLAW_VENDOR_DIR) { $env:PHXCLAW_CLAW_VENDOR_DIR } else { Join-Path $Root "private/vendor/claw-code/upstream" }
$Repo = "https://github.com/ultraworkers/claw-code.git"
if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw "git is required" }
$Tmp = "$Dest.tmp"
if (Test-Path $Tmp) { Remove-Item -Recurse -Force $Tmp }
git clone --filter=blob:none --no-checkout $Repo $Tmp
if ($LASTEXITCODE -ne 0) { throw "git clone failed" }
git -C $Tmp checkout $Ref
if ($LASTEXITCODE -ne 0) { throw "git checkout failed" }
if (Test-Path $Dest) { Remove-Item -Recurse -Force $Dest }
Move-Item $Tmp $Dest
$Commit = (git -C $Dest rev-parse HEAD).Trim()
$Lock = [ordered]@{
  repository = "https://github.com/ultraworkers/claw-code"
  requested_ref = $Ref
  commit = $Commit
  fetched_at = [DateTime]::UtcNow.ToString("o")
  path = "private/vendor/claw-code/upstream"
  license = "MIT"
  canonical_source = "rust/"
}
$Lock | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 (Join-Path $Root "private/vendor/claw-code/UPSTREAM.lock.json")
$License = Join-Path $Dest "LICENSE"
if (Test-Path $License) { Copy-Item $License (Join-Path $Root "private/vendor/claw-code/LICENSE.upstream") -Force }
Write-Host "Claw Code vendored at $Dest"
Write-Host "commit=$Commit"

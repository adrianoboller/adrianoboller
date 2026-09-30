$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$Py = Get-Command python -ErrorAction SilentlyContinue
if (-not $Py) { $Py = Get-Command python3 -ErrorAction SilentlyContinue }
if (-not $Py) { throw 'Python 3 is required for source/bootstrap installation.' }
& $Py.Source "$Root\installer\bootstrap.py" install --yes
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host 'PhxClaw bootstrap completed.'
Write-Host 'Next: bin\phx.cmd core status'

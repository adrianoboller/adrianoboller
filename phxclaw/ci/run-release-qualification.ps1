param(
  [string]$Root = ".",
  [string]$Output = "",
  [switch]$Strict
)
if ([string]::IsNullOrWhiteSpace($Output)) { $Output = Join-Path $Root "reports/release-qualification/manual" }
$argsList = @((Join-Path $Root "tools/qualify_release.py"), $Root, "--output", $Output)
if ($Strict) { $argsList += "--strict" }
python @argsList
exit $LASTEXITCODE

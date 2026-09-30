$ErrorActionPreference = "Stop"
if (-not $env:PHXCLAW_GA_VERSION) { throw "PHXCLAW_GA_VERSION required" }
if ($null -eq $env:PHXCLAW_GA_PREVIOUS_SEQUENCE) { throw "PHXCLAW_GA_PREVIOUS_SEQUENCE required" }
python tools/build_ga_release.py . --rc $env:PHXCLAW_RC_ARCHIVE --multi-platform $env:PHXCLAW_MULTI_PLATFORM --compatibility-matrix $env:PHXCLAW_COMPAT_MATRIX --version $env:PHXCLAW_GA_VERSION --sequence $env:PHXCLAW_GA_SEQUENCE --previous-sequence $env:PHXCLAW_GA_PREVIOUS_SEQUENCE --base-url $env:PHXCLAW_GA_BASE_URL --output "dist/ga/$($env:PHXCLAW_GA_VERSION)" @args

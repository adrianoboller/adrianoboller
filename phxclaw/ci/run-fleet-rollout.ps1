param([string]$Root='.',[Parameter(Mandatory=$true)][string]$Plan,[Parameter(Mandatory=$true)][string]$Health,[string]$Out='var/fleet/state.json')
$ErrorActionPreference='Stop'
python "$Root/tools/verify_v028.py"
python "$Root/tools/evaluate_rollout.py" "$Root" --rollout-plan "$Plan" --health-evidence "$Health" --out "$Out"

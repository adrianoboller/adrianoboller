param(
  [string]$Root='.',
  [Parameter(Mandatory=$true)][string]$Rc,
  [Parameter(Mandatory=$true)][string]$Artifact,
  [Parameter(Mandatory=$true)][string]$Out,
  [Parameter(Mandatory=$true)][string]$CandidateVersion,
  [Parameter(Mandatory=$true)][ValidateSet('linux','windows','macos')][string]$Platform,
  [Parameter(Mandatory=$true)][ValidateSet('x86_64','aarch64')][string]$Architecture,
  [switch]$FirstRelease,
  [string]$PreviousVersion
)
$ErrorActionPreference='Stop'
$args=@($Root,'--rc',$Rc,'--artifact',$Artifact,'--evidence-dir',$Out,'--candidate-version',$CandidateVersion,'--platform',$Platform,'--architecture',$Architecture)
if($FirstRelease){$args += '--first-release'} else {$args += @('--previous-version',$PreviousVersion)}
python "$Root/tools/run_platform_gates.py" @args
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
python "$Root/tools/create_platform_evidence.py" $Root --artifact $Artifact --evidence-dir $Out --output "$Out/platform-evidence.json"
exit $LASTEXITCODE

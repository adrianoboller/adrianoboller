<#
.SYNOPSIS
  Instalador do WX Claude Code (Windows, PowerShell 5.1 ou 7).

.DESCRIPTION
  O mesmo caminho do instalar.sh, para quem usa Windows -- que e o publico do
  WINDEV. Confere os pre-requisitos, poe o corpus no lugar, valida o pacote,
  instala o plugin no Claude Code e, com -Serial, ativa a licenca.

  Nao mexe em nada fora de ~\.claude e ~\.wx-claude-code.

.EXAMPLE
  .\instalar.ps1
.EXAMPLE
  .\instalar.ps1 -Serial "WX2.…"
.EXAMPLE
  .\instalar.ps1 -Conferir
.EXAMPLE
  .\instalar.ps1 -Corpus C:\pacote\Help_WL_12k_Json.zip
#>
[CmdletBinding()]
param(
  [string]$Raiz = $PSScriptRoot,
  [string]$Serial = "",
  [string]$Corpus = "",
  [switch]$Conferir
)
$ErrorActionPreference = "Stop"

function Passo($t) { Write-Host ""; Write-Host $t -ForegroundColor White }
function Ok($t)    { Write-Host "  ok    $t" -ForegroundColor Green }
function Aviso($t) { Write-Host "  aviso $t" -ForegroundColor Yellow }
function Morrer($t, $dica) {
  Write-Host "  falha $t" -ForegroundColor Red
  if ($dica) { Write-Host "        $dica" -ForegroundColor Red }
  exit 1
}
function Existe($nome) { $null -ne (Get-Command $nome -ErrorAction SilentlyContinue) }

Passo "1. Pre-requisitos"
# No Windows o executavel costuma ser 'python'; 'python3' existe em alguns setups.
$py = if (Existe "python") { "python" } elseif (Existe "python3") { "python3" } else { $null }
if (-not $py) { Morrer "Python nao encontrado" "instale o Python 3.11 ou mais novo e marque 'Add to PATH'" }
$pyv = & $py -c "import sys;print('%d.%d'%sys.version_info[:2])"
& $py -c "import sys;raise SystemExit(0 if sys.version_info>=(3,11) else 1)"
if ($LASTEXITCODE -ne 0) { Morrer "Python $pyv e antigo demais" "o plugin precisa de 3.11 ou mais novo" }
Ok "$py $pyv"
if (Existe "claude") { Ok ("claude " + (claude --version 2>$null | Select-Object -First 1)) }
else { Aviso "o CLI 'claude' nao esta no PATH; a instalacao do plugin sera pulada" }

$manifesto = Join-Path $Raiz ".claude-plugin\plugin.json"
if (-not (Test-Path $manifesto)) { Morrer "$Raiz nao parece a pasta do plugin" "falta .claude-plugin\plugin.json; use -Raiz" }
$versao = (Get-Content $manifesto -Raw | ConvertFrom-Json).version
Ok "pacote encontrado: WX Claude Code $versao"

Passo "2. Corpus do Help WLanguage"
$destinoCorpus = Join-Path $Raiz "skills\conversao-wx\resources\Help_WL_12k_Json.zip"
if ($Corpus) {
  if (-not (Test-Path $Corpus)) { Morrer "corpus nao encontrado em $Corpus" }
  if (-not $Conferir) {
    New-Item -ItemType Directory -Force -Path (Split-Path $destinoCorpus) | Out-Null
    Copy-Item $Corpus $destinoCorpus -Force
    Ok "corpus copiado de $Corpus"
  } else { Ok "corpus seria copiado de $Corpus" }
}
if (Test-Path $destinoCorpus) {
  $mb = [int]((Get-Item $destinoCorpus).Length / 1MB)
  Ok "corpus no lugar ($mb MB)"
} else {
  Aviso "corpus ausente: o G0 fica DEGRADED e a semantica WLanguage some"
  Aviso "use -Corpus C:\caminho\Help_WL_12k_Json.zip (parte 2 do pacote)"
}

Passo "3. Conferencia do pacote"
# arquivo temporario proprio, apagado no fim: -Conferir nao deixa rastro
$saidaValidacao = Join-Path $env:TEMP ("wx-validacao-" + [guid]::NewGuid().ToString("N") + ".json")
& $py (Join-Path $Raiz "skills\conversao-wx\scripts\validate_plugin_bundle.py") $Raiz *> $saidaValidacao
if ($LASTEXITCODE -ne 0) {
  Get-Content $saidaValidacao | Write-Host
  Morrer "o pacote nao passou na validacao" "veja os erros acima; nao instale assim"
}
$v = Get-Content $saidaValidacao -Raw | ConvertFrom-Json
Ok ("valido: {0} skills, {1} agentes, {2} erros" -f $v.skills, $v.agents, $v.errors.Count)
if (Existe "claude") {
  claude plugin validate $Raiz *> $null
  if ($LASTEXITCODE -eq 0) { Ok "manifesto aceito pelo claude" } else { Aviso "claude plugin validate reclamou; siga com cuidado" }
}

Passo "4. Instalacao no Claude Code"
if ($Conferir) { Aviso "-Conferir: nada foi instalado" }
elseif (Existe "claude") {
  $pai = Split-Path $Raiz -Parent
  if (Test-Path (Join-Path $pai ".claude-plugin\marketplace.json")) {
    claude plugin marketplace add $pai *> $null
    claude plugin install wx-claude-code@wx-claude-code *> $null
    if ($LASTEXITCODE -eq 0) { Ok "plugin instalado do marketplace local" }
    else { Aviso "instalacao pelo marketplace falhou; use: claude --plugin-dir `"$Raiz`"" }
  } else { Aviso "marketplace.json nao esta ao lado; use: claude --plugin-dir `"$Raiz`"" }
} else {
  Aviso "sem o CLI claude; depois rode: claude plugin marketplace add <pasta-pai>; claude plugin install wx-claude-code@wx-claude-code"
}

Passo "5. Licenca"
$lic = Join-Path $Raiz "skills\conversao-wx\scripts\licenca.py"
if ($Serial -and -not $Conferir) {
  & $py $lic instalar $Serial *> $null
  if ($LASTEXITCODE -ne 0) { Morrer "serial recusado" "confira se copiou inteiro e se e desta maquina (licenca.py maquina)" }
}
$estado = & $py $lic verificar 2>$null
if ($estado -match '^valida') { Ok $estado }
else {
  Aviso "sem licenca valida: os hooks vao recusar os scripts do plugin"
  Aviso "mande ao fornecedor a saida de: $py `"$lic`" maquina"
}

Remove-Item $saidaValidacao -ErrorAction SilentlyContinue

Passo "Pronto"
Write-Host @"
  Comece por aqui, dentro da pasta do projeto de destino:

    /wx-claude-code:questionario     o questionario inteiro (bloco 0, A a M)
    /wx-claude-code:comandos         o indice dos comandos e das perguntas

  Manual: $Raiz\MANUAL.md e docs\manual-de-uso.pdf
  Ativacao por serial: $Raiz\licenca\ATIVACAO.md
"@

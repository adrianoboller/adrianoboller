# Prova do P2P do phxvpn num Windows REAL (o que o Linux e o Wine nao provam:
# o driver TAP-Windows6, o netsh e o tunel de verdade).
#
# Requisitos: OpenVPN 2.6+ instalado com o componente TAP-Windows6 (vem
# marcado), PowerShell COMO ADMINISTRADOR, phxvpn.exe nesta pasta, e um Linux
# (ou outro Windows) na mesma LAN rodando o outro lado.
#
# Uso:   .\prova-windows.ps1 -Convite "phxvpn1...." -IpDoAnfitriao 10.78.0.1
param(
    [Parameter(Mandatory = $true)][string]$Convite,
    [Parameter(Mandatory = $true)][string]$IpDoAnfitriao
)
$ErrorActionPreference = "Stop"
$exe = Join-Path $PSScriptRoot "phxvpn.exe"

Write-Host "1. Autoteste (vetores oficiais) neste Windows"
& $exe cmd /modo:ferramentas /comando:AUTOTESTE

Write-Host "2. Adaptador TAP do phxvpn (tapctl.exe do OpenVPN)"
& $exe p2p placa --interface phxvpn

Write-Host "3. Aceitar o convite (a senha da rede e pedida)"
& $exe p2p entrar $Convite
$rede = (Get-ChildItem *.p2p | Select-Object -First 1).BaseName

Write-Host "3b. O arquivo da rede e so do dono (ACL protegida, uma entrada)"
$acl = Get-Acl "$rede.p2p"
$eu = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
$regras = @($acl.Access)
$acl.Access | Format-Table IdentityReference, FileSystemRights -AutoSize
if ($regras.Count -ne 1 -or $regras[0].IdentityReference.Value -ne $eu -or -not $acl.AreAccessRulesProtected) {
    Write-Host "PROVA FALHOU: o arquivo da rede nao e so de $eu"; exit 1
}

Write-Host "4. Ligar o P2P em segundo plano"
$p = Start-Process -FilePath $exe -ArgumentList "p2p ligar --rede $rede" -PassThru -NoNewWindow
Start-Sleep -Seconds 8

Write-Host "5. Ping pelo tunel ate o anfitriao"
$ok = Test-Connection -ComputerName $IpDoAnfitriao -Count 4 -Quiet
Stop-Process -Id $p.Id
if ($ok) { Write-Host "PROVA OK: o tunel P2P passou no Windows" } else { Write-Host "PROVA FALHOU"; exit 1 }

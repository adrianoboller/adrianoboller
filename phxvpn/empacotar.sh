#!/usr/bin/env bash
# Pacotes do phxvpn, gerados sempre por este roteiro -- pacote montado a mao
# e pacote que ninguem consegue refazer igual.
#
#   ./empacotar.sh [pasta-de-saida]      (padrao: ./pacotes)
#
# Sai:
#   phxvpn_<versao>_amd64.deb       Linux (Debian/Ubuntu): /usr/bin/phxvpn{,cmd}
#   phxvpn-<versao>-x64.msi         Windows: Arquivos de Programas + atalho
#                                   (sem por no PATH: o wixl nao conhece a
#                                   tabela Environment do MSI)
#   phxvpn-<versao>-windows-x64.zip Windows sem instalador
#   SHA256SUMS
#
# Pede: cargo com o alvo x86_64-pc-windows-gnu (MinGW), dpkg-deb, wixl, zip.
set -euo pipefail
AQUI=$(cd "$(dirname "$0")" && pwd)
SAIDA=$(mkdir -p "${1:-$AQUI/pacotes}" && cd "${1:-$AQUI/pacotes}" && pwd)
VERSAO=$(grep -m1 '^version' "$AQUI/Cargo.toml" | cut -d'"' -f2)
# Fixo para sempre: e por ele que o Windows reconhece a versao nova como
# atualizacao da velha, e nao como um segundo programa.
UPGRADE_CODE=51f0706e-bb34-4d51-9350-e122666fa01e
TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT

echo "== compilando $VERSAO (Linux e Windows, release)"
(cd "$AQUI" && cargo build --release -q && cargo build --release -q --target x86_64-pc-windows-gnu)
LIN=$AQUI/target/release
WIN=$AQUI/target/x86_64-pc-windows-gnu/release

echo "== .deb"
D=$TMP/deb
install -Dm755 "$LIN/phxvpn" "$D/usr/bin/phxvpn"
install -Dm755 "$LIN/phxvpncmd" "$D/usr/bin/phxvpncmd"
install -Dm644 "$AQUI/README.md" "$D/usr/share/doc/phxvpn/README.md"
install -Dm644 "$AQUI/docs/PHXVPN.md" "$D/usr/share/doc/phxvpn/PHXVPN.md"
install -d "$D/DEBIAN"
TAM=$(du -sk "$D/usr" | cut -f1)
cat > "$D/DEBIAN/control" <<EOF
Package: phxvpn
Version: $VERSAO
Architecture: amd64
Maintainer: phxvpn <phxvpn@localhost>
Installed-Size: $TAM
Section: net
Priority: optional
Recommends: openvpn (>= 2.6)
Suggests: postgresql, linux-tools-generic
Description: redes virtuais no estilo Radmin, P2P ou sobre OpenVPN
 Criar rede e entrar na rede com nome e senha: modo P2P (Noise, sem
 servidor) ou modo servidor (OpenVPN + PostgreSQL), com USB pela rede.
 Um binario so, sem dependencias de biblioteca.
 Servico do sistema: phxvpn servico instalar painel|repasse|p2p.
EOF
DEB=$SAIDA/phxvpn_${VERSAO}_amd64.deb
dpkg-deb --root-owner-group --build "$D" "$DEB" >/dev/null

echo "== .zip (Windows)"
Z=$TMP/zip/phxvpn-$VERSAO
mkdir -p "$Z"
cp "$WIN/phxvpn.exe" "$WIN/phxvpnw.exe" "$WIN/phxvpncmd.exe" "$AQUI/prova-windows.ps1" "$Z/"
cp "$AQUI/README.md" "$Z/LEIA-ME.md"
ZIP=$SAIDA/phxvpn-$VERSAO-windows-x64.zip
rm -f "$ZIP"; (cd "$TMP/zip" && zip -qr "$ZIP" "phxvpn-$VERSAO")

echo "== .msi (Windows)"
cp "$AQUI/README.md" "$TMP/LEIA-ME.md"
cat > "$TMP/phxvpn.wxs" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="phxvpn" Language="1046" Codepage="1252" Version="$VERSAO"
           Manufacturer="phxvpn" UpgradeCode="$UPGRADE_CODE">
    <Package InstallerVersion="500" Compressed="yes" InstallScope="perMachine"
             Description="phxvpn $VERSAO" Comments="Redes virtuais no estilo Radmin" />
    <Media Id="1" Cabinet="phxvpn.cab" EmbedCab="yes" />
    <MajorUpgrade DowngradeErrorMessage="Uma versao mais nova do phxvpn ja esta instalada." />
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="INSTALLDIR" Name="phxvpn">
          <Component Id="Binarios" Guid="*" Win64="yes">
            <File Id="phxvpn.exe" Source="$WIN/phxvpn.exe" KeyPath="yes" />
            <File Id="phxvpnw.exe" Source="$WIN/phxvpnw.exe" />
            <File Id="phxvpncmd.exe" Source="$WIN/phxvpncmd.exe" />
            <File Id="LEIAME" Name="LEIA-ME.md" Source="$TMP/LEIA-ME.md" />
          </Component>
        </Directory>
      </Directory>
      <Directory Id="ProgramMenuFolder">
        <Directory Id="MenuPhxvpn" Name="phxvpn">
          <Component Id="Atalho" Guid="*" Win64="yes">
            <Shortcut Id="AtalhoMesa" Name="phxvpn" Description="Redes virtuais"
                      Target="[INSTALLDIR]phxvpnw.exe" WorkingDirectory="INSTALLDIR" />
            <RemoveFolder Id="MenuPhxvpn" On="uninstall" />
            <RegistryValue Root="HKLM" Key="Software\\phxvpn" Name="instalado" Type="integer"
                           Value="1" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>
    <Feature Id="Tudo" Level="1">
      <ComponentRef Id="Binarios" />
      <ComponentRef Id="Atalho" />
    </Feature>
  </Product>
</Wix>
EOF
MSI=$SAIDA/phxvpn-$VERSAO-x64.msi
wixl -a x64 -o "$MSI" "$TMP/phxvpn.wxs"

(cd "$SAIDA" && sha256sum "$(basename "$DEB")" "$(basename "$MSI")" "$(basename "$ZIP")" > SHA256SUMS)
echo "== pronto em $SAIDA"
(cd "$SAIDA" && ls -l "$(basename "$DEB")" "$(basename "$MSI")" "$(basename "$ZIP")" | awk '{print $5, $9}')

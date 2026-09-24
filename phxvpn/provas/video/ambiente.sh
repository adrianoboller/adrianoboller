#!/bin/bash
# Grava as tres janelas do video: tres computadores em `ip netns` numa LAN
# virtual, cada um com a sua janela do phxvpn e o tunel P2P de verdade.
# USB: sysfs de mentira (sem pendrive no conteiner); o que o KERNEL faria --
# trocar o driver no bind, marcar a porta usada no attach -- este roteiro faz.
#
# Uso (root): ./ambiente.sh     -> brutos e cenas em /var/tmp/phx-demo
set -u
U=$(cd "$(dirname "$0")" && pwd)
BIN=${PHXVPN_BIN:-$U/../../target/debug/phxvpn}
D=/var/tmp/phx-demo
rm -rf "${D:?}"; mkdir -p $D
limpar() { for n in dA dB dC; do ip netns pids $n 2>/dev/null | xargs -r kill 2>/dev/null; ip netns del $n 2>/dev/null; done; ip link del pxdemo 2>/dev/null; }
limpar
ip link add pxdemo type bridge; ip link set pxdemo up
i=1; for n in dA dB dC; do
  ip netns add $n; ip -n $n link set lo up
  ip link add v$n type veth peer name eth0 netns $n; ip link set v$n master pxdemo up
  ip -n $n addr add 192.168.77.$i/24 dev eth0; ip -n $n link set eth0 up; i=$((i+1))
done
# sysfs: A tem o pendrive; B e C tem o controlador virtual livre.
F=$D/sysA; DV=$F/bus/usb/devices/1-1; mkdir -p $DV/1-1:1.0 $F/bus/usb/drivers/usbip-host $F/bus/usb/drivers/usb-storage
ln -s $F/bus/usb/drivers/usb-storage $DV/driver
for kv in busnum=1 devnum=5 speed=480 idVendor=0781 idProduct=5583 bcdDevice=0100 bDeviceClass=00 bDeviceSubClass=00 bDeviceProtocol=00 bConfigurationValue=1 bNumConfigurations=1 bNumInterfaces=1 usbip_status=1 "product=Ultra Fit" manufacturer=SanDisk; do echo "${kv#*=}" > "$DV/${kv%%=*}"; done
echo 08 > $DV/1-1:1.0/bInterfaceClass
for q in B C; do V=$D/sys$q/devices/platform/vhci_hcd.0; mkdir -p $V
  printf 'hub port sta spd dev      sockfd local_busid\nhs  0000 004 000 00000000 000000 0-0\nhs  0001 004 000 00000000 000000 0-0\n' > $V/status; done
for q in a b c; do Q=${q^^}
  mkdir -p $D/pasta-$q
  ip netns exec d$Q env PHXVPN_SYSFS=$D/sys$Q $BIN mesa --pasta $D/pasta-$q --porta 8490 --sem-janela > $D/mesa-$q.log 2>&1 &
done
sleep 1.5
for q in a b c; do grep -o 'http://[^ ]*' $D/mesa-$q.log | head -1 > $D/url-$q; done
cd $U
for q in a b c; do Q=${q^^}; ip netns exec d$Q env DEMO=$D node $q.mjs > $D/pw-$q.log 2>&1 & done
# O "kernel" do USB
for _ in $(seq 1500); do [ -f $D/a-compartilhou ] && break; sleep 0.2; done
ln -sfn $F/bus/usb/drivers/usbip-host $DV/driver; touch $D/kernel-bind
for _ in $(seq 1500); do [ -f $D/b-anexou ] && break; sleep 0.2; done
V=$D/sysB/devices/platform/vhci_hcd.0; P=$(cut -d' ' -f1 $V/attach 2>/dev/null)
sed -i "s/^hs  000$P 004 000 00000000 000000 0-0/hs  000$P 006 003 00010005 000009 3-1/" $V/status; touch $D/kernel-attach
wait $(jobs -p | tail -3) 2>/dev/null
for _ in $(seq 600); do [ -f $D/bruto-a.webm ] && [ -f $D/bruto-b.webm ] && [ -f $D/bruto-c.webm ] && break; sleep 0.5; done
echo "--- A:"; cat $D/pw-a.log; echo "--- B:"; cat $D/pw-b.log; echo "--- C:"; cat $D/pw-c.log
echo "USB: bind=$(cat $F/bus/usb/drivers/usbip-host/bind 2>/dev/null) attach=$(cat $V/attach 2>/dev/null)"
pkill -x phxvpn; sleep 1; limpar
ls -la $D/*.webm $D/cenas-*.json 2>&1 | awk '{print $5, $9}'

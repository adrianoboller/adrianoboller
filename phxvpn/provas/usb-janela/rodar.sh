#!/bin/bash
# Exercicio da tela de USB: duas janelas em netns, tunel P2P de verdade,
# sysfs de mentira nos dois lados; o que o KERNEL faria (trocar o driver no
# bind, marcar a porta usada no attach) este roteiro faz a mao.
set -u
U=$(cd "$(dirname "$0")" && pwd); S=${SAIDA:-/tmp/phxvpn-usb-janela}; mkdir -p "$S"
BIN=${PHXVPN_BIN:-$U/../../target/debug/phxvpn}; D=/var/tmp/mesa-usb
rm -rf $D; mkdir -p $D/a $D/b $D/sysA $D/sysB
for n in muA muB; do ip netns del $n 2>/dev/null; ip netns add $n; ip -n $n link set lo up; done
ip link add mva netns muA type veth peer name mvb netns muB
ip -n muA addr add 192.168.99.1/24 dev mva; ip -n muA link set mva up
ip -n muB addr add 192.168.99.2/24 dev mvb; ip -n muB link set mvb up
# sysfs A: pendrive 1-1 no usb-storage
F=$D/sysA; DV=$F/bus/usb/devices/1-1; mkdir -p $DV/1-1:1.0 $F/bus/usb/drivers/usbip-host $F/bus/usb/drivers/usb-storage
ln -s $F/bus/usb/drivers/usb-storage $DV/driver
for kv in busnum=1 devnum=5 speed=480 idVendor=0781 idProduct=5583 bcdDevice=0100 bDeviceClass=00 bDeviceSubClass=00 bDeviceProtocol=00 bConfigurationValue=1 bNumConfigurations=1 bNumInterfaces=1 usbip_status=1 product=Ultra\ Fit manufacturer=SanDisk; do echo "${kv#*=}" > "$DV/${kv%%=*}"; done
echo 08 > $DV/1-1:1.0/bInterfaceClass
# sysfs B: vhci com portas livres
V=$D/sysB/devices/platform/vhci_hcd.0; mkdir -p $V
printf 'hub port sta spd dev      sockfd local_busid\nhs  0000 004 000 00000000 000000 0-0\nhs  0001 004 000 00000000 000000 0-0\nss  0002 004 000 00000000 000000 0-0\n' > $V/status
ip netns exec muA env PHXVPN_SYSFS=$D/sysA $BIN mesa --pasta $D/a --porta 8490 --sem-janela > $D/a.log 2>&1 &
ip netns exec muB env PHXVPN_SYSFS=$D/sysB $BIN mesa --pasta $D/b --porta 8491 --sem-janela > $D/b.log 2>&1 &
sleep 1.5
grep -o 'http://[^ ]*' $D/a.log | head -1 > $D/url-a; grep -o 'http://[^ ]*' $D/b.log | head -1 > $D/url-b
cd $U
ip netns exec muA env SAIDA=$S node a.mjs > $D/pw-a.log 2>&1 &
ip netns exec muB env SAIDA=$S node b.mjs > $D/pw-b.log 2>&1 &
# o "kernel"
for _ in $(seq 300); do [ -f $D/a-compartilhou ] && break; sleep 0.2; done
echo "bind escrito por A: $(cat $F/bus/usb/drivers/usbip-host/bind 2>/dev/null) / match: $(cat $F/bus/usb/drivers/usbip-host/match_busid 2>/dev/null) / unbind do usb-storage: $(cat $F/bus/usb/drivers/usb-storage/unbind 2>/dev/null)"
ln -sfn $F/bus/usb/drivers/usbip-host $DV/driver; touch $D/kernel-bind
for _ in $(seq 300); do [ -f $D/b-usou ] && break; sleep 0.2; done
echo "sockfd entregue ao usbip-host em A: $(cat $DV/usbip_sockfd 2>/dev/null)"
echo "attach escrito por B: $(cat $V/attach 2>/dev/null)"
P=$(cut -d' ' -f1 $V/attach 2>/dev/null)
sed -i "s/^hs  000$P 004 000 00000000 000000 0-0/hs  000$P 006 003 00010005 000009 3-1/" $V/status; touch $D/kernel-attach
for _ in $(seq 300); do [ -f $D/b-fim ] && break; sleep 0.2; done
sleep 1; echo "detach escrito por B: $(cat $V/detach 2>/dev/null)"
echo "--- A:"; cat $D/pw-a.log; echo "--- B:"; cat $D/pw-b.log; echo "--- log da mesa A (usb):"; grep -i usb $D/a.log | tail -5
pkill -f "mesa --pasta $D" ; sleep 1; for n in muA muB; do ip netns del $n; done

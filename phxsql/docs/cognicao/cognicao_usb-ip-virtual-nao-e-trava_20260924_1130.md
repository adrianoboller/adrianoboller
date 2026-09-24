# Escutar no IP virtual não prende o serviço à VPN

## O que aconteceu

O USB pela rede (`phxvpn/src/usb.rs`) usa o USB/IP, que não tem senha nem
cifra. A trava planejada era escutar a porta 3240 **só no IP virtual** da
rede, para que só quem estivesse dentro da VPN a alcançasse.

## O que eu concluí primeiro, e estava errado

1. Que escutar no IP virtual bastava: o endereço só existe na placa da VPN,
   então só pacote vindo da VPN chegaria nele. Errado: o Linux usa o modelo de
   **host fraco**. Um endereço local é aceito vindo de qualquer placa. Quem
   está na LAN só precisa pôr uma rota para `10.99.0.0/24` apontando para a
   máquina.
2. Que, sem USB e sem os módulos `usbip-host`/`vhci-hcd` no contêiner, não
   havia como provar a interoperação. Também errado: o `usbip list -r` de
   referência só usa a rede, e sobe sem módulo nenhum.

## O que a medição disse

Montei três netns: A com a placa da VPN (`phxt`, 10.99.0.1) e a da LAN
(`vA`), B na LAN, C na VPN.

- Servidor **sem** trava (Python em `10.99.0.1:3240`): B, pela LAN,
  **alcançou**.
- `phxvpn usb servir` com `SO_BINDTODEVICE` em `phxt`: B recebe
  `Connection refused (os error 111)`; C é atendido
  (`10.99.0.2 listou 0 dispositivo(s)`).
- `usbip list -r` (usbip-utils 2.0) contra o nosso servidor: lista
  `SanDisk Corp. : Ultra Fit (0781:5583)` com a interface
  `Mass Storage / SCSI / Bulk-Only (08/06/50)`. Sem o byte de preenchimento
  da interface, o mesmo teste falha (RED).

## A regra

Serviço que só pode ser alcançado por dentro da VPN se **prende à placa**
(`SO_BINDTODEVICE`), e não ao endereço. Sem conseguir prender, recusa subir.

## Como está guardado hoje

- `usb::escutar` prende o soquete na placa, e o `usb servir` exige
  `--interface`.
- A prova em netns está no `docs/PHXVPN.md` (seção USB). Ela **não é teste
  automático**, porque exige root e `ip netns`.
- **O buraco:** nenhum teste da suíte reprova quem tirar o
  `SO_BINDTODEVICE`.
- O painel do modo servidor escuta em 127.0.0.1 e não depende dessa trava.

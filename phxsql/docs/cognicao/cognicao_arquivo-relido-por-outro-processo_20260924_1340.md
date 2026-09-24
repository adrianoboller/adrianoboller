# Arquivo que outro processo relê se troca inteiro, nunca se reescreve

## O que aconteceu

Na primeira prova do phxvpn com o `openvpn` 2.6.19 de verdade, o servidor
registrava `CRL: cannot read CRL from file …/crl.pem` e, logo em seguida,
`CRL: loaded 1 CRLs`, a cada conexão depois de o painel regravar a CRL. A
revogação funcionou: `certificate revoked`, com o CN e a série da removida.

## O que eu concluí primeiro, e estava errado

1. **Que o aviso vinha de uma leitura no meio da gravação.** O `gravar` do
   painel truncava e reescrevia o próprio arquivo. Era plausível, mas foi
   desmentido: com um servidor isolado, só um `touch` no `crl.pem` (sem
   gravar nada) já produz o aviso. Ele sai igual com a CRL recodificada pelo
   `openssl crl`. É o OpenVPN ao recarregar, e é inofensivo.
2. **Que, derrubada a hipótese, não havia nada a consertar.** Também errado:
   ela caiu como causa do aviso, mas a janela existe. Um `openvpn` que lesse o
   arquivo truncado ficaria com zero CRLs, e o revogado entraria.

## O que a medição disse

- `touch crl.pem`, mesmo conteúdo: `cannot read` + `loaded 1`.
- CRL regravada pelo `openssl crl`: o mesmo par de linhas.
- Teste `gravar_troca_o_arquivo_inteiro_sem_meio`:
  - com truncar-e-escrever, quem já estava lendo viu `"novo"` (RED);
  - com temporário + renomear, viu `"VELHO-INTEIRO"`.
- No Windows (Wine), renomear por cima de um arquivo aberto dá
  `Access denied (os error 5)`: o sistema não troca o que outro processo
  segura.

## A regra

Arquivo que outro processo relê (config, CRL, lista de revogados) se grava
num temporário e se troca por renomear. No Windows, tenta de novo por um
prazo curto e, não conseguindo, dá erro — nunca volta a escrever por cima.

## Como está guardado hoje

- `painel.rs::gravar` usa temporário + `trocar` (no Windows, 20 × 50 ms), com
  o teste nos dois sistemas.
- O `prova-openvpn.sh` exige `UID set to nobody` e a recusa da removida.
- **O buraco:** o `gravar_secreto` do `comandos.rs` (arquivos da rede P2P) já
  grava por temporário, mas o `usb.rs` escreve no sysfs diretamente, e ali
  não há alternativa: o sysfs não aceita renomear.

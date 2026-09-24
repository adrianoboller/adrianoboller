# RED que falha pelo motivo errado: o binário velho que o `nobody` não executa

## O que aconteceu

`provas/ciclo-openvpn/rodar.sh` roda o mesmo roteiro com o binário novo e com
o de antes (o RED). O de antes foi compilado numa pasta de rascunho em
`/tmp/claude-0/…` (0700). No RED, os membros **nunca entraram na lista** — o
que, lido às pressas, parecia «o RED reprova, ótimo». O `openvpn.log` dizia
outra coisa: `TLS CRYPT V2 VERIFY SCRIPT ERROR … could not execute external
program`. O `tls-crypt-v2-verify` chama o próprio `phxvpn`, e o OpenVPN já
rodava como `nobody`, que não entra numa pasta 0700.

Na mesma prova, o segundo erro do mesmo tipo: o tempo de volta dos dois
membros era medido um DEPOIS do outro, então o da ana saía contado a partir
da volta do admin — «ana +0,2 s» no RED, quando ela também esperou os 60 s.

## O que eu concluí primeiro, e estava errado

Que o RED estava pronto porque reprovava. Reprovava por não conectar, não por
não avisar a saída: o número que a prova existe para medir (131 s) nem chegou
a ser medido.

## O que a medição disse

Com o binário velho copiado para uma pasta 0755: membro some da lista em
**131,1 s** (novo: 10,4 s) e os dois religam em **57,9 / 59,5 s** contados do
mesmo clique (novo: 3,2 / 3,4 s).

## A regra

RED só vale se falhar NO PONTO que a prova mede: confira no log que o velho
chegou até ali; e todo tempo de uma prova se conta do mesmo instante, nunca
do fim da espera anterior.

## Como está guardado hoje

- `phxvpn/provas/ciclo-openvpn/rodar.sh`: a volta dos dois é medida por um
  laço só, do clique; o roteiro exige `s_dois_na_lista` também no RED (lá
  aparece 9,8 s — o velho conectou antes de falhar no que importa).
- **Buraco que fica:** o roteiro não recusa sozinho um `PHXVPN_BIN_VELHO`
  numa pasta que o `nobody` não lê — o sintoma volta como `null` na lista.

## Estado

- **Estado:** INFRUTÍFERO
- **Evidência:** `phxvpn/provas/ciclo-openvpn/resultados.json`, `phxvpn/provas/ciclo-openvpn/rodar.sh`
- **Causa:** o `tls-crypt-v2-verify` executa o `phxvpn` como o usuário sem privilégio do OpenVPN, e o binário do RED estava numa pasta 0700; e duas esperas em sequência mediam o segundo tempo do fim da primeira.
- **Prevenção:** binário de RED numa pasta 0755 (`/tmp/phxvpn-velho-bin/`), conferir no log que o RED chegou ao ponto medido, e medir os tempos paralelos de um laço só a partir do mesmo instante.
- **Validado em:** 24/09/2026

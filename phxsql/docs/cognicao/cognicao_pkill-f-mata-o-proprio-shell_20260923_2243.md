# `pkill -f` casa a linha de comando do próprio shell que o chama

## O que aconteceu

Na prova ponta a ponta do Phoenix VPN (23/09/2026), o passo «derrubar o painel
e subir de novo» foi escrito como `pkill -f 'phxvpn painel'` dentro do mesmo
comando de shell que depois subia o painel. O comando inteiro morreu com
código **144** (128 + 16) antes de imprimir uma linha.

## O que eu concluí primeiro, e estava errado

Que o `-f` casaria só o processo `phxvpn painel` em execução. Casa **qualquer**
processo cuja linha de comando contenha o padrão — e o `bash -c "…pkill -f
'phxvpn painel'…"` que eu mesmo rodava contém o padrão por inteiro. O `pkill`
matou o próprio shell que o chamou.

## O que a medição disse

Trocando para `pkill -x phxvpn` (casa o **nome** do executável, não a linha),
o mesmo roteiro rodou inteiro: reinício com cofre trancado, senha mestre
errada recusada, destrancar, CLI baixando o perfil — 0 falha.

## A regra

Para derrubar um servidor de teste pelo nome, use `pkill -x <executavel>`;
`pkill -f` só com padrão que o próprio comando não carregue.

## Como está guardado hoje

Só neste arquivo. Não há conferidor de roteiro; o `zelador.sh` já não mata
processo por decisão (papel D), então o buraco é só dos roteiros de prova.

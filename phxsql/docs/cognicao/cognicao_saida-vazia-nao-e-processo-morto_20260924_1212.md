# Saída vazia não é processo morto — e o `pgrep -f` acha a si mesmo

**Estado:** INFRUTÍFERO
**Causa:** concluí que dois `claude -p --output-format json` lançados com `( … &)` tinham morrido porque o arquivo de saída estava com 0 byte e o registro não tinha vereditos novos; esse formato só escreve no fim, e os dois seguiam vivos. E duas esperas com `pgrep -f "<texto>"` nunca terminaram porque o texto estava na linha de comando da própria espera.
**Prevenção:** antes de relançar, conferir o processo por PID (`ps -p`) e não pelo arquivo de saída; em espera, casar pelo PID guardado ou por padrão que a espera não contém (`pgrep -f "^claude "`), nunca por um trecho do próprio comando.

## O que aconteceu

12:12 UTC: dei por mortos os lotes 255..427 e o de controle, e relancei os dois.
12:20, os originais gravaram 14 vereditos em duplicata. Custo em dobro
(`plugins/phxjev/bancada/custo.jsonl`, linhas marcadas `desperdicio`).

## O que eu concluí primeiro, e estava errado

Que `( … &)` não sobrevive ao fim da chamada do shell. Sobrevive. Escrevi isso
num commit (`f40d35e`) como fato.

## O que a medição disse

`ps` mostrou os PIDs 26721 e 27318 vivos às 12:2x. A duplicata virou medida
de repetibilidade: `real` e `ja_tratado` variaram no máximo 0,05; severidade
e `defeito_ativo` trocaram veredito em cima dos limiares — daí as faixas
limítrofes do `phxjev.py`.

## A regra

Arquivo vazio diz que o processo não terminou, não que morreu.

## Como está guardado hoje

Só por esta cognição: não há guarda automática contra relançar processo vivo.
E o custo da duplicata se perdeu: os dois processos escreveram no mesmo arquivo
de saída, um por cima do outro — relançar com `>` no mesmo caminho apaga a
medida do processo que ainda vive.

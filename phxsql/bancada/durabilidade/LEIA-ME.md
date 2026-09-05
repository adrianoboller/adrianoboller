# O buraco do `.reg` no fecho da janela, contra o servidor de pé

`crates/phxsql-store/examples/sonda-do-fecho.rs` achou, no `phxsql-store`
isolado: reabrir uma tabela só para sincronizá-la (sem escrever nela) sincroniza
sete arquivos e pula o `.reg` — porque `Table::abrir` lê o cabeçalho do `.reg`
com um `File::open` cru, fora do `Volumes` que faz o `fsync`, enquanto os
outros sete deixam o descritor no cache ao ler o cabeçalho deles. O que faltava
era provar isso contra o `phxsqld` de pé — o artefato que importa — nos dois
caminhos que fecham a janela em `recursos.durabilidade: por_lote`.

## Como rodar

```bash
cargo build --release
python3 bancada/durabilidade/prova-do-fecho.py
```

Sobe até três `phxsqld` (um por regime/cenário), ANEXA `strace -f -y` no PID de
cada um (nunca substitui o processo por ele) e conta `fsync` por arquivo, já
com o caminho resolvido pela própria opção `-y` — nenhum número é digitado.
Escreve `bancada/durabilidade/resultado-do-fecho.json` com a matriz bruta de
cada corrida. Leva de dez a trinta segundos, a maior parte no cenário (b)
esperando o relógio de fundo.

## O parser do traço foi consertado em 05/09, e o número diz por quê

Desde que o fecho passou a sincronizar as K tabelas **ao mesmo tempo**
(`docs/CONCORRENCIA.md` §12.6), o `strace -f` parte cada chamada concorrente em
`<unfinished ...>` mais `<... fsync resumed>` — só a primeira traz o caminho, só
a segunda traz o resultado. A expressão que este script usava exigia o `= 0` na
mesma linha do caminho e perdia **170 de 480** `fsync` num fecho de K=4, em
silêncio: a matriz saía zerada e o script culpava o relógio («o `strace` foi
solto antes de terminar»), quando o `strace` tinha visto tudo. Hoje são três
expressões e a volta se casa com a ida **pelo pid**. Ver
`docs/cognicao/cognicao_strace-parte-syscall-concorrente-e-o-parser-perde-um-terco_20260905_1120.md`.

## Os dois cenários e os dois controles

- **Controle 1**: `durabilidade: por_operacao`, uma inserção. Prova que o cano
  `strace → regex → classificador` VÊ um `.reg` sincronizado quando o código
  pede um — sem isso, "nenhum fsync no `.reg`" pode ser o defeito ou pode ser
  o instrumento cego.
- **Cenário (a)**: duas tabelas ficam sujas (`lote_operacoes: 3`, tempo
  irrelevante) e uma terceira gravação fecha a janela por CONTAGEM. A terceira
  tabela é o **controle 2**, na mesma sessão de `strace`: ela é quem disparou o
  fecho, e sincroniza os oito arquivos — enquanto as duas primeiras reabrem e
  sincronizam sete, sem o `.reg`.
- **Cenário (b)**: duas tabelas ficam sujas e ninguém escreve depois — só o
  relógio de fundo (`ligar_relogio_de_gravacao`) pode fechar a janela. Aqui não
  há tabela-gatilho para se salvar; o controle é INTERNO (os outros sete
  arquivos de cada tabela aparecem sincronizados, só o `.reg` falta — se o
  instrumento estivesse surdo para aquele PID, os sete também sairiam zerados).

## A armadilha que este script pagou, duas vezes, antes de confiar em um número

`desde` (o relógio da janela) só reseta quando uma janela FECHA — nunca por
estar ocioso, e começa a contar na SUBIDA DO SERVIDOR, não na primeira
escrita. Se o tempo real entre a subida e as duas gravações de teste (DDL,
login, o sono para o `strace` assentar, ida-e-volta de rede numa máquina
ocupada) passar de `lote_milissegundos`, a PRIMEIRA gravação fecha a própria
janela sozinha — pelo MESMO mecanismo do cenário (a), com etiqueta errada: o
que sai medido não é "o relógio fechou sem ninguém escrevendo", é outro (a).
Foi o que aconteceu com `lote_milissegundos=300`: as duas tabelas saíram com
`.reg` sincronizado, e o `strace` estava certo — a premissa do cenário é que
não era a que o nome dizia. `_tentativa_cenario_b` audita isso: exige
`gravacoes_pendentes == 2` logo após as duas escritas, e refaz com um `ms`
maior (2s, 5s, 10s) quando isso não se sustenta, em vez de publicar um número
que mede outro caminho.

A segunda: `janela.fechar()` (que zera o contador) roda ANTES do laço que de
fato reabre e sincroniza cada tabela suja, na mesma thread do relógio — não no
mesmo instante. Parar de olhar assim que `gravacoes_pendentes` volta a 0 pode
cortar o `strace` antes de a SEGUNDA tabela da lista ser sequer tocada, e aí o
"zero fsync" dela não prova nada, porque nada dela foi visto. O script espera
um segundo depois de ver o contador zerar, e ainda audita: se alguma das duas
tabelas aparecer com ZERO fsync em TODOS os arquivos (não só o `.reg`), refaz.

## O que este script NÃO mede

Tempo. Ele conta chamadas de sistema — imunes a máquina ocupada, ao contrário
de latência. Ainda assim confere `bancada/esta-medindo.sh` de cortesia; não
aborta por causa disso.

---

# A matriz real de durabilidade — SP000010

O desenho do protocolo de commit está inteiro em `docs/TRANSACOES.md` §5. O
que faltava era a **matriz**: cruzar os pontos de morte de uma escrita
transacional com os três regimes de `recursos.durabilidade`, provando cada
célula por `SIGKILL` de verdade — nunca por teste unitário.

## Como rodar

```bash
cargo build --release
python3 bancada/durabilidade/prova.py
```

Sobe um `phxsqld` de verdade na porta **7530**, mata-o com `SIGKILL` dezenas
de vezes (nunca `pkill` — só os PIDs que o próprio script criou) e escreve
`bancada/durabilidade/resultado.json` com **tudo que foi medido**. A matriz
que entra em `docs/TRANSACOES.md` §5.7 é **transcrita** desse arquivo — se um
número aparecer no documento, ele saiu daqui.

Leva de dois a quatro minutos: cada célula da matriz é uma varredura de
atrasos (não uma tentativa só), porque matar o processo no instante certo é
uma corrida — a mesma lição do §5.6 do documento.

## O método, em três peças

1. **`corrida()`** sobe o servidor, faz `BEGIN` + as operações, dispara o
   `COMMIT` numa thread e mata o processo com `SIGKILL` num atraso
   controlado *depois* de a marca `.tx` aparecer no disco. `calibrar()` mede
   antes um `COMMIT` limpo do mesmo tamanho, e é essa medida — não um chute —
   que decide a faixa de atrasos da varredura.

2. **`classificar()`** lê SÓ o relatório do arranque (`PHXSQL Recovery`) e os
   contadores que ele expõe (`achadas`, `descartadas`, `completadas`,
   `reaplicadas`, `ja_aplicadas`, `impossiveis`) para dizer em qual dos cinco
   pontos de morte a queda caiu — sem espiar o disco por fora do protocolo.
   `ja_aplicadas == N` com `reaplicadas == 0` é o ponto 4 (a passada inteira
   já tinha chegado ao disco quando a marca ainda estava lá); `reaplicadas ==
   N` com `ja_aplicadas == 0` é o ponto 2; um valor no meio é o ponto 3.

3. **A cascata (ponto 5)** verifica, depois de CADA queda, se toda filha
   aponta para o valor novo da mãe, ou toda para o velho — nunca uma mistura
   silenciosa. Quando aparece uma mistura, o critério não é "isso é um
   defeito": é "o relatório do arranque **denunciou** isso em
   `operacoes IMPOSSIVEIS`?". Perder em silêncio é o único desfecho que
   reprova a prova.

## Uma armadilha que este script já pagou

A primeira rodada acusava cascata "parcial sem aviso" toda vez que a mãe
tinha mais de 1.000 filhas — porque o `buscar` por índice **pagina**, e o
`max_linhas` padrão do servidor (1.000) cortava a resposta em silêncio. Não
era o motor: era o próprio verificador comparando "1.000" com "1.200" e
chamando isso de defeito. O `config()` deste script sobe `max_linhas` para
10.000 por causa disso — a mesma lição do CLAUDE.md sobre número que não se
mede: aqui era um número que se media **errado**, por um teto que ninguém
tinha olhado.

## O que NÃO está aqui

A queda de **energia** (não de processo). Nenhum processo em espaço de
usuário provoca isso, e a linha já está escrita em `docs/DESEMPENHO.md`
§4.12 para a exclusão — a mesma lei vale aqui: o `write` já foi entregue ao
sistema operacional em toda gravação, então uma queda de PROCESSO nunca perde
o que já tinha sido escrito. O que a queda de energia arriscaria (páginas do
`.ndx`/`.reg` que só existiam no cache do kernel) está descrito, não medido,
em `docs/TRANSACOES.md` §5.7.

# O relógio da janela de durabilidade começa na SUBIDA do servidor, não na
# primeira escrita — e isso contamina quem tenta provar o cenário do relógio

- **Quando:** 2026-09-04, 14:40
- **Onde:** `bancada/durabilidade/prova-do-fecho.py` (`_tentativa_cenario_b`),
  provando contra o `phxsqld` de pé o buraco de `fsync` no `.reg` (mesmo
  assunto de `crates/phxsql-store/tests/fecho-da-janela-sincroniza-o-reg.rs`,
  do papel G, na mesma rodada — ver "Como está guardado hoje")
- **Custo:** duas corridas completas do script (cerca de dois minutos) e uma
  reprodução manual fora dele para achar a causa, antes de eu confiar em
  qualquer número do cenário (b)

## O que aconteceu

A tarefa pedia provar dois caminhos que fecham a janela de `por_lote`: (a) uma
gravação seguinte fecha; (b) o relógio de fundo fecha, sem ninguém escrevendo.
Montei (b) com `lote_milissegundos=300`, escrevi em duas tabelas (`d`, `e`) e
fiquei quieto esperando. O resultado saiu **errado nos dois sentidos**, em
duas rodadas diferentes:

- Rodada 1: `d.reg` **e** `e.reg` apareceram sincronizados — o oposto do que
  a sonda do `phxsql-store` isolado já tinha medido.
- Rodada 2 (com `ms` maior, 5000): `d.reg` sincronizou de novo sozinho, e
  depois `e` apareceu com **zero fsync em TODOS os arquivos**, não só no
  `.reg` — um resultado que não prova nada, porque nada de `e` foi visto.

## O que eu concluí primeiro, e estava errado

Na rodada 1, minha primeira hipótese foi que o `strace` estava perdendo
eventos por causa da concorrência com um `cargo test` rodando ao lado (o
`AVISO` de `esta-medindo.sh` tinha disparado) — ou seja, culpei o
**instrumento**, exatamente o tipo de suspeita que a tarefa mandava ter, só
que no lugar errado. Quase reescrevi a captura para usar `-yy` com mais
detalhe, o que não teria mudado nada.

A causa real só apareceu numa reprodução manual, com `strace` também vendo
`write` (não só `fsync`): a MESMA thread que processou o insert em `d`
gravou a linha **e imediatamente** sincronizou os oito arquivos — inclusive
o `.reg` — no mesmo request, sem nenhum segundo insert ter disparado o
fecho por contagem (`lote_operacoes` estava em 1.000.000). O mecanismo era o
do cenário (a) (a própria gravação fecha a própria janela), não o do
cenário (b). E ele se repetia mesmo com `ms=5000`, porque o relógio da
janela (`desde`, em `Janela::nova`) começa a contar na **subida do
processo**, não na primeira escrita — e o tempo real entre subir o servidor
e chegar ao primeiro insert (DDL, login, o sono de 0,6 s para o `strace`
assentar, idas-e-voltas de rede) já bastava para estourar o `ms`, sozinho,
antes de qualquer gravação de teste acontecer.

## O que a medição disse

Isolando com `strace -f -y -ttt -e trace=fsync,write` contra um `phxsqld`
recém-subido, sem intervalo de telemetria entre as duas escritas:

```
12638 ...520325 write(...d.reg>, ...)          <- a PRIMEIRA escrita do teste
12638 ...520866 fsync(...d.trash>) = 0
12638 ...753905 fsync(...d.reg>)   = 0          <- fechou sozinha, na hora
```

`d` era a primeira gravação depois da subida — e mesmo assim fechou a
própria janela, porque `desde.elapsed()` já tinha passado do `ms` configurado
antes de a gravação acontecer. Confirmado direto pelo `config` do servidor:
`lote_milissegundos` estava correto (5000, 300 conforme o teste); o que
estava errado era supor que o relógio começava a contar do primeiro escrito.

Depois do ajuste (auditar `gravacoes_pendentes == 2` logo após as duas
escritas, sem chamada nenhuma entre elas, e repetir com `ms` maior — 2s, 5s,
10s — até essa pré-condição se sustentar), três corridas seguidas com
`ms=2000` fecharam a pré-condição já na primeira tentativa (`pendentes: 2`),
e o cenário (b) saiu limpo: `d.reg=0`, `e.reg=0`, os outros sete arquivos de
cada uma com pelo menos 1 `fsync`.

Achei ainda uma segunda armadilha, irmã da primeira: `janela.fechar()` (que
zera `gravacoes_pendentes`) roda ANTES do laço que de fato reabre e
sincroniza cada tabela suja, na mesma thread do relógio — não no mesmo
instante. Parar de olhar assim que o contador volta a 0 corta o `strace`
antes de a segunda tabela da lista ser sequer tocada (foi o que produziu o
"zero fsync em tudo" da rodada 2). Um segundo de folga depois de ver o
contador zerar, mais uma auditoria de que os outros sete arquivos de CADA
tabela aparecem sincronizados, fechou essa segunda brecha.

## A regra

**Um relógio que só reseta ao FECHAR (nunca por estar ocioso) e que começa a
contar na subida do processo transforma qualquer script de teste em um
cronômetro contra o próprio tempo de setup — e a defesa não é encurtar o
setup, é auditar a pré-condição antes de confiar no resultado.** Quem for
testar "o fecho por tempo, sem ninguém escrevendo" tem de confirmar, com um
número lido do próprio servidor (aqui, `gravacoes_pendentes`), que a escrita
de teste realmente ficou pendente — e não assumir isso pela ausência de
`fsync`, porque a ausência pode ser o cenário certo ou pode ser o cenário
errado com a etiqueta certa.

## Como está guardado hoje

`bancada/durabilidade/prova-do-fecho.py` (`_tentativa_cenario_b`) audita as
duas pré-condições e tenta de novo com folga maior em vez de publicar um
número contaminado; o comentário da função registra a armadilha. O
`LEIA-ME.md` da pasta tem uma seção própria sobre isso.

O que fica **fora** deste arquivo, de propósito: o defeito em si (fecho não
sincroniza o `.reg` de tabela reaberta) não é aprendizado novo aqui — já
estava provado no `phxsql-store` isolado por
`crates/phxsql-store/examples/sonda-do-fecho.rs` e, na mesma rodada, guardado
por teste em `crates/phxsql-store/tests/fecho-da-janela-sincroniza-o-reg.rs`
(papel G). O que este arquivo registra é só a armadilha do **instrumento de
teste em processo real**, que não existe num teste de biblioteca porque lá
não há "tempo desde a subida de um servidor" para se preocupar.

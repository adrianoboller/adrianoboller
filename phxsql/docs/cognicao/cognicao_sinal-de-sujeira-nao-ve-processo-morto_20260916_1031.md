# O sinal de sujeira em RAM nao ve o processo morto

Descoberto em 16/09/2026, 10:31, pelo papel C (DBA) ao dar parecer sobre o
pedido 258 — pular `fsync` de descritor limpo em `Volumes::sincronizar`.

## 1. O que aconteceu

O pedido 258 propoe que `Volumes::sincronizar` pare de sincronizar todo
descritor aberto e pergunte quem mudou. O registro que responderia isso existe
e e completo: `ESCRITAS_PENDENTES` (`crates/phxsql-store/src/volume.rs:151`),
um mapa por familia, marcado nos **cinco** caminhos de escrita do modulo
(`escrever` :568, `escrever_par` :594, `definir_tamanho` :614,
`escrever_so_no_principal` :358, `criar` :524) e limpo so por `sincronizar`
(:642) e `apagar_tudo` (:721). Nao ha escrita por baixo do `Volumes` no mesmo
inode, nao ha `mmap` no codigo, e nao ha `flock` — o motor ja assume um
processo escritor por diretorio.

Ainda assim, trocar os alvos por «so os pendentes» perde commit confirmado.

## 2. O que eu conclui primeiro, e estava errado

Lendo so o `volume.rs`, conclui que o registro era completo — e e — e que por
isso «alvos = pendentes» era seguro. **Certo sobre o registro, errado sobre a
pergunta.** O chamador que quebra nao esta no `volume.rs`: e o
`transacao::recuperar` (`crates/phxsql-server/src/transacao.rs:1309`), que
sincroniza num processo recem-nascido e apaga a marca `.tx` logo depois
(:1206).

A sequencia, provada por leitura:

1. Processo A, regime `por_lote`, COMMIT de uma linha: grava a marca `.tx` com
   `sync_all`, escreve o slot no `.reg` (fica no cache do nucleo), a janela nao
   fecha, a marca fica pendurada (`servidor.rs:14114-14118`). Cliente ouve OK.
2. A morre de SIGKILL. A pagina suja sobrevive no nucleo — e o pedido 186.
3. Processo B arranca, acha a marca, le o slot que ja esta la vindo do cache,
   conta como «ja aplicada», e chama `t.sincronizar()`.
4. Hoje o `.reg` entra em `abertos` (`reg.rs:2189-2196`), o `fsync` acontece, a
   pagina de A vai ao prato, e so entao a marca e apagada. Com «alvos = so
   pendentes», o registro de B nasceu vazio: zero `fsync`, e a marca some
   assim mesmo.
5. Queda de energia: commit confirmado perdido, sem bilhete.

A lista `abertos`, que parecia cache, e a **unica cobertura entre processos**
que o motor tem — o proprio `abrir_para_sincronizar` diz isso em
`volume.rs:493-496`.

## 3. O que a medicao disse

**0 `fsync` no `.reg`** naquela sequencia com «alvos = so pendentes», contra
**>= 1** hoje — contado por leitura de `reg.rs:2189-2196` e
`volume.rs:662-682`; nao cronometrado, e o parecer diz que nao cronometrou.

O alcance honesto do conserto tambem saiu medido, e e menor do que o pedido
supunha: dos **8** `fsync` de um inserir, **2** sao do `.ndx` (fora do
`Volumes`) e **1** e do `.log` (diario append por operacao, legitimamente
sujo). O teto e **8 -> 4**, que e o «4 de 8» que o `PESQUISA-FSYNC-SELETIVO.md`
§8 ja contava. E o aviso de tamanho vem do `DESEMPENHO.md` §16.2: na ablacao do
fecho, cortar 4 de 8 comprou **17%**, nao 2x.

## 4. A regra

**Pule descritor limpo so depois do primeiro `fsync` daquela familia neste
processo** — a pagina suja que um processo morto deixou no nucleo nao tem marca
em RAM nenhuma, e o primeiro `fsync` e o unico que a alcanca.

E o corolario que separa as duas perguntas: um sinal de sujeira em RAM responde
*«o que ESTE processo escreveu»*; o `fsync` pergunta *«o que ha de sujo NESTE
inode»*. As duas respostas so coincidem depois de o processo ter provado ao
disco o que herdou.

## 5. Como esta guardado hoje, e onde o buraco ficou

**Guardado como condicao a implementar**, nao como codigo: o parecer entrega a
frase do comentario e o desenho (`batizada: bool` por familia, em RAM, que so
nasce verdadeiro quando todos os `fsync` da primeira sincronizacao
confirmarem). A frente de engenharia recebeu o veredito antes de mexer no
`sincronizar`.

Tres buracos ficaram nomeados, e nenhum e dispensa silenciosa:

- **A marca vem DEPOIS do `write_all`** em todos os caminhos. Hoje e inofensivo
  porque `abertos` cobre; com «pular limpo» vira furo. O conserto entra no
  mesmo commit, no unico lugar que entrega descritor de escrita —
  `arquivo(volume, criar=true)` (`volume.rs:457`), antes do `write`.
- **O `.ndx` fica de fora, e por um motivo proprio.** Ele nao esta no `Volumes`
  e nao tem registro de processo; o `fechar()` (`ndx.rs:861-874`, chamado pelo
  `Drop` a cada pedido) grava o cabecalho com `sujo=0` **sem `fsync`**. Quem
  lesse esse byte como «sincronizado» publicaria indice atrasado em silencio, o
  unico estrago que o `FORMATO.md` (l.719-721) diz nao ter conserto. O byte 52
  significa «a arvore pode estar incompleta», e sao duas perguntas para um byte
  so.
- **A bateria de SIGKILL nao acusaria** a perda do item 2, pelo mesmo cego do
  pedido 186: o SIGKILL nao distingue «esta na midia» de «esta no cache». Por
  isso a guarda pedida conta `fsync`, e nao linhas.

Nao e petrea nova: e o **alcance** da que ja estava escrita no
`volume.rs:117-125`, «a marca SOMA `fsync`, nunca subtrai». Ela pode subtrair —
mas so a partir do instante em que o processo provou ao disco o que herdou.

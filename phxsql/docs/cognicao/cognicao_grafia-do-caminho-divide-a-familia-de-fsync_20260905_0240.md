# A grafia do caminho dividia a família de `fsync` — e a degradação não era benigna

**Descoberto em 05/09/2026, 02:40**, medindo a premissa que o comentário do
`static ESCRITAS_PENDENTES` afirmava sobre si mesmo.

## 1. O que aconteceu

O registro `ESCRITAS_PENDENTES` (`crates/phxsql-store/src/volume.rs`) é o que
faz o fecho da janela de durabilidade alcançar o volume que uma instância **já
morta** sujou. A chave dele é a família de arquivos — `diretorio/nome.ext` —, e
até esta rodada era o caminho **cru**, do jeito que quem chamou escreveu.

O comentário do próprio registro dizia, sobre esse risco:

> «Duas grafias diferentes do mesmo diretório dariam duas famílias e a marca se
> perderia — e aí a degradação é a mesma de acima, para o comportamento antigo,
> nunca para menos que ele.»

**As duas metades estavam erradas**, e a sonda
`--example sonda-do-volume-do-meio` (do orquestrador, commit `cd7a1f8`) mediu
as duas com `strace` — prova externa, e não contador interno.

## 2. O que eu concluí primeiro, e estava errado

Concluí, lendo `RegFile::sincronizar`, que a divisão da família seria **cara e
não perigosa**: pagar um `fsync` a mais aqui e ali, nunca um a menos. É o que
o comentário prometia, e a promessa é plausível — o registro é lido só para
**somar** `fsync`, jamais para pular um, e essa assimetria está escrita e está
certa.

O erro foi confundir *«o registro nunca subtrai»* com *«perder o registro não
subtrai»*. Perder o registro não subtrai do registro: **subtrai do resultado**,
porque o que sobra sem ele é o comportamento antigo — `abrir_para_sincronizar(1)`
mais a fronteira de escrita —, e esse comportamento **não alcança o volume do
meio**. A fase 3 da sonda já tinha medido exatamente isso, no mesmo arquivo, e
eu li o número sem ligá-lo à frase.

E errei uma segunda vez, no sentido oposto: supus que *qualquer* grafia
diferente dividisse a família. `/tmp/./x` e `/tmp/x` **não** dividem — e uma
sonda escrita com essa grafia teria medido «a marca atravessa» e teria acertado
pelo motivo errado.

## 3. O que a medição disse

**As grafias, medidas (`PathBuf` compara por `components()`):**

| par | mesma chave? | por quê |
|---|---|---|
| `/tmp/x` e `/tmp/./x` | **sim** | o componente `CurDir` some |
| `/tmp/x` e `/tmp/x/` | **sim** | a barra final some |
| `/tmp/x` e `x` (cwd `/tmp`) | **não** | é esta que divide |
| `/tmp/x` e `/tmp/y/../x` | **não** | e continua não sendo, de propósito |

**O efeito, por `strace`, na sonda de cinco volumes:**

```text
                                              antes            depois
fase 3, volume 3 sujo pela MESMA grafia    001, 003, 005    001, 003, 005
fase 5, volume 2 sujo por OUTRA grafia     001,      005    001, 002, 005
```

Antes, o volume 2 — sujo e não sincronizado — **ficava para trás**. A perda só
apareceria numa queda de energia: página suja no cache do núcleo sobrevive a
`SIGKILL`, e foi por isso que a bateria inteira passou por cima disto.

**A varredura de quem produz o caminho.** Dentro do `phxsqld` há **um** produtor
e um só: `Servidor::novo` chama `Raiz::nova(&config.base)` (`servidor.rs:711`),
e todo caminho mais fundo nasce de um `join` a partir dele — `Instancia`,
`Database::diretorio`, `abrir_qualificada`, `abrir_para_ler`. O `restaurar`
move diretório por `rename` e não abre tabela por outra grafia; o `reindexar`
usa a tabela que já está aberta; o DbLink e a replicação não montam raiz
própria. **O servidor sozinho não produz duas grafias.**

Quem produz são os outros dois portões da mesma biblioteca: a **FFI**
(`phx_base_abrir`, `crates/phxsql-ffi/src/lib.rs:334`) recebe o caminho do
chamador C **a cada chamada**, e nada impede um aplicativo de chamar
`"dados"` numa tela e `"/app/dados"` noutra; e a **CLI**
(`crates/phxsql-cli/src/main.rs:239,259`). Some-se que `config.base` é
relativo nos exemplos de container (`"base": "base"`) e absoluto nos de
servidor (`/var/lib/phxsql/dados`), e o par relativo/absoluto deixa de ser
hipótese.

**O custo do conserto.** O ramo caro é só o do caminho relativo — um `getcwd`,
**395 ns**. O caminho já absoluto paga um `is_absolute()`. Como `Table::abrir`
monta **sete** conjuntos de volumes, resolver uma vez em `Table` deixa os sete
no ramo barato: `--example custo-de-abrir` mediu **49,31 µs** de mínimo antes e
**48,68 µs** depois, em dez corridas — o conserto está **dentro do ruído** do
medidor.

## 4. A regra

**Chave de registro que atravessa instâncias se resolve num lugar só, e o lugar
é onde a chave nasce.** E, antes disso: *toda afirmação de «a degradação é
benigna» é uma hipótese até alguém medir o que sobra quando ela acontece.*

## 5. Como está guardado hoje

* a resolução mora em `volume::familia` e `volume::absoluto_lexico`
  (`crates/phxsql-store/src/volume.rs`) — **o único lugar onde a chave nasce**;
  `Table::resolver` a chama por velocidade, não por correção, e diz isso;
* a guarda sem `strace` é
  `crates/phxsql-store/tests/grafia-do-diretorio-nao-divide-a-familia.rs`, que
  conta `Volumes::sincronizados()` — o arquivo que passou pelo `sync_all`, e
  não `sincronizacoes()`/`selo()`, que sobem antes do laço e mediriam a
  intenção. **Prova real:** com a resolução desfeita, ela falha com
  `sincronizados = 0` onde espera 1;
* a prova externa continua sendo a sonda, e o cabeçalho dela agora traz as duas
  corridas — antes e depois — em vez de só a que motivou o conserto;
* a premissa das grafias virou teste de unidade
  (`a_chave_da_familia_junta_as_grafias_que_o_pathbuf_ja_junta`), porque
  premissa que só vive em comentário envelhece calada;
* **o buraco que fica, nomeado:** `..` no meio do caminho e dois *symlinks*
  para o mesmo diretório continuam dividindo a família. Só `canonicalize` os
  juntaria, e ela toca o disco e **falha em caminho que ainda não existe** — o
  que quebraria `Table::criar`. Nenhum dos dois alcança o servidor (o
  `validar_nome` recusa `..` em database, schema e tabela); alcançam quem passa
  a **raiz**, que é o `config.json` e o chamador C da FFI.

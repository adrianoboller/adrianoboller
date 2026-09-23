# Parecer do papel C (DBA) — 20 caixas, 1 servidor, re-medido contra a 0.19.0

**Data:** 23/09/2026 · **Commit lido:** `43fbeb2` · **Papel:** C (DBA senior), so leitura.
**Substitui em parte** o `parecer-dba-vinte-caixas-um-servidor-2026-09-17.md`, que **fica**: este o
corrige em dois pontos, e a correcao so se entende ao lado do original.

## 0. O que divergiu do contrato que abriu a frente

1. **O v10 saiu COM metade da faixa.** O `passo` da `Sequence` esta no disco
   (`schema.rs:711-716`, `1226-1256`, `1954`, `2162-2196`) e e declaravel pelo protocolo
   (`valores.rs:620-635`). **O `inicio` nao saiu.**
2. **`julga_integridade` mudou de endereco**: `table.rs:1524-1526` (era 1315-1317). Mesmo corpo,
   mas os portoes passaram de **7 para 9**.
3. **O «964 de 1.000» e numero de corrida, nao constante.** Re-medido em 23/09, perfil debug:
   **1000 carimbos distintos, 30 milissegundos distintos, maior empate 38 linhas** → 970 das 1.000
   dividem milissegundo. Confirma a decisao em especie; o digito e da maquina e do dia.
4. **O pedido 290 esta ☑️ e nao entrega a garantia.** §2.
5. **A objecao (b) do 325 — «aplicar por ROWID com 20 sequencias» — nao vale nos dois arranjos.**
   E verdade no *consolidado* e **falsa** no *espelho*. E o achado principal. §4.

**Achado fora do escopo, entregue porque e do dominio:** o `docs/FORMATO.md` se contradiz sobre
durabilidade do `.ndx`. Uma secao diz que o cache e **write-back desde a 0.18.0**; trinta linhas
abaixo, um paragrafo sobrevivente da 0.17.0 ainda afirma que *«a gravacao atravessa sempre para o
arquivo»*. O codigo concorda com a primeira (`ndx.rs:199-204`). **Documento afirmando garantia de
durabilidade que nao existe mais e pior que documento faltando.**

## 1. As seis pernas, re-medidas contra o codigo de hoje

| # | perna | 17/09 | 23/09 | onde |
|---|---|---|---|---|
| 1 | gravar local no caixa sem servidor | EXISTE | **EXISTE** | `phxsql-ffi/src/lib.rs`, 44 funcoes `extern "C"` |
| 2 | o operador nao perceber a queda | NAO EXISTE | **NAO EXISTE** | zero ocorrencias de reconexao nos clientes; `phxsql-odbc/src/conexao.rs:205-206` |
| 3 | fila local ate a rede voltar | NAO EXISTE | **NAO EXISTE, e pior** | o unico gancho de aplicacao do FFI (`lib.rs:1413`) chama `aplicar_evento`, que aplica **por ROWID** |
| 4 | subir e replicar ao religar | PARCIAL (1↔1) | **PARCIAL, com mais osso** | `servidor.rs:2524`, `2887-2908`; `config.rs:264-270` |
| 5 | identidade da venda sem colisao | PARCIAL | **MUDOU DE ESTADO — metade nasceu** | §2 |
| 6 | venda + itens integros | PARCIAL POR DESENHO | **PARCIAL POR DESENHO, com 2 portoes a mais** | `table.rs:1524-1526` + nove chamadores |

**Acerto que vale registrar:** `bidirecional.rs:158-178` passou a usar `e_coluna_de_sistema` no
`chave_unica` **de proposito**, para que indice unico sobre `rowstamp` nunca vire identidade
replicavel. Coluna de sistema nova entrou **sem** abrir essa porta — a licao do `rownum` aplicada
antes do estrago.

**Correcao do parecer de 17/09:** o read-your-own-writes da FK dentro da transacao **existe desde
12/09** (`table.rs:256-267`, trait `MaesEmProgresso`, commit `2fe8658`). O invariante «so existe
filho se o pai existir primeiro» esta **implementado**, nao pendente.

## 2. A identidade, com o v10 na mao

### 2.1 A recomendacao de 17/09 saiu pela metade

| peca | no disco? | via de producao? | onde |
|---|---|---|---|
| `passo` da faixa | **sim** | **sim** | `schema.rs:711-716`; `valores.rs:620-635` |
| `na_faixa` (aritmetica) | — | **sim** | `no.rs:86-102`; `reg.rs:794-810` |
| conferencia na abertura | **sim** | **sim** | `reg.rs:497-519` |
| **`inicio` do no** | zero byte, por desenho | **NAO** | `no.rs:62-84` |

**`definir_inicio_da_sequencia` tem ZERO chamadores de producao** — os dois unicos sao
`tests/carimbo-e-faixa.rs:430` e `:458`. Nao ha campo no `config.json`, nao ha operacao de
protocolo, nao ha chamada no arranque.

**Consequencia:** todo `phxsqld` roda com `INICIO = 0`. Declarar `passo = 20` nas 20 caixas **nao
separa faixa nenhuma** — as 20 numeram `20, 40, 60…`, identicas. A colisao fica 20x mais esparsa e
continua **100%**.

> **Parecer: o pedido 290 esta fechado na tabela e aberto no dado.** A metade que falta **nao muda
> um byte de formato** — o invariante e `proxima_sequencia ≡ inicio (mod passo)`, e a
> `proxima_sequencia` ja mora nos bytes 36..44 do cabecalho do `.reg`.

### 2.2 Entre `Sequence` com faixa, `Uuid` v7 e `rowstamp`

**`rowstamp`: NAO.** Ele e **ordem**, nao **identidade**: nasce em 1 em todo no (`no.rs:36-42`), e o
proprio motor ja o protege de virar identidade replicavel. Responder «o rowstamp» aqui seria
promover a coluna que o codigo acabou de blindar.

**`Uuid` v7: alternativa declarada, nao recomendacao.** Aritmetica re-conferida contra o fonte
(`ndx.rs:784-785`, `keyenc.rs`, `types.rs:87`), pagina de 4096 em claro:

| chave | `ck_len` | por folha | por no interno |
|---|---:|---:|---:|
| `Sequence`/`UInt8` | 17 B | **239** | **162** |
| `Uuid` v7 | 25 B | **162** | **123** |

239/162 = **+47,5% de folhas**. E a localidade do v7 se perde **exatamente na reconexao do caixa
atrasado**, que e o evento para o qual este projeto existe. *(Leitura do desenho; a bancada que a
provaria nao rodou.)*

> **Recomendacao, inalterada: `Sequence` COM FAIXA.** Falta o `inicio`, e ele custa **zero byte**.

## 3. O que muda o formato, separado do que nao muda

### 3.1 Nao muda formato — pode andar sem a mesa do dono

`passo` (ja entrou) · `rowstamp`/`rowtime` (ja entraram) · **`inicio` do no** (zero byte) ·
guarda de sobreposicao de database entre origens · 20 databases com nomes distintos ·
chave composta no bidirecional (pedido 331). **Zero migracoes.**

### 3.2 Muda — vai a mesa como pergunta fechada

**PERGUNTA 1 — a faixa `mod N` do `rowstamp` entra, ou fica de fora para sempre?**
Custo em bytes: **zero**. Custo real: **muda o significado do que ja esta gravado**, e nao ha como
decidir retroativamente em que faixa cada linha nasceu. **E decisao cedo-ou-nunca mesmo custando
zero byte.** *Recomendacao de C: NAO ligar* — o `mod N` mata o empate e **nao cria a ordem**, e
nenhum dos dois arranjos compara `rowstamp` entre nos.

**PERGUNTA 2 — o `.log` ganha id de transacao?**
E a unica forma de o central saber que chegou **meio commit**. O cabecalho tem **44 bytes e esta
CHEIO** (`log.rs:176-187`), sem byte reservado.
- **(A)** id de 8 bytes → cabecalho 44→52, **versao nova do `.log`**; uma migracao por tabela
  replicada, e source e replica sobem JUNTOS.
- **(B)** dois bits do byte de flags (so o bit 0 e usado) → **zero byte, zero migracao** — mas a
  posicao da replicacao e **por tabela**, entao ela **nao agrupa venda com itens**. Entrega menos
  do que a pergunta pede, e isso esta dito em vez de vendido barato.
- **(C)** nao fazer: a orfa transitoria continua indistinguivel da permanente.

**PERGUNTA 3 — quem migra as tabelas v9?** `Table::migrar_para_psch_v10` existe, e retomavel e
idempotente (`table.rs:1020-1080`) e **nao tem UM chamador de producao**. Nao e mudanca de formato:
e a **porta** dela, e esta faltando. A migracao reescreve o `.reg` inteiro **duas vezes** — numa
tabela de vendas de um ano, isso e janela de parada da loja.

## 4. Onde o DBA diz NAO

### (a) O central virar `replica` de 20 origens — **SIM, com uma condicao**

O aviso de `servidor.rs:2513-2523` esta **certo**: *«replica sem `somente_leitura`: se a aplicacao
escrever aqui, os rowids divergem e a replicacao para»* — e `table.rs:4576-4581` fail-stopa.

**Recuso o arranjo SEM a condicao:** o central so e legitimo com **`somente_leitura: true`**. A
replica aplica **por dentro** (`Table::aplicar_evento`), nunca pelo protocolo — medido: a op
`aplicar` foi chamada **zero** vezes nos `acessos.log` de uma bancada com os dois de pe.

**O que custa ao produto:** *o central deixa de ser onde se escreve.* Vira painel e retaguarda.

**Se precos/produtos precisarem DESCER do central**, o instrumento certo ja existe e **nao** e o
`somente_leitura` global: e o comando proibido **por base** (`blacklist.rs:125,270-277`). O caixa
proibe escrita na base `precos`, que ele puxa, e escreve a vontade na base `vendas`, que e dele.
O que falta e o aviso **dizer isso** — hoje ele e global e nao nomeia a base em risco.

### (b) O `aplicar_evento` por ROWID — **DEPENDE, e e aqui que o 325 estava incompleto**

`abrir_para_replicar` (`servidor.rs:2887-2908`) cria o destino **com o mesmo nome da origem**, e
`Origem.databases` e **filtro**, nao mapa.

**Arranjo ESPELHO** (20 caixas com nomes distintos, `caixa01`…`caixa20`): o central fica com 20
databases, cada um com **exatamente um escritor**. Os rowids batem por construcao. **Cada `.reg` do
central e a ordem de digitacao REAL daquele caixa, byte a byte** — a petrea nao e contornada, e
**preservada 20 vezes**. **Veredito: SIM**, e e o arranjo recomendado.

**Arranjo CONSOLIDADO** (os 20 no mesmo nome): rowids colidem no segundo evento da segunda origem.
**Veredito: NAO, e a petrea nomeada e a ordem de digitacao.**

**Agravante que so apareceu lendo o `aplicar_evento_interno`:** a conferencia de rowid existe **so
na inclusao** (`table.rs:4574-4582`). A **alteracao** faz `self.atualizar(rowid, &valores)`
(`table.rs:4584`) **sem conferencia nenhuma**. A trava alta cobre o caso comum; o caminho da
alteracao e onde a sobreposicao seria **calada**. A premissa «este rowid e meu porque so eu escrevo
aqui» e verdadeira hoje e **nao e conferida em lugar nenhum**.

### (c) NAOs mantidos de 17/09

Reconferir integridade referencial no central (custa dado: 0 de 2 eventos) · calar a unicidade no
ponto de convergencia (violacao de unico **nao se cura** com o lote seguinte) · prometer «pai
estritamente antes do filho» com pai e filha em caixas diferentes e servidor fora do ar (nao ha
formato que cumpra) · chamar isto de «transparente» sem o contrato escrito do que se perde.

### (d) NAO novo, de processo

**Nao tratar o pedido 290 como entregue.** Esta ☑️ e a garantia nao existe no dado, e **pedido
fechado e exatamente o que ninguem reabre para conferir**.

## 5. A petrea da integridade contra este cenario

### 5.1 Os nove portoes calados na replica

| linha | o que deixa de acontecer na aplicacao de evento replicado |
|---:|---|
| `932` | `CHECK` de coluna nova |
| `2053` | planejamento da cascata (certo: o evento da cascata vem sozinho) |
| `3001` | `Uuid` que nasce sozinho **nao** dispara (certo: id local divergiria do retrato) |
| `3306` | `DEFAULT`, coluna calculada e `CHECK` |
| `3518` | **FK na inclusao — nao confere se o pai existe** |
| `3824` | FK na alteracao |
| `3925` | cascata do `ao_alterar` |
| `4127` | **`conferir_filhas` na exclusao — mata o pai que tem filhos** |
| `4234` | FK na restauracao |

**Traduzido: no central, a regra primordial da integridade NAO e imposta** — e isso esta **certo**,
e o desenho medido. A garantia e da **origem**; a da replica e de **fidelidade**, conferida por
SHA-256 por linha.

### 5.2 O eixo, com a excecao medida

> **A integridade referencial do PhxSql e local a cada `.reg`, e a UNIAO de 20 bancos integros nao
> e um banco integro.**

Vale **inteira no consolidado** e **nao morde no espelho** — no espelho nao ha uniao: sao 20 bancos
separados, cada um fiel ao caixa que o gerou. A orfa permanente continua possivel (a posicao da
replicacao e por tabela e nao ha id de transacao no `.log`), mas fica **confinada ao database
daquele caixa**, com dono e hora conhecidos, em vez de contaminar a tabela que a loja consulta.

### 5.3 «Impossivel o filho ter a MESMA data do pai», offline

**Caso (a) — pai e filha no MESMO caixa: a petrea SE CUMPRE**, elo por elo:
1. o contador do `rowstamp` e do **processo**, nao do `Table` (`no.rs:1-27`);
2. o pai vem antes porque a FK do filho o exige, e **dentro da transacao a conferencia enxerga o pai
   empilhado** (`table.rs:256-267`);
3. o carimbo **viaja na imagem** e a replica **honra o que veio** (`table.rs:2635-2674`);
4. o carimbo e emitido **no inserir e nunca no atualizar** (`table.rs:2618-2626`);
5. no reinicio, a marca d'agua volta **do dado** (`reg.rs:216-230`) — e **nao subiu a versao do
   `.reg`**, porque ali zero significa «ainda nao carimbou nada», que e verdade num `.reg` anterior
   ao v10. Ausencia benigna nao sobe versao.

**Caso (b) — pai e filha em caixas DIFERENTES: NAO se cumpre, e nao ha formato que cumpra** com o
servidor caido. Os dois contadores sao independentes e ambos comecam em 1. O empurrao de Lamport
exige que o evento do pai **tenha chegado**, que e o que o cenario nega.

**Saida de modelagem:** toda relacao pai/filha com chave **conferida** do caminho de venda nasce no
mesmo caixa; o que atravessa caixas e lancamento proprio com referencia **nao conferida**
(`"verificar": false`, escolha escrita, nunca omissao).

**E a consequencia que a casa cobra antes da primeira loja:** chave conferida precisa de **indice
dos dois lados** (`table.rs:1480-1484`). Nao e opcional e nao e barato de descobrir depois.

## 6. A primeira frente de codigo que pode comecar JA

> **A guarda que separa o arranjo que funciona do que se destroi: duas origens nao podem entregar o
> mesmo nome de database.**

Medido: `Config::validar()` (`config.rs:4010-4071`) confere papel, `id_servidor`, `somente_leitura`,
`imagem_da_linha`, hora, cluster, pinos e cifra — **nao confere sobreposicao de database entre
origens**. `Origem.databases` vazio quer dizer «todos», e o teste `le_origens_de_replicacao`
(`config.rs:5653-5674`) tem uma origem com lista e outra sem, no mesmo `config.json`, e `validar()`
aprova. A descoberta dinamica (`servidor.rs:2859-2866`, `4438-4445`) e onde o servidor sabe pela
primeira vez que duas origens vao entregar o mesmo nome — **e nao pergunta**.

Por que e a frente certa: **recusa na declaracao** (o arranjo errado morre no arranque, nomeando as
duas origens e o database em comum, em vez de no fail-stop no meio do expediente — ou calado no
caminho da alteracao) · **zero formato, zero migracao, zero decisao do dono** · **e o que torna o
espelho seguro**.

**A armadilha para quem escrever:** *guarda nova entra pedida, nao imposta*. O teste que mais
importa e o do comportamento **velho** — um `config.json` de par (1↔1) tem de continuar subindo byte
a byte. E o caso das duas origens com listas vazias e **indecidivel no `validar()`**: so a
descoberta sabe.

**Na fila atras, tambem sem formato:** o `inicio` do no (§2.1) e a porta do `migrar_para_psch_v10`.

## 7. O que foi MEDIDO e o que esta CITADO

**Medido em 23/09/2026:** o carimbo (1000 distintos / 30 ms distintos / maior empate 38) · 44
funcoes `extern "C"` · 9 portoes sob `julga_integridade()` · **zero** chamadores de producao de
`definir_inicio_da_sequencia` e de `migrar_para_psch_v10` · zero ocorrencias de reconexao nos
clientes.

**Aritmetica conferida contra o fonte (nao e bancada):** 239 / 162 chaves por folha, **+47,5%**.

**CITADO, nao remedido:** «4 insercoes → 2 linhas» (07/09) · «0 de 2 eventos» da reconferencia de FK
na replica · 0,862% do selo do `.fts` · 0,553 µs/linha da migracao v10 · **a perda de localidade do
`Uuid` v7 na reconexao — leitura do desenho, nunca medida** · as nove linhas da §7 do parecer de
17/09 continuam todas sem bancada.

## VEREDITO

**Hoje, ainda NAO — mas a distancia encurtou, e por um motivo diferente do que eu escrevi em 17/09.**

O v10 trouxe o `passo`, que era metade da recomendacao. A outra metade — o `inicio` do no — **nao
tem via de producao**, custa **zero byte** e e o que separa um pedido fechado de uma garantia
entregue.

E o caminho por ROWID **nao esta morto**: morre no consolidado e **vive inteiro no espelho** — 20
caixas com 20 nomes de database, um central em `somente_leitura` puxando de 20 origens,
consolidacao por leitura (`unir` com `partes`; teto: cada parte cabe em memoria, limitada por
`recursos.max_linhas`). **E o unico arranjo em que as duas petreas sobrevivem.**

**O que falta, na ordem:** a guarda dos nomes de database (§6, comeca ja) · o `inicio` do no (zero
formato) · as pernas 2 e 3, **que continuam sem uma linha de codigo e sao de produto, nao de
motor** · as duas perguntas fechadas da §3.2 na mesa do dono.

**O que e verdade por formato, e tem de estar no contrato antes da primeira loja subir:** nesta
janela **nao ha unicidade global, nao ha integridade referencial imposta no ponto de convergencia,
nao ha ordem entre nos e nao ha atomicidade de commit entre tabelas.** Sem essa frase escrita, a
palavra «transparente» do enunciado e maior que o produto.

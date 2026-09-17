# Parecer do papel C — `PSCH` v10, byte a byte

**DBA senior, 17/09/2026. So leitura; nenhum arquivo do repositorio foi escrito
pela frente.** Conferido pelo integrador (papel A) no fonte antes de entrar aqui:
os quatro pontos de §3.3, o crivo de `valores.rs`, o mapa do cabecalho e a
aritmetica de ponto flutuante de §4.2 foram remedidos e batem.

Contrato: as decisoes do dono de 17/09/2026 07:10 UTC nos pedidos 289 e 290,
mais os itens de formato do 229, num bump unico.

## 0. A correcao do dono sobre a 290 esta certa, e foi conferida no fonte

`abrir_para_replicar` (`crates/phxsql-server/src/servidor.rs:2796-2818`) cria a
tabela que nao existe com `db.criar_tabela(schema.as_deref(), e.clone())`, e `e`
e o `Schema` desserializado do bloco que o source mandou
(`crates/phxsql-server/src/replica.rs:302-306` e `:326-332`). `criar_tabela`
passa adiante sem tocar (`crates/phxsql-store/src/catalogo.rs:599-607`), **sem
passar por `Schema::new`**. Dois `u64` gravados no `PSCH` chegariam identicos nos
dois nos. A proposta do parecer anterior de C nao resolvia o defeito que existe
para resolver. **C aceita a correcao.**

## 1. O bloco de esquema hoje (v9), medido

Mora em `crates/phxsql-core/src/schema.rs` (2.321 linhas), gravado no `.reg`
logo apos o cabecalho do volume (`crates/phxsql-store/src/reg.rs:26-35`).

- `MAGIC_ESQUEMA = b"PSCH"` — `schema.rs:14`
- `VERSAO_ESQUEMA: u16 = 9` — `schema.rs:51` · minima 2 — `schema.rs:52`
- `serializar` — `schema.rs:1247-1352`, versao nos bytes 4..6 (`:1250`)
- `desserializar` — `schema.rs:1354-1547`, portao da faixa em `:1360-1366`

O portao recusa fora da faixa: «versao de esquema N nao suportada (este motor le
da 2 a 9)».

### O precedente, v3 a v9

| versao | o que acrescentou | forma | ausencia |
|---|---|---|---|
| 3 | metadados de coluna + modo de particao | dentro do laco de colunas | `Uuid::v7()` novo + textos vazios |
| 4 | coluna `softdeleted` + byte `motivo_obrigatorio` | 1 byte no fim | `false` |
| 5 | coluna `rownum` | so a lista de colunas, **zero bytes de bloco** | coluna nao existe |
| 6 | `DadoPessoal` por coluna | bloco no fim, 1 byte por coluna | `Nao`; truncado = `break` |
| 7 | byte `verificar` por chave estrangeira | dentro do registro da FK | desligado |
| 8 | lista dos indices de texto | lista propria no fim, `u16` + entradas | vazia; truncado = `break` |
| 9 | expressoes (`padrao`, `check`, `calculada`, `onde`) | bloco no fim, em texto | **truncado = ERRO** (`schema.rs:1519`) |

### As tres leis que o precedente escreveu

1. **Bloco novo vai no FIM** (`schema.rs:26-33`, `docs/FORMATO.md:340-345`): quem
   le versao antiga para antes dali. A primeira v8 pos bytes no meio do registro
   do indice e duas guardas de compatibilidade cairam.
2. **Versao sobe em vez de roubar bit livre** (`schema.rs:45-50`): binario antigo
   lendo bit que nao conhece abriria a tabela e ignoraria a coisa nova, que e
   corrupcao silenciosa. Com versao nova ele **recusa**. Recusa alta e melhor que
   aceite errado.
3. **A leitura NAO acrescenta coluna.** `Schema::do_disco` (`schema.rs:572-778`)
   monta exatamente as colunas gravadas; `Schema::new` (`:552-570`) e o unico que
   empurra `softdeleted` e `rownum`. Motivo em `schema.rs:566-571`: uma coluna a
   mais na leitura deslocaria o offset de todas as seguintes **e o CRC do slot
   continuaria batendo**, porque os bytes nao mudaram, so a interpretacao.

### Medido, do bloco v9 real

Tabela `clientes` = `Sequence` + `Str(40)` + `Str(20)` + `Decimal(15,2)` + as duas
de sistema:

```
PSCH v9 bytes = 481 (2 indices) / 462 (1 indice)
colunas = 6 -> [id, nome, cidade, saldo, softdeleted, rownum]
offsets: 1, 9, 49, 69, 85, 86   bitmap_len = 1   payload_len = 94
slot (SLOT_CAB 24 + payload) = 118
ida e volta serializar->desserializar igual? true
```

Do `.reg` no disco: `cab_len = 128`, `esquema_len = 462`, `data_offset = 640`.
**Folga entre o fim do bloco e o primeiro slot: 640 - 128 - 462 = 50 bytes.**

## 2. O que o v10 acrescenta

### 2.1 A parte que custa ZERO byte de bloco, e mesmo assim exige o bump

A coluna de data/hora (289) e uma coluna como outra qualquer e entra na lista de
colunas, igual ao que a v5 fez com o `rownum`. **Por que subir a versao, entao?**

Um binario v9 que abrisse um bloco v9 com uma coluna chamada `rowstamp` a leria
como **coluna comum do usuario** — o `e_coluna_de_sistema` dele
(`schema.rs:83-85`) so conhece dois nomes. A coluna apareceria na grade, no
formulario, no `INSERT` e no `UPDATE`, e o usuario poderia **digitar por cima do
carimbo**. A versao no byte 4..6 e a unica coisa que se le antes de decidir o que
o resto significa. **O bump da 289 nao carrega byte: carrega uma recusa.**

### 2.2 A parte que carrega bytes — 290 e 229

Dois blocos novos, **depois** do bloco de expressoes da v9, cada um com contagem
`u16` a frente e lista propria por posicao de coluna. Campo dentro do laco de
colunas obriga cada versao antiga a um desvio dentro do laco de desserializacao,
que e onde nasce o campo deslocado que ainda passa no CRC (o erro da v8).

**Bloco v10-a — faixa e identidade da `Sequence`:**

| campo | tipo | bytes | ausencia |
|---|---|---|---|
| `n_sequencias` | `u16` | 2 | 0 |
| `coluna` | `u16` | 2 | — |
| `passo` | `u64` | 8 | **1** |
| `sempre_do_motor` (IDENTITY ALWAYS) | `u8` | 1 | **0**, `BY DEFAULT` |
| `nome_da_sequencia` | `u16` + n | 2+n | vazio |

**O `inicio` NAO esta aqui**, e e a correcao do dono. O bloco viaja byte a byte
para a replica, e um `inicio` gravado aqui chegaria igual nos dois nos. O `passo`
**pode** viajar porque e comum aos nos **por definicao**: e o denominador da
faixa, e a faixa so e disjunta se os dois usarem o mesmo passo. Replica-lo nao e
tolerado, e **exigido**.

So ha uma coluna `Sequence` por tabela (`crates/phxsql-core/src/types.rs:67-68`;
contador unico em `reg.rs:195-199`), entao `n_sequencias` e 0 ou 1 hoje. A lista
fica com contagem mesmo assim: lista com contagem detecta truncamento, campo
solto nao.

**Bloco v10-b — `Uuid` que nasce sozinho (229):** `n_uuids_automaticos: u16`,
depois `coluna: u16` + `versao_do_uuid: u8` (7 ou 4). Ausencia: nao nasce sozinho.

**Custo medido do bump**, tabela `clientes` com uma `Sequence`:

| item | antes | depois |
|---|---|---|
| bloco `PSCH` | 462 B | ~607 B (+132 pela coluna na lista, +13 pelos dois blocos) |
| payload | 94 B | 102 B |
| slot | 118 B | 126 B |

### 2.3 O que um leitor v9 faz com um v10, e vice-versa

| quem le | o que faz | certo? |
|---|---|---|
| v9 le v10 | **RECUSA** (`schema.rs:1361-1366`) | Sim. E o que impede o binario velho de deixar digitar por cima do carimbo. |
| v10 le v9 | le tudo, para antes dos blocos novos: `passo = 1`, sem `ALWAYS`, sem `Uuid` automatico, **e sem coluna de carimbo** | Sim. Petrea: banco que ja existe nao quebra. |
| v10 le v10 truncado | **ERRO**, no molde da v9 e nao no da v6/v8 | Sim. `passo` que sumisse calado viraria faixa 1 e faria dois masters numerarem a mesma faixa outra vez. |

Acessor da ausencia no molde de `coluna_softdeleted` (`schema.rs:783-789`):
`Schema::coluna_rowstamp() -> Option<usize>`, `None` em tabela anterior ao v10.
A mensagem de quem pedir a garantia **diz isso**, no molde de `exigir_softdeleted`
(`crates/phxsql-store/src/table.rs:807-816`).

Guarda do comportamento VELHO, que e a que mais importa: o molde esta pronto em
`schema.rs:1828-1846` (`esquema_v5_abre_sem_marca_nenhuma`), que fabrica arquivo
antigo trocando os bytes 4..6 e truncando a cauda.

## 3. A coluna de data/hora

### 3.1 O nome: `rowstamp`, nao `criado_em`

`Schema::new` decide criar a coluna de sistema por nome (`schema.rs:556`, `:576`)
e `do_disco` tem guarda de tipo por nome (`schema.rs:721-757`) que **recusa a
tabela inteira**. Se a coluna se chamar `criado_em`, **toda tabela de usuario que
ja tem `criado_em DATETIME` fica inabrivel e incriavel a partir do bump**.
`criado_em` e o nome de auditoria mais comum que existe num banco em portugues;
`softdeleted` e `rownum` passaram porque ninguem os digita.

Rotulo em portugues vai no `caption` (`schema.rs:1264`), que e para isso. **Rotulo
se traduz, nome de coluna e estrutura.**

### 3.2 Onde entra

No **fim** da lista, depois de `rownum`, em `Schema::new` (`schema.rs:574-586`).
Offsets medidos: `softdeleted` 85, `rownum` 86, `rowstamp` 94; payload 94 -> 102.

**Alerta de largura:** `bitmap_len = colunas.len().div_ceil(8)` (`schema.rs:758`).
Numa tabela com **8** colunas, a nona empurra o bitmap de 1 para 2 bytes e o
payload cresce **9**, nao 8, e todos os offsets andam. O `acrescentar_coluna` ja
sabe (`reg.rs:1242-1247`), quem escrever a migracao precisa saber.

### 3.3 Quem filtra pela primeira HOJE — quatro achados, tres sao defeito

Conferidos no fonte pelo integrador, literais:

1. **`crates/phxsql-server/src/dblink/sincronia.rs:248-256`, `posicoes_de_negocio`.
   O pior.** `.filter(|(_, c)| c.nome != "softdeleted" && c.nome != "rownum")`,
   literais crus, nem as constantes. Com a coluna nova, o carimbo vira **coluna de
   negocio**, e o comentario logo abaixo diz que faltar uma seria falha. **A
   sincronia do DbLink quebra em toda tabela migrada.**
2. **`crates/phxsql-server/src/bidirecional.rs:158-171`, `chave_unica`**, e e
   literalmente um `find(`. Um indice unico sobre a coluna nova viraria **a
   identidade replicavel da tabela** no bidirecional: casamento por chave sobre um
   carimbo que e local por no. Silencioso.
3. **`crates/phxsql-store/src/table.rs:2329-2353`, `completar`.** Par cravado, e o
   ramo de queda e pior: `None => Value::Bool(false)` — o carimbo cairia ali. Uma
   linha curta cai no `return None` e a aridade reclama, que e exatamente a
   mensagem «a lista tem N valores» que o `rownum` produziu na tela.
4. **`crates/phxsql-core/src/carga.rs:747-758`, `linha_da_carga`.** Mesmo desenho,
   queda em `Ok(Value::Null)`; a coluna de sistema e obrigatoria, entao a carga por
   texto passa a recusar toda linha.

**O que NAO quebra, e e a peca a copiar:** `colunas_de_sistema_no_fim`
(`table.rs:2288-2296`) conta do fim para tras com `take_while(e_coluna_de_sistema)`.
Uma terceira coluna no fim entra sozinha. A tela ja sarou desta familia:
`crates/phxsql-server/ui/index.html:5052-5063` usa `filter(c => !c.sistema)` com o
comentario do defeito ao lado.

**Conserto entra no caminho que o motivou, e o caminho IRMAO fica:** os quatro
fazem a mesma pergunta e tres a respondem a mao. Os quatro passam a chamar
`e_coluna_de_sistema` (`schema.rs:83-85`), cujo proprio doc dizia «a lista
repetida em tres lugares e onde a quarta seria esquecida». Estava repetida em
**quatro**.

## 4. O avanco forcado, e as duas formas com preco

> **C nao mediu os motores maduros nesta frente** — a conta dos quatro e do papel
> J. O indicio que C mediu na pesquisa desta casa: `docs/SPRINTS-MARIADB.md:404-406`
> registra que o MariaDB descreve o versionamento preciso «em cima de IDs de
> transacao», e nao de relogio; e `:373-376` chama o carimbo do nosso `.log` de
> «em tudo menos no nome, um `row_start`». Indicio, nao prova.

### 4.1 O que o avanco forcado exige em qualquer forma

**A garantia e POR NO, nao por tabela.** Pai e filho estao em tabelas diferentes
por definicao (a FK e declarada na filha). Um contador no `Table` nao cumpriria a
ordem: dois `Table` abertos emitiriam carimbos independentes. → **O contador e do
PROCESSO**: `static ULTIMO: AtomicU64` com `fetch_update`, em `phxsql-store`.
Nao e peca nova: `ndx.rs:156-158`, `volume.rs:88`, `cofre.rs:177` e `diario.rs:38`
ja sao isso, e nenhum sai da `std`.

Sob a trava global (`servidor.rs:1492-1520`, `RwLock::write`) dois commits nunca
se cruzam. Mas o `phxsql-store` tambem e usado direto pela CLI, pela FFI, pelos
examples e pelos testes, **sem** aquela trava — por isso CAS e nao `+= 1` com a
trava de fora como premissa. Premissa de trava alheia e a que se perde numa
divisao de arquivo.

**A ordem pai-antes-de-filho sai de graca do que ja existe**, e e o achado bom
desta secao: o conjunto de escrita da transacao e empilhado na ordem pedida
(`crates/phxsql-server/src/transacao.rs:218-221`) e a cascata viaja achatada na
ordem pai-antes-de-filha (`transacao.rs:1359-1362`); e o pai **tem** de vir antes
porque a conferencia de FK do filho exige que ele exista. Basta o carimbo ser
emitido no momento em que a linha e codificada.

### 4.2 FORMA A — um campo, `u64` de nanos com avanco forcado

Bytes por linha 8; slot 118 -> 126 (+6,8%); em 10M linhas 1.125,3 -> 1.201,6 MiB
(+76,3 MiB).

No arranque o ultimo carimbo **tem** de voltar do dado, senao um no que reinicie
depois de um salto de NTP para tras reemite carimbo menor — a garantia morre
calada no caso que ela existe para cobrir.

Custo honesto que precisa estar na descricao da coluna: depois de um salto de NTP
para tras de Δ, a coluna **mente sobre a hora de parede** por Δ, reportando futuro.

**E o defeito que a mata, remedido pelo integrador com o `Json` desta casa e com
a aritmetica IEEE 754:**

```
INTEIRO_EXATO_MAX (json.rs:25) = 9007199254740992   (2^53)
carimbo em nanos               = 1789629737134573795
como f64                       = 1789629737134573824   (erro de 29 ns so em ida)
n e n+1 empatam como f64?      = True
grade do f64 nessa magnitude   = 256 ns
magnitude do carimbo           = 198,7x acima de 2^53
```

`valor_para_json` manda `UInt` por `Json::de_u64` sem crivo
(`crates/phxsql-server/src/valores.rs:745`); o crivo de 2^53 existe **so para
`Sequence`** (`valores.rs:742-744`), e o comentario ali **ja nomeia o irmao**:
«vale so para a `Sequence`; o `UInt8` partilha o teto e esta anotado em
`docs/AUTONUMBER.md` como irmao». **Duas linhas gravadas a 1 ns de distancia saem
IDENTICAS no fio**, e a grade de 256 ns significa que qualquer diferenca abaixo de
~128 ns desaparece. O avanco forcado sobrevive no disco e morre no soquete.

Agravante de terceiro: o retrato SHA-256 da bancada e calculado sobre o **JSON**
das linhas (`bancada/replicacao/achados-do-dba.py:170-177`). A ferramenta que
pegou a doenca do `rownum` ficaria **cega** para divergencias menores que a grade.

Consertavel com um segundo caso especial no `valor_para_json` (sair como texto
acima do teto, como a `Sequence` ja faz). Mas ai o campo e um inteiro que viaja
como texto, e a coluna que se pediu para ser data/hora nunca se parece com uma.

### 4.3 FORMA B — dois campos, `rowstamp` (contador) + `rowtime` (relogio). **A que C recomenda**

Bytes por linha 16; slot 118 -> 134 (+13,6%); em 10M linhas 1.125,3 -> 1.277,9 MiB.
**76,3 MiB a mais que a Forma A por dez milhoes de linhas.**

- `rowstamp`: `UInt8`, contador puro do no, comeca em 1, **nunca toca no relogio**.
  E o `xmin`/`DB_TRX_ID` desta casa.
- `rowtime`: `DateTime` (`i64` ms, `types.rs:38-39`), relogio de parede, **pode
  empatar e isso nao e defeito**.

**No fio os dois sao honestos, e isso e medido:** `DateTime` ja sai como texto ISO
(`valores.rs:729`), zero problema de precisao e zero caso especial novo; o contador
comeca em 1 e so cruza 2^53 depois de 9x10^15 commits, que a um milhao de commits
por segundo sao **285 anos**.

No arranque so o `rowstamp` precisa voltar, e voltar e mais seguro que na Forma A
porque ele nao tem relacao nenhuma com o relogio: a semente e
`max(marcas d'agua das tabelas abertas) + 1`. Um no que reinicie com o relogio
bagunçado nao tem como emitir carimbo menor, porque relogio nenhum entra na conta.

### 4.4 As duas examinadas e NAO recomendadas

- **Forma C — so o contador, 8 bytes, sem relogio.** A mais limpa e barata, e a
  hora de parede ja existe no `.log` (`crates/phxsql-store/src/log.rs:10-20`).
  Nao recomendada porque o dono pediu uma coluna de data/hora, e chegar ao evento
  N do diario e caminhar pelos anteriores (`log.rs:38-43`): responder «quando esta
  linha nasceu?» viraria varredura. Fica registrada como a opcao de 8 bytes se o
  dono aceitar trocar leitura humana por espaco.
- **Forma D — um `u64` empacotado, µs no alto + contador embaixo** (HLC/snowflake,
  51+13 bits). Cabe em 8 bytes e e estritamente crescente. Nao recomendada: continua
  com magnitude 1,79x10^18 e morre no fio pelo mesmo numero; e campo empacotado e
  dado que so se le decodificando, a mesma familia do «rotulo se estiliza, dado
  nunca».

### 4.5 Parecer de C

**Forma B.** O argumento nao e gosto: a propria decisao do dono ja diz que a
garantia vem do avanco forcado e nao da resolucao do relogio. Aceito isso, o
relogio dentro do mesmo campo nao compra garantia nenhuma, e cobra a grade medida
no fio. Separa-los custa 76,3 MiB por dez milhoes de linhas e devolve dois campos
que nunca mentem.

## 5. O que a replica honra

O carimbo viaja na imagem e a replica **honra o que veio**. E a doenca do `rownum`
do pedido 291 vista antes de acontecer.

O portao ja existe: `Table::como_replica` (`table.rs:417`), ligado em par por
`aplicar_evento` (`:3983-3997`), `inserir_replicado` (`:4019-4025`),
`atualizar_replicado` (`:4028-4035`) e `excluir_de_vez_replicado` (`:4040-4047`),
consultado por `julga_integridade` (`:1315-1317`). **O carimbo usa o mesmo portao
e nenhum outro.**

**O molde que NAO se deve copiar e o `numerar_linha`** (`table.rs:2377-2392`):
```
if !matches!(valores[i], Value::UInt(n) if n > 0) || anterior.is_none() {
    let reservado = self.reg.rownum_atual();
    valores[i] = Value::UInt(reservado);   // sobrescreve SEMPRE na inclusao
```
O `|| anterior.is_none()` forca o numero local em **toda inclusao**, inclusive nas
que vem de `aplicar_evento_interno` (`table.rs:4083-4086`). **E por isso que o
pedido 309 existe**, e a coluna nova nao pode nascer com o mesmo desenho, senao o
309 ganha um irmao no mesmo dia.

A forma correta, e e uma linha: se `como_replica`, mantem o carimbo que veio e
**empurra o contador local para pelo menos esse valor**; senao, emite. O empurrao
nao e enfeite: sem ele, um evento remoto com carimbo alto entra e a proxima escrita
local sairia com carimbo menor que o de uma linha que ja esta la. E o mesmo remedio
do `anotar_sequencia` (`reg.rs:810-816`).

**No bidirecional**, onde o `rownum` e local por desenho, o carimbo **nao** pode
ser local (o retrato SHA-256 divergiria, como em `252fa89db5038769` x
`d92da11a064d6f11`), e isso traz uma consequencia que precisa estar escrita antes
de alguem prometer o contrario:

> **A garantia «pai estritamente antes do filho» vale DENTRO de um no.** Entre
> dois masters `multi`, os carimbos vem de dois contadores independentes e podem
> se intercalar. **Nao se reivindica ordem global entre nos sem prova**, e ela nao
> existe hoje.

Ha conserto, e e a mesma medicina da 290: se o carimbo do no *i* for emitido na
faixa `carimbo ≡ i (mod N)`, os carimbos de nos diferentes ficam globalmente
distintos por construcao, o empurrao arredonda para cima dentro da propria faixa,
e a ordem por no continua exata. **So e de graca se entrar agora**: depois e
adivinhar em que faixa cada linha ja gravada nasceu.

## 6. O `passo` no esquema e o `inicio` no no

### 6.1 Onde o no declara o `inicio` — e a primeira recusa de C aqui

Hoje a identidade e `replicacao.id_servidor` (`crates/phxsql-server/src/config.rs:275`),
uma `String`, exigida no papel `multi` (`config.rs:3641-3644`).

**C RECUSA derivar o `inicio` de hash do `id_servidor`.** Com `passo = 2` e dois
nos, `hash(nome) mod 2` colide em **50% dos casos**, e colidir aqui e
**literalmente o defeito 229(a)** que a mudanca existe para consertar, agora
produzido pelo conserto e em silencio. Aniversario nao se usa para particionar
espaco de chave.

O `inicio` e inteiro explicito, campo novo em `replicacao`, com tres recusas no
arranque: `inicio >= passo`; dois nos do mesmo cluster com o mesmo `inicio` (a
lista esta em `config.rs:504-533`); e `inicio` diferente do que a tabela ja usou.

Precedente a favor: no MySQL e no MariaDB isso e `auto_increment_increment` /
`auto_increment_offset`, **variaveis de servidor e nao campo do esquema da
tabela** — registrado em `docs/AUTONUMBER.md:321-328`, que tambem nomeia a nossa
divergencia: «a faixa tem de viajar com o arquivo», porque um `.reg` restaurado
noutro servidor precisa continuar sabendo de que faixa e. **A 290 fica exatamente
na forma dos dois primos para o offset (e do no) e diverge para o increment (viaja
no `PSCH`), com a restricao escrita.**

### 6.2 Onde o `inicio` usado fica gravado — e custa ZERO byte

O valor ja esta gravado: `proxima_sequencia` mora nos bytes 36..44 do cabecalho do
volume 1 (`reg.rs:899`, lido em `:416`), e a faixa e uma classe de resto:

```
proxima_sequencia ≡ inicio (mod passo),  sempre que proxima_sequencia != 0
```

Conferido no `abrir`, da a recusa pedida sem gastar um byte e sem inventar um
segundo lugar onde a verdade pode divergir de si mesma.

Tres consequencias para o engenheiro:

1. **`proxima_sequencia == 0` e «nunca usada» e pula a conferencia.** Tabela nova
   nasce com 0 (`reg.rs:302`) e a primeira entrega e forcada a 1 por
   `self.proxima_sequencia.max(1)` (`reg.rs:748`). **E o que faz a tabela nascida
   por replicacao funcionar**: herda o `passo` do bloco do source e adota a faixa
   deste no na primeira escrita.
2. **`anotar_sequencia` tem de arredondar para a PROPRIA faixa.** Hoje
   `usado + 1` (`reg.rs:812-816`), que com faixa e o proximo numero **do outro
   no** — o defeito medido em `docs/AUTONUMBER.md:306-312` («alfa ajustada para 1,
   beta para 1.000.000; depois da primeira ida e volta o contador de alfa estava em
   1.000.002»).
3. **`ajustar_sequencia` e a unica porta dos fundos** (`reg.rs:834-837`, grava
   qualquer valor, inclusive para tras). Ou arredonda para a classe, ou recusa na
   hora. Recusar na hora e melhor: recusar na abertura seguinte transforma uma
   ordem de manutencao numa tabela que nao abre mais.

### 6.3 A marca d'agua do carimbo — os bytes 116..124

Mapa do cabecalho de 128 bytes da versao 4, de `montar_cabecalho`
(`reg.rs:883-933`), conferido campo a campo pelo integrador: 0..8 magica ·
8..10 versao · 10..12 `cab_len` · 12..16 volume · 16..20 `slot_size` ·
20..28 `slot_count` · 28..36 `live_count` · **36..44 `proxima_sequencia`** ·
44..52 `data_offset` · 52..56 `esquema_len` · 56..60 `esquema_crc` ·
60..68 `criado_em` · 68..76 `alterado_em` · 76..84 `primeiro_rowid` ·
84..92 `chave_periodo` · **92..100 `proximo_rownum`** · 100..108 baldes ·
108..116 `marcadas` · **116..124 LIVRE, medido zerado** · 124..128 CRC-32.

**E o ultimo `u64` livre da versao 4.** Cabe exatamente uma marca d'agua, o que
reforca a Forma B, onde so o contador precisa de durabilidade.

**E ela NAO exige subir a versao do `.reg`**, ao contrario do que o
`proximo_rownum` exigiu, e a diferenca e precisa: a nota de `reg.rs:107-114` diz
que a v3 e a v4 subiram porque zero ali significaria uma coisa **falsa** num
arquivo que ja tem linhas. Aqui zero significa «esta tabela ainda nao carimbou
nada», e e **verdade** num `.reg` anterior ao v10, porque a coluna nao existia.
Ausencia benigna, nao sobe versao. O CRC ja cobre `[..124]` (`reg.rs:929-931`).

## 7. A migracao

### 7.1 O custo, medido

De `docs/DESEMPENHO.md` §4.14 (linhas 1680-1745), `--example custo-do-alter`, na
mesma tabela de slot 118:

| linhas | `.reg` antes | alterar | µs/linha |
|---:|---:|---:|---:|
| 1.000.000 | 112,5 MiB | 0,536 s | 0,536 |
| 10.000.000 | 1.125,3 MiB | 5,53 s | 0,553 |

Linear, dominado pelo disco (427 MiB/s lendo e escrevendo). Construir a mesma
tabela custou 90,4 s: **a alteracao e 6,1% do que custou digitar o dado.** Pico de
espaco 2x o `.reg`, mais 2x o `.bkp` quando ha espelho. Para 100 milhoes de linhas,
~55 s e ~24 GiB de pico: janela de parada de menos de um minuto por tabela.

### 7.2 Por que NAO ha caminho que evite reescrever

**(a) O bloco novo nao cabe onde o velho esta.** `data_offset` e calculado quando a
tabela nasce e nao se mexe: `alinhar(cab_len + len(bloco), 64)` (`reg.rs:1253`).
Medido: folga de **50 bytes** contra um bloco que cresce **145**. O
`gravar_cabecalho` ja recusa com «gravar aqui destruiria dado» (`reg.rs:865-872`),
guarda que `reg.rs:859-864` diz ser «inalcancavel hoje e existe para o dia em que
alguem mudar isso sem perceber». **Este e o dia.** No pior caso a folga e zero.

**(b) Duas larguras de slot nao existem neste formato.** `slot_size` e um campo so
(`u32`, bytes 16..20), e e dele que sai `offset = data_offset + (slot-1) * slot_size`
(`reg.rs:4-11`). Um mapa `rowid -> offset` custaria 8 bytes por linha (80 MiB em
10M) e trocaria uma multiplicacao por uma busca: medido em `DESEMPENHO.md` §4.14,
ler pela conta custa **1,08 µs** e descendo arvore custa **2,55 µs**, **2,36x em
toda leitura para poupar uma passada uma vez**. Recusado com o numero.

### 7.3 O caminho, e o buraco que ele tem hoje

**`acrescentar_coluna` NAO serve como esta** (`table.rs:809-814`): recusa nome de
coluna de sistema. A migracao precisa de caminho proprio, que use a maquinaria de
`RegFile::acrescentar_coluna` (`reg.rs:1208+`: conta de offsets, `*.novo`,
`rename`, retomada de queda em `reg.rs:517+`) mas entre por outra porta e **troque
o bloco de esquema inteiro para v10 no mesmo passo**.

### 7.4 Source e replica sobem JUNTOS, e a REPLICA PRIMEIRO

`op_posicao` com `com_esquema` manda `t.esquema().serializar()`
(`servidor.rs:22337-22346`), uma re-serializacao na versao corrente e nao os bytes
crus do disco — o comentario ali diz «CRU, do jeito que mora no `.reg`» e **isso e
impreciso; vale corrigir no mesmo commit**.

- source v10 -> replica v9: o bloco chega com versao 10 e `desserializar` recusa
  (`schema.rs:1361`). **Fail-stop, alto e claro.** E a prova de que nao da para
  subir o source primeiro.
- replica v10 -> source v9: funciona. A replica le um bloco v9, cria a tabela sem a
  coluna, e o payload da imagem casa byte a byte.

→ **A ordem do rollout e: REPLICA PRIMEIRO, depois o source.** Nao e indiferente, e
nao esta escrito em lugar nenhum hoje.

### 7.5 A tabela que NAO pode migrar

`table.rs:800-807` recusa coluna nova em tabela em modo **ledger**: «o hash de cada
bloco cobre o conteudo na ordem do esquema, e acrescentar coluna faria a
verificacao acusar adulteracao numa cadeia intacta».

→ **Toda tabela ledger fica em v9 para sempre**, ou a cadeia quebra. **Isso nao
esta em nenhum dos tres pedidos e tem de ir a mesa do dono antes do bump**, porque
depois vira descoberta. Nota a favor: `ledger.rs:205` ja usa `e_coluna_de_sistema`,
entao o hash do ledger **exclui** colunas de sistema, o que sugere que uma tabela
ledger **nascida** em v10 funciona. O que nao funciona e **migrar** uma que ja tem
cadeia.

## 8. As sete recusas de C

Nenhuma revoga a decisao do dono; cada uma nomeia o que quebraria se o item entrasse
como esta escrito.

**NAO 1 — Recuso o carimbo `UInt8` de nanos entregue pelo `Json` sem crivo.**
Medido acima: a grade do `f64` naquela magnitude e de 256 ns, duas linhas a 1 ns de
distancia saem identicas, e o retrato SHA-256 da bancada ficaria cego para
divergencias abaixo de ~128 ns. Ou entra a Forma B, ou entra um segundo crivo de
2^53 no `valor_para_json` ao lado do da `Sequence`. Entregar sem um dos dois e
entregar uma garantia que o protocolo desfaz. *(Nota do integrador: isto fecha a
dispensa registrada em `docs/AUTONUMBER.md` §C.4, onde a frente G2 recusou alargar
o teto ao `Int8`/`UInt8` sem papel C, e onde vive a guarda
`a_faixa_imprecisa_continua_passando_por_decisao_registrada`, que cai no dia em que
C decidir. C decidiu.)*

**NAO 2 — Recuso derivar o `inicio` de hash do `id_servidor`.** 50% de colisao com
`passo = 2`, silenciosa, produzindo o proprio 229(a).

**NAO 3 — Recuso `criado_em` como nome.** Toda tabela de usuario com
`criado_em DATETIME` ficaria inabrivel e incriavel depois do bump.

**NAO 4 — Recuso subir a versao do `.reg` (4 -> 6).** `reg.rs:388-395` recusa fora
de {4, 5}, e o proprio migrador leria pelo mesmo portao: todo `.reg` existente
ficaria ilegivel sem caminho de volta. A marca d'agua vai nos bytes 116..124, cuja
ausencia e benigna.

**NAO 5 — Recuso a entrega sem os quatro lugares de §3.3.** Meia funcionalidade
aqui e pior que nenhuma: a coluna entra, o banco fica maior, e a sincronia para.

**NAO 6 — Recuso reivindicar «pai estritamente antes do filho» no bidirecional.**
A garantia e por no. Ou entra a faixa por no no carimbo (§5), ou a garantia se
declara com o alcance que tem. Nao se reivindica o que nao se prova, a mesma lei do
`SERIALIZABLE`.

**NAO 7 — Recuso reescrever o bloco `PSCH` no lugar.** Folga de 50 bytes contra um
bloco que cresce 145.

**Aviso que nao e recusa:** tabela em modo ledger nao migra. Vai a mesa do dono.

## 9. Onde este desenho se apoia na regua dos motores

- **(a) Convergencia que o desenho ADOTA, e que ainda e hipotese:** a ordem mora
  num contador e o relogio fica para leitura humana. C mediu so o indicio da
  pesquisa da casa; **nao mediu** `xmin`/LSN, `DB_TRX_ID` nem o `CURRENT_TIMESTAMP`
  constante na transacao. **Se o papel J confirmar o trio, a Forma B deixa de ser
  recomendacao e vira aceite automatico**, e a Forma A passa a ser a que diverge
  dos tres. C anota qual seria a restricao nossa a justificar a divergencia:
  **nenhuma** — a nossa restricao (o `Json` de um tipo numerico so) empurra para o
  **mesmo** lado do trio.
- **(b) Convergencia que o desenho JA SEGUE:** offset da auto-numeracao e do
  servidor, increment e comum (`auto_increment_offset` / `auto_increment_increment`,
  `docs/AUTONUMBER.md:321-328`), com a nossa divergencia nomeada: a faixa tem de
  viajar com o arquivo, porque um `.reg` restaurado noutro servidor precisa
  continuar sabendo de que faixa e.
- **(c) Onde a convergencia bate numa petrea, e o choque APARECE:** os quatro
  maduros reaproveitam espaco de linha morta (`VACUUM`/purge/pagina livre). A ordem
  de digitacao proibe (`reg.rs:15-24`). Nao e assunto deste bump; fica anotado
  porque a pergunta «por que nao compactar?» vem junto da conta de espaco.
- **(d) Onde C NAO se apoiou na regua, de proposito:** as quatro decisoes de layout
  de bytes (bloco no fim, lista propria com contagem, truncado-e-erro, versao sobe)
  saem do precedente medido desta casa, v3 a v9, e do erro que cada uma custou aqui.
  Formato de arquivo e onde a convergencia menos vale: os quatro tem formatos
  incompativeis entre si.

## Resumo de uma linha

`PSCH` v10 = v9 + **uma coluna de sistema no fim** (zero byte de bloco; o bump
serve para **recusar** o binario velho, nao para carregar campo) + **dois blocos no
fim** (`passo`/`ALWAYS`/nome da sequencia; `Uuid` automatico), com ausencia = como
era antes e truncamento = erro. O `inicio` sai da identidade do no e se confere pelo
resto de `proxima_sequencia` modulo `passo`, a custo zero. A marca d'agua do
contador vai nos 8 bytes livres 116..124 do cabecalho do `.reg`, **sem subir a
versao do `.reg`**. Migracao por reescrita completa: 0,553 µs/linha, 5,53 s em 10M,
pico 2x, **replica antes do source**, e **tabela ledger nao vai**.

**Recomendacao de C sobre a §4: Forma B, duas colunas, 16 bytes/linha** (+76,3 MiB
por 10M de linhas em relacao a Forma A). A decisao e do dono; o preco das quatro
formas esta acima, medido.

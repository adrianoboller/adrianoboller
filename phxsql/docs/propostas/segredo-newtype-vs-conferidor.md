# Parecer do papel J: newtype `Segredo(String)` contra «onze `impl Debug` à mão mais um conferidor de texto»

**Papel J (pesquisa), 17/09/2026, só leitura.** Este é o único arquivo que
esta frente escreveu. Nenhuma linha de `crates/`, `bancada/` ou dos outros
`docs/` foi tocada; os experimentos correram numa **cópia** do workspace no
scratchpad da sessão, a partir do commit `74de67e`, e os números abaixo saem
dela — do compilador, não de `grep`.

**A pergunta:** o conserto de `74de67e` (nove `impl std::fmt::Debug` à mão,
cada um desestruturando sem `..`) mais o conferidor de texto que a frente irmã
construiu (`bancada/guardas/debug-com-segredo.py`, 807 linhas, já no disco e
ainda não comitado) é o desenho certo — ou um newtype cujo `Debug` esconde por
construção tornaria os `impl` desnecessários e o conferidor menor?

**Veredito em uma linha: (b) — o newtype, com o conferidor REDUZIDO às partes
que o tipo não substitui. E a migração custa 135 linhas alteradas e 361
removidas, medidas, sem mudança de formato em disco — é agora que ela é
barata.**

O que sustenta a linha, em três números: **152** pontos de uso quebram quando
os 16 campos mudam de tipo, dos quais **71** são código de produto e só **34**
precisam expor o segredo de verdade; nos dois crates pequenos a migração fechou
com **zero** erro em cascata e **zero** aviso de `clippy`; e as duas partes do
conferidor que a cognição das 00:38 mediu como **invisíveis quando morrem**
(m4, a detecção do `derive`; m5, a leitura do `impl` à mão) são exatamente as
duas que o newtype torna desnecessárias.

---

## 0. O que o briefing dizia errado — e o que eu li errado depois

Briefing de orquestrador também é número citado. Conferido:

1. **«nove structs em quatro crates».** São **três** crates — `phxsql-odbc`,
   `phxsql-server`, `phxsql-sql` (`git show 74de67e --stat`: cinco arquivos de
   fonte). A revisão SEC (`docs/propostas/revisao-sec-saidas-de-segredo.md`
   §11.1) mediu antes de mim.
2. **«nove `impl`».** Hoje há **onze** `impl Debug` à mão escondendo segredo em
   `String` — as nove de `74de67e` mais `Cifra` (`config.rs:1345`) e `CifraFio`
   (`config.rs:1654`), que já faziam isso antes — e **duas** escondendo chave em
   `[u8; 32]` (`cofre::Chave`, `store/cofre.rs:124`; `fio::Transporte`,
   `core/fio.rs:361`). O newtype de texto alcança as onze; as duas de bytes
   ficam como estão.
3. **«14 campos».** Contados pelo **tipo** e não pelo `derive`, há um décimo
   sétimo portador de segredo: `odbc::Canal.token` (`odbc/conexao.rs:226`).
   Não deriva `Debug`, então não é defeito de `Debug` — e é por isso que o
   crivo por nome+`derive` nunca vai olhá-lo. Um inventário por tipo o vê.
4. **«o conferidor vira quase trivial».** Não vira. O léxico de nomes e as
   isenções (cinco do briefing mais duas que a frente irmã achou:
   `PapelDeChave.chaves_estrangeiras`, `Transacao.chaves`) **ficam**, porque
   são por nome e o tipo não os dispensa. O que sai é a detecção do `derive` e
   a leitura do `impl` — que são, medido, as partes mais frágeis dele (§4).
5. **«cada `self.senha` que hoje é `String` vira `.expor()`».** Não: dos 152
   pontos, **48** são `token: "t".into()` de teste, absorvidos por um
   `From<&str>` sem tocar em linha nenhuma; **28** são construção
   (`.to_string()`, `String::new()`, campo por nome no `de_json`); **8** são
   «está vazio?». Só **34** pontos de produto expõem o valor (§2).
6. **«o newtype fecha `{:?}`».** Fecha mais: `{}` (sem `Display`) e `==` (sem
   `PartialEq`) deixam de compilar. O que ele **não** fecha está no §3, e é
   maior do que o briefing supunha: as cópias que existem **antes** de o tipo
   existir, e as que estão em disco por desenho.
7. **«outra frente está construindo agora»** — está construído: 807 linhas,
   `TETO_DEBUG_COM_SEGREDO = 0` (linha 79), léxico de 18 palavras (linha 87),
   autoteste de 26 casos, e o `--numeros` roda o autoteste antes de responder
   (cognição `cognicao_catraca-que-nasce-em-zero-nao-distingue-regua-morta_20260917_0038.md`).
   Eu li o arquivo, não a descrição.

**O que eu mesma errei no caminho, e está aqui para não voltar:** a primeira
divisão produto/teste dos 152 pontos deu **15 de produto** — o script pegou o
primeiro `#[cfg(test)]` de cada arquivo, e `servidor.rs` tem um na **linha 21**
(`use crate::apoio_teste::DirTemp`), então `servidor.rs:2327` saiu como teste.
O número certo, pelo primeiro `#[cfg(test)] mod`, é **71**. E a primeira prova
do «apagar a memória» lia a memória **depois** do `free` — media o alocador,
não o apagar (o controle sem `Drop` deu o mesmo byte). A prova certa intercepta
o `dealloc` (§5).

---

## 1. Como os outros fazem — lido no fonte, com a linha

### 1.1 `secrecy` 0.10.3 (`iqlusioninc/crates`, `secrecy/src/lib.rs`, 348 linhas)

- **Objetivos declarados** (linhas 4–10): *«Make secret access explicit and
  easy-to-audit via the `ExposeSecret` … traits; prevent accidental leakage
  … via debug logging; ensure secrets are wiped from memory on drop»*. E a
  limitação declarada (12–17): **não** faz `mlock`/`mprotect`; quem quiser
  isso vai à crate `secrets`, que depende de `libc`.
- **O tipo** (56–60): `SecretBox<S: Zeroize + ?Sized> { inner_secret: Box<S> }`.
  `SecretString = SecretBox<str>` (215).
- **`Debug`** (142–146): `write!(f, "SecretBox<{}>([REDACTED])",
  any::type_name::<S>())`. Marcador fixo, sem tamanho.
- **Exposição** (160–162): `expose_secret(&self) -> &S`. Não há `Deref`, não
  há `Display`, não há `PartialEq`. **Todo** uso do valor passa por uma chamada
  com esse nome — é o que torna a exposição auditável por `grep`.
- **`Clone`** (148–156): só para `S: CloneableSecret` — opt-in por tipo, para
  desencorajar cópias.
- **`Drop`** (69–73): chama `zeroize()`.
- **`Serialize`** (300, 328): opt-in por marcador `SerializableSecret`; por
  padrão o tipo **não** serializa.

### 1.2 `zeroize` 1.9.0 (`RustCrypto/utils`, `zeroize/src/lib.rs`, 850 linhas)

- `impl Zeroize for String` (561–565): `self.as_mut_vec().zeroize()`; para
  `Vec`, zera a capacidade sobressalente e os elementos (doc: *«Cannot ensure
  that previous reallocations did not leave values on the heap»*).
- O mecanismo é `core::ptr::write_volatile` (737, 763) — **é `std`**, não
  precisa de crate. Não há `compiler_fence` no arquivo (grep: zero); a garantia
  declarada (142–148) é a semântica *volatile* do LLVM.
- **Limites declarados** (155–175): ataques microarquiteturais, *stack
  spilling*, `move` e realocação deixam cópias que o `zeroize` não vê.

### 1.3 A `std`

- `Mutex<T>`: `Debug` à mão que põe um **marcador no lugar do valor** —
  `d.field("data", &"<locked>")` (`library/std/src/sync/poison/mutex.rs:710`).
  É o mesmo idioma do `&"(oculta)"` desta casa; o precedente é da biblioteca
  padrão.
- `DebugStruct::finish_non_exhaustive` (`library/core/src/fmt/builders.rs:220–232`,
  estável desde 1.53): esconde por **omissão** (`, .. }`). Esta casa a recusou
  de propósito em `74de67e` — omitir esconde o campo novo em silêncio; o
  marcador diz que o campo existe e está oculto.
- `#[derive(Debug)]` exige `Debug` em todo campo. **Medido** (corrida 0, §4):
  struct derivando `Debug` com um campo cujo tipo não implementa `Debug` dá
  `error[E0277]: Segredo doesn't implement Debug` na linha do `derive`.

### 1.4 Os clientes dos três motores, em Rust

| biblioteca | o que faz com a senha no `Debug` | linha |
|---|---|---|
| `tokio-postgres` (`sfackler/rust-postgres`, `tokio-postgres/src/config.rs`) | **`impl` à mão** com um `struct Redaction` local que imprime `_` | 764–776 |
| `mysql_async` (`blackbeam/mysql_async`, `src/opts/mod.rs`) | **`impl` à mão**, `.field("pass", &"<REDACTED>")` | 760–764 |
| `sqlx` PostgreSQL (`launchbadge/sqlx`, `sqlx-postgres/src/options/mod.rs`) | **`#[derive(Debug, Clone)]` com `password: Option<String>` dentro** — imprime | 18–24 |
| `sqlx` MySQL (`sqlx-mysql/src/options/mod.rs`) | idem | 66–72 |

Dois escrevem à mão (o desenho (a) desta casa), um deriva e vaza, **nenhum usa
newtype**. Isto **não** dispara o aceite automático dos três motores — a regra
é sobre o que o *banco* faz, não sobre a biblioteca cliente —, mas é o estado
medido do ecossistema: o defeito de `74de67e` está vivo no `sqlx` de hoje.

---

## 2. O custo aqui, contado pelo compilador

### 2.1 Método

Cópia do workspace no scratchpad, em `74de67e`. Em `phxsql-core` entrou um
`Segredo(String)` **sem** `Debug`, `Display`, `PartialEq` e `Deref` — de
propósito, para que **todo** uso do valor quebre e o compilador conte. Os 16
campos (os 14 de `74de67e` mais `Cifra.senha` e `CifraFio.chave_privada`)
mudaram de tipo:

| arquivo | linhas dos campos |
|---|---|
| `crates/phxsql-server/src/config.rs` | 152, 163, 166 (`Origem`); 446, 448 (`Cluster`); 1136 (`Email`); 1298 (`Cifra`); 1644 (`CifraFio`); 2061 (`Rest`); 2896 (`Config`) |
| `crates/phxsql-server/src/dblink/mod.rs` | 125, 143 (`Definicao`) |
| `crates/phxsql-server/src/usuarios.rs` | 683 (`Usuario`) |
| `crates/phxsql-sql/src/usuario.rs` | 55 (`Comando`, `Option<String>`) |
| `crates/phxsql-odbc/src/conexao.rs` | 47, 49 (`Receita`) |

Duas corridas de `cargo check --all-targets --offline` (a do `phxsql-sql`
separada, porque o server depende dele), erros deduplicados por
`arquivo:linha:coluna`. **Limite do método:** os `examples/` e os `tests/` de
integração do `phxsql-server` não foram conferidos, porque a `lib` não compila
durante a medição; o teto por `grep` neles é **1** ponto
(`tests/mcp_http.rs:67`, `c.rest.token = BEARER.into()` — absorvido pelo
`From<&str>`).

### 2.2 O número

**152 pontos, 151 linhas distintas** (`servidor.rs:2327` tem dois). Por
arquivo:

| arquivo | produto | teste |
|---|---:|---:|
| `phxsql-server/src/servidor.rs` | 18 | 55 |
| `phxsql-server/src/config.rs` | 25 | 14 |
| `phxsql-server/src/dblink/mod.rs` | 13 | 0 |
| `phxsql-odbc/src/conexao.rs` + `lib.rs` | 7 | 5 |
| `phxsql-sql/src/usuario.rs` | 4 | 3 |
| `phxsql-server/src/usuarios.rs` | 2 | 3 |
| `phxsql-server/src/replica.rs` | 2 | 0 |
| `phxsql-server/src/rest.rs` | 0 | 1 |
| **total** | **71** | **81** |

`phxsql-cmd`, `phxsql-cli` e `phxsql-ffi` não tocam campo nenhum dos 16
(`grep`: zero).

### 2.3 Os 71 de produto, por tipo de uso — a resposta à pergunta «quantos K expõem mesmo»

| tipo de uso | quantos | o que vira | expõe? |
|---|---:|---|---|
| construção `.to_string()` / atribuição de `String` | 16 | `.into()` | não |
| construção `String::new()` | 6 | `Segredo::default()` | não |
| construção por nome no `de_json` (`senha,` / `token,`) | 5 | `senha: senha.into()` | não |
| construção de variável (`senha_hash: hash,` `usuarios.rs:1118`) | 1 | `.into()` | não |
| `derive(PartialEq)` do `Comando` (`sql/usuario.rs:48`) | 1 | sai o `PartialEq` | não |
| «está vazio?» (`.is_empty()`; a lógica `_env` de `dblink/mod.rs:305`, `config.rs:3519`…) | 8 | `esta_vazio()` no tipo | não |
| serialização para disco / pedido / login (`Json::texto_de(&self.senha)`: `dblink/mod.rs:299, 306`; `sql/usuario.rs:85`; `odbc/conexao.rs:289`; `config.rs:1262, 1586, 1781, 2154`…) | 10 | `.expor()` | **sim** |
| criptografia / autenticação (`token_confere` `config.rs:3650`; `hmac_sha256(config.token.as_bytes())` `servidor.rs:9276`; `senha::conferir(…, &senha_hash)` `usuarios.rs:798`, `config.rs:1491`; `sal_e_iteracoes`/`derivado_do_hash` `servidor.rs:9270, 9390`; `cofre::definir_com` `config.rs:1555`; `chave_de_hex` `config.rs:1739`) | 8 | `.expor()` | **sim** |
| passagem como `&str` a quem autentica no outro lado (`autenticar`/`apertar_a_mao`: `dblink/mod.rs:359, 363, 437, 461`; `replica.rs:413, 422`; `servidor.rs:3013, 3029, 3745, 3757`; `config.rs:1209, 1539`) | 12 | `.expor()` | **sim** |
| comparação `==` (`servidor.rs:7613`, o `Bearer` do REST) | 1 | `confere()` em tempo constante | sim, por dentro |
| clone para uma `String` de fora (`config.token.clone()` `servidor.rs:7616, 22347`; `odbc/conexao.rs:273`) | 3 | `.expor().to_string()` | **sim** |
| **total** | **71** | | **34 expõem** |

Ou seja: **34 dos 71** pontos de produto são lugares onde o segredo **tem** de
sair — para o disco (`para_disco`), para o fio (login do ODBC, `autenticar` do
DbLink e da réplica), para o hash. Hoje esses 34 são invisíveis a um `grep`:
`&self.senha` em `dblink/mod.rs:437` tem a mesma cara que `&self.nome`. Com o
tipo, são 34 ocorrências de `expor(` — auditáveis uma a uma, que é o primeiro
objetivo declarado do `secrecy` (§1.1) e que o desenho de hoje não tem.

### 2.4 Os 81 de teste

**48** são `token: "t".into()` (todos em `servidor.rs`) — não mudam. Sobram
**33**: 20 `assert_eq!(r.senha, "…")` que viram `r.senha.expor()`, 8
`String::new()`, 3 `as_deref()` do `Comando`, 2 outros.

### 2.5 Sonda de cascata: os dois crates pequenos, migrados de verdade

Na cópia, com o `Segredo` completo (43 linhas: `Clone`, `Default`, `From<String>`,
`From<&str>`, `expor()`, `esta_vazio()`, `confere()` em tempo constante, `Debug`
que escreve `(oculto)` — sem `Display`, sem `PartialEq`, sem `Deref`):

| crate | linhas alteradas (diff contra `74de67e`) | erro em cascata | `clippy --all-targets` |
|---|---:|---:|---|
| `phxsql-sql` | **9** (7 pontos + o `derive` + 1) | **1** — `assert_eq!(comando(texto).unwrap(), None)` (`usuario.rs:313`) precisa de `PartialEq`; vira `.is_none()` | **0 avisos** |
| `phxsql-odbc` | **14** (12 pontos + 2 linhas de tipo) | **0** | **0 avisos** |

**Não medido:** a cascata no `phxsql-server` (58 pontos de produto, 73 de
teste). Os dois crates pequenos deram 1 e 0; o grande não foi migrado.

---

## 3. O que o newtype NÃO resolve

1. **`.expor()` dentro de um `impl Debug`/`Display`/`para_json`.** É o mesmo
   defeito por outro caminho, e o tipo não o impede. O que muda: hoje o
   conferidor irmão precisa **ler o `impl`** para saber se ele toca o campo
   (`le_o_campo`, `debug-com-segredo.py:469`); com o tipo, a regra é textual
   e de uma linha — `expor(` não aparece dentro de `impl … Debug`/`Display`.
   No `para_json` a exposição é **legítima** (o `para_disco` de
   `dblink/mod.rs:299` existe para gravar a senha), e só a leitura decide —
   as seis provas de `74de67e` continuam sendo o que fecha isso.
2. **As cópias que existem antes de o tipo existir.** No `CREATE USER …
   PASSWORD 'x'` a senha passa por **cinco** `String` antes de virar hash:
   a linha lida do soquete; o `Json::Texto` do campo `texto`; o
   `Token::Texto(texto)` do léxico (`sql/lexico.rs:303`); o `t.clone()` que
   vira `Comando.senha` (`sql/usuario.rs:233`); e o `Json::texto_de(s)` do
   `pedido()` (`sql/usuario.rs:85`, `s.into()` de `&String` clona). O newtype
   é dono de **uma** das cinco. Quem fecha as outras é o `sem_a_senha`
   (`sql/usuario.rs:165`) e a redação por análise do Profiler — e a revisão
   SEC achou que a lista por nome do Profiler não tem `token_remoto` (A1).
3. **As cópias em disco, por desenho.** Dos 16 campos, **15** têm cópia
   persistida: `token` no `config.json` (`exemplos/Config_exemplo_01.json:6`),
   `senha_hash` dos usuários (:420–459), `senha`/`token_remoto` do
   `dblink.json` (`para_disco`, `dblink/mod.rs:299, 306`), a linha de conexão
   do ODBC (`conexao.rs:130, 132`). Só `Comando.senha` é transitória. Um
   segredo que está em claro num arquivo `0644` (SEC A4) não fica mais
   protegido por um newtype na memória.
4. **Campo de segredo com nome fora do léxico, em qualquer struct.** Nem o
   tipo nem o conferidor veem `pin: String`. É o mesmo buraco de hoje, para os
   272 `derive(Debug)` do repositório (structs e enums, contados por linha).

---

## 4. O que o conferidor NÃO resolve e o newtype resolve

1. **A décima struct.** Corrida 0, medida: `#[derive(Debug)] struct Decima {
   senha: Segredo }` com um `Segredo` **sem** `Debug` → `error[E0277]` na linha
   do `derive`; ela não nasce. Com o `Segredo` de `Debug` redator (a variante
   recomendada), ela **nasce derivando e imprime `senha: (oculto)`** — saída
   medida: `Decima { nome: "ana", senha: (oculto) }`. Nos dois casos é catraca
   do compilador; o conferidor de texto é catraca de script que alguém precisa
   rodar — e a cognição das 00:38 mediu que, quando a parte dele que detecta
   `derive` morre (m4) ou a que lê o `impl` morre (m5), **o número continua
   0**, indistinguível da árvore sã. **As duas partes do conferidor que morrem
   invisíveis são as duas que o tipo dispensa.**
2. **A auditoria da exposição.** 34 `expor(` em vez de 34 `&self.x`
   indistinguíveis (§2.3).
3. **`{}` e `==`.** Sem `Display`, `format!("{}", def.senha())` para de
   compilar; sem `PartialEq`, o `==` de `servidor.rs:7613` (`apresentado ==
   self.config.rest.token`) para de compilar e é **obrigado** a passar por um
   `confere()` em tempo constante — a mesma regra que o token do protocolo já
   cumpre em `token_confere` (`config.rs:3649–3660`) e o `Bearer` do REST hoje
   não cumpre (`String == String` compara comprimento e sai no primeiro byte
   diferente). Achado lateral, gravidade baixa; fica registrado.
4. **A guarda do catálogo trava a LEI, não a struct.** Hoje há **2** entradas
   com catraca (`debug-da-cifra-mostra-a-senha`, `debug-da-ligacao-mostra-a-senha`)
   para **11** structs, e as outras 9 só têm prova. Com o tipo, **uma** entrada
   em `segredo.rs` — trocar `f.write_str("(oculto)")` por
   `f.write_str(&self.0)` — repõe o defeito nas 11 de uma vez, **compila** (a
   troca de tipo não compilaria; guarda que não compila não prova nada) e as
   seis provas caem juntas.
5. **O campo que não deriva nada.** `Canal.token` (§0.3) e os que a SEC listou
   sem `Debug` (`Sessao`, `replica::Cliente`, `dblink::phx::Conexao`,
   `email::Sessao`): o crivo por `derive` não olha para eles por definição; a
   regra «nome do léxico com tipo que não é `Segredo`» olha.

---

## 5. Memória: é outra pétrea? — o parecer, com o número

A pétrea diz *arquivo, log, resposta*. A revisão SEC (§8) já recomendou **não**
estender à memória, com três motivos de leitura. Aqui vai o que eu **medi**, e
ele aponta para o mesmo lado:

- **A `std` basta para apagar.** `std::ptr::write_volatile` + `compiler_fence`
  num `impl Drop` compilam e rodam sem crate (`zera_alloc.rs`, no scratchpad).
  Custo: 8 linhas — e o **primeiro `unsafe` do `phxsql-core`**, que hoje tem
  **zero** (contados: core 0, store 0, sql 0, server 5, odbc 78, ffi 81).
- **O que chega ao `dealloc`, interceptando com um `#[global_allocator]` da
  `std`**, em `-C opt-level=0`, `3` e `3 + lto=fat`:

  | variante | bytes no `dealloc` |
  |---|---|
  | A — ninguém apaga (o que esta casa faz hoje em todos os 16 campos) | os 17 bytes da senha, **intactos**, nos três níveis |
  | B — `*b = 0` comum | 17/17 zero nos três níveis |
  | C — `write_volatile` | 17/17 zero nos três níveis |

  **Número não confirmado:** a eliminação de escrita morta, que é o motivo de o
  `zeroize` usar *volatile*, **não** se reproduziu aqui — e não por ser
  mentira: o espião no `dealloc` torna as escritas observáveis, que é
  justamente o que impede o otimizador de eliminá-las. O experimento prova que
  a `std` apaga; não prova que a escrita comum falha. Como *volatile* custa
  zero, a escolha certa é *volatile* de qualquer jeito.
- **O que o apagar compraria aqui:** dos 16 campos, **1** não está em disco
  (`Comando.senha`); desse 1, o tipo é dono de **1 das 5 cópias** (§3.2). E as
  onze structs derivam `Clone` — cada `config.clone()` multiplica as cópias que
  o `Drop` não vê. Regra que não se consegue cumprir é regra que se ignora.

**Parecer: memória não entra na pétrea agora.** Entra como item medido —
«ganho de 1/5 de 1/16, ao custo do primeiro `unsafe` do core» — e a cláusula só
muda no dia em que o caminho do `CREATE USER` tiver menos cópias. O único
caminho por onde a memória vira uma das três saídas é o *core dump* (SEC A6),
e esse se fecha com `setrlimit(RLIMIT_CORE, 0)` por `extern "C"`, sem crate.

---

## 6. Os três desenhos, com custo e o que cada um deixa aberto

| | **(a) hoje**: 11 `impl` sem `..` + conferidor completo | **(b)** newtype + `derive(Debug)` de volta + conferidor reduzido | **(c)** newtype + conferidor completo |
|---|---|---|---|
| linhas de `impl` | **361** (9 de `74de67e` = 335; `Cifra` 14; `CifraFio` 12) | **0** | 0 |
| tipo | — | **43** linhas (sem `Drop`, sem os testes do tipo — não escritos) | 43 |
| conferidor | **807** linhas; léxico 18, isenções 7, detecta `derive` + lê `impl`; autoteste 26 | as mesmas partes de nome/tipo/isenção, **menos** m4 e m5, **mais** «tipo não é `Segredo`» e «`expor(` fora de `impl Debug/Display`». Tamanho não medido | 807 + a regra do tipo |
| a décima struct | só o script vê — e m4/m5 morrem invisíveis | **o compilador**: nasce derivando e imprime `(oculto)` | idem (b); o script mede 0 para sempre nos campos `Segredo` |
| campo novo numa das 11 | **para de compilar** no `let Struct { … } = self` — é o que (a) tem e (b) perde | entra no `derive`; se o nome casa o léxico e o tipo é `String`, o conferidor reduzido acusa; se não casa, entra calado — como em qualquer um dos outros 272 `derive(Debug)` | idem (b) |
| `{}` / `==` | abertos (fechados hoje só porque nenhuma das 11 tem `Display`) | **fechados pelo tipo** | fechados |
| auditar a exposição | impossível por texto | **34 `expor(`** | 34 |
| guarda no catálogo | 2 entradas para 11 structs; 9 só com prova | **1 entrada**, em `segredo.rs`, repõe nas 11 e compila | 1 |
| migração | 0 | **135 alteradas** (16 tipo + 11 `derive` + 71 produto + 33 teste + 4 provas com `(oculta)`) **+ 361 removidas + 43 acrescentadas**; 2 entradas do catálogo viram 1; `SEGURANCA.md` §16 ganha um adendo | idem |
| formato em disco | — | **não muda** (o JSON gravado é o mesmo) | não muda |

**O que (a) tem e (b) perde, dito com todas as letras:** a desestruturação sem
`..` obriga quem acrescenta um campo a **decidir** se ele é segredo — em 11
structs. É uma catraca real, e o custo dela é 361 linhas que só valem para
essas 11. Em (b) a decisão se muda de lugar: para o **tipo** do campo, em
**qualquer** struct. Quem escreve `pin: String` não decidiu nada nos dois
desenhos; a diferença é que em (b) o léxico do conferidor é a única rede, e em
(a) é a única rede para os outros 261 `derive(Debug)`.

**(c) não compra nada medível sobre (b):** com todos os campos de texto em
`Segredo`, a detecção de `derive` e a leitura de `impl` medem **0 por
construção** — e continuam com o defeito de m4/m5, que é morrer em silêncio.
A versão honesta de (c) é (b).

**Entra cedo ou já é tarde?** Não é formato em disco — o DBA não tem migração
aqui. O que cresce é o número de pontos: 16 campos e 71 usos hoje, 17 campos
contando o `Canal.token`, e cada segredo novo acrescenta os dele. Os dois
crates pequenos fecharam em 9 e 14 linhas com clippy limpo. **É agora.**

**Uma colisão de nome, medida:** já existe um `struct Segredo` **privado** em
`crates/phxsql-store/src/cofre.rs:140` (a senha do cofre com as iterações).
Não conflita em compilação — é privado —, mas dois `Segredo` com sentidos
diferentes na mesma base é o tipo de coisa que engana quem lê. O nome do
newtype é decisão do integrador; o fato fica aqui.

---

## 7. Onde esta lógica DIVERGE do `secrecy`, e qual restrição nossa causou a divergência

Se a resposta fosse «em lugar nenhum», seria cópia. Não é:

| # | `secrecy` faz | aqui sai diferente | a restrição que obrigou |
|---|---|---|---|
| 1 | `SecretBox<S: Zeroize + ?Sized>` genérico, com `Box<S>` | `Segredo(String)` concreto, sem genérico e sem `Box` | **zero dependências**: o `Zeroize` é uma crate; e a casa serializa por `Json::texto_de(impl Into<String>)` (`core/json.rs:208`), então o portador natural é a `String` da `std` |
| 2 | `Clone` opt-in por `CloneableSecret` (148–156) | `#[derive(Clone)]` | as 11 structs derivam `Clone` e o `token` é clonado em `servidor.rs:7616, 22347`; sem a pétrea da memória (§5), lutar contra o clone é custo sem garantia |
| 3 | nenhuma comparação | `confere(&self, &str) -> bool` em tempo constante, e **sem** `PartialEq` | `iguais_em_tempo_constante` já é lei da casa (`core/hash.rs`, `token_confere`); o `==` do REST em `servidor.rs:7613` é o único ponto que a ausência do `PartialEq` obriga a consertar |
| 4 | nenhum «está vazio» | `esta_vazio()` sem expor | **8** pontos de produto perguntam só isso (a lógica `_env`: o segredo veio da variável de ambiente ou do arquivo?) — 8 exposições a menos |
| 5 | `SecretBox<str>([REDACTED])` com o nome do tipo | `(oculto)`, sem tipo e sem tamanho | regra da casa em `74de67e`: «não se mascara com asteriscos — o tamanho já é informação»; e o `_env` fica visível ao lado, porque diz de onde a credencial devia ter vindo |
| 6 | `Serialize` opt-in por marcador no tipo | exposição explícita por `expor()` **no lugar** que grava (`para_disco`), e em nenhum outro | não há `serde`; o `para_json` é escrito à mão e a prova `a_senha_da_ligacao_nunca_aparece_no_json` guarda o irmão que **não** pode expor |
| 7 | `Drop` que zera | **sem `Drop`** por ora | a pétrea nomeia saídas, não memória; o ganho medido é 1/5 de 1/16 e custaria o primeiro `unsafe` do core (§5) |
| — | sem `Deref`, sem `Display`, exposição por método nomeado | igual | aqui é **convergência**, não divergência: é a parte que faz o compilador contar |

Sete divergências com restrição nomeada. Passou pela nossa cabeça.

---

## 8. O que NÃO foi medido, e fica dito

- A cascata da migração no `phxsql-server` (58 pontos de produto, 73 de
  teste). Os dois crates pequenos deram 1 e 0 erros em cascata.
- Os `examples/` e `tests/` de integração do server sob (b) (teto por `grep`:
  1 ponto, absorvido).
- A eliminação de escrita morta que justifica o *volatile* (§5): o experimento
  não a reproduz por construção.
- O tamanho do conferidor **reduzido** — não existe; o de 807 linhas existe e
  a redução é da frente dele.
- Os testes do próprio `Segredo` (piso de 43 linhas é sem prova).

---

## 9. Fontes primárias

- `secrecy` 0.10.3: https://github.com/iqlusioninc/crates/blob/main/secrecy/src/lib.rs
- `zeroize` 1.9.0: https://github.com/RustCrypto/utils/blob/master/zeroize/src/lib.rs
- `std::sync::Mutex` `Debug`: https://github.com/rust-lang/rust/blob/master/library/std/src/sync/poison/mutex.rs
- `finish_non_exhaustive`: https://github.com/rust-lang/rust/blob/master/library/core/src/fmt/builders.rs
- `tokio-postgres` `Config` `Debug`: https://github.com/sfackler/rust-postgres/blob/master/tokio-postgres/src/config.rs
- `mysql_async` `MysqlOpts` `Debug`: https://github.com/blackbeam/mysql_async/blob/master/src/opts/mod.rs
- `sqlx` `PgConnectOptions`: https://github.com/launchbadge/sqlx/blob/main/sqlx-postgres/src/options/mod.rs
- `sqlx` `MySqlConnectOptions`: https://github.com/launchbadge/sqlx/blob/main/sqlx-mysql/src/options/mod.rs
- Desta casa: `74de67e`; `docs/SEGURANCA.md` §16; `docs/cognicao/cognicao_guarda-trava-a-struct-nao-a-lei_20260917_0010.md`;
  `docs/cognicao/cognicao_catraca-que-nasce-em-zero-nao-distingue-regua-morta_20260917_0038.md`;
  `docs/propostas/revisao-sec-saidas-de-segredo.md`; `bancada/guardas/debug-com-segredo.py`.
- Experimentos (scratchpad da sessão, não versionados): `medir.sh`, `run0.log`,
  `run1.log`, `run2.log`, `erros.txt`, `classif2.py`, `zera_alloc.rs`,
  `decima.rs`, e a cópia `phx/` com o `phxsql-sql` e o `phxsql-odbc` migrados.

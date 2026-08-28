# MultiLink e o DbLink: por que não dá para ligar, e o que dá

Duas análises, de dois pacotes diferentes. A primeira, sobre
`phoenixmultilink v10.0.0` (release 20260822), que trazia **só binários**. A
segunda, sobre `PHOENIX_FONTES_MULTILINK_V10_S11_RECONCILIADO` (20260808), que
trouxe **os fontes** — e que derruba o motivo da primeira e o troca por outro,
maior e medido. Ver a seção **Segunda análise**.

## O que veio na caixa

Nove arquivos, **todos binários**:

```
binaries/libmultilink.rlib              10,9 MB
binaries/libml_driver_sybase.rlib          303 KB
binaries/libml_driver_as400_drda.rlib       34 KB
binaries/libml_license.rlib                241 KB
binaries/ml_admin                          506 KB
binaries/XlsxView  ·  binaries/XlsxReindex
RELEASE_MANIFEST.md  ·  SHA256SUMS.txt
```

O manifesto diz «contém uma cópia dos fontes Rust», mas **os fontes não estão
no pacote** — só os binários.

## Por que não dá para ligar como está

**1. O `.rlib` foi compilado por outro compilador.** Não é opinião:

```
$ rustc teste.rs --extern multilink=libmultilink.rlib
error[E0514]: found crate `multilink` compiled by an incompatible version of rustc
  = note: crate `multilink` compiled by rustc 1.98.0 (88d9e12ae 2026-08-18)
```

O ambiente aqui é o 1.94.1. O formato do `.rlib` **não é estável entre versões
do compilador** — não é uma questão de versão próxima ou distante: qualquer
diferença recusa. Igualar as duas resolveria hoje e voltaria a quebrar na
próxima atualização de qualquer um dos lados.

**2. Um `.rlib` é dependência externa.** É a regra que sustenta o projeto — a
mesma que fez a compilação cruzada para Windows funcionar de primeira e que
permite `cargo build --offline`. Um `.rlib` de 10,9 MB é uma dependência
binária, que é a forma mais forte: não dá nem para ler o que ela faz.

**3. Não há fachada C.** O `.rlib` não exporta símbolos `extern "C"`, então
nem o caminho de FFI (que contornaria a versão do compilador) está aberto.

**4. Há licenciamento por máquina.** O `ml_admin` revela o modelo:

```
ml_admin hwid
ml_admin generate-keypair
MULTILINK_LICENSE_PRIVATE_KEY_B64=<privada> ml_admin generate <customer> <hwid> <days>
```

Chave privada, identificador de máquina e prazo em dias. Linkar isso dentro do
`phxsqld` faria o **servidor de dados inteiro** passar a depender de uma
licença válida para subir. É uma decisão de produto, não de engenharia.

## O que o MultiLink traria

Vale registrar, porque é o motivo de a pergunta existir. Os nomes que aparecem
no binário:

`sqlite` · `redis` · `mysql` · `postgres` · `sybase` · `mssql` · `duckdb` ·
`mariadb` · `clickhouse` · `as400` · `teradata` · `oracle` · `hfsql` ·
`cassandra`

Contra os dois do DbLink hoje: MySQL(R) escrito à mão, e PostgreSQL(R)
cadastrável sem cliente.

## O caminho que funciona: falar por protocolo

Em vez de **linkar**, **conversar**. É o mesmo desenho que o DbLink já usa
para o MySQL(R) e que o PhxSql usa para tudo: um processo de um lado, um
protocolo no meio.

O MultiLink roda como processo próprio e o PhxSql fala com ele por soquete —
JSON por linha, como a porta 5000, ou o protocolo que ele já falar. Ganha-se:

- **A regra fica de pé.** Nenhuma crate entra no `phxsqld`; o binário continua
  compilando offline e cruzando para Windows.
- **A versão do compilador deixa de importar.** Dois processos não compartilham
  ABI.
- **A licença fica onde ela é.** Se o MultiLink não subir, o DbLink perde
  aqueles motores — o servidor de dados continua de pé.
- **Uma falha lá não derruba o PhxSql.** Um driver de Sybase(R) que entra em
  laço derruba o processo dele, não o banco.

Custa uma ida e volta de rede por consulta, que para uma ligação externa é
ruído perto da latência do banco do outro lado.

O trabalho concreto seria: um motor `multilink` no cadastro do DbLink, com
endereço e porta em vez de credencial direta, e a tradução do resultado dele
para o formato que a grade já consome. As duas telas — cadastro e navegador —
não mudam.

## Segunda análise: os fontes vieram

Em 2026-08-28 chegou o `PHOENIX_FONTES_MULTILINK_V10_S11_RECONCILIADO`, com
**244 arquivos** e 14 manifestos. **O motivo desta página caiu:** os fontes
estão lá — `multilink/src/`, os `ml-driver-*`, o `phoenix_clarion_rw`, o
`phoenix_tps_rw`, o `phoenix_dbf_rw`, e 22 binários de ferramenta.

O motivo novo é outro, e é maior. Medido no `Cargo.lock` que veio junto:

```
$ grep -c '^\[\[package\]\]' mldbx/Cargo.lock
596
$ find mldbx -name Cargo.toml | wc -l
14
```

**596 pacotes menos 14 locais = 582 crates externas.** O PhxSql tem zero.

E não é uma questão de escolher *features*. Com `default = []`, o manifesto do
`multilink` ainda exige cinco dependências obrigatórias:

| dependência | o que arrasta |
|---|---|
| `tokio` (`rt-multi-thread`, `net`, `io-util`, `time`) | um **runtime assíncrono inteiro** |
| `serde` + `serde_json` | derive macros, e com elas `syn`/`quote`/`proc-macro2` |
| `log` | fachada de log |
| `ml-driver-api` | e a árvore dos drivers atrás dela |

Linkar significaria pôr um executor assíncrono dentro de um servidor que hoje é
uma thread por conexão e `std::net`. Não é acrescentar uma biblioteca: é trocar
o modelo de execução do processo.

## O caminho que os fontes abrem, e o preço dele

Os `ml-driver-*-ffi` são `crate-type = ["cdylib", "staticlib"]` com ABI C
limpa — identificadores como `int`, textos por *buffer*:

```rust
pub extern "C" fn dat_open(dat_path: *const c_char, copybook: *const c_char) -> c_int;
pub extern "C" fn dat_query(conn: c_int, sql: *const c_char) -> c_int;
pub extern "C" fn dat_rs_value(rs: c_int, row: c_int, col: c_int,
                               buf: *mut c_char, buf_len: c_int) -> c_int;
```

ABI C **se chama da `std` sem crate nenhuma**: um bloco `extern "C"` declarando
`dlopen`/`dlsym` à mão, e as 582 crates ficam do lado de lá do `.so`. É
tecnicamente viável, e é uma opção real que antes não existia.

O preço é que o `.so` passa a rodar **dentro do processo do banco**: um driver
que trava trava o `phxsqld`, um que estoura derruba o `phxsqld`, e o
licenciamento por máquina com prazo passa a valer para o servidor de dados
subir. Um banco de dados não deve morrer porque um driver de planilha morreu.

## O que continua recomendado

**Falar por protocolo** — e agora com uma forma melhor, que só os fontes
permitem: compilar o MultiLink como **executável separado** (ele já tem 22
binários e o `Cargo.lock` inteiro para isso), com o `phxsqld` falando com ele
pela rede ou por *pipe*.

Assim as 582 crates vivem no processo *dele*, a regra da casa fica de pé, a
versão do compilador deixa de importar, a licença fica onde ela é, e um driver
que entra em laço derruba o processo dele e não o banco.

E há um terceiro caminho, que os fontes tornam barato: **ler o driver e portar
o que se precisa**. Foi assim que o MySQL(R) entrou no DbLink — 700 linhas
escritas aqui, sem crate nenhuma. Com o fonte à mão dá para fazer o mesmo com o
que interessar, e deixar de fora o que não interessa.

---

Sybase, AS/400, Oracle, Teradata, HFSQL, MySQL, MariaDB, PostgreSQL e
ClickHouse são marcas dos seus respectivos donos, citadas aqui por referência
técnica.

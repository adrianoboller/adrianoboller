# MultiLink e o DbLink: por que não dá para ligar, e o que dá

Sobre o pacote `phoenixmultilink v10.0.0` (release 20260822).

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

## E se os fontes vierem

Aí muda tudo. Com o fonte dá para ler o que cada driver faz, portar **o driver
de que se precisa** para dentro do PhxSql seguindo a regra (só `std`), como já
foi feito com o do MySQL(R), e deixar de fora o que não interessa. Foi assim
que o MySQL(R) entrou: 700 linhas escritas aqui, sem crate nenhuma.

---

Sybase, AS/400, Oracle, Teradata, HFSQL, MySQL, MariaDB, PostgreSQL e
ClickHouse são marcas dos seus respectivos donos, citadas aqui por referência
técnica.

# Update the MULTILINK verdict
# 28/08 19:09

import io
p='docs/MULTILINK.md'
s=io.open(p,encoding='utf-8').read()
velho='''## E se os fontes vierem

Aí muda tudo. Com o fonte dá para ler o que cada driver faz, portar **o driver
de que se precisa** para dentro do PhxSql seguindo a regra (só `std`), como já
foi feito com o do MySQL(R), e deixar de fora o que não interessa. Foi assim
que o MySQL(R) entrou: 700 linhas escritas aqui, sem crate nenhuma.

---'''
novo='''## Segunda análise: os fontes vieram

Em 2026-08-28 chegou o `PHOENIX_FONTES_MULTILINK_V10_S11_RECONCILIADO`, com
**244 arquivos** e 14 manifestos. **O motivo desta página caiu:** os fontes
estão lá — `multilink/src/`, os `ml-driver-*`, o `phoenix_clarion_rw`, o
`phoenix_tps_rw`, o `phoenix_dbf_rw`, e 22 binários de ferramenta.

O motivo novo é outro, e é maior. Medido no `Cargo.lock` que veio junto:

```
$ grep -c '^\\[\\[package\\]\\]' mldbx/Cargo.lock
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

---'''
assert velho in s
s=s.replace(velho,novo,1)
s=s.replace('''# MultiLink e o DbLink: por que não dá para ligar, e o que dá

Sobre o pacote `phoenixmultilink v10.0.0` (release 20260822).''',
'''# MultiLink e o DbLink: por que não dá para ligar, e o que dá

Duas análises, de dois pacotes diferentes. A primeira, sobre
`phoenixmultilink v10.0.0` (release 20260822), que trazia **só binários**. A
segunda, sobre `PHOENIX_FONTES_MULTILINK_V10_S11_RECONCILIADO` (20260808), que
trouxe **os fontes** — e que derruba o motivo da primeira e o troca por outro,
maior e medido. Ver a seção **Segunda análise**.''',1)
io.open(p,'w',encoding='utf-8').write(s)

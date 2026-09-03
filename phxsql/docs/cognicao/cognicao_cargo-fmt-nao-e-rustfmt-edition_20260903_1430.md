# `cargo fmt` não é `rustfmt --edition`, e a diferença passa despercebida

**03/09/2026, 14:30** — descoberto conferindo o portão de formatação antes de
fechar a rodada.

## 1. O que aconteceu

Outra frente trabalhava no mesmo diretório com arquivos não commitados. Para não
passar por cima do trabalho dela, evitei `cargo fmt --all` e chamei o `rustfmt`
direto nos meus arquivos:

```bash
rustfmt --edition 2024 crates/phxsql-store/src/table.rs ...
```

A intenção estava certa. O comando estava errado: **`rustfmt` invocado direto
não recebe o `--style-edition` que o `cargo fmt` deriva do `edition` do
workspace.** Ele reordenou os blocos `use` pela ordem ASCII — maiúscula antes de
minúscula — enquanto o repositório usa a ordem que o `cargo fmt` produz:

```rust
- use phxsql_core::value::{escrever_inline, ler_inline, Ponteiro, Value};   // o repositorio
+ use phxsql_core::value::{Ponteiro, Value, escrever_inline, ler_inline};   // o que eu commitei
```

Isso ficou **commitado**, e o portão `cargo fmt --all -- --check` passou a
reprovar numa árvore que estava verde quando eu cheguei.

## 2. O que eu concluí primeiro, e estava errado

Ao ver o portão reprovar, concluí que **outra frente** tinha mudado a
configuração de formatação, ou que o toolchain tinha sido atualizado embaixo de
mim — o `rust-toolchain.toml` fala exatamente desse risco, e eu li o arquivo
procurando confirmação.

Errado, e a confirmação veio pelo caminho oposto: peguei a versão de `table.rs`
do commit **anterior à minha sessão** e rodei o portão sobre ela. Passou limpa.
Não era o ambiente. Era eu, e a distância entre `rustfmt` e `cargo fmt`.

Foi a mesma armadilha do «diagnóstico plausível»: culpar o ambiente é sempre
plausível quando o ambiente é compartilhado, e é justamente aí que ele mais
engana.

## 3. O que a medição disse

`cargo fmt --all -- --check` acusou **2** diferenças, e as duas no mesmo
arquivo: `table.rs`, linhas 18 e 31.

Esse número é o achado, e não o incidente: **`table.rs` é o único arquivo do
workspace cujos blocos `use` misturam nome de tipo e nome de função no mesmo
`{}`** — que é a única situação em que as duas ordens diferem. Chamei o
`rustfmt` errado em **sete** arquivos; em seis a troca não produziu diferença
nenhuma.

Ou seja: com 6/7 de chance o erro não teria aparecido, e a árvore ficaria
divergente do portão sem ninguém saber desde quando.

## 4. A regra

**Para formatar sem pisar em arquivo alheio, use `cargo fmt -p <crate>`, nunca
`rustfmt` direto.** O `-p` limita o alcance *e* preserva a configuração; o
`rustfmt` cru limita o alcance e joga a configuração fora.

E o corolário: **o portão que só falha num arquivo do repositório inteiro é o
mais perigoso, não o mais inofensivo** — porque a mesma causa passou em silêncio
por todos os outros.

## 5. Como está guardado hoje

* Consertado no commit «`cargo fmt` não é `rustfmt --edition`», com o portão de
  volta a zero.
* **Onde o buraco ficou:** não há nada que impeça a próxima sessão de repetir
  isto — o `CLAUDE.md` manda rodar `cargo fmt --all` antes de commitar, e é
  justamente essa ordem que uma frente evita quando divide a árvore com outra. O
  `-p` é a resposta, e ela não está escrita em lugar nenhum além deste arquivo.

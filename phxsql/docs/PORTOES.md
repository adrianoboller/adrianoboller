# Os tres portoes: o que cada um pega, e como rodar

Este documento descreve os tres portoes obrigatorios antes de qualquer commit
(ja listados na secao "Antes de commitar" do `CLAUDE.md` da raiz) e como o CI
os aplica sozinho em todo `push` e toda `pull request`, em
`.github/workflows/portoes.yml`.

Um portao so serve se ninguem conseguir passar por ele sem querer. Os tres
aqui rodam **sem cano nenhum** na saida — nada de `| tail`, `| grep -v ...`
ou coisa parecida. Um cano troca o codigo de saida do comando pelo do ultimo
elo do cano: o `cargo test` pode reprovar e o `tail` que veio depois devolver
sucesso do mesmo jeito, e o CI marca o commit como verde com um teste quebrado
dentro dele. Ja foi defeito real aqui — por isso o workflow chama cada
comando puro e deixa o proprio codigo de saida decidir se o job passa.

## Portao 1 — formatacao

```bash
cargo fmt --all --check
```

Confere que todo arquivo `.rs` do workspace esta formatado do jeito que o
`rustfmt` formataria. **Nao formata nada** — o `--check` faz ele so apontar a
diferenca e sair com codigo diferente de zero se houver uma. O que ele pega:
diffs de revisao poluidos por reformatacao misturada com mudanca de logica, e
o "no meu editor fica bonito" que depende de configuracao pessoal.

Para corrigir localmente (sem o `--check`, que reescreve os arquivos):

```bash
cargo fmt --all
```

## Portao 2 — lints, zero avisos

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

`--workspace` cobre as oito crates do `Cargo.toml` raiz; `--all-targets`
inclui testes, exemplos e benchmarks, nao so a biblioteca. `-D warnings`
transforma todo aviso do clippy em erro — sem isso o clippy so imprime e sai
com sucesso, e um aviso ignorado por meses e um aviso que ninguem le mais.

O que ele pega que o compilador sozinho nao pega: clone desnecessario,
comparacao que sempre da o mesmo resultado, `unwrap` onde ha um jeito de
propagar o erro, import que sobrou. Nenhum desses impede a compilacao — todos
custam alguma coisa em tempo de execucao ou em legibilidade.

## Portao 3 — a suite inteira

```bash
cargo test --workspace
```

Roda todo teste de toda crate do workspace, unitario e de integracao. E o
portao mais lento e o unico que exercita comportamento, nao so forma do
codigo — os outros dois nao pegam uma regra de negocio quebrada.

Vale o que o `CLAUDE.md` ja registra sobre este portao: ele prova o que roda
**dentro do processo**. Comportamento que depende do sistema operacional —
queda de conexao, por exemplo — se prova contra o sistema operacional (um
soquete de verdade), porque ha coisa que um teste unitario nao enxerga por
construcao.

## Rodando os tres de uma vez, como o CI roda

```bash
cd phxsql
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Cada comando roda isolado. Se um deles reprovar, pare ali e conserte antes de
seguir para o proximo — e nunca encadeie os tres com `&&` escondendo qual foi
o que falhou, nem redirecione a saida para um arquivo e leia so o fim dele.

## O toolchain e pinado, nao "o que estiver instalado"

`phxsql/rust-toolchain.toml` fixa a versao exata do compilador (e os
componentes `rustfmt` e `clippy` que os portoes 1 e 2 exigem). O `rustup`
le esse arquivo sozinho — tanto na sua maquina quanto no runner do GitHub — e
troca para a versao pinada antes do primeiro `cargo`/`rustc`. Sem ele, os
portoes rodam com o toolchain "default" de quem digitou o comando, que muda
sozinho a cada `rustup update` e pode ser uma versao diferente da do CI: um
clippy mais novo acrescenta lints, um rustfmt mais novo reformata diferente,
e o mesmo commit passa numa maquina e falha noutra sem nenhuma linha de
codigo ter mudado.

## O que o CI roda, e onde

`.github/workflows/portoes.yml`, na raiz do repositorio, dispara em todo
`push` e toda `pull request` e roda os tres portoes nessa ordem, num runner
`ubuntu-latest` que ja traz `rustup` — nenhuma acao de terceiro entra so para
instalar o compilador, e nenhuma crate externa entra no workspace por causa
do CI. O job falha no primeiro portao que reprovar; os passos seguintes nem
rodam, entao o log sempre aponta o portao certo sem precisar catar erro no
meio da saida inteira.

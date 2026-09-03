# Cognição: provar "zero dependências externas" não se faz mutando manifesto

## 1. O que aconteceu

O QA-PDCA apontou que a pétrea mais repetida do `CLAUDE.md` — "zero
dependências externas, só a `std`" — não tinha guarda nenhuma: nada além de
`cargo build --offline` (que recusa por acidente de falta de cache, não por
regra) impedia um `serde = "1.0"` de entrar num `Cargo.toml`. A tarefa era
escrever a guarda e prová-la nos dois sentidos pelo mecanismo que a casa já
tem para isso, `bancada/guardas/` (mutar um trecho do fonte, conferir que o
teste nomeado cai).

Tentei três formas de "repor o defeito" contra os manifestos de verdade, e as
três falharam por motivos diferentes — não por bug meu, por como o `cargo`
funciona:

1. **Acrescentar um pacote fantasma ao `Cargo.lock`** (`[[package]] name =
   "serde" version = "1.0.219" source = "registry+..."`, sem nenhuma
   dependência real apontando para ele) e rodar `cargo test --offline`.
   Resultado: o `cargo` **reescreveu o `Cargo.lock` sozinho** e podou a
   entrada antes de qualquer teste rodar — o arquivo, medido depois, voltou
   a ficar idêntico ao original. Nenhum teste Rust chegou a ver o defeito.

2. **Acrescentar `serde = "1.0"` de verdade a um `Cargo.toml`** e rodar
   `cargo test --offline`. Resultado: `error: no matching package named
   \`serde\` found` — a resolução falha **antes de compilar qualquer coisa**,
   e nenhum binário de teste chega a existir para reprovar nada. Isto já era
   esperado (é a proteção "por acidente" que o achado descreve), mas prova
   que este caminho também não serve para o mecanismo de mutação: o defeito
   quebra o `cargo test` inteiro, não um teste nomeado.

3. Por curiosidade, testei se a mesma dependência resolveria **sem**
   `--offline`: resolveu — o `cargo` desta máquina tem acesso de rede ao
   `crates.io` pelo proxy, e o `Cargo.lock` ganhou `serde 1.0.229` e mais
   cinco pacotes de verdade, com checksum e tudo. A "proteção por acidente"
   é ainda mais frágil do que o achado dizia: não depende só do cache local
   estar vazio, depende de **não haver rede** — e nesta máquina há.

## 2. O que eu concluí primeiro, e estava errado

Concluí primeiro que bastava copiar o padrão dos outros conferidores
(`conferidor.rs`, `conferidor_grades.rs`): escrever a lógica de detecção,
mutar o Cargo.lock/Cargo.toml de verdade pelo mecanismo de trecho/troca do
catálogo, e pronto — seria só mais um `#[test]` no molde dos outros 57.
Não é: os outros mutam **código Rust que o cargo já sabe compilar de
qualquer jeito**, mesmo com o defeito reposto. Uma dependência externa é
diferente por natureza — ela muda o que o **cargo precisa resolver antes de
compilar qualquer coisa**, e essa resolução acontece num estágio que o
mecanismo de mutação (que só sabe rodar `cargo test` no binário nomeado)
nunca alcança de um jeito que produza um veredito de teste normal.

## 3. O que a medição disse

- Um pacote no `Cargo.lock` sem nenhum `Cargo.toml` apontando para ele **não
  sobrevive** a um `cargo test` — é podado antes da compilação. Confirmado
  lendo o arquivo depois: idêntico ao original, byte a byte.
- Uma dependência real e não resolvível offline **quebra a resolução do
  workspace inteiro**, não só do pacote que a declara — `cargo test -p
  phxsql-core --lib` falhou por causa de uma linha em `phxsql-core/Cargo.toml`
  mesmo pedindo só aquele pacote, porque o `cargo` resolve o grafo do
  workspace inteiro antes de escolher o que compilar.
- Uma dependência de **caminho para FORA do workspace** (não um pacote de
  registro) resolve **offline, sem erro nenhum** — testado com um
  `pacote-de-fora-do-workspace` de mentira, referenciado por `path = "../..."`
  a partir de `phxsql-core`: `cargo build --offline` compilou os dois sem
  reclamar. E esse pacote **não leva o campo `source`** no `Cargo.lock` —
  os oito membros do próprio workspace, que são path deps entre si, já
  provam isso lendo o `Cargo.lock` real (nenhum tem `source`). Um conferidor
  que procurasse `source = "registry+..."` teria deixado passar exatamente
  este caso.

## 4. A regra

**Uma guarda que depende de metadados de dependência (Cargo.lock/Cargo.toml)
não se prova mutando o manifesto de verdade** — prova-se com uma função pura
de detecção, testada contra um fixture embutido no próprio teste, e
**separadamente** contra o `Cargo.lock` real (a guarda que protege o
repositório todo dia, sem entrar no catálogo de mutação porque o defeito que
ela previne não sobrevive ao próprio `cargo test`). E o filtro de detecção
compara **nomes contra o que o workspace DECLARA** (`[workspace] members`),
nunca a presença do campo `source` — que uma dependência de caminho de fora
também não leva.

## 5. Como está guardado hoje

`crates/phxsql-server/src/conferidor_dependencias.rs`: duas camadas, como a
seção 3 descreve. A camada "real" é
`conferidor_dependencias::testes::workspace_zero_dependencia_externa`, que
lê o `Cargo.lock` e o `Cargo.toml` de verdade — é ela que reprovaria um
`serde` de amanhã, dentro do `cargo test --workspace` normal, sem precisar
do catálogo de mutação. A camada "lógica",
`deteta_pacote_de_fora_do_workspace`, usa um `Cargo.lock` de mentira embutido
no teste e é a que entra em `bancada/guardas/catalogo.py`
(`dependencia-de-fora-fica-invisivel`) — provada com
`python3 bancada/guardas/provar-guardas.py --so dependencia-de-fora-fica-invisivel`.

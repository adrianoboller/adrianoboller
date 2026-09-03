# Módulo filho enxerga o privado do pai — e isso mudou o custo da SP000005 de dez rodadas para uma

**Descoberto:** 03/09/2026, 16:20.
**Onde:** `crates/phxsql-server/src/servidor.rs` (23.171 linhas);
medição em `docs/fronteiras/mapa-do-servidor.py`, resultado em
`docs/FRONTEIRAS-DO-SERVIDOR.md`.

## 1. O que aconteceu

O primeiro passo da SP000005 é medir onde o `servidor.rs` se divide. Medi o
grafo de chamadas por região e a conta apareceu feia: a região
`portoes-e-despacho` chama **127** métodos distintos do `Servidor`, e
`operacoes-de-dados` é chamada por **110**. Como `travar_dados`, `TravaMedida`
e `Sessao` são todos privados, a leitura óbvia era que a sprint custaria
dezenas de itens virando `pub(crate)` — cada um deles uma garantia que passa a
depender de disciplina em vez de compilador.

## 2. O que eu concluí primeiro, e estava errado

Que essas 127 travessias eram **o custo da sprint**, e que o plano teria de
começar por abrir a visibilidade do que hoje é privado.

Estava errado por uma suposição que nunca virou pergunta: eu assumi que
«dividir o arquivo» significava criar módulos **irmãos** — que é o único
layout que este crate usa hoje (`cluster.rs`, `transacao.rs`, `travas.rs` são
todos irmãos de `servidor.rs`). Com irmãos a conta está certa. Só que a
divisão não precisa ser em irmãos.

Em Rust, item privado é visível no módulo que o declara **e em todos os
descendentes dele**. Um módulo **filho** lê campo privado, chama método privado
e nomeia tipo privado do pai sem um `pub` sequer.

## 3. O que a medição disse

Provado com `rustc` nos dois sentidos, em arquivos de rascunho — não por
leitura de documentação:

| layout | o filho/irmão lê campo privado, chama método privado, nomeia tipo privado |
|---|---|
| **irmão** (`pub mod servidor;` + `pub mod cluster;`) | `error[E0616]` campo privado + `error[E0624]` método privado — **2 erros** |
| **filho** (`servidor.rs` + `mod cluster;` em `servidor/cluster.rs`) | **0 erros** |

E o layout de filho não exige nem renomear o arquivo: na edição 2021 — a do
workspace — `servidor.rs` continua se chamando `servidor.rs` e os filhos moram
em `servidor/`. Também medido: **0 erros**. Não há renomeação, então o
histórico das linhas que ficam sobrevive intacto.

**As 127 travessias custam zero na divisão em filhos.** O número está certo; o
que estava errado era o layout que eu supus para ele.

O mesmo vale para os testes, e é o que mais se ganha: **17 dos 18** módulos
`#[cfg(test)]` de dentro do arquivo constroem `Sessao` (tipo privado) e chamam
`despachar` ou `executar` (métodos privados). Como filhos eles continuam
compilando; movidos para `tests/`, **nenhum** compila.

## 4. A regra

**Antes de medir o custo de dividir um módulo, decida se a divisão é em irmãos
ou em filhos — a mesma medida dá ordens de grandeza diferentes nos dois.** Em
Rust, filho enxerga o privado do pai; irmão não.

## 5. Como está guardado hoje

Na seção 3.1 do `docs/FRONTEIRAS-DO-SERVIDOR.md`, com os dois trechos de
`rustc` e as duas saídas, e na seção 4 como a ordem proposta.

**Onde o buraco ficou:** as duas provas foram rodadas em arquivos de rascunho
e **não estão no repositório** — não há teste que impeça alguém de começar a
SP000005 pelo layout irmão e descobrir os dois erros só na hora. Quem abrir a
sprint faz o primeiro filho e roda o `cargo` antes de mover a segunda região;
o erro é barato (não compila), mas ele é a única guarda que existe hoje.

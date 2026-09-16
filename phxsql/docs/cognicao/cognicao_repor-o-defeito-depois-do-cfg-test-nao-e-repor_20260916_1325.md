# Cognição: a prova real passou verde com o defeito na árvore — eu tinha reposto o defeito em território de teste

- **Assunto:** prova real de uma catraca que lê o fonte Rust
- **Descoberta:** 16/09/2026, 13:25 UTC
- **Arquivos:** `crates/phxsql-server/tests/catraca-do-mapa-das-threads.rs`,
  `bancada/concorrencia/mapa-das-threads.py`, `crates/phxsql-core/src/paralelo.rs`

## 1. O que aconteceu

Teste novo escrito, verde. Para a prova real, repus o defeito que ele existe
para pegar — um `std::thread::spawn` de produção sem entrada no catálogo de
threads — acrescentando a função no **fim** do `crates/phxsql-core/src/paralelo.rs`
e rodando o binário do teste já compilado. **Passou.** Teste verde com o
defeito na árvore é o pior estado possível, e é pior que teste faltando.

## 2. O que eu concluí primeiro, e estava errado

Que o teste tinha um furo — que o `Command` não estava vendo a árvore certa,
ou que o `cwd` mentia. Nada disso. O medidor estava **certo**: ele ignora tudo
a partir do `#[cfg(test)] mod`, e o `paralelo.rs` abre o módulo de teste na
**linha 118** de 199. Acrescentar no fim do arquivo é acrescentar dentro do
módulo de teste — thread de teste não tem teto porque não é do servidor. Quem
estava errado era a minha reposição, não a régua.

## 3. O que a medição disse

- `grep -n "cfg(test)" crates/phxsql-core/src/paralelo.rs` → **118**; o arquivo
  tem **199** linhas. O `spawn` acrescentado no fim caiu na linha ~200.
- Reposto **acima** da 118, o teste reprova e **nomeia**: `SUBIU
  spawn-sem-teto 1 (teto 0)` e `SEM TETO crates/phxsql-core/src/paralelo.rs:120
  thread::spawn`.
- O segundo sentido também: entrada de catálogo cuja agulha não casa com sítio
  nenhum dá `SUBIU catalogo-envelhecido 1 (teto 0)`, nomeando o arquivo.

## 4. A regra

**Repor defeito em arquivo Rust exige saber onde a produção ACABA, e não só
qual arquivo é: o fim do arquivo quase sempre é território de `#[cfg(test)]`,
e defeito reposto ali não é defeito reposto.**

## 5. Como está guardado hoje

As duas provas, com o número e a linha, estão na §13 do `docs/CATRACAS.md`,
inclusive a tentativa que passou — porque a tentativa que passou é o que
ensina, e omiti-la deixaria a §13 parecendo fácil.

# Cognição: o peso de uma dependência é a árvore TRANSITIVA, não as deps diretas

**Descoberta:** 11/09/2026 ~18:50 UTC.

## 1. O que aconteceu

O dono pediu para provar que "tudo se encaixa" no correio, **sem dependências**.
Estendi o protótipo `crates/phxsql-core/examples/correio-e2e.rs` para o fluxo que
ele descreveu — (A) aceitar a confiança cobrando um pix de valor sem teto, ou de
graça; (B) ligar "seguro alto" = p12 + cada um a sua senha (E2E); (C) Masson cuja
chave da 3a camada é o **id da maçonaria** — e a prova saiu **33/33 VERDE**, tudo
zero-deps.

No caminho houve um desvio de rumo do dono: ele mandou **"usar o psig integrado
como crate"** e, na mensagem seguinte, **"criptografia nativa sem OpenSSL e
diversas dependências, 100% sem dependências"**. As duas não cabem juntas.

## 2. O que eu concluí primeiro, e estava errado

Ao ver o `Cargo.toml` do psig, contei as **dependências diretas** e disse ao dono
**"~50 crates"**. Foi o número que ele levou para decidir "crate dentro".

Estava errado por baixo: deps diretas **escondem a árvore transitiva**. Cada uma
puxa as suas, e o que entra de verdade no `cargo build` é o fecho transitivo,
não a lista do `[dependencies]`.

## 3. O que a medição disse

Compilei o psig sozinho (`--no-default-features`, features de rede desligadas):
**189 crates** compiladas, `target` de **785 MB**, binário de 78 MB, 36 s de
build. O número real era **3,8× maior** que a minha estimativa das deps diretas.
E a rede DESTE sandbox alcança o crates.io (o `cfg-if` baixou), então o bloqueio
que eu temia não existia — a feasibility era real, só o **custo** é que era o
dobro do dito.

Com o número na mesa, o dono reafirmou o norte — **sem dependências, nativo** —,
o que reverteu o "crate dentro": o correio fica no `.p12` **escrito à mão**
(a fatia 1, já provada contra o OpenSSL), e o psig vira **referência/ferramenta
externa opcional**, não crate.

## 4. A regra

**O peso de uma dependência é a árvore TRANSITIVA, não as deps diretas — meça o
fecho (`cargo tree`/o build inteiro) e reporte ESSE número antes de aceitar um
"use X". E quando as ordens do dono conflitam, o valor repetido e enfático
(aqui, zero-deps) é o norte: o choque APARECE na mesa, não se escolhe calado.**

## 5. Como está guardado hoje

Protótipo em `correio-e2e.rs`, seção 10, **33/33 VERDE**, só `std` + a cripto da
casa. O `.p12` nativo (fatia 1) é a base; o psig fica de fora do `Cargo.toml`.
Buraco que segue aberto: o formato em disco das tabelas PSCH do correio
(`docs/CORREIO-FORMATO.md`, as cinco decisões) e a fatia 2 do `.p12` (o embrulho
PKCS#12 da privada), ambos ainda esperando o dono.

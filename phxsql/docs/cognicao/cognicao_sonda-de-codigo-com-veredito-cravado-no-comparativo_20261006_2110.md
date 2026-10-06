# A sonda de código do comparativo tinha o veredito escrito no código, e continuou dizendo «sem TLS» depois de o TLS existir

**Estado:** PENDENTE

## 1. O que aconteceu

No fecho da rodada de 01–02/10/2026 (descoberto em 06/10), ao corrigir a prosa que dizia «o TLS não
existe» (`README.md`, `MANUAL.txt`, `CONTRATO-1.0.md`), a catraca
`docs/dossie/catraca-prosa-x-celula.py` **reprovou a frase certa**: a célula
`tls_no_transporte` do comparativo (`bancada/comparativo/resultados.json`,
medida em 2026-09-17) dizia `nao`, e o léxico de `contrato-das-capacidades.json`
manda reprovar quem afirma que o motor fala TLS. O TLS 1.3 de servidor existe
desde o pedido 572 (`crates/phxsql-core/src/tls.rs`, `config.rs::TlsPorta`).

A causa estava em `bancada/comparativo/medir.py`: a sonda de código
`fora["tls_no_transporte"]` chamava `citar(...)` só para **escrever a
evidência**, e o veredito era a constante `NAO` — não dependia do que o `citar`
achasse. Uma sonda de código cujo resultado não depende do código é um número
digitado com cara de medida.

## 2. O que eu concluí primeiro, e estava errado

Que a catraca estava com o léxico largo demais e que bastava ajustar a frase do
README para não casar. O ajuste passou — e deixaria a célula errada publicada no
dossiê (§33) e no `COMPARATIVO.md` por mais um ciclo, com a catraca verde
**protegendo a célula velha**. A catraca estava certa em reprovar; o que ela
comparava é que estava velho.

## 3. O que a medição disse

`bancada/comparativo/medir.py`: a sonda do `trava_por_linha` (a vizinha de
cima) usa o molde certo — três `tem(...)` (existe, ligado, usado) e o veredito
sai de `TEM if (a and b and c) else NAO`. A do TLS não: `"phxsql": (NAO, ond)`.
Reescrita no molde da vizinha neste commit (existe em `tls.rs`, ligado em
`servidor.rs` por `phxsql_core::tls::aceitar`, pedido pela config
`TlsPorta`). **O `resultados.json` NÃO foi remedido** — exige rodar a bancada do
comparativo —, então a célula publicada continua `nao`, datada de 17/09, até a
próxima corrida.

## 4. A regra

Sonda de código tem de decidir pelo que acha, nunca por constante; e quando a
catraca de prosa reprova uma frase verdadeira, **a primeira hipótese é a célula
velha**, não a frase.

## 5. Como está guardado hoje

Corrigida a sonda; **buraco que fica:** (a) a célula `tls_no_transporte` só
vira `TEM` quando alguém rodar `bancada/comparativo/medir.py`; (b) nada impede
outra sonda de código com `(NAO, ...)` ou `(TEM, ...)` cravado — há pelo menos
`medir.py:1268`/`:1290`, que são tabelas dos OUTROS motores (citadas, não
medidas), e as demais sondas de `fora` não foram revisadas uma a uma. Pedido
de varredura a abrir por quem mantém `PENDENCIAS.md`.

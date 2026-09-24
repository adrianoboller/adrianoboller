# Limite exclusivo contra relógio de milissegundo: o teste que usa «agora» como limite floca

**Estado:** PENDENTE

*24/09/2026, 12:17 — pedido 368, provas do expurgo da trilha.*

## 1. O que aconteceu

O expurgo derruba o volume cujo registro mais novo é **anterior** ao limite
(`carimbo < limite`, exclusivo, de propósito: o que é do limite em diante
fica). Dois testes do servidor e a prova contra o sistema operacional
(`bancada/lgpd/prova-do-expurgo.py`) mandavam `ate_ms = agora` logo depois de
gravar. Numa corrida da suíte inteira do servidor, **1.411 passaram e 1 caiu**:
`o_expurgo_pede_administrar_e_roda_no_servidor_somente_leitura`, com o volume
que devia sair ainda no disco.

## 2. O que eu concluí primeiro, e estava errado

O primeiro diagnóstico não ficou registrado (o raciocínio daquele momento se
perdeu numa compactação da sessão) — e isso já é parte do aprendizado: a
cognição tem de nascer na hora. O suspeito que esta casa manda olhar primeiro
quando um teste cai só na suíte inteira é **estado global entre vizinhos**
(`cognicao_estado-global-entre-testes-do-mesmo-binario_20260909_0610.md`), e o
corte da trilha **é** global de processo. Conferido depois, não era: no
binário do servidor só o `Config::ler` aplica o corte, sempre com o padrão, e o
teste do config que usa valores fora do padrão passa pelo `de_json`, que não
aplica.

## 3. O que a medição disse

O último registro gravado e o `agora` do teste podem cair no **mesmo
milissegundo** — e igual não é anterior. Com `agora + 1` nos testes e no
script, a condição do dado fica a mesma (o registro gravado **antes** da
chamada sai) e a queda não voltou nas corridas seguintes da suíte. A
frequência não foi medida em laço: é uma queda observada, não uma taxa.

## 4. A regra

Quando a regra é exclusiva no tempo e o teste usa «agora» como limite, some
uma unidade do relógio ao limite — senão o teste mede a resolução do relógio,
e não a regra.

## 5. Como está guardado hoje

`crates/phxsql-server/src/servidor.rs::o_administrador_expurga_so_volume_fechado_e_o_rastro_fica`,
`crates/phxsql-server/src/servidor.rs::o_expurgo_pede_administrar_e_roda_no_servidor_somente_leitura`
e o `bancada/lgpd/prova-do-expurgo.py`, cada um com o `+ 1` e o porquê
comentado. A regra do motor não mudou: o limite continua exclusivo.

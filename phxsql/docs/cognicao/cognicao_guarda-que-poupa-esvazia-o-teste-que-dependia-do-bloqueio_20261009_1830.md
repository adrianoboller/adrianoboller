# Guarda que poupa um caminho esvazia o teste que dependia dele

**Estado:** PENDENTE

## O que aconteceu

Pedido 766, P9: o loopback passou a ser poupado do bloqueio
(`Politica::poupar_loopback`, nasce `true`). A suíte do `phxsql-server` rodou
inteira com a guarda ligada: **1 teste de integração caiu** (`tests/servico.rs`,
o que espera o bloqueio pela porta) e **1 ficou verde por engano** —
`tests/firewall-que-pendura.rs::firewall_que_pendura_nao_para_o_servidor`.

Ele prova que um comando de firewall pendurado (`/bin/sleep 60`) não para o
servidor: três tokens errados de `127.0.0.1`, o terceiro bloqueia e dispara o
firewall, e outro cliente tem de ser atendido em 2 s. Com o loopback poupado, o
terceiro token **não bloqueia**, o firewall **nunca roda**, e «o outro cliente
foi atendido» continua verdade — por outro motivo.

## O que eu concluí primeiro, e estava errado

Que a lista dos testes a ajustar era a dos que **caíram**. Ajustaria o
`servico.rs` e seguiria. O verde do `firewall-que-pendura` parecia confirmação
de que ele não dependia do bloqueio do loopback.

## O que a medição disse

O teste verde não conferia a própria premissa: nada nele perguntava se o IP
foi bloqueado. Com a guarda ligada e sem o escape, ele passa; com o escape
(`poupar_loopback = false`) e a conferência nova (o `127.0.0.1` tem de estar
no `blacklist.json` ao fim), ele mede de novo. Tirar o escape agora derruba o
teste na linha da premissa — a guarda `fw-pendura-sem-premissa` do catálogo.

## A regra

Quando uma guarda nova **poupa** um caminho, procure os testes que **dependiam
de passar por ele** — inclusive os que ficaram verdes —, e faça cada um conferir
a premissa (o bloqueio aconteceu, o comando rodou), não só o desfecho.

## Como está guardado hoje

- `tests/firewall-que-pendura.rs` confere o bloqueio ao fim; `tests/servico.rs`
  e ele carregam o escape escrito `poupar_loopback = false`.
- O catálogo de guardas tem a entrada que tira o escape e exige a queda.
- **O buraco:** não há conferidor que ache, sozinho, teste cuja premissa uma
  guarda nova tornou falsa. A procura foi manual (`grep` de `127.0.0.1` ×
  `bloque`/`firewall` em `tests/` e `src/servidor/testes_*`, 6 + 6 arquivos lidos).

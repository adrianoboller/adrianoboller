# Documentação de protocolo que mente é pior que documentação que falta

- **Quando:** 2026-09-02, 19:35 (integração)
- **Onde:** `crates/phxsql-server/src/catalogo.rs`, verbete `declarar_fk`
- **Custo:** o catálogo ensinava a violar uma regra pétrea

## O que aconteceu

O catálogo de operações — que é o que a tela, o MCP e o explorador de API leem
para descrever o protocolo — dizia que `ao_excluir` aceita *«restringir,
cascata, anular ou nada»*.

A pétrea diz o contrário, com palavra do dono: *«1 para muitos.
Cascade/Restrict sempre. Nunca pode matar o registro pai se tem filhos.»* Em
código, `ao_excluir` aceita **só** `restringir`, e recusa na declaração.

Quem lesse o catálogo escreveria `"ao_excluir": "cascata"` e levaria uma
recusa que a documentação dizia não existir.

## O que eu concluí primeiro, e estava errado

Nada — este eu não errei antes, mas **também não achei**: a mentira estava lá
desde que a pétrea entrou, e só apareceu porque uma frente que mexia noutro
campo do mesmo verbete leu a linha de cima.

## O que a medição disse

Duas ocorrências, `catalogo.rs:829` e `:872`, ambas afirmando o conjunto de
quatro ações para os **dois** lados.

## A regra

**Quando uma regra passa a recusar algo, procure quem DOCUMENTA aquilo.** A
recusa entra no código e a descrição fica — e a descrição é o que a pessoa lê
primeiro. Falta de documentação faz alguém perguntar; documentação errada faz
alguém confiar.

## Como está guardado hoje

Corrigido: `ao_excluir` diz «SÓ restringir» com o motivo, e `ao_alterar`
descreve as quatro ações e desde quando elas acontecem na gravação.

**O buraco que ficou:** nada liga o catálogo ao que o código aceita. Um teste
que exercitasse cada valor descrito e conferisse se ele é mesmo aceito pegaria
esta classe inteira — e não existe.

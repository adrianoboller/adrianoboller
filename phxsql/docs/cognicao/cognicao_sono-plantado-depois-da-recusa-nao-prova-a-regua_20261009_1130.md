# Sono plantado depois da recusa não prova a régua do relógio

**Estado:** PENDENTE

## O que aconteceu

Pedido 760: `identidade-do-pulso::o_pulso_nao_diz_quais_nos_tem_pino` caiu
sob carga. Reproduzido com 4 `yes` mortos pelo PID: **1 de 50** corridas do
binário, `vies de 13 em 40 (19 diferentes)`. Duas hipóteses escritas antes:
(a) o servidor vaza tempo; (b) a régua de 40 pares com teto fixo não aguenta
ruído. **As duas se sustentaram, em escalas diferentes.**

## O que eu concluí primeiro, e estava errado

1. Que era só (b). Medido em processo, 2.000 pares intercalados, o nó COM
   pino saía ~6 µs mais lento na mediana (1.100 contra 900 e 1.089 contra
   911 pares): o `NoCluster::pino_do_fio` voltava cedo sem pino e, com pino,
   pagava dois `format!` e a leitura de 64 hex. Pequeno demais para derrubar
   o teste (z≈0,7 no `ms` com 200 pares), mas vazamento de verdade.
2. Que o RED com `sleep` podia ir em qualquer lugar do caminho sem pino. O
   primeiro, posto no `if pino.is_none()` **depois** do `pulso::conferir`,
   passou verde com 1 ms de sono: a prova forjada já é recusada no
   `conferir`, e o sono nunca é alcançado. RED que não roda é o teste que
   passa por engano.

## O que a medição disse

- Depois do conserto (o nó sem pino decodifica o hex do `x25519::BASE` e o
  descarta): 1.013/987, 1.029/971 e 996/1.004 pares; medianas a 0,5–2,4 µs.
- Régua nova, sob carga, sem defeito: z máximo visto 2,1 em 6 medidas.
- Régua nova, sob carga, sono **antes** do X25519: 100 µs → 5/5 vermelhos
  (vies 59–119 de 200); 1 ms → 5/5 (vies 152–178).
- 50 de 50 verdes sob carga com a régua nova.

## A regra

Planta o RED **no trecho que o caminho medido de fato percorre**, antes do
primeiro passo que recusa — e confere que ele derruba o teste antes de
confiar que a régua pega.

## Como está guardado hoje

- `regua_do_relogio` em `crates/phxsql-server/tests/identidade-do-pulso.rs`:
  200 pares de ordem intercalada, teste do sinal no `ms` e na parede do
  cliente, teto de 1/4 dos pares e 5 desvios.
- Guarda `relogio-do-pulso-com-sono-plantado` no catálogo, PROVADA 1/1.
- **Buraco:** o conserto dos ~6 µs não tem guarda automática. Uma prova em
  processo precisa de ~2.000 pares para separar 6 µs (z≈4,5) e flocaria sob
  carga; a evidência dele é a medição acima, não um teste.

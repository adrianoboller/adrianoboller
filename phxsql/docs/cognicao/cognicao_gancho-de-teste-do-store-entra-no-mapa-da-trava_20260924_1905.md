# O gancho de teste do store entra no mapa da trava como código de produção

**Estado:** PENDENTE

## O que aconteceu

Para provar o pedido 540 contra o SO (`SIGKILL` no meio da cascata solta),
a frente precisou de um processo PARADO no meio da gravação de uma filha. O
lugar natural foi o `ndx::panico_de_teste::passar`, o ponto por onde o
`Table::atualizar` já passa com o pânico de teste. A primeira versão da pausa
escrevia um arquivo de aviso (`fs::write`) e dormia (`thread::sleep`) para
sempre.

A catraca `rede-ou-espera` do `bancada/concorrencia/mapa-da-trava.py`, teto
**0**, saiu **5**: `op_dblink_sincronizar`, `reaplicar_diario_ate` e outras três
seções «esperavam com a trava na mão» — pela cadeia
`atualizar_com_maes_opt_conferindo -> passar`.

## O que eu concluí primeiro, e estava errado

Que `#[cfg(debug_assertions)]` deixava o gancho fora de qualquer régua, como o
`#[cfg(test)]` deixa. O mapa separa só `cfg(test)`, e o gancho do store não pode
ser `cfg(test)`: o `cfg(test)` não atravessa crate, e o servidor compila o store
como dependência comum. Para a régua, `passar` é produção.

## O que a medição disse

- Com `sleep` + `fs::write`: `rede-ou-espera` 5 (teto 0), REPROVADO.
- Com `eprintln!` + `thread::park()`: 0 (teto 0); `alcancam-fsync-2` 23 = 23;
  `codigo-do-dono` 5 = 5. O pai lê o aviso no erro padrão do filho
  (`filho.err`), e a prova `sigkill_no_meio_da_cascata_solta_o_arranque_a_completa`
  continua vermelha com o defeito reposto (`[6, 5]`).

## A regra

Gancho de teste que mora numa função de produção entra na régua da trava: faça
o gancho com o mínimo de efeito que a prova precisa, e diga no comentário dele
que a régua o lê — para ninguém «melhorar» a espera dele de volta para
`sleep`.

## Como está guardado hoje

- O comentário do `armar_pausa` (`crates/phxsql-store/src/ndx.rs`) diz por que
  é `park` e erro padrão.
- O buraco: o mapa continua sem saber o que é `debug_assertions`. Um gancho
  novo que precise de rede ou de disco de verdade vai reprovar a catraca de
  novo, e a saída honesta aí é discutir a régua — não desviar dela. Não há
  guarda que acuse isso; a própria catraca é quem acusa, como acusou.

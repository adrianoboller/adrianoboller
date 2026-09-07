# Conversão protege o dado que já está lá; só a catraca protege o de amanhã

**07/09/2026, 15:48** — pedido 150, o alcance do conserto anterior.

## 1. O que aconteceu

O `phxsql-store` **já tinha sido convertido** numa rodada anterior do pedido
150: 22 arquivos, 203 testes, o crate inteiro sob o guarda `DirTemp`, com
prova real e com «zero diretório sobrando em `/tmp` depois da corrida»
escrito no próprio pedido.

Ao medir o `store` outra vez nesta rodada, antes de tocar nele: **21
diretórios por corrida**. Três sítios novos, todos do padrão velho, todos da
frente do `.fts` — que é de hoje mesmo (pedido 200).

Ninguém repôs o defeito por descuido de leitura. Repôs porque **escrever
`std::env::temp_dir()` é o que se conhece**, e nada na árvore dizia o
contrário no momento de escrever.

## 2. O que eu concluí primeiro, e estava errado

Concluí que o `store` estava fora do trabalho desta rodada. O pedido dizia,
com todas as letras, o que faltava: *«`phxsql-server` (233 testes…),
`phxsql-cmd/tests/console.rs` (9) e `phxsql-cli/src/main.rs` (1) — 301 testes
ainda sujando»*. O `store` não estava na lista, e a lista era recente e
detalhada.

Só que **lista de pendência é retrato, não sensor**. Ela conta o que era
verdade no dia em que foi escrita, e a frente do `.fts` entrou depois. Medi o
`store` por hábito de medir antes de mexer, não porque desconfiei — e foi o
hábito que achou, não a desconfiança.

## 3. O que a medição disse

| | antes | depois |
|---|---:|---:|
| `phxsql-server` + `cmd` + `cli` | **265** diretórios/corrida | **0** |
| `phxsql-store` (já «convertido») | **21** diretórios/corrida | **0** |
| `cargo test --workspace` | 1.669 testes | 1.669 testes, 0 falhas |

Os 21 saíram de **3 sítios**: `fts.rs::com_dobra` e dois em
`tests/indice-de-texto.rs`. Três sítios, num crate declarado 100% sob o
guarda, escritos no mesmo dia em que o guarda já existia há rodadas.

## 4. A regra

**Conversão protege o código que já está lá; só a catraca protege o que se
escreve amanhã — e o intervalo entre as duas é de uma frente.**

Esta é uma pétrea que já existe («catraca só desce»), e por isso o que este
arquivo registra não é a lei: é o **alcance** dela. O alcance é que *um
conserto completo, com prova real e número medido, não é uma garantia* —
é um retrato. A garantia é a régua que reprova o próximo, e ela não nasce
junto do conserto a não ser que alguém a escreva no mesmo commit.

E o corolário para escolher **quando** vale a catraca: quando o padrão errado
é o que qualquer um escreveria sem pensar. `std::env::temp_dir()` é a linha
óbvia; `DirTemp::novo` só é óbvia para quem já leu o guarda.

## 5. Como está guardado hoje

`crates/phxsql-server/src/conferidor_temporarios.rs`, catraca
`TETO_TEMP_DIR_SOLTO = 0`, com relatório em
`cargo run --example temporarios-sem-guarda -p phxsql-server`.

Duas decisões de desenho vieram deste caso e ficam registradas:

- **A lista de arquivos sai do disco**, não do código. Uma lista digitada
  teria o `fts.rs` de fora pelo mesmo motivo que a lista do pedido tinha o
  `store` de fora — a receita de um número também envelhece.
- **A isenção é por arquivo COM a quantidade esperada.** Isenção aberta
  («o `mensagens.rs` pode») deixaria uma chamada nova entrar num arquivo já
  perdoado, que é a mesma porta por outro nome. São 17 isenções hoje, cada
  uma com o motivo escrito ao lado.

O teto entra em `docs/CATRACAS.md` §6 e sai medido no `docs/QA-PDCA.md` pelo
`docs/qa/medir.py` — que o encontrou **sozinho**, porque o exemplo novo
imprime a linha `catraca:` que o gerador procura.

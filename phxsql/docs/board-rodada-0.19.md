# Board da rodada 0.19 — aberto em 23/09/2026 03:0x

Arquivo de apoio e controle de contexto (pétrea do `CLAUDE.md`). **Não é
entregável de produto.** Só o integrador comita, por caminho explícito.

## O que motivou a rodada

Pergunta do dono, 23/09/2026: *«Essa versão do motor não sai do 18. Tem muitos
gaps a serem feitos. Algo que eu possa te ajudar?»*

Medido na hora, e a primeira metade é **defeito, não estado**:

- `version = "0.18.0"` foi selada em **29/08/2026** (`baff46e`) e há **887
  commits** desde então. O número é **digitado à mão** no `Cargo.toml` e
  **nenhuma regra, portão ou gerador decide quando ele sobe**. O selo da capa
  do dossiê sai de gerador — mas o gerador **lê a linha digitada**. É a lei da
  casa quebrada um nível acima: o gerador está certo e a **fonte** dele
  envelhece calada.
- Agravante: o `empacotar.sh` trava «versão igual em `Cargo.toml`/`Cargo.lock`/
  `MANUAL`/`CHANGELOG`». Confere que as quatro **concordam**; nenhuma confere
  se o número ainda **descreve** o que existe. Quatro cópias do mesmo número
  velho passam na trava.
- Gaps: **101 abertos** — 87 planejados + 14 parciais. Deles **15 paravam no
  dono**, 21 são de segurança (4 BLOQUEIO/ALTO).

## O que o dono decidiu, em duas rodadas de pergunta

| # | decisão | quando |
|---|---|---|
| versão | portão da versão **e** selar 0.19.0 agora | 23/09 |
| 393 | o braço do `unir` vira **pedido**, não nome de tabela | 23/09 |
| 324 | **só o regime (b)**: adiar o `.ndx` sob a reserva, **sem thread** | 23/09 |
| 314 | ledger com cadeia **fica em v9**, e o motor **diz** o motivo | 23/09 |
| 342 | **exigir a cifra do fio** quando a tabela tem coluna marcada | 23/09 |
| 340 | saída **(d)**: selar a página do `.fts` — entra por aceite automático | 23/09 |
| 289 | nanos com **avanço forçado**, 8 bytes, `u64` | 17/09 07:10 |
| 290 | **passo** no esquema, **início** na identidade do nó | 17/09 07:10 |
| 355 | recusar na **declaração**: ledger + coluna marcada não nasce | 18/09 |

**Quatro dos que ele marcou já estavam decididos e só esperavam engenharia.**
Isso é achado de processo: pedido que carrega a decisão do dono no corpo
continua marcado «Planejado» e some da vista como se estivesse travado nele.

## Frentes desta onda

| frente | pedidos | nível do modelo | por quê |
|---|---|---|---|
| **P** | 289 + 290 + 314 | forte | formato em disco, migração que não se desfaz |
| **S** | 340 + 342 | forte | criptografia e formato juntos |
| **U** | 393 | forte | a trava única morre; é concorrência |
| **V** | versão + 0.19.0 | leve | regra mecânica e verificável |

**Dispensados com motivo** (dispensa registrada é decisão; silenciosa é
esquecimento): **E** designer — nenhuma frente toca tela; **J** pesquisador —
as quatro já vêm medidas; **D** zelador — roda de hora em hora e está
silencioso.

## Onda 2, e o motivo de não ser paralela

**324** (adiar o `.ndx` sob a reserva) mexe na reserva dentro do `servidor.rs`;
**393** mexe na trava do mesmo arquivo. Esta casa já pagou **três defeitos que
só apareceram no encontro das frentes**. Os dois entram em ordem, não juntos.

## Cerca de cada frente (para o defeito não nascer no merge)

- **P**: não toca `op_unir`, reserva do BULKINSERT, `.fts`, `Cargo.toml`.
- **S**: não toca `PSCH`/`schema.rs`, `op_unir`, reserva, `Cargo.toml`.
- **U**: não toca `PSCH`/`schema.rs`, `.fts`/cofre, reserva, `Cargo.toml`.
- **V**: **único** autorizado em `Cargo.toml`/`Cargo.lock`; não toca motor.

Quem precisar de algo fora da cerca **nomeia no relatório** em vez de mexer.

## Estado da árvore ao abrir

Ponta `e1edcc3`, local e no `origin` conferidos por `git ls-remote`. Árvore
limpa. Portão dos geradores VERDE (27 ok, zero velhos). Backup provado
(`phxsql-20260923-0117.bundle`, 1019 commits, restaurado e comparado). Suíte
**2.647 passando / 0 falhando**, `clippy` zero avisos. As sete páginas
republicadas.

## O que fica para o dono, ainda

- **326** — o dossiê compartilhado mostra uma versão **FIXADA**: quem abre o
  link não vê o que a gente publica. A troca é pelo menu Share, que o agente
  não alcança.
- As outras decisões travadas nele: 251, 255, 274, 293, 294, 300, 309, 325,
  333, 337, 368.

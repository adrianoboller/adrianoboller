# Board de controle — backlog aberto por pilar e por escalão

Esta é a planilha de comunicação que o dono pediu, no molde do Phoenix Cast,
**adaptada à nossa casa**. Ela responde a três perguntas de uma vez: *o que está
aberto*, *de que pilar é*, e *qual escalão de modelo cada item merece* — para
não gastar token à toa (pétrea de 16/09, `docs/MODELOS.md`).

O que ela **não** é: a fonte de verdade dos números. Os totais medidos vêm de
`docs/PENDENCIAS.md`, `docs/STATUS.md` e das tarefas; esta página é a
**avaliação datada** de escalão e dono, como o `STATUS.md` é a avaliação de nota.
Quem discorda de um escalão muda a linha aqui com o motivo; quem discorda de um
número roda o gerador ou a bancada.

## A régua do escalão — «o erro se vê?»

- **forte** — projeto e risco: arquitetura, cripto, formato em disco,
  concorrência, protocolo, transporte P2P, sigilo de contrato. O erro não aparece
  no teste; aparece em produção como dado perdido.
- **meio** — medição com interpretação, refatoração local com garantia sutil.
- **leve** — mecânico e verificável: tradução, documentação, varredura, rodar
  gerador ou bancada. O resultado se confere sozinho.

Papéis: A orquestrador · B engenheiro · C DBA · E designer · F prova-real ·
G QA · H documentação · I versionador · J pesquisador (+ subagentes
`pesquisa-motor`, `pesquisa-bancada`, `pesquisa-rede`) · SEC segurança.

---

## Pilar 1 — Base de dados (PhxSql)

| ID | entrega | dono | escalão | depende | status |
|---|---|---|---|---|---|
| P1-SQL-6 | Planejador de índice (o primeiro compatível vence) | B, C | **forte** | — | aberto |
| P1-SQL-2 | Subconsulta **correlacionada** (`IN`/escalar) | B, J | **forte** | P1-SQL-6 | aberto |
| P1-SQL-3 | `RIGHT`/`FULL`/`CROSS JOIN` | B | **meio** | — | aberto |
| P1-SQL-5 | `UNION`/`DISTINCT` | B | **meio** | — | aberto |
| P1-SQL-4 | `COUNT(coluna)`, `ORDER BY` por nome qualificado | B, H | **leve** | — | aberto |
| P1-158 | MCP (leitura + escrita guardada) + aviso de problema (e-mail+REST) | B, SEC | **forte** | — | aberto |
| P1-108 | Quórum de escrita, rota do canal aberto (pedido 207) | B, C | **forte** | — | aberto |
| P1-107 | Conferir se o estado parcial está velho (pedido 164) | B, G | **meio** | — | aberto |
| P1-82 | Corrida real do JNI (a premissa fechou; a corrida continua) | B, F | **meio** | ambiente | aberto |
| P1-95 | Bancada 1.000.000 de linhas em tabela complexa, COM e SEM senha | `pesquisa-bancada` | **leve** | — | aberto |
| P1-ISO | Isolamento acima de `READ COMMITTED` (Sombra/MVCC) | C, B | **forte** | — | **parado por decisão do dono** (`docs/SOMBRA.md`) |

## Pilar 2 — E-mail P2P *(domínio novo)*

| ID | entrega | dono | escalão | depende | status |
|---|---|---|---|---|---|
| P2-DESIGN | Desenho do transporte P2P: descoberta, NAT, gossip/anti-entropia, identidade sem domínio — **medido contra o nosso gargalo antes de virar plano** | `pesquisa-rede`, J | **forte** | — | aberto (pesquisa primeiro) |
| P2-CAIXA | Reenquadrar o #159 (server/client → P2P): caixa em disco, formato, protocolo de troca | B, C | **forte** | P2-DESIGN | aberto |
| P2-CIFRA | Caixa cifrada em repouso + identidade Ed25519 do par (fundação do fio já existe) | SEC, B | **forte** | P2-DESIGN | aberto |

## Pilar 3 — Blockchain de mini-contratos sigilosos *(domínio novo, fundação paga)*

Roteiro medido em `docs/propostas/phxblockchain-melhorias-2026-09.md`.

| ID | entrega | dono | escalão | depende | status |
|---|---|---|---|---|---|
| P3-1 | Id de algoritmo por bloco + **recusar `UPDATE`/`DELETE` no motor** para tabela-ledger | B, C | **forte** | — | aberto (o barato-e-cedo) |
| P3-2 | **Sigilo**: cifrar o corpo do mini-contrato; compromisso sobre o cifrado, não sobre o claro | SEC, `pesquisa-motor` | **forte** | — | aberto |
| P3-5 | Assinatura Ed25519 por bloco (o Ed25519 já existe e está em uso) | B, SEC | **forte** | — | aberto |
| P3-3 | Âncora externa via replicação WORM (fecha o furo do nó adversário) | B, C | **forte** | — | aberto |
| P3-4 | **Medir** o custo do `verificar_cadeia` (E6) ANTES de construir Merkle/MMR | `pesquisa-bancada` | **meio** | — | aberto (medir a premissa) |
| P3-6 | `DROP` redobrado (endurecimento) | B, C | **meio** | P3-1 | aberto |

## Governança / transversal

| ID | entrega | dono | escalão | status |
|---|---|---|---|---|
| GOV-1 | Agente multilíngua (tradutor pétreo) + documentador (pedido #110) | H | **leve** | aberto |
| GOV-2 | Ativar a equipe completa: pétrea de escalão, `docs/VISAO.md`, este board, e o subagente `pesquisa-rede` (os 11 já eram versionados no `.claude/agents/` do root) | A, I | **leve** | **fechado nesta rodada** |
| GOV-3 | Gerador do rollup deste board (contar aberto/fechado por pilar e escalão, medido) | H | **leve** | aberto (gap: hoje o rollup seria digitado) |

---

## Como esta página fecha um item

Um item só sai daqui com o mesmo aceite do resto da casa: **prova real nos dois
sentidos** (falha com o defeito reposto, passa com o conserto), portões verdes
(`fmt`, `clippy` zero avisos, suíte), e o número **medido**, nunca de memória. O
escalão da linha é o do trabalho de **decidir**; a integração e o commit são
sempre do orquestrador. Ao fechar, o item vira pedido em `docs/PENDENCIAS.md`
com a evidência, e some deste board.

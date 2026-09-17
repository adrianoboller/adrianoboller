# Rodada de 17/09/2026 — Replicação: bateria, revisão e conclusão

Ordem do dono, 17/09/2026 02:27 UTC: «Dossiê atualizado · Status · Replicação
bateria de testes, revisão e conclusão». Board de apoio do orquestrador (papel A);
não é entregável de produto. Números só medidos.

## Estado medido antes de convocar (02:27 UTC)

- `bancada/replicacao/resultados.json` — quando `2026-09-07` (mtime 07/09 16:24)
- `bancada/cluster/resultados.json` — sem `quando` (mtime 11/09 22:20)
- `bancada/quorum/resultados.json` — quando `2026-09-07 16:37`
- `bancada/replicacao/docker/resultados.json` — sem `quando` (mtime 05/09 05:49)
- portão «está medindo?» livre · disco ~2,3 GiB · Docker: daemon FORA do ar
- binário `target/release/phxsqld` 00:59 **mais velho** que `ui/index.html` 01:01
- `docs/REPLICACAO.md` termina na §20 (credencial recusada) · `STATUS.md` B = 8, construído

## Onda 1 — em paralelo (largada 02:30 UTC)

| papel | frente | escalão | por que | dispensa |
|---|---|---|---|---|
| F | a bateria inteira: build, `montar`+`medir`, `modos`, `trava`, `credencial-recusada`, `cluster/provar`+`fresta`+`escalonar`, `quorum/medir`+`canal`, docker se o daemon subir | forte | prova real é o papel mais fácil de fingir; cada ERRO precisa de diagnóstico medido separando motor/bancada/encontro de frentes | — |
| SEC | revisão adversária de replica/cluster/quórum/portões 2a–2b-bis, só leitura | forte | sempre o mais forte; cada achado exige prova de leitura e severidade | — |
| C | parecer das garantias de dado: o que a réplica garante, o commit com cascata, a posição, o bidirecional, PITR, formato pendente | forte | formato em disco e garantias; é quem diz NÃO | — |
| G | inventário guarda × pétrea da replicação, ESTÁTICO, e a lista dos `--so` para depois da bateria | médio | inventário é mecânico mas «pétrea sem guarda» exige leitura; não compila para não disputar o flock com F | — |

## Onda 2 — depois da onda 1

| papel | frente | escalão | por que |
|---|---|---|---|
| G (A roda) | `provar-guardas.py --so` da família da replicação, na ordem de G | — | compila; só depois de F soltar as portas e o flock |
| B | só se F/SEC/C acharem defeito com conserto delimitado | forte | motor/concorrência |
| H | REPLICACAO.md §21 (bateria de 17/09, revisão, conclusão), STATUS.md linha B reavaliada, PENDENCIAS, CHANGELOG, geradores | médio | todo número sai de gerador ou dos relatórios; varredura verificável |
| A/I | integração, portões, commit por caminho, push, sete páginas, backup | — | — |

## Dispensas registradas

- **E (designer)** — nenhuma tela nova; as páginas se regeneram do mesmo molde exercitado esta noite.
- **D (zelador)** — rodou às 02:15 (33 MiB; 2,3 GiB livres); o gatilho de hora em hora continua. F apaga o que a bancada cria, por caminho.
- **J (pesquisador)** — a revisão é do que existe, não do que os outros fazem; o quórum já tem a matriz de evidência (`quorum-de-escrita.md`).
- **tradutor** — nenhum texto de tela.

## Regras desta rodada (para toda frente)

- Ninguém comita; ninguém mexe no índice do git.
- Todo `cargo` sob `flock /tmp/phx-cargo.lock`; bancada de tempo passa pelo `esta-medindo.sh`.
- Número só medido, com data e hora; hipótese sem medição é «não medido».
- Nome de modelo de IA não entra em arquivo nenhum: o `MODELOS.md` guarda o nível.

## Retornos da onda 1

- **G — voltou 02:47 UTC** (12 min 48 s, 93 ferramentas). Treze entradas do catálogo tocam a família (12 provadas em 16/09 15:25, 1 nascida em 17/09 e nomeada como não julgada). Conferido por A: `cluster.rs` tem **0** entradas no `catalogo.py` (grep do campo `arquivo`), e `TETO_DO_LOTE_SERVIDO`/`TETO_DA_RESPOSTA` só existem na declaração (`servidor.rs:567`) e num uso (`:21802`) — sem teste e sem bancada. Sete pétreas com teste real e sem guarda no catálogo, nomeadas com o teste que cairia. Lista de 13 `--so` (~189 s) para depois de F. Entregas: `docs/propostas/inventario-qa-replicacao-2026-09-17.md`, cognição `cluster-fora-do-catalogo-de-guardas_20260917_0234`.
- **C — voltou 02:55 UTC** (16 min 49 s, 92 ferramentas). Oito garantias que NÃO valem, três delas MEDIDAS num binário isolado fora do repositório: `rownum` diverge entre source e réplica depois de uma inserção recusada (contador consumido antes do CHECK/unicidade; 2 de 5 linhas diferentes com rowids iguais); unicidade num índice secundário na réplica PARA o par (o lote volta para sempre); 12 eventos num único milissegundo («mais recente vence» empata como regra). Duas correções ao briefing (cabeçalho do evento tem 44 bytes, `log.rs:81`; `reconciliar_sequencia` é do store). Seis decisões do dono (a mais urgente: coluna de data/hora de sistema por linha — e a medição dos 12/1 ms muda o desenho). Cinco NÃO. O item de maior retorno não é formato: a réplica não tem a conferência de continuidade que o PITR tem (`diario_vivo_continua`, `servidor.rs:18640`) — o irmão ficou. Conferido por A: `EVENTO_CAB = 44`, `return Ok(0)` em `servidor.rs:2808`, `julga_integridade` e `diario_vivo_continua` existem. Entregas: `docs/propostas/parecer-dba-replicacao-2026-09-17.md` (556 linhas), cognição `contador-consumido-antes-da-recusa_20260917_0239`.
- **J — voltou 02:56 UTC** (14 min 10 s, 65 ferramentas) — a frente paralela do Query Designer do Phoenix. Recusar o CÓDIGO da crate com número (concatena valor no SQL sem escapar aspa, `lib.rs:309` — conferido por A; 0 linhas `///` em 91 `pub`; a API do `.md` não é a do zip: `sql_detalhe`/`Consulta` têm 0 ocorrências). Aproveitar a IDEIA de duas coisas: `<dataBar>` no XLSX (0 ocorrências em `crates/` — conferido; ~6 linhas de XML) e um construtor visual de consulta com AND/OR (a tela «Consulta» é de UMA condição e a op `sql` só é alcançável da tela pelo painel de IA, `claude.js:1317` — conferido). Seis recursos do mockup já existem aqui, dois deles mais fundo (sete junções com Venn, `juncao.rs` 1.159 linhas; `conferir_uniao` confere contagem E tipo). Correção ao briefing de A: LLM local custa UMA origem no `connect-src` (`http.rs:303`), não cliente HTTP — o `fetch` sai da tela. Relatório com bandas reabre o 161 pela mão do dono. Entregas: `docs/propostas/phoenix-query-designer-2026-09-17.md` (558 linhas), cognição `a-recusa-tambem-se-mede-o-llm-local-custava-uma-linha-de-csp_20260917_0244`.

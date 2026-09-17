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

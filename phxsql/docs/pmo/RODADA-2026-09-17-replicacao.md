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
| B | os cinco itens delimitados do contrato abaixo (A5, A2+G, A1 parcial, A3, continuidade de C) — **largada 02:59 UTC**, com F ainda de pé mas sem bancada medindo (flock e portas livres, medido) | forte | motor, concorrência e portão de permissão: o escalão mais forte disponível |
| H | REPLICACAO.md §21 (bateria de 17/09, revisão, conclusão), STATUS.md linha B reavaliada, PENDENCIAS, CHANGELOG, MODELOS — **largada 02:57 UTC em duas etapas**: a revisão (SEC/C/G/J, já devolvidos) agora; a bateria, a conclusão e a linha B quando F e B devolverem | médio | todo número sai de gerador ou dos relatórios; varredura verificável |
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

- **G — integrado em `728a46f` (02:43 UTC)** (12 min 48 s de frente, 93 ferramentas). Treze entradas do catálogo tocam a família (12 provadas em 16/09 15:25, 1 nascida em 17/09 e nomeada como não julgada). Conferido por A: `cluster.rs` tem **0** entradas no `catalogo.py` (grep do campo `arquivo`), e `TETO_DO_LOTE_SERVIDO`/`TETO_DA_RESPOSTA` só existem na declaração (`servidor.rs:567`) e num uso (`:21802`) — sem teste e sem bancada. Sete pétreas com teste real e sem guarda no catálogo, nomeadas com o teste que cairia. Lista de 13 `--so` (~189 s) para depois de F. Entregas: `docs/propostas/inventario-qa-replicacao-2026-09-17.md`, cognição `cluster-fora-do-catalogo-de-guardas_20260917_0234`.
- **C — integrado em `9da28a4` (02:47 UTC)** (16 min 49 s de frente, 92 ferramentas). Oito garantias que NÃO valem, três delas MEDIDAS num binário isolado fora do repositório: `rownum` diverge entre source e réplica depois de uma inserção recusada (contador consumido antes do CHECK/unicidade; 2 de 5 linhas diferentes com rowids iguais); unicidade num índice secundário na réplica PARA o par (o lote volta para sempre); 12 eventos num único milissegundo («mais recente vence» empata como regra). Duas correções ao briefing (cabeçalho do evento tem 44 bytes, `log.rs:81`; `reconciliar_sequencia` é do store). Seis decisões do dono (a mais urgente: coluna de data/hora de sistema por linha — e a medição dos 12/1 ms muda o desenho). Cinco NÃO. O item de maior retorno não é formato: a réplica não tem a conferência de continuidade que o PITR tem (`diario_vivo_continua`, `servidor.rs:18640`) — o irmão ficou. Conferido por A: `EVENTO_CAB = 44`, `return Ok(0)` em `servidor.rs:2808`, `julga_integridade` e `diario_vivo_continua` existem. Entregas: `docs/propostas/parecer-dba-replicacao-2026-09-17.md` (556 linhas), cognição `contador-consumido-antes-da-recusa_20260917_0239`.
- **J — integrado em `9da28a4` (02:47 UTC)** (14 min 10 s de frente, 65 ferramentas) — a frente paralela do Query Designer do Phoenix. Recusar o CÓDIGO da crate com número (concatena valor no SQL sem escapar aspa, `lib.rs:309` — conferido por A; 0 linhas `///` em 91 `pub`; a API do `.md` não é a do zip: `sql_detalhe`/`Consulta` têm 0 ocorrências). Aproveitar a IDEIA de duas coisas: `<dataBar>` no XLSX (0 ocorrências em `crates/` — conferido; ~6 linhas de XML) e um construtor visual de consulta com AND/OR (a tela «Consulta» é de UMA condição e a op `sql` só é alcançável da tela pelo painel de IA, `claude.js:1317` — conferido). Seis recursos do mockup já existem aqui, dois deles mais fundo (sete junções com Venn, `juncao.rs` 1.159 linhas; `conferir_uniao` confere contagem E tipo). Correção ao briefing de A: LLM local custa UMA origem no `connect-src` (`http.rs:303`), não cliente HTTP — o `fetch` sai da tela. Relatório com bandas reabre o 161 pela mão do dono. Entregas: `docs/propostas/phoenix-query-designer-2026-09-17.md` (558 linhas), cognição `a-recusa-tambem-se-mede-o-llm-local-custava-uma-linha-de-csp_20260917_0244`.
- **SEC — integrado em `75b2f33` (02:54 UTC)** (20 min 10 s de frente, 116 ferramentas). Onze achados de leitura, arquivo:linha: dois de severidade alta e independentes — A1, o pulso do cluster aceita identidade auto-declarada e época/posição/prioridade sem teto, porque `cluster_pulso` não está em `OPS_DE_REPLICACAO` (`servidor.rs:309`) e o portão 2a-bis só tranca essa lista; A2, `replicar` com `"max":0` lê o diário inteiro com imagens sob a trava global e o `TETO_DO_LOTE_SERVIDO` corta depois. A3 (média-alta) bate na pétrea: `aplicar` num mestre sem `somente_leitura` desliga FK/CHECK/cascata. Mais A4–A11 (maiorias assimétricas, sonda sem prazo, mapa da infraestrutura a quem só lê, IP atrás de proxy, dado pessoal sem trilha, carimbo sem teto, `config` com a lista do arranque, oráculo de ids) e o §Z já documentado e ainda aberto. **Posição de SEC para a conclusão: A1, A2 e A3 exigem decisão registrada (conserto ou aceite do dono) antes de a replicação se declarar revisada e conclusa.** Conferido por A: os cinco pontos verificáveis batem no fonte (lista, `de_json`, `max` sem clamp com `if limite > 0` em `log.rs:692`, `ligar`→`connect` sem prazo, `cluster_estado`→`Ler`). Entregas: `docs/propostas/revisao-sec-replicacao-2026-09-17.md` (879 linhas), cognições `o-portao-tem-duas-listas…_20260917_0241` e `teto-que-corta-a-resposta…_20260917_0241`.

- **F — integrado em `34ef2c1` (03:11 UTC)** (41 min 41 s de frente, 129 ferramentas). Dez bancadas, todas verdes, binário release reconstruído às 02:29:41 e portão «está medindo?» livre antes de cada tempo: `medir.py` 45.117 linhas/s no master (33.883 em 07/09, 1,33×), réplica 43.606 ev/s, alcance 2,3 s, retrato `72554b753253cd5d` nos quatro; `modos.py` 9/9 (não grava arquivo); `trava.py` 4/4, alcance 2,39 s (4,54 s em 05/09); `credencial-recusada.py --tela` 8/8 em navegador; `cluster/provar.py` 26/26, promoção 4,3 s; `cluster/fresta.py` 10/10 (não grava arquivo); `cluster/escalonar.py` 0 de 43 escritas recusadas; `quorum/medir.py` 3,04×/3,56×; `quorum/canal.py` 0,146 ms contra 0,089 — faixas se cruzam, sem vencedor; `docker/provar.py` 16/16 em 7,9 min, com `dockerd` subido e parado pela frente. **Os três achados medidos de C confirmados pelo soquete com controle por estágio** (`bancada/replicacao/achados-do-dba.py`, novo, 03:00 UTC): o `rownum` só queima dentro de `inserir_lote` com `parar_no_erro:false` (recusa em operação própria não queima — a instância é reaberta), único secundário para o par inteiro, tabela recriada congela a réplica em silêncio. Duas premissas mortas: não há fsync por escrita (104 em 100.000 linhas) e o contêiner não é 2,8× mais lento — é a libc (gnu 48.510 × musl 20.965, `custo-do-binario.py`, novo). Cinco decisões nomeadas para A/dono: guarda de queda que enfraquece sozinha, dois blocos sem gerador sob a data de hoje, quatro `resultados.json` sem data, duas bancadas sem arquivo e quatro medidas invisíveis na página dos testes. Conferido por A: `quando` de hoje e 45.117 no `resultados.json`, `medido_em` 03:00 no `achados-do-dba.json`, 2,31 no `custo-do-binario.json`, `docker.sock` ausente, binário 02:29:41 > `index.html` 01:01:51, sete JSON válidos e `cluster/resultados.json` byte a byte igual ao commitado. Entregas: `docs/propostas/bateria-replicacao-2026-09-17.md` (500 linhas), cognições `guarda-que-enfraquece-quando-o-motor-melhora_20260917_0235` e `recusa-que-so-queima-o-contador-dentro-do-lote_20260917_0258`.

- **H etapa 1 — integrado em `e6c86f0` (03:17 UTC)** (19 min 17 s de frente, 68 ferramentas). 31 pedidos novos, 278–308, todos ☐ e todos com fonte, hora e arquivo:linha: SEC 278–288, C 289–300, G 301–303, J 304–305, F 306–308; quatro acréscimos in-place (item 16 do §3.2, 161, 193, 291) em vez de duplicar. REPLICACAO.md §21.1–21.4 (294 linhas), MODELOS.md (rodada, 38 linhas), CHANGELOG Sabido (32 linhas). Conferido por A: numeração 278–308 sem falta nem duplicata, zero nome de modelo e zero placeholder no diff; uma frase do CHANGELOG dizia «F continua em curso» depois de F ter devolvido — corrigida pelo integrador no mesmo commit. Contador do topo do PENDENCIAS continua «277» até o `pagina-dos-pedidos.py` rodar no fecho. Etapa 2 (§21.5, linha B do STATUS, Corrigido) espera B.

## Retorno da onda 2

- **B — integrado em `49a3af7` (03:53 UTC)** (49 min 12 s de frente, 124 ferramentas). Os cinco itens do contrato, inteiros, +1.240/−111 em sete arquivos mais o teste novo `tests/continuidade-da-replica.rs` (313 linhas). RED medido em cada um: `ligar` pendurado além de 5 s com o `connect` cru; `left: 20 right: 4` no `percorrer` sem teto e `left: 600 right: 500` no `replicar max:0`; época absurda envenenando `maior_epoca_vista` e pulso passando fora da lista; `aplicar` pela rede apagando o pai com filha num source aberto (`aplicados: 1`); réplica com `[1, 2, 3, 14]` e carimbos lavados. Irmãos além do contrato, achados pela compilação e cobertos pela mesma linha: dblink para PhxSql e console (prazo). Medição prévia do item 4: zero chamadores de `aplicar` pela rede em source/isolado. Conferido por A: `OPS_DE_REPLICACAO` com quatro entradas (`servidor.rs:320`), crivo 2b-bis sem `somente_leitura` (`:9518`), chave nova na fábrica e zero referências à velha, `PRAZO_DE_CONEXAO`/`FOLGA_DE_EPOCA`/`LOTE_PADRAO`/`TETO_DE_EVENTOS_POR_LOTE` existem, `percorrer` com `teto_bytes`, `forcar_proximo_evento` na réplica fiel (`:2831`); portões rodados de novo pelo integrador: fmt limpo, clippy 0, **2.427 passaram / 0 falharam / 4 ignorados** (71 binários, 1 min 36 s). Decisões que B deixou nomeadas para o dono: `FOLGA_DE_EPOCA = 1.000.000`, `TETO_DE_EVENTOS_POR_LOTE = 5.000` (não amarrado ao `max_linhas`), `isolado` também recusa `aplicar` pela rede, a réplica fiel grava carimbo/origem do source, a conferência de continuidade não entrou no bidirecional, texto da recusa fora da fábrica (é estado, não erro de portão). Documentação que envelheceu: REPLICACAO §6/§7/§13/§18 e linha ~271, CLUSTER.md, PENDENCIAS #214(c) — para H, etapa 2.

## Onda 2 — contrato da frente B (escrito às 02:58 UTC, larga quando F soltar o flock)

Escalão **forte** (motor, concorrência e portão de permissão). Cinco itens
independentes, em ordem de risco crescente; cada um inteiro ou não entra —
meia funcionalidade que for pior que nada volta como parecer, não como código.
Prova real nos dois sentidos em todos: o teste FALHA com o defeito reposto.

1. **A5 (SEC)** — `replica::ligar` cai em `Cliente::conectar` sem prazo
   (`replica.rs:409-415`); `conectar_com_prazo` (`replica.rs:80`) só serve o
   pulso (`servidor.rs:3010`, `:3742`). Conserto: `ligar` passa a usar o irmão
   com prazo. Prova: a que o SO permitir (endereço não roteável com prazo
   curto, medindo o tempo de retorno), documentada.
2. **A2 (SEC) + G** — `op_replicar` lê `max` sem clamp (`servidor.rs:21770`:
   `.max(0)`), e `log.rs:692` trata `limite == 0` como «sem limite»; o
   `TETO_DO_LOTE_SERVIDO` (16 MiB) corta a RESPOSTA depois de ler tudo sob a
   trava. Conserto: `0` vira o padrão, `max` ganha teto de servidor, e o teto
   de bytes entra em quem aloca (`diario_com_imagem`/`percorrer`), não em quem
   responde. Irmãos a conferir com a mesma pergunta: `op_diario`
   (`servidor.rs:21426`, passa por `self.limite`) e `absorver_diario_local`
   (`servidor.rs:4130`, passa `0`). Prova: unitário `percorrer_com_limite_zero_nao_le_tudo`
   medindo QUANTO foi lido; e o teste dos dois tetos que G apontou sem prova.
3. **A1 parcial (SEC)** — `cluster_pulso` entra em `OPS_DE_REPLICACAO`
   (`servidor.rs:309`) para o portão 2a-bis alcançá-lo; e época/posição do
   pulso ganham teto sadio (`maior_epoca_vista + folga`). Prova:
   `um_pulso_de_epoca_absurda_nao_destrona` (unitário em `cluster.rs`) e o
   teste do comportamento VELHO: lista vazia, nada muda. **A amarração da
   identidade do nó à chave do fio é desenho e vai para o dono** — não entra.
4. **A3 (SEC)** — o crivo de papel do portão 2b-bis (`servidor.rs:9162`) só
   roda com `somente_leitura`; num source gravável `aplicar` pela rede desliga
   FK/CHECK/cascata. Conserto: o crivo passa a valer independentemente do
   `somente_leitura` (cluster continua fora, como hoje). **Antes de mexer,
   medir quem chama `aplicar` pela rede** num servidor gravável (grep em
   `bancada/`, `tests/`, `ui/`, docker): se houver uso legítimo, volta como
   parecer. Prova: `aplicar_pela_rede_num_source_nao_mata_o_pai_com_filhos`
   + o velho (`excluir` normal recusa; réplica/multi continuam aplicando).
5. **Continuidade (C)** — `alcancar_tabela` (`servidor.rs:2808`,
   `if posicao >= no.eventos { return Ok(0) }`) devolve silêncio quando o
   source apagou e recriou a tabela; o PITR tem `diario_vivo_continua`
   (`servidor.rs:18640`) para o mesmo caso. Conserto: a réplica confere o
   evento `posicao-1` do source contra o seu (um evento pela rede) e, se não
   bate ou o source tem menos eventos, marca a tabela como incompleta com a
   frase «a tabela foi apagada e recriada» em vez de `Ok(0)`. Sem formato
   novo. Prova pelo soquete: dois servidores, tabela apagada e recriada no
   source, réplica acusa em vez de calar.

Fora de B (mesa do dono): A1 identidade (`known_hosts` pela chave do fio),
A4, A6, A7, A8, A9, A10, A11, as seis decisões de formato de C, o 161.

- **H etapa 2 — integrado em `46b2420` (04:05 UTC)** (10 min 26 s de frente, 75 ferramentas). CHANGELOG +76/−16 (cinco Corrigido, dois Mudado, Sabido reescrito sem contradizer o Corrigido), CLUSTER.md +19, MODELOS +26/−8, PENDENCIAS 279/280/295 ☑️ e 278/282/303 ◐ com commit e testes, REPLICACAO §6/§7/§13/§18/§21.1–21.3 +69/−7, STATUS linha B **8 → 7** datada com fonte por afirmação (e duas afirmações já contraditas por pedidos de 07/09 e 08/09 saíram). Conferido por A: estados no arquivo, zero nome de modelo, zero placeholder (três acertos do crivo eram «todo/todos»). §21.5 espera os vereditos das guardas depois de G2 e B2.

## Onda 3 — «Continue fazendo os gaps» (ordem do dono, 03:58 UTC)

Os 13 `--so` de G rodados por A às 03:53–03:57 UTC, um por vez, cada um com
o próprio JSON (fora do `ultima-corrida.json`, que é a corrida completa de
16/09 e não pode ser sobrescrito por uma parcial): **7 PROVADA**
(`replica-julga-fk` 1,59 s, `cascata-sem-imagem-no-diario` 1,66 s,
`replica-refaz-a-cascata` 1,58 s, `bidirecional-julga-fk` 1,43 s,
`bidirecional-julga-as-filhas` 1,46 s, `marca-de-replica-fica-acesa` 1,55 s,
`pulso-do-cluster-em-claro` 19,56 s); **1 QUEBRADA** — `trava-atras-da-rede`,
«o trecho não está mais em `servidor.rs`»: envelheceu com `49a3af7`, e o
`trecho-vivo.py --catraca` reprova com `TETO_TRECHO_MORTO: 1 (teto 0)` às
04:00 (encontro de frentes: B1 mexeu no lote e a âncora da guarda ficou para
trás); **5 sem veredito por defeito do provador** (as `--lib`:
`colisao-de-sequence-calada`, `replicacao-do-cluster-em-claro`,
`posicao-sem-portao`, `replica-insiste-na-credencial-recusada`,
`cluster-devolve-a-credencial-na-tela`) — a árvore LIMPA da cópia reprova em
`segredos::testes::todo_parametro_com_cara_de_segredo_esta_na_lista`, que lê
`bancada/guardas/debug-com-segredo.py` por `CARGO_MANIFEST_DIR/../..`, e a
lista `COPIAR` do provador (`provar-guardas.py:118`) não copia `bancada/`.
Medido por A nos logs; a suíte no repositório passa (2.427/0/4).

| papel | frente | escalão | por que | largada |
|---|---|---|---|---|
| B2 | nove gaps com conserto delimitado e sem formato, em ordem de valor: rownum no lote (291), A1 pleno pelo túnel (278), A8 trilha LGPD do `replicar` (285), A4 `propagar:false` (281), A6 `cluster_estado` partido (283), A5 resto (282), A9 (286), A10 (287), A11 (288); cada um mede a premissa antes e volta como parecer se ela cair | forte | segurança, concorrência e integridade | 04:01 |
| G2 (estático) | reancorar `trava-atras-da-rede`; provador copiar o que os testes leem fora de `crates/` com conferidor derivado do código; `--so --json` mesclar por id em vez de sobrescrever; sete pétreas + `cluster.rs` no catálogo (301/302); lista de `--so` para A | médio | catálogo e ferramenta em Python, sem cargo — o provador copia a árvore de trabalho e B2 está mutando `crates/` | 04:02 |
| F2 | depois de B2: `quando` nos quatro `resultados.json` sem data (308), `modos.py`/`fresta.py` gravando arquivo e a página dos testes vendo as quatro medidas (193), a guarda de queda do `trava.py` (306), os dois blocos sem gerador (307), teste do `TETO_DA_RESPOSTA` (303), e a corrida das guardas | forte | bancada com servidores de pé mede tempo — não pode correr junto de B2 | fila |
| E + B3 | onda 4: as duas ideias de J — `<dataBar>` no XLSX (304) e o construtor visual AND/OR (305), exercitados no navegador | forte/médio | tela só se prova exercitando | fila |
| H | etapa 3 depois de B2/G2/F2: pedidos, REPLICACAO §21.5 conclusão, CHANGELOG, STATUS | médio | — | fila |

Fora de qualquer onda (mesa do dono, formato): as seis decisões de C
(289–294), o 161, e o que B2 devolver como parecer.

### Retornos da onda 3

- **G2 — integrado em `6470943` (04:31 UTC)** (28 min 10 s de frente, 128 ferramentas, sem cargo). `trava-atras-da-rede` reancorada no laço do `puxar` (mesmo defeito); `COPIAR` do provador ganha os dois arquivos que testes leem por `CARGO_MANIFEST_DIR` (`debug-com-segredo.py` e `mapa-das-threads.py` — o segundo era buraco latente) e o conferidor `verificar_copiar()` com autoteste nos dois sentidos; `--so --json` mescla por id (8 casos de autoteste); sete entradas novas, `cluster.rs` sai do zero, `PISO_DAS_ENTRADAS` 180→187; `docs/TESTES.md` republicado nomeando 44 não julgadas de 187. Conferido por A às 04:30: `trecho-vivo.py --catraca` sete réguas verdes (rc 0), `--autoteste-copiar`, `--autoteste-mescla-json` e `--conferir-copiar` todos rc 0, zero nome de modelo. Fica: 13 `--so` para A rodar depois de B2 (5 sem veredito + 1 reancorada + 7 novas), num só `--json bancada/guardas/ultima-corrida.json` para usar a mescla; `TETO_DA_RESPOSTA` continua sem guarda por não haver teste que caia (F2). Cognição `copiar-e-lista-que-precisa-de-conferidor_20260917_0426`; parecer `docs/propostas/qa-onda3-replicacao-2026-09-17.md`.
- **B2 — integrado em `eeb9925` (04:46 UTC)** (41 min 43 s de frente, 116 ferramentas). Oito dos nove itens inteiros, +1.385/−52 em nove arquivos, 20 testes novos, RED medido em cada: rownum `[1,2,3,5]` contra `[1,2,3,4]`; trilha `total: 0` contra 1; `propagar:false` de cliente removendo nó (`removido: true`); leitor recebendo `nos[]`; sonda vazando `Connection refused (os error 111)`; toque com carimbo `i64::MAX`; `config` com a lista de ontem (2 contra 3); pulso com `ESQUEMA_INVALIDO` contra `ACESSO_NEGADO`. Item 2 (A1 pleno) voltou como parecer com a premissa morta na leitura do `fio.rs`: aperto NX, iniciador anônimo, a sessão só guarda a transcrição — identidade do pulso exige prova por DH das estáticas ou XX/IK (desenho, dono). Decisões nomeadas: via (a) do 291 (não retroativa), `FOLGA_DO_CARIMBO_MS` 5 min, `contar_pulso_desconhecido` nasce desligado, quatro classes de falha na sonda, `isolado`/`degradado`/`epoca` continuam para `ler`. Conferido por A: `consumir_rownum`, `sem_propagar_so_de_dentro`, `chave_da_falha_de_rede`, `sondar_origem`, `FOLGA_DO_CARIMBO_MS`, seis chaves na fábrica, NX no `fio.rs`; portões rodados de novo: fmt limpo, clippy 0, **2.447 / 0 / 4** (71 binários). Documentação que envelheceu (CLUSTER §2.3/§2.5, SEGURANCA tabela, REPLICACAO §12/§21, LGPD §4, MENSAGENS, FORMATO rownum, CIFRA-DO-FIO §12, CAPABILITIES) — H etapa 3.

- **H etapa 3 — integrado em `4be5ce5` (05:03 UTC)** (12 min 36 s de frente, 99 ferramentas). +317/−39 em onze arquivos: oito pedidos ☑️ (281, 282, 283, 285, 286, 287, 288, 291 via (a)), 278 ◐ com o parecer do NX, **309 novo** (via (b) do rownum, compatível e não-retroativa). CLUSTER §2.3/§2.5, SEGURANCA (linha `contar_pulso_desconhecido` e a sonda), LGPD §4, FORMATO §rownum, CIFRA-DO-FIO §12 (o parecer completo), MENSAGENS (seis chaves), MODELOS (onda 3), CHANGELOG (oito Corrigido, quatro Mudado, Sabido reescrito), REPLICACAO §12/§21.1/§21.2. **STATUS linha B 7 → 8**, segunda reavaliação do mesmo dia, com as duas horas escritas na própria linha e o motivo: nove dos onze achados de SEC fecharam hoje com prova real; ficam A1 pleno (parecer), A7, cinco decisões do dono e o 309. Conferido por A: os dez estados no arquivo, zero nome de modelo, e que H não tocou o `ultima-corrida.json` das guardas (ela mesma nomeou o arquivo alheio). §21.5 espera os vereditos.

- **As 13 guardas — corridas por A, 04:50–05:03 UTC, integradas em `9438bd0`**: **13 de 13 PROVADA** (16 a 36 s cada). A `trava-atras-da-rede` reancorada voltou a pegar o defeito; as cinco sem veredito eram o provador, não o código; a mescla por id levou o arquivo de 143 a **151 julgadas de 187**, com data por entrada, e as 36 restantes seguem nomeadas no `docs/TESTES.md`. Sete réguas verdes, piso 187.
- **H etapa 4 (§21.5, a conclusão) — integrada em `25440d1` (05:07 UTC)** (2 min 22 s de frente). Declara revisado o que tem prova e nomeia o que falta com o pedido de cada um; registra que a condição de SEC está cumprida por leitura — A2/A3 por conserto, A1 por parecer com premissa medida — e diz onde reler o argumento. Fecha com as três vezes em que o defeito estava no instrumento. Conferido por A: não repete «pronta» nem a frase da folha de marca (a única ocorrência é a negação), e os hashes citados existem.

## Onda 4 — «Parcial 164: fazer» (ordem do dono, 04:57 UTC)

O 164 deve duas coisas, e só elas: **encurtar as cinco seções do gatilho
BEFORE** e **a medição final em máquina parada**. O resto dele (MVCC, RwLock,
Sombra) está medido e decidido — a Sombra é decisão do dono e não se reabre.

Estado medido por A às 04:56 UTC (`bancada/concorrencia/mapa-da-trava.py`):
**86 seções críticas**, **5 rodam código do dono do banco com a trava na mão,
419 linhas** — `empilhar` (`servidor.rs:13642`, 162), `op_inserir` (`:17531`,
86), `op_inserir_lote` (`:17703`, 82), `op_atualizar` (`:17952`, 48),
`op_excluir` (`:18027`, 41); todas soltam a trava cedo. O teto de **parede**
(`PRAZO_DO_GATILHO_ANTES = 500 ms`, `servidor.rs:696`) já existe desde 03/09 e
não se refaz.

| papel | frente | escalão | por que | largada |
|---|---|---|---|---|
| B (164-B) | medir a repartição do tempo DENTRO da seção crítica — preparação × corpo do dono × gravação — e só então decidir se há o que tirar da trava; conserto com prova real, ou parecer com o número se a preparação for irrelevante diante do corpo | forte | concorrência e semântica de gatilho: o que o gatilho enxerga não pode mudar | 04:58 |
| F (164-F) | a medição final em máquina parada, com o `quieta.Vigia` aprovando, e o mapa da trava remedido | forte | bancada de tempo não corre junto de compilação | fila, depois das provas de guarda e de B |

Limite escrito no contrato de B: não mexer no teto de parede, não ajustar a
catraca `alcancam-fsync` (23/22 é o pedido 252, mesa do dono) e não tocar
formato. Se a preparação fora da trava abrir janela para o estado mudar entre
preparar e executar, o item vira parecer — meia garantia é pior que nenhuma.

## Onda 5 — a fila depois dos dois pedidos do dono (06:25 UTC)

Integrado antes desta onda: **164** (`20d2c59`) e **190** (`6319396`), com os
portões do integrador nos dois (fmt limpo, clippy 0, 2.450/0/4; conferidor de
botões 321/182/22/**119**, catraca em 119; textos cravados 950 com catraca 950
intacta). E as **catorze decisões do dono** dos parciais, gravadas nos pedidos
(`f8b1040`, `df92381`, `1c4b94b`, `57b983f`): 179 e 194 fecharam como recusa
medida; 197, 207, 229, 238, 239, 245, 249, 259, 263, 273, 278 e 303 com rumo
escrito.

| papel | frente | escalão | por que | largada |
|---|---|---|---|---|
| H4 | a documentação que ficou para trás nos dois pedidos: #164 (o «falta encurtar» morreu medido; «76 seções» e «sem teto de duração» caducaram), #190 (194 → 119 e a frase dos assistentes que morreu medida), CONCORRENCIA e CATRACAS (a régua conta ATRIBUIÇÃO), DESEMPENHO (o achado do `empilhar` e a hipótese morta), TESTES §13, CHANGELOG, MODELOS, e dois pedidos novos | médio | varredura verificável de commits que já existem; nenhum número de memória | 06:25 |
| 303-F | o teste do `TETO_DA_RESPOSTA`, o lado que RECEBE — decisão do dono das 05:32. A QA disse que não há teste que caia, e guarda sem teste que caia não é guarda. Se o teto estiver no lugar errado, como o irmão estava, é parecer e não conserto | forte | prova real é o papel mais fácil de fingir que se cumpriu | 06:26 |

Fila depois: **164-F** (medição em máquina parada + mapa remedido), **F2**
(bancada: 308 datas nos `resultados.json`, 193, 306, 307), **onda 4 do
Phoenix** (304 dataBar no XLSX, 305 construtor AND/OR), e o fecho — com o
**provador INTEIRO** rodando, que é decisão do dono no 263.

Na mesa do dono, esperando: a régua do mapa da trava em **25 com teto 22**
sem nenhum `fsync` novo (dividir a porta mudou a atribuição), o `ler` dentro
de transação pagando O(pendentes) sob a trava (38 µs com zero, **1.118,50 µs
com 1.600**), e a versão que o `sondar_origem` não devolve e a tela mostra
como buraco.

## Onda 6 — as cinco decisoes de formato sairam (17/09/2026 07:10 UTC)

O dono decidiu 289, 290, 292, 293 e 294, as cinco que destravam o 229. Uma
delas derrubou a proposta do proprio parecer: a 290 pedia `inicio` e `passo`
no bloco de esquema, e `abrir_para_replicar` (`servidor.rs:2806-2814`) cria a
tabela do MESMO bloco de esquema do source, byte a byte — os dois `u64`
chegariam iguais nos dois nos e as faixas voltariam a colidir. Ficou: `passo`
no `PSCH`, `inicio` na identidade do no. Medido na hora da decisao, antes de
virar contrato.

Os cinco pedidos ficam em estado **planejado** de proposito: a decisao saiu, o
formato nao mudou. Marcar feito por causa de uma decisao seria a mentira que
esta casa passa o tempo todo consertando.

| frente | contrato | escalao | motivo do escalao | largada |
|---|---|---|---|---|
| C-PSCH | desenho do `PSCH` v10 byte a byte: 289 (nanos com avanco forcado), 290 (passo no esquema, inicio no no) e os itens de formato do 229, num bump so. Migracao, o que a replica honra, e o que C recusa se recusar | forte | formato em disco e a definicao de projeto e risco | 07:19 |
| B-RECUSAS | 292 e 293: as duas recusas na declaracao, e o laco infinito que para de repetir o mesmo lote contando e gritando como o `colisao_de_criacao` ja faz | forte | muda o que o motor aceita replicar, e uma delas hoje grava dado errado calado | 07:19 |

**Papeis convocados e dispensados nesta onda** (a clausula cobra a diferenca
entre dispensa registrada e esquecimento):

- **Convocados**: A (integrador, esta linha), C (formato em disco — e dele a
  palavra), B (as duas recusas), F por dentro do contrato de B (prova real nos
  dois sentidos, com o vermelho medido e dito).
- **Dispensados, com motivo**: **E** (designer) — nenhuma das cinco decisoes
  toca a tela nesta onda; a coluna de data/hora vai mexer na grade quando
  existir, e ai ele entra, porque «coluna de sistema nova quebra quem filtra
  pela primeira» ja foi pago tres vezes aqui. **G** (QA) — entra na onda
  seguinte, para catalogar as guardas do 164 e do 303 e as que estas duas
  frentes criarem; catalogar guarda de codigo que ainda nao existe seria
  catalogo de promessa. **H** (documentacao) — entra no fecho, junto do resto.
  **J** (pesquisador) — as cinco decisoes ja estavam medidas contra o nosso
  gargalo nos pareceres; nao ha receita de fora nova para trazer. **D**
  (zelador) — rodou as 07:16, liberou 1.264 MiB medidos. **SEC** — o 293 e
  achado dele e ja esta com o parecer escrito; ele volta para revisar a recusa
  depois de implementada, nao antes.

### Correcao do dono, 07:2x — as cinco recomendacoes sairam sem a regua dos motores

Ordem dele: *«Suas recomendacoes devem levar em conta o que o Mariadb e o
PostgreSQL faz.»* E ele esta certo. Eu recomendei as cinco contra o nosso fonte
e contra as nossas petreas, e nao passei nenhuma pela lei que esta casa tem
justamente para isto: tres motores maduros convergindo e aceite automatico, e
onde nao convergem decide a media ponderada (PostgreSQL 4, MariaDB 3, MySQL 2,
SQLite 1).

O alcance que eu errei: eu lia essa regua como uma peneira para **receita que
vem de fora**, e ela vale tambem para **decisao nossa sobre o que o banco faz**
— que e exatamente o que as cinco sao. Semantica de carimbo de tempo, faixa de
sequencia, conflito de unicidade, cifra em repouso e criterio de eleicao sao
todos «o que o banco faz».

| frente | contrato | escalao | motivo do escalao | largada |
|---|---|---|---|---|
| J-REGUA | as cinco decisoes contra PostgreSQL, MariaDB, MySQL e SQLite, na fonte primaria; a conta da convergencia ou da media ponderada; e o veredito por decisao: confirma, muda, ou choca com petrea (e o choque APARECE) | forte | e a lei que decide se quatro frentes constroem a coisa certa | 07:2x |

**Frentes seguradas enquanto a regua nao volta**, porque construir a guarda
errada custa mais que esperar:

- **B-RECUSAS**: 293 parado e a recusa da tabela do 292 parada. Segue so a
  parte 2 do 292, parar o laco infinito, que esta certa sob qualquer das tres
  saidas — nenhum motor maduro fica repetindo o mesmo lote para sempre, entao
  isso e defeito nosso sob qualquer regua.
- **C-PSCH**: segue, com uma secao a mais. A hipotese que o pesquisador esta
  medindo e que os maduros **nao** dao carimbo unico por linha de proposito: o
  `CURRENT_TIMESTAMP` do PostgreSQL e constante na transacao inteira, a ordem
  mora no `xmin` e no LSN, e o MariaDB oferece versionamento por TRANSACTION
  como alternativa ao por TIMESTAMP. Se for por empate, os maduros poem a ordem
  num contador e deixam o relogio para leitura humana — e o desenho certo vira
  duas coisas gravadas, nao uma. C desenha as duas com o preco em bytes.

O que eu espero que a regua **confirme**: a 290, porque `auto_increment_offset`
e `auto_increment_increment` do MySQL e do MariaDB sao variaveis de servidor e
nao campo do esquema, que e a nossa decisao; e a 294, porque `seqno` do Galera
e LSN do WAL sao escalares globais, nao vetores por tabela. Esperanca nao e
medicao: fica escrito aqui para que o retorno do J possa me desmentir.

## Onda 7 — a regua dos motores derrubou duas das cinco (17/09/2026 07:52)

Papel J foi ao **fonte alheio**, nao ao manual
(`docs/propostas/regua-dos-motores-decisoes-289-294-2026-09-17.md`). O que voltou,
e as tres decisoes novas do dono:

| # | veredito | o que o dono decidiu depois |
|---|---|---|
| 289 | **MUDA o meio** — nenhum dos tres forca relogio a avancar | duas colunas, 16 bytes: contador que ordena, relogio que pode empatar |
| 290 | **CONFIRMA** — o Galera faz `auto_increment_offset = own_index()+1` | mantida |
| 292 | **(1) CHOCA** — nenhum dos tres recusa a tabela | parada visivel do par no lugar da recusa |
| 293 | os tres **decifram antes de mandar** | recusa fica; a ideia do UUID como sal foi ao SEC |
| 294 | **CONFIRMA** — LSN, `seqno` e peso+UUID sao todos escalares | mantida |

**O erro que eu cometi e que motivou a ordem do dono**: recomendei as cinco sem
passar pela regua, porque eu a lia como peneira para receita de FORA. Ela vale
tambem para decisao nossa sobre **o que o banco faz**.

**E um erro de fato dentro da minha propria justificativa do 292**: escrevi que
«nao ha ninguem funcionando», e isso e falso para o par que nao colide e para todo
unidirecional, onde so um lado escreve. A petrea «guarda nova entra pedida, nao
imposta» estava batendo de frente, e eu a afastei com argumento que nao passa pela
medicao.

## Onda 8 — o SEC derrubou a ideia do dono, e o fato dele estava certo (08:02)

Veredito **NAO ENTRA** (`docs/propostas/parecer-sec-uuid-como-sal-2026-09-17.md`).
O `id` da coluna e mesmo um `Uuid::v7()` que chega identico na replica — e e
identico **pelo motivo errado para o papel de sal**: publico, escolhivel e
previsivel. Quatro bloqueadores, os quatro reconferidos no fonte pelo integrador
antes de aceitar; o quarto **nao se conserta**, porque e o comprimento do UUID:
**62 bits aleatorios onde o NIST manda 128**, com `shall`.

**O que sobrevive da intuicao do dono, e e o que importa**: existe mesmo um caminho
que replica identico, e o que deve viajar por ele **nao e o sal, e a chave
envelopada** — o envelope da §11.5.

| frente | escalao | motivo do escalao | resultado |
|---|---|---|---|
| J-REGUA | forte | e a lei que decide se quatro frentes constroem a coisa certa | 5 vereditos, fonte primaria em cada um |
| SEC-UUID | forte | criptografia | NAO ENTRA, com 4 bloqueadores medidos |
| B-292-PARTE-2 | forte | semantica de replicacao | laco infinito acabou, guarda PROVADA 1/1 |
| G-CATALOGO | **leve** | catalogar guarda que ja existe e mecanico e verificavel | 3 entradas, as tres provadas |

**A dispensa do escalao leve se pagou, e com juros**: a QA nao so catalogou como
**corrigiu a razao que eu tinha dado** para nao estender a guarda ao soquete. Eu
disse «pendura 15,4 s». Ela mediu e achou a causa de verdade, que e pior: com
aquele defeito reposto **um dos dois testes de soquete fica VERDE**, porque a
conferencia de quanto foi lido acontece depois de ler os 129 MiB. Veredito certo
com a memoria ja gasta — a propria lei escrita naquela entrada, aparecendo do
outro lado.

## O que o ambiente cobrou nesta rodada, e o que ele ensinou

Tres coisas que nao eram do produto e que consumiram tempo real:

- **O backup estava 45 commits atras por uma premissa.** «Refaz no fecho, senao
  compete com quem compila» nunca foi medido. Sao **15,5 s**, porque o script e
  `git bundle` e nao compila nada. Diagnostico plausivel nao e diagnostico medido,
  agora aplicado a um papel em vez de a um numero. `docs/BACKUP.md`.
- **O disco caiu duas vezes na mesma hora**, a segunda para 986 MiB com frente
  compilando. Tres buracos, todos medidos: o `target/debug/incremental` (2,8 GiB),
  o cache do provador de guardas (2,9 GiB, que o zelador so apaga frio e uma
  corrida deixa quente por meia hora) e o `/tmp` (4,4 GiB, fora do alcance dele).
  Pedido **317**.
- **O CHANGELOG ficou com o numero que eu corrigi de madrugada.** Conserto entra no
  caminho que o motivou e o **irmao fica** — e foi do irmao que a frente seguinte
  copiou. A lei da casa virada contra quem a escreveu.

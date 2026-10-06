# Tecnologias do PhxClaw

Isto não é o `README` (como usar) nem o `GUIA_DO_OPERADOR` (o que configurar): é o inventário
do que se usou **para fazer o produto e para fazer o trabalho** — as duas metades, porque a
segunda é a que se reaproveita e a que ninguém escreve.

Regra que o rege: **todo número visível sai de um gerador, ou está errado e ninguém percebeu
ainda.** Os blocos entre `<!-- gerado:tec:… -->` saem de

```bash
python3 tools/gerar_tecnologias.py
```

(só biblioteca padrão do Python 3; a lista do que fica fora da conta é de **pastas** —
`target`, `node_modules`, `third_party`, `private` — e não de arquivos, para não envelhecer).
Rodar de novo reproduz as tabelas contra a árvore do momento; o carimbo diz sobre qual commit
e com quantos arquivos ainda não comitados — porque nesta casa há frentes editando em
paralelo, e um número medido com outra frente a meio caminho é número com data, não número
errado. Fora dos blocos, cada número cita o documento ou o comando de onde saiu.

<!-- gerado:tec:carimbo:inicio -->
Medido em 2026-10-06 na arvore de trabalho sobre o commit `610fd82c` (branch `phxclaw/rascunho-equipe-20261002`), por `python3 tools/gerar_tecnologias.py`. Arquivos modificados e nao comitados no momento da medida: 3.
<!-- gerado:tec:carimbo:fim -->

## 1. Linguagens e volume, contados

<!-- gerado:tec:linguagens:inicio -->
| linguagem | arquivos | linhas | onde (pastas com mais linhas) |
|---|---:|---:|---|
| `.rs` | 381 | 153621 | `crates` 146648, `apps` 6895, `sdk` 78 |
| `.js` | 13 | 13513 | `apps` 13302, `tests` 203, `integracoes` 8 |
| `.mjs` | 16 | 3322 | `tests` 3318, `apps` 4 |
| `.py` | 175 | 12902 | `tools` 8029, `scripts` 2887, `sdk` 768 |
| `.ts` | 4 | 375 | `integracoes` 356, `integrations` 19 |
| `.html` | 29 | 1440 | `docs` 779, `apps` 373, `ui` 194 |
| `.css` | 6 | 1231 | `apps` 1225, `crates` 6 |
| `.sh` | 33 | 960 | `tools` 553, `ci` 147, `scripts` 141 |
| **total** | **657** | **187364** | |
<!-- gerado:tec:linguagens:fim -->

Leitura do retrato: o produto é Rust (`crates/` + `apps/phxclaw`); o JavaScript é a interface
(`apps/phxclaw-ui`, sem framework nem bundler) e os roteiros de prova no Chromium
(`tests/desktop/*.mjs`); o Python é ferramenta de trabalho (`tools/`, `scripts/`), nunca
produto; o TypeScript é um pacote só, o nó da comunidade do n8n (`integracoes/`), fora do
workspace Rust.

### 1.1 Rust

<!-- gerado:tec:rust:inicio -->
- Rust: **122565** linhas fora de `tests/` e `examples/`, **30514** em `tests/` de integracao, **542** em `examples/`; proporcao teste/codigo (so integracao) 30514/122565 = 0.25x.
- **118** crates em `crates/` + os binarios em `apps/`.
- **778** funcoes marcadas `#[test]`/`#[tokio::test]`.
- **39** dependencias diretas no `[workspace.dependencies]` do `Cargo.toml`: `anyhow`, `aes-gcm`, `secrecy`, `zeroize`, `axum`, `futures-util`, `getrandom`, `tokio-stream`, `base64`, `chrono`, `csv`, `ed25519-dalek`, `enigo`, `postgres`, `quick-xml`, `reqwest`, `semver`, `serde`, `serde_json`, `sha2`, `thiserror`, `tokio`, `url`, `rustls`, `rustls-pki-types`, `webpki-roots`, `tokio-tungstenite`, `uuid`, `xcap`, `walkdir`, `tree-sitter`, `tree-sitter-rust`, `tree-sitter-python`, `tree-sitter-javascript`, `tree-sitter-typescript`, `tree-sitter-go`, `tree-sitter-c`, `tree-sitter-cpp`, `tree-sitter-java`.
- **849** pacotes no `Cargo.lock` (a arvore inteira, transitivas incluidas).
<!-- gerado:tec:rust:fim -->

## 2. Dependências, e o que a escolha comprou ou custou

O PhxClaw **não** segue a pétrea de zero dependências do PhxSql: ele é um agente que fala
HTTP/TLS, WebSocket, Postgres, XML, e analisa código em oito linguagens (as oito gramáticas `tree-sitter-*` de 1.1) — a lista direta está
em 1.1 e a árvore inteira no `Cargo.lock`. O que a casa guarda em vez de «zero» é **«nenhuma
crate nova para o que já se faz com uma que existe»**, e o custo aparece medido quando se paga:

- **HMAC-SHA256 e HMAC-SHA1 escritos sobre o `sha2` que já existia** (`canais/cripto.rs`), em
  vez de puxar `hmac` + `sha1` «para quinze linhas»; conferidos contra RFC 4231, RFC 2202 e
  FIPS 180 (§3).
- **`quick-xml` está na árvore, mas o XMPP não o usa como analisador de fluxo**: o fluxo XML
  do XMPP só termina quando a conexão cai, e o analisador quer o documento inteiro. O canal
  recorta estrofes (`<message>…</message>`) à mão (`canais/xmpp.rs`, doc do módulo) — a
  dependência existe para outro uso e não resolveu este.
- **Zero crate para o motor de fluxo** (`fluxos.rs`): grafo e fila são o `phxclaw-task-graph`
  da própria casa; expressão é caminho JSON sobre `serde_json`, sem motor de expressão nem
  sandbox de JS (`docs/N8N.md` §8c).
- **O n8n inteiro não cabe nesta máquina** (`docs/N8N.md` §6): `npm install n8n@2.41.5` chegou
  a 2.162 MB em `node_modules` + 1.245 MB de cache, ainda incompleto, com o disco em 436 MB
  livres. Foi apagado. É o número que decide «integrar pelos padrões, não embutir».
- **O nó da comunidade do n8n** custa 96 pacotes npm, 87 MB, 6 s — só em desenvolvimento, só
  pelos tipos (`n8n-workflow` é Sustainable Use License e não entra no binário).

## 3. O que foi escrito à mão, e as normas conferidas

| O quê | Onde | Norma / vetor |
|---|---|---|
| HMAC-SHA256, HMAC-SHA1 | `crates/phxclaw-agent/src/canais/cripto.rs` | RFC 4231, RFC 2202, FIPS 180 |
| Cliente XMPP: SASL PLAIN, bind, presença, STARTTLS, MUC | `crates/phxclaw-agent/src/canais/xmpp.rs` | RFC 6120 §5/§6.4.6, XEP-0045 §7.2/§7.4, XEP-0199, XEP-0203 |
| Recorte de estrofe XML por fluxo | idem | — (decisão nossa; §2) |
| Motor de fluxo por itens: `se`, `juntar`, `lote`, `parar_com_erro`, `ao_errar`, `fluxo_de_erro`, `teto_ms`, expressões por caminho JSON | `crates/phxclaw-agent/src/fluxos.rs` | inspiração no n8n 2.41.5 (`workflow/src/interfaces.ts`, `workflow-execute.ts`); as 7 divergências e a restrição de cada uma em `docs/N8N.md` §8c |
| Assinatura de webhook com carimbo e janela de 5 min | `canais/webhook.rs`, `n8n.rs`, `gatilhos.rs` | a MESMA função nos três sentidos (lei: função não se duplica) |
| Streamable HTTP do MCP em `POST /mcp` | `crates/phxclaw-agent/src/mcp.rs` | especificação MCP (sessão por `Mcp-Session-Id`, 202 para notificação, 405 em `GET`) |

## 4. As ferramentas do trabalho

**Geradores** — o que escreve número em documento ou página; nenhum número visível se digita:

<!-- gerado:tec:geradores:inicio -->
| gerador | o que escreve |
|---|---|
| `docs/absorcao/gerar_absorcao.py` | Quanto o PhxClaw absorveu de cada fonte, medido capacidade por capacidade. |
| `tools/dossie/dossie_da_pasta.py` | Acha o dossie do PhxClaw: UM dono so, por varredura, nunca por nome digitado. |
| `tools/dossie/embutir.py` | Fontes e imagens da marca dentro do HTML, como data URI. |
| `tools/dossie/gerar_dossie.py` | Dossie do PhxClaw: a pagina INTEIRA sai daqui; nenhum numero visivel e digitado. |
| `tools/dossie/numerar_figuras.py` | Numera TODAS as figuras do dossie na ordem do documento, e as referencias a elas. |
| `tools/dossie/numeros.py` | Leitores do dossie do PhxClaw: cada numero visivel sai daqui, com a FONTE e a DATA. |
| `tools/gerar_doc_agente.py` | Gera os trechos medidos do docs/AGENTE_AUTONOMO.md -- nenhuma lista de ferramenta, de |
| `tools/gerar_guia_operador.py` | Gera os trechos medidos do docs/GUIA_DO_OPERADOR.md -- nenhum comando, opcao, variavel de |
| `tools/gerar_tecnologias.py` | Regrava os blocos medidos de `docs/TECNOLOGIAS.md` (entre `<!-- gerado:tec:NOME:inicio/fim -->`). |
<!-- gerado:tec:geradores:fim -->

Mais os que vivem fora do padrão de nome: `tools/suite.sh` (roda o `cargo test --no-fail-fast`
e grava a saída que o dossiê lê; sai 1 se falhou, 2 se pulou recurso que a máquina tem) e os
`tests/desktop/*.mjs` (Chromium via Playwright: `ui_navegacao.mjs`, `ui_config.mjs`,
`qualificar.mjs`, `textos_fora_da_fabrica.mjs` com teto 0), que são a única prova da tela —
**interface só se prova exercitando**: os três defeitos do L1 da casca só apareceram no
navegador (`docs/ui/COMPARACAO_MOCKUP_2026-10-02.md` §5).

**Como se orquestrou nesta rodada (02/10/2026):** três frentes em paralelo sobre a mesma
árvore — engenheiro (`fluxos.rs`, `agenda.rs`, `gatilhos.rs`, `apps/phxclaw`), designer
(`apps/phxclaw-ui/`) e documentação (`docs/`) — com a lista de arquivos de cada uma declarada
antes de começar, para o merge não escolher o lado de quem não tinha a seção. O `cargo` ficou
proibido para a documentação porque o disco (~1,2 GB compartilhado) não comporta dois
`target/`; por isso o que esta rodada diz do código foi lido em `git show HEAD:…`, nunca
compilado por esta frente, e o documento diz isso onde vale (`docs/N8N.md` §8b).

**Como se mede a absorção:** `python3 docs/absorcao/gerar_absorcao.py` lê `fontes.json` e
`phxclaw.json` e imprime a cobertura por fonte (sim / pela metade / não); o número do n8n
desta rodada (55,9% no agente, de 40,7%) saiu dele, não da previsão (+15,3 pp previstos,
+15,2 medidos).

## 5. O que foi avaliado e RECUSADO, com o número

Recusa medida impede a mesma proposta de voltar. Fonte: `docs/absorcao/TRIAGEM_PESQUISAS_2026-10-01.md`
(tabela R1–R24) e `docs/absorcao/SPRINTS.md` («Hipóteses que morreram, com o número»).

| Proposta | Decisão | O número / o motivo |
|---|---|---|
| **Embutir o n8n** (R12) | recusado | Sustainable Use License proíbe embutir em produto; e a instalação não cabe: 2.162 MB + 1.245 MB com 436 MB livres (`N8N.md` §6) |
| **xyflow** como editor de fluxo (R20) | recusado, `⏸` | editor de fluxo não pedido; e desenhar sobre um motor de string seria desenhar o que ia mudar — o motor de itens veio primeiro (onda 1), o editor é a onda 5, em SVG próprio como o UI-IR já faz |
| **Copiar o `pairedItem` do n8n** | hipótese morta | o n8n precisa dele porque o nó reordena/filtra itens dentro do mesmo nó (`interfaces.ts:1854`, `:1873`); aqui cada passo é uma chamada pelo portão com evidência própria no ledger. Entra só se um `juntar` por campo pedir — o da onda 1 não pediu (`N8N.md` §8d) |
| **JavaScript nas expressões do fluxo** | recusado | segunda sandbox para código do operador seria segunda política; expressão é caminho JSON (`N8N.md` §8c) |
| **Queue mode com Redis** | hipótese morta | as peças de claim já existem em Postgres (`PostgresTaskJournal`, `claim_batch`, `claim_ready_token`) e nenhuma é usada pelo agente; raciocinado, não medido — a bancada decide (onda 4) |
| **robotframework** (R13) | recusado | runner Python; as provas da casa são Rust + Chromium |
| **browser-use** (R17) | recusado | duplica `browser_*` que já existe |
| **refine** (R19) | recusado | duplica o CRUD gerado do `react.rs` |
| **docling** (R22) | recusado | arrasta torch + modelos de layout; `read_document` + OCR já cobrem |
| **graphiti** (R24) | recusado | exige Neo4j/FalkorDB |
| **Filtro por ocupante dentro da sala XMPP** | não decidido | é produto: sobe ao dono. Até lá, sala em `PERMITIDOS` = qualquer ocupante comanda o agente (`GUIA_DO_OPERADOR.md`, «Canal XMPP») |

## 6. O que este documento NÃO conseguiu medir nesta rodada

- Nada de `cargo` (disco): o total de testes verdes (450 / 0 / 1 ignorado) é o parecer do
  integrador na sprint `docs/sprints/Sessao_00001_Sprint_SP000035_20261002060631.md`, rodado
  um alvo por vez, não uma corrida desta frente.
- O exemplo de fluxo do `N8N.md` §8b foi conferido contra `ler`/`validar` por leitura, não
  executado.
- Nenhum servidor MUC real exercitou a sala XMPP; só o servidor falso dos testes.
- O `CHANGELOG.md` não tem entradas v0.68 e v0.69 (`grep -c "v0.68\|v0.69" CHANGELOG.md` = 0);
  esta rodada acrescentou a v0.70 e não inventou as duas que faltam.

## Como se refaz

```bash
python3 tools/gerar_tecnologias.py        # regrava os blocos medidos deste arquivo
python3 docs/absorcao/gerar_absorcao.py   # o número de absorção citado em §4
```

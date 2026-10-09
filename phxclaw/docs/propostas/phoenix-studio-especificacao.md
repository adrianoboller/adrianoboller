# Phoenix Studio — especificação e backlog (fase 1 da fábrica)

**Papel J, 09/10/2026.** Ordem do dono do mesmo dia: o PhxClaw constrói o Phoenix Studio como
fábrica — **especificação e backlog** (esta fase), depois a ponte com o compilador Phoenix
(Octopus), e a auto-evolução (SP000015) passa a propor itens do Studio. Nada aqui está
implementado nem comitado.

Legenda de toda afirmação: **[fonte]** = lido no arquivo citado · **[medido]** = rodado nesta
sessão, com o comando · **[hipótese]** = raciocinado, não medido, com o que decidiria na bancada.

Medido nesta sessão (scratchpad `j-studio/`, alvo de compilação próprio, `nice -n 19`):

| Medida | Comando | Resultado |
|---|---|---|
| gerador do UI-IR a partir de SQL | `cargo run -p phxclaw-ui-ir --example gerar -- tests/fixtures/pedidos.sql Vendas …` | **19 arquivos** (html, react, flutter, rust, wlanguage, `ui-ir.json`) em 37 s de primeira compilação |
| o crate Rust gerado compila | `cargo build --offline` na pasta `rust/` gerada | compila; **0 avisos de clippy — porque a linha 4 é `#![allow(clippy::all)]`**; tirada a linha, **1 aviso** (`manual_range_contains`) |
| `ON DELETE CASCADE` na origem | DDL de duas tabelas com `ON DELETE CASCADE ON UPDATE CASCADE` | sai **restringir** («exclua-os antes») com **0 avisos**; `cascade` some do `ui-ir.json` (0 ocorrências) |
| o laço de espera da ponte Octopus | réplica std-only do `invoke` (`crates/phxclaw-octopus-bridge/src/lib.rs:102-153`) | ≤ 65.536 bytes de saída: sai em 10 ms; **65.537 bytes: TIMEOUT** (filho preso no `write` do pipe); o teto declarado de 8 MiB é inalcançável |
| `octopus-console` nesta máquina | `which octopus-console` | **ausente** |
| testes da ponte e do plugin Octopus | `grep -c '#[test]'` | **0** e **0** |

---

## 1. O que é o Phoenix Studio

**Para quem.** Quem tem aplicação em Clarion® ou WinDev®/WLanguage e quer levá-la para Rust sem
reescrever à mão — o projeto Phoenix do dono, «Clarion→Rust migration»
[fonte: `base-de-conhecimento/01-PEDIDOS.md:485`, `:740`] —, e quem quer criar aplicação nova no
mesmo molde RAD.

**Que problema.** O Phoenix converte (compilador/gerador «Octopus»); o Studio é onde a pessoa
**vê, corrige e confere** o que foi convertido e cria o que falta: dicionário de dados, janelas,
consultas, relatórios, código, e o botão que gera e prova que compila. Sem ele, a conversão é uma
caixa-preta de linha de comando [hipótese de produto, coerente com o agente 038 «LLM só resolve
bordas; transformações determinísticas permanecem no compiler», `config/agents/038-octopus-agent.agent.json`].

**O que entrega.** Um projeto Phoenix (envelope PHX JSON com o UI-IR dentro — decisão já tomada,
`docs/ui/STUDIO_UI_R03.md`, linha «PHX JSON × UI-IR») que gera, do **mesmo modelo**, telas
(HTML/Bootstrap/React/Flutter) e regras de negócio em Rust e em WLanguage com os mesmos nomes e
mensagens [fonte: `crates/phxclaw-ui-ir/src/lib.rs:1-6`, `src/rust.rs:1-6`, `src/wlanguage.rs:1-11`].

**Requisitos do dono que já existem por escrito** (não são hipótese):

| Requisito | Fonte |
|---|---|
| Studio como aba da interface web do PhxClaw, inspetor phx-grid, prévias em iframe | `docs/ui/STUDIO_UI_R03.md` (papel J, 01/10) |
| Editor de tabelas, campos e relacionamentos «similar ao Analysis do WinDev / DbSchema / pgModeler» | `01-PEDIDOS.md:3042` |
| **MULTISCREEN «obrigatório no Phoenix»**: Designer no monitor 1, Código no 2, Banco/SQL no 3, Debug/IA no 4; memorizar posição/monitor; DPI por monitor | `01-PEDIDOS.md:4536-4549` (pedido 127, 29/08) |
| Barra de dados no XLSX e LLM local atravessaram as 3 gerações do Query Designer do dono | `phxsql/docs/propostas/phoenix-query-designer-2026-09-17.md` §0 |
| Agente 034 «UI/RAD»: Window/Screen/Panel/Button/Grid/Form, inspector bindings, templates; «não codifica texto internacionalizável fixo» | `config/agents/034-ui-rad-agent.agent.json` |
| Style Phoenix Padrão (telas do PhxSql, multitela 1–4 regiões, cores de ação) | `docs/ui/STYLE_PHOENIX_PADRAO.md` |

---

## 2. Módulos: o que existe, o que falta, aceite testável

Estado: **0 módulos completos, 7 parciais, 1 ausente.** «No binário» = alcançável pelo `phxclaw`
(`apps/phxclaw/Cargo.toml` → `phxclaw-agent`, que depende do `phxclaw-ui-ir`, `Cargo.toml:30`).

| # | Módulo | Já existe (arquivo · no binário?) | Falta | Aceite testável |
|---|---|---|---|---|
| M1 | **Designer de janelas** | UI-IR v3 (`phxclaw-ui-ir/src/ir.rs`, 480 linhas; List/Form/MasterDetail; intenção de layout, `responsivo.rs`); ferramentas `design_erp_ui` e `screenshot_to_erp_ui` (`phxclaw-agent/src/ui.rs:17`, `:337`) · **sim**. Seis adaptadores; 45 `#[test]` no crate | a **tela** (não há aba Studio: as 7 telas do PWA são tarefas, fluxos, ide, agentes, ferramentas, absorcao, config); `id` UUIDv7 na `Section` (IR v4, lote L1 do R03); arrastar/inspetor | arrastar campo entre seções muda **ordem e grupo** no IR e nunca grava `col-*`/HTML; DOM = motor (`responsivo::explicar`) nas larguras 575/576; dois temas; catraca de idiomas não sobe |
| M2 | **Designer de relatórios** | escritor OOXML `.docx/.xlsx/.pptx` determinístico (`phxclaw-office/src/lib.rs`) · sim | o artefato `relatorio` (bandas título/grupo/detalhe/rodapé/sumário, mestre-detalhe); **PDF: 0 escritores** no repositório (`grep '%PDF-1'` só acha testes de tipo) | mesmo projeto → mesmo `.docx` byte a byte (hash); subtotal por grupo confere com soma recalculada no teste |
| M3 | **Dicionário de dados com integridade** | DDL → entidades/campos/chaves (`schema.rs`, 817 linhas); regras com **restringir, nunca cascata** (`regras.rs:13`) | edição no modelo; **IR → DDL** de volta; **aviso quando a origem pede cascata** (hoje 0 avisos, medido); importador Clarion `.txd` e Analysis WinDev (0 parsers: `grep -ril clarion crates` só acha comentários) | ida e volta SQL→IR→SQL idêntica nos 4 casos do `pedidos.sql`; `ON DELETE CASCADE` gera aviso nomeando tabela e coluna; FK sem índice dos dois lados é recusada **na declaração** |
| M4 | **Editor de código WLanguage/Clarion com diagnóstico** | IDE web: Helix num PTY no bwrap (`ide.rs`, 784 linhas), LSP só-leitura (`lsp.rs`, 952), completar por IA (`phxclaw-snippet-ls`) · sim | Helix 25.07.1 daqui configura **9 linguagens, nenhuma Clarion/WLanguage** (`/opt/helix/languages.toml`); `snippet-ls` não publica diagnóstico (0 ocorrências de `diagnostic`) | `.wl` aberto mostra aviso para função fora de `FUNCOES_CONFERIDAS` (`wlanguage.rs:19-32`) com a página do Help; o aviso some ao trocar por função conferida |
| M5 | **Query designer** | nada no PhxClaw; no PhxSql, tela de 1 condição e motor que aceita expressão inteira (`phoenix-query-designer-2026-09-17.md` §1.1) | artefato `consulta` com N filtros AND/OR e **parâmetro posicional** — o valor nunca entra no texto | valor `x' OR '1'='1` vira parâmetro, não texto (teste adversarial, que a crate do dono não tinha: §5.1 do parecer) |
| M6 | **Gerador / compilador via Octopus** | gerador próprio (ui-ir → Rust/WL/telas) · sim; ferramenta `rust_project` (cargo check/test/clippy no bwrap, JSON de diagnóstico, `sistema.rs:1961-2011`) · sim; ponte Octopus (`phxclaw-octopus-bridge`, 324 linhas) · **não** (só `phxclaw-octopus-plugin` e o workspace a usam) | ver §3.2 | ver §3.2 |
| M7 | **Depurador / execução** | `debug` por DAP com `gdb -i dap` e `debugpy` no bwrap (`dap.rs`, 789) · sim | ligar ao projeto Studio: rodar os testes do crate gerado e parar numa regra | breakpoint em `incluir_pedido_item` do crate gerado para e lê a variável dentro do bwrap |
| M8 | **Projeto / versão** | git no bwrap, `git_worktree`, revisão sha256 + `If-Match` no padrão da tela Fluxos (`fluxos_tela.rs:17-19`) · sim | envelope `studio-projeto`; publicar = commit; diff por UUID | gravação concorrente de duas abas dá 409 com a revisão atual; publicar não regrava versão publicada |
| M9 | **IA assistente** | `design_erp_ui`/`screenshot_to_erp_ui`; tela→UI-IR por OCR com prova de fidelidade (SP000021/22) | a IA propõe **patch do IR por UUID**, validado pelo motor antes de aparecer | patch inválido nunca chega à tela (recusado pelo `de_json`+`validar`); aplicar exige clique |

---

## 3. Arquitetura proposta

### 3.1 Studio no mesmo binário ou separado — hipóteses e veredito

| H | Hipótese | Rival | Veredito, com o número |
|---|---|---|---|
| H1 | Studio como app separado (Tauri+React próprio, como sugere `estudo/projetos_rust_para_phoenix_2026-10-09.md` §1) | **aba do `phxclaw servir`**, telas no mesmo PWA | **Rival vive.** O desktop já carrega o MESMO PWA (`apps/phxclaw-desktop/src-tauri/tauri.conf.json:6`, `"frontendDist": "../../phxclaw-ui"`); o `phxclaw-ui-ir` já está linkado no binário (`phxclaw-agent/Cargo.toml:30`), custo marginal de código-motor ≈ 0; autenticação, RBAC (`rbac.rs`), matriz de rotas cruzada nos dois sentidos (`rotas.rs:1-8`) e fábrica de idiomas seriam a **segunda cópia** num app separado — é a lei «função e comando vêm do mesmo motor». Decisão do R03 confirmada |
| H2 | O Octopus entra no binário | **processo externo** pela ponte | **Rival vive, por licença:** workspace Octopus `Proprietary` × PhxClaw `Apache-2.0` (`docs/OCTOPUS_REUSE_AUDIT_V09.md` §«Regra de licença»; `Cargo.toml:138`) |
| H3 | A ponte é o caminho para «gerar e conferir que compila» | o oráculo já existe sem Octopus | **Rival vive.** As 11 capacidades da ponte (`lib.rs:76-86`) **nenhuma gera nem compila** (`toolchain.status` só relata); o crate gerado pelo ui-ir compila hoje (medido). O Octopus traz o que o PhxClaw não tem — **frontends legados** (Clarion/WL → IR) — e não o portão de compilação |
| H4 | A ponte de hoje aguenta a saída real do Octopus | trava acima de 64 KiB | **Hipótese morreu, medido:** 65.537 bytes → timeout. O mesmo padrão em **7 lugares**: as pontes octopus, openclaw-rs, claw-code, rustclaw, `system-automation`, `media-intelligence` (só stderr) e o `ProcessRunner` do `phxclaw-process-protocol/src/lib.rs:161-168`. O conserto **já existe** no `phxclaw-sandbox/src/lib.rs:399-400` («ler so no fim trava o filho quando ele enche o pipe (64 KiB) e vira timeout falso») — vem desse motor, não de um sétimo remendo |
| H5 | «0 avisos de clippy» no crate gerado prova qualidade | é vazio | **Morreu:** `#![allow(clippy::all)]` esconde 1 aviso (medido) |
| H6 | O editor WL/Clarion sai de graça do Helix | precisa de léxico próprio | **Rival vive:** 0 gramáticas Clarion/WL configuradas; 0 parsers Clarion no repositório. Primeira fatia: diagnóstico pela lista conferida do Help (M4), sem gramática |

**Desenho:**

```text
PWA (navegador e Tauri, o mesmo)          phxclaw servir (Apache-2.0, um binário)
 aba Studio ──/v1/studio/*──▶ rotas.rs + rbac.rs ──▶ phxclaw-ui-ir (IR, dicionário, geradores)
   designer · dicionário · consulta          │            └─ rust_project / dap / lsp (bwrap)
   código (Helix) · relatório · IA           └──▶ extension-host ──processo──▶ octopus-plugin
                                                                  └─▶ octopus-console (Proprietary, fora)
```

Tecnologia de tela: **segue a decisão de `docs/propostas/ui-wasm-2026-10.md`, que ainda não
existe**; a bancada está rodando agora (leptos, dioxus, yew em `scratchpad/bench`). Até ela, as
telas seguem o padrão vigente — JS sem framework, SVG à mão como `fluxos.js` (905 linhas, xyflow
recusado na R20). Só PS0009 (designer) e PS0020 (multitela) têm portão nessa decisão.

**Divergências da origem, com a restrição nossa que as causou:**

1. Estudo dos 68 projetos → Studio React/Tauri separado, SQLx, wasmtime, Rhai. **Aqui:** aba do
   servidor, sem React, sem segunda linguagem. *Restrição: função e comando do mesmo motor
   (RBAC/idiomas/rotas) e H3 do `estudo-phoenix-rust-n8n-2026-10.md` (nó declarativo, não linguagem).*
2. Clarion/WinDev → posição absoluta do controle (DLU/pixel) como verdade da janela. **Aqui:** a
   intenção de layout manda e a posição lida fica só para a prova de fidelidade (UI-IR v2,
   `ir.rs:7-13`). *Restrição: grade responsiva única para seis adaptadores.*
3. Clarion RI com `Cascade` no excluir → **aqui:** restringir, e a importação **avisa**.
   *Restrição: regra primordial da integridade.* O que isso faz com a aplicação migrada sobe (§5).
4. Crate do Query Designer do dono concatenava valor (`lib.rs:309` dela) → **aqui:** parâmetro
   posicional. *Restrição: defesa contra injeção estrutural, não função de escape.*
5. Octopus monolítico → **aqui:** processo com allowlist e caminho confinado. *Restrição: licença.*

### 3.2 A ponte Octopus: o que expõe hoje e o que falta para «gerar e conferir que compila»

**Expõe hoje** (`crates/phxclaw-octopus-bridge/src/lib.rs`): 11 capacidades em allowlist fixa —
`repo.map`, `repo.grep_ast`, `code.analyze`, `quality.evaluate`, `security.scan`, `migration.plan`,
`contract.verify` (só `py_file` × `rs_file`: Python→Rust), `shadow.check`, `dependency.map`,
`knowledge.search`, `toolchain.status`; caminho confinado ao workspace; timeout 120 s; teto de
8 MiB (que na prática é 64 KiB, H4). Fora do binário; plugin de processo
(`apps/phxclaw-octopus-plugin/src/main.rs`, protocolo `PROCESS_PROTOCOL_V1`). **0 testes.**

**Falta, em ordem:**

1. **Runner que não trava** (PS0002) — sem ele, qualquer saída real acima de 64 KiB vira timeout de 120 s.
2. **Capacidade de geração** (`octopus.generate`: fonte legado ou IR → crate Rust numa pasta do
   workspace) e **de conferência** — o contrato é do Octopus e **não está no repositório** (pergunta P8).
3. **Conferência que não depende do Octopus dizer que deu certo:** o crate que ele gerar passa pelo
   **mesmo** `rust_project` (check + clippy `-D warnings` + test) que o crate do ui-ir (PS0001) —
   um portão só para os dois geradores.
4. **Equivalência:** os mesmos casos de teste rodando no crate do Octopus e no do ui-ir;
   `contract.verify` hoje só conhece Python→Rust.
5. **Testes da ponte com executável falso** (script no `PATH` do teste), porque o real não existe aqui.

---

## 4. Backlog — prefixo PS, sprints pequenas

Coluna **AE** (auto-evolução, SP000015): `propor` = o diff cabe em gerador/IR/tela/testes e não toca
o vetado; `humano` = toca segurança, sandbox, capacidades, RBAC, formato em disco ou execução de
processo — **na dúvida, humano**. Caminhos que proponho para o portão de veto da SP000015
[hipótese, a lista deve sair do código como lei da casa]: `crates/phxclaw-sandbox/`,
`crates/phxclaw-agent/src/rbac.rs`, `rotas.rs`, `config/capability-catalog.json`,
`apps/phxclaw-desktop/src-tauri/capabilities/`, `crates/phxclaw-octopus-bridge/` (allowlist).

Papéis: A orquestrador · B engenharia · C DBA · D zelador · E designer · F prova real · G QA ·
H documentação · I versionador · J pesquisador. Nível de modelo: **forte** = formato, integridade,
segurança, concorrência; **leve** = tela roteirizada, varredura, teste mecânico.

| Sprint | Objetivo | Tarefas | Aceite (prova real: falha com o defeito reposto) | Papéis (dispensados) | Nível | AE | Depende |
|---|---|---|---|---|---|---|---|
| **PS0001** | Oráculo «gera e confere» **sem Octopus** | `phxclaw studio gerar <ddl> --conferir`: ui-ir → pasta → `rust_project` check + clippy `-D warnings` + test no bwrap; relatório JSON por gerador; tirar `#![allow(clippy::all)]` do `rust.rs` e consertar o aviso | com o `allow` reposto o portão acusa; um erro de tipo injetado no crate gerado volta com arquivo:linha; `pedidos.sql` verde | B F G H (C, D, E, I, J) | leve | propor | — |
| **PS0002** | Runner de processo único (defeito medido H4) | extrair o leitor em threads do `phxclaw-sandbox` para um ponto só; trocar os 7 laços | teste com filho que escreve 65.537 B: vermelho no laço velho, verde no novo, nos 7 | A B F G (C, E, H, J) | forte | humano | — |
| **PS0003** | Integridade que **avisa** | aviso quando a DDL pede `ON DELETE/UPDATE CASCADE/SET NULL`; concordância «Mae tem **filhas ligadas**» (medido: «filhas ligados») | `cascata.sql` dá 1 aviso nomeando tabela e coluna; regra continua restringir | B C F (D, E, I, J) | forte | propor | — |
| **PS0004** | IR v4 + envelope `studio-projeto` | `id` UUIDv7 em `Section`/`Field`; envelope com lista de artefatos (dicionário, janelas, consultas, relatórios, código); `FORMATO`/`UI_IR_V4.md` | v1–v3 leem e desenham igual (hash dos 6 adaptadores); campo desconhecido recusado com caminho | B C G H (E, D, J) | forte | humano | PS0001 |
| **PS0005** | Casca da aba Studio | rotas `/v1/studio/*` na matriz `rotas.rs` + RBAC; aba vazia com seleção de projeto; textos pela fábrica | `toda_rota_do_fonte_esta_na_tabela` reprova rota sem linha; sem usuários, nada muda (comportamento velho) | B E F G (C, D, J) | forte | humano | PS0004 |
| **PS0006** | Dicionário: ida e volta | editar entidade/campo/chave/relação no IR; IR → DDL; FK sem índice dos dois lados recusada na declaração | SQL→IR→SQL idêntico no `pedidos.sql`; par de testes «recusa cascata no excluir» e «não recusa tudo» | B C F G (D, E, J) | forte | propor | PS0003, PS0004 |
| **PS0007** | Tela Dicionário + ER | grade de entidades (phx-grid) + diagrama ER em SVG à mão | exercitado no navegador nos dois temas; «Blumenau» continua «Blumenau» (dado sem text-transform) | B E F (C, D, J) | leve | propor | PS0005, PS0006 |
| **PS0008** | Importar dicionário Clarion `.txd` | parser do texto exportado → dicionário; aviso por RI não suportada | amostra real do dono importa sem perda nas chaves/relações; RI `Cascade` vira aviso | B C F J (D, E) | forte | humano | PS0006 · **BLOQUEADA** (P1, amostra) |
| **PS0009** | Designer de janelas (R03 L4) | prévias em iframe com divisor, inspetor por linha, arrastar campo entre seções | aceite de M1 | B E F G (C, D, J) | forte | humano | PS0005 · portão ui-wasm |
| **PS0010** | Templates e propagação (R03 L2+L3) | `impacto(velha, nova, instâncias)`; portão de publicação | merge ingênuo apaga sobrescrita (vermelho); rejeitar não muda o hash das instâncias | B G (C, D, E, J) | forte | propor | PS0004 |
| **PS0011** | Query designer | artefato `consulta`: N filtros AND/OR, parâmetro posicional; render para SQL explicativo, Rust e WL | teste adversarial `O'Brien` e `x' OR '1'='1` viram parâmetro; nenhum valor no texto gerado | B C F G J (D, E) | forte | humano | PS0004 |
| **PS0012** | Diagnóstico WLanguage | `snippet-ls` publica diagnóstico em `.wl`: função fora de `FUNCOES_CONFERIDAS` = aviso com a página do Help; `.wl` no `languages.toml` | aviso aparece e some ao trocar pela função conferida (prova no Helix do IDE) | B F G (C, D, E, J) | leve | propor | — |
| **PS0013** | Ponte Octopus: gerar | capacidades `octopus.generate` e conferência; registro no extension-host; teste com executável falso | saída de 1 MiB não trava; caminho fora do workspace recusado; capacidade fora da allowlist recusada | A B F G J (C, E) | forte | humano | PS0002 · **BLOQUEADA** (P8) |
| **PS0014** | Equivalência ui-ir × Octopus | os mesmos casos de teste nos dois crates; `contract.verify` para Rust×Rust | divergência injetada num gerador é acusada | B F G (C, D, E) | leve | propor | PS0001, PS0013 |
| **PS0015** | WLanguage sai do UNVERIFIED | compilar `Regras.wl` no WinDev e rodar os casos do teste Rust | os 2 casos do `regras.rs` passam no WinDev; 1 função fora da lista quebra a compilação | B F J (C, D, E) | forte | humano | **BLOQUEADA** (SP000025, máquina Windows) |
| **PS0016** | Executar e depurar o gerado | do Studio: `rust_project test` e `debug` (gdb) no crate gerado | breakpoint em regra gerada para e lê variável no bwrap | B F (C, D, E, J) | leve | propor | PS0001 |
| **PS0017** | IA assistente por patch | a IA devolve patch do IR por UUID; motor valida antes de mostrar; aplicar exige clique | patch inválido não chega à tela; aplicar duas vezes dá o mesmo | B F G (C, D, J) | forte | propor | PS0004 |
| **PS0018** | Relatório com bandas | artefato `relatorio` (título/grupo/detalhe/rodapé/sumário, mestre-detalhe) → `.docx`/`.xlsx` pelo `phxclaw-office`; PDF ⏸ | mesmo projeto, mesmo hash; subtotal confere com soma recalculada | B C F (D, J) | leve | propor | PS0011 |
| **PS0019** | Projeto e versão | publicar = commit no worktree; histórico; diff por UUID; `If-Match` | duas abas: a segunda recebe 409 com a revisão; publicada não se regrava | B G I (C, E, J) | forte | humano | PS0004 |
| **PS0020** | Multitela (pedido 127, «obrigatório») | soltar painel em janela (navegador e Tauri), lembrar posição/tamanho/monitor | exercitado com duas janelas; reabrir restaura; DPI diferente não corta painel | B E F (C, D, J) | forte | humano | PS0009 · portão ui-wasm |

**Contagem:** 20 sprints · AE `propor` **10** (PS0001, 0003, 0006, 0007, 0010, 0012, 0014, 0016,
0017, 0018) · `humano` **10** · **BLOQUEADAS** 3 (PS0008, PS0013, PS0015) · 2 com portão na decisão
de UI-WASM (PS0009, PS0020). Prontas para começar sem nada de fora: **PS0001, PS0002, PS0003, PS0012**.

**Primeira sprint: PS0001.** Motivo: é o oráculo que todas as outras usam como aceite («gera e
compila»), sai inteira de motores que já estão no binário (`phxclaw-ui-ir` + `rust_project`), não
espera o Octopus (ausente aqui) e é alcance da auto-evolução — a SP000015 pode propô-la como
primeiro item do Studio.

---

## 5. O que sobe ao dono — e só isto

1. **Produto — banco-alvo do app gerado.** As fontes divergem: o estudo dos 68 projetos mira
   PostgreSQL (`projetos_rust_para_phoenix_2026-10-09.md` §2); o PhxSql nasceu «como parte do
   Phoenix» (`01-PEDIDOS.md:485`) e já guarda a `PICTURE` do Clarion no esquema (`:1319`). PhxSql,
   PostgreSQL, ou os dois? Decide o adaptador de dados de M3/M5/M6.
2. **Choque com pétrea — RI do legado.** Dicionário Clarion/WinDev com `Cascade` no excluir:
   a regra primordial manda restringir, então a aplicação migrada **muda de comportamento**
   (o excluir que cascateava passa a recusar). Restringir e avisar (recomendação), ou recusar a
   importação inteira? E a pétrea vale para app gerado sobre PostgreSQL, ou só sobre o PhxSql?
3. **Prazo/escopo.** As 20 sprints entram na conta da versão (SP000016) ou nascem ⏸ («depois da
   versão») pela regra de escopo congelado? O Studio é ordem do dono, então a regra (A) diz «conta»
   — mas isso muda a porcentagem do que falta; a decisão é dele.
4. **Recurso — Octopus.** Acesso ao `octopus-console` (ausente aqui) e o contrato dos comandos de
   geração (P8). Sem isso, PS0013/PS0014 ficam BLOQUEADAS.
5. **Recurso — máquina Windows com WinDev** (já decidida em 09/10, acesso pendente,
   `docs/diretivas/DECISOES_DO_DONO_20261009.md` item 3). Bloqueia PS0015.

Não sobe: onde mora o Studio (R03 + H1), Octopus fora do binário (licença, H2), conserto da ponte
(H4), PDF depois (docx/xlsx já existem), recusas da §6.

### Perguntas sobre Clarion/WinDev em aberto (para o orquestrador, que tem as skills)

- **P1 `.txd`:** gramática das seções (`[DICTIONARY]`, `[FILES]`, `[KEYS]`, `[RELATIONS]`…); como
  a RI aparece (Update/Delete: Restrict/Cascade/Clear/None); tipos (`STRING`, `CSTRING`, `PSTRING`,
  `DECIMAL`, `LONG`, `DATE`, `TIME`, `MEMO`, `BLOB`, `GROUP`, `OVER`) e atributos de chave (`DUP`,
  `NOCASE`, `OPT`, `PRIMARY`). Há amostra real do dono?
- **P2 `.txa`:** como procedimentos, templates (ABC × Clarion/Legacy) e **embeds** aparecem; o Studio
  tem de preservar embed escrito à mão na regeração?
- **P3 Janela Clarion:** estrutura `WINDOW … END`, unidade de diálogo, controles (`ENTRY`, `LIST`
  com `FORMAT`, `SHEET/TAB`, `BUTTON`, `PROMPT`, `OPTION/RADIO`, `CHECK`, `SPIN`, `COMBO`), `USE` e
  `FROM` — para mapear em intenção de layout sem posição absoluta.
- **P4 Browse/Form ABC:** `BrowseClass`, procedimento de update, `FormVCR` → `List`/`Form`/
  `MasterDetail` do UI-IR: o mapeamento é 1:1?
- **P5 WinDev:** a Análise e as janelas têm exportação **textual** estável (formato texto para
  controle de versão) e desde qual versão? Ou só pela UI (pywinauto, SP000025)?
- **P6 WLanguage:** quais funções de tela/dados o gerador precisaria além das 12 conferidas
  (`HFilter`, `TableDisplay`, `Open`, `Close`…), com a página do Help de cada uma.
- **P7 `PICTURE`:** a lista de tokens (`@n`, `@d6`, `@s20`, `@p…p`) e a semântica de cada um, para
  a máscara do dicionário.
- **P8 Octopus:** o que o `octopus-console` expõe para **gerar** (comando, entrada — fonte legado
  ou Phoenix IR, saída — crate Rust?), se o frontend Clarion/WLanguage existe nele, e o formato do
  `octopus-phoenix-ir` (citado no `OCTOPUS_REUSE_AUDIT_V09.md` §13).

---

## 6. Matriz de evidência

| Fonte | O que resolve | Custo | Uso |
|---|---|---|---|
| `docs/ui/STUDIO_UI_R03.md` | onde mora o Studio, template/instância, lotes L1–L5 | 0 (decidido) | **adotado** (PS0004/05/09/10) |
| `phxclaw-ui-ir` (8.575 linhas com testes) | IR, 6 adaptadores, regras Rust/WL, dicionário a partir de DDL | 0 (no binário) | **base** de M1/M3/M6 |
| `rust_project` (`sistema.rs:1961`) | cargo check/clippy/test no bwrap com JSON | 0 | **oráculo** de PS0001 |
| `phxclaw-sandbox/src/lib.rs:399` | leitura de pipe sem trava | extrair para um ponto só | conserto de PS0002 |
| `phxclaw-octopus-bridge` | 11 capacidades de análise, confinamento | 0 testes; trava >64 KiB; sem gerar | **consertar e estender** (PS0002/13) |
| `dap.rs`, `lsp.rs`, `ide.rs`, `snippet-ls` | depurar, ler símbolos, editar no Helix | diagnóstico WL a escrever | M4/M7 |
| `fluxos.js` + `fluxos_tela.rs` | editor em grafo SVG à mão; revisão + `If-Match` | 0 | **molde** de tela e de gravação |
| `phxclaw-office` | docx/xlsx determinístico | sem PDF | M2 |
| Query Designer do dono (`phxsql/…/phoenix-query-designer-2026-09-17.md`) | N filtros, dataBar, LLM local | código recusado (injeção, 0 testes adversariais) | **ideia** (PS0011) |
| Estudo dos 68 projetos + `estudo-phoenix-rust-n8n` | React/Tauri, wasmtime, Rhai, SQLx | +37 a +113 pacotes por peça | **recusado para o Studio** (H1); vale ao Phoenix fora daqui |
| `OCTOPUS_REUSE_AUDIT_V09.md` | licença, ordem de absorção | processo externo | **adotado** (H2) |
| Pedidos 127 e «Analysis do WinDev» (`01-PEDIDOS.md`) | multitela obrigatória, editor de modelo | — | PS0020, PS0007 |

## 7. Recusas medidas (para não voltarem)

| Proposta | Número que a recusa |
|---|---|
| Studio como app separado | desktop já serve o mesmo PWA (`frontendDist`); duplicaria RBAC, rotas, idiomas |
| Octopus linkado no binário | `Proprietary` × `Apache-2.0` |
| «0 avisos» do crate gerado como prova | `allow(clippy::all)` esconde 1 aviso |
| Remendar só a ponte Octopus | o mesmo laço em 7 lugares; o motor certo já existe no sandbox |
| Gramática Clarion/WL antes do diagnóstico | 0 gramáticas e 0 parsers hoje; a lista conferida do Help dá a primeira fatia sem parser |
| Posição absoluta do legado como verdade da janela | UI-IR v2 a guarda só para fidelidade; a grade responsiva serve 6 adaptadores |
| Código da crate do Query Designer, `LIMIT 100` automático, 7 dicas de tuning | já recusados com número no parecer de 17/09 §2 |

## 8. Lacunas (o que não pude medir)

- Suíte do `phxclaw-ui-ir` **não rodada** nesta sessão: 6 `cargo` vivos disputavam o alvo e o disco
  estava em 4,2 GB livres. Medi o caminho gerador → crate → compilação em alvo próprio.
- Tamanho do binário de release: só existe o debug (135 MB); o custo do Studio no release decide-se
  com `cargo build --release` + `ls -l` antes e depois de PS0005.
- Decisão de UI-WASM ainda não escrita; bancada em curso.
- Octopus real, amostra `.txd`, máquina Windows: ausentes (§5).

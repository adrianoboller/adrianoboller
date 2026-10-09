# Visão computacional + engenharia reversa — especificação e backlog

**Papel J (pesquisador), 09/10/2026.** Ordem do dono do mesmo dia: «Visão
computacional do projeto através de vídeo ou imagem e faça a engenharia
reversa». Fase de **especificação**: nada aqui foi implementado, compilado nem
comitado.

Legenda de toda afirmação: **[fonte: arq:linha]** = lido no código/documento
citado · **[medido]** = rodado com número reproduzível · **[raciocinado]** =
não medido, com o que decidiria na bancada. **Nesta sessão não compilei nem
rodei bancada** (fase de especificação): todo número «medido» abaixo foi lido de
comentário/documento do próprio repositório e traz a fonte; o que seria medido
numa bancada vai marcado **[raciocinado, não medido]**.

---

## 0. Alcance e limite ÉTICO (lei do documento)

A engenharia reversa por visão vale **só** para material que o operador tem
**direito** de analisar: a própria aplicação, software autorizado, ou sistema
legado que ele vai migrar — o caso Phoenix, ler telas de apps Clarion®/WinDev®
e levar para Rust [fonte: `docs/propostas/phoenix-studio-especificacao.md:1`].

**Não é** ferramenta para quebrar proteção de terceiros, burlar DRM ou licença,
nem reconstruir UI alheia sem direito. O limite não é conselho: é guarda.

Guarda proposta (**VR0002**, nasce com a primeira ferramenta):

- **O operador declara a origem e a autorização** ao abrir a tarefa de RE
  (campo `origem`: `propria` | `autorizada` | `legado_a_migrar`, e uma frase de
  justificativa). Sem a declaração, a ferramenta **recusa** — como o
  `transcribe` recusa sem configuração [fonte:
  `crates/phxclaw-agent/src/visao.rs:1133`], e como o `screenshot_to_erp_ui`
  recusa quando nenhum rótulo se confirma [fonte:
  `crates/phxclaw-agent/src/ui.rs:414`].
- **A evidência registra** a declaração no `EvidenceLedger` que o custo já usa
  [fonte: `crates/phxclaw-agent/src/custo.rs:21`], com a origem, o hash da
  imagem/vídeo de entrada e a data. O laudo da RE carrega a origem declarada,
  para quem lê julgar o direito em vez de acreditar nele — o mesmo princípio do
  «gate externo mostra a frase que casou» do PMO [fonte: `CLAUDE.md`].
- **Isto é guarda, não prova de direito.** A declaração não torna lícito o que
  não é; ela **fixa a responsabilidade** no operador e deixa rastro. Decisão de
  produto sobre texto legal/ToS **sobe ao dono** (§5).

---

## 1. O pipeline, em etapas (entrada → saída, aceite testável)

### 1.a IMAGEM — screenshot → UI-IR → código → fidelidade

Este caminho **já existe quase inteiro**. A novidade é torná-lo medido contra
API multimodal e genérico de controle (não só ERP).

```
screenshot(.png/.jpg)
  → [E1] OCR em TSV (tesseract, bwrap, psm 11)      → palavras+caixas
  → [E2] layout por geometria (sem modelo)          → seções/rótulos/grade/ordem
  → [E3] modelo multimodal (3 perguntas)            → rótulos candidatos
  → [E4] confirmação pelo OCR (só o que está escrito)→ campos/itens
  → [E5] UI-IR (app.phx.json) via SQL sintético     → App
  → [E6] renderizadores (html/bootstrap/react/
           flutter/rust/wlanguage)                  → código nos 6 alvos
  → [E7] medidor de fidelidade (ida-e-volta)        → Medida (estrutura, não pixel)
```

| Etapa | Entrada | Saída | Onde roda | Aceite testável |
|---|---|---|---|---|
| E1 OCR | PNG/JPG | TSV (palavra+caixa+página) | **local**, tesseract no bwrap sem rede [fonte: `visao.rs:95`, `:154`] | arquivo hostil não escapa do sandbox; sem tesseract, recusa nomeando o pacote [fonte: `visao.rs:126`] |
| E2 layout | palavras TSV | seções, rótulos, grade, ordem de leitura/tabulação | **local**, heurística de geometria, limiares em múltiplos de `h0` [fonte: `layout.rs:1-16`] | a prova é a fidelidade (E7), **não** a leitura do código [fonte: `layout.rs:14`] |
| E3 modelo | PNG + 3 perguntas | 3 respostas JSON (campos, listas, grade) | **local hoje** (Ollama `qwen2.5vl:3b`) [fonte: `ui.rs:221`]; **API = falta nascer** (VR0005) | respostas degeneradas toleradas por E4 [fonte: `imagem.rs:213`] |
| E4 confirmação | respostas + linhas OCR | `Rotulo{texto,obrigatorio,lista}` | **local**, puro [fonte: `imagem.rs:41`] | rótulo que o modelo inventou e **não** está na tela é descartado [fonte: `imagem.rs:38`, `ui.rs:341`] |
| E5 UI-IR | campos/itens | `App` (UI-IR) + `tela.sql` | **local**, puro [fonte: `ui.rs:291`] | tipo/obrigatório saem de regra fixa, não do modelo [fonte: `imagem.rs:8`,`:120`] |
| E6 código | `App` | 6 alvos | **local**, puro [fonte: `lib.rs:4-6`] | o crate Rust gerado compila `--offline` [fonte: `phoenix-studio-especificacao.md` tabela medida] |
| E7 fidelidade | `App` origem × `App` lido | `Medida` (achado/perdido/inventado, rótulo, tipo, obrigatório, τ de Kendall, Rand do grupo, distância) | **local**, puro + Chromium para fotografar o gabarito [fonte: `fidelidade.rs:1-12`, `fidelidade_ui.rs`] | 20 telas do gabarito, faixa min–max por métrica, chave fora da conta [fonte: `fidelidade.rs:30`,`:12`] |

**Pixel vs. estrutura — recusa medida.** LPIPS e CLIP (similaridade de imagem)
**morreram na triagem de 01/10**: a tela gerada difere da original de propósito,
então medir aparência puniria o acerto [fonte: `fidelidade.rs:6`]. O que se mede
é **estrutura**. Esta recusa já paga: impede a proposta «meça pixel a pixel» de
voltar. A fidelidade de pixel só reaparece como **sanidade** no vídeo (E-V2,
diferença de quadro), nunca como nota de conversão.

**Local vs. API multimodal (E3) — o número de cada um.**

| Dimensão | Modelo local (Ollama `qwen2.5vl:3b`) | Modelo por API (chave do operador) |
|---|---|---|
| Custo em dinheiro | zero declarado pelo operador [fonte: `custo.rs:11`] | pago, contado pelo R2 (`custo.precos`) e limitado pelo R3 (`orcamento.*`) [fonte: `custo.rs`, `orcamento.rs`] |
| Latência | **[medido, lido]** «segundos a minutos por print na CPU» [fonte: `tests/visao.rs:2`]; a prova de fidelidade evita o modelo para «não esperar ~90 s por tela» [fonte: `ui.rs:235`] | **[raciocinado, não medido]** 1ª resposta tipicamente < 10 s; o que decide é a bancada de latência com a chave real |
| Acerto medido | **[medido, lido]** campos 8/8 no cadastro (30/09); a pergunta de listas achou «Situação» que a 1ª perdeu [fonte: `ui.rs:201`] | **[raciocinado, não medido]** esperado ≥ local em telas densas; decide a mesma bancada `ui fidelidade --modelo N` com alvo API |
| Rede / sigilo | sem rede: a tela nunca sai da máquina | a tela **sai** para o provedor — sobe ao dono quando a origem é sensível (§5) |
| Caminho no código | existe [fonte: `ui.rs:258`] | os 4 provedores já traduzem `.images` para o bloco multimodal [fonte: `phxclaw-llm/src/{anthropic.rs:54,openai.rs:55,gemini.rs:82,ollama.rs:130}`], **mas** `ler_tela` não passa por eles nem pela conta da tarefa — é o que VR0005 liga |

Divergência da origem (nossa restrição): o modelo **nunca decide sozinho** — ele
sugere, o OCR confirma, a regra fixa tipa. É a inversão do REA genérico, onde o
modelo narra livre; aqui a borda determinística segura o palpite. A restrição é
a **integridade do dado lido**: «o que o modelo inventa não passa» [fonte:
`imagem.rs:8`].

### 1.b VÍDEO — amostragem → diff → estados → grafo de navegação + UI-IR por tela

Este caminho **não existe**: `phxclaw-media-intelligence` não tem vídeo
[medido: `grep -ni video|mp4|frame` → 0], só há `ffmpeg_record_command`
(captura) [fonte: `crates/phxclaw-system-automation/src/lib.rs:296`] e o padrão
«FFmpeg como processo externo no bwrap» do OpenMontage [fonte:
`docs/propostas/openmontage-especificacao.md:273`, via `visao::isolado_com`
`visao.rs:177`].

```
vídeo(.mp4/.mov/.webm) [origem declarada]
  → [V0] ffprobe no sandbox: formato/duração/fps na lista branca    → ok/recusa
  → [V1] amostragem de quadros (FFmpeg -vf, por cena + cadência)    → frames PNG
  → [V2] diff de quadros (perceptual leve, em Rust)                 → quadros-chave (estados)
  → [V3] cada estado distinto → pipeline IMAGEM (E1..E5)            → UI-IR por tela
  → [V4] transições entre estados (o que mudou + evento inferido)   → arestas
  → [V5] grafo de navegação (nós=telas, arestas=transições)         → fluxo (JSON) + telas
```

| Etapa | Entrada | Saída | Onde roda | Aceite testável |
|---|---|---|---|---|
| V0 triagem | arquivo não confiável | metadados + veredito | **local**, `ffprobe` no bwrap sem rede [padrão: `openmontage` §4] | lista de reprodução / executável disfarçado de `.mp4` recusado antes de decodificar [fonte: `openmontage-especificacao.md:275`] |
| V1 amostra | vídeo + política | PNGs numerados | **local**, FFmpeg no bwrap, `-protocol_whitelist file` [fonte: `openmontage-especificacao.md:273`] | N quadros = cadência × duração ± 1 [raciocinado, não medido]; teto de disco e de quadros |
| V2 diff | PNGs | índices de quadro-chave | **local**, Rust puro (sem crate de visão) | dois quadros idênticos → 1 estado; corte real → 2 estados (teste de ouro com vídeo sintético) |
| V3 por tela | quadro-chave | UI-IR | reusa 1.a | mesma fidelidade da imagem, por tela |
| V4 transição | par de estados | aresta rotulada | **local** + decisor R1 opcional p/ rotular o evento | «clicou em X → abriu tela Y» só entra se o rótulo está escrito num dos quadros; senão a aresta é `transição não rotulada` |
| V5 grafo | nós + arestas | `fluxo.json` + telas | **local**, puro; alimenta a tela Fluxos existente [fonte: `fluxos_tela.rs`] | grafo recarrega na tela Fluxos; nó órfão ou ciclo aparece, não some |

**Limite do vídeo, medido no gargalo certo.** O gargalo não é decodificar: é
**rodar o pipeline de imagem por quadro-chave**. Um vídeo de 10 min a 1 quadro/s
= 600 quadros; se cada leitura com modelo local custa «segundos a minutos»
[fonte: `tests/visao.rs:2`], 600 leituras são horas. Por isso **V2 vem antes de
V3**: a amostragem grosseira e o diff reduzem 600 quadros a **dezenas** de
estados distintos antes de qualquer modelo. É a mesma lição do PhxSql «medir a
premissa do item antes de implementar o item»: processar todo quadro é o item
errado; processar só o estado novo é o certo. **[raciocinado, não medido]** — o
que decide o número é a bancada: contar estados distintos num vídeo real de tela
e o tempo total com modelo local vs. API.

---

## 2. O que já existe vs. o que falta nascer (em Rust), peça por peça

| Peça | Estado | Fonte | Aceite do que falta |
|---|---|---|---|
| OCR em TSV no sandbox | **existe** | `visao.rs:95` | — |
| Layout por geometria | **existe** | `layout.rs` | — |
| Confirmação pelo OCR | **existe** | `imagem.rs:41` | — |
| UI-IR + 6 renderizadores | **existe** | `lib.rs`, `html/react/flutter/rust/wlanguage.rs` | — |
| Leitura com modelo local | **existe** | `ui.rs:236` | — |
| Prova de fidelidade (20 telas) | **existe** | `fidelidade.rs`, `fidelidade_ui.rs` | — |
| Declaração de origem/autorização + registro na evidência | **falta** | — | recusa sem declaração; evidência carrega origem+hash+data (VR0002) |
| E3 por **API multimodal**, contado pelo R2/R3 | **falta** (o transporte existe nos 4 provedores; o reitor `ler_tela` não usa) | `phxclaw-llm/src/*.rs`, `orcamento.rs:LlmDaTarefa` | `ler_tela` aceita um `Llm` da tarefa; a chamada multimodal cobra a conta; sem preço, para em «não medido» (VR0005) |
| V0 triagem de vídeo (ffprobe/lista branca) | **falta** | padrão `openmontage` | arquivo malicioso recusado (VR0006) |
| V1 amostragem de quadros (FFmpeg -vf) | **falta** | `ffmpeg_record_command` só captura | quadros = cadência×duração ±1; teto de disco (VR0006) |
| V2 diff de quadros em Rust | **falta** | — | estados corretos em vídeo sintético (VR0007) |
| V4/V5 grafo de navegação + `fluxo.json` | **falta** | tela Fluxos existe | grafo recarrega na tela Fluxos (VR0008) |
| Camada RE além da UI (binário) | **falta**, e entra como **ferramenta externa no portão** (§3) | — | §3 |

---

## 3. Engenharia reversa ALÉM da UI (binário/comportamento)

O dono pediu «faça a engenharia reversa». Visão reconstrói a **tela**; o
comportamento (formato de arquivo, protocolo, lógica de um binário legado) é
outra camada. A regra desta casa decide o corte: **zero dependências externas**
no nosso código — então nada de desmontador embarcado. O que cabe é **ferramenta
externa do operador, no mesmo portão e sandbox** do tesseract/FFmpeg
[fonte: `visao.rs:154`, padrão `openmontage`].

| Ferramenta | Entra? | Como | Limite |
|---|---|---|---|
| `strings`, `nm`, `objdump`, `readelf`, `file` | **sim** | processo externo no bwrap sem rede, binário só leitura, saída em pasta temporária, teto de tempo/bytes [padrão `isolado_com`] | só leitura estática; nunca **executa** o binário-alvo |
| `ghidra` / `radare2` (headless) | **sim, como comando externo do operador** | igual acima; o operador instala e configura a variável, como o `voz.rs`/FFmpeg [fonte: `openmontage-especificacao.md:307`] | pesado; opcional; **licença e instalação são do operador** |
| **Executar** o binário-alvo para observar | **não nesta fase** | — | executar código de terceiro é outra superfície de ataque e outra decisão ética; sobe ao dono |
| Decompilar para **copiar** lógica proprietária | **não** | — | choca a lei ética (§0): RE para **migrar o que é do operador**, não para copiar alheio |
| Embarcar desmontador como crate | **não** | — | choca «zero dependências externas» (pétrea PhxSql herdada) |

**O que a camada binária entrega:** fatos estáticos (símbolos, cadeias, formato,
dependências) para o operador **entender o próprio legado**, com a mesma guarda
de origem da §0, e **sempre com o LLM analisando, nunca decidindo** — o decisor
R1 em forma fechada [fonte: `decisao.rs:1-14`] pode classificar («este símbolo é
de rede? sim/não»), mas o veredito é do operador.

**Licença do REA (`morluto/rea`).** O estudo externo citado na ordem. Conferido
agora: **não é toolkit de RE de binário** — é um *skill*/pacote npm
(`@morluto/rea`) de orquestração de prompt, rotulado **MIT** por badges e
listagens de terceiros (Trendshift, Prismix, TomeVault)
[fonte: busca web, §Fontes]. **O que falta conferir antes de qualquer leitura:**
o texto do arquivo `LICENSE` no fonte (não recuperei o conteúdo, só o rótulo) e
se a licença MIT cobre também os *prompts/skills* e não só o código. Decisão:
como a pétrea manda — **só integração externa se a licença for permissiva
conferida**; sendo MIT conferido no `LICENSE`, vale como **inspiração de método,
nunca cópia**, e nada dele vira dependência (é Node, nós somos Rust std-only).
Se vier copyleft/proprietária no `LICENSE`, **não se lê o código** — só a ideia
pública. **[a conferir, não resolvido nesta sessão.]**

---

## 4. Backlog em sprints pequenas (prefixo VR)

Cada sprint com **prova real nos dois sentidos**: o teste falha com o defeito
reposto e passa com o conserto. Coluna **AE** = a auto-evolução (SP000015) pode
**propor** (`propor`) ou exige **humano/decisão** (`humano`).

| Sprint | Entrega | Prova real | AE | Depende |
|---|---|---|---|---|
| **VR0001** | **1ª entrega útil**: de um screenshot de formulário simples, gerar a UI-IR e o HTML, com **fidelidade medida** | uma tela nova (fora do gabarito) → `screenshot_to_erp_ui` → `Medida` com revocação e rótulo-exato impressos; defeito reposto (campo some) derruba a revocação | propor (já há motor; é fixture + aceite) | — |
| **VR0002** | Guarda ética: declaração de origem + registro na evidência | tarefa de RE sem `origem` **recusa**; com origem, o `EvidenceLedger` grava origem+hash+data (teste lê de volta) | **humano** (regra ética) | — |
| **VR0003** | Gabarito de leitura com **telas reais do operador** (não sintéticas), origem declarada | fidelidade medida em ≥ 5 telas reais; faixa min–max publicada | humano (dados do dono) | VR0001–2 |
| **VR0004** | Generalizar além do ERP: ler telas **não-formulário** (lista, painel) para UI-IR | lista com N colunas vira `Screen::List`; defeito reposto (coluna trocada) acusa | propor | VR0001 |
| **VR0005** | E3 por **API multimodal** pela chave do operador, contado pelo R2/R3 | `ler_tela` recebe `Llm` da tarefa; chamada multimodal cobra a conta (teste de orçamento); sem preço → «não medido»; comparação de fidelidade local × API na mesma bancada | **humano** (a tela sai da máquina; decisão de produto) | VR0001 |
| **VR0006** | Vídeo V0+V1: triagem `ffprobe` + amostragem de quadros no sandbox | `.mp4` malicioso (playlist/exe) recusado; quadros = cadência×duração ±1; teto de disco respeitado | propor | — |
| **VR0007** | Vídeo V2: diff de quadros em Rust → estados | vídeo sintético: 3 cortes → 4 estados; ruído de compressão não cria estado falso | propor | VR0006 |
| **VR0008** | Vídeo V4+V5: transições + grafo `fluxo.json` na tela Fluxos | grafo de 3 telas recarrega na tela Fluxos; aresta sem rótulo aparece como «não rotulada», não some | propor | VR0007, VR0003 |
| **VR0009** | RE estática de binário: `strings/nm/objdump/file` no portão+sandbox | binário só leitura; rede recusada; saída em pasta temporária; sem a ferramenta, recusa nomeando-a | **humano** (nova capacidade/ética) | VR0002 |
| **VR0010** | Ghidra/radare2 headless como comando externo do operador | roda só se o operador configurou a variável; laudo carrega origem; ausência recusa com instrução | **humano** (instalação/licença do operador) | VR0009 |

**1ª entrega útil = VR0001**, e ela quase não tem código novo: o motor existe.
O trabalho é a **fixture + o aceite** que prova a ida-e-volta numa tela fora do
gabarito — exatamente o que faltava para a capacidade deixar de ser promessa e
virar garantia medida. **[raciocinado, não medido]**: o número de fidelidade que
VR0001 publicará só sai rodando `phxclaw ui fidelidade`; não rodei nesta sessão.

---

## 5. Riscos e o que sobe ao dono

| Risco | Natureza | Mitigação | Sobe ao dono? |
|---|---|---|---|
| Direito autoral da origem | ético/legal | guarda da §0: declaração + evidência; laudo mostra a origem declarada | **sim** — texto legal/ToS e o caso de origem sensível são produto |
| Alucinação da visão virar «fato» | integridade do dado | E4: só entra o que está **escrito** na tela; modelo sugere, OCR confirma, regra fixa tipa [fonte: `imagem.rs:8`] | não — a guarda é técnica e já é a lei da casa |
| Custo do modelo multimodal por API | dinheiro | R2 conta, R3 limita e **para** em `budget_exceeded`; sem preço → «não medido», nunca zero [fonte: `custo.rs:11`, `orcamento.rs`] | **sim** — teto de gasto e a decisão «a tela sai da máquina» são produto |
| Vídeo longo e disco | recurso | V2 reduz quadros a estados antes do modelo; teto de disco/quadros no sandbox; pasta temporária apagada [padrão `PastaTemp` `visao.rs:63`] | não — teto é config do operador |
| Decodificador como superfície de ataque | segurança | tudo no bwrap sem rede, entrada só leitura, lista branca de formato/protocolo [fonte: `visao.rs:154`, `openmontage §4`] | não — padrão já estabelecido |
| Licença do REA não conferida no `LICENSE` | legal | não ler código até conferir o arquivo; inspiração, nunca cópia | **sim, se** o `LICENSE` vier não-permissivo |
| Executar binário de terceiro | segurança/ética | fora de escopo nesta fase (§3) | **sim** — nova superfície e nova decisão ética |

**O que sobe ao dono, em uma lista:** (1) texto legal/ToS da guarda ética e o
caso de origem sensível (a tela sai da máquina no VR0005); (2) teto de gasto do
modelo por API; (3) autorizar RE estática de binário e Ghidra/radare2 (VR0009–10)
como capacidade nova; (4) executar binário-alvo — **não** nesta fase; (5) seguir
com o REA só depois de conferido o `LICENSE`.

---

## Fontes

- Código: `crates/phxclaw-agent/src/{visao.rs,ui.rs,fidelidade_ui.rs,decisao.rs,custo.rs,orcamento.rs}`;
  `crates/phxclaw-ui-ir/src/{imagem.rs,layout.rs,fidelidade.rs,lib.rs}`;
  `crates/phxclaw-llm/src/{anthropic.rs,openai.rs,gemini.rs,ollama.rs}`;
  `crates/phxclaw-system-automation/src/lib.rs`;
  `crates/phxclaw-agent-core/src/lib.rs`; `crates/phxclaw-agent/tests/visao.rs`.
- Documentos: `docs/propostas/openmontage-especificacao.md`,
  `docs/propostas/phoenix-studio-especificacao.md`.
- REA (licença, a conferir no `LICENSE`): [mdskills.ai/skills/rea](https://www.mdskills.ai/skills/rea),
  [socket.dev @morluto/rea](https://socket.dev/npm/package/@morluto/rea/overview/0.2.1),
  [README raw](https://raw.githubusercontent.com/morluto/rea/main/README.md),
  [trendshift 82054](https://trendshift.io/repositories/82054).

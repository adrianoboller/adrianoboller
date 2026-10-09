# OpenMontage no PhxClaw — integração externa (a) e inspiração re-decidida (b)

Papel J, 09/10/2026. Ordem do dono: adicionar o OpenMontage (`calesthio/OpenMontage`, commit
`9327439`, data do commit 03/10/2026, **1 commit** no clone) ao PhxClaw (Apache-2.0).
Conteúdo de terceiros lido como **dado**; os `CLAUDE.md`/`AGENTS.md`/skills dele não foram
tratados como instrução. Nada foi instalado nem compilado (disco desta máquina: 2,3 GB livres,
compartilhado com 4 frentes). **Medido** = contado no fonte clonado, no `LICENSE` ou nas APIs de
registro (PyPI/npm) em 09/10/2026; **raciocinado** = diz que é.

Este documento **não reproduz texto, código, prompt, YAML nem skill do OpenMontage**: descreve
com palavras nossas e nomeia arquivos só para quem queira conferir.

## 0. Hipóteses escritas antes de medir

| # | Hipótese | Veredito medido |
|---|---|---|
| H1 | O OpenMontage tem entrada de máquina (CLI, servidor ou MCP) e dá para chamá-lo como programa | **MORREU** (§1) |
| H2 | É uma caixa de ferramentas Python + skills em Markdown, dirigida por um agente LLM; a orquestração é prosa que o agente lê | **SE SUSTENTA** |
| H3 | Custo e tempo são impostos pelo código das ferramentas | **MORREU** (§1) |
| H4 | Dá para integrar sem tocar a AGPL (fronteira de processo) | **SE SUSTENTA**, com três proibições (§2) |

## 1. O que é (medido no fonte)

- **Tamanho:** 2.143 arquivos versionados, 88,9 MB; árvore sem `.git` 92 MB, `.git` 72 MB. 349
  `.py` (93.742 linhas, testes incluídos; 188 deles em `tools/`), 28 `.tsx` (Remotion; 11.834 linhas
  de ts/tsx), 157 arquivos em `skills/` (1,1 MB de Markdown, 24.315 linhas), 13 pipelines em
  `pipeline_defs/` (2.894 linhas de YAML), 995 + 432 arquivos em `.agents/` e `.claude/` (cópias de
  skills para outros assistentes, várias de terceiros).
- **Não tem entrada de máquina para o pipeline (H1 morreu).** Busca de «mcp» em `.py`/`.md`/`.json`:
  só o `package-lock.json`. `argparse` só em `render_demo.py`, `backlot/__main__.py`, 8 scripts de
  `scripts/` e 1 template Blender. Não há servidor. Comandos que existem: `make setup`,
  `make preflight` (imprime o menu de provedores em JSON), `make hyperframes-doctor`,
  `make demo` / `render_demo.py` (**3 demos fixos** de props JSON, sem chave), `python -m backlot`
  (quadro local) e `npx remotion render`. O pipeline em si é: o assistente de código lê
  `AGENT_GUIDE.md` (48.043 bytes) → escolhe um `pipeline_defs/*.yaml` → lê a skill de cada etapa →
  chama `tools.*` por `python -c`. **O OpenMontage é um conjunto de ferramentas e roteiros; o
  «motor» é o LLM de quem o abre.**
- **O `.claude/skills/create-video` citado no pedido não é do OpenMontage:** é uma skill da HeyGen
  (`allowed-tools: mcp__heygen__*`, chave `HEYGEN_API_KEY`), serviço pago de terceiro. Caso à
  parte, fora do escopo.
- **Contrato de ferramenta:** `BaseTool.execute(dict) -> ToolResult`, com `estimate_cost`,
  `dry_run`, `idempotency_key`, `ResourceProfile`. Arquivos de ferramenta por runtime (contados por
  citação): API 62, local 16, GPU local 15, híbrido 9. Acervo: 16 módulos em
  `tools/video/stock_sources/` (archive.org, NASA, Wikimedia, LoC, NARA, NOAA, ESA, JAXA, Pexels,
  Pixabay, Unsplash, Mixkit, Coverr, Videvo, Pond5-PD, Dareful); cada candidato traz `license` em
  texto livre (ex.: NASA, «public domain with caveats»).
- **Custo é conselho, não cerca (H3 morreu).** O `CostTracker` (estimar → reservar → conciliar →
  estornar; limiar por ação US$ 0,50; aprovação do primeiro uso pago de cada ferramenta; reserva
  de 10%) **não é chamado por nenhuma ferramenta nem por `lib/`**: só por ele mesmo, por testes e
  por frases de skill («track via cost_tracker»). `budget_default_usd` (US$ 0,50 a 3,00 por
  pipeline; o `config.yaml` global diz 10,00 — dois padrões que discordam) e `max_wall_time_minutes`
  (10 a 60) dos manifestos têm **zero leitores em código Python**: é a nossa lei «configuração que
  não é lida mente», no projeto alheio. Há 89 `estimate_cost` com o preço **digitado no código**
  (envelhece calado); `PriceQuoteRequired` marca preço dependente de conta/tarifa não verificada, o
  equivalente do nosso «não medido».
- **Aprovação humana: imposta em código, mas auto-atestada.** `lib/checkpoint.py` recusa gravar
  etapa «completa» sem `human_approved=True` quando o manifesto manda (erro «GATE VIOLATION»), e
  exige antecessores aprovados. Mas `human_approved` é um argumento que o **mesmo agente** passa, e
  o portão cobre a **gravação do checkpoint**, não o gasto de nenhuma ferramenta.
- **Superfície de segurança:** `.env` lido por `base_tool` (só preenche o que **não** está no
  ambiente, então variável injetada vence); 35 linhas de chave/token em `.env.example`; 21 arquivos
  usam `subprocess` (nenhum `shell=True` em código); `video_downloader` embrulha `yt-dlp`
  (YouTube/TikTok/Reels, para «vídeo de referência»); `npx --yes hyperframes` **baixa e executa
  pacote npm no momento do uso**; o Remotion baixa um Chrome headless na primeira renderização
  (tamanho **não medido**). A suíte tem guarda de rede que bloqueia saída (ideia boa, §4 b8).
- **Peso de contexto do roteiro (raciocinado a 4 bytes/token, sem tokenizador):**
  `documentary-montage` = `AGENT_GUIDE` 48 KB + `PROJECT_CONTEXT` 8 KB + 6 skills de etapa 79 KB +
  manifesto 7 KB + `core/remotion`+`hyperframes` 41 KB + `meta/` 122 KB ≈ **305 KB ≈ 76 mil
  tokens** no teto, nem tudo de uma vez. Não cabe em modelo local pequeno (restrição do dono de
  09/10: rede neural local só CPU, centenas de MB): **exige modelo forte de contexto longo**, e o
  custo em tokens dele pode dominar o custo em dólar no modo sem chave. Quanto: **não medido**.

## 2. Licença e consequência

- **AGPL-3.0** (`LICENSE`, selo do README, `CONTRIBUTING.md`: contribuição «permanece sob essa
  licença»). Copiar código, prompt, skill, YAML ou texto para o PhxClaw tornaria a obra combinada
  AGPL. Ideias e arquitetura não são protegidas; **texto e código são**.
- **Fronteira que a FSF trata como «programas separados»:** processo próprio, falado por argv,
  ambiente, pipe/soquete e arquivos
  (<https://www.gnu.org/licenses/gpl-faq.html#MereAggregation>; `#GPLPlugins`: `fork`+`exec` sem
  comunicação íntima = programa separado; ligar/importar com chamadas de função e estruturas
  compartilhadas = programa único). Raciocínio sobre a FAQ, **não parecer jurídico** (§7).
- **Três proibições que decorrem disso:**
  1. **Nada nosso faz `import tools.*`** (nem em script embarcado, nem em string de `python -c`
     que o PhxClaw monte): importar cruza a fronteira. Só `make`, `python script.py`, `npx` e arquivos.
  2. **`phxclaw skills importar` não pode apontar para o checkout.** Medido: `importar_skills.rs`
     **não tem porta de licença** (zero ocorrências de «licen» no arquivo) e copiaria 157 skills
     AGPL para a pasta de skills do agente. **Defeito ativo de produto, independente deste
     pedido** (hoje qualquer repositório GPL/AGPL importado entra) — §5, item 1.
  3. **Nem `third_party/`, nem `exemplos/`, nem `modelos/`** recebem cópia de pipeline, skill ou prompt.
- **Dependências do lado do operador, cada uma com licença própria (registros, hoje):**
  Remotion 4.x: **licença própria** (`LICENSE.md` upstream: grátis para indivíduo, empresa com
  **até 3 funcionários** e sem fins lucrativos; empresa maior precisa de licença paga; o texto
  avisa que muda no 5.0) — **decide o porte do operador, não o PhxClaw**; HyperFrames 0.8.143
  Apache-2.0; `piper-tts` 1.8.0 **GPL-3.0-or-later**; `openai` e `google-genai` Apache-2.0;
  `numpy`/`pillow`/`pydantic`/`fastapi` permissivas; FFmpeg LGPL ou GPL conforme o build
  (raciocinado). Mesma lição do `voz.rs` (espeak-ng GPL): embutir ou baixar pelo nosso instalador
  é decisão de produto; o operador instalar à parte não é.
- **Instalação (medida onde deu, sem instalar):** árvore 92 MB; `package-lock` do Remotion =
  **244 pacotes npm** (`remotion` 1,77 MB e `@remotion/cli` 0,51 MB desempacotados; HyperFrames
  34,4 MB); maiores rodas Python: `piper-tts` 34,1 MB, `numpy` 18,5 MB, `pillow` 7,2 MB,
  `openai` 2,2 MB, `google-genai` 1,2 MB (tamanho de roda, não instalado). **Não medidos:**
  `node_modules` completo, Chrome do Remotion, venv completo, modelo de voz do Piper, modelos de GPU
  (`requirements-gpu.txt`), FFmpeg. O que decidiria: um `du` numa máquina descartável do operador
  depois de `make setup`.

## 3. Desenho (a) — integração externa

**Princípio:** o PhxClaw não *contém* o OpenMontage nem o *importa*; **empresta o cérebro e a
cerca**. O LLM do PhxClaw faz o papel do «assistente de código» que o OpenMontage espera, dentro
do bwrap, lendo o checkout do operador **no lugar**, só leitura. É o mesmo molde do `voz.rs`: motor
externo no bwrap, binário e modelo só leitura, saída numa pasta temporária conferida antes de
entrar na tarefa.

| Peça | Desenho | Reaproveita (não duplica) |
|---|---|---|
| Instalação | Do operador: `git clone` + `make setup`, **fora** do repositório PhxClaw; `video.openmontage_dir` na config (variável `PHXCLAW_VIDEO_OPENMONTAGE`); vazio = ferramenta ausente | `config::texto_de` |
| Capacidade | `media.video` (a família `media.*` já tem `media.tts`, `media.stt`, `media.generate`; o exemplo do pedido era `video.render`). **Fora de `CAPACIDADES_PADRAO`**; protegida (aprovação humana, como `device.command`) | portão único `Agent::call_tool` |
| Ferramenta | `video_render{brief≤4000, duracao_s, modo, orcamento_usd, pipeline?}`; `pipeline` só vale se for nome de arquivo existente em `pipeline_defs/` do checkout (listado, não copiado) | `ToolSpec`, `esquema.rs` |
| Preflight | `make preflight` e `make hyperframes-doctor` no bwrap → o JSON do menu de provedores vira a resposta de «o que dá para fazer agora». Sem modelo, sem custo | `isolado_com` |
| Execução | Um **subagente** (`rodar_filhas`) só com `shell`+`read_file`; o contexto-de-sistema é **nosso** («leia `AGENT_GUIDE.md` em /om e atenda ao brief»), sem citar texto deles | `parallel_research`/`team_delegate` |
| Sandbox | `ro_binds`: checkout → `/om`; `env`: `OPENMONTAGE_PROJECTS_DIR=/work/projects` (variável que o `lib/paths.py` honra), `HOME` e cache em `/work`; pasta da tarefa lida/escrita | `SandboxExtras` |
| Segredos | O broker concede **por NOME** (`phxclaw video chave <PROVEDOR>`), concessão curta, só dos provedores que o `modo` e o `orcamento_usd` autorizam, injetados em `env` do bwrap (nunca em argv: `/proc/<pid>/cmdline` é público). Nunca `.env`: o clone fica só leitura e o preflight recusa um `.env` com valor | `SecretBroker`, `scrub_text` na saída |
| Entrada | brief + (opcional) pasta `midia/` da tarefa com material do próprio operador | `confine` |
| Saída | `renders/*.mp4` + `cost_log.json` + checkpoints, achados por glob **dentro de /work** (o código deles não fixa o caminho do `cost_log`); o MP4 só entra na pasta da tarefa depois da conferência do hospedeiro (§4 b5); ausência = erro, nunca «feito» | `voz.rs` (WAV truncado não chega ao usuário) |

**Três modos, do mais cercado ao mais solto:**

1. `local` — **sem rede** (`--unshare-net`, como hoje), sem chave: Piper + Remotion/HyperFrames +
   FFmpeg, só o material que o operador pôs em `midia/`. É o único modo **realmente dentro da
   cerca**. Pré-requisito do operador: ter feito o `npx`/Chrome **no setup**; offline o
   `npx --yes` falha (falha limpa, que é o certo).
2. `acervo` — rede ligada, chaves **gratuitas** (Pexels/Pixabay/Unsplash; archive.org, NASA e
   Wikimedia não pedem chave). Custo em dinheiro zero; custo em direitos autorais: §6.
3. `pago` — chaves de provedor pago, por nome (fal, ElevenLabs, Kling, Veo…). **Não entra agora.**
   Motivo medido no nosso código: `WorkdirCommand.network` é `bool` e o bwrap faz `--share-net` ou
   `--unshare-net`; **não há filtro por destino para processo-filho** (o `EgressBroker` filtra
   origem só para HTTP *dentro* do nosso processo). Com `--share-net` o filho — e o código que o LLM
   escreve nele, e o pacote que o `npx` baixa — alcança qualquer host com a chave na mão. O que
   destravaria: proxy CONNECT local por origem (lista = hosts dos provedores concedidos), medido
   numa bancada com provedor falso.

**Fluxo recomendado (`fluxos.rs`, nenhum nó novo):** `preflight` → `proposta` (subagente roda **só
até a etapa de proposta** do pipeline: roteiro, custo estimado, lista de fontes) → `esperar`
(aprovação humana mostrando estimativa e fontes) → `producao` (com `teto_ms` e orçamento reservado)
→ `conferir` (hospedeiro) → entrega. É o «checkpoint humano entre etapas» deles, mas com o
`esperar` que **sobrevive a reinício** (descarrega para o disco).

**Custo — como casar com R2/R3.**
- R2 mede **tokens × tabela do operador, uma moeda por tabela, «sem preço = não medido, nunca
  zero»**. R3 corta **depois** da chamada que estourou (excesso máximo = 1 chamada). Para LLM serve;
  para geração de vídeo paga (US$ 1 a 5 por clipe/filme, números do README deles: US$ 1,33, ~4, ~5 —
  **citados, não medidos**) uma chamada pode ser o orçamento inteiro.
- Então o orçamento do `video_render` é **reservado antes** (debita `orcamento_usd` da conta da
  tarefa, devolve o que sobrar): extensão de R3, §4 b3.
- O `cost_log.json` deles entra como **custo declarado pelo convidado, não conferido** (H3: quem
  escreve é o mesmo LLM que gasta). A moeda dele é dólar fixo; se a tabela do operador for outra
  moeda, o total do vídeo fica **«não medido»** (não se converte câmbio).
- O limite que de fato segura está **fora do convidado**: chave com teto na conta do provedor
  (raciocinado; a bancada decide), concessão que expira, `teto_ms`, e só conceder chave de provedor
  **que tem preço no arquivo do operador** (sem preço = sem chave).
- Os tokens do subagente entram pelo R2/R3 de sempre, **somados** à tarefa-mãe.

**Tempo e teto.** Declarado por eles: 10–60 min por pipeline (sem leitor em código). Nosso:
`teto_ms` do passo, padrão proposto 30 min, configurável (o fluxo já tem teto de uma hora). **Tempo
real não medido** — decide o piloto (§8).

**Roda sem chave?** Sim: `local` (e `acervo` com as fontes sem chave). **Mas só com modelo forte de
contexto longo** (§1); com o modelo local pequeno do dono, não.

## 4. Lista (b) — inspiração re-decidida

Régua: «onde DIVERGIRIA, e por qual restrição nossa». Sem divergência = passou pelos dedos.

| # | Ideia deles | O que o PhxClaw tem (medido) | Veredito e **divergência causada por restrição nossa** |
|---|---|---|---|
| b1 | Cada etapa **produz um artefato com esquema** e a seguinte exige o anterior | `fluxos.rs`: DAG, `depende`, retomada com `fluxo_sha256`; **saída de passo = lista JSON sem contrato** (o esquema só confere `args` na entrada, pelo portão) | **ENTRA como ⏸.** `saida_esquema` opcional no passo, validado pelo **`esquema.rs` que já existe** (todos os erros de uma vez, com caminho; não se escreve outro validador). Diverge: (i) opcional, porque «fluxo antigo continua rodando igual» e a assinatura `fluxo_sha256` só muda para quem usar; (ii) a violação **falha o passo e entra em `tentativas`** do grafo, e não «devolve ao diretor» — o nosso passo pode ser ferramenta determinística, sem LLM para reescrever; (iii) o esquema é do operador, não nosso. Premissa a medir: frequência de passo de agente que devolve JSON fora do formato — **não medida** |
| b2 | Política de aprovação por fluxo (`guided`/`manual_all`/`auto_noncreative`) e etapas com aprovação padrão | `esperar` explícito (descarrega para o disco); capacidade protegida exige aprovação no servidor | **PARCIAL, divergente.** A deles é imposta na gravação do checkpoint, mas **auto-atestada** (o agente passa `human_approved=True`) e não cobre o gasto. Aqui a aprovação nasce no **portão**: ferramenta marcada `gasta`/`publica` é recusada sem aprovação registrada, **qualquer que seja o desenho do fluxo** (restrição: portão único — «fluxo com portão próprio é a segunda cópia da política»). Aproveitável: marcar `gasta:true` no passo para o fluxo inserir o `esperar` sozinho. Baixo valor isolado |
| b3 | **Estimar → reservar → conciliar → estornar**; limiar por ação; aprovação do **primeiro uso pago** de cada ferramenta | R2 (preço do operador, «não medido ≠ zero»), R3 (três camadas; corta **depois**) | **ENTRA — a única ideia da lista que muda um número nosso.** `custo.precos_ferramentas` (arquivo do operador, com data e fonte, como o de modelos) + **reserva antes** da chamada paga + aprovação do primeiro uso por ferramenta. Divergências: (i) **preço não mora em código** (eles têm 89 `estimate_cost` digitados); (ii) **fica no portão**, não numa skill (H3); (iii) **sem preço = recusa na criação**, como o R3 já faz com modelo; (iv) a conciliação é **nossa** (nós contamos a chamada) e o `cost_log` do convidado só audita |
| b4 | **Proveniência por ativo** (provedor, URL original, licença) obrigatória no manifesto | ADR-0060 (firewall de proveniência/licença; estado «overlay / portões nativos pendentes»), `phxclaw-provenance-core` | **ENTRA reaproveitando o firewall**, sem esquema novo. Diverge: licença é **texto declarado, nunca booleano «livre»** (a NASA diz «domínio público com ressalvas»; nossa régua: «declarada, não validada»); ativo sem licença = «não medido» e **`site.publish` recusa** |
| b5 | Verificação de entrega antes de apresentar (ffprobe, amostra de quadros, nível de áudio, «promessa de entrega») | `voz.rs`: WAV conferido no hospedeiro antes de entrar; `visao.rs` usa ffmpeg | **ENTRA como conferidor de MP4 no hospedeiro** (caixas `ftyp`/`moov`, tamanho, duração por `ffprobe` do operador ±10% do pedido; sem `ffprobe` = «duração não conferida»). Diverge: eles se **autoavaliam** (o LLM que produziu escreve o `final_review`); aqui o convidado é **não confiável** (restrição: sandbox, e «saída de terceiro só entra depois de conferida») |
| b6 | `tools_available` por etapa (menu de ferramentas por etapa) | subagente fica na interseção do papel com o pai; capacidades por passo, pelo portão | **JÁ EXISTE** (equivalente). Nada a fazer |
| b7 | Seleção de provedor **pontuada em 7 dimensões** com log de decisão (`scoring.py`, 591 linhas, pesos à mão: ex. `task_fit` 0,30) | `roteamento.rs`, arena de modelos (medida), um provedor de imagem por variável | **RECUSADO com número.** Pesos digitados e não medidos; 62 arquivos de ferramenta de API a manter contra **5** provedores de geração de mídia nossos (imagem: openai-compatível, ComfyUI, Nano Banana; voz: local, ElevenLabs). O operador escolhe o provedor; o que medimos é a arena |
| b8 | Guarda de rede **nos testes** (bloqueia `connect` fora de loopback, para a suíte nunca gastar dinheiro) | `voz_e_midia.rs` usa «servidor falso» em loopback para openai/comfyui | **CONVERGE** (mesmo fim, outro meio). Sem guarda global de soquete na suíte: **⏸** — só vale quando houver teste que possa chamar provedor pago de verdade |
| b9 | Quadro vivo (Backlot) derivado dos arquivos do projeto | live-bus, UI de tarefas | **RECUSADO.** Tela nova sem pedido; o live-bus já publica o estado da tarefa |
| b10 | Análise de vídeo de referência (baixar YouTube/TikTok/Reels com `yt-dlp`) | `web.browse`, egress por origem | **RECUSADO.** Direitos e termos das plataformas (§6) + dependência com runtime Deno; nenhuma pétrea precisa dele |
| b11 | «Retrieval-first»: montar corpus de acervo com busca por CLIP e preencher *slots* de um roteiro temático | memória/índice local (`phxclaw-memory-context`) | **NÃO ABSORVER agora.** O CLIP nem está no `requirements.txt` deles (opcional); a nossa rede neural local é CPU e de centenas de MB. Reavaliar só se o piloto mostrar que o acervo é o gargalo |

## 5. Recomendação (decidida pelo pesquisador, pétrea «o pesquisador decide»)

Só os itens 1 e 2 não dependem de o dono querer vídeo:

1. **Porta de licença no `phxclaw skills importar`** (e em todo importador de diretório externo):
   recusa quando o `LICENSE` da raiz de origem for AGPL/GPL ou ausente, dizendo o nome da licença;
   passar por cima exige flag explícita, registrada na evidência. **Defeito ativo** (copia texto
   alheio para dentro do produto): entra na conta do que falta. Teste com prova real: importar uma
   árvore com `LICENSE` AGPL **passa hoje**; tem de falhar depois.
2. **b3 em R3** (reserva antes + preço de ferramenta no arquivo do operador). Vale para ElevenLabs
   e Nano Banana **que já existem**, sem o OpenMontage.
3. **b5** (conferidor de MP4 no hospedeiro) e **b4** (licença do ativo bloqueia publicação).
4. **Piloto (§8) antes de qualquer código de `media.video`**: sem ele o valor de (a) é palpite.
5. **Só então `media.video`**, modos `local` e `acervo`, nascendo **desligado** e **⏸ «depois da
   versão»** na conta do que falta (capacidade nova, não defeito). `pago` fora até existir o
   proxy por origem (§3).
6. b1 e b2 como ⏸; b7, b9, b10, b11 **recusados** (números na tabela).

## 6. Riscos

| Risco | Medido / raciocinado | Mitigação |
|---|---|---|
| **Custo descontrolado** | Medido: custo e tempo são prosa para o LLM (zero leitores em código); 62 ferramentas de API; 35 linhas de chave | Reserva antes, preço do operador, chave só com preço, teto na conta do provedor, `pago` desligado |
| **Direitos autorais do material** | Medido: licença é texto livre por fonte; `video_downloader` baixa de YouTube/TikTok/Reels; NASA «com ressalvas». Raciocinado: Pexels/Pixabay/Unsplash têm licenças próprias que **não** são domínio público; acervo histórico pode ter direitos de terceiros (pessoas, marcas) | `yt-dlp` fora (o operador não concede a ferramenta de download); b4 (licença registrada; vazia = não publica); aviso de que a licença é **declarada pela fonte, não validada por nós** |
| **Rede aberta + chave no ambiente** | Medido: bwrap sem filtro por destino; `npx --yes` executa pacote baixado na hora | `local` offline; `acervo` só com chaves gratuitas e **concessão curta**; `pago` bloqueado; setup (com rede) separado do uso |
| **Injeção por conteúdo** | Raciocinado: o agente lê páginas e metadados de acervo e roteiros de ~300 KB | `instrucoes::varrer` para o que vier de fora ao contexto; subagente só com `shell`+`read` |
| **Contaminação por AGPL** | Medido: importador sem porta de licença | §5.1 e as três proibições de §2 |
| **Licença do Remotion** | Medido no `LICENSE.md`: empresa com mais de 3 funcionários paga | O preflight mostra o aviso; a licença é do operador |
| **Saída com marca deles** | Não medido (mascote «Monty» aparece em vídeos de vitrine do README; não achei menção em `remotion-composer/src`) | Conferir no piloto |
| **Modelo fraco segue mal um roteiro de ~76 mil tokens** | Raciocinado | O piloto mede; recusar `media.video` abaixo de um contexto mínimo configurável |

## 7. O que sobe ao dono (só estes, pela pétrea)

- **Choque com pétrea nossa:** nenhum. A AGPL **não** choca com a Apache-2.0 do PhxClaw enquanto a
  fronteira for processo; o desenho (a) é feito para isso.
- **Decisão de produto (uma linha, sem recomendar):** *se o dono quiser o OpenMontage **dentro** do
  produto (embarcado, importado ou no instalador), a única via é relicenciar o PhxClaw (AGPL-3.0 ou
  compatível) — decisão dele; a licença paga do Remotion, para empresa com mais de 3 pessoas, é
  custo dele ou do cliente.*
- **Produto:** oferecer vídeo gerado ao cliente (preço, SLA, quem arca com provedor pago e com
  direito autoral do acervo).
- **Fora do que a pesquisa alcança:** parecer jurídico sobre AGPL — aqui é raciocínio sobre a FAQ
  da FSF, não aconselhamento legal. Se a fronteira de processo for decisiva para cliente
  empresarial, vale leitura de advogado.

## 8. Lacunas e piloto (nada rodou)

**Não medido:** tempo real de um vídeo; tokens do roteiro; tamanho instalado (`node_modules`,
Chrome, venv); se o Remotion/Chrome roda **dentro do bwrap** neste host (precedente:
`voz_e_midia.rs` roda o Chromium no sandbox para o canvas, com `chromium_e_node()` como
pré-condição); se a licença de cada fonte de acervo vem preenchida; marca d'água.

**Piloto (decide o §5.5), numa máquina descartável do operador, `pesquisa-bancada`:** instalar o
checkout; `make preflight`; pedir ao PhxClaw, em `local` e depois em `acervo`, um
`documentary-montage` de 30 s com 5 clipes; registrar tokens de entrada e saída do subagente,
minutos de relógio, MP4 válido por `ffprobe`, `cost_log.json` presente ou ausente, hosts acessados
(`ss`/log de proxy) e pastas gravadas fora de `/work`. Aceite: MP4 válido, zero gravação fora de
`/work`, zero host fora da lista, custo declarado zero no `local`.

## 9. Matriz de evidência (fonte → o que resolve → custo)

| Fonte | O que resolve | Custo de obter |
|---|---|---|
| `LICENSE`, `README.md` (seção License), `CONTRIBUTING.md` (OpenMontage @ `9327439`) | AGPL-3.0 | ler |
| `tools/cost_tracker.py` + busca de usos em `tools/`, `lib/` | H3: custo é prosa | ler + grep |
| `lib/checkpoint.py` (portão GI-4), `lib/pipeline_loader.py` | aprovação imposta mas auto-atestada | ler |
| `config.yaml`, `pipeline_defs/*.yaml` (+ busca de `max_wall_time` em `.py`) | padrões que discordam; campos sem leitor | ler + grep |
| `tools/base_tool.py`, `lib/paths.py`, `Makefile`, `render_demo.py` | contrato de ferramenta, variável da pasta de projetos, únicos comandos | ler |
| <https://raw.githubusercontent.com/remotion-dev/remotion/main/LICENSE.md> | licença do Remotion (≤3 funcionários) | 1 requisição |
| <https://www.gnu.org/licenses/gpl-faq.html> (`#MereAggregation`, `#GPLPlugins`) | fronteira processo × programa único | 1 requisição |
| PyPI / registro npm (JSON) | licenças e tamanhos de roda/pacote | 1 requisição por pacote |
| PhxClaw: `fluxos.rs`, `custo.rs`, `orcamento.rs`, `voz.rs`, `sandbox/lib.rs`, `importar_skills.rs`, `esquema.rs`, ADR-0060 | o que já existe e onde diverge | ler |

*Nenhum código escrito, nenhum arquivo de produto tocado, nada comitado.*

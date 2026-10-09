# SP000032 — R5 (minimapa e dobra no IDE) e R1 (RSA para JWT RS256): parecer do pesquisador

Papel J, 02/10/2026. Decisão do pesquisador pela pétrea «o pesquisador decide; o dono é o impasse».
Hipóteses escritas antes de medir; a que morreu fica registrada com o número. Nada aqui foi
compilado (disco em 1,1 GB livres): o que é medido é medido no fonte, no diff e nos vetores; o que
é raciocinado está dito como raciocinado.

Scratch usado e apagado ao fim: clone esparso do Helix no commit fixado (5,6 MB), dois diffs de PR
(299 KB e 84 KB), `rsa_signature_2048_sha256_test.json` (211 KB), `186-3rsatestvectors.zip` (4,6 MB).

---

## R5 — minimapa e dobra de código no IDE

### Hipóteses, escritas antes de medir

- **(a) patch próprio no Helix**, aplicado pelo `tools/instalar_helix.sh` no commit fixado
  `a05c151` (tag 25.07.1), com teste de que aplica limpo.
- **(b) painel no IDE web** (`apps/phxclaw-ui/assets/ide.js`): minimapa em canvas do texto do
  arquivo; dobra via `textDocument/foldingRange` pelo `lsp.rs` do agente.
- **(c) declarar fora**, com o motivo medido.

### O que o Helix 25.07.1 tem (medido no fonte, clone esparso em `a05c151bb6e8e9c65ec390b0ae2afe7a5efd619b`)

| Medida | Número |
|---|---|
| Ocorrências de `fold`/`folding`/`folds` como palavra em `helix-term`, `helix-view`, `helix-lsp`, `helix-core` | 12 + 0 + 0 + 2 — **todas** são `Iterator::fold` (ex.: `helix-term/src/commands.rs:3425`); zero dobra de código |
| Ocorrências de `minimap` nos quatro crates | **0** |
| `runtime/queries/rust/` | `highlights`, `indents`, `injections`, `locals`, `textobjects` — **não há `folds.scm`** |
| Capacidade LSP `foldingRange` no cliente (`helix-lsp/src/client.rs`, 1.554 linhas) | **0** ocorrências |
| Statusline padrão (`helix-view/src/editor.rs:519-531`) | esquerda `Mode, Spinner, FileName, …`; direita `…, Position, FileEncoding` |
| Calha padrão (`helix-view/src/editor.rs:102`) | `GutterType::LineNumbers` ligada |

A evidência do `phxclaw.json` («Helix 25.07.1 nao tem dobra de codigo») confirma-se no fonte.

### Upstream (fonte primária: github.com/helix-editor/helix)

| Item | Estado | Número |
|---|---|---|
| Issue #1840 «Add Code Folding» | aberta desde 18/03/2022 | sem PR vinculado na seção *Development* |
| PR #14593 «Code folding» (DubrovinEIu) | **aberta** desde 13/10/2025, 17 commits, última atividade 25/11/2025, sem revisão de mantenedor visível | diff `patch-diff.githubusercontent.com/raw/helix-editor/helix/pull/14593.diff`: **40 arquivos, +6.314 / −390**; `helix-term/src/commands.rs` em **27 hunks**; mexe em «30+ comandos de movimento e seleção» |
| PR #16305 «Add code folding» (glapa-grossklag) | **fechada** em 22/09/2026 pelo autor («closing while workshopping»), cita #1840 e #14593 | 16 arquivos, +1.467 / −46 |
| Issue #2210 «Minimap and Scrollbar» | aberta desde 21/04/2022, rótulo `A-gui`, 24 reações | **nenhum PR** |

**`git apply --check` dos dois PRs no commit fixado (medido):**

| PR | Resultado | Arquivos que falham |
|---|---|---|
| #14593 | **não aplica** — 16 erros em 7 arquivos | `helix-core/src/syntax.rs`, `helix-term/src/commands.rs`, `helix-term/src/ui/editor.rs`, `helix-term/src/ui/mod.rs`, `helix-term/tests/test/commands.rs`, `helix-view/src/document.rs`, `helix-view/src/editor.rs` |
| #16305 | **não aplica** — 8 erros em 4 arquivos | `helix-core/src/syntax.rs`, `helix-term/src/commands.rs`, `helix-term/tests/integration.rs`, `helix-view/src/document.rs` |

### O que o IDE já tem (medido no nosso fonte)

- `ide.js` (499 linhas) recebe a grade do terminal (`linhas_alteradas: Vec<LinhaGrade>` com
  `texto`, `phxclaw-terminal/src/lib.rs:115`) e **já raspa a statusline** para saber o arquivo
  aberto (`arquivoDaGrade`, `ide.js:204-215`, modo `NOR|INS|SEL` + nome).
- A trilha já pede `GET /v1/ide/simbolos?arquivo=` (`ide.rs:12`, `lsp.rs:373`
  `simbolos_do_arquivo`, sessão `ide` compartilhada, `documentSymbol` hierárquico) e **já manda
  comando ao Helix** (Esc, `:goto`, Enter — `ide.js:239`).
- `phxclaw-snippet-ls` (`lib.rs:202-210`) anuncia **só** `textDocumentSync` e
  `completionProvider` — **não expõe `foldingRange`**. O `rust-analyzer`/`pyright` expõem, pela
  mesma sessão `ide` do `lsp.rs`.

### Decisão

**Minimapa → hipótese (b), painel no IDE web.** Custo raciocinado (não medido; calibrado pela
trilha, que é a mesma arquitetura): ~40 linhas Rust (`GET /v1/ide/arquivo?arquivo=` no `ide.rs`,
mesmo portão e mesma `relativo()` do `simbolos`) + ~150 linhas JS (canvas 1 px por caractere por
linha, retângulo da janela visível, clique → `:goto N` pelo caminho da trilha) + teste do endpoint
(recusa fora da raiz, como o `simbolos`). **Zero dependência nova, zero toque no Helix.** A janela
visível sai do **número da calha** na primeira linha da grade (calha ligada por padrão); o cursor
sai do `Position` da statusline. Risco nomeado: usuário com `line-numbers = "relative"` quebra a
raspagem da calha — o painel cai para «só cursor» em vez de mentir, e isso é teste.

Divergência da origem (VS Code desenha o minimapa do buffer do próprio editor): aqui o buffer é de
**outro processo** (Helix no PTY); a restrição nossa é «Helix separado e sem modificação»
(`instalar_helix.sh`, cabeçalho) — por isso o minimapa lê o arquivo pelo agente e a janela pela
grade, e **não** reflete texto não salvo. Esse limite se declara na evidência do `phxclaw.json`.

**Dobra de código → hipótese (c), declarar fora — e o `phxclaw.json` fica `nao` com o motivo
medido**, não `parcial`. Três números matam as outras duas:

1. (a) morreu: nenhum PR de folding aplica em 25.07.1 (7 e 4 arquivos em conflito), nenhum foi
   aceito pelos mantenedores, e o mais completo tem **+6.314 linhas em 27 hunks do `commands.rs`**.
   Patch dessa envergadura é um fork — quebra a cada atualização da `TAG` e revoga a decisão
   escrita no `instalar_helix.sh` («PROCESSO SEPARADO e SEM MODIFICACAO», o que mantém a MPL-2.0
   longe do nosso Apache-2.0). Patch pequeno de minimapa em TUI também morre por aqui: zero código
   upstream para se apoiar, e o resultado em célula de terminal (braille 2×4) é pior que o canvas
   que já temos.
2. (b) não dobra: `foldingRange` devolve **faixas**, mas quem esconde linhas é o editor, e o Helix
   não tem o comando. Um painel que colapsa nós já existe — é a trilha hierárquica
   (`hierarchicalDocumentSymbolSupport: true`, `lsp.rs:290`). Chamar isso de dobra seria número
   mentindo na porcentagem.
3. O caminho barato aparece sozinho quando o upstream fechar: **subir a `TAG`** do
   `instalar_helix.sh` (um número, um commit) — zero linhas nossas. Até lá, `nao` com esta URL.

**`remoto_ssh_containers`** (fora do pedido ao pesquisador): fica como a sprint diz, prova contra
`sshd` local — sem hipótese a decidir aqui.

### Hipótese que morreu, com o número

- (a) patch no Helix: **+6.314/−390 em 40 arquivos, 7 arquivos em conflito com 25.07.1, 0 PRs
  aceitos em 4 anos de issue aberta.**

### Execução do minimapa (papel E, 09/10/2026)

- Rota `GET /v1/ide/arquivo?caminho=` em `ide.rs` (aceita também `arquivo=`): Bearer, o mesmo
  `tarefa::confine` do `simbolos`, teto `TETO_DO_ARQUIVO` de 2 MiB (413), binário recusado (415).
- Painel `#ideMinimapa` em `ide.js`. **Limite declarado na tela e aqui:** o mapa é o arquivo
  **em disco**; texto não salvo não aparece — com `[+]` na linha de estado o painel avisa, e
  ao salvar ele relê sozinho.
- **Correção de nome:** a chave do Helix é `line-number` (singular), não `line-numbers`
  (medido: `:set line-numbers relative` não muda nada no 25.07.1; `:set line-number relative`
  muda). Com ela relativa, ou a calha desligada, o painel cai para «só cursor»: a faixa só se
  desenha se os números da calha forem crescentes, alinhados na mesma coluna e contiverem o
  cursor da linha de estado.
- Provas: `crates/phxclaw-agent/tests/ide_web.rs` (rota, RED do `../`) e
  `tests/desktop/ide_minimapa.mjs` (agente e Helix reais, `out/ide_minimapa.json`).

---

## R1 — RSA PKCS#1 v1.5 + SHA-256 para conferir JWT RS256 (Teams, Google Chat)

### Hipóteses, escritas antes de medir

- **H1** reaproveitar a aritmética do `bip340.rs` (campo de 256 bits) para RSA 2048/4096.
- **H2** escrever uma verificação só (sem assinar): inteiro longo de tamanho variável, `modexp` com
  `e = 65537`, comparação EMSA-PKCS1-v1_5, JWKS com `n`/`e` em base64url.
- **H3** usar o `ring`, que talvez já esteja na árvore de dependências (então «zero crate nova»).

### O que a casa já tem (medido com `grep`/`Cargo.lock`)

| Peça | Onde | Serve para RSA? |
|---|---|---|
| SHA-256 | crate `sha2` (`cripto.rs:7,28`); «escrito aqui» são o HMAC e o SHA-1, não o SHA-256 | sim, `cripto::sha256` |
| HMAC-SHA256 contra RFC 4231 | `cripto.rs:122` | padrão de prova a copiar |
| Inteiro de 256 bits `[u64; 4]`, soma/sub/produto escolar, `reduzir` bit a bit (`bip340.rs:160`), `pot_p` | `bip340.rs` (445 linhas) | **não como está**: largura fixa em 4 limbs e redução especial de `p = 2^256 − C`. O **método** (produto escolar + redução por deslocamento) generaliza para `Vec<u64>` |
| base64url sem padding | `oauth.rs:93` (`URL_SAFE_NO_PAD`, crate `base64`) | sim |
| JSON | `serde_json` | sim (JWKS, cabeçalho e corpo do JWT) |
| `ring` 0.17.14 | **alcançável** pelo `phxclaw-agent` (via `rustls-webpki`/`quinn-proto`), 515 crates na árvore; `aws-lc-rs` 1.18.1 também está | H3 é tecnicamente possível |
| `num-bigint` | alcançável (via `num`/`num-rational`) | idem |
| crate `rsa` | **ausente** | — |

### O que os dois serviços exigem (fonte primária)

**Teams / Bot Framework** —
`learn.microsoft.com/en-us/azure/bot-service/rest-api/bot-framework-rest-connector-authentication`
(atualizada 01/09/2026), seção *Authenticate requests from the Bot Connector service to your bot*:

1. `Authorization: Bearer`; 2. JWT válido; 3. `iss = https://api.botframework.com`; 4. `aud` = App
ID do bot; 5. validade com tolerância de **5 min**; 6. assinatura com chave do JWKS e algoritmo do
`id_token_signing_alg_values_supported` (= `RS256`); 7. claim `serviceUrl` **igual** ao
`serviceUrl` da Activity; endorsements por `channelId` → **403** se faltar; JWKS recarregado **ao
menos a cada 24 h**. «Implementers shouldn't expose a way to disable validation.»
Medido em `https://login.botframework.com/v1/.well-known/keys`: **225 chaves**, todas `RSA`
**2048 bits**, `e = AQAB` (65537), com `x5c` e `endorsements`. O caminho do Emulador (outro
issuer, `login.microsoftonline.com/common/discovery/v2.0/keys`) fica **fora** desta onda.

**Google Chat** — `developers.google.com/workspace/chat/verify-requests-from-chat`: `Authorization:
Bearer`; `iss = chat@system.gserviceaccount.com`; `aud` = número do projeto (JWT) **ou** URL do
endpoint (ID token OIDC); resposta **401** se falhar. Medido:
`https://www.googleapis.com/service_accounts/v1/jwk/chat@system.gserviceaccount.com` → **4
chaves** `RSA` 2048, `alg RS256`, `e = AQAB`; `https://www.googleapis.com/oauth2/v3/certs` → 200.

Os dois usam **2048 bits e e = 65537 hoje**; a implementação aceita até 4096 (limbs variáveis)
porque o JWKS é deles e pode mudar amanhã.

### Vetores oficiais (medidos, baixados)

| Fonte | Conteúdo | Uso |
|---|---|---|
| Wycheproof `testvectors_v1/rsa_signature_2048_sha256_test.json` (C2SP, 211 KB) | **259** casos, 3 grupos, 2048/SHA-256/e=65537: **9 válidos, 249 inválidos, 1 «acceptable»**; flags: `InvalidAsnInPadding` 117, `ModifiedPadding` 75, `WrongHash` 21, `BerEncodedPadding` 14, `InvalidSignature` 8 | a prova do EMSA (comparação **byte a byte** do `EM` inteiro, nunca «acha o hash e confere»); o «acceptable» se trata como inválido — DER estrito |
| NIST CAVP `186-3rsatestvectors.zip` → `SigVer15_186-3.rsp` | mod 1024/2048/3072 × SHA-1..512; **SHA-256: 18 casos por módulo (3 P, 15 F)** | a prova do primitivo (`RSAVP1` + `EMSA`) nos três tamanhos — os de 1024 só nos testes |
| RFC 8017 | **não traz vetores** (apêndices A–C são ASN.1 e notas); é a norma (§5.2.2 RSAVP1, §8.2.2 RSASSA-PKCS1-v1_5-VERIFY, §9.2 EMSA-PKCS1-v1_5, DigestInfo SHA-256 de 19 bytes na nota 1 de §9.2) | a referência de escrita |

### Custo de uma verificação (raciocinado; o que decidiria na bancada é um `--example` com a chave do JWKS real)

`e = 65537 = 2^16 + 1` → **16 quadrados + 1 produto = 17 multiplicações modulares**. Com 32 limbs:

| Redução | ops de limb por multiplicação | verificar (×17) | assinar com `d` de 2048 bits (~3.072 mult.) |
|---|---|---|---|
| produto escolar + bit a bit (o método do `bip340.rs`) | 132.096 | 2,2 M ops ≈ **~7 ms** | 406 M ops ≈ **~1,2 s** |
| produto escolar + Montgomery | 3.072 | 52 k ops ≈ ~0,2 ms | 9,4 M ops ≈ ~30 ms |

Piso medido (CPython, bigint em C, 1.000×): 0,19 ms por verificação; 34 ms por assinatura.
Um webhook paga uma verificação por mensagem; **~7 ms fica abaixo do RTT** do próprio webhook.

### Decisão

**Viável na onda seguinte, pela H2: verificação só, escrita aqui, zero crate nova.**

Estimativa de linhas (raciocinada, calibrada pelo `bip340.rs` = 445 linhas com testes para um
primitivo mais difícil):

| Parte | Linhas |
|---|---|
| inteiro longo `Vec<u64>`: de/para big-endian, comparar, somar/subtrair, produto escolar, redução bit a bit, `modexp` | ~110 |
| EMSA-PKCS1-v1_5: `EM = 00 01 FF…FF 00 ‖ DigestInfo(19 B) ‖ H`, comparação byte a byte do `EM` inteiro, `k = len(n)` | ~30 |
| JWK → (`n`, `e`) e cache do JWKS por `kid` com recarga de 24 h (e recarga imediata em `kid` desconhecido, uma vez) | ~60 |
| JWT: `split('.')`, base64url, `alg == "RS256"` **exigido** (recusar `none`/`HS256`), `iss`, `aud`, `exp`/`nbf` ±300 s, `serviceUrl` (Teams), endorsements por `channelId` | ~80 |
| testes: Wycheproof (arquivo em `tests/`), subconjunto NIST SHA-256 2048/3072 embutido, RED com o teste de padding desligado | ~100 |
| **total** | **~380 (±80)** |

Prova real nos dois sentidos: (1) os 9 válidos + 3 P do NIST passam; (2) com a comparação do
`EM` trocada por «procura o hash no fim» (o defeito clássico), os **117 `InvalidAsnInPadding` + 75
`ModifiedPadding`** têm de reprovar o teste — é o RED medido. Não é tempo constante, e isso se
declara: verificação usa só chave pública, como o `bip340.rs` já declara.

Divergência da origem: o `bip340.rs` reduz com módulo fixo; aqui o módulo vem do JWKS, e por isso
os limbs são variáveis — restrição nossa: a chave não é nossa. E a comparação do `EM` é do bloco
inteiro, não «parse ASN.1» como as bibliotecas fazem: é o que a flag `BerEncodedPadding` (14
casos) exige, e o que menos linhas custa.

### Hipóteses que morreram, com o número

- **H1** (`bip340.rs` como está): largura fixa `[u64; 4]` e `reduzir_p` com `C = 0x1000003D1`
  — **0 funções** reutilizáveis sem reescrever para `Vec<u64>`. Reaproveita-se o **método**, não o
  código.
- **H3** (`ring` já na árvore): alcançável, sim (`ring 0.17.14`, 2 dependentes transitivos), mas
  **nenhum deles é nosso** — a presença depende das features do `rustls`, que já traz o
  `aws-lc-rs` ao lado; e vira dependência **direta** nova no `Cargo.toml` do agente, que é o que
  a sprint chama de «crate nova». Recusada: 313 vetores oficiais tornam o oráculo desnecessário.
- **Montgomery** para verificar: compraria ~40× num custo que já é ~7 ms — **recusado nesta
  onda**, com uma condição escrita: **se a saída do Google Chat pela API (conta de serviço, que
  ASSINA RS256) entrar, o bit a bit custa ~1,2 s por token** (raciocinado), e aí ou entra
  Montgomery (~60 linhas) ou o token de 3.600 s se guarda — o que choca com a decisão escrita no
  `teams.rs` («o token de saida e pedido a cada envio e nao fica guardado»). Isso é da onda de
  assinatura, não desta.

### Lacunas

- O custo é raciocinado; o número de bancada sai de um `--example` que confira um JWT real do
  `login.botframework.com` (sem credencial: basta a chave pública e um token capturado).
- Teams: a lista de `channelId` que exige endorsement é configurável por bot; a onda decide o
  padrão (`msteams` obrigatório) e escreve.
- Google Chat: o modo «URL do endpoint» (ID token OIDC) usa `oauth2/v3/certs`; a onda escolhe um
  modo por configuração, nunca os dois sem dizer.

---

## Resumo

| Item | Decisão | Número que sustenta | Hipótese morta |
|---|---|---|---|
| R5 minimapa | painel no IDE web (b), ~190 linhas, zero dependência | Helix 25.07.1: 0 `minimap`; issue #2210 sem PR; a grade e a trilha já existem (`ide.js:204`, `lsp.rs:373`) | patch no Helix |
| R5 dobra | fora (c), `nao` com motivo; volta ao subir a `TAG` quando o upstream fundir | PR #14593 +6.314/−390, 40 arquivos, **não aplica** em 25.07.1 (7 arquivos); #16305 fechado; `foldingRange` não esconde linha | patch (a) e painel (b) |
| R1 RSA | viável, H2, ~380 linhas, prova Wycheproof 259 + NIST 18×3 | ambos os JWKS são RSA-2048 e=65537; 17 mult. modulares ≈ 7 ms (raciocinado) | `bip340.rs` como está (0 fn), `ring` (dep. direta nova), Montgomery (40× num custo que não dói) |

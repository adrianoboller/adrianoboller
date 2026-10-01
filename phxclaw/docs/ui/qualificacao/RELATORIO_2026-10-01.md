# Qualificação da UI do PhxClaw — medida em 01/10/2026

Árvore: `apps/phxclaw-ui` @ 5db73930 (cópia; o repositório não foi tocado). Chromium headless, 4 larguras × PT/EN × 8 telas (splash + 7) = 64 medições; 120 capturas em `cap/`. Roteiros e números brutos em `roteiros/` (`matriz.json`, `acess.json`, `estados.json`, `offline.json`, `perf.json`).
axe-core: **não existe** em /opt nem no projeto, e não foi baixado. As checagens equivalentes foram feitas por script: árvore AX pelo CDP, Tab real, contraste WCAG calculado, `getPlatformFontsForNode`.

## Achados (severidade · tela · largura · idioma · evidência → conserto)

### Bloqueia
B1 · Tarefas · todas · PT/EN · com Enter ou Espaço numa célula da grade o detalhe não abre (`detalhe aberto por teclado: false`; com clique, abre). Sem o detalhe não há como APROVAR, RESPONDER ou CANCELAR pelo teclado → linha da grade ativável por Enter/Espaço (`role=row` + `aria-selected`).
B2 · Tarefas/Agentes/Ferramentas/Config · 390 · PT/EN · a grade não é utilizável: altura mediana da linha de 185 px em Tarefas (coluna Objetivo com 83 px), 203 px em Agentes, 316 px em Ferramentas (máximo de 1.240 px) e 90 px em Config. Cabem 2 a 5 linhas por tela. Tarefas é a tela de partida do PWA → modo cartão abaixo de 640 px (uma linha = um cartão), ou colunas mínimas e objetivo com `min-width`.

### Grave
G1 · Todas · 390 · PT/EN · a barra inferior não cabe: o `nav` mede 422 px numa janela de 390 e não rola (`scrollWidth = clientWidth = 422`). «Configuração» fica em x 361–428, com o rótulo cortado e só 29 px tocáveis. O avatar «PO» também sai da tela → `.sidebar{min-width:0}` e `nav{width:100%;max-width:100vw}`, e o rótulo vira ícone + `aria-label` abaixo de 400 px.
G2 · Visão geral · 390 · PT/EN · rolagem horizontal na própria tela (`#tela-geral` com scrollWidth 499 e clientWidth 366). «v0.0.0-stub» e «FERRAMENTAS» saem do cartão. O painel Absorção está cortado à direita → `.metric-grid` em 2 colunas ≤640 px e `.dashboard-grid` em 1 coluna.
G3 · Splash · todas · PT/EN · o rodapé «AGENTES • IA • PESSOAS…» fica por cima da 3.ª fileira de módulos (719 px² a 1920, 1.056 px² a 390), porque `.boot-modules{height:42px}` é fixa e os chips quebram em 3 linhas. A 390 «CLAW» sai da tela → `height:auto` (ou `min-height`) e wordmark com `clamp()`.
G4 · Splash · todas · o foco do Tab entra em controle invisível: `#enterButton` (opacidade 0) e depois o app inteiro, que está com `aria-hidden=true` e opacidade 0 → `inert` no `#app` enquanto o splash cobre a tela, e `tabindex=-1` no botão até ele ficar `.ready`.
G5 · Barra do topo · todas · «CORE ONLINE», com o ponto verde pulsando, está cravado no HTML e aparece mesmo sem host (estado «sem_host»: WEB PREVIEW / NO NATIVE HOST). É mentira sobre o estado → derivar do `host_status` ou remover.
G6 · Barra do topo · todas · os botões ⌕ e ⌘ não fazem nada (clique não muda o DOM). O nome acessível deles é só o símbolo, e o glifo cai em FreeSerif/DejaVu → remover, ou ligar a uma ação com `aria-label` pela fábrica.
G7 · PWA sem rede, 1.ª visita · 390 · a casca do SW não guarda `equipe.json`, `ferramentas.json`, `absorcao.json` nem `fonte/exo2-latin.woff2`. Sem rede: «assets/equipe.json não existe — Gere com: cargo run…» (o arquivo existe, quem caiu foi a rede, e a tela manda rodar cargo), e a fonte fica com `Exo 2 error`, caindo no fallback. Na 2.ª visita os quatro já estão no cache e tudo abre → pôr os 4 no `CASCA` e subir o nome para `phxclaw-casca-3`. No `lerJson`, distinguir 404 de falha de rede.
G8 · Tarefas/Config · sem rede · «Falhou: Failed to fetch» aparece cru, em inglês, numa tela em PT, e não diz o que fazer. O mesmo com erro 500: «Falhou: falha interna simulada», sem botão de tentar de novo → mensagem por CHAVE para rede (`tarefas.sem_rede`: «Sem conexão com o agente — confira a rede; a tela tenta de novo em N s») e botão RECARREGAR (azul).
G9 · Config · todas · 143 controles sem nome acessível na árvore AX (117 caixas de texto, 14 caixas de marcar, 8 numéricos, 4 listas): são os editores da coluna Valor → `aria-label` = chave da linha (`acao.bin`).
G10 · IDE · todas · o Tab fica preso no canvas do terminal (`Tab` no `#termCanvas` volta para `#termCanvas`). O foco é uma sombra de 1 px com alfa .35, cerca de 2,2:1 sobre o fundo, abaixo dos 3:1 de indicador → atalho de saída documentado (Esc e depois Tab, ou Ctrl+Shift+Tab) e anel de 2 px #38d6ff igual ao do menu.
G11 · Visão geral e barra · 1920–390 · 953 dos 3.986 nós de texto têm menos de 12 px, e 210 têm menos de 10 px. Há 4 regras de 6 px, 9 de 7 px e 8 de 8 px no CSS (rodapé de 7 px, `.policy` de 7 px, cartão ZERO TRUST de 7/8 px, `.cap-list em` de 6 px). Passa no contraste e falha na leitura → piso de 11 px (12 px no corpo) e escala 11/12/13/15/21/25/37.

### Médio
M1 · Rodapé · todas · 3,0:1 (#42647b sobre #07121d, 7 px), abaixo dos 4,5 exigidos → #7894aa (`--muted`) e 11 px.
M2 · Splash · ≤1366 · «AGENTES • IA…» com 2,83:1 (#4e7187, 10 px) → #7894aa.
M3 · Tarefas (paginação) · todas · «« ‹ › »» com 4,17:1 (#6d7985 sobre #071521) → clarear para #8a96a3.
M4 · Splash · todas · a barra de progresso é um temporizador (15 × 190 ms + 420 ms) que lista «Connectivity • HTTP + PostgreSQL + Ollama» sem medir nada, e segura a primeira tela útil: 3,4 s no desktop e 7,6 s no celular (4G lenta, CPU 4×). O `start_url` do PWA passa por ele toda vez → pular o splash quando há `?tela=`/hash ou no modo standalone, ou ligar as etapas a eventos reais.
M5 · Todas · o phx-grid (563.582 B, 145.247 B em gzip) é síncrono no fim do body: no celular o DOMContentLoaded vai de 1.553 para 4.274 ms com ele, +2,7 s. A Visão geral não usa grade → carregar `phx-grid.js/css` sob demanda na 1.ª tela com grade. **Vale.**
M6 · Todas · nenhuma `prefers-reduced-motion`: órbitas, pulso e marca flutuando giram sem fim → bloco `@media (prefers-reduced-motion: reduce)` que zera as animações.
M7 · Tarefas/Config · as mensagens `#tarefasStatus` e `#configStatus` não estão em `aria-live`. O único `aria-live` da página é o `#eventConsole` → `role="status"` nas duas; e `role="alert"` quando é erro.
M8 · Tarefas · carregando · `#tarefasStatus` fica vazio enquanto a API não responde (as outras telas dizem «Lendo…») → `geral.lendo` também ali.
M9 · Cores da ação · «VOLTAR AO PADRÃO» (Config) está em vermelho/exclui, mas não apaga nada de vez: é um desfazer → rosa (marca) ou amarelo. O tom rosa nem existe no CSS (`.acao.marca` ausente). «FECHAR TERMINAL» em vermelho pode ficar: encerra um processo.
M10 · Marca · o fundo do shell é o gradiente #07121d→#06101a→#08141f e o do splash é #020910→#07131f; nenhum dos dois é #010418. O `theme-color` é #07111c. São 161 hex distintos no app.css contra 13 tokens e 27 `var(--)`, e 47 cores de texto em uso, só 8 delas token → shell e splash sobre `var(--bg)`, e consolidar a paleta em tokens (lote próprio).
M11 · Ícones do menu e do topo · ◫ ◉ ⌨ ⚒ ◈ ☰ ⚙ ⌕ ⌘ não estão no woff2 latin do Exo 2. Desenham em DejaVu Sans e FreeSerif aqui, e em outra fonte em cada sistema → SVG inline (sem dependência) com `aria-hidden`.
M12 · Datas · EN · o phx-grid formata `dd/mm/aaaa` e milhar com «,» decimal fixo (`fmt.dataHora`). Em EN, «01/10/2026» se lê 10 de janeiro. O `toLocaleTimeString()` do console segue o navegador e não o idioma da tela → passar `idiomas.atual` para o formatador (ISO `2026-10-01 06:00` é neutro).
M13 · Idiomas · catraca medida: **35** cravados (teto 35, laço 223/223, 0 mortas, 0 faltando). Em EN continuam em PT: rodapé (DISCIPLINA…LIBERDADE, «Juntos somos infinitos»). Em PT aparecem em EN: CORE ONLINE, TAURI CONNECTED, EVENT BUS WAIT/LIVE, WEB PREVIEW, NO NATIVE HOST, as 15 etapas do splash, `aria-label="PhxClaw splash screen"` e «BUILDING A BETTER SOFTWARE TOMORROW».
M14 · Config · EN · as descrições do catálogo saem em PT e sem acento («Binario do phxclaw…», «so ambiente: … nao configuracao»), e a coluna Descrição fica com cerca de 90 px e quebra em 4 linhas enquanto «Ação» fica vazia e larga. Decidir se o catálogo é rótulo (fábrica) ou dado; o acento é defeito de texto em qualquer caso.
M15 · IDE · 390 · o terminal abre em 110×30 e o canvas corta à direita («echo Ph…») → `terminal_redimensionar` pela largura real do `#ideArea`.
M16 · Alvos de toque · 390 · 39 seletores abaixo de 44 px, e 25 deles abaixo de 24 px no Config. Os menores: ▼ do filtro (12×12), × da pílula (6×13), caixas de marcar (13×13), «VOLTAR AO PADRÃO» (23 de altura); botões `.acao` com 32 px; filtros com 34 px → `.acao`/inputs com `min-height:44px` abaixo de 640 px, e área de toque de 24 px no mínimo nos ícones da grade.

### Cosmético
C1 · Foco: só `.nav` tem anel da marca (2 px #38d6ff). O resto usa o anel padrão do Chromium (`auto`). É visível (medido), mas inconsistente → `:focus-visible{outline:2px solid #38d6ff;outline-offset:2px}` global.
C2 · No toque o `:hover` gruda: depois do toque o botão fica cheio (visto em «GUARDAR TOKEN» a 390) → hover dentro de `@media (hover:hover)`.
C3 · Selos 111/64/1 cobrem parte do glifo do menu (19–71 px²) → `right:4px; top:4px`.
C4 · Avatar «PO» é texto fixo sem fonte de dado → derivar do usuário ou remover.
C5 · Landmark: o rodapé está dentro de `<main>` e por isso não vira `contentinfo` → tirar de `main`.
C6 · `@font-face` declara 400–700, e o CSS pede 720/800 (wordmark e h1) → declarar 400–800, se a fonte variável cobrir, ou usar 700.
C7 · Absorção (grade) · 1366 · cabeçalho termina em 1.282 px e o painel em 1.336 px, uma faixa morta de 54 px.

### Verificado e aprovado (medido)
- Contraste: fora M1–M3, todo texto visível passa (64 medições). O 1,15:1 de «pela metade» era falso positivo do gradiente; a captura mostra a hachura só na borda (`cap/zoom_pela_metade.png`).
- A página nunca rola na horizontal (`documentElement.scrollWidth == clientWidth` nas 64). O que rola de lado é interno: G2, e o envoltório do phx-grid (aceitável).
- Tab: todos os controles visíveis são alcançados, em ordem lógica, em todas as telas, a 1366 e a 390 (ex.: Config 202/202), e nenhum fica sem indicador, exceto o canvas.
- Botão sem nome: 0, fora G6/G9. Imagens sem `alt`: 0. Landmarks: banner, navigation, complementary, main e region por tela.
- Estados bons: sem JSON gerado (diz o arquivo e o comando, PT e EN), token recusado, tarefa vazia («Nenhuma tarefa ainda.»), filtro sem resultado («0 linhas» + «Limpar todos»), sem fábrica de idiomas (cai no PT, por desenho).
- 0 erros de console nas 64 medições.

## Estados por tela (vazio / carregando / erro / sem rede / sem JSON)
| Tela | vazio | carregando | erro | sem rede | sem JSON |
|---|---|---|---|---|---|
| Visão geral | n/a | «Lendo…» ok | n/a | ok na 2.ª visita; 1.ª: G7 | ok («sem fonte, sem número»), sem o comando |
| Agentes | ok (0 linhas) | ok | n/a | G7 | ok + comando |
| IDE | ok | ok | sem host: ok | n/a | n/a |
| Ferramentas | ok | ok | n/a | G7 | ok + comando |
| Absorção | ok | ok | n/a | G7 | ok + comando |
| Tarefas | ok | **vazio** (M8) | **sem ação** (G8) | **G8** | n/a |
| Config | ok | ok | **sem ação** (G8) | **G8** | catálogo |

## Desempenho (mediana de 3, servidor local sem gzip)
| Cenário | FCP | DCL | 1.ª tela útil | bytes |
|---|---|---|---|---|
| desktop, com splash | 244 ms | 110 ms | 3.379 ms | 936.854 |
| desktop, direto | 244 | 116 | imediata | 936.854 |
| desktop, sem phx-grid | 256 | 137 | — | 373.180 |
| celular 4G+CPU4×, PWA (start_url) | 1.672 | 4.304 | **7.573** | 926.479 |
| celular, direto | 1.648 | 4.274 | — | 936.854 |
| celular, sem phx-grid | 1.516 | 1.553 | — | 373.180 |
Tarefas longas: 1 de 220–263 ms no celular (parse) e 53–118 ms no desktop.

## Veredito por tela (critério: QUALIFICADA = 0 bloqueia e 0 grave nas 4 larguras e 2 idiomas; COM RESSALVAS = 0 bloqueia, graves só fora do caminho principal da tela ou só em 390 numa tela de mesa; NÃO = algum bloqueia, ou grave no caminho principal)
- Splash — NÃO QUALIFICADA (G3, G4 em todas as larguras; M4)
- Barra do topo — NÃO QUALIFICADA (G5, G6; G1 a 390)
- Visão geral — QUALIFICADA COM RESSALVAS (ok em 1920/1366/768; G2 e G11 a 390)
- Agentes — QUALIFICADA COM RESSALVAS (ok ≥768; B2 só a 390, numa tela de mesa)
- IDE — QUALIFICADA COM RESSALVAS (G10; M15 a 390)
- Ferramentas — QUALIFICADA COM RESSALVAS (ok ≥768; B2 a 390)
- Absorção — QUALIFICADA COM RESSALVAS (ok ≥768; cortes a 390)
- Tarefas — NÃO QUALIFICADA (B1 em todas; B2 a 390 é o caso de uso do PWA; G8)
- Configuração — NÃO QUALIFICADA (G9; G1 a deixa inalcançável a 390; G8)
- Painel do host — QUALIFICADA COM RESSALVAS (G5 vizinho; M12)
- Rodapé — QUALIFICADA COM RESSALVAS (M1, M13, C5)
- PWA sem rede — NÃO QUALIFICADA (G7, G8)

## Plano de consertos, em lotes pequenos e nesta ordem
1. **Teclado em Tarefas** (B1) — Enter/Espaço na linha abre o detalhe; prova por Tab+Enter no `ui_navegacao.mjs`.
2. **Casca do PWA** (G7 + G8) — 4 arquivos no `CASCA`, `casca-3`, 404 ≠ rede, mensagens de rede por chave + RECARREGAR; prova: servidor derrubado na 1.ª visita.
3. **Barra inferior 390 + Visão geral 390** (G1, G2) — `min-width:0`, grades de 2/1 coluna; prova: `scrollWidth` da `#tela-geral` e `getBoundingClientRect` do nav ≤ 390.
4. **Grade a 390** (B2) — modo cartão no phx-grid abaixo de 640 px (Tarefas primeiro); prova: altura mediana da linha ≤ 120 px.
5. **Splash** (G3, G4, M4) — `inert`, altura automática, pular com `?tela=`/standalone; prova: Tab durante o splash não sai dele e a 1.ª tela útil no PWA fica abaixo de 2 s.
6. **Topo honesto** (G5, G6, C4) — status do host derivado, ⌕/⌘ removidos ou ligados.
7. **A11y de formulário** (G9, G10, M7) — `aria-label` nos editores, saída do terminal, `role=status/alert`.
8. **Escala tipográfica e contraste** (G11, M1–M3) — piso de 11 px; prova: zero nós abaixo de 11 px e zero abaixo de 4,5:1.
9. **Fábrica** (M13, M12) — 35 → 0 (topo, splash, rodapé, `aria-label` do splash), formatador de data pelo idioma; baixar a catraca no mesmo commit.
10. **phx-grid sob demanda** (M5) — prova: DCL no celular ≤ 1,6 s.
11. **Marca** (M9–M11, M6, C1–C3, C5–C7) — rosa `.acao.marca`, fundo #010418, ícones SVG, `reduced-motion`, `:focus-visible` global, tokens.

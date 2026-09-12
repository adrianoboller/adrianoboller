# Plano de integração — phx-grid v0.78.1 → tela do PhxSql

Avaliação feita por E (Designer) + C (DBA) + B (Engenheiro), papel de AVALIAÇÃO/PLANO,
em 12/09/2026. É a **medição nova** que a recusa do pedido 160 (FX SDK externo) exigia —
mas o material aqui é o **nosso próprio** phx-grid (Phoenix/WX), que o dono anexou.
**Nada foi compilado, executado ou commitado durante a avaliação.** Todo binário recebido
(`conector/bin/*`, `__pycache__/*.pyc`) foi deixado intocado — só o fonte foi lido, na
quarentena do scratchpad. Este documento foi integrado ao repositório pelo orquestrador
como a medição versionada; o pacote `phx-grid v0.78.1` avaliado continua na quarentena,
**fora** do repositório (os binários pré-compilados não entram — ver §2.3).

**Achado que muda o enquadramento do pedido:** isto não é "avaliar uma lib de terceiro" — é
avaliar uma atualização do **mesmo** componente que o PhxSql já vendoriza. O PhxSql já roda
`phx-grid v0.9.3` (`phxsql/crates/phxsql-server/ui/grid/phx-grid.js`, 1.860 linhas,
`CHANGELOG-phx-grid.md`), a mesma linhagem "Phoenix / WX Soluções — ES5 estrito, zero
dependências". O material em quarentena é a **v0.78.1** (10.252 linhas, 19 módulos-fonte,
74 suítes/687 blocos, `LEIAME.md`:5-6) — **69 versões adiante**. A pergunta certa não é
"adotar ou não", é "como subir de 0.9.3 para 0.78.1 sem perder as adaptações que o PhxSql já
fez por cima do fonte original".

---

## 1. O que é o phx-grid v0.78.1 e o que ele traz para "altíssima qualidade responsiva"

Grid ES5 estrito, zero dependências, "offline-first", pensado para substituir o ActiveX
Janus GridEX 2000 dentro de telas WinDev/WebDev sem reescrever a tela
(`LEIAME.md`:1-4). Módulos-fonte em `src/modulos/` (19 arquivos, `ORDEM.txt`), concatenados
por `build.py` no `src/phx-grid.js` gerado — o fonte de verdade é o modular, nunca o gerado
(`LEIAME.md`:7-18).

Frente a frente com o que o PhxSql tem hoje (0.9.3, 1.860 linhas — só o núcleo S01–S09:
colunas, ordenação, filtro Excel, agrupamento, congelar coluna, exportar vista, layout em
`localStorage`), o que a v0.78.1 acrescenta e pesa em "qualidade gráfica responsiva":

| Recurso | Onde | Nota |
|---|---|---|
| **Tema total por token** (`temaDe(corBase)` deriva a paleta inteira — hover, header, chip, zebra, foco — de UMA cor) | `src/modulos/15-tema.js`:70-88 | gera contraste automático (`luminancia()`, linha 65-69) para decidir texto claro/escuro em cima do acento |
| **Configuração visual com 59 chaves**, cada uma mapeada 1:1 a um token `--phx-*`, chave desconhecida **não aplica em silêncio** — volta em `ignorados` | `docs/CONFIGURACAO-VISUAL.md`:1-181 | isso é o oposto de "CSS espalhado": é uma API só, auditável |
| **Densidade** (`compacta/confortavel/espacosa`) recalculando altura de linha inclusive dentro do virtual scroll | `CHANGELOG.md`:1617 (v0.71) | medido: 100k linhas recalculadas em 4,7 ms |
| **Virtualização de COLUNAS** (grades muito largas) além da de linhas, com espaçadores exatos preservando `colspan` de banda/grupo | `CHANGELOG.md`:1247-1271 (v0.41) | desligada por padrão; 84% menos DOM medido acima de ~100 colunas |
| **Cubo/pivô interativo v2**: chips de campo arrastáveis, múltiplas medidas, níveis aninhados expandir/recolher, salvar/ler cubo em slot, exportar direto para XLSX, gráfico do grupo | `docs/PARIDADE-JANUS.md`:90-91, 100 | motor próprio de PivotTable + slicers + PivotChart (v0.59–0.61), sem lib de gráfico |
| **Gantt, Kanban (WIP+arraste), Matriz editável, Árvore com agregação por nó, Lançamento (Enter contínuo, partida dobrada)** | `CHANGELOG.md`:1038 | "Cards (mobile)" citado na mesma linha como MODO, não como responsividade automática (ver §6) |
| **Modo Kiosk/TV**: rotação de recortes, `destruir()` do que sai do ar (sem vazamento num painel ligado 8h) | `CHANGELOG.md`:49-98 (v0.78.0) | é o próprio 0.78.1 que achou e consertou um bug de tema nele (ver §3) |
| **Gate visual pixel-a-pixel** (Puppeteer/Chrome real, 10 cenas, tolerância 0,15% de pixels) | `gate-visual.py`:7,44-47,157,263-277 | é a prática que a pétrea local "interface só se prova exercitando" já pede — aqui já é ferramenta, não só princípio |
| **Exportação XLSX própria** (deflate/zip/CRC-32 escritos à mão, não lib) | `PhxGrid.exec` list: `_deflate`, `_inflate`, `_leZip`, `_crc32`, `_utf8Bytes` (`src/modulos/18-export-e-fechamento.js`) | converge com o costume da casa de escrever formato à mão em vez de puxar dependência |
| **ARIA + teclado completo** nos 3 modos de render (paginado, alinhado, virtual) | `CHANGELOG.md`:1591-1592 | `role=grid/row/columnheader/gridcell`, `aria-rowindex` global, navegação por teclado célula-a-célula |

**Maturidade declarada**: 74 suítes, 687 blocos de prova, 58 demos single-file, dossiê de
paridade com o Janus zerado (98 OK, 0 parcial, 0 faltando) — `LEIAME.md`:5-6. Não medi essas
687 provas rodando (proibido nesta tarefa); registro o número como **declaração da fonte**,
não como medição própria.

---

## 2. Segurança — veredito da camada segura Rust↔cliente

### 2.1 O que é sólido (lido, com prova)

`conector/src/seguranca.rs` é **zero dependência** (`std` só — `HashMap`,
`SystemTime`) e faz exatamente o que a casa já faz: SHA-256 e HMAC-SHA256 escritos à mão,
conferidos contra vetor oficial:

- **FIPS 180-4**: vetores de `""`, `"abc"`, a string de 56 bytes do NIST, e o de um milhão de
  `'a'` (linhas 313-322) — inclusive o caso caro de exercitar muitos blocos.
- **RFC 4231 / RFC 2104**: os casos 1, 2, 3 e 6 do HMAC, incluindo **chave maior que o
  bloco** (linha 351-354), que é onde implementação à mão erra o pré-hash da chave.
- **Fronteiras de padding** (55/56/63/64/65 bytes, linhas 326-334) — onde o SHA-256 muda de
  bloco, testado deliberadamente.
- **Comparação em tempo constante** (`iguais_tempo_constante`, linhas 120-132) — percorre o
  buffer mesmo com tamanhos diferentes para não vazar o tamanho pelo tempo; é o padrão certo,
  não `a == b`.
- **Dupla implementação como oráculo**: o mesmo algoritmo existe em ES5
  (`src/modulos/19-seguranca.js`) e `docs/SEGURANCA.md`:68-69 afirma que as duas batem byte a
  byte, inclusive com acento e não-latino (`"Ação — São Paulo 日本"`) — não confirmei essa
  paridade cruzada linha a linha (não é objeto desta avaliação de tela), mas o **método** é
  exatamente o que o `CLAUDE.md` do PhxSql já exige para a própria criptografia da casa.
- **Política de rede com falha fechada, testada**: `valida_vinculo` recusa expor fora do
  loopback sem TLS (linhas 176-188, teste `vinculo_falha_fechada` 369-377); anti-DNS-rebinding
  por `Host` exato (`host_permitido`, 203-213, teste 393-403); `Origin` **nunca** `*`
  (`origem_permitida`, 192-199); anti-replay com janela e limpeza do cache (`GuardaReplay`,
  217-251, teste `replay_limpa_janela` provando que o cache não cresce sem limite); limite de
  tentativas com bloqueio temporizado (`LimiteTentativas`, 255-284).
- **Assinatura da requisição** amarra método+caminho+timestamp+nonce+SHA256(corpo) numa string
  canônica (linha 291) — adulterar qualquer um dos cinco campos muda o HMAC; o teste
  `assinatura_valida_e_detecta_adulteracao` (450-467) prova isso campo a campo, exatamente a
  disciplina "prova real nos dois sentidos" da casa.
- `docs/SEGURANCA.md` é **honesto sobre o limite**: admite que a comparação é em tempo
  constante mas não protege contra canais laterais mais sofisticados (linha 74-77), e que o
  **término TLS em si não está implementado** — hoje quem expõe em rede precisa de proxy
  reverso na frente (linha 114-118, repetido na seção 7 como item em aberto). Isso é o oposto
  de "afirmação sem prova": é recusa de prometer o que não foi feito, com o número e o motivo.

**Veredito da parte de política/cripto**: sólida, e sólida do jeito que a casa reconhece como
sólido — vetor oficial, não "parece certo". Nenhuma bandeira vermelha encontrada nela.

### 2.2 O que não serve ao PhxSql como está — e por quê

O **binário completo do conector** (`conector/src/main.rs`, que soma a esse núcleo o ODBC e a
leitura de planilhas) declara em `conector/Cargo.toml`:1-14:

```
tiny_http = "0.12"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
odbc-api = "8"
calamine = "0.26"
csv = "1"
```

Isso **quebra a pétrea "zero dependências externas. Só a std."** do PhxSql se algum dia for
ligado dentro do workspace `phxsql`. E mais: **é desnecessário para o nosso caso** — o
conector existe para o phx-grid falar com bancos e planilhas de **terceiros** (HFSQL, SQL
Server, xlsx…) quando ele substitui um ActiveX dentro de uma tela WinDev. O PhxSql **já é o
servidor**: a grade fala com `phxsql-server` pelo mesmo contrato `fonte: {carregar(params,
cb)}` que ela já usa hoje (`LEIAME-phx-grid.md`:82-86, idêntico nas duas versões). Não há ODBC
nem planilha para ler — o dado já está aqui.

**Recomendação**: o núcleo de política/cripto (`seguranca.rs`) é uma referência de desenho
para endurecer o **próprio** `http.rs` do PhxSql no futuro (é decisão de orquestrador, fora do
escopo desta tela) — mas **o binário `phx-conector` inteiro não entra** no workspace. Se um
dia o dono quiser o bridge ODBC como ferramenta separada (para importar de um ERP legado, por
exemplo), ele nasce como **projeto próprio, fora do `Cargo.toml` do `phxsql`**, compilado do
fonte, na fila de compilação **atrás do P0** — nunca linkado ao `phxsql-server`.

### 2.3 O que NÃO entra em hipótese nenhuma

- `conector/bin/phx-conector-windows-x86_64.exe` e `…-linux-x86_64` — binários pré-compilados,
  não lidos além do `file`/tamanho, **nunca executados** aqui e nunca versionados.
- `__pycache__/gate-documentacao.cpython-312.pyc`, `…gate-visual.cpython-312.pyc` — bytecode
  compilado, cruft de execução alheia.
- `indow.__pivot = PhxGrid.pivotar("#grid", {,+10p` — **confirmado inofensivo por leitura**:
  é um recorte de JS de demonstração (gerador de 4.000 linhas de pedido fictício com PRNG
  determinístico, chama `PhxGrid.pivotar`) cujo nome de arquivo é um acidente de empacotamento
  — alguém colou/truncou um trecho de shell e o início da linha (`window.__p` virou
  `indow.__p`) foi usado como nome. Não é código malicioso, é lixo de nome de arquivo; fica de
  fora por higiene, não por risco.

---

## 3. Como casa com a nossa tela

### 3.1 O bom: o PhxSql já tinha resolvido os dois pontos mais perigosos, para a 0.9.3

A tela do PhxSql **já fez** o trabalho que a pétrea de design manda, e já provou com teste:

- **Ponte de tema já existe** (`ui/index.html`:1395-1399): `.phx-grid` reescreve os seis
  tokens-base (`--phx-bg/--phx-bg2/--phx-fg/--phx-meta/--phx-borda/--phx-acc`) para apontar
  para os tokens da marca (`--painel`, `--painel-2`, `--texto`, `--laranja`…) e troca fonte
  para Exo 2. Como o resto dos tokens do phx-grid deriva desses seis via `var()` (confirmado
  nas duas versões — `src/phx-grid.css`:3 na quarentena tem os mesmos seis nomes-base), a
  ponte funciona por cascata de custom property, não por cópia de valor.
- **A cerca contra o CSS global já existe e já é testada**
  (`crates/phxsql-server/src/http.rs`:618-643, teste `a_cerca_do_css_global_continua_de_pe`):
  reescreve `input[type=checkbox]`, `input[type=radio]` e `label` **dentro de `.phx-grid`**,
  desfazendo o `width:100%` e o `text-transform:uppercase` globais — é a exata lição do
  «Blumenau» virando «BLUMENAU», já fechada em teste que reprova se alguém apagar a regra.
- **Rótulo vs. dado, já correto nas duas versões**: `formato:` é HTML puro escrito pelo
  chamador (LEIAME das duas versões diz "O HTML é seu, e o escape também" —
  `ui/grid/LEIAME-phx-grid.md`:50-52 e o mesmo texto em `phx-grid/LEIAME.md`); o grid não
  aplica `text-transform` nem reformatação estilística em cima do valor da célula — o único
  `text-transform` configurável é `cabecalho.caixa` (`docs/CONFIGURACAO-VISUAL.md`:43), que é
  rótulo (cabeçalho), não dado. Os formatadores (`fmt.numero`, `fmt.moeda`…) fazem formatação
  numérica de exibição (separador de milhar, casas decimais) — isso é apresentação do MESMO
  valor, não é o defeito que a pétrea persegue (que é mudar a leitura do dado, tipo caixa).

### 3.2 O que a v0.78.1 muda e exige trabalho de reconciliação (não é "colar por cima")

**i18n: o gancho mudou de arquitetura, e o novo não serve à nossa fábrica de idiomas como
está.** Na 0.9.3 vendorizada, o hook é por string, em tempo de render, delegando direto na
página hospedeira:

```js
// ui/grid/phx-grid.js (0.9.3), função txt()
function txt(nome, padrao) { return root.txt ? root.txt(nome, padrao) : padrao; }
```

Isso casa perfeitamente com a `FABRICA_TELA`/`root.txt` do PhxSql — cada string pede a
tradução na hora de desenhar, então trocar o idioma na tela de configuração já re-renderiza
certo. Na v0.78.1, o mecanismo virou uma **tabela interna fixa**, em português, resolvida por
chave com fallback para a própria chave (`src/modulos/02-formatacao.js`:233-238):

```js
var TEXTOS = { itensPorPagina: "itens por página", arrasteAgrupar: "...", ... };
function T(chave) { return TEXTOS[chave] == null ? chave : TEXTOS[chave]; }
```

com um setter global e não-reativo exposto em `PhxGrid.definirTextos`/`PhxGrid.textos`
(`src/modulos/18-export-e-fechamento.js`, lista de exports). Isso **não é uma regressão da
pétrea "rótulo se traduz por chave, nunca por frase"** — o `T()` já resolve por chave — mas
**não é multi-idioma em tempo real**: é um dicionário único, global (não por instância), que
alguém troca por fora com `definirTextos(mapa)`. Para casar com a nossa máquina de idiomas
pétrea (`crates/phxsql-server/src/idiomas.rs`, `FABRICA_TELA`), a integração precisa de um
adaptador pequeno e explícito: no boot da página **e** a cada troca de idioma, chamar
`PhxGrid.definirTextos({ itensPorPagina: txt("grid.itens_por_pagina", "itens por página"), ...
})` para as ~14 chaves do dicionário. **Sem esse adaptador, a grade nova fala português fixo
mesmo com o usuário em inglês/espanhol** — é exatamente o "texto cravado que a fábrica não vê"
que a pétrea de i18n do projeto existe para pegar; aqui ele não escapa por descuido de quem
escreveu a tela, escapa porque o mecanismo upstream mudou de forma.

**CSS: a cerca e a ponte de tema cobrem `.phx-grid`, e os componentes novos vivem em OUTRAS
classes que compartilham o mesmo bloco de tokens.** A folha da v0.78.1 declara os tokens uma
vez para oito seletores:

```css
.phx-grid,.phx-kb,.phx-gantt,.phx-matriz,.phx-cubo,.phx-dropdown,.phx-slicer,.phx-kiosk{ ...
```

(`src/phx-grid.css`:2). A ponte de tema do `index.html` e a cerca do `http.rs` hoje só citam
`.phx-grid`. Se a tela adotar cubo, kanban, gantt, matriz ou kiosk, **os mesmos dois problemas
que já foram corrigidos para a tabela voltam para os componentes novos**, porque eles também
têm `input`/`label`/`checkbox` (o field-chooser do cubo, o slicer, o dropdown) e os mesmos
`--phx-header-bg`/`--phx-acc` etc. — mas fora do seletor que a ponte/cerca reescreve. **A
própria v0.78.1 já pagou uma vez por não estender a lista de tokens**: o changelog da 0.78.1
(`CHANGELOG.md`:12-41) documenta que `.phx-kiosk` foi esquecido nas DUAS listas de token
(claro e escuro) quando o kiosk foi criado na 0.78.0, e todo `var()` dele resolveu para nada —
fundo, barra e progresso todos transparentes, título em serifa — **com as 12 provas de
JavaScript passando o tempo todo**. É a prova, na própria fonte, de que token que falta falha
em silêncio e só a captura de tela mostra — a mesma lição que gerou a nossa cerca em 2026.

**Cores de status (badge) não vêm da marca por padrão.** `--phx-ok`, `--phx-perigo`,
`--phx-aviso`, e os cinco pares `--phx-badge-*-bg/fg/bd` (verde/âmbar/azul/vermelho/cinza) são
hex fixos no `phx-grid.css` (`docs/CONFIGURACAO-VISUAL.md`:19,30-31 e o bloco de badges no
`src/phx-grid.css`:3, linhas `--phx-badge-verde-bg:#dafbe1`…) e **não** derivam de
`--phx-acc`/`--phx-bg` como o resto — a ponte de seis tokens do `index.html` não os alcança.
São *pills* de status de dado (ex.: "Pago"/"Pendente"), não botões de ação, então a pétrea
"verde inclui, amarelo altera… sempre contorno" (que é sobre botão) não se aplica a eles
diretamente — mas, se a tela quiser esses badges na paleta da marca (âmbar `#FFC43D`, laranja
`#FF8A1C`, vermelhão `#C63C0A` no claro), a ponte de tokens precisa crescer para incluir esse
bloco também, senão o grid mistura verde/vermelho estilo GitHub com o resto da tela em
laranja/âmbar.

### 3.3 Resumo do encaixe

| Pétrea de tela | v0.78.1 respeita? | Ação necessária |
|---|---|---|
| Rótulo se estiliza, dado nunca | ✅ (confirmado nas duas versões, `formato:` é HTML do chamador, sem transform em dado) | nenhuma |
| i18n pela fábrica de idiomas | 🟡 resolve por chave, mas dicionário fixo/global, não por instância | adaptador `definirTextos()` chamado no boot e na troca de idioma |
| A marca manda (tokens, Exo 2, contorno) | 🟡 a ponte de 6 tokens já existe e funciona por cascata; não cobre badges nem os 7 seletores novos | estender a lista de seletores e o bloco de badges na ponte de tema |
| CSS global não morde componente novo | 🟡 a cerca existe e é testada, só para `.phx-grid` | estender a cerca (`http.rs` + `index.html`) para `.phx-kb,.phx-gantt,.phx-matriz,.phx-cubo,.phx-dropdown,.phx-slicer,.phx-kiosk` **antes** de ligar qualquer um deles na tela |

---

## 4. Plano de integração, em passos

Pipeline de compilação é de uma vez por vez, com o P0 ocupando a fila agora. Por isso a ordem
separa o que não pede `cargo build` do que pede.

### Fase A — não compila (JS/CSS de tela; pode começar já, sem esperar o P0)

1. **Não copiar `src/phx-grid.js` (gerado) por cima do nosso.** Partir dos **19 módulos** em
   `src/modulos/` e portar feature por feature para dentro da estrutura que o PhxSql já mantém
   (hoje um único arquivo vendorizado, sem `build.py` próprio — `ui/grid/LEIAME-phx-grid.md`:
   111-114 já registra que os caminhos "no projeto de origem" não existem nesta árvore). Cada
   feature trazida reaplica, no ato, duas coisas que a 0.9.3 já tinha e a v0.78.1 teria que
   ganhar de novo: o hook `txt()` por string (não o `TEXTOS`/`T()` fixo) e nenhuma dependência
   de `root.txt` sendo undefined (o fallback já existe, confirmar que sobrevive).
2. Nesta portagem, trazer primeiro o que mais eleva "qualidade gráfica responsiva" com menor
   risco de regressão: **tema total (`temaDe`/`registrarTema`)**, **configuração visual por
   token (as 59 chaves)**, **densidade**, e **virtualização de coluna** — são aditivos, não
   mudam o contrato de coluna/fonte que a tela já usa.
3. Em seguida, **estender a cerca e a ponte de tema** (§3.2) para os seletores novos **antes**
   de trazer qualquer um dos componentes que os usam (cubo, kanban, gantt, matriz, kiosk,
   dropdown/slicer) — nesta ordem, e não na ordem inversa, porque a própria v0.78.1 mostrou o
   preço de esquecer um seletor na lista de tokens (§3.2, caso do `.phx-kiosk`).
4. Escrever o **adaptador de i18n** (`PhxGrid.definirTextos(...)` no boot + no evento de troca
   de idioma) antes de expor a tela a usuário de outro idioma.
5. Só depois trazer cubo/kanban/gantt/matriz/kiosk, um de cada vez, cada um seguido do
   exercício em navegador do item 6.
6. Ao final da Fase A: `cargo fmt`/`clippy`/testes Rust **não são tocados** (é tudo estático
   embutido por `include_str!`), mas os testes que **existem hoje** em Rust sobre esse
   material (`grade_versao_nao_mente`, `a_cerca_do_css_global_continua_de_pe`) precisam ser
   **estendidos**, não substituídos — isso sim entra na fila de compilação normal quando o
   engenheiro de plantão for aplicar o plano (não nesta tarefa).

### Fase B — compila (só se o dono decidir que quer o bridge ODBC/planilha)

7. **Fora do escopo padrão desta integração.** Se decidido, o `phx-conector` nasce como
   projeto Rust **separado**, fora do `Cargo.toml` do workspace `phxsql`, com o
   `seguranca.rs` como referência de desenho (não como dependência de código) — o zero-deps do
   PhxSql não pode ser furado para carregar `odbc-api`/`calamine`/`tiny_http`/`serde`. Entra na
   fila de compilação **depois** do P0, como qualquer trabalho novo.
8. Se algum dia quisermos aproveitar só a **política** (rate limit, anti-rebinding, HMAC,
   anti-replay) para endurecer o `http.rs` do próprio PhxSql — que já tem seu próprio SHA-256
   /HMAC — isso é decisão de arquitetura do orquestrador, medida à parte, e não decorre
   automaticamente de "gostamos do grid".

---

## 5. Licença

**Falta `LICENSE` no pacote inteiro** — não há arquivo `LICENSE*` na raiz do phx-grid nem
menção de licença em `LEIAME.md`/`CHANGELOG.md`. O único lugar que declara algo é
`conector/Cargo.toml`:6, `license = "MIT"` — **MIT só**, diferente do `MIT OR Apache-2.0` que
o próprio `phxsql/Cargo.toml`:18 usa. Recomendação:

- Adicionar `LICENSE-MIT`/`LICENSE-APACHE` (ou um `LICENSE` dual) na raiz do phx-grid,
  replicando o `MIT OR Apache-2.0` do PhxSql — é o mesmo ecossistema/autor (WX Soluções), não
  há razão para licença diferente.
- Corrigir `conector/Cargo.toml` de `"MIT"` para `"MIT OR Apache-2.0"` no mesmo movimento, se e
  quando o conector for adotado como projeto próprio (Fase B).
- Enquanto isso não acontece, qualquer trecho **portado** para dentro de `phxsql/` (Fase A)
  já nasce sob a licença do `phxsql/Cargo.toml`, porque passa a ser parte deste repositório —
  mas vale registrar a origem (versão e arquivo-fonte) no cabeçalho do trecho portado, do jeito
  que o `phx-grid.js` vendorizado já faz hoje (cabeçalho cita a versão e a razão do número).

---

## 6. O que medir/exercitar no navegador antes de adotar

"Interface só se prova exercitando" — nada abaixo foi executado nesta avaliação (proibido
nesta tarefa); é a lista do que falta medir quando o engenheiro de plantão aplicar o plano.

1. **A cerca estendida, pixel a pixel, não só por `grep`.** O teste
   `a_cerca_do_css_global_continua_de_pe` confere que a REGRA existe no texto da página — não
   que ela é suficiente para os componentes novos. Cada componente adotado (cubo, kanban,
   gantt, matriz, kiosk) precisa da mesma checagem visual que achou o funil de 204 px e o
   «BLUMENAU»: abrir no navegador, medir largura de checkbox/rádio e o texto de um rótulo
   real, comparar antes/depois de estender a cerca.
2. **A ponte de tema nos dois temas, para cada seletor novo.** Reproduzir manualmente o que a
   0.78.1 já pagou uma vez (o kiosk invisível): trocar para tema escuro, abrir cada componente
   novo, e medir com o inspetor (`getComputedStyle`) que `background`/`color` não caem em
   `rgba(0,0,0,0)` nem em transparente. As 12 provas de JavaScript do próprio kiosk passavam
   com o bug presente (`CHANGELOG.md`:19) — é o caso concreto de que teste unitário de token
   CSS não substitui olhar a tela.
3. **Responsividade real em largura de telefone, medida, não presumida.** `grep -c "@media"
   src/phx-grid.css` devolve **zero** nas duas versões (0.78.1 na quarentena e 0.9.3
   vendorizada) — não há breakpoint nenhum dentro da folha do grid. O que existe é: (a)
   virtualização/densidade, que ajudam performance mas não reflow; (b) um modo alternativo de
   card view (`vertical()`/`13-cardview.js`) que a página hospedeira **decide** quando chamar.
   Antes de anunciar "responsivo" no dossiê, medir no Chrome em ~400px de largura se a tabela
   rola horizontalmente de forma aceitável, e decidir/testar o ponto de corte em que a tela do
   PhxSql troca para `vertical()` via `matchMedia` — isso é trabalho nosso, o grid não faz
   sozinho.
4. **O adaptador de i18n, nos dois sentidos.** Provar que trocar o idioma na tela de
   configuração (a máquina pétrea de `idiomas.rs`) também troca o texto do rodapé do grid
   (`itensPorPagina`, `mostrando`…) sem recarregar a página — hoje isso não acontece sem o
   adaptador do item 4 da Fase A.
5. **Gate visual próprio.** `gate-visual.py` é Puppeteer/Chrome real, 10 cenas, tolerância
   0,15% (`gate-visual.py`:44-47). Vale copiar o método (não o script, que é do projeto de
   origem) para uma cena equivalente dentro de `testes-web/grade/` do PhxSql, cobrindo pelo
   menos: tema claro, tema escuro, e o componente novo mais arriscado (o primeiro que ligar
   `.phx-kb`/`.phx-cubo`/etc.).
6. **Contagem de texto cravado.** Rodar o conferidor que já existe
   (`textos-fora-da-fabrica`, citado no `CLAUDE.md` do projeto) depois de cada componente novo
   entrar, porque cada um deles chega com o próprio conjunto de rótulos de UI (nomes de menu,
   dicas) que também precisam passar pela fábrica — não só o `TEXTOS`/`T()` de paginação
   coberto no item 4.

---

## Fontes citadas neste documento

- Quarentena: `LEIAME.md`, `CHANGELOG.md`, `docs/SEGURANCA.md`, `docs/PARIDADE-JANUS.md`,
  `docs/CONFIGURACAO-VISUAL.md`, `src/phx-grid.css`, `src/modulos/{02-formatacao,15-tema,
  18-export-e-fechamento}.js`, `src/modulos/ORDEM.txt`, `conector/Cargo.toml`,
  `conector/src/{main,seguranca}.rs`, `gate-visual.py`, `tests/package.json`.
- Repositório: `phxsql/crates/phxsql-server/ui/grid/{phx-grid.js,phx-grid.css,
  LEIAME-phx-grid.md,CHANGELOG-phx-grid.md}`, `phxsql/crates/phxsql-server/ui/index.html`
  (linhas 1391-1424), `phxsql/crates/phxsql-server/src/http.rs` (linhas 570-643),
  `phxsql/crates/phxsql-server/src/idiomas.rs`, `phxsql/marca/LEIA-ME.md`,
  `phxsql/Cargo.toml`.

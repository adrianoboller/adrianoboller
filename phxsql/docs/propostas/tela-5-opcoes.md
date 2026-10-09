# Interface web: cinco opções (+1) medidas contra o nosso gargalo — pedidos 773/774

**Papel J, 09/10/2026.** Ordem do dono do mesmo dia: *«O que protege de verdade é: segredo nunca no
cliente, permissões checadas no servidor e CSP.»* Tudo abaixo foi medido nesta data, nesta máquina,
salvo onde diz **citado** ou **raciocinado, não medido**.

## 1. Hipóteses, escritas ANTES de medir

| # | Hipótese | Veredito medido |
|---|---|---|
| H1 | Leptos/Dioxus/Yew puxam >100 crates e exigem exceção à pétrea igual à do 774 | **Confirmada**: 179 / 170 / 103 pacotes no `Cargo.lock` |
| H2 | WASM de tela mínima ≥100 KiB gz; React 45–60 KiB gz; JS atual < metade do React | **Morreu em parte**: Leptos 55,3 e Yew 66,4 KiB gz (abaixo de 100); só Dioxus passou (118,6). React deu **69,4**, acima da faixa. JS: 0,4 KiB — confirmada |
| H3 | A CSP atual não tem `unsafe-eval`, mas talvez `unsafe-inline` em estilo; o custo real está no *inline*, não no framework | **Confirmada e pior que o previsto**: o `unsafe-inline` está também no **`script-src`** |
| H4 | Via 6 (WASM à mão, só `std`) é viável e <30 KiB gz, com custo de engenharia na ponte de DOM | **Confirmada**: 21,0 KiB gz, 1 pacote no lock; ponte de 7 importações para UMA tela |
| H5 | O JS atual endurecido vence em segurança e migração; Leptos vence só na tendência «Rust sempre que possível» | **Confirmada** (§4) |

## 2. A tela equivalente e como se mediu

Mesma tela nas seis vias: tabela de 50 linhas, busca por texto, formulário de 2 campos que inclui
linha. Perfil `opt-level="z"`, `lto`, `codegen-units=1`, `panic="abort"`, `strip`; WASM passado por
`wasm-bindgen 0.2.129 --target web` e `wasm-opt -Oz` (binaryen 125); React 19.3.0 por esbuild 0.28.2
`--minify`; gzip `-9`. Prova funcional e de CSP em Chromium 141.0.7390.37 (Playwright), servidor
estático que manda a CSP no cabeçalho, ouvinte de `securitypolicyviolation`. Versões: Leptos 0.8.22,
Yew 0.21.0, Dioxus 0.6.3. Toolchain 1.94.1 (o pinado).

## 3. Matriz de evidência

| Critério | 1 Leptos | 2 Dioxus | 3 Yew | 4 React | 5 JS endurecido | 6 WASM à mão (só `std`) |
|---|---|---|---|---|---|---|
| Bundle cru (wasm + cola JS) | 114.482 + 29.438 = **143,9 KB** | 249.175 + 51.683 = **300,9 KB** | 141.271 + 27.041 = **168,3 KB** | **224,7 KB** | **0,6 KB** (min) | 41.597 + 1.080 = **42,7 KB** |
| Bundle gzip -9 | 49.194 + 6.073 = **55,3 KB** | 108.265 + 10.305 = **118,6 KB** | 60.584 + 5.821 = **66,4 KB** | **69,4 KB** | **0,4 KB** | 20.400 + 565 = **21,0 KB** |
| Funciona sob CSP estrita (`script-src 'self' 'wasm-unsafe-eval'; style-src 'self'`, sem `unsafe-inline`) | sim, 0 violações | sim, 0 violações | sim, 0 violações | sim, 0 violações | sim, 0 violações | sim, 0 violações |
| Funciona SEM `'wasm-unsafe-eval'` | **não** (`script-src wasm-eval` bloqueado) | **não** | **não** | **sim** | **sim** | **não** |
| `eval`/`new Function` na cola | 0 (1 falso positivo: texto de depuração) | 0 | 0 | 0 | 0 | 0 |
| Acessibilidade (DOM real: `getByRole('table')`, `getByLabel`, foco no botão após incluir) | 3/3 | 3/3 | 3/3 | 3/3 | 3/3 | 3/3 |
| Pacotes de terceiros (lock) | **179** | **170** | **103** | 3 de execução (react, react-dom, scheduler) + 2 de montagem (esbuild, @esbuild/linux-x64) | **0** | **0** (lock = 1, ele mesmo) |
| Código de terceiros que RODA na montagem (build.rs + proc-macro) | 28 + 23 | 23 + 23 | 14 + 11 | binário nativo do esbuild | 0 | 0 |
| Árvore vendorizada (necessária para `--offline`) | 111 MB | 96 MB | 74 MB | 10 MB (node_modules de execução) | 0 | 0 |
| Compilação limpa | 141 s | 52 s | 73 s | 0,06 s (esbuild) | — | 0,6 s |
| Ferramenta de fora exigida | `wasm-bindgen` CLI na versão EXATA do crate | idem | idem | esbuild (React 19 **não tem mais UMD** — só ESM por CDN, que o 774 proíbe) | nenhuma | nenhuma (`wasm-opt` opcional: 44,6 → 41,6 KB) |
| Reuso do Rust | tipos/validação compilam (core e servidor passam `cargo check --target wasm32` em 3 s / 14 s) | idem | idem | nenhum | a `FABRICA_TELA` **já** é reusada por `/idiomas?idioma=` | idem às 1–3, sem framework |
| Testes atuais que quebram ao migrar uma tela | os casos da tela (ver §5) | idem | idem | idem | **0** | idem às 1–3 |
| Pétrea «zero dependências» | **exige exceção nova** (o 774 cobre só React) | **exige exceção nova** | **exige exceção nova** | coberto pelo 774 | dentro | dentro |

Comandos: `cargo build --release --target wasm32-unknown-unknown`; `wasm-bindgen --target web
--no-typescript`; `wasm-opt -Oz`; `gzip -9c | wc -c`; `grep -c '^name = ' Cargo.lock`; `cargo vendor
--versioned-dirs`; prova no navegador por script Playwright que sobe servidor estático com a CSP no
cabeçalho (12 corridas, 6 vias × 2 CSPs).

### 3.1 A CSP que o `http.rs` manda HOJE (medida no `phxsqld` de 09/10/2026 13:50)

```
default-src 'none'; style-src 'unsafe-inline'; font-src data:; script-src 'unsafe-inline'; img-src data:;
connect-src 'self' https://api.anthropic.com; form-action 'none'; frame-ancestors 'none'; base-uri 'none'
```

(`crates/phxsql-server/src/http.rs:456-478`; o `connect-src` da Anthropic só com a integração ligada.)
A moldura é boa (`default-src 'none'`, `frame-ancestors`, `base-uri`, `form-action`), mas o
**`script-src 'unsafe-inline'` anula a parte da CSP que protege de XSS**. Medido trocando só essa
diretiva por `'self'` (rota interceptada, servidor real, `entrar()` da bateria): **7 bloqueios
`script-src-elem`, a página cai em modo demonstração**. A página servida tem 1.498.220 bytes, **8**
`<script>` embutidos, 1 manipulador `on*=` em atributo, e há **224** atribuições a `innerHTML` em
`ui/` — cada uma é um lugar onde um `<img onerror=…>` vindo de dado executaria hoje.
Trocando só `style-src` por `'self'`: 12 `style-src-elem` + **65** `style-src-attr` num passeio de
15 nós da árvore (97 `style=` textuais em `ui/`).

**O que isto diz contra a ordem do dono:** nenhuma das cinco opções conserta a CSP sozinha — a página
atual convive com qualquer migração tela a tela por meses, e enquanto houver um `<script>` embutido o
`'unsafe-inline'` fica. O endurecimento do JS atual é **pré-requisito de todas as opções**, não uma
alternativa a elas.

## 4. Decisão — por dominância, não por peso inventado

O dono não deu pesos por critério, e inventá-los seria decidir por número digitado. Decide-se por
**dominância medida**: uma opção sai se outra é igual ou melhor em todos os critérios medidos.

- **5 (JS endurecido) domina 4 (React)**: bundle 0,4 vs 69,4 KB gz; CSP igual (0 violações, nenhum
  dos dois precisa de `wasm-unsafe-eval`); acessibilidade igual; React não reusa nada de Rust; React
  quebra os testes da tela migrada e traz montador nativo. **React RECUSADO** — a exceção do 774
  fica concedida e **sem uso**.
- **6 (WASM à mão) domina 1, 2 e 3**: menor bundle (21,0 vs 55,3/66,4/118,6 KB gz), mesma CSP
  (`wasm-unsafe-eval` nas quatro), mesmo reuso de Rust, mesma acessibilidade, e **zero** pacotes
  contra 103–179 — sem exceção à pétrea, sem `wasm-bindgen` de versão casada, `--offline` de graça
  (lock de 1 pacote). O que 1–3 compram e 6 não tem é **ergonomia** (componente, reatividade): não
  é critério da lista do dono, e o preço dela é a pétrea. **Leptos, Dioxus e Yew RECUSADOS.**
  Entre os três, se a pétrea for revogada pelo dono, o vencedor seria **Leptos** (55,3 KB gz, o
  menor), não Dioxus (118,6 KB, 2,1× maior, e com `prevent_default()` no `submit` de semântica
  **invertida** — `dioxus-web-0.6.3/src/dom.rs:104-109`: chamá-lo ENVIA o formulário; a forma
  normal do DOM recarregou a página na primeira prova).
- **5 contra 6**: 5 vence bundle (0,4 vs 21,0), CSP (dispensa `wasm-unsafe-eval`) e migração (0
  testes); 6 vence só o reuso de Rust. E o reuso medido é menor do que parece: a `FABRICA_TELA` já
  chega à tela pelo `/idiomas` (um idioma por vez), e linkada no WASM ela custa **704.845 B cru /
  280.859 B gz** com os seis idiomas (519.835 B são o próprio texto); a validação no cliente é só
  conforto, porque a ordem do dono põe a permissão **no servidor**, que valida de novo.

**Decisão: opção 5, endurecida, agora; a via 6 fica como caminho «Rust sempre que possível» para
módulos de LÓGICA pesada, um por vez, cada um entrando com o número que o justifica.** A tendência
do dono não vence aqui porque o número não a sustenta para a tela inteira: o único ganho do Rust no
cliente é reuso, e o maior reuso (idiomas) já existe sem WASM.

### 4.1 O endurecimento, e onde diverge da receita de fora

A receita corrente para tirar `'unsafe-inline'` é **nonce por resposta** (CSP3 §2.3.1, a «strict
CSP» dos navegadores). **Divergimos para hash** (`'sha256-…'`), e a restrição nossa que causou é a
**página montada uma vez por processo** (`include_str!` + `format!` no `http.rs:228-231`): os 8
blocos são fixos durante a vida do servidor, então o hash se calcula **uma vez no arranque com o
SHA-256 desta casa** (conferido contra FIPS 180-4), sem reescrever 1,5 MB por pedido e sem crate.
Nonce exigiria remontar a página a cada resposta para nada. O 1 `on*=` vira `addEventListener`; os
12 `<style>` também por hash; os `style=` em atributo (97 no fonte) migram para classe, ou ficam sob
`style-src 'unsafe-inline'` — que é o risco **menor** (CSS não executa script) e pode ser a segunda
etapa. `script-src` sai sem `'unsafe-inline'` e **sem** `'wasm-unsafe-eval'` até a via 6 entrar.

### 4.2 O custo do `'wasm-unsafe-eval'`, como o dono pediu que pesasse

Citado da norma (W3C CSP Level 3, §4.5.1 `EnsureCSPDoesNotBlockWasmByteCompilation`): sem a palavra,
compilar WASM vira violação `wasm-eval` e `WebAssembly.CompileError` — **medido**: as quatro vias WASM
morreram sem ela, as duas JS não. O custo é pequeno e real: com ela, script que já roda pode compilar
WASM arbitrário. Pequeno porque exige execução de script primeiro, que é o que o `script-src` estrito
fecha; real porque é uma palavra a mais que React e JS não pedem. Entra só junto do primeiro módulo
da via 6, nunca antes.

## 5. Migração tela a tela e a bateria

A bateria (`testes-web/`) se prende ao DOM e ao escopo global: **407** seletores `#id` distintos e
**517** `page.evaluate` (vários chamando funções globais como `telaDiagramaER`), mais 460 ganchos em
`botoes-exercitados.txt`. Medido por `grep` em 09/10/2026. Componente de framework esconde estado e
função dentro dele — cada tela migrada para 1–4 reescreve os casos dela (**raciocinado, não medido**:
decidiria a bancada migrar UMA tela e contar os casos vermelhos). A opção 5 não muda seletor nenhum. A
catraca de textos fora da fábrica (`conferidor.rs`) lê o fonte de `ui/`; texto dentro de `view!`/`rsx!`
ou JSX escaparia dela até alguém ensiná-la — custo das opções 1–4, nulo na 5.

`cargo build --offline` e o cruzado para Windows: em todas as opções o produto da tela são bytes
embutidos; continuam valendo **se** o artefato de WASM/JS for montado fora do `cargo build` do
servidor e versionado com SHA-256 (condição já escrita no 774). **Raciocinado, não medido** para 1–4.
Para a via 6, medido: compila com `--offline` (lock de 1 pacote); exige acrescentar
`targets = ["wasm32-unknown-unknown"]` no `rust-toolchain.toml` (o pinado 1.94.1 não tinha o alvo: o
primeiro `cargo check` falhou com `can't find crate for std`).

## 6. Recusas medidas (para não voltarem sem número novo)

| Recusado | Número |
|---|---|
| React (4) | 69,4 KB gz por tela mínima contra 0,4; zero ganho de CSP sobre a opção 5; sem UMD no 19, exige montador nativo |
| Dioxus (2) | 118,6 KB gz (o maior); 170 pacotes; semântica de `submit` invertida |
| Yew (3) | 66,4 KB gz; 103 pacotes; dominado pela via 6 |
| Leptos (1) | 55,3 KB gz; **179** pacotes, 28 build.rs e 23 proc-macros rodando na máquina de quem compila; dominado pela via 6 |
| WASM para reusar a `FABRICA_TELA` | 280.859 B gz com seis idiomas, contra o `/idiomas` que já serve um |

## 7. Lacunas

- Custo de engenharia da via 6 em tela real: medido só numa tela mínima (7 importações de ponte).
  Decide: portar UM módulo de lógica (ex.: o filtro da grade, onde hoje há **dois motores do mesmo
  filtro** que discordam — `docs/GRADE.md`, pedido 138) e medir linhas, bundle e casos da bateria.
  Esse é o candidato natural porque ali o reuso do Rust **elimina uma duplicação viva**, o que a lei
  «função e comando vêm do mesmo motor» já pede.
- Dioxus 0.7 não medido (resolveu 0.6.3 pela faixa pedida).
- Quantos casos da bateria quebram por tela migrada: raciocinado, não medido.
- Os scripts desta medição ficaram no rascunho da sessão; extraí-los para `bancada/` é passo do
  integrador.

## 8. O que sobe ao dono

**Choque com pétrea — só se o dono quiser um framework Rust mesmo assim.** Leptos/Dioxus/Yew exigem
uma exceção NOVA à «zero dependências externas» (o 774 cobre React, não crates). A recomendação não
precisa dela; se o dono a quiser pela tendência, o número que ele estará comprando é este: 179
pacotes e 111 MB vendorizados (Leptos) para 34,3 KB gz a mais que a via 6 por tela.

**Não sobe:** a escolha entre as opções (dominância medida), nem o `wasm-unsafe-eval` (entra só com
a via 6). **Informativo:** a exceção do 774 fica sem uso.

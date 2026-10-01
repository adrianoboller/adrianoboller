# UI-IR v3 — intenção de layout (Phx Responsive UI)

Formato do UI-IR a partir de 01/10/2026. Código: `crates/phxclaw-ui-ir/src/ir.rs` (tipos),
`src/responsivo.rs` (motor), `breakpoints.json` (pontos de quebra).

## O que entrou na v3

Dois campos **opcionais** em cada seção (`Section`, nas telas `form` e no `header` do
`master_detail`):

```json
{
  "title": "Dados principais",
  "fields": ["nome", "cnpj"],
  "layout": {
    "tipo": "grade",
    "gap": "md",
    "colunas": 1,
    "responsivo": {
      "base": "conteiner",
      "alvo": "tela",
      "regras": [
        {"min_largura_px": 576, "colunas": 2},
        {"min_largura_px": 768, "colunas": 3},
        {"min_largura_px": 992, "colunas": 4}
      ]
    }
  },
  "conteiner": "secao"
}
```

| Campo | Valores | Significado |
|---|---|---|
| `layout.tipo` | `grade`, `fila`, `pilha` | grade = CSS Grid; fila = Flexbox em linha que quebra; pilha = Flexbox em coluna |
| `layout.gap` | `xs`, `sm`, `md`, `lg`, `xl` | **token** de espaço, nunca pixel: o valor é do tema (`--phx-gap-*`) |
| `layout.colunas` | 1–12 | colunas na largura mais estreita (mobile-first) |
| `responsivo.base` | `janela`, `conteiner` | `janela` vira `@media`; `conteiner` vira `@container` |
| `responsivo.alvo` | nome | conteiner consultado: o da própria seção ou `tela` (a área de conteúdo) |
| `responsivo.regras` | lista crescente | a partir de `min_largura_px`, `colunas`; regra só vale para `grade` |
| `conteiner` | `[a-z][a-z0-9_-]{0,31}` | nome do conteiner que a seção abre; `tela` e `tabela` são reservados |

**Nunca classe de framework.** `row`/`col-*` no IR amarrariam o desenho a um adaptador; o IR
diz *quantas colunas a partir de que largura*, e cada adaptador desenha componentes.

### Também na v3

| Onde | Campo | Significado |
|---|---|---|
| `App` | `quebra_da_casca_px` | largura da **janela** em que menu e conteúdo ficam lado a lado (a única regra de janela da composição); ausente = `md` |
| `App` | `tokens` | tokens do tema que o app sobrepõe: `gap-xs`…`gap-xl` (px), `acento` (`#rrggbb`, só no tema escuro), `raio` (px) |
| `screens[]` | `pattern: "painel"` | painel com `conteiner` nomeado e uma `colecao` (`id`, `origem`, `chave`, `template`, `template_versao`, `layout`, `itens` de cartões com `id`, `titulo`, `subtitulo`, `sigla`, `tom`, `tarefa`, `capacidades`, `detalhe`) |

O painel é o `Panel`/`Collection` do PHX JSON. React e Flutter ainda não o desenham e dizem
isso no arquivo gerado; o HTML («phoenix») e o Bootstrap desenham.

## PHX JSON (envelope)

`phx_json::ler`/`escrever` traduzem o `app.phx.json` do Phoenix (chaves em inglês camelCase)
para o UI-IR e de volta — **uma** representação interna, o envelope só traduz:
`basis`↔`base`, `target`↔`alvo`, `minWidthPx`↔`min_largura_px`, `columns`↔`colunas`,
`spacing.md`↔`md`, `ui.shellBreakpointPx`↔`quebra_da_casca_px`, `agents`↔`colecao.itens`. O que o
UI-IR não representa (versão do contrato, adaptador e versão pedidos, tema, valores iniciais,
templates, componentes, simulação) fica no `Envelope`. Subconjunto: um `Panel` com uma
`Collection` `grid`; campo desconhecido, ausente ou de tipo errado é recusado **com o caminho**
(`$.view.children[0].layout.foo: campo desconhecido`). A ida e volta do exemplo do dono é
idêntica byte a byte (teste `ida_e_volta_identica_byte_a_byte`). CLI: `phxclaw ui importar`.

## Padrão

Seção sem `layout`/`conteiner` recebe a intenção padrão do motor
(`responsivo::intencao_padrao`): grade, `gap` `md`, 1 coluna, e 2/3/4 colunas a partir de
`sm`/`md`/`lg` **do conteiner `tela`**. O analisador de SQL escreve essa intenção no IR, então
o JSON gerado já a traz explícita.

Por que o alvo padrão é `tela` e não a própria seção: medido em 01/10, com a seção como alvo
a 1280 px ela ficava logo abaixo de `lg` (3 colunas), e a prova de fidelidade piorou
(revocação mínima 0,67 contra 0,75 da linha de base). Com `tela`, a 1280 px dá 4 colunas e a
fidelidade volta igual à linha de base em todas as medianas e faixas.

## Versões

- `de_json` lê v1, v2 e v3, e recusa versão maior que a do leitor.
- `layout`/`conteiner` num JSON com `ir_version` < 3 é **recusado**: intenção de layout num v2
  é mentira de versão.
- Toda intenção passa por `responsivo::validar` ao ler: nome de conteiner e token vão parar
  dentro do CSS gerado, e texto livre ali seria deixar o IR escrever folha de estilo.
- v1/v2 sem intenção desenham **igual** ao v3 com a intenção padrão (teste
  `ui_ir_v1_continua_lendo_e_versao_do_futuro_se_recusa`).

## Pontos de quebra

`crates/phxclaw-ui-ir/breakpoints.json`: `sm 576`, `md 768`, `lg 992`, `xl 1200`, `xxl 1400`
— os do Bootstrap 5.3 (`$grid-breakpoints`). As constantes Rust (`responsivo::bp`) e todo CSS
gerado saem dali; o teste `numeros_de_quebra_so_no_json` reprova o número escrito em outro
lugar do `src/` da crate ou da prova responsiva do agente.

## O motor (`responsivo.rs`)

- Grid na estrutura (a seção), Flexbox nas barras (ações, totais, controle + botão).
- `@container` no componente; `@media` **só** na composição geral (menu + conteúdo).
- `min-inline-size:0` nos filhos de grade e fila.
- Tabela com rolagem **própria** (exceção bidimensional da WCAG 1.4.10); em conteiner
  `tabela` abaixo de `sm`, a linha vira cartão e cada célula mostra o rótulo (`data-rotulo`).
- Nunca `overflow:hidden` na página.
- O mesmo motor serve os adaptadores `html` («phoenix») e `bootstrap`, e o React lê as classes
  calculadas no JSON embutido (`_conteiner`, `_layout`, `_largo`).

## Prova

`phxclaw ui responsivo [--alvo html|bootstrap] [--bootstrap-css ARQ]` — ver
`crates/phxclaw-agent/src/responsivo_ui.rs` e os resultados em `docs/ui/fidelidade/`.

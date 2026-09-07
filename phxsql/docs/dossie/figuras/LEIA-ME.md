# As figuras soltas do dossiê

SVG que vive **fora** do `dossie-phxsql-*.html` e é embutido nele (ou aberto
sozinho). As convenções saem da Figura 8 do dossiê, e as três primeiras são o
que faz a figura funcionar nos dois lugares:

1. **Cor por variável, com fallback**: `var(--ndx, #6a44a8)`. Dentro do dossiê
   ela segue o tema (claro, escuro e a preferência do sistema); aberta sozinha,
   cai na cor da marca. As variáveis são `--acento`, `--reg`, `--ndx`, `--bin`,
   `--memo`, `--log`, `--ok`, `--pend`, e o texto usa `currentColor`.
2. **Sem `<style>` e sem `<script>` dentro do SVG.** Uma folha de estilo dentro
   do arquivo ganha da do dossiê e trava a figura num tema só.
3. **Texto de 9 a 13 px**, marcadores de seta em `<defs>`, `role="img"` e um
   `aria-label` que **conta a figura inteira** — é o que quem não enxerga lê.

## Provar é obrigatório, e não é ler o arquivo

```bash
node docs/dossie/figuras/provar-figuras.mjs
```

Abre cada figura em fundo claro e em fundo escuro (`#010418`, o da marca),
grava `docs/dossie/capturas/<figura>-<tema>.png` e **reprova três coisas** que
ler o fonte não acha:

- **fora do `viewBox`** — a caixa real do texto contra o quadro;
- **sobrepostos** — dois textos ocupando o mesmo lugar;
- **na borda** — texto meio dentro e meio fora de uma caixa, riscado pela linha.

Na primeira prova das figuras do auto number, as três conferências acharam 1, 3
e 1 defeito. Nenhum aparecia no fonte. *Interface só se prova exercitando.*

## O que há hoje

| figura | a afirmação |
|---|---|
| `autonumber-como-esta.svg` | os três números crescentes nascem em pontos diferentes do mesmo caminho, os três contadores moram no mesmo cabeçalho de 128 bytes, e divergem em três lugares medidos |
| `autonumber-ideal.svg` | os dois buracos medidos, a mudança que fecha cada um, e a linha de corte da migração |

As duas saem do `docs/AUTONUMBER.md`, e cada número delas de
`bancada/sequencias/resultados.json`.

# phx-grid — Phoenix Cognitive Data Engine · Grid

Data grid **ES5 estrito, zero dependências, offline-first** do ecossistema Phoenix (WX Soluções).
Onda 1 em construção conforme `PHX-GRID-PLANO-SPRINTS-v1.1` (28 + 10 sprints) sobre a
`PHX-GRID-ESPECIFICACAO-v1.2` (118 recursos, 22 telas + visão Cognitive).

## Uso

```html
<link rel="stylesheet" href="phx-grid.css">
<script src="phx-grid.js"></script>
<script>
var grid = PhxGrid.criar("#alvo", {
  colunas: [
    { campo: "quando",  titulo: "Data",        tipo: "dataHora", dimensao: "dim_tempo" },
    { campo: "cliente", titulo: "Cliente",     dimensao: "dim_cliente" },
    { campo: "vtotal",  titulo: "Valor Total", tipo: "moeda",    agregador: "sum" },
    { campo: "margem",  titulo: "Margem %",    tipo: "percentual", agregador: "avg" }
  ],
  dados: minhasLinhas,                 // OU fonte: {carregar(params, cb)}
  bandas: ["cliente",                  // S02: campos soltos e bandas na ordem desejada
    { titulo: "2024", filhos: [
      { titulo: "Trimestres", colunas: ["q1", "q2", "q3", "q4"] },
      { titulo: "Acumulado",  colunas: ["total"] } ] }],
  pagina: { tamanho: 100, opcoes: [50, 100, 200] }
});
// fixa: "esq" | "dir" na coluna congela com sticky (offsets medidos automaticamente)
</script>
```

## `formato` é o DESENHADOR DA CÉLULA — não é máscara de número

Esta linha existe porque o nome engana, e engana de um jeito caro: procurando
por `render` ou `desenhar` não se acha nada, e conclui-se que o grid não
desenha célula. Um leitor experiente desta base já chegou a planejar
**construir** o gancho que estava pronto e testado.

```js
{ campo: "obrigatoria", titulo: "Obrigatória",
  // Recebe (valor, linha) e devolve HTML, que entra direto no <td>.
  formato: (v) => v ? '<span class="pino ok">NOT NULL</span>'
                    : '<span class="pino nao">NULO OK</span>' },

{ campo: "acoes", titulo: "", ordenavel: false,
  formato: (_, linha) => `<button class="botao mini" data-rowid="${linha.rowid}">editar</button>` }
```

Duas regras ao usá-lo:

- **O HTML é seu, e o escape também.** O grid não escapa o que `formato`
  devolve — é isso que permite o pino e o botão. Todo valor que vier do dado
  passa pelo `esc()` de quem escreve.
- **Rótulo pela fábrica de idiomas, dado nunca.** O texto do pino é rótulo e
  se traduz; o valor da coluna é dado e não se estiliza. É a mesma lição do
  «Blumenau» virando «BLUMENAU».

Botão dentro de `formato` não recebe `onclick`: o corpo é reescrito a cada
render. Ligue por delegação no contêiner, ou use `aoAbrirLinha`.

## Contrato de fonte remota (PWS/REST)

O grid envia `{ pagina, tamanho, ordem: { campo, dir: "asc"|"desc"|null, tipo } }` e espera
`cb(erro, { linhas: [...], total: N })`. É o mesmo contrato que evoluirá para o
**Query Planner com pushdown** (sprint C9): o cliente descreve a operação; o servidor devolve só o resultado.

## API
`ordenar(campo, dir)` · `pagina(n)` · `tamanhoPagina(n)` · `mostrarColuna(campo, bool)` ·
`moverColuna(campo, antesDe)` · `colunasVisiveis()` · `linhas()` · `estado()` · `logs()` ·
`redesenhar()` · `destruir()`

## Qualidade
- Suíte `tests/grid-nucleo.test.js` (16 blocos, verde 2× exigido): `node tests/run.js`
- Gates: `node --check` + acorn `ecmaVersion: 5`
- Demos single-file geradas por `demos/build.py` (0 CDN, 0 src externo)
- Telemetria por sprint publicada no `CHANGELOG.md` (Chrome real, medida)

## Roadmap
O plano completo (fases, aceites, telemetria e logs exigidos por sprint) está em
`PHX-GRID-PLANO-SPRINTS-v1.1.html`. Próxima: **S09 — drill-down: linha de detalhe expansível (master-detail)**.

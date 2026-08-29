# Os roteiros que provam a interface

Interface só se prova exercitando. Estes quatro roteiros são o que sustenta os
números de `docs/DESIGN.md` — nenhum deles foi estimado.

## Antes de rodar

Suba um `phxsqld` **seu**, com config própria e portas próprias, e semeie
dados (grade vazia não prova layout). O `exercicio.mjs` fala com
`http://127.0.0.1:5770` e entra com `adriano` / `design123` / token
`design-token`; ajuste as três constantes no alto do arquivo se as suas forem
outras.

**Nunca derrube um `phxsqld` que não seja o seu.** Há o de demonstração e os
de outros agentes, e todos sobem com `--config config.json` — casar por esse
padrão mata os alheios. Mate pelo **PID que você criou**, ou case pelo caminho
do seu worktree.

## Os roteiros

| Arquivo | O que faz |
|---|---|
| `exercicio.mjs` | 32 telas × 3 viewports (390 / 820 / 1440) × 2 temas = 192 combinações. Mede rolagem do corpo, transbordo, corte-sem-rolo e contraste; guarda uma captura de cada e um `relatorio.json` |
| `lateral.mjs` | 27 conferências do painel retrátil: recolhe, expande, pina, despina, sobrevive ao recarregar, arrasta a largura, anda pelo teclado, e o botão de reabrir existe em todos os estados |
| `colisoes.py` | varre o `<style>` do `index.html` e lista todo nome de classe com dois blocos de declaração fora de `@media` — o padrão de defeito da §3 do `DESIGN.md` |
| `contraste.py` | a conta de contraste fora do navegador, para escolher um token novo antes de gastar uma rodada de build |

```
node exercicio.mjs <pasta-de-saida> [rotulo]
node lateral.mjs   <pasta-de-saida>
python3 colisoes.py ../../crates/phxsql-server/ui/index.html
python3 contraste.py
```

Os `.mjs` importam o Playwright de `/opt/node22/lib/node_modules/playwright`
e usam o chromium de `/opt/pw-browsers` (`PW_BROWSERS_PATH`).

## Por que a medição é do jeito que é

O reflexo é medir `document.documentElement.scrollWidth <= innerWidth`. Com
`body{overflow:hidden}` — que o console tem, e deve ter — **isso é sempre
verdade**, inclusive numa tela cortada ao meio. A primeira corrida respondeu
«0 de 192 telas vazam» com o celular quebrado.

Por isso o roteiro mede três coisas, e não uma:

1. a rolagem do corpo (piso, mantido);
2. **o que passa da borda direita sem ter um contêiner rolável no caminho** —
   um botão da barra de ferramentas em 1806px não é defeito, porque a barra
   rola; a página é que não pode rolar;
3. **o que está cortado sem rolo** — `overflow:hidden` com `scrollWidth >
   clientWidth`, ignorando quem corta com reticências (reticência avisa;
   corte mudo não). Conteúdo inalcançável é pior que rolagem, e foi aqui que
   apareceram as 28 telas quebradas.

E o contraste compõe os fundos translúcidos até a primeira superfície opaca:
medir a cor declarada do elemento contra a cor declarada do pai erra sempre
que houver um `rgba()` no meio — e há.

# A bancada do painel de bolhas

Interface só se prova exercitando, e este diretório é o que torna isso barato:
o painel de bolhas da telemetria é exercitado **sem servidor e sem carga**,
porque o módulo (`ui/telemetria.js`) não fala com o servidor — ele recebe uma
função `api(op, params)` de quem o chama.

```bash
python3 bancada/telemetria/monta-bancada.py      # gera bancada.html
node    bancada/telemetria/conferir-desenho.mjs  # a geometria e o contraste
node    bancada/telemetria/conferir-interacao.mjs # o clique, a busca, os níveis
node    bancada/telemetria/prova-das-cores.mjs   # as cores configuráveis, com servidor
```

`conferir-interacao.mjs` grava três capturas ao lado dele; `TLM_CAPTURAS=<dir>`
manda para outro lugar. As duas coisas que ele gera — a página montada e as
capturas — ficam fora do repositório: retrato guardado envelhece calado.

`bancada.html` também se abre à mão no navegador: a barra do topo troca entre
oito retratos inventados (1, 3, 8, 12 e 40 atividades, uma dominante de 27,6 s,
todas parelhas, nenhuma) e o tema.

O Playwright **não entra no projeto** — a regra de zero dependência vale, e um
conferidor de tela não é motivo para quebrá-la. Os dois `.mjs` o procuram em
`/opt/node22/lib/node_modules/playwright/index.mjs`; `PLAYWRIGHT=<caminho>`
aponta para outro lugar.

## Por que a bancada monta o CSS global junto

Ela recorta o `<style>` do próprio `index.html` em vez de guardar uma cópia.
Cópia envelhece calada, e o que a bancada precisa provar é justamente o escopo
`.tlm` contra as regras que existem **hoje** — `input{width:100%}` e
`label{text-transform:uppercase}`, as duas que mordem todo componente novo.

## O que cada conferidor pega

**`conferir-desenho.mjs`** — oito retratos × três larguras × dois temas, e
falha quando:

- um rótulo sai da esfera (o rótulo é medido contra a **corda** do círculo, não
  chutado pelo raio);
- uma esfera **ou a sombra dela** sai da caixa (a primeira versão media só o
  círculo, e o que vazava era o `feDropShadow` — que o olho lê como bolha
  cortada);
- um alvo de clique fica abaixo de 11 px;
- o contraste do rótulo contra o corpo da esfera cai de 4,5:1 em qualquer tema.

**`conferir-interacao.mjs`** — o que não aparece numa captura:

- clicar na **menor** bolha do painel com o desenho vivo;
- a deriva morrer com o ponteiro dentro e voltar quando ele sai;
- descer para a vista por estação, entrar numa e voltar pela trilha;
- a busca filtrar, e o painel vazio dizer **por que** está vazio;
- ocultar e mostrar a legenda;
- escolher uma bolha pelo teclado;
- `prefers-reduced-motion` deixar tudo parado.

**`prova-das-cores.mjs`** — o único dos três que **sobe um servidor**, porque a
cor configurável atravessa o `config.json`, o servidor e a tela, e nenhuma das
três pontas se prova sem as outras duas. Ele usa as portas **6600/6601** e
derruba o processo pelo PID no fim. Sobe 400.000 linhas, põe quatro somas de
verificação concorrentes para produzir os três estados vivos, e falha quando:

- a carga não produz bolhas de estados diferentes (senão a prova seria de um
  painel todo azul, e passaria dizendo nada);
- a legenda de fábrica não diz a palavra da cor **e** o traço da borda;
- a cor escolhida na tela não chega ao painel depois de salvar;
- a palavra «amarelo» sobrevive ao amarelo ter virado roxo;
- um rótulo fica abaixo de 4,5:1 dentro da bolha, em qualquer tema;
- o aviso de contraste não aparece para uma cor de meio-tom (`#797979`, a que
  nenhuma das duas tintas salva).

Ele guarda as capturas com `--capturas <dir>`: painel de fábrica e trocado nos
dois temas, a legenda de perto, e a tela de Configurações com o aviso aceso.

O que os dois primeiros **não** provam é o servidor: a carga de verdade e o encerrar
pela bolha estão em `docs/TELEMETRIA.md` §4.6.

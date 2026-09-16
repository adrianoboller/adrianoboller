# A bancada da telemetria — o painel de bolhas, e o que ligar custa

São duas coisas nesta pasta, e elas não se confundem:

| arquivo | o que responde | grava |
|---|---|---|
| `custo.py` | **quanto custa ligar a telemetria**, por soquete | `resultados.json` |
| `monta-bancada.py` + os `.mjs` | **como o painel de bolhas se comporta**, no navegador | nada (retrato guardado envelhece calado) |

## `custo.py` — o preço da instrumentação, com a data ao lado

```bash
flock /tmp/phx-cargo.lock cargo build --release -p phxsql-server --bin phxsqld
python3 bancada/telemetria/custo.py
python3 bancada/telemetria/custo.py --autoteste   # só o veredito, em segundos
```

Ele existe porque a sétima página tinha uma seção que **não nascia** (pedido
265): *seção só entra com gerador*, e antes do gerador faltava a **fonte** —
esta pasta media o painel e não gravava `resultados.json`, então não havia a
data em que o número foi medido para pôr ao lado dele.

**Um servidor só, com o interruptor virado em tempo de execução.** O
`bancada/profiler/custo.py` sobe dois porque lá o que se mede é o *portão*
(`if false` contra `if ligado.load(Relaxed)` são dois binários). Aqui a
pergunta é outra — o que os pontos de captura cobram quando estão ligados — e
`telemetria_ligar`/`telemetria_desligar` viram um `AtomicBool`: o mesmo
processo, os mesmos arquivos e o mesmo cache medem os dois lados, e some do
número toda diferença que não seja o interruptor.

**Três correções que a primeira corrida obrigou, e o número de cada uma:**

1. **Par curto, e muito par.** Nove pares de 2.000 `ping` numa máquina em carga
   12 deram a telemetria «acelerando» o servidor em **37%** — as amostras de um
   mesmo lado iam de 54 a 128 µs. O trabalho total é quase o mesmo; o que muda
   é o **grão** do par.
2. **O piso, não a média.** O que a telemetria acrescenta é uma **constante**,
   e num `t = base + c` o `c` aparece inteiro no *menor* tempo. A média mede o
   vizinho: a mediana do `ping` andou de 59 µs para 161 µs entre duas corridas
   com dois minutos de diferença, sem uma linha mudar.
3. **Veredito e estimativa saem da mesma lista.** Numa corrida o teste de sinal
   disse «ligada custa» com **231 de 400** pares a favor e a estimativa ao lado
   dizia **−0,04%**. Hoje o custo é a **mediana das diferenças por par**, e o
   sinal conta essas mesmas diferenças — os dois não podem discordar.

**O veredito é um teste de sinal a 3 σ**, e não a regra das faixas min–max do
pedido 155: aqui o efeito é de ~1% e o ruído de uma máquina compartilhada chega
a 3×, então as faixas sempre se cruzam e a regra crua diria «não sei» para
sempre — inclusive se a telemetria dobrasse o custo. Carga que não resolve sai
como `resolvido: false`, e a seção publica «dentro do ruído» com a contagem dos
pares ao lado, para quem lê julgar o veredito em vez de acreditar nele.

O `--autoteste` prova a porta **nos dois sentidos** sem subir servidor: efeito
real resolve, ruído simétrico não, a borda dos 3 σ confere, e veredito e
estimativa nunca discordam.

Quem lê o `resultados.json` é `docs/status/telemetria-medida.py`, que escreve a
seção «Telemetria e logs» da sétima página. Porta **6340**, diretório próprio,
e mata **só o PID que subiu**.

---

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

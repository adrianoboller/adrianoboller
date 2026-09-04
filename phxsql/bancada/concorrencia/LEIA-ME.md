# A bancada da concorrência

Quatro arquivos, e cada um responde **uma** pergunta. A ordem abaixo é a ordem
em que elas se fazem — pular a primeira leva a medir a segunda sem saber se
havia o que medir.

| arquivo | a pergunta | precisa de máquina parada? |
|---|---|---|
| `mapa-da-trava.py` | **o que a trava segura?** Quantas seções críticas, e o que cada uma faz enquanto está com a trava na mão | **não** — lê o fonte |
| `a-trava-serializa.py` | **a trava custa?** Vazão com N clientes, contra uma curva de controle que não toma a trava | sim |
| `escolher-o-desenho.py` | **o que pôr no lugar?** O teto de cada um: trava por tabela, `RwLock`, MVCC | sim |
| `quanto-a-trava-fica-presa.py` | **quanto a trava fica PRESA?** O µs de posse por operação, lido por dentro (telemetria), com o par `por_lote` × `por_operacao` isolando o `fsync` | sim |
| `quieta.py` | **este número vale?** O vigia que reprova a bateria rodada em máquina ocupada | — |
| `ruido-do-controle.py` | **o teto do próprio vigia está certo?** Muitas corridas seguidas de `ping` puro, para medir — e não citar — a dispersão que o controle mostra hoje, e testar se `tolerancia_controle` merece descer | sim, para achar a base limpa; sem ela, mede a sujeira mesmo |

O relatório que sai dos quatro primeiros está em
[`docs/CONCORRENCIA.md`](../../docs/CONCORRENCIA.md). O quinto responde por
ele mesmo: sem uma medição da dispersão do controle, "quieta" era uma palavra
com um número ao lado que ninguém tinha corrido para confirmar.

## Como rodar

```bash
cargo build --release                                        # os dois de vazão precisam

python3 bancada/concorrencia/mapa-da-trava.py --autoteste     # as guardas do medidor
python3 bancada/concorrencia/mapa-da-trava.py                 # o mapa
python3 bancada/concorrencia/mapa-da-trava.py --classe codigo-do-dono
python3 bancada/concorrencia/mapa-da-trava.py --json          # para outro gerador

python3 bancada/concorrencia/a-trava-serializa.py
SEGUNDOS=5 CLIENTES=1,2,4 python3 bancada/concorrencia/escolher-o-desenho.py
GRAVACOES=4000 LEITURAS=400 python3 bancada/concorrencia/quanto-a-trava-fica-presa.py

RODADAS=30 python3 bancada/concorrencia/ruido-do-controle.py         # o teto do vigia
RODADAS=40 RODADA_S=1.5 python3 bancada/concorrencia/ruido-do-controle.py --json

python3 bancada/concorrencia/escolher-o-desenho.py --autoteste   # a conta do teto exclusivo
python3 bancada/concorrencia/mapa-da-trava.py --catraca           # os tres tetos de QA
```

**A catraca do mapa e a unica coisa desta pasta que roda sozinha**, como item 0
da `bancada/bateria/prova-bateria.py` -- antes de qualquer servidor subir,
porque e estatica. Ela guarda tres tetos: `codigo-do-dono` (5),
`alcancam-fsync` (22) e `rede-ou-espera` (0). SO DESCE: medir mais reprova
porque alguem acrescentou o que a lei proibe, e medir MENOS tambem reprova,
porque quem melhorou baixa o teto no mesmo commit.

**As corridas limpas ficam versionadas em `corridas/`**, com data e hora no
nome. O medidor imprime e some; guardar a corrida crua e o que permite
conferir depois se o numero de um documento saiu dela ou da memoria de quem
escreveu.

**A guarda `quieta.confira_a_pagina` roda antes de qualquer numero**, e vale
saber por que ela existe: ate 04/09 as QUATRO bancadas desta pasta mandavam
`{"varrer", ..., "limite": 50}`. O `op_varrer` le o campo **`max`**; `limite`
nao existe no pedido e era ignorado em silencio, entao toda leitura devolvia
1.000 linhas -- o teto de configuracao. Nenhuma das quatro podia perceber:
**como todas mandavam o mesmo campo errado, nenhuma discordava de nenhuma.**
Quem pegou foi um medidor de OUTRA camada (`--example onde-doi-na-leitura`,
em processo) discordando do de rede.

A guarda recebe o **construtor de pedido da propria bancada** e confere que
pedir N linhas devolve N. Receber o construtor, e nao montar o pedido por
conta propria, e a diferenca entre conferir o servidor e conferir a bancada --
a primeira versao dela montava `{"max": n}` sozinha e teria passado com o
defeito de pe.

**O `--autoteste` do `escolher-o-desenho.py` prova UMA conta**, e ela merece o
teste porque eu errei a leitura dela: o teto que o relatorio imprime para o
MVCC (`leitor-com-escritor / leitor-sozinho`) inclui o custo de haver
**qualquer** segundo cliente, que o `RwLock` ja recupera. O que so o MVCC
compra e `leitor-com-escritor / dois-leitores` -- e a diferenca nao e
academica: em 04/09 o primeiro deu 1,19x-1,38x e o segundo deu 0,91x-1,13x.
O autoteste traz as quatro medicoes gravadas e exige que o denominador antigo
de um numero DIFERENTE, senao troca-lo de volta passaria despercebido.

**O quarto medidor tem um controle que os outros não têm**, e vale escrever por
quê. Os três primeiros comparam contra o `ping`, que não toma a trava — isso
prova que a máquina não andou. O `quanto-a-trava-fica-presa.py` compara também
contra a **leitura**, que toma a MESMA trava e não sincroniza nada: se ela
andar entre as duas baterias, a deriva é da máquina e não do `fsync`. Medido em
03/09, ela andou 1,01× e 0,95× — ficou parada, e é isso que dá o direito de
dizer que os 10,3×–12,3× da gravação são o `fsync`.

Variáveis: `SEGUNDOS` (por rodada), `LINHAS` (semeadas por tabela), `CLIENTES`
(a curva, só no `escolher-o-desenho.py`), `PHX_PHXSQLD` (outro binário).

**Portas 7600–7699**, escolhidas **livres** dentro da faixa — fora dela há
servidor de outra frente. O servidor é morto **pelo PID**, nunca por `pkill`:
matar por nome derruba o de quem está medindo ao lado, e isso já derrubou uma
sessão aqui.

## As três coisas que esta pasta aprendeu, e que valem para qualquer medidor

**1. Medidor de concorrência erra para o lado da hipótese.** Numa máquina
ocupada a curva achata — e a curva achatada é exatamente o sintoma que se
esperava da trava. O ruído *confirma*, com casas decimais. Por isso a recusa é
o comportamento e não um aviso: bateria reprovada **não imprime número**.
Publicar sujo com ressalva ao lado não resolve nada, porque três documentos
adiante a ressalva ficou para trás e o número virou fato.

**2. Instrumento que acusa a si mesmo recusa sempre.** A primeira versão do
vigia contava as tarefas rodáveis sem descontar as do próprio arnês, e acusava
«4 tarefas além do medidor» numa rodada de dois clientes — que eram os dois
clientes, o servidor e o amostrador. Recusar sempre não é mais útil que nunca
recusar; as duas coisas são o mesmo instrumento quebrado.

**3. Medidor estático nunca quebra — passa a responder outra coisa.** É o
motivo do `--autoteste`, e as seis guardas dele não são hipotéticas: cada uma
repõe um defeito que o `mapa-da-trava.py` de fato teve, incluindo o dia em que
ele classificou o `op_juntar` como «atravessa a rede com a trava na mão» com
confiança 1,0 — por causa de uma **fechadura local** chamada `montar` que o
resolvedor por nome confundiu com o `Cliente::montar(fluxo: TcpStream, …)` do
`replica.rs`. Conferido à mão, `juncao.rs` tem zero operações de rede.

**4. «Zero vizinhos» não é «máquina parada» nesta caixa.** Medido em 04/09 com
o `ruido-do-controle.py`: em 10 das 30 corridas o `quieta.Amostra` não viu
nenhum vizinho rodável (`procs_running` excedente = 0) e mesmo assim a
ocupação ficou em 50–83% e o `ping` variou até 49,5% entre corridas.

A explicação óbvia é `steal` (o hospedeiro tirando a vCPU de baixo do
processo sem deixar rastro na fila local) — e é exatamente o tipo de
diagnóstico plausível que esta casa já errou por não medir. Medido direto do
`/proc/stat` no momento em que isto foi escrito, `steal` estava em **0,0%**;
a explicação não se sustenta como causa geral, e fica como hipótese **morta**,
não como fato. A explicação que os números sustentam é mais chata: o
`Amostra` lê `procs_running` em só 3–5 instantes por janela (`passos`), e um
pico de tarefa rodável entre dois desses instantes passa batido, enquanto a
ocupação (`idle` acumulado pelo kernel ao longo da janela inteira) não perde
nenhum. Não foi medido qual dos dois pesa mais neste dia — só que `vizinhos`
sozinho já mostrou ter esse ponto cego, medido. É por isso que o vigia nunca
decide por um sinal só: `vizinhos`, `ocupada` e o `controle` cobrem pontos
cegos diferentes, e o `controle` foi o que pegou este.

## O método do mapa, em um parágrafo

Ele resolve chamada **por nome**, e nesta árvore há nome com 23 definições
(`abrir`), 33 (`para_json`), 35 (`nome`). Escolher uma seria chutar; unir todas
diria que o `op_varrer` grava em disco. Então **não se escolhe: conta-se** —
cada salto do caminho carrega a fração das definições daquele nome que alcançam
o marcador (`sincronizar(9/9)`), e a confiança do caminho é a **menor** fração
dele. Confiança 1,0 é afirmação: qualquer que seja o homônimo certo, o marcador
está lá. Abaixo disso é suspeita **com endereço**, e o caminho vai impresso
para quem lê conferir em vez de acreditar no rótulo.

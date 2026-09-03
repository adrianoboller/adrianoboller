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

O relatório que sai dos quatro está em [`docs/CONCORRENCIA.md`](../../docs/CONCORRENCIA.md).

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
```

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

## O método do mapa, em um parágrafo

Ele resolve chamada **por nome**, e nesta árvore há nome com 23 definições
(`abrir`), 33 (`para_json`), 35 (`nome`). Escolher uma seria chutar; unir todas
diria que o `op_varrer` grava em disco. Então **não se escolhe: conta-se** —
cada salto do caminho carrega a fração das definições daquele nome que alcançam
o marcador (`sincronizar(9/9)`), e a confiança do caminho é a **menor** fração
dele. Confiança 1,0 é afirmação: qualquer que seja o homônimo certo, o marcador
está lá. Abaixo disso é suspeita **com endereço**, e o caminho vai impresso
para quem lê conferir em vez de acreditar no rótulo.

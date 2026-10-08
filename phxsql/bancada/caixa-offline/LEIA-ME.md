# 20 caixas e 1 central, com o central derrubado no meio do expediente (pedido 678)

```bash
CARGO_INCREMENTAL=0 cargo build --release -p phxsql-server
bancada/esta-medindo.sh && echo "ha medicao em curso -- espere"
python3 bancada/caixa-offline/medir.py 5            # 5 voltas, 20 caixas, 20 s por fase
python3 bancada/caixa-offline/medir.py 1 --caixas 2 --fase 5   # ensaio
```

Ensaio não sobrescreve o resultado da casa: `PHX_CAIXA_OFFLINE` muda o
diretório (padrão `/tmp/phx-caixa-offline`) e `PHX_CAIXA_SAIDA` o arquivo de saída.
Portas **7300** (central) a **7320**.

## O desenho medido

É o **espelho** do pedido 325 (`docs/propostas/caixa-offline-325.md`, MANUAL
§16.1). Cada caixa é um `phxsqld` dono do database `caixaNN`. O central é
réplica das 20 origens, cada uma com `"espelho": true`. O cadastro
(`cadastro.produtos`) só se escreve no central e **desce** para os caixas pela
via inversa, também com `"espelho": true`. Os 21 nós são `papel: replica` com
escrita local: cada um escreve no que é dele e recebe o que é do outro.

Uma venda é o que o contrato do 680 diz que o caixa faz: lê o preço do
**cadastro local**, que é o último que desceu, e grava numa transação só:

```
begin; 1 vendas; k itens (3 a 10, FK conferida para vendas); k atualizar estoque; commit
```

## As quatro perguntas, e como cada uma se mede

| # | pergunta | como |
|---|---|---|
| 1 | as 20 origens chegam juntas ao central? travam na trava global? | por venda, do `COMMIT` ok no caixa até ela aparecer num `varrer` do central. Para a trava, uma **sonda** no central faz `ping` (não toca a trava) e `varrer` (toca) a cada 20 ms, também com o central **ocioso** por 3 s antes da primeira venda. Se só o `varrer` sobe, é a trava |
| 2 | com o central morto (`SIGKILL`, pelo PID do `Popen`), o caixa vende? | conta a venda recusada e mede a venda no caixa, do `begin` ao `commit`, **antes / durante / depois** |
| 3 | ao religar, quanto tempo até alcançar? chegou alguma pela metade ou duplicada? | tempo até atender e até ter tudo o que os caixas tinham cometido no instante do religar. Durante a volta inteira, um **sanduíche** (`vendas`, `itens`, `vendas`) no central exige que os itens visíveis sejam exatamente a soma dos itens das vendas visíveis. No fim, contagem e **SHA-256 linha a linha** de `vendas`, `itens` e `estoque`, caixa contra central |
| 4 | quanto o diário do caixa cresce por venda? | bytes dos `.log` do database `caixaNN`, depois menos antes, divididos pelas vendas cometidas |

## Como ler a chegada

A chegada **não é custo de transporte**. Quando a rodada anterior não achou
nada, o laço da réplica dorme `reconectar_em` (1 s, o mínimo que o
`config.json` aceita) antes de perguntar de novo. A chegada soma esse sono, o
transporte e a resolução do observador (`rodada_do_observador_ms`). O custo de
levar o dado está medido à parte, em `bancada/quorum/`.

Ela vai **separada pela fase em que a venda foi cometida**. As vendas do
«durante» medem a queda, não a replicação. As do «antes» que o `SIGKILL`
pegou antes de serem puxadas vão para `antes_pega_pela_queda`. No primeiro
ensaio elas estavam misturadas, e o p95 do «antes» dizia **10,7 s** com a
mediana em **0,6 s**.

## O que a bancada NÃO prova

- **O detector de meia venda não foi provado com o defeito reposto.** Ele dá
  zero, e a conferência final por SHA também dá igual. Zero, aqui, quer dizer
  «não vi», e não «provei que pega». A prova com o defeito reposto é do
  `tests/venda-inteira-na-replica.rs` (676) e da guarda do 682.
- **Os observadores pesam no central**: a cada rodada, 20 `varrer` e 60
  `agrupar`. A sonda mede a trava **com** essa carga junto.
- **É loopback, num contêiner de 4 núcleos**, com 21 processos e o cliente em
  Python. Rede de loja de verdade soma o RTT a cada um dos ~17 pedidos de uma
  venda.
- A queda é do **processo**, não da rede. O cabo cortado em silêncio tem
  bancada própria (`bancada/replicacao/trava.py`, `docker/`).
- O caixa **não** é derrubado. A queda do caixa é a da origem, coberta pela
  retomada da `bancada/replicacao/`.

## Os números de 08/10/2026 (commit `bf4bbdb9`, 5 voltas, 20 caixas, 20 s por fase)

Faixa = **min–max das 5 voltas**; entre parênteses, a mediana das voltas.

| o quê | número |
|---|---|
| vendas por volta | 5.576–5.660 (≈ 280 por caixa, 6,5 itens em média) |
| chegada ao central, central no ar, **mediana** | 539–558 ms antes da queda; 569–627 ms depois |
| chegada, **p95** / **máx** | 1.015–1.127 ms / 1.256–2.065 ms |
| vendas que o `SIGKILL` pegou antes de puxadas | 32–48 por volta; chegaram todas depois de religar |
| **venda recusada no caixa** | **0** nas 5 voltas (antes, durante e depois) |
| venda no caixa, mediana antes / durante / depois | 11,4–12,6 / 9,5–10,1 / 10,8–12,9 ms |
| venda no caixa, máx antes / durante / depois | 102–116 / 34–105 / 45–82 ms |
| central religado: atende | 21–51 ms |
| central religado: alcança as 20 origens | **1,38–1,86 s** (1,50), com 3.737–3.781 vendas cometidas no religar |
| sonda `varrer` no central, máx: ocioso / com 20 origens / alcance | 1,1–3,4 / 46–103 / **251–595 ms** |
| sonda `ping` no central, máx: ocioso / com 20 origens / alcance | 1,0–4,1 / 6,7–12,9 / 9,2–43 ms |
| meia venda vista (sanduíches conferidos) | **0** em 5.084–5.574 por volta |
| `vendas`, `itens`, `estoque`: SHA-256 caixa = central | **iguais** nos 20 caixas, nas 5 voltas; contagem igual, zero duplicada |
| diário (`.log`) do caixa por venda | **1.454–1.629 B** por caixa (mediana 1.549–1.564) |
| disco inteiro do `caixaNN` por venda | 2.672–2.706 B (mediana das voltas) |
| pico de disco da bancada | 60–62 MB por volta |

A leitura que importa:

- **A trava global aparece, e não trava.** Com o central ocioso, o `varrer`
  máximo é de 1–3 ms. Com 20 origens chegando, sobe para 46–103 ms. No
  alcance depois do religar, sobe para 251–595 ms. O `ping`, que não toca a
  trava, fica em 7–43 ms (os 4 núcleos com 21 processos). A diferença é a
  leitura esperando a aplicação. Nenhuma origem parou, nenhuma deu erro
  (`replicacao_estado`: `ultimo_erro` nulo e zero `transacoes_em_pedacos` nas
  20).
- **O caixa fica mais rápido com o central caído** (mediana 9,9 contra 12,2
  ms), porque deixa de atender os `replicar` do central.
- **A chegada é o sono do laço.** Mediana ~0,55 s e p95 ~1,02 s, com
  `reconectar_em` de 1 s. Prometer «chega em até 1 s» é prometer o sono,
  não o transporte.
- **O diário cresce ~1,55 KB por venda de 6,5 itens**, e não tem expurgo
  (lacuna L3 do 325). Um caixa com 1.000 vendas por dia grava ~1,5 MB/dia de
  `.log`, ~2,7 MB/dia contando tudo. Conta derivada, não medida em um dia.

## Com o expurgo do diário ligado (pedido 706)

```bash
python3 bancada/caixa-offline/medir.py 1 --caixas 4 --fase 120 --expurgo
```

Os caixas sobem com `"diario": {"expurgo": true, "consumidores":
["central"], "volume_kib": 64, "passada_s": 2, "prazo_dias": 30}`: o volume
no piso e uma passada a cada 2 s comprimem meses de loja em minutos. O prazo
é o do dono (30 dias), que nenhuma volta alcança, então aqui só sai o que o
central confirmou. O resultado vai para `resultados-expurgo.json`, ao lado, e
o `resultados.json` do 678 fica como está.

Com `--prazo-s N` (o `diario.prazo_s` de ensaio) e a fase maior que N, o
central fica fora ALÉM do prazo: o caixa solta o diário que ele não puxou, e o
central, ao voltar, se refaz pelo retrato do caixa sozinho
(`central_refeito_por_retrato` conta as linhas que ele escreve no
`servidor.log`). Ex.: `medir.py 1 --caixas 4 --fase 120 --expurgo --prazo-s 30`.

O que a volta acrescenta, em `diario_com_expurgo`:

- **`log_por_caixa_kib_por_fase`**: os bytes de `.log` de cada caixa, a cada
  segundo, por fase. O disco estável é o `antes` e o `fim` limitados enquanto
  o `log_gerado_por_caixa_kib` cresce. O `durante` mostra o diário segurando o
  que o central caído não confirmou.
- **`log_gerado_por_caixa_kib`**: o que ficou mais o que o próprio caixa
  disse ter tirado, nas linhas `expurgo do diario: N volume(s), E evento(s),
  B bytes` do `servidor.log`. É medido por quem apagou, e não estimado aqui.
- **`vendas_perdidas_no_central`**: as cometidas menos as que o central tem
  no fim. Junto da conferência de sempre (SHA-256 linha a linha das três
  tabelas), é a prova de nenhuma venda perdida com o central voltando depois
  do expurgo.

## Onde os números moram

Saem do `resultados.json` desta pasta: data, commit, versão e hora da
compilação do binário, as faixas min–mediana–max de todas as voltas
(`faixas`) e cada volta inteira (`voltas`). O binário mais velho que o fonte
**recusa medir**, porque medidor com binário velho mede o passado. A linha do
pedido 678 em `docs/PENDENCIAS.md` cita os números da corrida que fechou o pedido.

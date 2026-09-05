# Bancada de replicação — quatro servidores

Master + três espelhos, no ar de verdade, com a medição do que chega e em
quanto tempo. Como toda medição aqui, é para ser **refeita**.

```bash
cargo build --release
python3 bancada/replicacao/montar.py /tmp/phx-replicacao
python3 bancada/replicacao/medir.py 100000
```

`montar.py --cascata` põe o Slave03 puxando do Slave01 em vez do master, para
medir o segundo salto.

**O diretório vai nos dois lados.** O `montar.py` o aceita como argumento e o
`medir.py` o lê de `PHX_REPLICACAO`, porque o estágio da retomada precisa dele
em disco para subir o `slave03` de volta:

```bash
python3 bancada/replicacao/montar.py /tmp/outro
PHX_REPLICACAO=/tmp/outro python3 bancada/replicacao/medir.py
```

Montar num diretório e medir noutro **morria no último estágio**, depois de
três minutos de trabalho e com o `slave03` já derrubado pelo PID — um
`FileNotFoundError` cru. Desde 05/09/2026 a conferência acontece **antes** da
carga, e a recusa traz os dois comandos: *prova que só descobre no fim que não
podia começar mede o estrago, não a replicação.*

E o alvo do `kill` da retomada passou a ser resolvido por **caminho absoluto**.
Antes ele varria os `phxsqld` da máquina inteira procurando um `cwd` que
*terminasse* em `slave03` e guardava o **último** — com outra bancada de pé, a
seleção casava os dois e quem morria dependia da ordem do `pgrep`. Medido com
os dois vivos no mesmo instante: a seleção antiga casou
`/tmp/phx-rep-frente-dist/slave03` **e** `/tmp/phx-vizinho-frente-dist/slave03`;
a nova casou só o desta corrida. *Matar o servidor do vizinho já derrubou a
própria sessão aqui.*

| Arquivo | O que é |
|---|---|
| `montar.py` | escreve os quatro `config.json` e sobe os quatro processos |
| `medir.py` | a bancada: atraso por tipo de escrita, vazão, queda e retomada |
| `modos.py` | os quatro modos, nas portas 5330-5339 |
| `trava.py` | **a trava de dados contra a leitura de rede**, nas portas 7050-7055 — ver abaixo |
| `resultados.json` | a última corrida completa |
| `trava.json` | a última corrida do `trava.py` |
| `docker/` | **os mesmos quatro modos em contêineres**, que é onde endereço, firewall e partição existem de verdade — ver `docker/LEIA-ME.md` |

## O que esta bancada NÃO alcança, e por isso existe a de contêiner

Aqui tudo se enxerga por `127.0.0.1`. Isso torna três coisas impossíveis de
provar, e as três acharam defeito quando ficaram possíveis
(`bancada/replicacao/docker/`, `docs/REPLICACAO.md` §17):

- **endereço** — o `bind` do servidor e o endereço que a réplica procura são o
  mesmo por acidente. Num contêiner não são, e `bind: 127.0.0.1` não replica
  nada **sem avisar**;
- **firewall e isolamento** — no loopback não há o que trancar, e foi assim
  que o `replicas_autorizadas` passou versões sendo um campo que ninguém lia;
- **partição** — matar um processo é fácil; cortar a rede **sem matar
  ninguém** não existe aqui. E é a partição que mostra o abraço mortal do
  bidirecional.

## `trava.py` — o corte silencioso sem contêiner nenhum

```bash
python3 bancada/replicacao/trava.py            # os quatro estagios, ~1,5 min
python3 bancada/replicacao/trava.py congela    # so o corte silencioso
python3 bancada/replicacao/trava.py alcance    # so a vazao de aplicacao
python3 bancada/replicacao/trava.py queda      # so a conexao caindo
python3 bancada/replicacao/trava.py abraco     # so o bidirecional
```

O terceiro item da lista acima — cortar a rede sem matar ninguém — tem **uma
metade** que o loopback alcança, e é a que achou o defeito da §18: um **tubo**
em Python entre a réplica e o source, que repassa byte a byte até mandarem
emudecer e a partir daí segura os dois soquetes abertos **sem repassar nada**.
Do ponto de vista da réplica é o mesmo silêncio de um `iptables -j DROP`: a
conexão de pé, o pedido enviado, e a resposta que nunca vem.

O que ele **não** substitui continua sendo a queda de processo e a partição de
verdade — para essas vale a bancada de contêiner. O que ele compra é rodar em
1,5 min, em qualquer máquina, sem daemon nenhum: os dois números da §18 saíram
daqui, e batem com os do contêiner (30.079 ms contra 29.456 ms; 33,0 s contra
33,3 s).

Os quatro estágios, e por que cada um:

| estágio | o que mede |
|---|---|
| `congela` | com o tubo mudo, `ping` (não toca na trava) contra `varrer` (precisa dela) — mais `totais.trava_ms` da telemetria da réplica, que é a testemunha de dentro |
| `alcance` | a vazão de aplicação e o pior `varrer` do cliente **durante** um alcance de rotina, com a rede sã |
| `queda` | dez cortes de conexão de verdade no meio do alcance, julgados pela soma de verificação dos dois lados |
| `abraco` | as duas metades da carga escritas ao mesmo tempo num par bidirecional, com sonda rodando durante a carga |

Três armadilhas que esta bancada pagou, e que valem para qualquer uma:

- **prazo de cliente menor que o defeito mede «não respondeu» em vez de
  «esperou 30 s»**. A primeira versão morria de `timeout` no meio da medição.
  Toda conexão daqui nasce com prazo de leitura de 120 s;
- **o servidor fecha a conexão ociosa em `timeout_s` (30 s)**, e a sonda do
  abraço fica parada durante os 33 s da carga. A `Ligacao` refaz a conexão e
  repete o pedido uma vez — é a mesma armadilha que o `docker/LEIA-ME.md` já
  tinha registrado;
- **medir depois do estrago mede o que sobrou.** A primeira versão do `abraco`
  só sondava na convergência e publicou «pior `posicao` 0 ms» com a trava presa
  por segundos: quando as sondas começaram, a aplicação já tinha acabado. Hoje
  a sonda roda **durante** a carga.

## A topologia

```
Master 5800 ──┬──► Slave01 5801
              ├──► Slave02 5802
              └──► Slave03 5803
```

Quem procura é a réplica; o master não empurra nada. É o desenho do MySQL(R), e
existe por causa do firewall: o master abre **uma** porta de entrada e não
precisa alcançar ninguém de volta.

## O que ela compara — e por que não é «quantas linhas»

Duas tabelas com o mesmo número de linhas podem ter conteúdo diferente. A
bancada tira um **SHA-256 de cada linha inteira**, com `rowid` e `rownum`
juntos, lendo pelo cursor. Se um único campo atravessar errado, o retrato muda.

O `rowid` entrar na conta é o ponto: ele **não é transmitido**. O `.reg` nunca
reaproveita slot e o rowid é sempre `slot_count + 1`, então uma réplica que
aplicou todos os eventos na ordem chega ao mesmo número sozinha. Se não chegar,
divergiu — e a replicação para ali em vez de espalhar.

## A senha não fica em claro

O `montar.py` chama `phxsqld --senha` para gerar o `senha_hash`, e é dele que a
réplica deriva a chave do desafio-resposta. Não há senha em claro em nenhum
`config.json` que ele escreve.

## O que a última corrida mediu

**Os números estão no `resultados.json`, e ele é GRAVADO pelo `medir.py`** —
desde 05/09/2026, porque até ali o script só imprimia `RESULTADO` e alguém
copiava à mão. Esta seção não repete nenhum deles: *número digitado à mão
envelhece calado*. O arquivo traz junto o `quando`, a `versao` do binário e um
campo `maquina_ocupada`, que é o veredito do `bancada/esta-medindo.sh` no
instante da gravação — tempo medido com outra frente compilando é tempo
**nomeado**, e quem lê o arquivo depois não tem como adivinhar isso.

Ele **mescla** de propósito, preservando por nome as duas medidas que não saem
desta corrida: o `custo_da_imagem` (duas corridas com o interruptor mudando) e a
`cascata` (um `montar.py --cascata`). A primeira versão da gravação fazia um
`update` cego e deixou vivo um bloco `atraso_ms` de outra corrida ao lado dos
números novos — o próprio defeito que ela existe para matar, cometido dentro do
conserto. *Chave preservada é chave nomeada.*

E o `medir.py` **recusa** rodar sobre uma montagem já carregada. Rodar duas
vezes contra os mesmos quatro servidores parece funcionar: os ids da semente
colidem, as réplicas já estão alcançadas, `alcance_s` arredonda para 0,0 e a
divisão publica **63.390.598 eventos/s** — ao lado dos 20.826 da corrida
anterior, no mesmo arquivo. *Cenário que não exercita o caminho mede o caminho
errado, e o número que ele produz é o mais perigoso de todos, porque parece
medição.*

O que **não** muda de corrida para corrida, e por isso fica escrito: os quatro
retratos SHA-256 saem idênticos no fim, e o atraso de uma escrita até as três
réplicas é dominado pelo `reconectar_em` (2 s aqui) e não pelo trabalho —
quando a escrita cai logo depois de a réplica adormecer, o atraso é o que resta
da janela de 2 s. Baixar o intervalo baixa o atraso e sobe o tráfego de
perguntas em vão.

### O que estava escrito aqui, e estava errado

Esta seção dizia: «a réplica aplica mais devagar do que o master escreve — a
razão está no caminho: aplicar decodifica a imagem para `Value` e **reencoda**
o payload». Medido, a acusação não se sustenta: `aplicar_evento` custa
**16,15 µs** e uma inserção local pura custa **15,88 µs**
(`--example onde-doi-na-replica`). Decodificar e reencodar custam **0,27 µs**.

Os 4.273 eventos/s eram **229 µs por evento**, e o caminho de CPU inteiro dos
dois lados custa 20,5 µs. Os outros 208 estavam em dois lugares, nenhum deles
na réplica:

1. **O source varria o diário desde o começo a cada lote.** Servir «500 eventos
   a partir de P» caminhava pelos P anteriores lendo o cabeçalho de cada um —
   alcançar 100.000 em lotes de 500 custava **4,07 s só do lado de quem serve**
   (`--example custo-do-desde`). Com a marca de posição, **0,09 s: 45×**.
2. **O laço dormia depois de toda rodada, inclusive das produtivas.** O
   `reconectar_em` é o intervalo entre perguntas **em vão**; uma rodada que
   aplicou eventos volta na hora.

E um terceiro, menor: `bytes_para_hex` fazia um `format!` — e uma alocação de
`String` — **por byte** da imagem. Tabela de dígitos no lugar: 3,48 → 0,24 µs
por evento, **14,5×**.

**4.273 → 17.450 eventos/s por réplica: 4,08×**, e o alcance de 100.000 eventos
caiu de 18,7 s para 5,7 s. Em conjunto as três aplicam ~52.000 eventos/s, mais
do que o master escreve — o que era o pedido 111.

## O que a imagem custa no master

Mesma tabela, mesmas 100.000 linhas, só o interruptor mudando:

| `imagem_da_linha` | linhas/s | bytes por evento | `.log` |
|---|---:|---:|---:|
| desligada | 21.740 | 44 | 4,4 MB |
| ligada | 19.531 | 223 | 22,3 MB |

**10% mais devagar, e um diário 5,1× maior.** É o preço de a réplica receber a
linha e não só o aviso de que ela mudou. Quem só quer auditoria deixa desligado.

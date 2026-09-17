# Bateria de replicação, cluster e quórum — 17/09/2026

*Papel F (usuários de teste e revisor de prova real). Ordem do dono, 17/09/2026
02:27 UTC: «Replicação: bateria de testes, revisão e conclusão». Esta frente é a
**bateria**: rodar de verdade, contra servidores de pé, e entregar os números
com a data. A revisão de código é do SEC e do DBA; a conclusão escrita é da
documentação.*

Tudo aqui foi **medido nesta corrida**. Nenhum número desta página foi copiado
de documento anterior sem estar na coluna «anterior», e cada coluna «anterior»
diz de onde veio.

## 0. Como a corrida foi montada

| passo | o que foi feito | prova |
|---|---|---|
| binário | `flock /tmp/phx-cargo.lock cargo build --release` | `target/release/phxsqld` de **02:29:41 UTC**, contra `crates/phxsql-server/ui/index.html` de **01:01:51** — o binário ficou **5.270 s mais novo** que a tradução da noite. *Medidor com binário velho mede o passado.* |
| musl | `cargo build --release --target x86_64-unknown-linux-musl --bin phxsqld`, sob o mesmo `flock` | 50,77 s; `target/x86_64-unknown-linux-musl` = **56 MB** |
| portão | `bash bancada/esta-medindo.sh` antes de **cada** bancada de tempo | saiu **1 (livre)** nas oito vezes |
| carga | `/proc/loadavg` no instante de cada largada | coluna «carga» da tabela §1 |
| disco | 2,3 GiB livres no início; pior momento **1.580 MiB** (durante a bancada de contêiner); 1.952 MiB no fim | `df` antes e depois de cada bancada |
| limpeza | cada diretório de bancada apagado **por caminho**, depois de provar que nenhum processo vivo o usava (`/proc/<pid>/cwd`) | nenhum `phxsqld` ficou solto ao fim |

## 1. Bancada × medida × hoje × antes × veredito

### 1.1 Replicação clássica — `montar.py` + `medir.py`

Rodado às **02:30:22–02:31:11 UTC**, carga 1,36. 100.000 linhas, master + três
espelhos.

| medida | hoje (17/09) | antes (07/09, do próprio `resultados.json`) | veredito |
|---|---:|---:|---|
| master | **45.117 linhas/s** | 33.883 | **1,33×** |
| réplica aplica | **43.606 eventos/s** | 37.311 | 1,17× |
| alcance das três | **2,3 s** | 2,7 s | melhor |
| retomada: voltou a atender | **296 ms** | 335 ms | melhor |
| retomada: alcance | **0,3 s** | 0,3 s | igual |
| retrato SHA-256 dos quatro | `72554b753253cd5d` nos **quatro** | iguais | **PASSA** |
| linhas no fim | 105.001 | 105.001 | igual |
| atraso até as três (faixa) | **333–2.004 ms** | 1.134–2.012 ms | dominado pelo `reconectar_em` de 2 s, como o arquivo já diz |

**O `master_linhas_s` é o número que o pedido 193 deixou aberto.** Ver §5.1.

### 1.2 Os quatro modos — `modos.py` (**esta bancada NÃO grava arquivo**)

Rodado às **02:32:21–02:33:45 UTC**, carga 0,53. **9 de 9 estágios [ok]**. Ela
imprime `RESULTADO` e **não grava `resultados.json`** — é a pendência que o
pedido 193 nomeou e que continua de pé. Por isso o veredito por estágio fica
escrito aqui:

| estágio | medido hoje |
|---|---|
| a — modo A pelas ops de um assistente | alcançou em 0,0 s; iguais=True; 50 eventos em A; chave `id` |
| b — agendamento `cada_minutos: 1` | não apareceu em 35 s, apareceu em **56,5 s** |
| c — bidirecional ida e volta | alfa→beta **0,4 s**; beta→alfa **1,0 s**; eventos param em 2 e 2 |
| c2 — o que a supressão de origem poupa | beta leva 1 onde uma réplica sem `para` leva 2 — **50%** |
| d — conflito nos dois sentidos | k1: beta vence nos dois em **0,8 s**; k2: alfa vence nos dois em **0,8 s** |
| e — tabela sem chave única | recusada, com o motivo legível |
| f — spare | `varrer`/`inserir` 4004; após `spare_promover`, papel `source` e os dois passam |
| g — read replica | leitura ok; escrita **REDIRECIONA 4003 → 127.0.0.1:5338** |
| h — comportamento velho | replica como antes; recusa continua `ACESSO_NEGADO` |

### 1.3 A trava de dados — `trava.py` (`trava.json`)

Rodado às **02:33:58–02:35:23 UTC**, carga 0,34, 1,4 min.

| estágio | hoje (17/09) | antes (05/09) | veredito |
|---|---|---|---|
| congelamento: pior `ping` / pior `varrer` na réplica | **8 ms / 24 ms** (188.203 amostras) | 4 ms / 5 ms (118.793) | **ok** (teto 2.000 ms) |
| congelamento: posse da trava (telemetria de dentro) | **12,1 s** | 11,6 s | ok |
| alcance: 200.000 eventos | **2,39 s — 83.567 eventos/s** | 4,54 s — 44.062 | **1,90×** |
| alcance: pior `varrer` durante a aplicação | 128 ms | 147 ms | melhor |
| queda: soma de verificação dos dois lados | `1aa1e8124df2cba0` (200.000/200.000) nos dois | a **mesma** soma | **PASSA** |
| queda: cortes de conexão | **4** | 10 | ver §3.3 — o número é consequência, não alvo |
| abraço: escrita nos dois lados | **1,5 s** (sozinho 2,0 s = **0,73×**) | 2,5 s (3,9 s = 0,64×) | ok |
| abraço: `EAGAIN` novos | **0 e 0** | 0 e 0 | **PASSA** |

### 1.4 Credencial recusada não bloqueia o IP — `credencial-recusada.py --tela`

Rodado às **02:36:29–02:37:09 UTC**, carga 0,74. **8 de 8 casos — PASSA**,
incluindo o caso 7 pelo **botão «Religar» num navegador de verdade**
(`testes-web/religar-na-tela.mjs`, Playwright pelo caminho absoluto).

| medida | hoje | antes (09/09) |
|---|---|---|
| binário usado | **`target/release/phxsqld`** | `target/debug/phxsqld` |
| tentativas da réplica com hash errado | **1** | 1 |
| bloqueou o IP | **não** | não |
| operador do mesmo IP derrubado | **não** | não |
| controle: 5 logins errados **ainda** bloqueiam | sim, `blacklist` com `tentativas: 5` | sim |
| controle: origem fora do ar, recuo | **[1,0; 2,0; 4,0; 8,0] s**, 5 conexões | idêntico |

### 1.5 Cluster — `provar.py` (`resultados.json`)

Rodado às **02:37:19–02:38:10 UTC**, carga 0,46. **26 de 26 conferências ok,
`falhas: []`.**

| medida | hoje | antes (arquivo de 11/09) |
|---|---:|---:|
| promoção do no2 | **4,3 s** | 4,3 s |
| escrita aceita no novo master | **4,3 s** | 4,3 s |
| e-mails de promoção / degradação | **1 / 6** | 1 / 6 |
| o nó isolado NÃO se promoveu | **true**, época intacta | true |
| linhas no fim | 3.801, retrato `fe057d23b9a39478` nos três | 3.801 |

Detalhe que vale registrar: o arquivo saiu **byte a byte igual** ao commitado —
`git status` não o vê como modificado. A corrida foi real (o log da bancada está
acima), e a reprodutibilidade é boa notícia; o efeito colateral não é, e está
em §3.2.

### 1.6 A fresta — `fresta.py` (**também NÃO grava arquivo**)

Rodado às **02:38:25–02:39:32 UTC**, carga 0,28. **10 de 10, `falhas: []`**, nas
duas ordens de morte:

| ordem | papel do isolado | época | escrita liberada | decisão registrada pelo nó |
|---|---|---|---|---|
| master primeiro | **master** | 0→1 | **false** | «PROMOVIDO a master na época 1 — eleito entre 2 vivos de 3 configurados» |
| par primeiro | **replica** | 0→0 | **false** | «master calado há 4s e sem maioria visível (1 de 3): NÃO promovo» |

Nos dois casos os três convergem para **um** master, **uma** época e o retrato
`10bbe14a6497ed77` idêntico.

### 1.7 Escalonar um cluster vivo — `escalonar.py` (`resultados-escalonar.json`)

Rodado às **02:39:58–02:40:13 UTC**, carga 0,19. **Todas as conferências ok,
`falhas: []`.**

| medida | hoje | antes (07/09) |
|---|---:|---:|
| a quente **sem avisar**: pulso do nó novo | recusado, `ACESSO_NEGADO` | idem |
| reinício do no2 / no3 | **0,342 s / 0,354 s** | 0,367 / 0,368 |
| master indisponível no caminho do reinício | **0,001 s** | 0,001 |
| escalonamento total pelo reinício | **1,056 s** | 1,11 |
| `cluster_no_acrescentar` (a quente) | **0,008 s**, propagado aos 4 | 0,005 |
| no5 vivo em todos, sem ninguém reiniciar | **0,211 s** | 0,207 |
| escritas recusadas durante o escalonamento | **0 de 43**, maior buraco **11,7 ms** | 0 de 42, 11,5 ms |
| retratos no fim | `be1326ecc077b07f` nos **cinco** | `15a0c89c86780c04` nos quatro |

### 1.8 Quórum — `medir.py` (`resultados.json`)

Rodado às **02:40:27–02:40:32 UTC**, carga 0,35. 60 voltas, três servidores.

| medida | hoje (mediana, faixa) | antes (07/09 16:37) | veredito |
|---|---|---|---|
| gravar no master | **0,152 ms** [0,122; 4,749] | 0,206 [0,153; 4,204] | faixas se cruzam — **sem vencedor** |
| levar para uma réplica | **0,349 ms** [0,262; 4,235] | 0,470 [0,346; 3,482] | faixas se cruzam |
| commit hoje | **0,152 ms** | 0,206 | — |
| commit com 2-de-3 | **0,462 ms — 3,04×** | 0,634 — 3,08× | **a razão se manteve** |
| commit com 3-de-3 | **0,541 ms — 3,56×** | 0,733 — 3,56× | **idêntica** |

*A regra do pedido 155 aplicada: as medianas caíram, mas as faixas se cruzam —
não se declara vencedor dentro do ruído. O que se sustenta é a **razão**, que
saiu igual em duas corridas de dez dias de distância.*

### 1.9 O canal do pulso — `canal.py` (`resultados-canal.json`)

Rodado às **02:40:45–02:40:49 UTC**, carga 0,28. 60 voltas.

| medida | hoje | antes (07/09 17:54) | veredito |
|---|---|---|---|
| canais abertos | **6** (dois por nó) | 6 | igual |
| pulso quente | **0,146 ms** [0,078; 0,388] | 0,089 [0,069; 0,232] | faixas se cruzam — **sem vencedor** |
| empurrar um evento | **0,579 ms** [0,326; 3,646] | 0,466 [0,337; 34,988] | faixas se cruzam |
| quórum 2-de-3 / 3-de-3 pelo canal | **0,528 / 0,675 ms** | 0,447 / 0,498 | idem |
| bidirecional tem canal | **false** | false | igual |

### 1.10 Os quatro modos em contêiner — `docker/provar.py`

**O daemon subiu.** `sudo -n dockerd` às 02:41:03; API no ar em 8 s. Bancada
rodada às **02:42:44–02:50:39 UTC**, **7,9 min**, **16 de 16 estágios [ok]**,
4 reconexões. Ela removeu contêineres, redes e volumes sozinha; a imagem e o
`PHX_BASE` foram removidos por mim, e o `dockerd` que eu subi foi **parado**
(`/var/run/docker.sock` não existe mais) — a máquina voltou ao estado em que a
encontrei.

| medida | hoje (17/09) | antes (05/09) | veredito |
|---|---|---|---|
| imagem: camada / comprimido / `docker images` | **10,01 / 4,03 MB / 14,1MB** | 8,22 / 3,3 / 11.5MB | cresceu ~22% |
| (0) `bind: 127.0.0.1` dentro do contêiner | vizinho **não** alcança, 0 evento em 20 s; com `0.0.0.0` alcança em **0,51 s** | 0 evento; 0,41 s | **ok** |
| (a) source em contêiner | **16.030 linhas/s**; réplica alcança **1,92 s** (52.140 ev/s) | 13.462; 2,94 s (34.033) | ok |
| (a) retrato SHA-256 | `39787c620feeed8f` nos dois, 101.013 linhas | iguais | **PASSA** |
| (a2) `docker kill` na réplica | volta em **459 ms**, alcança 4.000 em **0,46 s** | 538 ms; 0,54 s | ok |
| (a3) corte silencioso | pior `ping` **7 ms**, pior `varrer` **40 ms** (60.710 amostras) | 9 / 6 ms | **ok** (teto 2.000 ms) |
| (a-processos) o mesmo em processos | **16.060 linhas/s**; 1,82 s (54.972 ev/s) | 13.459; 2,91 s | ver §5.2 |
| (b-partição) corte sem matar ninguém | 1,3 s fora da rede; convergiram em **0,81 s**; rowids diferentes entre servidores; conteúdo por chave **IGUAL** | idem | **ok** |
| (b-cortes) retomada por tipo de corte | REJECT 0,0/0,0/0,2 s; DROP 0,2/0,2/**24,8 s** | 0,0/0,0/0,3; 1,0/0,2/**25,1** | ok, e o DROP-45s continua sendo o caro |
| (b-abraço) 100.000 nos dois lados | **2,9 s**, convergiram **5,4 s** depois, `EAGAIN` **0 e 0** | 3,6 s; 8,1 s; 0 e 0 | ok |
| (c) spare | primário morto; promoção em **7 ms** | 6 ms | ok |
| (d) read replica | alcance **0,31 s**; `REDIRECIONA` com o nome de serviço | 0,16 s | ok |
| (e) firewall da §7 | intruso leva **200 → 0 → 0**; com `iptables` nem abre a porta; 33 linhas dele no `acessos.log`; acabou na lista negra; o source **não alcança ninguém** | idêntico | **ok** |

## 2. O que NÃO rodou, e o comando de cada um

Nenhuma das dez bancadas pedidas ficou de fora — inclusive a de contêiner. O que
ficou por medir são **blocos dentro de um arquivo** e **itens do parecer do
DBA**:

| o que | por que não rodou | como se roda |
|---|---|---|
| `custo_da_imagem` do `replicacao/resultados.json` | **não tem gerador**: sai de duas corridas com o interruptor `imagem_da_linha` mudando, montadas à mão, e o `medir.py` só **preserva** o bloco por nome. Os números de lá são de 29/08 e carregam o `quando` de hoje | editar `config_master` do `montar.py`, rodar `medir.py` com e sem a imagem, e anotar — é exatamente o que precisa virar gerador (§3.1) |
| `cascata` do mesmo arquivo | mesmo motivo. E rodar `montar.py --cascata && medir.py` **sobrescreveria** os números principais de hoje com os de uma topologia diferente, sem atualizar o bloco `cascata` | idem |
| itens 2, 3, 7 e 8 do §3 do parecer do DBA (coluna `Bin`; coluna cifrada só num lado; comparar o `.log` dos dois; comparar a lixeira dos dois) | não couberam nesta corrida; os três «de uma linha» (1, 5 e 6) foram os escolhidos e estão em §4 | `bancada/replicacao/achados-do-dba.py` é o lugar de cada um — cada estágio já nasce com controle |
| medir o commit anterior a `0c639a7` num *worktree* próprio | continua sem caber no disco (1,6–2,3 GiB livres a noite toda; o `target` do repositório sozinho já tem **10 GB**) | ver §5.1 — a pergunta que ele responderia mudou de dono |

## 3. Defeitos e fragilidades achados, cada um com a prova

### 3.1 Dois blocos sem gerador viajam com a data de hoje

**Medido.** O `bancada/replicacao/resultados.json` gravado hoje diz
`"quando": "2026-09-17"` e carrega, dentro, `custo_da_imagem` (medido em 29/08)
e `cascata` (idem). O `medir.py` os **preserva por nome** de propósito — e isso
está certo, foi conserto de um defeito anterior. O que não existe é um
**produtor** para nenhum dos dois: nenhuma linha do `medir.py` os calcula.

O dano hoje é contido — as páginas (`pagina-dos-testes.py`,
`numeros-da-bancada.py`, `graficos-dos-testes.py`, `pagina-de-status.py`) leem
só `master_linhas_s`, `replica_eventos_s`, `alcance_s`, `atraso_ms`,
`retomada_*` e `iguais_no_fim`, todos frescos. O risco é o dia em que alguém
publicar o `custo_da_imagem`: sairá com a data errada, e com autoridade.

**Recomendação (decisão de A/H):** ou o `medir.py` ganha o par de corridas que
produz o `custo_da_imagem`, ou cada bloco preservado ganha **data própria**
dentro dele.

### 3.2 Quatro arquivos de resultado não trazem a data em que foram medidos

**Medido, arquivo por arquivo, depois desta corrida:**

| arquivo | campo de data |
|---|---|
| `bancada/replicacao/resultados.json` | `quando` ✔ |
| `bancada/replicacao/credencial-recusada.json` | `medido_em` ✔ |
| `bancada/quorum/resultados.json` | `medido_em` ✔ |
| `bancada/quorum/resultados-canal.json` | `medido_em` ✔ |
| `bancada/replicacao/trava.json` | **nenhum** |
| `bancada/replicacao/docker/resultados.json` | **nenhum** |
| `bancada/cluster/resultados.json` | **nenhum** |
| `bancada/cluster/resultados-escalonar.json` | **nenhum** |

O `pagina-dos-testes.py` é honesto sobre isso: `quando_de()` cai no `mtime` e
**marca** que caiu. A fragilidade tem nome e hoje ela apareceu: o
`cluster/resultados.json` saiu **byte a byte igual** ao commitado — `git status`
não o vê modificado —, e mesmo assim o `mtime` andou. O caminho contrário é o
perigoso: um `git checkout` move o `mtime` sem medir nada, e a página passa a
anunciar como de hoje um número de outro dia.

Não consertei aqui por escolha: a prova do conserto exige rodar o gerador da
página, que é artefato de outra frente e estava sendo mexido esta noite.
**Fica nomeado para G/H**, e o conserto é de uma linha por script.

### 3.3 O estágio `queda` do `trava.py` enfraquece sozinho quando o motor melhora

**Medido.** O laço corta a conexão a cada 0,3 s *enquanto a réplica não
alcançou*, com teto de 12 s (`trava.py`, `estagio_queda`), e o veredito só exige
`cortes > 0`:

| corrida | alcance do estágio | cortes de conexão |
|---|---:|---:|
| 05/09 | 4,54 s | **10** |
| 17/09 | 2,39 s | **4** |

A quantidade de cortes não é alvo: é **subproduto da lentidão do que se mede**.
O `LEIA-ME.md` da bancada descreve o estágio como «dez cortes de conexão de
verdade» — número que hoje está errado e que ninguém digitou de novo desde que
virou 4. E o fim da escada é pior que um número errado: se o alcance cair abaixo
de ~0,3 s, `cortes` chega a 0 e **a bancada reprova sem haver defeito nenhum** —
a mesma família da guarda que só passava com a trava presa
(`cognicao_limitacao_que_envelheceu_20260905_0430.md`).

Não mexi: mudar o gatilho do corte muda o desenho da medição, e isso é decisão
de A. As duas saídas óbvias — cortar por **progresso aplicado** em vez de por
relógio, ou exigir um número mínimo de cortes e alongar a carga até consegui-lo
— têm custo diferente e nenhuma é neutra.

### 3.4 Duas bancadas do trio não gravam arquivo nenhum — e a lista dizia uma

O pedido 193 registrou «e o `modos.py` continua sem gravar arquivo de
resultado». **Medido: são duas.** `bancada/cluster/fresta.py` também só imprime
`RESULTADO` (`fresta.py:168`; nenhum `open(...,"w")` no arquivo). Lista que
enumera menos casos do que existem não protege menos hoje — protege menos no dia
em que alguém a usar como inventário.

E há a consequência na página que existe para dizer em que se pode confiar: o
`pagina-dos-testes.py` só mostra as bancadas do **plano dele** (18 entradas). Das
medidas de replicação/cluster desta noite, **quatro ficam invisíveis** por não
estarem no plano — `trava.json`, `resultados-escalonar.json`,
`resultados-canal.json` e `docker/resultados.json` — e **duas** por não existirem
(`modos.py`, `fresta.py`). «Bancada sem arquivo aparece como NÃO MEDIDA» só vale
para quem está na lista; quem não está some calado, que é o pior dos dois modos
de falhar.

## 4. Os três achados do DBA, provados pelo soquete — e o quarto que a prova gerou

O papel C achou três defeitos **lendo o código**. Ler código não prova defeito de
replicação: o que depende de dois processos e de um soquete se prova contra dois
processos e um soquete. Escrevi `bancada/replicacao/achados-do-dba.py`, em que
**cada estágio roda o cenário E o controle** — o mesmo roteiro com a única linha
do defeito retirada. Rodado às **02:58:51–03:00:08 UTC**, 1,3 min, carga 0,18.
Resultado em `bancada/replicacao/achados-do-dba.json`.

### 4.1 `rownum` divergente — **CONFIRMADO, e o gatilho é mais estreito do que o parecer diz**

O parecer aponta `numerar_linha` (`table.rs:2361-2377` → `reg.rs:764-768`)
consumindo o contador **antes** da conferência de unicidade. A leitura do código
está certa. A prova pelo soquete mostrou **onde ela morde e onde não**:

| a recusa aconteceu… | ela foi recusada? | o source queimou um `rownum`? |
|---|---|---|
| chave duplicada na **primária**, em operação própria | sim — `[SP000020] índice único porId já tem essa chave` | **não** |
| chave duplicada num **único secundário**, em operação própria | sim — `porEmail` | **não** |
| **coluna obrigatória** faltando, em operação própria | sim — `[SP000018] coluna nome é obrigatória e recebeu NULL` | **não** |
| chave duplicada **dentro de um `inserir_lote` com `parar_no_erro: false`** | a linha, sim (`gravadas=5`, `recusadas=1`) | **SIM** |

O caso que morde, medido:

```
  rowid |  id | rownum source | rownum replica
      1 |   1 |             1 |              1
      2 |   2 |             2 |              2
      3 |   3 |             3 |              3
      4 |   4 |             5 |              4  <<< DIVERGIU
      5 |   5 |             6 |              5  <<< DIVERGIU
retrato SHA-256   source 252fa89db5038769   replica d92da11a064d6f11   (DIFERENTE)
controle (as mesmas 5 linhas, sem recusa)    d92da11a064d6f11 nos dois  (IGUAL)
```

**Por que as três primeiras não divergem, e a quarta sim:** `proximo_rownum` é
estado **da instância aberta** do `Reg`. Entre duas operações o servidor reabre a
tabela e o contador nasce de novo do disco — a queima se perde. Dentro de um
lote, a instância é a mesma do começo ao fim, e a queima sobrevive até a linha
seguinte.

Isso **não** diminui o achado; muda o alcance dele, e para pior no lugar que
importa: `inserir_lote` com `parar_no_erro: false` é exatamente o caminho de
**importação/carga**, que é o lugar onde uma linha recusada é rotina e não
acidente.

**E é uma linha na carga da bancada**: o retrato SHA-256 já inclui o `rownum` —
ele só nunca acusou porque a semeadura do `medir.py` jamais falha uma inserção.

### 4.2 Único secundário para o par bidirecional — **CONFIRMADO**

Par bidirecional, tabela com primária `porId` **e** único secundário `porEmail`.

| | controle (e-mails distintos) | cenário (mesmo e-mail) |
|---|---|---|
| ids em beta no fim | **[1, 2, 3]** | **[2]** |
| a linha do conflito chegou? | sim | **não** |
| a escrita **seguinte**, que não conflita com nada, chegou? | **sim** | **não** |
| `ultimo_erro` da origem | `null` | `[SP000020] chave duplicada: índice único porEmail já tem essa chave` |

A segunda linha é a que separa «uma linha perdida» de «o par de servidores
parado», e foi ela que julgou: com o conflito de pé, **nada mais atravessa**, e
o mesmo lote volta para sempre. É a doença que o `INTEGRIDADE.md` §3.1 nomeou e
curou para a chave estrangeira; para o único secundário ela continua inteira.

### 4.3 Tabela apagada e recriada no source — **CONFIRMADO, e é o silêncio que assusta**

| | controle (sem apagar) | cenário (`excluir_tabela` + `criar_tabela`) |
|---|---|---|
| source no fim | — | ids **[91, 92, 93]**, **3** eventos |
| réplica no fim | ids [1..8], as três novas chegaram | ids **[1, 2, 3, 4, 5]**, **5** eventos |
| as linhas novas chegaram? | **sim** | **não** |
| `ultimo_erro` / `parada` | `null` / `null` | **`null` / `null`** |

A réplica fica com a tabela velha **para sempre**, e `replicacao_estado` não tem
uma palavra sobre isso: `aplicados: 5`, `ultimo_erro: null`, `parada: null`. O
PITR pega este caso (`diario_vivo_continua`); a réplica não. É o padrão desta
casa em estado puro — *o conserto entrou no caminho que o motivou e o caminho
irmão ficou*.

## 5. Hipóteses que morreram medidas

### 5.1 «A queda do `master_linhas_s` foi o `fsync` que a onda 2 pôs no caminho de escrita»

O pedido 193 registrou a queda **34.048 (29/08) → 26.762** e nomeou o candidato,
dizendo com todas as letras que **não estava medido**. Medi a **premissa** dele,
que é o que vem antes do item:

```
strace -f -c -e trace=fsync,fdatasync,sync_file_range  numa carga de 100.000 linhas
  104 chamadas  {'fsync': 104}  =  0,00104 fsync por linha
```

**A premissa é falsa: não há `fsync` por escrita.** São 104 chamadas para 100.000
linhas — ordem de grandeza de *fecho de janela*, não de linha. Bate com o que o
código diz de si mesmo: a catraca que a onda 2 criou chama-se
`TETO_FSYNC_POR_FECHO_V2` e mede **`fsync` por fecho de janela**; o comentário do
group commit (`servidor.rs:14334-14339`) registra que a primeira versão sim
chamava `sincronizar()` por tabela por commit, **e que ela foi trocada por
isso**.

E o número de hoje, com esse `fsync` **de pé** e a catraca V2 cobrando: **45.117
linhas/s** — 1,33× acima dos 33.883 de 07/09 e 1,69× acima dos 26.762 que o 193
registrou.

**O que eu NÃO medi, e por isso não afirmo:** *o que* causou a queda de 05/09.
Isso continua exigindo o commit anterior num *worktree* com `target` próprio, e
continua sem caber no disco. O que morreu medido foi o **candidato nomeado**, e
com ele a pergunta: o número não está preso em 26.762, então a investigação
deixou de ser urgente e virou curiosidade histórica.

### 5.2 «O contêiner é ~2,8× mais lento que o processo»

Quem puser lado a lado o `a-processos` da bancada de contêiner (**16.060
linhas/s**) e o `medir.py` (**45.117 linhas/s**) conclui isso. **Está errado, e a
causa é a libc.** A bancada de contêiner roda o estágio de processos com o
binário **musl** de propósito — para que contêiner e processo comparem trabalho
igual, o que é a regra 4 e está certo. Só que isso torna o número dela
**incomparável** com o do `medir.py`, que roda o `gnu`.

Medido com a **mesma carga, mesmo esquema, mesmo cliente, um servidor cada**
(`bancada/replicacao/custo-do-binario.py`, 03:01 UTC, portão livre, carga 0,14):

| binário | 100.000 linhas | linhas/s |
|---|---:|---:|
| `target/release/phxsqld` (**gnu**) | 2,061 s | **48.510** |
| `target/x86_64-unknown-linux-musl/release/phxsqld` (**musl**) | 4,770 s | **20.965** |
| | | **gnu / musl = 2,31×** |

Os 2,31× do binário explicam quase toda a diferença; o resto é o daemon do
Docker no ar e uma réplica contra três. **O contêiner não é lento: o `musl` é.**
E isso é informação de produto, não de bancada — a imagem `FROM scratch` que a
casa publica **é** a musl.

*Nada aqui recomenda trocar de libc:* o `musl` é o que faz a imagem ter 10 MB e
não precisar de libc solta, e a medição não diz qual metade da troca vale mais.
Diz só que o número tem dono, e que o dono não é o contêiner.

### 5.3 «Basta uma inserção recusada para o `rownum` divergir»

Morreu na forma em que foi escrita, e a morte gerou a medição que ficou: três
recusas em operação própria **não** divergem; a recusa **dentro de um lote**
diverge. Está em §4.1.

## 6. Consertos de bancada, com a prova nos dois sentidos

Todos no roteiro **novo** (`achados-do-dba.py`), nenhum em bancada antiga. Os
três foram achados pela própria medição, e os três são da mesma família: *prova
que não confere o próprio estrago mede outra coisa*.

| # | o defeito | como apareceu | prova nos dois sentidos |
|---|---|---|---|
| 1 | `excluir_tabela` **sem** `confirmar` é recusado, e o estágio não conferia a resposta | o cenário e o controle deram o **mesmo** resultado: réplica com `[1,2,3,4,5,91,92,93]` nos dois — a tabela nunca foi apagada | **com o defeito:** `chegou=True` e veredito `[FALHA]` («o defeito não existe»). **Com o conserto** (`"confirmar": "clientes"` + parada explícita se a montagem falhar): source `[91,92,93]` 3 eventos, réplica `[1,2,3,4,5]` 5 eventos, `chegou=False`, veredito `[ok]` |
| 2 | `eventos()` pedia `posicao` com `"tabela"` no pedido e o campo mora em `resultado.tabelas.<tabela>.eventos` | devolvia `None` calado, e todo `esperar(... == 5)` esgotava o prazo em vez de esperar | **com o defeito:** `eventos_source: null`, `eventos_replica: null` no JSON de 02:54. **Com o conserto:** `eventos_source: 3`, `eventos_replica: 5` — e é esse par que sustenta o achado 4.3 |
| 3 | `--so <estágio>` **sobrescrevia** o `achados-do-dba.json` inteiro | a corrida `--so rownum` deixou um arquivo com um estágio só, parecendo a bateria completa | **com o defeito:** o arquivo ficou com `{"rownum": …}` e sumiram `unico` e `recriada`. **Com o conserto** (mescla por nome + campo `preservados_de_corrida_anterior`): a corrida parcial preserva os outros dois **e diz quais preservou** |

## 7. Arquivos que esta frente tocou

**Escritos por bancada (não editados à mão):**

- `bancada/replicacao/resultados.json`
- `bancada/replicacao/trava.json`
- `bancada/replicacao/credencial-recusada.json`
- `bancada/replicacao/docker/resultados.json`
- `bancada/cluster/resultados.json` *(conteúdo idêntico ao anterior)*
- `bancada/cluster/resultados-escalonar.json`
- `bancada/quorum/resultados.json`
- `bancada/quorum/resultados-canal.json`

**Criados por esta frente:**

- `bancada/replicacao/achados-do-dba.py` e `achados-do-dba.json`
- `bancada/replicacao/custo-do-binario.py` e `custo-do-binario.json`
- `docs/propostas/bateria-replicacao-2026-09-17.md` (esta página)
- `docs/cognicao/cognicao_guarda-que-enfraquece-quando-o-motor-melhora_20260917_0235.md`
- `docs/cognicao/cognicao_recusa-que-so-queima-o-contador-dentro-do-lote_20260917_0258.md`

**Não tocados:** nenhum arquivo de motor, nenhum gerador de página, nenhum
`.json` editado à mão, e nada no índice do git.

## 8. Como se refaz tudo isto

```bash
flock /tmp/phx-cargo.lock cargo build --release
flock /tmp/phx-cargo.lock cargo build --release --target x86_64-unknown-linux-musl --bin phxsqld

bash bancada/esta-medindo.sh                      # antes de CADA bancada de tempo
python3 bancada/replicacao/montar.py /tmp/phx-replicacao
PHX_REPLICACAO=/tmp/phx-replicacao python3 bancada/replicacao/medir.py 100000
python3 bancada/replicacao/montar.py --derrubar
python3 bancada/replicacao/modos.py /tmp/phx-modos
python3 bancada/replicacao/trava.py
python3 bancada/replicacao/credencial-recusada.py --tela
python3 bancada/cluster/provar.py /tmp/phx-cluster
python3 bancada/cluster/fresta.py /tmp/phx-fresta
python3 bancada/cluster/escalonar.py /tmp/phx-escalonar
python3 bancada/quorum/medir.py
python3 bancada/quorum/canal.py

sudo -n dockerd &                                  # o daemon nao sobe sozinho aqui
python3 bancada/replicacao/docker/provar.py        # ~8 min, remove tudo no fim

python3 bancada/replicacao/achados-do-dba.py       # os tres achados do DBA
python3 bancada/replicacao/custo-do-binario.py     # musl x gnu, e a conta de fsync
```

Cada diretório de bancada sai por `rm -rf` **depois** de provar, por
`/proc/<pid>/cwd`, que nenhum processo vivo o usa — nunca por nome nem por data.

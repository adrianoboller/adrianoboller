# Replicação no PhxSql

**Pergunta:** dá para ter no PhxSql a replicação Source → Replica do MySQL(R)?

**Resposta:** dá, e desde a 0.15.0 **está funcionando**. O que faltava era uma
coisa só — a imagem da linha no `.log` — e ela entrou.

Quatro servidores no ar, com a medição em `bancada/replicacao/`:

```
Master 5800 ──┬──► Slave01 5801
              ├──► Slave02 5802
              └──► Slave03 5803
```

| | |
|---|---|
| Master, com a imagem no diário | 34.048 linhas/s |
| Aplicação, por réplica (as três em paralelo) | 17.450 eventos/s |
| Atraso de uma escrita até as três | 140 ms a 2,0 s |
| Réplica derrubada: voltar a atender e alcançar 4.000 eventos | 323 ms + 0,3 s |
| Retrato SHA-256 das quatro tabelas, no fim | idênticos |

Desde então entraram os **quatro modos** — Primary→Replica, Multi-Master,
Spare/Failover e Read Replica (§9 a §12) —, o **agendamento** por janela (§11)
e a promoção manual `spare_promover` (§10), com a bancada dos oito estágios
na §14.

O que ainda **não** existe está na §13 e na §15.

---

## 1. O paralelo peça a peça

| MySQL(R) | PhxSql | Situação |
|---|---|---|
| Binary log (`mysql-bin.000001`) | `Tabela.log`, já paginado em `_001`, `_002` | **existe** |
| Relay log | mesmo `.log`, do lado da réplica | existe (é o mesmo formato) |
| Posição (`MASTER_LOG_POS`) | ordinal do evento no `.log` | **existe** — o evento N *é* a posição N |
| GTID / `SOURCE_AUTO_POSITION=1` | par `(tabela, sequência)` | **existe**, sem campo novo |
| Porta 3306 | porta **5000** | existe |
| Réplica inicia a conexão | idem | é como o servidor já funciona |
| Usuário exclusivo de replicação | token + `replicas_autorizadas` | existe no `config.json` |
| Row-based binlog (imagem da linha) | `.log` v2, atrás de `imagem_da_linha` | **existe** |

A direção da conexão é a mesma do MySQL(R), e é o ponto que você destacou:

```
   REPLICA 192.168.50.20  ──── TCP 5000 ────►  SOURCE 10.1.1.102
        (quem procura)                            (quem responde)
```

O Source **não empurra** nada. Ele mantém o diário e responde a quem pergunta
"tem evento novo depois do número N?". Isso já é o modelo do servidor atual.

---

## 2. O que já dá para fazer hoje, sem mudar nada

A réplica consegue **descobrir** tudo o que aconteceu:

```json
{"token":"...","op":"diario","database":"Z","tabela":"cadastroClientes"}
```

devolve, em ordem cronológica, cada inclusão, alteração e exclusão com data,
hora, rowid e versão. Isso basta para auditoria e para monitorar divergência.

O que **não** basta é para aplicar: o evento diz *que* o rowid 42 foi
alterado, mas não diz *para quê*.

---

## 3. A peça que faltava: a imagem da linha

Até a 0.14.0 o `.log` guardava 36 bytes por evento — carimbo, operação, rowid,
versão, usuário e CRC. Faltava o conteúdo: o evento dizia *que* o rowid 42
mudou, não dizia *para quê*.

### O formato (versão 2 do `.log`)

Cabeçalho do evento passou de 36 para **44 bytes**, e ganhou um corpo:

| Off | Tam | Campo |
|----:|----:|---|
| 0 | 8 | carimbo — ms desde a época |
| 8 | 1 | operação: 1 inclusão, 2 alteração, 3 exclusão |
| 9 | 1 | flags — bit 0: tem imagem |
| 10 | 2 | reservado |
| 12 | 8 | rowid |
| 20 | 8 | versão do registro depois da operação |
| 28 | 4 | usuário |
| 32 | 4 | **tamanho da imagem** |
| 36 | 4 | CRC-32 do cabeçalho **e da imagem** |
| 40 | 4 | reservado |
| 44 | N | **imagem da linha** |

O CRC cobrir a imagem, e não só o cabeçalho, é o detalhe que importa: a imagem
é o que a réplica grava **como dado**. Um byte trocado ali entraria na réplica
sem ninguém notar.

E há um preço que o formato cobra: até a versão 1 o evento N morava no offset
`64 + N × 36`, e pular era uma conta. Agora não é — chegar ao evento N é
caminhar pelos anteriores lendo o tamanho de cada um. O que salva a leitura é o
`qtd_eventos` no cabeçalho de cada volume: um volume inteiro se pula sem abrir.

A imagem não é o texto do registro — é o **payload cru do `.reg`**, os mesmos
bytes que a réplica precisa gravar. Sem reencodar, sem perder precisão.

### O detalhe que quase passa batido: os blobs

O payload do `.reg` guarda **ponteiros** para `.bin` e `.memo`, e esses
ponteiros são offsets locais. Os offsets do Source não valem na Réplica.

Então a imagem carrega também o conteúdo externo:

```
imagem = [tam_payload u32][payload]
         [qtd_externos u16]
         [ (coluna u16, tamanho u32, conteúdo) ... ]
```

Ao aplicar, a réplica grava os blobs no **seu** `.bin`/`.memo`, recebe os
ponteiros locais, remenda o payload e só então grava o registro. A linha sai
idêntica, com ponteiros válidos naquela máquina.

Operações que **não** precisam de imagem: a exclusão. O rowid basta.

### Custo

Medido, mesma tabela e mesmas 100.000 linhas, só o interruptor mudando:

| `imagem_da_linha` | linhas/s | bytes por evento | `.log` |
|---|---:|---:|---:|
| desligada | 21.740 | 44 | 4,4 MB |
| ligada | 19.531 | 223 | 22,3 MB |

**10% mais devagar, e um diário 5,1× maior.** Por isso o `.log` já nasceu
paginado, e por isso a imagem fica atrás de um interruptor no `config.json`:

```json
"replicacao": { "imagem_da_linha": true }
```

Quem só quer auditoria deixa desligado e continua com 44 bytes por evento.
Quem quer replicar liga — e num servidor com `papel: source` ela **já vem
ligada**, porque um source sem imagem no diário é um source que não replica, e
descobrir isso pela réplica parada seria o pior jeito de descobrir. O arranque
avisa em voz alta se alguém desligar.

---

## 4. Posição: por que o PhxSql não precisa inventar um GTID

No MySQL(R) antigo você controlava à mão:

```
MASTER_LOG_FILE='mysql-bin.000187'
MASTER_LOG_POS=9837443
```

e o GTID veio para acabar com isso.

No PhxSql o problema já não existe: o `.log` é uma sequência de eventos de
tamanho conhecido, então **o evento N é a posição N**. A réplica guarda um
número por tabela e pede o que falta:

```json
{"op":"replicar","database":"Z","tabela":"cadastroClientes","desde":1234}
```

Equivale ao `SOURCE_AUTO_POSITION=1`, sem campo novo no formato.

**Por que por tabela e não por servidor?** Porque o PhxSql ainda não tem
transações entre tabelas. Sem transação, não existe ordem global que precise
ser preservada — e uma sequência por tabela deixa as tabelas replicarem em
paralelo. Quando as transações entrarem, entra junto um número de sequência do
database inteiro; o campo reservado do cabeçalho do evento já está guardado
para isso.

---

## 5. O rowid é o que faz a réplica ser fiel

Esta é a parte mais bonita do desenho, e vem de graça de uma decisão que já
está no formato.

O `.reg` **nunca reaproveita slot** e o rowid é sempre `slot_count + 1`. Então,
se a réplica aplicar **todos** os eventos, **na ordem**, e **mais ninguém**
escrever nela, os rowids saem exatamente iguais aos do Source — sem precisar
transmitir nem negociar nada.

Isso dá uma verificação forte e barata: ao aplicar uma inclusão, a réplica
confere se o rowid que ela gerou bate com o do evento. Se não bater, ela
divergiu, e a replicação **para na hora** em vez de propagar a divergência —
o mesmo comportamento do SQL thread do MySQL(R) parando num erro.

É também o motivo de o `Config_exemplo_03.json` vir com:

```json
"somente_leitura": true
```

Uma réplica escrita pela aplicação quebra a numeração e perde a sincronia.

**A exceção é o modo bidirecional**, e ela é do desenho: ali os dois lados
escrevem, os rowids divergem por construção, e a identidade entre servidores
passa a ser a **chave única** (§12). Cada `.reg` continua com a sua ordem de
digitação — que é o ponto: a ordem é sagrada *em cada servidor*, não entre
eles.

---

## 6. Protocolo

Três operações, no mesmo JSON Lines da porta 5000:

```json
{"token":"...","op":"posicao","database":"Z","com_esquema":true}
{"ok":true,"resultado":{
   "papel":"source","imagem_da_linha":true,
   "tabelas":{"cadastroClientes":{"eventos":1234,"registros":1200,
                                  "esquema":"50534348..."}}}}

{"token":"...","op":"replicar","database":"Z","tabela":"cadastroClientes",
 "desde":1234,"max":500}
{"ok":true,"resultado":{"eventos":[...],"desde":1234,"ate":1734,
                        "total":1734,"fim":true}}

{"token":"...","op":"aplicar","database":"Z","tabela":"cadastroClientes",
 "eventos":[...]}
{"ok":true,"resultado":{"recebidos":500,"aplicados":500,"posicao":1734,
                        "erro":null}}
```

A imagem viaja em **hexadecimal**, porque o transporte é JSON e JSON não tem
bytes. Dobra o tamanho; a alternativa seria acrescentar um formato binário ao
protocolo, e isso é uma decisão maior do que esta.

O `com_esquema` traz o **bloco de esquema cru**, o mesmo que mora dentro do
`.reg`. É assim que a réplica cria uma tabela que ainda não existe nela: a
partir dos mesmos bytes, e não de uma remontagem coluna a coluna a partir de
JSON — que é onde um tipo ou uma escala se perderiam sem ninguém notar.

**Três permissões diferentes, de propósito.** `posicao` e `replicar` exigem
`replicar`, que é uma permissão própria: o fluxo é o diário com a linha inteira
dentro, e dá para concedê-lo a uma réplica sem conceder mais nada. `aplicar`
exige `administrar`, porque grava com o rowid escolhido e o payload cru, por
fora das conferências normais.

**`aplicar` não está na lista de operações de escrita**, e a ausência é
deliberada: uma réplica roda em `somente_leitura` justamente para a aplicação
não escrever nela, e a única escrita que ela deve aceitar é a que vem do source.

Mais quatro campos e três operações que os modos novos trouxeram:

```json
{"token":"...","op":"replicar","database":"Z","tabela":"c","desde":0,"para":"belgica-01"}
   ... eventos que NASCERAM em belgica-01 não voltam; `ate` anda por cima deles
   ... cada evento traz agora "carimbo_ms" e "origem"

{"token":"...","op":"posicao","database":"Z"}
   ... a resposta traz "id_servidor" e, por tabela, "chave" (nula = sem
       identidade replicável, e o modo bidirecional a recusa)

{"token":"...","op":"replicacao_estado"}          → papel vivo, posição por
   origem e tabela, última rodada, último erro, recusas com o motivo
{"token":"...","op":"replicacao_testar","origem":"curitiba"}  → prova a ligação
   pela MESMA conexão e autenticação do laço, e lista os impedimentos por modo
{"token":"...","op":"spare_promover","motivo":"..."}          → a promoção manual
```

As três exigem `administrar` — `replicacao_testar` porque a resposta descreve
outro servidor (papel, id, tabelas), e `spare_promover` porque vira o papel do
servidor inteiro. **Nenhuma delas é anônima**, e nenhuma devolve credencial:
nem o token, nem a senha, nem o `senha_hash` que quem perguntou acabou de
mandar.

A réplica roda um laço: pergunta a posição, puxa em lotes de 500, aplica,
dorme `reconectar_em` segundos, repete. Uma **thread por origem**, para uma
origem lenta ou caída não segurar as outras. Erro não mata a thread — escreve e
espera; um source que caiu volta e a réplica retoma do número em que parou.

O laço mora dentro do próprio `phxsqld`: basta `papel: replica` e uma origem no
`config.json`. As operações continuam existindo para quem quiser dirigir a
replicação de fora.

### A senha não viaja

A réplica se autentica pelo mesmo desafio-resposta do resto do protocolo: pede
um nonce, calcula o HMAC com a chave derivada e manda a **prova**. No
`config.json` da réplica mora o `senha_hash` — o mesmo texto que já mora no
cadastro de usuários —, e dele sai a chave derivada. Não há senha em claro em
lugar nenhum.

```json
"origens": [
  {"nome":"curitiba","host":"10.1.1.102","porta":5000,"token":"...",
   "usuario":"replicador","senha_hash":"pbkdf2-sha256$210000$...",
   "databases":["Z"],"reconectar_em":10}
]
```

---

## 7. Firewall — o mesmo desenho que você descreveu

| Servidor | Direção | Porta | Para |
|---|---|---|---|
| Source | ENTRADA | TCP 5000 | somente o IP da Réplica |
| Source | SAÍDA | retorno TCP | Réplica |
| Réplica | SAÍDA | TCP 5000 | Source |
| Réplica | ENTRADA | conexão estabelecida | Source |

No PhxSql isso é imposto em **dois lugares**, não só no firewall:

```json
"ips_permitidos": ["192.168.50.20"],
"replicacao": { "replicas_autorizadas": ["192.168.50.20"] }
```

E toda tentativa — inclusive a recusada — cai no `acessos.log` com IP, data e
hora. Quem bateu na porta e não entrou fica registrado.

### Curitiba ↔ Bélgica

```
BRASIL                                    BÉLGICA
┌────────────────┐                    ┌────────────────┐
│ SOURCE         │                    │ REPLICA        │
│ 10.1.1.102     │                    │ 192.168.50.20  │
│ :5000          │                    │ somente_leitura│
└───────▲────────┘                    └────────┬───────┘
        │                                      │
   ┌────┴─────┐        IPSec             ┌─────▼────┐
   │ Mikrotik │◄════════════════════════►│ Mikrotik │
   └──────────┘                          └──────────┘

        A porta 5000 nunca sai do túnel.
        Internet ──X── 5000
```

---

## 8. Multi-source e failover

**Multi-source** já está no `config.json`: a réplica abre uma conexão
independente por origem.

```
Curitiba  10.1.1.102:5000 ─┐
São Paulo 10.2.1.10:5000  ─┼──► REPLICA (Bélgica)
Bruxelas  10.3.1.7:5000   ─┘
```

Cada origem tem token, lista de databases e intervalo de reconexão próprios —
ver `exemplos/Config_exemplo_03.json`.

**Failover.** Promover uma réplica a Source tem agora um degrau de operação:
a op **`spare_promover`** (seção 10) para o laço de réplica, abre a escrita e
vira o papel para `source` **no processo vivo** — o `config.json` continua
sendo do administrador, e a resposta diz o que ajustar nele para o próximo
arranque. As outras réplicas passam a apontar para o promovido; como a posição
é o ordinal do evento no `.log` e todas aplicaram a mesma sequência, elas
continuam de onde pararam.

O ponto delicado, e é honesto dizer: se o Source cair **no meio** de uma
gravação, réplicas diferentes podem ter parado em pontos diferentes. Sem
transações, a promoção é segura quando as réplicas estão na mesma posição, e
exige conferência quando não estão. Failover **automático** — eleição, quórum,
heartbeat — é outra frente; o degrau manual daqui é o que ela vai chamar.

---

## 9. Os quatro modos

O paralelo é o Centro de Controle do HFSQL(R) (Replicação → configurar), com
nomes mais explícitos e um modo a mais:

```
A) Primary → Replica            B) Multi-Master ↔ Multi-Master
   ┌───┐  eventos  ┌───┐            ┌───┐  eventos  ┌───┐
   │ A │ ────────► │ B │            │ A │ ◄───────► │ B │
   └───┘           └───┘            └───┘           └───┘
   escrita só em A                  escrita nos DOIS; conflito
   (distribuição, filial,          pelo carimbo mais recente;
   datacenter secundário)          identidade = chave única

C) Primary → Standby (spare)    D) Read Replica
   ┌───┐  eventos  ┌───┐            ┌───┐  eventos  ┌───┐
   │ A │ ────────► │ S │            │ A │ ────────► │ R │
   └───┘           └───┘            └───┘           └───┘
   S não atende cliente             R atende SÓ leitura; escrita
   NENHUM até `spare_promover`      é recusada apontando A
```

| modo | `papel` no config | escrita de cliente | leitura de cliente |
|---|---|---|---|
| A | `source` + `replica` | só no primário | nos dois |
| B | `multi` nos dois | nos dois | nos dois |
| C | `source` + `spare` | só no primário | só no primário |
| D | `source` + `read_replica` | só no primário (a réplica recusa **apontando-o**) | nos dois |

O modo A é o que este documento descreve desde a seção 1 — os outros três
são papéis por cima do mesmo laço. `replica` continua existindo e continua
**exatamente** como era: os papéis novos são pedidos, não impostos.

---

## 10. Read replica e spare — recusa com endereço

**`read_replica`** formaliza o que o `somente_leitura` fazia por convenção. A
diferença está na recusa: em vez do genérico «servidor em modo somente
leitura», a escrita recebe um erro **próprio e estável** que aponta o primário:

```json
{"ok":false,"nome":"ESCRITA_NA_REPLICA","codigo":4003,
 "erro":"escrita na replica: este servidor e uma replica de leitura; escreva no primario 10.1.1.102:5000 (curitiba)"}
```

Código para o cliente tratar (reconectar no primário), texto para gente. A
réplica clássica **não muda**: quem tem `papel: replica` continua recebendo a
recusa antiga — cliente escrito antes deste papel não passa a receber um erro
que não conhece.

**`spare`** é a reserva de contingência, e reserva é reserva: recusa **também
a leitura** de cliente comum, com `SPARE_EM_ESPERA` (4004) — e o texto ensina
a saída (`spare_promover`). O que o spare atende é uma **lista de permissão**
(`OPS_NO_SPARE`): sessão, administração, monitoramento, conferência
(`checksum`, `verificar`, `diario`, `backup`), metadado (`bancos`, `tabelas`,
`esquema`) e a própria replicação. Operação nova nasce **barrada** no spare
até alguém decidir o contrário — o mesmo princípio do portão que nega operação
desconhecida.

**`spare_promover`** é o equivalente do `HRSTransformSpareIntoServer` do
HFSQL(R): operação **local e manual**, exige `administrar`, e no processo vivo
faz três coisas — o laço de réplica para na rodada seguinte, a escrita abre
(mesmo com `somente_leitura` no arquivo), o papel vira `source` (o `ping`
passa a dizê-lo). Ela **não** reescreve o `config.json`: o arquivo é do
administrador, com os comentários dele, e a resposta avisa o que ajustar para
o próximo arranque. A promoção é uma função coesa no servidor
(`promover_para_primario`), e a op é uma casca fina — de propósito, para a
promoção automática da frente de cluster se pendurar no mesmo degrau.

---

## 11. Agendamento — streaming ou por janela

O laço de sempre é **streaming**: puxa, aplica, dorme `reconectar_em`, repete.
Dois campos novos na **origem** trocam isso por janelas:

```json
"origens": [
  {"nome":"curitiba", "host":"10.1.1.102", "porta":5000, "...":"...",
   "cada_minutos": 15},
  {"nome":"matriz",   "host":"10.2.1.10",  "porta":5000, "...":"...",
   "hora": "02:30"}
]
```

- `cada_minutos: N` — uma janela a cada N minutos;
- `hora: "HH:MM"` — uma janela por dia, àquela hora (**UTC**, a mesma
  convenção do backup agendado — as duas agendas do servidor não podem
  discordar de fuso);
- **ausentes ou zero = streaming**, byte a byte o comportamento de sempre. É o
  teste que mais importa (`origem_sem_agendamento_continua_streaming`).

Na janela a réplica puxa **até esgotar** (repete rodadas até «nada a fazer»),
porque a próxima chance é só na janela seguinte. A primeira rodada acontece no
arranque, sem esperar: uma réplica que sobe atrasada não fica horas fingindo
que está em dia. A agenda é **por origem** — multi-source pode ter uma origem
streaming e outra noturna — e mora no próprio laço da réplica, não no
subsistema de jobs: job roda pedidos de protocolo, e a réplica não passa pelo
protocolo.

---

## 12. Bidirecional (multi-master) — a parte funda

Dois servidores, cada um réplica do outro, os dois recebendo escrita:

```json
"replicacao": {"papel": "multi", "id_servidor": "alfa",
               "origens": [{"nome":"beta", "host":"10.2.1.10", "porta":5000, "...":"..."}]}
```

Os dois problemas reais, e a peça de cada um:

### O laço infinito, a origem no evento — e o que é mesmo que o mata

A alteração que A aplicou vinda de B não pode voltar para B. Cada evento do
`.log` carrega agora a **origem** da escrita — os 2 bytes reservados do
cabeçalho viraram um u16 com o hash do `id_servidor` de onde ela nasceu; zero
= local, e todo evento antigo lê zero, que é a leitura certa
(`docs/FORMATO.md` §4). O `replicar` ganhou o campo **`para`**: o source **não
devolve** os eventos cuja origem é quem pede, e a posição (`ate`) anda por
cima deles mesmo assim — suprimir é não mandar de volta, não fingir que o
evento não existe. A réplica ainda descarta por conta própria o que tiver a
origem dela.

**E aqui vai uma correção que a bancada obrigou a escrever.** A primeira
redação desta seção dizia que a supressão de origem é o que mata o laço
infinito. Medido, não é. Repondo o defeito — o filtro do `para` fora — o laço
**não** viveu; tirando também a segunda guarda, **continuou não vivendo**. A
razão está na seção do conflito: a regra «mais recente vence» é
**idempotente**, porque empate de carimbo *e* de origem **não** vence. O
evento que volta encontra no outro lado o toque idêntico que ele mesmo criou,
perde a comparação, e é descartado **sem gerar escrita** — e sem escrita não
há evento novo, que é o único jeito de o laço se alimentar.

O que a supressão compra, então, não é a correção: é **trabalho e rede**.
Medido na bancada, estágio (c2): do diário de alfa, beta leva 1 evento onde
uma réplica sem `para` leva 2 — **50% do tráfego de volta poupado** nesse par,
mais o custo de decodificar cada imagem para jogar fora. Ela também é a rede
de proteção do dia em que a regra do conflito mudar: qualquer variante em que
a reaplicação gere um evento reabre o laço na hora, e aí a supressão passa a
ser load-bearing. Fica pelos dois motivos, agora sabendo qual é qual.

*Diagnóstico plausível não é diagnóstico medido — e o errado sobrevive melhor
quando o conserto funciona por outro motivo.*

O hash é CRC-32 dobrado em u16 e **pode colidir** (1 em 65.535 por par); a
colisão suprimiria eventos de um servidor inocente, então a rodada confere ao
conectar — ids diferentes com o mesmo hash param com erro que manda trocar um
id. Os dois lados com o **mesmo** `id_servidor` idem.

### O conflito: modificação mais recente vence

O mesmo registro alterado dos dois lados antes de sincronizar: vence o
**carimbo mais recente**, o que o `.log` já tem por evento. Para isso ser
justo, o evento **aplicado** guarda o carimbo do *nascimento* da escrita
(copiado do evento original), e não o relógio da chegada — senão venceria
sempre quem sincronizou por último.

Com todas as letras: **essa regra exige relógios sincronizados entre os
servidores (NTP)**. Sem isso, o lado com o relógio adiantado vence sempre —
toda escrita dele parece «mais recente», e o trabalho do outro lado é
desfeito em silêncio. Empate de carimbo desempata pela **origem numérica
maior**: arbitrário, determinístico e igual dos dois lados, que é o que faz
os dois convergirem (exatamente um aplica, o outro descarta).

### A identidade é a chave, nunca o rowid

**A ordem de digitação é sagrada em cada servidor**: cada `.reg` mantém a SUA
ordem de chegada, e o insert local de A e o de B podem ganhar o mesmo rowid.
Entre servidores a linha se identifica pela **chave única de uma coluna**
(chave primária, ou o primeiro índice único) — o mesmo desenho da sincronia do
DbLink. O aplicador busca pela chave: achou, altera **mantendo o rowid e o
rownum locais**; não achou, insere e a linha entra na ordem de chegada
*daqui*.

**As colunas do motor se comportam sozinhas, e isso foi conferido e não
suposto:** o `rownum` que chega de fora é **ignorado** numa inclusão (a linha
entra na ordem de chegada *daqui*) e **herdado do local** numa alteração —
`numerar_linha` já fazia isso por outro motivo, e é o que mantém a ordem de
digitação de cada servidor intacta. A coluna de **sequência**, ao contrário,
**viaja**: o valor que veio é gravado como está, e o contador local só *aprende*
com ele (`anotar_sequencia`), de modo que nenhum dos dois servidores reemite um
número que o outro já usou.

Consequência honesta: **o modo bidirecional exige tabela com chave única** —
o HFSQL(R) também impõe identificador adequado para replicar. Tabela sem
chave (ou só com chave composta, que fica para quando alguém precisar) é
**recusada com o motivo escrito**, visível em `replicacao_estado`:

```json
"recusas": {"loja/log_livre": "sem chave unica de uma coluna: o bidirecional
            casa as linhas pela chave, e log_livre nao tem uma (crie um
            indice unico, ou uma chave primaria)"}
```

### Exclusão viaja — com a chave dentro

O evento de exclusão existe no diário, então **viaja**. Só que a exclusão
clássica vai sem imagem (o rowid basta entre réplicas fiéis), e no
bidirecional o rowid não identifica nada do outro lado. No papel `multi` a
exclusão física passa a **carregar a imagem da linha** — a chave mora nela. O
conflito exclusão×alteração se resolve pela **mesma regra do mais recente**:
excluir é a última modificação como qualquer outra. Alteração mais nova que a
exclusão vence (a linha reaparece re-inserida); exclusão mais nova vence (a
linha sai). A exclusão **suave** já viajava como alteração (é o que ela é no
`.reg`) e continua.

### A posição vira estado próprio — e perdê-la não é grave

No modo A a posição é o próprio diário da réplica: cada evento aplicado gera
exatamente um evento local. No bidirecional isso quebra — o diário local
mistura escrita local com aplicada, e os eventos suprimidos pelo `para`
avançam a posição sem gerar nada aqui. A posição consumida por
origem/tabela vai então para `replicacao-posicoes.json`, ao lado dos dados.
Perder o arquivo recomeça do zero e é **inofensivo**: a aplicação é por chave
com «mais recente vence», e reaplicar um evento já visto perde para o toque
igual que já está registrado — custa releitura, nunca dado.

### O que o bidirecional NÃO é

- **Não é para mais de dois ainda.** O desenho (origem por evento) suporta
  malha, mas só o par foi provado na bancada. Três ou mais entram quando
  houver prova.
- **Não conserta relógio.** NTP é pré-requisito, não sugestão.
- **Não olha o passado.** O confronto por chave lê as imagens do diário;
  eventos gravados **antes** de o modo multi ligar não têm origem (leem
  «local») e podem não ter imagem. Um servidor que passou a vida como réplica
  clássica não vira metade de um par multi sem recomeçar as tabelas.
- **Não substitui transação.** Duas escritas relacionadas em tabelas
  diferentes podem chegar ao outro lado em rodadas diferentes.

---

## 13. O que está feito, e o que falta

| | |
|---|---|
| ☑️ | `.log` versão 2 com imagem da linha, atrás do interruptor |
| ☑️ | Ops `posicao`, `replicar` e `aplicar` |
| ☑️ | Laço da réplica dentro do `phxsqld`: puxar, aplicar, conferir o rowid |
| ☑️ | Criar na réplica a tabela que ainda não existe, do esquema cru do source |
| ☑️ | Reconexão e retomada pela posição — medido: 1,0 s para 4.000 eventos |
| ☑️ | Multi-source: uma thread por origem |
| ☑️ | **Cascata** — Master → Slave01 → Slave03. O segundo salto custou 1.827 ms contra 1.679 do primeiro |
| ☑️ | **Quatro modos**: A (source→réplica), B (multi-master), C (spare), D (read replica) |
| ☑️ | **Agendamento** por origem: `cada_minutos` ou `hora`; ausente = streaming |
| ☑️ | `spare_promover` — o degrau **manual** da promoção |
| ☑️ | `replicacao_estado` e `replicacao_testar`: o que um assistente precisa para provar |
| ☐ | **Escrever a configuração pela tela.** Não há op que grave `replicacao` no `config.json`, e reescrevê-lo perderia os comentários do administrador — o caminho que o projeto já escolheu duas vezes é um arquivo próprio (como `dblink.json` e `jobs.json`). Enquanto não existir, um assistente configura *mostrando o que pôr no arquivo* e prova o resto pelas ops |
| ☐ | Bidirecional com **mais de dois** servidores: o desenho suporta, só o par foi provado |
| ☐ | Long-poll no Source, para a réplica não perguntar à toa |
| ☐ | Espera crescente na reconexão (hoje é intervalo fixo) |
| ☐ | TLS no transporte — hoje o JSON vai em claro e depende do IPSec |

### A posição é o diário da própria réplica

A réplica não guarda um arquivo com «apliquei até aqui». Ela **conta os eventos
do `.log` dela** — e é isso que faz a retomada funcionar sem estado extra:
matar a réplica no meio de um lote não perde nem repete, porque o número que
ela usa é o que os arquivos dela dizem, não o que ela lembrava.

Para isso valer, cada evento aplicado tem de gerar **exatamente um** evento
local. É por isso que uma exclusão que não acha o que excluir é tratada como
divergência e para: se passasse batido, o evento não geraria evento, a posição
não andaria, e a replicação giraria em falso puxando o mesmo para sempre.

### Cascata

Uma réplica pode ser origem de outra, e para isso ela precisa de
`imagem_da_linha` ligada **nela também** — senão o diário dela grava que a
linha mudou sem gravar a linha, e o segundo salto não tem o que aplicar. O erro
é explícito e diz o que ligar.

## 14. A bancada dos quatro modos

`bancada/replicacao/modos.py` sobe servidores **só dela**, nas portas
5330-5339, e derruba **só os processos que criou** (nunca `pkill phxsqld` —
pode haver outro servidor na máquina). Cada estágio escreve o resultado
esperado **antes** de rodar. A corrida completa:

| estágio | o que prova | medido |
|---|---|---|
| a | modo A pelas mesmas ops que um assistente chamaria | B alcança os 50 eventos, retratos iguais, A continua com 50 (B não devolve); `posicao` expõe a chave |
| b | agendamento com `cada_minutos: 1` | a linha gravada depois da janela **não** apareceu em 35 s e apareceu em **56,5 s** |
| c | bidirecional, ida e volta | alfa→beta 0,4 s, beta→alfa 1,0 s, e os eventos **param em 2 de cada lado** |
| c2 | o que a supressão de origem poupa | beta leva 1 evento onde uma réplica sem `para` leva 2 — **50% do tráfego de volta** |
| d | conflito nos dois sentidos | k1: beta (mais novo) vence **nos dois** em 1,0 s; k2: alfa vence **nos dois** em 1,1 s |
| e | tabela sem chave única | recusada, com o motivo legível em `replicacao_estado` |
| f | spare | `varrer` e `inserir` recusados (4004), `ping`/`checksum` passam; após `spare_promover`, papel `source` e os dois passam |
| g | read replica | leitura ok; escrita recusada com 4003 **apontando 127.0.0.1:5338** |
| h | comportamento velho | source/réplica do molde dos `Config_exemplo_02/03` replicam como antes, e a recusa é a `ACESSO_NEGADO` de sempre |

### Aprendizados — inclusive os que não deram em nada

1. **A hipótese que morreu, e a que ela gerou.** Repor o defeito na supressão
   de origem não fez o laço viver (§12). A hipótese caiu, e do enterro saiu a
   medição que ficou: os 50% de tráfego do estágio (c2), e a explicação certa
   de quem mata o laço — a idempotência do conflito. O teste que **isola** o
   filtro do `para` acabou sendo o unitário
   (`evento_nao_volta_para_quem_o_escreveu`), que falha na hora com o defeito
   reposto; a bancada, por causa da redundância, não distinguia as camadas.
   Redundância é boa para o sistema e **cega para o teste** — quando houver
   duas guardas, o teste que vale é o que derruba uma de cada vez.
2. **O `rownum` já estava protegido, e por acidente feliz.** O medo era o
   bidirecional embaralhar a ordem de digitação: uma linha vinda de fora
   trazendo o número de ordem do outro servidor. Não traz — `numerar_linha`
   **ignora** `rownum` que chegue de fora numa inclusão e **herda o local**
   numa alteração, decisão que já estava no motor por outro motivo. A ordem de
   digitação de cada servidor continua sendo dele, sem uma linha de código
   nova.
3. **O que a réplica clássica faz com escrita de cliente já era certo** — ela
   recusa, pelo `somente_leitura`. O papel `read_replica` não conserta defeito
   nenhum: ele **nomeia** o contrato e troca a recusa genérica por uma que
   aponta o primário. Vale dizer porque a missão perguntava se havia defeito
   ali: não havia.
4. **Fechar o soquete do lado certo.** Os primeiros estágios não subiam ao
   reusar uma porta: quem fecha primeiro fica com o `TIME_WAIT`, e ele tem de
   ficar do lado do **cliente**. A bancada fecha os soquetes dela antes de
   derrubar cada servidor — é a mesma família da lição do `makefile()` que já
   está no CLAUDE.md.

## 15. O que isto NÃO é

- **Não é replicação síncrona.** É assíncrona, como o padrão do MySQL(R): a
  réplica fica atrás do Source por algum tempo. Medido: 1,3 s a 2,1 s com o
  laço em 2 s.
- ~~A réplica aplica mais devagar do que o master escreve~~ — **este limite
  caiu, e a causa que estava escrita aqui estava errada.** Medido
  (`DESEMPENHO.md` §4.5): reencodar o payload custa 0,35 µs de 229; o que
  custava era o **source** varrendo o diário desde o começo a cada lote. Com a
  marca de posição, cada réplica aplica **17.450 eventos/s** e as três juntas
  ~52.000 — mais do que os 34.048 que o master escreve. O que continua
  verdadeiro: o atraso normal é o `reconectar_em`, e réplica não é backup.
- ~~Não resolve conflito de escrita nos dois lados~~ — **resolve, no papel
  `multi`**: mais recente vence, pelo carimbo, com as três exigências da §12
  (chave única, relógios em NTP, `id_servidor` nos dois). O que continua
  verdadeiro: no papel `replica` é um caminho só, e ali escrita de aplicação
  quebra a numeração.
- **Não substitui backup.** Réplica repete o `DELETE` errado que você fez no
  Source, e repete rápido.
- **Não há transação**, então não há ordem global entre tabelas a preservar —
  e é por isso que a posição é por tabela. Quando as transações entrarem, entra
  junto um número de sequência do database inteiro.

---

## 16. Como refazer a medição

```bash
cargo build --release
# a replicação clássica, com os quatro servidores e a vazão:
python3 bancada/replicacao/montar.py /tmp/phx-replicacao
python3 bancada/replicacao/medir.py 100000
# os quatro modos, nas portas 5330-5339 (leva ~3 min: o estágio (b)
# espera uma janela de verdade):
python3 bancada/replicacao/modos.py /tmp/phx-modos
```

`montar.py --cascata` põe o Slave03 puxando do Slave01. Detalhes e a última
corrida em `bancada/replicacao/LEIA-ME.md`.

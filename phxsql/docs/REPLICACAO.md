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

**A segunda tranca não existia, e esta frase mentiu até a bancada em contêiner
medi-la.** `replicas_autorizadas` estava no `config.json`, nesta seção e na
tela de configuração desde que os papéis novos entraram, e **nenhuma linha de
código o lia**. O estrago, medido em contêiner (§17, estágio (e)): um vizinho
de rede com um `config.json` de réplica vazado — mesmo token, mesmo usuário,
mesmo `senha_hash` — levou os **200 de 200 eventos** do diário do source *com
a lista preenchida*. Consertado no portão único (`portoes_do_pedido`, portão
2a-bis), com as três garantias da casa:

- **pedida, não imposta**: lista vazia — o padrão, e o que todo `config.json`
  de hoje tem — libera todos, byte a byte o comportamento de sempre. O teste
  que trava isso é o do comportamento **velho**
  (`sem_replicas_autorizadas_nada_muda`);
- **um portão só**, e não espalhado por `posicao`/`replicar`/`aplicar`, porque
  a que alguém esquecesse viraria a porta dos fundos;
- **o campo novo que o portão passou a olhar é o IP da sessão**, então a
  pergunta obrigatória é quem *não* tem esse campo: job agendado, rotina
  interna e a replicação chamada de dentro chegam com `ip` vazio — e vazio ali
  é a verdade, não uma falta. Não vieram de fora, não há IP para autorizar, e
  o portão não se aplica a eles (`caminho_interno_sem_ip_nao_e_barrado_pela_lista`).

*Configuração que não é lida mente* — e mente pior quando o assunto é quem
alcança o dado. Depois do conserto o mesmo intruso leva **0 de 200**.

Vale dizer o que a lista de IPs **não** resolve, e o contêiner tornou isso
visível: num orquestrador o IP do vizinho muda a cada recriação. Lista por IP
só é operável com endereçamento fixo — no `compose-e-firewall.yml` isso é o
bloco `ipam`, e num datacenter é a reserva no DHCP. Sem isso, a lista barra a
própria réplica no primeiro `docker compose up` depois de um reboot.

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
| ☑️ | **Cifra do fio no transporte** — aperto de mão estilo Noise (X25519 + HKDF + ChaCha20-Poly1305), ligado por origem com `"cifra": true` e o pino em `"chave_do_fio"`. Não é TLS, e o limite está escrito: sem pino protege só de escuta passiva. Ver [CIFRA-DO-FIO.md](CIFRA-DO-FIO.md) |
| ☐ | **O pulso do CLUSTER continua em claro.** A replicação do cluster passa pelo mesmo laço e poderia ir cifrada, mas o pulso da eleição vai por outro caminho (`cluster.rs`): cifrar metade do tráfego do cluster é pior que não cifrar nenhuma, porque parece protegido |
| ☑️ | **`replicas_autorizadas` passou a ser lido** — era campo sem leitor até a bancada de contêiner medir 200 de 200 eventos vazando com a lista preenchida (§7) |
| ☐ | **Buscar o lote FORA da trava de dados.** Medido (§17): `varrer` esperou **30,7 s** numa réplica cortada em silêncio, e no bidirecional os dois lados se trancam por 30 s com a rede sã. É o item 2 da §3.2 do `PENDENCIAS.md` visto de dentro da replicação, e é a causa; a espera crescente e o `connect` com prazo tratam o sintoma |
| ☐ | **O endereço do `REDIRECIONA` é o da origem configurada**, que é «por onde *eu* alcanço o primário» e nem sempre «por onde *você* alcança» (§17, achado 3). Um campo próprio para o endereço que se anuncia ao cliente resolveria |
| ☐ | **`replicacao_estado` não conta nada durante um corte silencioso** — `ultima_rodada` fica com o carimbo de antes e `ultimo_erro` fica nulo, porque o laço está pendurado. Falta um «quando foi a última rodada BEM-SUCEDIDA» que envelheça sozinho, para o monitoramento distinguir «nada a replicar» de «cego» |

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

```bash
# e os quatro modos em CONTEINERES, que e onde endereco, firewall e
# particao existem de verdade (~20 min, e remove tudo no fim):
cargo build --release --target x86_64-unknown-linux-musl --bin phxsqld
python3 bancada/replicacao/docker/provar.py
```

`montar.py --cascata` põe o Slave03 puxando do Slave01. Detalhes e a última
corrida em `bancada/replicacao/LEIA-ME.md` e em
`bancada/replicacao/docker/LEIA-ME.md`.

---

## 17. Os quatro modos em contêiner — o que só o isolamento de rede prova

```bash
rustup target add x86_64-unknown-linux-musl        # uma vez
cargo build --release --target x86_64-unknown-linux-musl --bin phxsqld
python3 bancada/replicacao/docker/provar.py        # ~20 min, tudo
```

Cinco `compose`, um por modo mais o do firewall, e um `provar.py` que sobe,
mede e **remove tudo** — contêineres, redes e volumes — mesmo quando falha.
Detalhes em `bancada/replicacao/docker/LEIA-ME.md`.

### Por que Docker muda alguma coisa

A bancada de processos (§14 e §16) prova que os quatro modos **funcionam**.
Ela não prova — e não tem como — três coisas, e elas são a razão desta frente
existir.

**1. Endereço.** Com tudo em `127.0.0.1`, o `bind` do source e o endereço que
a réplica procura são o mesmo por acidente. Em contêiner a origem é o **nome
de serviço** (`"host": "fonte"`), resolvido pelo DNS do Docker, e isso obriga
o `bind` a ser `0.0.0.0:5000`. O estágio (0) repõe o defeito de propósito —
`bind: 127.0.0.1:5000` **dentro** do contêiner — e mede o silêncio dele: o
vizinho na mesma rede não abre a porta, e a réplica fica em **0 evento por
20 s sem um único erro em lugar nenhum**. Nem no log do source, nem no
`replicacao_estado`, nem na tela. Trocando a linha do config, ela alcança em
0,51 s. É o erro de configuração mais fácil de cometer ao pôr o PhxSql em
produção, e ele é **invisível** na bancada de processos.

**2. Firewall e isolamento (§7).** No loopback não há o que trancar. Aqui há:
rede própria com IPAM fixo, um intruso com o `config.json` vazado, e
`iptables` de verdade no namespace de rede do source. Foi assim que a §7
deixou de ser um desenho — e foi assim que o `replicas_autorizadas` que
ninguém lia apareceu.

**3. Queda e partição.** `docker kill` mata sem chance de fechar arquivo — o
que uma máquina que perde energia faz. E **cortar a rede sem matar ninguém**
só existe aqui: os dois lados vivos, os dois aceitando escrita, e cegos um
para o outro. É a lição do `BULKINSERT` um degrau adiante: teste unitário não
prova queda de conexão, soquete prova, e contêiner prova o que soquete no
loopback não alcança.

### A imagem, e os três tamanhos que ela tem

`FROM scratch` + o binário musl: sem shell, sem gerenciador de pacotes, sem
libc solta. É a regra de zero dependência externa cobrando o dividendo dela —
a mesma que fez a compilação cruzada para Windows funcionar de primeira. Um
contêiner sem shell não tem como um invasor rodar nada dentro dele; a
superfície é o próprio servidor e mais nada.

O tamanho merece cuidado, porque **três comandos dão três números** e a
primeira redação desta seção publicou o errado com o rótulo certo:

| de onde sai | quanto | o que é |
|---|---:|---|
| soma do `docker history` | **6,42 MB** | o conteúdo da imagem |
| `docker image inspect .Size` | 2,69 MB | o que se **baixa** (comprimido) |
| `docker images` | 9,11 MB | o de cima mais o manifesto de atestação do BuildKit |

Estava publicado «a imagem tem 2,7 MB» — número certo, rótulo errado: era o
comprimido, e quem rodasse `docker images` veria 9,11 e concluiria que o
documento mentia. Hoje o `provar.py` mede os três e grava os três, porque
número com rótulo trocado é a mesma família do número digitado à mão.

### Modo A — Primary → Replica

100.000 linhas, com `imagem_da_linha` ligada e `reconectar_em: 2`, no
contêiner e **em processos, com o mesmo código**: a função de carga não sabe
qual dos dois está do outro lado, que é a única forma de o trabalho ser
mesmo igual (regra 4 da bancada).

| | contêiner | processo |
|---|---:|---:|
| escrita no source | 17.147 linhas/s | 17.998 linhas/s |
| a réplica alcançar 100.000 eventos | **2,51 s** | 2,61 s |
| taxa de aplicação | 39.831 eventos/s | 38.331 eventos/s |
| atraso de 12 inserções soltas (mín/mediana/máx) | 4 / 1.993 / 2.041 ms | 3 / 3 / 2.025 ms |
| soma do servidor, linhas e slots | idênticos (101.013) | idênticos (101.013) |
| retrato SHA-256 de cada linha | `39787c620feeed8f` | `39787c620feeed8f` |

**A rede do Docker não custou nada de mensurável.** A escrita saiu 5% mais
lenta no contêiner nesta corrida e 5% mais **rápida** na anterior (17.936 ×
17.103); o alcance saiu mais rápido nas duas. Diferença que troca de sinal
entre corridas é ruído da máquina, não custo de transporte — e vale dizer que
a máquina estava compartilhada com outros trabalhos, o que é a razão de as duas
medições terem sido feitas **na mesma corrida, uma logo depois da outra**.

A linha do atraso merece um aviso, e ela é a quinta regra que esta bancada
aprendeu sozinha. **Uma amostra de atraso não é atraso** — a primeira corrida
mediu a mesma inserção em 2.035 ms no contêiner e 53 ms no processo, e a
diferença inteira era *onde no ciclo de 2 s do `reconectar_em`* a escrita caiu.
Doze amostras desfazem a manchete falsa («o Docker é 38× mais lento no
atraso»), mas **não** estabilizam a mediana: nesta corrida ela deu 1.993 ms no
contêiner e 3 ms no processo, e na anterior deu 1.983 ms no contêiner e
2.025 ms no processo — os dois lados já foram o lento. As amostras são
correlacionadas, porque cada escrita acontece logo depois de a anterior ter
chegado, então elas herdam a fase do laço. **O número que se sustenta é o
máximo**, e ele bate nos dois (2.041 e 2.025 ms): é a janela do
`reconectar_em`, e tudo abaixo dela é fase.

A escrita na réplica continua recusada com a `ACESSO_NEGADO` de sempre —
`papel: replica` não muda de comportamento por causa dos papéis novos.

### Queda do nó — o teste que a bancada de processos nunca fez direito

`docker kill` na réplica (SIGKILL, sem `Drop`, sem `sincronizar`, sem fechar
descritor), 4.000 linhas no source com ela morta, `docker start`:

| | |
|---|---:|
| voltou a atender | **480 ms** |
| alcançou os 4.000 eventos, contado desde o `docker start` | **0,48 s** |
| soma do servidor e retrato SHA-256 depois | idênticos ao source |

Matar o processo no meio da aplicação **não deixou rastro**: a posição é o
diário da própria réplica (§13), então ela conta os eventos que os arquivos
dela têm e retoma dali. Nada perdido, nada repetido.

### O que o contêiner mostrou e os processos escondiam

**Achado 1 — `replicas_autorizadas` não era lido.** Está contado na §7. Em
uma frase: a segunda tranca da §7 não existia, um vizinho com o `config.json`
de réplica vazado levava **200 de 200 eventos** do diário com a lista
preenchida, e hoje leva 0.

**Achado 2 — a trava de dados fica presa atrás de uma leitura de rede.** O
laço da réplica segura `self.dados.lock()` (`alcancar_tabela`, e o mesmo em
`alcancar_tabela_bidi`) e, **de dentro dela**, faz a ida e volta de rede que
busca o lote. Numa rede sã isso é invisível: a resposta chega em
microssegundos. Quando não chega, a trava fica presa até o prazo de leitura de
**30 s** do cliente da réplica — e todo pedido de cliente que precise da trava
espera atrás.

O contraste é o diagnóstico, e está no estágio (a3). Corte silencioso entre a
réplica e o source, com o source **escrevendo sem parar** (para o laço estar
dentro do `puxar` sob a trava quando o corte cai). Na réplica, em 43 s e
30.521 amostras:

| | |
|---|---:|
| pior `ping` (não precisa da trava) | **6 ms** |
| pior `varrer` (precisa da trava) | **29.456 ms** |

O servidor está no ar; o que espera é a trava. Isto **não** aparece com
`docker stop`: matar o processo devolve RST, o `puxar` falha na hora e a trava
é solta na hora. Só o corte que **não responde** produz a espera — e cortes
que não respondem são o caso comum de verdade (firewall, cabo, rota que
sumiu). É a consequência medida do item 2 da §3.2 do `PENDENCIAS.md`, as
tomadas da trava fora do ponto único.

**Achado 2b — no bidirecional, os dois lados se trancam um ao outro.** É o
mesmo mecanismo levado ao pior caso, e ele **não precisa de corte nenhum**.
`alcancar_tabela_bidi` toma a trava **deste** servidor e pede `replicar` ao
outro; do outro lado, servir `replicar` (e `posicao`) também precisa da trava
de **lá** — `op_replicar` e `op_posicao` chamam `travar_dados()`. Com fila nos
dois ao mesmo tempo, cada um segura a própria trava esperando a resposta do
outro, que não pode vir. Ninguém sai até o prazo de 30 s estourar nos dois, e
eles podem reentrar em passo.

Medido no estágio (b-abraco), com a rede **perfeitamente sã**: 50.000 linhas
escritas em cada lado ao mesmo tempo. As mesmas 100.000 linhas no modo A
entram em ~5,8 s; aqui a escrita do cliente levou **33,3 s**, com um `EAGAIN`
(`Resource temporarily unavailable`, que é o prazo de leitura estourando) no
diário de **cada** servidor. Um ciclo de abraço, e a escrita de quem estava
digitando parou junto.

É também a explicação certa do estágio (b-cortes), e vale registrar que a
primeira redação estava errada: ela dizia que a lentidão do corte silencioso
vinha da «espera exponencial do SYN do núcleo», porque o `connect` do laço não
tem prazo. Plausível, e falso — o diário dos dois contêineres tinha a
resposta: **sete `Resource temporarily unavailable` em cada lado**, sete vezes
30 s, no corte que levou 228,9 s para se recuperar. É o prazo de **leitura**,
não o de conexão. *Diagnóstico plausível não é diagnóstico medido.*

E a assinatura que fecha o diagnóstico é a **simetria**: os `EAGAIN` saem em
número igual nos dois lados — 7 e 7 — em corridas diferentes. Um nó sozinho
esperando um vizinho quieto daria contagens desiguais; o empate é o que só um
abraço produz, porque os dois esperam **um pelo outro** e saem juntos quando o
prazo estoura nos dois.

Vale a nota honesta de método: este achado **não veio de um estágio planejado**.
Veio de olhar `docker logs` dos dois contêineres enquanto um estágio demorava
mais do que devia. A bancada mediu o sintoma (228,9 s) e escreveu a causa
errada; o diário do servidor tinha a causa certa o tempo todo. *Quando o número
surpreender, leia o log antes de explicar o número.*

**Achado 3 — o `REDIRECIONA` aponta o endereço da ORIGEM, não o do cliente.**
A read replica recusa escrita com
`REDIRECIONA primario:5000 (primario) -- ...`, e `primario` é o nome de
serviço do compose: existe dentro da rede e **não resolve no hospedeiro**
(medido: `getaddrinfo` falha). O cliente que recorta o prefixo e reconecta —
que é para isso que o prefixo existe — não chega a lugar nenhum. Em
`127.0.0.1` o defeito é invisível, porque ali o endereço da origem por acaso
também serve para o cliente. O endereço sai de `origens[0]` do config da
réplica, então ele é «por onde **eu** alcanço o primário», e nem sempre é «por
onde **você** alcança».

**Achado 4 — e o de sempre: depois do failover manual, o redirecionamento
fica órfão.** No estágio (c) o primário morre, o spare é promovido, e a read
replica continua respondendo `REDIRECIONA primario:5000` — para um endereço
morto. Não é defeito do papel: é o que «failover **manual**» quer dizer (§8),
e está aqui escrito para quem escrever o failover automático saber que essa
ponta também precisa mudar.

### Modo B — Multi-Master, e a partição de verdade

Ida e volta e laço morto continuam como na §14. O que é novo é a **partição**:
`iptables` no namespace de beta, nos dois sentidos, contra o IP de alfa. Os
dois continuam vivos, os dois continuam aceitando escrita, e não se enxergam.

Durante o corte, cada lado alterou a mesma chave e criou chaves próprias. Ao
religar:

- a chave disputada ficou com **o carimbo mais novo nos dois** servidores;
- todas as chaves atravessaram nos dois sentidos;
- **os rowids são diferentes em cada servidor** — a linha que nasceu em beta
  com rowid 3 entrou em alfa com rowid 6, porque em alfa já havia três linhas
  que beta nunca viu. É a §12 provada em vez de afirmada: se a replicação
  casasse por rowid, ela gravaria a linha de um por cima da do outro.

E o corolário que caiu junto, e que muda como se confere o modo B:

> **A soma de verificação do servidor (`checksum`) não serve para comparar dois
> pares bidirecionais convergidos.** Ela é ORDENADA de propósito — multiplica
> antes de somar, justamente para que trocar duas linhas de lugar mude o
> resultado. Isso é o que se quer no modo A, onde a réplica reproduz a ordem
> de digitação do source. No modo B é o contrário: **a ordem de digitação é
> sagrada em cada servidor**, então dois lados convergidos têm somas
> diferentes por construção. Medido: conteúdo casado pela chave idêntico
> (`771ff218d033f64c` nos dois), somas do servidor `ae8056eac15401d1` e
> `bd5c6e435cd98de9`. Quem comparar modo B pela soma vai ver divergência onde
> não há nenhuma, e vai parar uma replicação sadia.
>
> A comparação certa no modo B é o conteúdo **ordenado pela chave, sem rowid e
> sem rownum** — é o que `retrato_por_chave` faz na bancada.

**Durante o corte, `replicacao_estado` não conta nada.** `ultima_rodada` fica
com o carimbo de antes do corte e `ultimo_erro` fica **nulo** — porque o laço
está pendurado num `connect`/`read` que não volta, e sem rodada não há erro
para gravar. Quem olha o estado vê o retrato de antes e conclui que está tudo
bem. Um corte silencioso é, para o monitoramento, indistinguível de «não houve
nada para replicar».

**Os dois cortes que o mundo tem, cronometrados.** Depois de a rede voltar,
quanto tempo até a linha chegar do outro lado:

| duração do corte | `REJECT` (processo morto, RST) | `DROP` (cabo cortado, silêncio) — 4 corridas |
|---:|---:|---|
| 3 s | 0,0 s | 0,2 · 0,2 · 0,4 · 0,2 s |
| 20 s | 0,0 s | **229,0 · 31,5 · 0,2 · 293,8** s |
| 45 s | 0,3 s | 25,4 · 24,4 · 25,2 · 25,4 s |

Com RST a retomada é **imediata sempre**, e não depende da duração: o
`connect` falha na hora, o laço tenta de novo no `reconectar_em` seguinte.

Com silêncio ela é **variável e não limitada**, e a linha do meio é a prova: o
**mesmo** corte de 20 s se recuperou em 0,2 s numa corrida e em 293,8 s em
outra. Não é a duração do corte que manda — é **quantos ciclos de abraço** (o
achado 2b) os dois lados gastam antes de saírem de passo, e cada ciclo custa os
30 s do prazo de leitura. Uma faixa de três ordens de grandeza para o mesmo
estímulo é a assinatura de um estado que se auto-sustenta, não de uma espera
proporcional.

É o argumento medido para dois itens da §13 — *espera crescente na reconexão*
e um `connect`/`read` com prazo curto no laço — e para o item 2 da §3.2 do
`PENDENCIAS.md`, que é o que resolve a causa em vez do sintoma.

**E o abraço, sem corte nenhum.** O estágio (b-abraco) é a prova de que o
achado 2b não precisa de rede quebrada: 50.000 linhas escritas em cada lado ao
mesmo tempo, com uma `Barrier` para as duas cargas largarem no mesmo instante,
e a rede perfeitamente sã.

| | |
|---|---:|
| as mesmas 100.000 linhas, num servidor só, em modo A | ~5,8 s |
| 50.000 em cada lado do par bidirecional, ao mesmo tempo | **33,3 s** |
| `EAGAIN` novos no diário | **+1 em alfa e +1 em beta** |
| pior `checksum` da bancada durante o episódio | 1.997 ms |

Um ciclo de abraço, e a escrita de quem estava digitando parou junto. O
`EAGAIN` (`Resource temporarily unavailable`) no diário dos dois é a
assinatura: é o prazo de leitura de 30 s do cliente da réplica estourando
porque o outro lado, vivo e saudável, não conseguiu responder.

Consequência prática para quem usa modo B: **as duas metades não devem receber
carga pesada simultânea** enquanto o laço buscar o lote de dentro da trava. Não
é um limite do desenho da replicação — é um limite de onde a trava é tomada.

### Modo C — Spare, e a morte do primário

O spare recusa `varrer` e `inserir` com `SPARE_EM_ESPERA` (4004), e deixa
passar `ping` e `checksum` — o monitoramento continua enxergando. Então
`docker kill` no primário (não `stop`: sem aviso e sem chance de fechar
arquivo), e a promoção com ele **realmente fora do ar**:

| | |
|---|---:|
| primário morto, conexão recusada | sim |
| `spare_promover` | **5 ms** |
| papel depois | `source` |
| `varrer` e `inserir` depois | passam |
| soma do primário antes de morrer × soma do promovido | 200 linhas → 201 (a que o teste inseriu depois) |

### Modo D — Read Replica

500 linhas alcançadas em **0,30 s**, leitura devolvendo as 500, escrita
recusada com `REDIRECIONA` **4003**, e soma do servidor idêntica nos dois
lados. O endereço do redirecionamento é o achado 3, acima.

### O estágio (e) — a §7, medida

| tranca no source | eventos que o intruso levou |
|---|---:|
| nenhuma | **200 de 200** |
| `replicas_autorizadas` (antes do conserto) | **200 de 200** |
| `replicas_autorizadas` (hoje) | 0 |
| `ips_permitidos` | 0 |
| `iptables` da §7 | nem abre a porta (*timeout*, não recusa) |

Com as regras da §7 no namespace do source — entrada TCP 5000 só do IP da
réplica (mais o gateway, que é a estação do administrador), saída só
`ESTABLISHED,RELATED` — as três coisas foram medidas juntas: **o intruso leva
timeout**, **a réplica autorizada continua replicando** e **o source não
consegue abrir conexão para ninguém**. A metade de saída do desenho nunca
tinha sido provada, e é a que mais importa: ela é o que faz o source não ser
uma ponte para dentro da rede dele.

Duas camadas apareceram sem ninguém pedir: **86 linhas** com o IP do intruso
no `acessos.log`, e o IP **na lista negra** do source ao fim da fase.

### Aprendizados — inclusive os que não deram em nada

1. **A hipótese do congelamento morreu na primeira montagem, e a segunda a
   ressuscitou — porque o cenário é que estava errado, não a hipótese.** O
   primeiro estágio (a3) cortou a rede com o **source parado** e não congelou
   nada: pior `ping` 8 ms, pior `varrer` 8 ms, em 107.365 amostras. Com o
   source parado a réplica passa a vida no `ligar`, que acontece **fora** da
   trava. Ela só entra na trava quando há evento para puxar. Repetindo com o
   source escrevendo sem parar: `varrer` em **29.456 ms**. *Cenário que não
   exercita o caminho mede o caminho errado* — e o «8 ms» teria arquivado um
   defeito real como inexistente, com número e tudo.
2. **E o mesmo aconteceu com o abraço, na direção contrária.** A primeira
   montagem do (b-abraco) escreveu 4.000 linhas de cada lado: convergiu em
   0,1 s, zero `EAGAIN`, nenhum travamento. A fila era pequena demais para as
   duas fases de puxar se sobreporem. Com 50.000 de cada lado e as duas cargas
   largando no mesmo instante (uma `Barrier`), o abraço aparece na primeira
   tentativa. **Duas montagens do mesmo estágio deram respostas opostas, e a
   diferença estava no tamanho da fila** — não na hipótese. Junto com o
   anterior, a regra: *cenário fraco não refuta hipótese, e o número que ele
   produz é o mais perigoso de todos, porque parece medição.*
3. **`docker network disconnect` não serve para cortar a rede numa bancada.**
   Ele leva junto a porta publicada, e a bancada fica cega justamente no
   momento em que precisa olhar os dois lados. A primeira versão do estágio da
   partição morreu com «o servidor fechou a conexão» ao tentar ler o nó
   desligado. `iptables` no namespace, contra o IP do outro, é cirúrgico e
   deixa os dois observáveis.
4. **O servidor fecha a conexão ociosa em `timeout_s` (30 s)**, e isso é
   razoável para um servidor e mortal para uma bancada que mede cortes de
   45 s. A primeira versão morria com `broken pipe` **depois** do corte, e o
   número saía como «não se recuperou» quando quem tinha ido embora era o
   cliente. Um teste que falha por engano é tão ruim quanto um que passa por
   engano.
5. **A lista negra sobrevive à corrida.** O intruso do estágio (e) acaba
   bloqueado — que é o certo —, e `blacklist.json` mora no volume: na corrida
   seguinte a fase **sem tranca nenhuma** mediu zero evento roubado. O número
   estava certo e a conclusão seria errada. Estado que persiste entre corridas
   é uma armadilha de bancada tão grande quanto binário velho.
6. **A imagem oficial e a da bancada são duas de propósito.** A oficial
   compila dentro do contêiner com `--offline`, que é o que prova a promessa
   de zero dependência; a da bancada carrega o binário musl da máquina, porque
   sobe e derruba dez contêineres por corrida. Mesmo alvo, mesmo `scratch`.

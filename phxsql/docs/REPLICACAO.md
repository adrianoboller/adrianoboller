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

### 5.1 E a alteração confere o CARIMBO, porque não gera rowid nenhum

A conferência de cima só cabe na inclusão: é ela que **gera** um rowid aqui
para comparar com o de lá. A alteração não gera nada — grava no rowid que o
evento mandou —, e até 23/09/2026 gravava **calada** (pedido 405). Medido no
dia: a alteração de uma segunda origem devolvia `Ok(1)` e a linha de um caixa
virava a de outro, sem um erro.

Hoje ela compara o **carimbo de criação** (`rowstamp`, PSCH v10) que a imagem
traz com o da linha que mora naquele rowid. Se forem dois números diferentes,
são duas linhas diferentes, e a replicação para com a mesma família de
mensagem da inclusão — nomeando a tabela, o rowid, o carimbo que veio e o que
está aqui.

**Por que o carimbo, e não a chave nem o `rownum`.** O carimbo é a única
coluna que cumpre as três condições: viaja na imagem, a réplica a **honra** na
inclusão, e nenhuma alteração a renova dos dois lados. A chave não serve
porque a imagem é o **depois**: uma alteração que muda a chave é legítima e
seria recusada. O `rownum` também não, porque ele se preenche *localmente* na
primeira alteração de uma linha nascida antes da coluna — e aí os dois lados
divergem sem ninguém ter errado.

**O alcance, que é menor do que o nome promete.** O carimbo é um contador do
**processo**, não um identificador de nó: dois servidores podem emitir o mesmo
número. Isto **pega** divergência, não **prova** acordo — exatamente como a
conferência de rowid da inclusão. E quando qualquer um dos lados traz zero
(«nasceu antes da coluna»), não há identidade para comparar e a conferência
sai de cena, em vez de recusar a replicação de uma tabela migrada. É por isso
que a guarda da declaração (pedido 406, §8.1) continua sendo o que sustenta o
arranjo: esta aqui é a rede embaixo, não a porta.

**O que ela NÃO alcança, e está medido:** a **exclusão** replicada. O evento
de exclusão não leva imagem — o rowid basta —, então não há carimbo para
comparar, e uma exclusão de outra origem continua apagando a linha errada em
silêncio. Sem mudar o formato do evento, esse caminho não tem conferência
possível.

**Custo:** zero mensurável. A conferência acontece **dentro** do `atualizar`,
onde o payload antigo já está lido; feita de fora custaria uma segunda leitura
do mesmo slot, medida em **+14,7%** no laço (21,49 → 24,65 µs/evento, faixas
que não se cruzam). Do jeito que ficou, o A/B intercalado de cinco pares com o
interruptor dá 22,18–23,14 µs sem e 22,09–24,60 µs com — faixas que se cruzam,
e por isso não se declara diferença. Tudo em `debug`.

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

**Cada evento carrega a própria `posicao` no diário do source** — desde
17/09/2026 (pedido 292, parte 1). É o nosso equivalente do LSN que o
`ALTER SUBSCRIPTION … SKIP (lsn)` do PostgreSQL recebe, e ele vem do source
porque quem puxa **não consegue contar**: no bidirecional o source suprime os
eventos cuja origem é quem pede e a posição anda por cima deles, então
`desde + índice_na_lista` dá um número menor que o verdadeiro. Um source que
não mande o campo continua funcionando — só não dá para usar o
`replicacao_pular` naquela tabela, e a operação **recusa dizendo isso** em vez
de adivinhar e descartar um evento que ninguém olhou.

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

**E isso custou um furo, que hoje está fechado.** A bateria
`bancada/seguranca/porta.py` (caso 4d-ii, 07/09/2026) mediu um **Source** posto
em `somente_leitura` pela tela: o `inserir` recusou com «servidor em modo
somente leitura» e o `aplicar` **gravou** a mesma linha na mesma sessão, com a
imagem que o `replicar` acabara de entregar. Quem tem o token escreve numa base
que o dono declarou fechada.

O portão **2b-bis** fecha isso pelo **papel**, e não pela lista de réplicas nem
pela existência de origens: papel é a declaração de para que este servidor
serve. `replica`, `read_replica`, `spare` e `multi` continuam aceitando
`aplicar` — byte a byte como antes; `source` e `isolado` recusam.

*Medir a premissa do item vem antes de implementar o item.* Antes de decidir,
esta casa mediu se a réplica legítima usa a op: um source e uma réplica de pé
(papel `replica`, `somente_leitura` ligado, 200 linhas alcançadas 200/200), e a
op `aplicar` foi chamada **zero** vezes nos dois `acessos.log` — o laço da
réplica puxa e aplica **por dentro**, com `Table::aplicar_evento`, e nunca pelo
protocolo. Quem chama `aplicar` pela rede é um empurrão de fora, e o único
servidor que tem motivo para aceitá-lo trancado é o que existe para receber
replicação.

**O crivo passou a valer ABERTO ou trancado — desde 17/09/2026 (revisão SEC,
A3; pedido 280; commit `49a3af7`).** Até então ele só rodava com
`somente_leitura` ligado, porque nasceu do caso 4d-ii da bateria (um source
**trancado**). Mas `aplicar` grava com `Table::aplicar_evento`, que desliga de
propósito o julgamento de integridade — sem FK, sem CHECK, sem cascata, porque
a garantia é da origem e conferir na réplica perdia dado nos três
ordenamentos. Num source **aberto** — que é o source de produção, por
definição — quem tinha `administrar` na tabela apagava pela rede o pai com
filhos, contornando a regra primordial da integridade por uma operação que a
declaração da tabela nunca viu. Medido antes de mexer: nenhum chamador
legítimo empurra `aplicar` num source ou isolado (as bancadas de quórum
empurram em réplicas e cluster). A mensagem trocou: a chave velha, que dizia
«servidor em modo somente leitura», saiu; a nova (`erro.aplicar_fora_de_replica`)
nomeia o papel do servidor, aberto ou trancado.

### O `aplicar_evento` ganhou um segundo dono: o PITR

Desde 08/09/2026 o `Table::aplicar_evento` não é mais só da replicação. A
restauração a um instante (`docs/RESTAURACAO.md` § 7) reaplica o diário vivo
sobre a cópia recém-restaurada **pelo mesmo método**, com a mesma marca de
`como_replica` — e isso foi escolha, não coincidência: *o segundo caminho é o
que um dia esquece uma conferência*.

Consequência a escrever antes que alguém a descubra pelo defeito: **quem mexer
no `aplicar_evento` mexe nos dois**. A guarda do rowid, a recusa do evento sem
imagem e o `julga_integridade` calado têm agora dois chamadores, e os testes
que os travam moram em lugares diferentes — os da réplica em
`phxsql-store/tests/replicacao*.rs`, os do PITR em `servidor.rs::testes_pitr`.
Rodar só um dos dois conjuntos e achar que provou o método é o engano que esta
nota existe para impedir.

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

### Lista vazia continua liberando — e agora se anuncia

O padrão é a lista vazia, e ele **não** mudou: fechar de fábrica quebraria toda
replicação montada sem a lista, que é a maioria. O que mudou é o **silêncio**,
e o motivo tem número: a bateria `bancada/seguranca/porta.py` (caso 4d-i,
07/09/2026) mediu um servidor de **fábrica** — papel `isolado`, imagem
desligada, ninguém configurou replicação nenhuma — entregando o diário a quem
só tinha o token. Quem opera não tinha como saber.

Com a lista vazia, o servidor passa a dizer isso em dois lugares:

```
ATENCAO: replicacao.replicas_autorizadas esta VAZIA -- `posicao`, `replicar` e
`aplicar` atendem QUALQUER endereco que tenha o token, e com
replicacao.imagem_da_linha ligada o `replicar` entrega a LINHA INTEIRA. Para
fechar: liste os IPs das replicas em replicacao.replicas_autorizadas.
```

e no `config` do protocolo, **estruturado** e não em prosa — a tela monta a
frase pela fábrica de idiomas, porque *rótulo se traduz, dado nunca*:

```json
"replicacao_aberta": {"aberta": true, "com_imagem_da_linha": true,
                      "ops": ["posicao","replicar","aplicar"]}
```

O campo **some** quando a lista está preenchida, em vez de voltar `false`: quem
lê não pode confundir «este servidor está fechado» com «este servidor é velho e
não sabe responder». E o aviso **não olha o papel** de propósito — as três
operações respondem em qualquer papel, e condicionar o aviso a `source` calaria
justamente o servidor que ninguém configurou para replicar e replica assim
mesmo.

**A lista de operações que este portão tranca cresceu para quatro em
17/09/2026 (revisão SEC, A1; pedido 278; commit `49a3af7`).** `cluster_pulso`
usa a mesma credencial de `replicar` e decide quem manda no cluster — época,
posição e papel de cada nó saem dele —, e não estava em
`OPS_DE_REPLICACAO` (`servidor.rs:320`): um pulso forjado passava pela lista
sem ela olhar. Hoje `OPS_DE_REPLICACAO` é `["posicao", "replicar", "aplicar",
"cluster_pulso"]`. **Consequência para quem opera um cluster com a lista
preenchida**: ela precisa trazer **todos os outros nós**, não só os clientes
externos — cada nó pulsa para cada outro, e um nó que falte na lista de outro
tem o próprio pulso barrado, o que degrada o cluster sem um aviso próprio (o
aviso de `replicacao_aberta` acima é sobre `posicao`/`replicar`/`aplicar`, não
sobre o pulso). Ver `docs/CLUSTER.md` §2.2.

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

### 8.1 Duas origens não entregam o mesmo nome de database

**A regra:** num servidor multi-source, cada nome de database tem **uma** origem
dona. Duas origens entregando `vendas` escreveriam no **mesmo** `vendas` daqui,
e a réplica fiel aplica **por rowid**: no segundo evento da segunda origem os
rowids divergem. A inclusão para com *fail-stop*; a **alteração** parou de
sobrescrever calada em 23/09/2026 — ela confere o carimbo de criação (§5.1,
pedido 405) —, e a **exclusão** continua sem conferência possível, porque o
evento dela não leva imagem. Era o arranjo que se destruía sozinho no meio do
expediente, e passou a morrer na declaração (pedido 406, parecer do papel C de
23/09/2026 §6). A guarda da declaração é a porta; a do carimbo é a rede
embaixo dela.

**Onde a recusa acontece, e por quê são dois lugares:**

| caso | quem decide | o que acontece |
|---|---|---|
| duas origens com o **mesmo nome** | `Config::validar()` | o servidor **não sobe**, nomeando posição e `host:porta` das duas |
| duas listas **declaradas** se cruzam | `Config::validar()` | o servidor **não sobe**, nomeando as duas origens e o database |
| duas listas **vazias** | `Config::validar()` | o servidor **não sobe**: o arranjo é indecidível, e no máximo **uma** origem pode ficar sem lista |
| uma lista declarada contra uma **vazia** | a descoberta | aquele database é recusado **para a origem de lista vazia**; os outros dela andam |

Lista vazia quer dizer «todos os databases daquela origem», e quais são eles
**só a origem sabe**. Por isso o último caso é *indecidível* no arranque:
recusá-lo ali seria palpite — e o palpite tiraria do ar o próprio
`Config_exemplo_03.json`, que traz `curitiba ["Z"]`, `saopaulo` (vazia) e
`bruxelas ["W"]` e é arranjo legítimo. Medido pelo próprio `validar()`
(`os_config_de_exemplo_do_repositorio_continuam_subindo`): os **quatro**
`config.json` que o repositório entrega continuam subindo — **zero** quebrados
pelas três recusas.

**Duas listas vazias são indecidíveis, e isso é motivo para recusar, não para
sortear.** O dono de um nome que as duas entreguem seria a thread que
conectasse primeiro — outro a cada arranque, por latência de rede. Deixar a
descoberta escolher trocaria uma falha *cedo* (as duas escrevendo, fail-stop na
inclusão) por uma falha **intermitente**, que é mais difícil de diagnosticar, e
não mais fácil. A recusa nomeia as duas origens e diz o que resolve: declarar
`databases` em todas menos uma.

**Nome de origem repetido desliga a guarda inteira, e não precisa de malícia.**
O campo `"nome"` é opcional e o padrão é `"origem"`: duas origens que apenas
**omitem** o campo nascem as duas `"origem"`, e aí a descoberta compara
`dono.origem == origem`, acha verdadeiro para as duas e deixa as duas passarem.
O nome é a mesma identidade de `replicacao_estado`, `replicacao_pular` e
`replicacao_ligar` — nas quatro a segunda se faz passar pela primeira. Por isso
`"nome"` passou a ser **obrigatório quando há mais de uma origem**, e a recusa
nomeia posição e `host:porta`, que é o que o nome repetido não distingue.

**Lista declarada ganha de lista vazia**, e ganha *antes* de qualquer laço subir:
as declaradas reivindicam seus nomes na ordem do `config.json`. Sem isso o
vencedor seria a thread que chegasse primeiro, e poderia ser outra a cada
arranque — o database local receberia linhas de um source hoje e de outro
amanhã, que é exatamente a divergência de rowid que a guarda existe para
impedir. Entre uma escolha escrita e um curinga, ganha a escolha.

**A recusa é daquele database, nunca da rodada.** Uma origem repetida não pode
derrubar as dezenove certas: o nome repetido sai da lista, o resto continua, e o
motivo aparece uma vez no log e fica em `replicacao_estado` → `origens` →
`recusas`, nomeando a origem dona e o database.

**Onde a guarda NÃO liga, e é de propósito:** com o bloco `cluster`, com um
papel que não puxa, e com **uma origem só** (o par 1↔1, que não cruza com
ninguém). O cluster é o caso que exige cuidado: ali quem puxa é um laço só, do
master **corrente**, e o nome da origem muda a cada eleição (`cluster:<id>`) —
um dono guardado por nome recusaria ao master novo o database do master velho,
e seria a promoção inteira parando.

**E os dois níveis desligam pelo MESMO crivo**, que mora num lugar só:
`Config::puxa_de_varias_origens()`. O primeiro corte desta guarda tinha a
condição escrita duas vezes — o nível dinâmico desligava com `cluster` e o
estático **não** —, e o efeito era uma regressão: um nó com bloco `cluster` e
`replicacao.origens` sobrando de antes (a lista que o próprio servidor avisa
que **ignora**) deixava de subir. Pior, `validar()` também roda no
`gravar_a_arvore`: a tela de configuração passava a recusar gravar o mesmo
arquivo que estava no disco, e o operador perdia as duas saídas no mesmo
upgrade. Duas cópias de uma condição é um nível ligado onde o outro está
desligado, e o teste que parecia cobrir o caso (`com_cluster_a_guarda_nao_liga`)
não cobria: ele montava o config e **nunca chamava** `validar()`. Hoje chama.

**A consequência de modelagem**, que é o motivo de a guarda existir: no arranjo
**espelho** — 20 caixas com 20 nomes de database, `caixa01`…`caixa20`, um
central em `somente_leitura` puxando dos 20 — cada `.reg` do central tem
**exatamente um escritor**, os rowids batem por construção e a ordem de digitação
de cada caixa é preservada byte a byte. No arranjo **consolidado**, os 20 no
mesmo nome, ela se perde. A guarda é o que separa os dois na hora de subir.

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
servidores (NTP) — e confia no par**. Sem NTP, o lado com o relógio adiantado
vence sempre — toda escrita dele parece «mais recente», e o trabalho do outro
lado é desfeito em silêncio. **Isso é a metade acidental.** A metade
adversária, achada na revisão SEC de 17/09/2026 (A9, pedido 286): um par que
**mente** o carimbo — não um relógio que deriva, um `carimbo_ms` forjado —
fixava o `Toque` local num valor que nenhuma escrita local jamais alcançava, e
passava a sobrescrever calado toda alteração deste lado, para sempre. Fechado
no mesmo dia (commit `eeb9925`): carimbo mais que `FOLGA_DO_CARIMBO_MS`
(**5 minutos**) no futuro entra com o **relógio local**, e a ocorrência é
contada em `replicacao_estado.carimbos_do_futuro` — sem recusar o evento, que
pararia o par por um campo que o desempate nem precisa levar a sério. Empate
de carimbo desempata pela **origem numérica maior**: arbitrário,
determinístico e igual dos dois lados, que é o que faz os dois convergirem
(exatamente um aplica, o outro descarta).

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

### O índice único SECUNDÁRIO: a recusa é contada, e o laço segue

O casamento usa **uma** chave; a unicidade dos **outros** índices continua
sendo conferida na gravação — e está certo que continue, porque violação de
índice único não se cura quando o próximo lote chega, ao contrário da chave
estrangeira. Com primária `porId` e um secundário `porEmail`, o evento do
outro lado que traz um e-mail já ocupado aqui é recusado por `porEmail`.

Essa recusa **parava o par de servidores** (pedido 292): ela subia pelo `?` do
laço, a posição consumida nunca andava e o **mesmo lote voltava para sempre** —
medido em `tests/laco-do-unico-secundario.rs` com o defeito reposto:
**20 repetições do mesmo erro em 20 s**, e a linha seguinte do diário, que nada
tinha a ver com o conflito, nunca chegou. Não é uma linha perdida: é a
replicação parada, sem ninguém saber.

Hoje ela é **contada e gritada**, no mesmo desenho do `colisoes_de_sequencia`
(pedido 229(a)) e do `carimbos_do_futuro` (A9): a linha do outro lado **não
entra**, o número aparece por tabela em `replicacao_estado`, uma linha vai ao
log do processo nomeando a chave e o índice, e o laço **segue** — inclusive
para as linhas seguintes do mesmo lote.

```json
"recusas_por_unicidade": {"loja/clientes": 1}
```

Só tabela com contagem aparece; instalação sã responde com o objeto vazio. O
erro que **não** é duplicidade continua subindo e parando a rodada — disco,
imagem corrompida e trava envenenada são o comportamento de sempre.

O que isto **não** resolve: as duas linhas continuam existindo, cada uma no
seu servidor, e os dois lados divergem naquela chave até alguém arrumar o
dado. Casar por N chaves — escolher vencedor por chave secundária — é
semântica nova de conflito e tem pedido próprio.

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
| ☑️ | **Espera crescente na reconexão** — só para falha de REDE: dobra a partir do `reconectar_em` até 60 s, e zera na primeira rodada boa. Medido pelo soquete: 21 conexões em 20 s viraram 5 (1, 2, 4, 8 s). §20 |
| ☑️ | **Credencial recusada pela origem ESTACIONA o laço** (pedido 203) — uma tentativa, e o laço só volta por `replicacao_ligar` ou reinício. Antes: 75 tentativas/min, o master bloqueava o IP na quinta, em 4 s, por 60 min, e derrubava o operador do mesmo endereço. §20 |
| ☑️ | **Cifra do fio no transporte, e ela NASCE ligada** (18/09/2026) — aperto de mão estilo Noise (X25519 + HKDF + ChaCha20-Poly1305). `replicacao.origens[].cifra` nasce `true`, e `"cifra": false` é o escape escrito de quem replica de um source anterior ao aperto. O pino continua sendo pedido (`"chave_do_fio"`) e **não** nasce: sem ele o túnel protege só de escuta passiva, e isso o arranque diz. Não é TLS. Ver [CIFRA-DO-FIO.md](CIFRA-DO-FIO.md) §13 |
| ☑️ | **O pulso do CLUSTER vai cifrado junto com a replicação** — um interruptor só (`cluster.cifra`, que também nasce ligado), porque cifrar metade do tráfego do cluster é pior que não cifrar nenhuma: parece protegido. As duas metades têm guarda própria (`pulso-do-cluster-em-claro` e `replicacao-do-cluster-em-claro`). [CIFRA-DO-FIO.md](CIFRA-DO-FIO.md) §12 |
| ☑️ | **`replicas_autorizadas` passou a ser lido** — era campo sem leitor até a bancada de contêiner medir 200 de 200 eventos vazando com a lista preenchida (§7) |
| ☐ | **Buscar o lote FORA da trava de dados.** Medido (§17): `varrer` esperou **30,7 s** numa réplica cortada em silêncio, e no bidirecional os dois lados se trancam por 30 s com a rede sã. É o item 2 da §3.2 do `PENDENCIAS.md` visto de dentro da replicação, e é a causa; a espera crescente e o `connect` com prazo tratam o sintoma |
| ☐ | **O endereço do `REDIRECIONA` é o da origem configurada**, que é «por onde *eu* alcanço o primário» e nem sempre «por onde *você* alcança» (§17, achado 3). Um campo próprio para o endereço que se anuncia ao cliente resolveria |
| ☐ | **`replicacao_estado` não conta nada durante um corte silencioso** — `ultima_rodada` fica com o carimbo de antes e `ultimo_erro` fica nulo, porque o laço está pendurado. Falta um «quando foi a última rodada BEM-SUCEDIDA» que envelheça sozinho, para o monitoramento distinguir «nada a replicar» de «cego» |
| ☑️ | **A réplica confere a continuidade do diário do source, como o PITR já fazia** (pedido 295; commit `49a3af7`, 17/09/2026) — `alcancar_tabela` deixou de devolver `Ok(0)` em silêncio quando o source apagou e recriou a tabela; ela compara o evento `posição-1` do source contra o seu (`diario_local_continua`, irmã de `diario_vivo_continua`), sem ida e volta a mais no caminho quente. Rompida, grava a recusa em `replicacao_estado.origens.<origem>.recusas["banco/tabela"]` e a tabela sai da rodada sem derrubar as outras. **Não entrou no bidirecional** (`alcancar_tabela_bidi`) — decisão do dono, nomeada e não tomada |
| ☑️ | **A réplica fiel grava o carimbo e a origem do source no próprio diário**, e não mais `agora_ms()`/`origem:0` (`servidor.rs:2831`, `forcar_proximo_evento`; commit `49a3af7`) — o mesmo que o PITR e o bidirecional já faziam (parecer do DBA, §2.3, 17/09/2026). É o que faz a conferência de continuidade acima valer: comparar `posição-1` só funciona se o carimbo/origem gravados aqui forem os mesmos que o source gravou lá |

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
  marca de posição, cada réplica aplica **17.450 eventos/s**.

  > **ERRO DE ARITMÉTICA, CORRIGIDO EM 17/09/2026 — e o texto errado fica
  > aqui, porque apagá-lo esconderia como ele passou.** Este parágrafo dizia:
  > *«as três juntas ~52.000 — mais do que os 34.048 que o master escreve»*, e
  > concluía que o conjunto acompanha a origem. **Não acompanha.** Somar a
  > vazão de três réplicas só valeria se o trabalho fosse **particionado**
  > entre elas; réplica completa recebe **todos** os eventos, não um terço. No
  > regime sustentado, cada réplica acumula **34.048 − 17.450 = 16.598
  > eventos/s de atraso**. Achado por parecer técnico externo (17/09/2026), e
  > o agravante é o pior possível para esta casa: os números estavam
  > **medidos** e a conta em cima deles estava errada. *Número citado é número
  > que não se mede* pega o número; **não pega a aritmética.** Nenhuma das
  > seis revisões anteriores olhou a conta. O que continua
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
# a trava de dados contra a leitura de rede, nas portas 7050-7055
# (~1,5 min; e a versao de loopback dos estagios (a3) e (b-abraco) da
# bancada de conteiner -- ver a §18):
python3 bancada/replicacao/trava.py
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

**Achado 2 — a trava de dados fica presa atrás de uma leitura de rede.**
*(Consertado — a §18 conta o conserto, com os números dos dois lados. O que
vem abaixo é o retrato do defeito, e ele fica porque é ele que explica por que
a §18 existe.)* O laço da réplica segurava `self.dados.lock()`
(`alcancar_tabela`, e o mesmo em `alcancar_tabela_bidi`) e, **de dentro dela**,
fazia a ida e volta de rede que busca o lote. Numa rede sã isso é invisível: a resposta chega em
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

**Achado 2b — no bidirecional, os dois lados se trancam um ao outro.**
*(Consertado junto, na §18.)* É o mesmo mecanismo levado ao pior caso, e ele
**não precisa de corte nenhum**.
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

*(Consertado. Refeito no contêiner em 05/09/2026: **3,6 s**, `EAGAIN` **0 e
0**, pior `checksum` 697 ms. A tabela dos dois lados está no fim da §18.)*

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
7. **Estágio que afirma o DEFEITO vira catraca contra quem o conserta**
   *(05/09/2026)*. O veredito do (a3) era `pior_varrer > 5_000`: ele nasceu
   para **achar** a trava presa e só passava com ela presa. Consertado o
   defeito, o `varrer` respondeu em 7 ms e o estágio saiu **reprovando o
   conserto** — e, o pior, sem que ninguém visse, porque o Docker esteve fora
   do ar entre o conserto e esta corrida. A regra que sai: *o estágio que acha
   um defeito, no dia em que ele for consertado, tem de passar a afirmar a
   garantia* — e o teto vem do mesmo lugar da bancada irmã, não de uma segunda
   cópia do número.

---

## 18. A trava de dados saiu de trás da rede

A §17 deixou dois achados abertos, e eles eram o mesmo defeito visto de dois
ângulos: **o laço da réplica segurava a trava global de dados enquanto lia do
soquete**. Esta seção é o conserto, com os números antes e depois.

### O ponto exato

`alcancar_tabela` tomava a trava na **primeira linha** e a segurava até o fim
da função — e dentro do laço mora `replica::puxar`, que é uma ida e volta de
rede. `alcancar_tabela_bidi` fazia o mesmo, com dois agravantes: os **dois**
lados de um par bidirecional rodam esse laço, e ela tomava `self.dados.lock()`
**cru**, sem passar pelo `travar_dados()` — que é o ponto único que cronometra
a trava. Ou seja: o pior dos dois caminhos era justamente o que a telemetria
**não conseguia ver**. Ponto de medição que pula um chamador mente do mesmo
jeito que campo de configuração que ninguém lê.

O prazo que soltava a trava é o `set_read_timeout(30 s)` de `replica::montar`.
Por isso o número do defeito é sempre ~30 s: não é uma lentidão, é um relógio.

### As duas testemunhas, e o que elas mediram

A bancada nova é `bancada/replicacao/trava.py`: quatro estágios, ~1,5 min,
portas 7050-7055, sem Docker. Ela é a versão de **loopback** dos estágios
`a3-congelamento` e `b-abraco` da §17 — no lugar do cabo cortado há um **tubo**
em Python entre a réplica e o source, que repassa byte a byte até mandarem
emudecer e a partir daí segura os dois soquetes abertos sem repassar nada. Do
ponto de vista da réplica é o mesmo silêncio de um `iptables -j DROP`.

O que o loopback **não** substitui continua sendo queda de processo e partição
de rede de verdade; para essas vale a bancada de contêiner da §17.

Duas testemunhas, e elas têm de contar a mesma história:

- **de fora** — um cliente cronometrando `ping`, que não toca na trava, contra
  `varrer`, que precisa dela. Se os dois travassem, o servidor estaria fora do
  ar e o problema seria outro; só o segundo travar é o que aponta a trava;
- **de dentro** — `totais.trava_ms` da telemetria da própria réplica, que só
  existe porque `travar_dados()` é o ponto único.

Com o source escrevendo sem parar e o tubo emudecido por 40 s:

| na réplica | antes | depois |
|---|---:|---:|
| pior `ping` | 4 ms | 5 ms |
| **pior `varrer`** | **30.079 ms** | **6 ms** |
| trava na mão (telemetria) | 35,8 s de 40 | 11,4 s de 40 |

O contêiner tinha medido 29.456 ms contra `ping` de 6 ms. O loopback reproduz
o mesmo por outro caminho, o que confirma que o mecanismo nunca dependeu do
Docker — o Docker só foi o primeiro lugar onde alguém conseguiu cortar a rede.

As duas colunas de telemetria **não são comparáveis**, e vale dizer por quê: a
sonda passou de 52.650 para 231.385 idas e voltas na mesma janela, porque
depois do conserto ela consegue perguntar cinco vezes mais. Os 11,4 s são
quase todos trabalho dela. Quem compara é o `varrer`.

### O abraço do bidirecional

200.000 linhas, metade escrita em cada lado ao mesmo tempo, rede sã, com um
cliente sondando alfa **durante** a carga:

| | antes | depois |
|---|---:|---:|
| escrita do cliente | 33,0 s | **1,7 s** |
| as mesmas 200.000 num servidor sozinho | 2,4 s | 2,4 s |
| razão | **14,0×** | **0,71×** |
| `EAGAIN` novos no diário | +1 alfa, +1 beta | 0 e 0 |
| **pior `varrer` durante a carga** | **31.375 ms** | **67 ms** |

Com 1.000.000 de linhas o antes fica ainda mais feio — **240,7 s contra
12,1 s, 19,8×, com sete `EAGAIN` de cada lado**. Sete e sete: a mesma
assinatura simétrica que a §17 tinha visto nos contêineres, em outra máquina e
por outro caminho. Contagem igual dos dois lados é o que só um abraço produz.

### O conserto: três fases

`alcancar_tabela` e `alcancar_tabela_bidi` estão partidas em três, e as duas
funções auxiliares de cada uma existem só para que a trava tenha começo e fim
visíveis:

1. **com a trava** — garantir o database, abrir (ou criar) a tabela, ler a
   posição local; soltar;
2. **sem a trava** — ler o lote inteiro do soquete;
3. **com a trava** — reabrir a tabela, **reler a posição**, aplicar, soltar.

A regra geral, e ela vale para qualquer coisa que se acrescente aqui:
**nenhuma leitura de rede acontece com a trava de dados na mão.** O `posicao`
do começo da rodada já era assim; o `puxar` passou a ser. Sobra o `connect` do
`ligar`, que também é fora da trava — e é por isso que ele continua sem prazo
de conexão, com a nota que já estava em `replica.rs`.

A releitura da posição na fase 3 não é enfeite. Entre a fase 2 e a 3 a trava
esteve solta, e aplicar um lote pedido a partir de outra posição gravaria o
evento errado no rowid errado. Quando a posição andou, o lote é **descartado**
e o laço pede de novo a partir de onde a tabela está agora: descartar custa uma
ida e volta, aplicar torto custaria o dado.

### O teto de memória, que passou a ser obrigatório

Enquanto o lote era lido **com** a trava na mão, o tamanho dele era o menor dos
problemas. Agora ele mora inteiro na memória da réplica até a trava chegar, e
«quanto isso pode crescer» virou pergunta com resposta obrigatória — e a
resposta não podia ser «o que o outro lado mandar».

- **no source**, `TETO_DO_LOTE_SERVIDO` = **16 MiB** de imagem por resposta. O
  `max` do pedido conta *eventos*, e evento não tem tamanho fixo: 500 linhas de
  60 bytes são 30 KiB, e 500 linhas com um memo de 200 KiB são 100 MiB — que em
  hexadecimal viram 200 MiB de texto montados de uma vez dos dois lados. Lote
  curto não perde nada, porque `ate` e `fim` saem do que foi realmente lido e a
  réplica só pergunta de novo. E o lote **nunca sai vazio** por causa do teto:
  o primeiro evento entra sempre, senão uma linha maior que o teto pararia a
  replicação para sempre em vez de atrasá-la. **Consertado em 17/09/2026
  (revisão SEC, A2; pedidos 279 e 303; commit `49a3af7`)**: até então o teto
  cortava a **resposta**, depois de o diário inteiro já ter sido lido com
  imagens para a RAM sob a trava global — um `"max":0` (ou negativo) lia tudo.
  Hoje `max` ausente, zero ou negativo vale o **padrão** (`LOTE_PADRAO_DE_REPLICACAO`
  = 500), o maior valor pedido é limitado a `TETO_DE_EVENTOS_POR_LOTE` = **5.000**,
  e o teto de bytes desceu para dentro de `Log::percorrer` (`log.rs:655`), que
  decide pelo tamanho do cabeçalho de cada evento **antes** de alocar a
  imagem — o teto agora limita a **leitura**, não só a resposta. Os irmãos que
  liam o diário inteiro receberam o mesmo tratamento: `op_diario` deixou de
  carregar tudo para descartar até sobrar a cauda, e `absorver_diario_local`
  passou a andar em lotes;
- **na réplica**, `TETO_DA_RESPOSTA` = **128 MiB** por linha lida — o dobro do
  que um par sadio produz, para que ele só sirva ao que existe: impedir que a
  réplica aloque sem limite por ordem de quem está do outro lado do fio.
  Estourou, a recusa é `LIMITE_EXCEDIDO` **com o número dentro**, e a conexão
  não se reaproveita (ela ficou no meio de uma linha) — a rodada volta e a
  próxima abre outra.

### A queda da conexão entre a leitura e a aplicação

O conserto abre uma janela que antes não existia: a conexão pode morrer com o
lote já na memória e ainda não gravado. As três saídas possíveis são perder o
lote, aplicá-lo duas vezes, ou aplicá-lo inteiro uma vez — e só a terceira
presta.

Na **réplica fiel** a posição nasce do diário daqui, então ela só anda depois
de o lote estar gravado: queda antes da fase 3 devolve exatamente o mesmo lote
na rodada seguinte. E não existe meio-lote, porque o lote inteiro chega antes
de a trava ser pedida. No **bidirecional** a posição consumida é estado próprio
e só é gravada depois da aplicação; repetir um lote ali é inofensivo, porque o
casamento é por chave e a regra é «mais recente vence» (§12).

Isso não se prova por teste unitário — é a lição do `BULKINSERT`. O estágio
`queda` da bancada nova prova por soquete: **10 cortes de conexão de verdade no
meio de um alcance de 200.000 eventos**, e a soma de verificação dos dois lados
fecha igual, com as mesmas linhas e os **mesmos slots** —
`('1aa1e8124df2cba0', 200000, 200000)` dos dois lados. O número de slots é o que
prova que os dois chegaram à mesma numeração sozinhos; contar linhas não acharia
uma que atravessou errada.

### O que o conserto custou: nada — e a primeira conta estava errada

Está medido em `DESEMPENHO.md` §4.13, e a conclusão em uma linha: partir em
fases obriga a abrir a tabela uma vez por lote, e isso não custa o que parecia.
O que custava era um `sincronizar()` que eu tinha deixado dentro da fase 3 —
**400 `fsync` num alcance de 200.000 eventos em vez de um**, com a trava na
mão. Com o `fsync` de volta ao fim do alcance, onde ele sempre esteve:

| 200.000 eventos, rede sã | eventos/s | pior `varrer` do cliente |
|---|---:|---:|
| antes do conserto | 67.406 | **2.727 ms** |
| conserto com `fsync` por lote | 55.433 | 292 ms |
| **conserto com `fsync` por alcance** | **68.000** | **76 ms** |

A linha do meio é o registro de uma hipótese que morreu: «reabrir a tabela por
lote custa 17% da vazão» era plausível, tinha número, e estava errada.

E a última coluna é o número que mais vale no dia a dia: durante um alcance
**de rotina, com a rede perfeitamente sã**, o cliente da réplica esperava
**2,7 segundos** por um `varrer`, porque a trava ficava presa pelo alcance
inteiro. Esse defeito não precisava de corte nenhum para machucar; só ninguém
tinha olhado.

### O que trava a regressão

- `crates/phxsql-server/tests/trava-atras-da-rede.rs` — por soquete, com um
  source de mentira que responde `posicao` e **emudece** no `replicar`. Duas
  provas no mesmo arquivo: a do defeito (`varrer` responde enquanto o source
  está mudo) e a do **comportamento velho** (`com_a_rede_sa_a_replica_conversa_
  e_o_servidor_atende`), sem a qual um conserto que quebrasse a replicação
  inteira passaria com louvor — laço que não replica nada também não segura
  trava nenhuma;
- cada sonda tem **prazo próprio de 8 s**, e isso não é detalhe: com o defeito
  reposto a sonda não falha, ela **pendura** por 30 s, e uma bateria que
  pendura não reprova ninguém, ela trava;
- a guarda `trava-atras-da-rede` no `bancada/guardas/catalogo.py`. Medido:
  **PROVADA**, 14,1 s com o defeito reposto contra 1,3 s com a árvore limpa, e
  a mensagem de reprovação já traz o diagnóstico pronto — *«`varrer` sem
  resposta em 8 s; o `ping`, que não precisa da trava, respondeu em 570 µs»*.

### A conta fechada no contêiner — 05/09/2026

Esta seção dizia que a bancada de contêiner da §17 **não foi refeita**, porque
o daemon do Docker desta máquina estava fora do ar. Ele voltou, e a bancada
rodou inteira: **15 estágios, todos verdes**, 8,1 min. Os dois estágios que
tinham medido o defeito lá agora medem o conserto, e é a segunda testemunha
que faltava:

| medida, no contêiner | com o defeito (30/08) | hoje (05/09) |
|---|---:|---:|
| `a3` pior `varrer` na réplica | **29.456 ms** | **6 ms** |
| `a3` pior `ping` (não precisa da trava) | 6 ms | 9 ms |
| `b-abraco` escrita nos dois lados | **33,3 s** | **3,6 s** |
| `b-abraco` `EAGAIN` novos no diário | 1 e 1 | **0 e 0** |
| `b-abraco` pior `checksum` | 1.997 ms | 697 ms |
| `b-particao` pior resposta da réplica | 29.456 ms (`varrer`) | 1.317 ms (`checksum`) |
| `b-cortes` retomada `DROP-20s` | **293,8 s** | **0,2 s** |

Os 293,8 s do `DROP-20s` são o achado que nenhum documento tinha registrado: a
réplica que perdia a rede com regra de descarte silencioso e prazo de 20 s
levava quase cinco minutos para voltar depois de a rede religar. Com a trava
fora do caminho da leitura, 0,2 s. As três regras de `REJECT` não mudaram —
recusa dá erro na hora, e a réplica sempre soube tratar erro na hora.

**A vazão não entra nesta tabela de propósito:** a máquina estava ocupada com
outra frente nas duas pontas da corrida (`bancada/esta-medindo.sh` acusou antes
e depois), então `fonte_linhas_s` e `replica_eventos_s` ficam nomeados e não
medidos. O que a carga da máquina não explica é uma diferença de 4.900× num
`varrer`.

**E o estágio `a3` reprovou o próprio conserto na primeira corrida.** A
afirmação dele era `pior_varrer > 5_000` — certa enquanto o defeito existia,
porque o estágio nasceu para **achar** a trava presa. Com a trava solta o
`varrer` respondeu em 7 ms e o veredito saiu `falha`. Hoje ele afirma a
garantia, com os mesmos tetos da bancada de loopback, e os tetos moram num
lugar só (`TETO_VARRER_MS`/`TETO_PING_MS` no `bancada/replicacao/trava.py`,
importados pelo `docker/provar.py`) — dois números iguais em dois arquivos são
um número que envelhece de um lado só.

**A imagem cresceu:** 6,42 → **8,22 MB** de camada (2,69 → 3,30 MB
comprimidos), que é o binário `musl` de hoje contra o de 30/08.

---

## 19. Transação com quórum — a pergunta, e o que a medição respondeu

Pergunta do dono, 07/09/2026: *«Transação com quórum, que permite replicar a
gravação para outros servidores sem usar replicação, apenas o quórum. E onde
existem os outros servidores, isso é possível? Simplifica? Ou fica esse
recurso na replicação?»*

### 19.1 Quórum não é alternativa à replicação — é uma regra sobre QUANDO responder

Esta é a parte que não depende de medição nenhuma, e por isso vem primeiro.
**Quórum conta confirmações.** Para haver o que contar, alguém tem de levar os
bytes até os outros servidores — e essa coisa que os leva **é** a replicação.
Quórum sem transporte não tem o que somar.

Então a resposta à primeira metade é: *não é possível* replicar por quórum sem
replicação, e não por limitação nossa. É o que quórum **é**. O que muda com
ele não é o caminho do dado; é o instante em que o cliente ouve «gravei».

### 19.2 Onde estão os outros servidores: eles já existem, e já votam

Esta é a boa notícia, e ela também não precisou de medição — precisou de
leitura do nosso próprio código. O `cluster.rs` já mantém o mapa dos nós, a
época e a **decisão de maioria**; a bancada do cluster mede eleição e promoção
com três nós.

O que falta é o alvo do voto. Hoje a maioria decide **quem é o master**, e não
**se uma escrita chegou** — e o módulo diz isso com todas as letras, no
cabeçalho, sem ninguém ter perguntado:

> *«Honestidade: isto NÃO é Raft. Não há log replicado por quórum de escrita: o
> master confirma a escrita sem esperar réplica nenhuma. A eleição por maioria
> impede DOIS masters duradouros, mas não impede a perda das últimas escritas
> de um master isolado: o que ele aceitou entre o início da partição e o
> momento em que se vê sem maioria não chegou a ninguém, e morre com o
> rebaixamento.»*

**Esse parágrafo é o buraco que um quórum de escrita fecharia.** Não é uma
funcionalidade a inventar: é uma perda já nomeada, esperando o mecanismo.

### 19.3 A premissa que precisava morrer antes de qualquer plano

A `bancada/replicacao/` publica um atraso de **826 a 2014 ms** por operação.
Lido como «o dado leva 826 ms para chegar na réplica», isso mataria a ideia
antes de começar: um commit síncrono a 826 ms é inviável.

**Não é o que aquele número mede.** Aquela bancada roda com
`reconectar_em: 2`, e o laço da réplica **dorme** esse tempo quando não acha
nada (`servidor.rs`, `Ok(0) => sleep(espera)`). O atraso publicado é, quase
todo, **sono** — e sono é escolha de configuração, não custo de transporte.

A `bancada/quorum/` separa as duas coisas. Ela sobe um master e duas réplicas
com `reconectar_em` de **uma hora** — para o laço delas não competir com a
medição — e cronometra o `replicar` + `aplicar` chamados **na hora**:

| | mediana | faixa |
|---|---:|---|
| gravar no master (o que se paga hoje) | **0,209 ms** | 0,171 – 3,357 |
| levar até UMA réplica, na hora | **0,475 ms** | 0,386 – 4,000 |
| commit esperando **2 de 3** | **0,661 ms** | **3,16×** |
| commit esperando **3 de 3** | **0,704 ms** | **3,37×** |

60 voltas, três processos em `127.0.0.1`. **O transporte custa 0,475 ms, e não
826.** O sono era 99,9% do número publicado.

E o aviso que viaja com a medida, porque sem ele ela mente: **está tudo em
localhost**. A rede real custa mais, e estes números são o **piso** do que um
quórum custaria. Numa LAN de ~0,3 ms de ida e volta, o termo dominante deixa
de ser o nosso motor e passa a ser a rede.

### 19.4 O obstáculo real não é o custo: é a DIREÇÃO

O medidor acima teve de puxar os eventos **à mão, do Python** — e isso não foi
comodidade de quem escreve a bancada. Foi a arquitetura falando.

A nossa replicação é **pull**, e está escrito no topo do `replica.rs`:

> *«Quem procura é a réplica; o source não empurra nada. É o mesmo desenho do
> MySQL®, e ele existe por causa do firewall: o source abre UMA porta de
> entrada para o IP da réplica, e não precisa alcançar a réplica de volta.»*

Um quórum síncrono exige o contrário: o master precisa saber, **no instante do
commit**, que N réplicas têm o dado. Com pull, o master não tem como fazer
ninguém buscar — ele só pode esperar que venham perguntar.

Daí as três rotas, e o preço de cada uma:

| rota | o que custa |
|---|---|
| **(a) manter pull, o master espera a réplica vir** | o commit passa a esperar o próximo ciclo do laço — os tais 826 ms. A pior das três: paga a latência e **não compra a garantia**, porque nada obriga a réplica a vir |
| **(b) canal aberto: a réplica conecta e FICA** | o master empurra por uma conexão que **a réplica** abriu. **Preserva o firewall** — quem abre continua sendo ela. É a rota certa |
| **(c) o master abre conexão com as réplicas** | simples de escrever, e **quebra** a propriedade de firewall que o desenho comprou. Recusável |

### 19.5 O que compra, o que custa, e onde o recurso fica

**Compra:** durabilidade que sobrevive à perda do disco do master, e o
fechamento do buraco que o `cluster.rs` confessa — as escritas que um master
isolado aceitou antes de se ver sem maioria.

**Custa disponibilidade, e isso é o oposto de simplificar.** Sem N réplicas
alcançáveis, o commit **falha**. Hoje ele aceita. Trocar «sempre aceita» por
«às vezes recusa» é decisão de produto, não de engenharia.

**Onde fica:** na **replicação**, como *política de commit* — nunca como
subsistema novo ao lado dela. E entra **pedida, não imposta**, pela mesma
pétrea da janela de conflito: quem não pedir quórum grava como hoje, e nenhum
cliente escrito antes para de funcionar.

**Correção da primeira redação desta seção, e ela vale mais que a frase que
substitui.** Eu havia escrito «política de commit **por origem**», e está
errado: `replicacao.origens` é a lista da **réplica** — de onde *ela* puxa.
Quem espera o quórum é o **master**, e do lado dele `origens` não existe. Do
lado do source há duas listas, e só uma serve:

| lista | o que é | serve de M? |
|---|---|---|
| `replicacao.replicas_autorizadas` | **IPs** autorizados a pedir o fluxo | **não** — é ACL, sem identidade nem saúde |
| `cluster.nos` | `id`, `endereco` e `porta` de **todos os nós, este incluído** | **sim** — e a maioria já é contada sobre ela |

Então o campo do quórum mora no bloco **`cluster`**, ao lado de `nos`, porque é
lá que o **M** já está declarado e a maioria já é apurada. Contar votos de
escrita sobre uma lista de IPs seria inventar uma segunda noção de membro, e
*lista que significa duas coisas* é defeito que esta casa já nomeou.

O corolário desagradável, e ele é decisão sua: **quórum de escrita passa a
exigir o bloco `cluster`.** Uma instalação com replicação simples e sem cluster
não teria onde declarar o M.

### 19.6 A armadilha que o Cassandra® já nos ensinou, medida no fonte deles

O `docs/CASSANDRA.md` registra, lido no fonte da 5.0.10: o `QUORUM` deles
**não** quer dizer «o dado está em N discos». No padrão
(`commitlog_sync: periodic`) quer dizer «N processos copiaram os bytes para um
`mmap`», com `fsync` a cada **10 segundos** numa thread de fundo.

Se este recurso nascer aqui, **o que o «ok» da réplica significa tem de ser
decidido e escrito**, não herdado: *recebeu*, *aplicou*, ou *aplicou e
sincronizou*? São três garantias diferentes com o mesmo nome, e a diferença
entre elas é exatamente a que separa «perdi um commit» de «não perdi».

### 19.7 A premissa do CANAL ABERTO, medida — e o que ela derrubou (07/09/2026)

O dono decidiu construir pela **rota do canal aberto**, e a linha do pedido
apontava o `bidirecional.rs` como onde essa rota mora. A frente F2 mediu antes
de implementar, e a medição partiu a frase em duas.

**`bidirecional.rs` não tem canal nenhum.** A primeira linha do cabeçalho dele
diz *«a parte funda da replicação bidirecional (multi-master), **sem rede**»*:
ele resolve conflito por carimbo e evita o laço pela origem no evento. Não abre
soquete, não conecta, não empurra.

**O canal aberto existe, e é o do PULSO.** O `laco_do_pulso`/`pulsar` abre uma
conexão de cada nó para cada outro e a mantém viva. Contados na telemetria
(`bancada/quorum/canal.py`), num cluster de três: **6 conexões longas**, e
**2 delas são do master para as réplicas** — abertas pelo master, já
autenticadas, já quentes.

E daí sai o achado que muda o desenho:

> **«O master não tem como fazer ninguém buscar» é verdade na REPLICAÇÃO e
> falsa no CLUSTER.**

Na replicação pura o desenho é *pull* por firewall. Num cluster, o próprio
pulso já obriga todo mundo a alcançar todo mundo — a propriedade de firewall
**já tinha sido gasta pelo cluster**, e não seria este pedido a gastá-la.

Medido, com conexões quentes, 60 voltas, tudo em `127.0.0.1` (é o **piso**):

| o que | mediana | faixa |
|---|---|---|
| piso do canal — `cluster_pulso` sem dado nenhum | **0,089 ms** | 0,069 – 0,232 |
| empurrar um evento pela conexão quente | **0,466 ms** | 0,337 – 34,988 |
| quórum 2-de-3 pelo canal | **0,447 ms** | — |
| quórum 3-de-3 pelo canal | **0,498 ms** | — |

**O que o piso diz:** o canal em si é barato — 5× menor que levar um evento. O
preço do quórum é o trabalho de **aplicar**, não o de **falar**, que é o
contrário do que se suporia. E ele explica a medição anterior: a `medir.py`
publicou `2-de-3 = 0,661 ms` abrindo o caminho a cada volta; pelo canal quente
são **0,447 ms**, e a diferença é o aperto de mão que o master **não pagaria**.

**Não foi implementado, e o motivo tem número.** Faltam quatro peças — a
primeira é decidir o que o «ok» da réplica significa (§19.6). O parecer inteiro,
com o caminho na ordem em que ele é testável, está em
`docs/propostas/quorum-de-escrita.md`. O que entrou foi só o **campo**
`cluster.quorum_minimo`, porque mudança de formato entra cedo, com o servidor
declarando `"quorum_imposto": false` ao lado dele — campo que finge efeito é
pior que campo ausente.

### 19.8 Como refazer

```bash
cargo build --release
python3 bancada/quorum/medir.py 60    # gravar / levar / quorum, conexao fria
python3 bancada/quorum/canal.py 60    # o canal aberto: quantos, e a que custo
```

Ele **para** — em vez de publicar um número bonito — se uma réplica puxar
sozinha (zero eventos no `replicar` significa que ela chegou antes, e o medidor
estaria medindo o próprio concorrente), e se as réplicas não alcançarem o
esquema antes da primeira volta.

## 20. A réplica que insistia na credencial recusada — e derrubava o operador junto

Pedido 203, filmado em 07/09/2026 e **medido pelo soquete em 09/09/2026**
(`bancada/replicacao/credencial-recusada.py`, contra o binário de antes do
conserto):

| | antes | depois |
|---|---|---|
| tentativas de login da réplica com `senha_hash` errado (`reconectar_em: 1`) | **5 em 4,0 s** — 75/min, até o bloqueio | **1**, e o laço estaciona |
| o master bloqueou o `127.0.0.1`? | sim, na 5ª, por **60 min**, «credencial invalida (login)» | não — `blacklist.json` vazio |
| conexões da réplica barradas na porta depois do bloqueio | 8 em 8 s (continuava batendo) | 0 |
| o operador do mesmo IP entra com a senha certa? | **não** — «bloqueado desde … até … por credencial invalida (login)» | sim |
| `replicacao_estado` diz por quê? | só `ultimo_erro` — o texto do bloqueio | `parada: "credencial_recusada"`, mais `ultimo_erro` |
| origem fora do ar (escuta que aceita e fecha) | 21 conexões em 20 s, intervalo fixo de 1 s | 5 conexões: **1, 2, 4, 8 s**, com `falhas_de_rede_seguidas` e `proxima_tentativa` publicados |
| 5 logins errados feitos pela própria bancada | bloqueia na 5ª | **bloqueia na 5ª** — a defesa do master não mudou |

Com o `reconectar_em` padrão de 10 s a conta é a mesma, só mais devagar: 6 por
minuto, bloqueio em ~40 s.

### O que estava errado, e onde

O laço tratava «a origem me recusou» como «a origem caiu»: dormia
`reconectar_em` e voltava. As duas têm a mesma cara no `Err` e são opostas na
natureza — a credencial recusada é **determinística** (a mesma prova contra o
mesmo hash dá a mesma resposta amanhã), e cada tentativa a mais só gasta a
tolerância de `tentativas_ate_bloquear` do master; a queda de rede é
transitória, e insistir nela é o trabalho da réplica.

**O bloqueio do master está certo e não mudou.** Distinguir «a mesma credencial
N vezes do mesmo processo» de «N credenciais diferentes» abriria a porta que
ele fecha: repetir o mesmo login com provas diferentes é exatamente a
assinatura de quem adivinha senha. O conserto é do lado de quem insistia.

### O desenho: três respostas para três falhas

`replica.rs` ganhou a classificação e a máquina de estados (`Falha`, `Ritmo`,
`Decisao`), puras e testadas sem thread:

- **`CredencialRecusada`** — `Autorizacao` vinda de `ligar` (token, login ou
  a própria porta barrada) → **estaciona**. Só sai por `replicacao_ligar` ou
  por reinício com a configuração corrigida. Por tempo, nunca: um laço que
  voltasse sozinho depois de uma hora seria o mesmo defeito em câmera lenta.
- **`Rede`** — `Io` (não conectou, ou caiu no meio) → **recuo exponencial**:
  `reconectar_em × 2^n`, teto de 60 s, sem nunca encurtar a base. O teto é
  baixo de propósito: uma conexão por minuto a um host morto não custa nada, e
  uma origem que volta depois de uma noite é encontrada em até um minuto.
- **`Outra`** — esquema, tabela recusada, corrompido → o intervalo fixo de
  sempre. Ninguém mediu que o recuo ajudaria aqui, e guarda nova entra pedida.

Só a fase de `ligar` classifica `Autorizacao` como credencial: um `replicar`
sem direito depois de entrar não conta como tentativa leve no master e se
corrige lá, sem religar aqui.

`replicacao_estado` publica `parada`, `religadas`, `falhas_de_rede_seguidas` e
`proxima_tentativa`. A operação nova **`replicacao_ligar`** (`administrar`)
deixa um pedido que o **laço** consome no passo seguinte dele, em até 1 s —
estacionado, ele acorda e tenta uma vez; só dormindo, vale como «tente já».
Pela tela, o diálogo «Acompanhar réplica…» mostra a parada com o botão
**Religar**.

### O irmão, e o que só a tela achou

Dois caminhos chamam o mesmo `ligar`, e os dois receberam o conserto:

- **o laço do cluster** puxa do master corrente pela credencial do bloco
  `cluster`. Ali não se estaciona para sempre, porque o master **muda**: a
  recusa fica anotada por master, e o laço volta a tentar quando a eleição
  entregar outro — ou por `replicacao_ligar` com a origem `cluster:<id>`;
- **a sonda da tela**: `replicacao_testar` com uma origem configurada liga
  nela com a mesma credencial do laço, e o diálogo «Acompanhar réplica…» a
  chamava a cada 3 s. **Medido só ao exercitar no navegador**: com o laço já
  estacionado direito, o diálogo aberto fez **uma tentativa a cada 3 s — cinco
  em 12 s** — e o master bloqueou o `127.0.0.1` pela sonda, não pelo laço. E
  o botão de
  religar nunca aparecia, porque a sonda falhada apagava as fichas e deixava
  só o erro dela. Hoje o diálogo lê o estado local **antes** de sondar, não
  sonda uma origem estacionada, e mostra parada + botão nos três caminhos; e
  `replicacao_testar` **recusa** uma origem estacionada nomeando o caminho de
  volta, porque a tela é um cliente entre vários e portão é um só.

### A prova, e os dois erros que ela me cobrou

`python3 bancada/replicacao/credencial-recusada.py` — oito casos pelo soquete,
com os dois controles na mesma corrida (o master continua bloqueando 5
erradas; a origem fora do ar continua sendo procurada, com recuo). Com
`--tela`, o religar do caso 7 é o **botão**, num navegador de verdade
(`testes-web/religar-na-tela.mjs`), e a bancada confere pelo protocolo que o
clique rendeu uma tentativa e o laço estacionou de novo. Sai com FALHA contra
o binário antigo — é a mesma corrida que serviu de antes e de depois.

Dois erros meus ficaram escritos nela: `usuario_alterar` não aceita
`senha_hash` pelo protocolo (hash pronto escolheria o próprio custo), e
consertar pelo master **não** serviria de qualquer jeito — o `senha_hash` da
réplica tem de ser o **mesmo texto** do cadastro de lá, porque é do sal dele
que ela deriva a chave; a senha certa com outro sal continua recusada. O
conserto do operador é no `config.json` da réplica, e o reinício é o outro
caminho de volta.

Guarda: `replica-insiste-na-credencial-recusada` no catálogo
(`bancada/guardas/catalogo.py`) — repõe o `Dormir` no lugar do `Estacionar` e
o teste cai. Cognição:
`docs/cognicao/cognicao_credencial-recusada-nao-e-falha-transitoria_20260909_0632.md`.

## 21. A revisão de 17/09/2026 — o que a replicação garante, o que não garante, e o que ficou para decidir

Ordem do dono, 17/09/2026 02:27 UTC: *«Dossiê atualizado · Status · Replicação
bateria de testes, revisão e conclusão»*. Três frentes correram em paralelo na
onda 1 (`docs/pmo/RODADA-2026-09-17-replicacao.md`): SEC fez a revisão
adversária (§21.1), C o parecer de DBA sobre as garantias de dado (§21.2) e G o
inventário estático de guarda × pétrea (§21.3) — todas **só leitura**, sem
subir servidor nem tocar `crates/`, porque a bateria de tempo (papel F) corria
ao mesmo tempo e disputaria o `flock`/soquetes. A bateria de F, a conclusão da
rodada e a linha B do `STATUS.md` entram nas §21.4 e §21.5 quando F devolver.

### 21.1 SEC — revisão adversária de replicação, cluster e quórum

**Quem:** papel SEC (revisor adversário), leitura pura — nenhum servidor
subido, nenhuma bancada de tempo rodada nesta frente. **Quando:** 17/09/2026,
parecer datado 02:41 UTC, integrado em `75b2f33` às 02:54 UTC (20 min 10 s de
frente, 116 ferramentas). **Fonte:** `docs/propostas/revisao-sec-replicacao-2026-09-17.md`
(879 linhas). O que a revisão procurou: o que quem tem o token, quem tem a
credencial de réplica (`replicar`) e quem tem `administrar` consegue pela
frente de replicação, e onde o portão que a casa já tem não alcança.

Onze achados, cada um com arquivo:linha, cenário de exploração e teste
adverso nomeado no parecer — o resumo, um por linha, com severidade e pedido
correspondente em `docs/PENDENCIAS.md`:

| # | severidade | uma frase | pedido |
|---|---|---|---:|
| A1 | **alta** | o pulso do cluster aceita identidade auto-declarada e época/posição/prioridade sem teto: um pulso forjado rebaixa o master e paralisa a eleição para sempre, inclusive depois de reiniciar | 278 |
| A2 | **alta** | `replicar` com `"max":0` lê o diário inteiro com as imagens para a RAM com a trava global de dados na mão; o teto de 16 MiB corta a **resposta**, depois | 279 |
| A3 | **média-alta** | `aplicar` é o caminho de gravação que desliga FK, CHECK, cascata e `conferir_filhas` — num source/multi sem `somente_leitura` ele está aberto e mata o pai que tem filhos, contra a pétrea | 280 |
| A4 | **média** | `cluster_no_remover` com `"propagar":false` cria maiorias assimétricas: dois masters graváveis, sem partição de rede nenhuma | 281 |
| A5 | **média** | `replicacao_testar` com host/porta livres é sonda de rede interna (SSRF cego), sem prazo de conexão nem teto de tentativas | 282 |
| A6 | **média** | `cluster_estado` entrega o mapa da infraestrutura (endereço:porta de cada nó, época, posição) a quem só tem `ler` | 283 |
| A7 | **média** | `replicas_autorizadas` é o mesmo portão da porta web: atrás de proxy reverso ou NAT a lista colapsa num IP só | 284 |
| A8 | **média** | `replicar` entrega o valor das colunas marcadas como dado pessoal e não grava registro na trilha (`.lgpd`) | 285 |
| A9 | **baixa-média** | modo B: o `carimbo_ms` vem do outro lado sem teto; um par hostil ganha todo conflito para sempre e sobrescreve calado | 286 |
| A10 | **baixa-média** | a op `config` publica a lista de nós do arranque, não a viva: o denominador da maioria mente depois de um escalonamento a quente | 287 |
| A11 | **baixa** | `cluster_pulso` é oráculo de ids de nó, com três respostas distintas e sem contar violação leve | 288 |

**A1, A2 e A3 foram consertados em `49a3af7` (17/09/2026, frente B — ver
§21.4), cada um com prova real nos dois sentidos.** A2 (pedido 279) fechou
inteiro: o teto de bytes agora limita a leitura, não só a resposta, e `max`
ganhou padrão e teto de eventos. A3 (pedido 280) fechou inteiro: o crivo do
portão 2b-bis vale independentemente do `somente_leitura`. A1 (pedido 278)
fechou **parcialmente**: `cluster_pulso` entrou em `OPS_DE_REPLICACAO` e
época/posição ganharam teto (`FOLGA_DE_EPOCA` = 1.000.000).

**A4, A5, A6, A8, A9, A10 e A11 foram consertados na onda seguinte, em
`eeb9925` (17/09/2026, frente B2), cada um com prova real nos dois
sentidos.** A4 (281): `"propagar":false` só vale de dentro do cluster ou com
credencial do cluster vinda de um nó da lista viva. A5 (282), o resto que
`49a3af7` não tinha fechado: a sonda classifica pelo `ErrorKind` em quatro
chaves da fábrica, sem texto do sistema operacional, e host fora da
configuração que não responde conta violação leve. A6 (283): `cluster_estado`
parte a resposta — `master`/`papel`/`época`/`escrita_liberada` para quem só
lê, `nos[]` só para quem administra. A8 (285): `replicar` grava acesso na
trilha `.lgpd`. A9 (286): carimbo do futuro além de 5 minutos entra com o
relógio local, contado e não recusado. A10 (287) fechou **por prova, não por
conserto**: a lista viva já injetava desde `a446c7a` (07/09), e faltava só o
teste e o campo `tem_pino`. A11 (288): id desconhecido e id do próprio nó
dão a mesma resposta, fechando o oráculo.

**A1 pleno voltou como PARECER, não como conserto** (frente B2, mesmo
commit): o cluster cifrado usa Noise **NX** — só o respondedor apresenta
chave estática no aperto, e quem manda o pulso é o **iniciador**, anônimo por
decisão já registrada em `docs/CIFRA-DO-FIO.md` §12. Amarrar a identidade do
nó ao pulso exigiria prova por Diffie-Hellman das estáticas que já existem
(`chave_do_fio` já é, de fato, um `known_hosts`) ou trocar o padrão do aperto
para XX/IK — as duas são desenho de protocolo cifrado, e ficam com o dono.

**Só A7 continua inteiramente aberto** (pedido 284 — `replicas_autorizadas`
colapsa atrás de proxy/NAT), porque o conserto ali é de infraestrutura
(trancar por credencial em vez de IP) e não entrou em nenhuma das duas ondas.
Ver o texto de cada pedido em `docs/PENDENCIAS.md` para o antes e o depois de
todos.

**§Z — já documentado e ainda aberto no código.** Não é achado novo: está em
`docs/SEGURANCA.md` §12.4 desde o pedido 194/item 16 de `docs/PENDENCIAS.md`
§3.2, e a SEC o reconferiu porque cai na fronteira desta frente. `reg.rs:1827-1830`
(`abrir_externo`) continua devolvendo os bytes cifrados como conteúdo quando a
réplica está sem cifra — *«o pior dos três casos, porque é o único que não dá
erro»*. Continua faltando a conferência que falta em `decodificar_com_externos`
(`table.rs:4051-4082`). Reconferido e cruzado no mesmo dia pelo parecer do
papel C (§21.2, pedido 293).

A revisão também nomeou dezoito pontos que **já estão bem**, cada um com
arquivo:linha — do túnel vindo antes do login à credencial nunca saindo em
resposta de protocolo — «para a conclusão do papel H não elogiar de memória»
(SEC, §«O que está bem»). E nomeou nove frentes que **exigem servidor de pé** e
ficam para o papel F (A1 e A2 pelo soquete, A5 contra o sistema operacional,
A4 pelo soquete, a cifra do fio ponta a ponta, os vetores de cripto, o quórum
de escrita, a porta REST/MCP).

**Posição de SEC para a conclusão desta rodada**, citada aqui porque a §21.5
vai decidir sobre ela: *«A1, A2 e A3 exigem decisão registrada (conserto ou
aceite do dono) antes de a replicação se declarar revisada e conclusa.»*

### 21.2 C — parecer do DBA sênior sobre as garantias de dado da replicação

**Quem:** papel C (DBA sênior), leitura pura — nenhuma linha de código
alterada, nenhum commit partiu desta frente. **Quando:** 17/09/2026 02:39 UTC,
integrado em `9da28a4` às 02:47 UTC (16 min 49 s de frente, 92 ferramentas).
**Fonte:** `docs/propostas/parecer-dba-replicacao-2026-09-17.md` (556 linhas).
Escopo: o que uma réplica entrega ao fim de um alcance, garantia por garantia,
com o cenário exato que quebra cada uma que não vale.

**Duas correções ao briefing**, antes de qualquer conclusão: o evento do
`.log` tem **44 bytes** de cabeçalho, não 36 (`log.rs:81`; os 36 são a
fronteira do CRC); e `reconciliar_sequencia` mora no *store*
(`table.rs:3009`), não no bidirecional.

**Três medições novas, feitas num binário isolado fora do repositório** (sem
tocar `crates/`, sem disputar o `flock` com a bateria de F):

- **`rownum` diverge depois de uma inserção recusada** — 2 de 5 linhas com
  `rownum` diferente entre source e réplica, rowids iguais nas 5 (§2.1;
  pedido 291). **Confirmado pelo soquete pela bateria de F, com o alcance
  corrigido**: a divergência só sobrevive dentro de um `inserir_lote` com
  `"parar_no_erro": false` — recusa em operação própria reabre a instância e
  a queima do contador se perde. Isso não diminui o achado (o caminho de
  importação/carga é exatamente onde a recusa é rotina); ver a §21.4 para a
  prova com controle por estágio, em vez de repetir aqui o alcance antigo.
  **FECHADO pela via (a) em `eeb9925` (17/09/2026, frente B2)**: o consumo do
  `rownum` passou a acontecer depois da última guarda que pode recusar a
  linha, e não antes — a linha recusada nunca chega a ter número. Não é
  retroativo (um buraco já gravado antes deste commit continua); a via (b)
  — a réplica honrar o `rownum` da imagem — segue aberta e compatível, como
  pedido próprio (309).
- **Unicidade num índice secundário trava o par de servidores no
  bidirecional para sempre** — `[SP000020] chave duplicada`, o mesmo erro em
  `inserir_replicado` e `aplicar_evento` (§2.5; pedido 292).
- **12 eventos gravados num único milissegundo** — um só carimbo distinto
  para os 12, medindo o que a coluna de data/hora de sistema por linha (decisão
  do dono de 11/09/2026) precisa resolver antes de nascer (§4.2; pedido 289).

**A tabela garantia × vale?** (§1 do parecer) resume treze linhas — rowid,
linha, `.memo`, `.bin`, `rownum`, `versao`, `.trash`/`.reason`, carimbo e
origem do `.log`, integridade referencial, unicidade, atomicidade de commit,
posição do cluster. As que **não valem**, com o pedido que as carrega:

| garantia | vale? | pedido |
|---|---|---:|
| mesmo `rownum` | **NÃO**, silencioso | 291 (decisão do dono, 3/6) |
| mesma `versao` | por construção, nunca conferida | 296 |
| mesmo `.trash`/`.reason` | NÃO, por desenho — sem ressalva escrita | 297 |
| mesmo carimbo/origem no `.log` | NÃO, unidirecional — o PITR já faz certo | 298 |
| integridade referencial na réplica | NÃO, decisão **já** registrada | pedido 171/`INTEGRIDADE.md` §3 |
| unicidade na réplica | SIM, mas trava o par no bidirecional | 292 (decisão do dono, 4/6) |
| atomicidade de commit | NÃO, e RECUSADO consertar sem o dono | 299 |
| posição somada do cluster | NÃO em quatro cenários | 294, 295, 300 |

**As seis decisões do dono** (§6 do parecer, pedidos 289–294): coluna de
data/hora de sistema por linha e sua resolução; `inicio`/`passo` da `Sequence`
no mesmo bump de `PSCH`; quem honra o `rownum` numa réplica; o que fazer com
único secundário no bidirecional; replicar coluna externa marcada — recusar no
motor ou esperar o envelope da §11.5 (cruza com o §Z de SEC, §21.1); o
critério de eleição do cluster. **A terceira (rownum, pedido 291) recebeu
metade de resposta em `eeb9925` (17/09/2026, frente B2)**: a via (a) — consumir
o número só depois da última guarda que recusa — fechou e resolve toda escrita
nova; a via (b) — a réplica honrar o `rownum` da imagem, para também os
buracos históricos — segue como decisão do dono, agora com pedido próprio
(309). As outras cinco continuam inteiras na mesa.

**O item de maior retorno** (pedido 295): dar à réplica a mesma conferência
de continuidade que o PITR já tem (`diario_vivo_continua`,
`servidor.rs:18640-18673`) — não é formato, não é consenso, não muda cliente
nenhum, é a guarda que já foi escrita alcançando o caminho irmão, no mesmo
padrão dos pedidos 172/173/176. **Consertado em `49a3af7` (17/09/2026, frente
B — §21.4)**: a réplica confere e acusa; a ressalva do bidirecional (não
entrou) ficou nomeada e é decisão do dono.

**O NÃO do papel C** — cinco propostas boas, recusadas com o número, para não
voltarem sem medição: devolver o contador do `rownum` na recusa (reintroduz
reuso de número de ordem, pedido 291); id de transação no evento do `.log`
(muda o formato do cabeçalho e não compra atomicidade entre tabelas, pedido
299); a réplica conferir a unicidade do secundário como confere FK (calaria um
índice declarado único, pedido 292); trocar a soma do cluster por vetor de
posições por tabela (mexe no critério de eleição, pedido 294); replicar coluna
externa e resolver a senha na configuração (a condição nunca se satisfaz,
pedido 293).

### 21.3 G — inventário QA: guarda × pétrea na família da replicação

**Quem:** papel G (QA), frente **estática** — nenhum `cargo`, nenhum
`provar-guardas.py`, nenhum servidor subido, para não disputar o `flock` nem
os soquetes com a bateria de F. **Quando:** 17/09/2026, medições com hora UTC
de cada comando (02:34 UTC para as réguas estáticas), integrado em `728a46f`
às 02:43 UTC (12 min 48 s de frente, 93 ferramentas). **Fonte:**
`docs/propostas/inventario-qa-replicacao-2026-09-17.md`.

**Treze entradas do catálogo** (`bancada/guardas/catalogo.py`) tocam
replicação/cluster/quórum, contra o veredito da última corrida do provador
(16/09/2026 15:25): doze **PROVADAS**, e a treze-ésima
(`cluster-devolve-a-credencial-na-tela`, nascida em 17/09) corretamente
nomeada como **NÃO JULGADA** — a sexta régua do catálogo
(`TETO_NAO_JULGADA_ESCONDIDA`) mede **0** entradas escondidas, ela incluída.

**O achado que atravessa a lista inteira**: `crates/phxsql-server/src/cluster.rs`
tem **zero** entradas em `bancada/guardas/catalogo.py` (confirmado por
`grep '"arquivo": ".*cluster.rs"'`), apesar de ser o arquivo da eleição
(pedido 211), do escalonamento a quente (pedido 217) e dos quatro modos A–D
(pedido 214), todos com teste real e comentado como prova — o catálogo tem
uma entrada em `replica.rs` e nenhuma em `cluster.rs` (pedido 302).

**Sete pétreas com teste real e sem guarda no catálogo** (pedido 301), cada
uma com o teste que existe hoje e o teste que cairia se o defeito voltasse:
`replicas_autorizadas` vazia libera; `incompleta:false` por omissão não
encolhe a posição em silêncio; a eleição prefere completa; réplica não atende
escrita (portão 2b-bis); `spare` não atende ninguém; read replica recusa
escrita apontando o master; e o pulso de id fora da lista + nó novo sem
reiniciar.

**`TETO_DO_LOTE_SERVIDO` e `TETO_DA_RESPOSTA`** (pedido 147) não têm prova
nenhuma, nem unitária nem de bancada — cruza com o A2 de SEC (§21.1, pedido
279): SEC mediu que o teto corta a resposta e não a leitura; G mediu que não
há nenhum teste do corte por bytes (pedido 303). **`TETO_DO_LOTE_SERVIDO`
ganhou prova em `49a3af7` (17/09/2026)** —
`log::tests::percorrer_com_limite_zero_nao_le_tudo` e
`o_primeiro_evento_entra_sempre_e_o_teto_so_conta_imagem`, mais os testes de
`servidor::testes_do_lote_de_replicacao` — e o próprio teto passou a limitar a
leitura, como SEC pedia; o pedido 303 fica **◐**, porque `TETO_DA_RESPOSTA`
continua sem prova.

**Catracas de replicação/concorrência medidas nesta sessão** (17/09/2026
02:34 UTC): `TETO_TRECHO_MORTO`, `TETO_TRECHO_AMBIGUO`, `TETO_TESTE_MORTO`,
`TETO_TESTE_FORA_DO_BINARIO`, `TETO_TESTE_SEM_MODULO` e
`TETO_NAO_JULGADA_ESCONDIDA` em **0**; `codigo-do-dono` em **5** (teto 5, sem
folga); `rede-ou-espera` (a catraca de REPLICACAO §18) em **0**;
`alcancam-fsync` em **23** (teto 22, vermelha na data desta corrida; a
pendência #252 fechou em 18/09/2026 — a metade que era código entrou por
mérito e a catraca foi **aposentada**, substituída pela `alcancam-fsync-2`
em 24); `PISO_DAS_ENTRADAS`
em **180**, subindo dos 177 anteriores — o comportamento correto de um piso
que só sobe.

Fora de escopo de guarda, por decisão já registrada: transação com quórum
(pesquisa/plano, papel J, `docs/propostas/quorum-de-escrita.md`, ainda não
implementado) e `alcancam-fsync` (dívida registrada na pendência #252, que
fechou em 18/09/2026 — ver acima).

### 21.4 F — a bateria de 17/09, número por número, contra servidores de pé

**Quem:** papel F (usuários de teste e revisor de prova real) — a bateria
propriamente dita, rodando contra `phxsqld` de verdade, nunca por dentro do
motor. **Quando:** 17/09/2026, 02:29–03:01 UTC, integrada em `34ef2c1`.
**Fonte:** `docs/propostas/bateria-replicacao-2026-09-17.md` (500 linhas) e os
`resultados.json` que a própria bateria regravou. Binário: `flock
/tmp/phx-cargo.lock cargo build --release`, `target/release/phxsqld` de
**02:29:41 UTC** — mais novo que a tradução da noite (01:01:51), o que a lei
do binário velho exige conferir antes de qualquer número.

#### As dez bancadas, hoje × antes

| bancada | hoje (17/09) | antes | fonte do «antes» | veredito |
|---|---|---|---|---|
| replicação clássica (`montar.py`+`medir.py`) — master | **45.117 linhas/s** | 33.883 | `resultados.json`, 07/09 | **1,33×** |
| idem — réplica aplica | **43.606 eventos/s** | 37.311 | idem | 1,17× |
| idem — retrato SHA-256 dos 4 | `72554b753253cd5d` nos quatro | iguais | idem | **PASSA** |
| os quatro modos (`modos.py`, não grava arquivo) | **9 de 9 estágios [ok]** | — | veredito ditado nesta página (§1.2), porque a bancada não grava | ok |
| a trava de dados (`trava.py`) — alcance de 200.000 eventos | **2,39 s — 83.567 ev/s** | 4,54 s — 44.062 | `trava.json`, 05/09 | **1,90×** |
| idem — queda: soma de verificação | `1aa1e8124df2cba0` nos dois lados | a mesma soma | idem | **PASSA** |
| credencial recusada (`credencial-recusada.py --tela`) | **8 de 8 casos — PASSA**, incluindo o botão «Religar» num navegador de verdade | idêntico | `credencial-recusada.json`, 09/09 | ok |
| cluster (`cluster/provar.py`) | **26 de 26 conferências ok**, `falhas: []` | idem (arquivo **byte a byte igual**) | `cluster/resultados.json`, 11/09 | **PASSA** |
| a fresta (`cluster/fresta.py`, não grava arquivo) | **10 de 10**, `falhas: []`, nas duas ordens de morte | — | veredito ditado nesta página (§1.6) | ok |
| escalonar a quente (`cluster/escalonar.py`) | escritas recusadas **0 de 43** | 0 de 42 | `resultados-escalonar.json`, 07/09 | ok |
| quórum (`quorum/medir.py`) — commit 2-de-3 / 3-de-3 | **3,04× / 3,56×** | 3,08× / 3,56× | `quorum/resultados.json`, 07/09 16:37 | **a razão se manteve** |
| o canal do pulso (`quorum/canal.py`) | pulso quente **0,146 ms** [0,078; 0,388] | 0,089 [0,069; 0,232] | `resultados-canal.json`, 07/09 17:54 | **faixas se cruzam — sem vencedor** |
| os quatro modos em contêiner (`docker/provar.py`) — source em contêiner | **16.030 linhas/s**; réplica alcança em 1,92 s | 13.462; 2,94 s | `docker/resultados.json`, 05/09 | ok |
| idem — retrato SHA-256 | `39787c620feeed8f` nos dois, 101.013 linhas | iguais | idem | **PASSA** |

A regra do pedido 155 vale aqui como valeu no quórum de 07/09: onde a mediana
caiu mas as **faixas se cruzam** (quórum e canal do pulso), não se declara
vencedor dentro do ruído — o que se sustenta é a **razão** entre os regimes,
que saiu igual em corridas de dez dias de distância.

#### Os três achados do papel C, confirmados pelo soquete com controle por estágio

Ler código não prova defeito de replicação — o que depende de dois processos
e um soquete se prova contra dois processos e um soquete. F escreveu
`bancada/replicacao/achados-do-dba.py`, em que **cada estágio roda o cenário
E o controle** (o mesmo roteiro com a única linha do defeito retirada),
rodado às **02:58:51–03:00:08 UTC**, resultado em
`bancada/replicacao/achados-do-dba.json`:

- **`rownum` divergente (§21.2) — CONFIRMADO, com o alcance mais estreito do
  que o parecer descreve.** A leitura do código (que `numerar_linha` consome
  o contador antes da conferência de unicidade) está certa, mas a prova pelo
  soquete separou onde ela morde: recusa por chave duplicada na primária, por
  chave duplicada num único secundário, ou por coluna obrigatória faltando —
  todas em **operação própria** — **não** divergem, porque `proximo_rownum`
  é estado da instância aberta do `Reg`, e o servidor reabre a tabela entre
  operações (a queima se perde). A divergência só sobrevive **dentro de um
  `inserir_lote` com `"parar_no_erro": false`**, onde a instância é a mesma
  do começo ao fim do lote: `[1,2,3,5,6]` no source contra `[1,2,3,4,5]` na
  réplica, retrato SHA-256 diferente (`252fa89db5038769` × `d92da11a064d6f11`),
  contra o controle das mesmas 5 linhas sem recusa, que bate
  (`d92da11a064d6f11` nos dois). O alcance mais estreito **não diminui o
  achado** — piora, porque `inserir_lote` com `parar_no_erro:false` é
  exatamente o caminho de importação/carga, onde uma linha recusada é rotina
  e não acidente. Ver §21.2, que aponta para aqui em vez de descrever o
  alcance antigo.
- **Único secundário trava o par bidirecional — CONFIRMADO.** Com
  e-mails distintos (controle), os ids em beta terminam `[1,2,3]` e a
  escrita seguinte, que não conflita, chega. Com o mesmo e-mail (cenário),
  beta fica em `[2]`, a linha do conflito **não** chega e a escrita
  **seguinte, que não conflita com nada**, também não — é a linha que separa
  «uma linha perdida» de «o par de servidores parado», e ela julgou a favor
  do segundo.
- **Tabela apagada e recriada no source — CONFIRMADO, e o silêncio é o que
  assusta.** Réplica fica com `[1,2,3,4,5]`/5 eventos enquanto o source tem
  `[91,92,93]`/3, as linhas novas nunca chegam, e `replicacao_estado`
  responde `ultimo_erro: null`, `parada: null` — sem uma palavra sobre o
  descompasso. O PITR pega este caso (`diario_vivo_continua`); a réplica
  não. Ver pedido 295.

#### Duas hipóteses que morreram medidas

- **«A queda do `master_linhas_s` foi o `fsync` que a onda 2 pôs no caminho
  de escrita»** (candidato nomeado pelo pedido 193) — **morta, medida**.
  `strace -f -c -e trace=fsync,fdatasync,sync_file_range` numa carga de
  100.000 linhas deu **104 chamadas de `fsync`** — 0,00104 por linha, ordem
  de grandeza de *fecho de janela*, não de linha (bate com
  `TETO_FSYNC_POR_FECHO_V2`, que mede `fsync` por fecho, e com o comentário
  de `servidor.rs:14334-14339`, que registra que a primeira versão sim
  chamava `sincronizar()` por tabela por commit e foi trocada). E o número
  de hoje, com esse `fsync` de pé, é **45.117 linhas/s** — 1,69× acima dos
  26.762 que o pedido 193 registrou. O que causou a queda de 05/09 continua
  sem medir, por falta de disco para o *worktree* — mas deixou de ser
  urgente, porque o número não está preso em 26.762.
- **«O contêiner é ~2,8× mais lento que o processo»** — **morta, medida, e a
  causa é a libc**. A bancada de contêiner roda o estágio de processos com o
  binário **musl** de propósito (para comparar trabalho igual entre
  contêiner e processo, regra 4 da bancada), o que torna o número dela
  incomparável com o `gnu` do `medir.py`. Medido com a mesma carga, mesmo
  esquema, mesmo cliente (`bancada/replicacao/custo-do-binario.py`, 03:01
  UTC): **gnu 48.510 linhas/s × musl 20.965 linhas/s = 2,31×** — os 2,31× do
  binário explicam quase toda a diferença; o resto é o daemon do Docker no
  ar e uma réplica contra três. O contêiner não é lento: o `musl` é — e isso
  é informação de produto (a imagem `FROM scratch` que a casa publica **é**
  a musl), não recomendação de trocar de libc.

#### Três consertos de bancada, cada um com o RED

Todos no roteiro **novo** (`achados-do-dba.py`), nenhum em bancada antiga.
Os três são da mesma família — *prova que não confere o próprio estrago mede
outra coisa*:

| # | o defeito | RED (com o defeito) | GREEN (com o conserto) |
|---|---|---|---|
| 1 | `excluir_tabela` sem `confirmar` é recusado, e o estágio não conferia a resposta | cenário e controle davam o **mesmo** resultado (réplica com `[1,2,3,4,5,91,92,93]` nos dois — a tabela nunca foi apagada); veredito `[FALHA]`, «o defeito não existe» | com `"confirmar":"clientes"` + parada explícita se a montagem falhar: source `[91,92,93]`/3 eventos, réplica `[1,2,3,4,5]`/5, `chegou=False`, veredito `[ok]` |
| 2 | `eventos()` pedia `posicao` com `"tabela"` no pedido, mas o campo mora em `resultado.tabelas.<tabela>.eventos` | devolvia `None` calado; todo `esperar(...==5)` esgotava o prazo em vez de esperar; JSON de 02:54 com `eventos_source: null`, `eventos_replica: null` | `eventos_source: 3`, `eventos_replica: 5` — o par que sustenta o achado da tabela recriada |
| 3 | `--so <estágio>` sobrescrevia o `achados-do-dba.json` inteiro | a corrida `--so rownum` deixava um arquivo só com `{"rownum": …}`, parecendo a bateria completa | mescla por nome + campo `preservados_de_corrida_anterior`: a corrida parcial preserva os outros dois estágios **e diz quais preservou** |

### 21.5 A conclusão

**Quem:** papel H, fechando a rodada aberta pela ordem do dono de 17/09/2026
02:27 UTC. **Quando:** 17/09/2026, depois do último veredito às 05:03 UTC.
**Fontes:** as quatro anteriores (§21.1–21.4), `git show 49a3af7` e `git show
eeb9925` (as duas ondas de conserto), e `bancada/guardas/ultima-corrida.json`
(commit `9438bd0`).

#### O que se declara revisado e provado

- **A bateria inteira**: dez bancadas, todas verdes, com ganho medido sobre a
  corrida anterior em cada uma (§21.4) — master **1,33×**, trava **1,90×**,
  cluster **26 de 26**, quórum com a razão do 2-de-3/3-de-3 mantida em duas
  corridas de dez dias de distância.
- **Os três achados medidos do parecer de C, confirmados pelo soquete com
  controle por estágio** (`bancada/replicacao/achados-do-dba.py`, §21.4): o
  `rownum` diverge só dentro de `inserir_lote` com `parar_no_erro:false`, o
  único secundário trava o par bidirecional para sempre, a tabela recriada
  no source congelava a réplica em silêncio.
- **As treze guardas da família da replicação/cluster/quórum, PROVADAS —
  13 de 13** (`bancada/guardas/ultima-corrida.json`, commit `9438bd0`,
  17/09/2026 04:50–05:03 UTC, 16,0 a 36,3 s cada):
  `replica-lista-e-pedida-nao-imposta` (36,33 s), `posicao-nao-encolhe-em-silencio`
  (34,80 s), `eleicao-prefere-completa` (29,32 s), `replica-nao-atende-escrita`
  (34,81 s), `spare-nao-atende-ninguem` (33,83 s), `read-replica-recusa-escrita`
  (33,88 s), `pulso-fora-da-lista-e-recusado` (34,46 s), `trava-atras-da-rede`
  (16,01 s), `colisao-de-sequence-calada` (29,23 s), `posicao-sem-portao`
  (34,81 s), `replicacao-do-cluster-em-claro` (29,39 s),
  `replica-insiste-na-credencial-recusada` (28,45 s),
  `cluster-devolve-a-credencial-na-tela` (30,88 s).
- **Treze consertos, em duas ondas, cada um com prova real nos dois
  sentidos** (o teste falha com o defeito reposto e passa com o conserto):
  cinco em `49a3af7` — A2 e A3 inteiros, A1 parcial, A5 parcial, a
  conferência de continuidade da réplica (item de maior retorno de C); oito
  em `eeb9925` — o `rownum` pela via (a), A4, A5 completo, A6, A8, A9, A10
  (por prova) e A11. Portões verdes nas duas integrações e na corrida final
  das guardas: `fmt` limpo, `clippy` zero avisos, **2.447 testes passaram, 0
  falharam, 4 ignorados pré-existentes** (71 binários).

#### A posição de SEC, que era o portão desta conclusão

SEC escreveu, ao entregar a revisão (§21.1): *«A1, A2 e A3 exigem decisão
registrada (conserto ou aceite do dono) antes de a replicação se declarar
revisada e conclusa.»* Com o que a rodada mediu:

- **A2 e A3 foram consertados**, inteiros, com prova real e teste do
  comportamento velho ao lado (`49a3af7`).
- **A1 tem decisão registrada — é parecer com premissa medida, não
  esquecimento.** O aperto de mão do cluster cifrado é Noise **NX**, e nesse
  padrão só o respondedor apresenta chave estática; quem manda o pulso é o
  **iniciador**, anônimo por decisão já tomada quando o `CIFRA-DO-FIO.md` foi
  escrito. Amarrar a identidade do nó ao pulso exige mudar o protocolo —
  prova por Diffie-Hellman das estáticas que já existem, ou trocar o aperto
  para XX/IK — e isso está escrito por inteiro em `docs/CIFRA-DO-FIO.md`
  §12, com o motivo de cada alternativa não ter entrado nesta rodada. O que
  cabia num conserto delimitado entrou (`eeb9925`): o teto de época que
  impede o estrago **permanente**, e o pulso passou a valer só dentro do
  portão das réplicas autorizadas.

**A condição de SEC está cumprida por essa leitura**: as três exigências
têm, hoje, ou conserto com prova, ou decisão registrada com a premissa
medida por escrito. Quem discordar de que um parecer satisfaz a condição —
e achar que só conserto de código deveria contar — tem onde discordar: o
parecer de B2 está inteiro em `docs/CIFRA-DO-FIO.md` §12, e a leitura dele
não depende desta conclusão para ser conferida de novo.

#### O que continua aberto, nomeado com pedido

- **A1 pleno** (278, ◐) — identidade do nó no pulso; exige redesenho do
  aperto de mão, decisão do dono.
- **A7** (284) — `replicas_autorizadas` colapsa num IP só atrás de proxy
  reverso ou NAT; é o único achado de SEC que nenhuma das duas ondas tocou.
- **Cinco decisões de formato do dono** (parecer de C, §6; pedidos 289, 290,
  292, 293, 294): a coluna de data/hora de sistema por linha e sua
  resolução; `inicio`/`passo` da `Sequence` no `PSCH`; o que fazer com único
  secundário no bidirecional (hoje trava o par para sempre); replicar coluna
  externa marcada (grava texto cifrado como conteúdo com a cifra desligada);
  o critério de eleição do cluster (a posição somada é um escalar de uma
  grandeza vetorial).
- **A via (b) do `rownum`** (309) — a réplica honrar o número que vem na
  imagem, para fechar também os buracos já gravados antes de `eeb9925`; a
  via (a), que já fechou, não é retroativa.
- **A conferência de continuidade não entrou no bidirecional**
  (`alcancar_tabela_bidi`) — só o modo unidirecional acusa tabela apagada e
  recriada; decisão do dono, nomeada e não tomada.
- **`TETO_DA_RESPOSTA` continua sem guarda no catálogo** (303) — e não por
  descuido: G mediu que não existe hoje nenhum teste, unitário ou de
  bancada, cujo defeito reposto o faria cair. Guarda sem teste que caia não
  é guarda; é uma entrada que passaria sempre.

#### O que a rodada aprendeu sobre si mesma

Três frentes acharam defeito no **próprio instrumento**, não no motor: o
provador de guardas não copiava um arquivo de `bancada/` que os testes leem
por `CARGO_MANIFEST_DIR`, e cinco guardas voltavam sem veredito por isso; a
guarda `trava-atras-da-rede` **envelheceu no dia seguinte** ao próprio
conserto que ela media, porque a âncora ficou no código que a conferência de
continuidade reescreveu; e uma corrida parcial (`--so`) sobrescrevia o
arquivo inteiro, o que teria trocado 143 vereditos de ontem por 13 de hoje e
escondido tudo que não foi medido nesta rodada. **Instrumento também
envelhece** — e as três vezes ele foi pego pela mesma disciplina que ele
existe para impor: medir de novo em vez de confiar na última vez que
mediu.

#### O que esta replicação garante hoje, e o que não garante

Não se declara «pronta», e não se repete *ACID compliant*. O que a rodada de
17/09/2026 deixa medido: a replicação entrega o dado — rowid, linha,
`.memo`, retrato SHA-256 idêntico entre master e réplicas — com throughput e
recuperação melhores do que a corrida anterior, e a superfície de ataque que
a revisão achou está fechada em nove de onze pontos, com prova. O que ela
**não** garante, e cada um tem pedido aberto contra o próprio nome: `rownum`
idêntico em toda topologia (só a escrita nova, via a); um índice único
secundário sobrevivendo a um conflito no bidirecional; atomicidade de um
commit multi-tabela atravessando o fio; coluna externa marcada replicada
com segurança; identidade criptográfica de quem manda o pulso do cluster. A
lista acima **é** a garantia — não a frase que a resumiria.

## 22. O conflito de unicidade PARA o par, marcado — e a saída é humana

**Pedido 292, parte (1), 17/09/2026.** O que entra aqui não é a forma que o
dono decidiu às 07:10 — é a que ele decidiu **depois**, quando a régua dos
motores maduros derrubou a primeira
(`docs/propostas/regua-dos-motores-decisoes-289-294-2026-09-17.md` §292).

### O que foi derrubado, e por quê

A forma antiga era **recusar a tabela com índice único secundário no modo
multi, na declaração**. Medido, nenhum dos três motores maduros faz isso: o
Galera **certifica por essa chave de propósito** (`ha_innobase::wsrep_append_keys()`
percorre todas as chaves e promove a exclusiva a que tem `HA_NOSAME`), o
PostgreSQL aplica e quebra visivelmente, e o Group Replication não recusa por
único secundário. E a justificativa que existia — «não há ninguém
funcionando» — é **falsa** para o par que não colide e para todo
unidirecional, onde só um lado escreve: a recusa tiraria do ar tabelas que
replicam bem hoje. A pétrea *«guarda nova entra pedida, não imposta»* estava
batendo de frente.

O teste que trava isso é `sem_colisao_o_laco_replica_como_sempre_e_nada_e_contado`,
e ele **não é decorativo**: repondo a forma derrubada (recusar a tabela quando
há único secundário, em `abrir_para_bidi`), os quatro testes de
`tests/laco-do-unico-secundario.rs` caem — inclusive esse, com «as duas linhas
do parceiro não aconteceu em 20 s». É a medida do estrago que a recusa teria
feito.

### O que entra no lugar

A tabela continua nascendo e replicando. No conflito:

1. **O par para naquela tabela**, marcado. `replicacao_estado` ganha
   `origens.<nome>.paradas`, com `motivo` (chave, nunca frase),
   `posicao` (onde parou, no diário da origem), `detalhe` e `desde`.
2. **A posição NÃO anda**, e isso é a mesma decisão escrita no fonte do
   PostgreSQL (`worker.c`, `replorigin_reset`: não avançar a origem é o que
   impede perder o evento).
3. **O grito carrega o que o PostgreSQL carrega** — índice, valor da chave, a
   linha daqui e a de lá —, no `replicacao_estado` e no diário do processo.
4. **A saída é manual**: `replicacao_pular`.

```json
{"op":"replicacao_pular","origem":"parceiro",
 "database":"loja","tabela":"clientes"}
{"ok":true,"resultado":{"pulou":0,"posicao":1,
  "motivo":"conflito_de_unicidade","detalhe":"índice \"porEmail\"...",
  "aviso":"o evento pulado NAO entra mais: ..."}}
```

### Três divergências deliberadas com o PostgreSQL, e a restrição de cada uma

- **O valor da chave sai REDIGIDO.** Ele imprime `Key (c)=(1)` porque não tem
  marca de dado pessoal no esquema; nós temos, e uma primária de CPF sairia no
  log do processo. A redação é por **análise** — cada coluna decidida pelo que
  o esquema diz dela —, e o que não se analisa (coluna marcada, `Bin`, `Memo`,
  valor acima de 48 bytes) vira o **tamanho em bytes**. A pétrea da casa é mais
  forte que a convergência: o comportamento (o conflito aparece) entrou
  inteiro; o meio (publicar o valor) não.
- **`replicacao_pular` EXIGE uma parada.** O `pg_replication_origin_advance()`
  aceita qualquer LSN, e o manual dele avisa que usar errado leva a
  inconsistência. Aqui a restrição é outra: no nosso laço **reaplicar é
  inofensivo** — o casamento é por chave e a regra é «mais recente vence» —,
  então andar a posição nunca conserta nada; só pode pular evento que ninguém
  olhou. Sem a parada exigida, a operação seria um botão que só tem como errar.
- **O estrangulamento é o próprio portão, e ele vem ANTES do trabalho.** O
  PostgreSQL estrangula em 5 s (`launcher.c`, «once per
  `wal_retrieve_retry_interval`»); aqui a tabela parada sai do alcance **sem
  tomar a trava de dados, sem absorver o diário local e sem uma ida e volta de
  rede**. Medido pelo soquete, contando os `replicar` que o parceiro serve em
  10 s com o par parado: **0 com o portão, 10 sem ele** — uma por segundo, o
  `reconectar_em` do cenário —, e as recusas contadas do lado de cá subiram de
  2 para 12 no mesmo intervalo: uma linha de log por segundo, para sempre.

### O irmão de mão única NÃO mudou, e o motivo é de formato

`alcancar_tabela` → `aplicar_lote_da_replica` → `aplicar_evento` para pelo
mesmo desenho, e ali parar é **projetado**: a réplica fiel aplica **por
rowid**, e o `.reg` nunca reaproveita slot, então o rowid que ela gera tem de
bater com o do evento — é a conferência de fidelidade que sai de graça. Pular
um evento ali deslocaria **todos** os rowids seguintes e transformaria um
problema que para num problema que diverge em silêncio. No PostgreSQL o `SKIP`
funciona porque a replicação lógica não alinha posição física nenhuma. A
restrição que causa a divergência é a nossa: *a ordem de digitação é sagrada em
cada servidor*.

Guardas no catálogo (`bancada/guardas/catalogo.py`):
`laco-preso-no-unico-secundario` (PROVADA 3/3),
`par-parado-reapresentado-a-cada-rodada` (PROVADA 1/1) e
`dado-pessoal-no-grito-do-conflito` (PROVADA 1/1).

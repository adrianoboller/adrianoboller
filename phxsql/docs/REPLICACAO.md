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

O que ainda **não** existe está na seção 10, e um item mudou de lugar: a réplica
aplica mais devagar do que o master escreve, e sob carga sustentada fica atrás.

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

**Failover.** Promover uma réplica a Source é trocar `papel` de `replica` para
`source` e desligar `somente_leitura`. As outras réplicas passam a apontar
para ela. Como a posição é o ordinal do evento no `.log` e todas as réplicas
aplicaram a mesma sequência, elas continuam de onde pararam.

O ponto delicado, e é honesto dizer: se o Source cair **no meio** de uma
gravação, réplicas diferentes podem ter parado em pontos diferentes. Sem
transações, a promoção é segura quando as réplicas estão na mesma posição, e
exige conferência quando não estão. Failover automático é assunto para depois
das transações.

---

## 9. O que está feito, e o que falta

| | |
|---|---|
| ☑️ | `.log` versão 2 com imagem da linha, atrás do interruptor |
| ☑️ | Ops `posicao`, `replicar` e `aplicar` |
| ☑️ | Laço da réplica dentro do `phxsqld`: puxar, aplicar, conferir o rowid |
| ☑️ | Criar na réplica a tabela que ainda não existe, do esquema cru do source |
| ☑️ | Reconexão e retomada pela posição — medido: 1,0 s para 4.000 eventos |
| ☑️ | Multi-source: uma thread por origem |
| ☑️ | **Cascata** — Master → Slave01 → Slave03. O segundo salto custou 1.827 ms contra 1.679 do primeiro |
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

## 10. O que isto NÃO é

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
- **Não resolve conflito de escrita nos dois lados.** É um caminho só,
  Source → Réplica. Multi-master é outro problema.
- **Não substitui backup.** Réplica repete o `DELETE` errado que você fez no
  Source, e repete rápido.
- **Não há transação**, então não há ordem global entre tabelas a preservar —
  e é por isso que a posição é por tabela. Quando as transações entrarem, entra
  junto um número de sequência do database inteiro.

---

## 11. Como refazer a medição

```bash
cargo build --release
python3 bancada/replicacao/montar.py /tmp/phx-replicacao
python3 bancada/replicacao/medir.py 100000
```

`montar.py --cascata` põe o Slave03 puxando do Slave01. Detalhes e a última
corrida em `bancada/replicacao/LEIA-ME.md`.

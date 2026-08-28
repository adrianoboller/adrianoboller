# Replicação no PhxSql

**Pergunta:** dá para ter no PhxSql a replicação Source → Replica do MySQL(R)?

**Resposta curta:** dá, e o PhxSql já está a meio caminho — porque o `.log`
que você pediu **é exatamente o binlog**. O que falta é uma coisa só, e ela é
uma mudança de formato que vale fazer agora.

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
| Row-based binlog (imagem da linha) | **falta** | ver seção 3 |

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

## 3. A única peça que falta: a imagem da linha

O `.log` de hoje guarda 36 bytes por evento — carimbo, operação, rowid, versão,
usuário e CRC. Falta o conteúdo.

### Formato proposto (versão 2 do `.log`)

Cabeçalho do evento passa de 36 para **44 bytes**, e ganha um corpo:

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
| 36 | 4 | CRC-32 do cabeçalho e da imagem |
| 40 | 4 | reservado |
| 44 | N | **imagem da linha** |

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

Uma tabela com registro de 200 bytes passa a gastar ~244 bytes de diário por
alteração, em vez de 36. Por isso o `.log` já nasceu paginado, e por isso a
imagem fica atrás de um interruptor no `config.json`:

```json
"replicacao": { "imagem_da_linha": true }
```

Quem só quer auditoria deixa desligado e continua com 36 bytes por evento.
Quem quer replicar liga.

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

Três operações novas, no mesmo JSON Lines da porta 5000:

```json
{"token":"...","op":"posicao","database":"Z"}
{"ok":true,"resultado":{"cadastroClientes":1234,"pedidos":87}}

{"token":"...","op":"replicar","database":"Z","tabela":"cadastroClientes",
 "desde":1234,"max":500}
{"ok":true,"resultado":{"eventos":[...],"ate":1734,"fim":false}}

{"token":"...","op":"aplicar","database":"Z","tabela":"cadastroClientes",
 "eventos":[...]}
```

A réplica roda um laço: pergunta a posição, puxa em lotes, aplica, repete.
Quando o Source responde `"fim":true`, ela espera e pergunta de novo — ou
mantém a conexão aberta e o Source segura a resposta até ter novidade
(long-poll), que é o mais parecido com o binlog dump do MySQL(R).

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

## 9. Ordem de implementação

1. `.log` versão 2 com imagem da linha, atrás do interruptor no `config.json`
2. Ops `posicao` e `replicar` no Source
3. Laço da réplica: puxar, aplicar, conferir o rowid, repetir
4. Long-poll no Source, para a réplica não ficar perguntando à toa
5. Reconexão com espera crescente e retomada pela posição
6. Multi-source (o `config.json` já modela; falta uma thread por origem)
7. TLS no transporte — hoje o JSON vai em claro e depende do IPSec

## 10. O que isto NÃO vai ser

- **Não é replicação síncrona.** É assíncrona, como o padrão do MySQL(R): a
  réplica fica atrás do Source por algum tempo.
- **Não resolve conflito de escrita nos dois lados.** É um caminho só,
  Source → Réplica. Multi-master é outro problema.
- **Não substitui backup.** Réplica repete o `DELETE` errado que você fez no
  Source, e repete rápido.

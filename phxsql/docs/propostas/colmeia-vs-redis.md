# A colmeia (a hive do regedit) medida contra o Redis e o «Redis SQL»

**12/09/2026. Papel J (pesquisa), contra o nosso crivo.** Ordem do dono:
*«A lógica da colmeia é o que o regedit faz. Compare com o Redis SQL.»* Este
documento ratifica o desenho da colmeia (é a hive do Windows) e mede o Redis
contra o nosso gargalo e as nossas pétreas — antes de qualquer receita dele
virar plano. Fontes lidas na data em §6.

## 1. A ratificação: a colmeia É a hive do regedit

«Hive» não é metáfora — é o nome literal do arquivo do registro do Windows
(formato **REGF**). O que o regedit faz, e o que o `colmeia-estrutura.md` já
desenhou:

- **Árvore de chaves por caminho.** `HKLM\Software\App\Config` é um caminho de
  chaves aninhadas; cada chave tem **valores nomeados** (poucos por chave). É
  config-shaped: dado hierárquico, muitos nós rasos, valor pequeno.
- **Arquivo em blocos/células, mapeado em memória.** O REGF é `bins` de `cells`,
  lido por `mmap` — a leitura é toque de página, não `seek`+`read` por registro.
- **Escrita preguiçosa.** O registro **não faz `flush` a cada escrita**: um
  *lazy flusher* despeja o hive em disco a cada poucos segundos (salvo
  `RegFlushKey` explícito). Durabilidade trocada por velocidade — guarde este
  ponto, ele reaparece no Redis (§3).
- **Leitura domina, escrita é rara.** Configuração se lê a cada arranque de
  programa e se escreve quando alguém muda um ajuste. É o inverso do laço quente
  de um `INSERT`.

Este é o `TipoDatabase::Hive` — hoje proposta reservada (`motor_pronto()==false`).
O executor H está medindo agora, na máquina parada, se essa forma **lê
config-shaped mais rápido que o Padrão** (frente H1/H2). Este documento é o eixo
que falta: como o Redis resolve o mesmo problema, e o que disso passa pelo crivo.

## 2. O que o Redis é — e o que «Redis SQL» realmente é

**Redis** é um armazém **em memória** de chave→valor com estruturas ricas
(string, hash, list, set, sorted-set, stream), servidor em C, protocolo **RESP**,
com persistência **opcional**:

- **RDB** — retrato do dataset em intervalos (rápido de restaurar, perde o que
  veio depois do último retrato).
- **AOF** — log append de toda escrita, relido no arranque. `appendfsync` tem
  três modos: `always` (durável, `fsync` por comando), **`everysec` (padrão)** —
  `fsync` a cada 1 s numa thread de fundo, **≤1s de escrita em risco** — e `no`
  (deixa o SO decidir).

**«Redis SQL» não é uma coisa só** — e a desambiguação importa para a decisão:

| O que chamam de «Redis SQL» | O que é de fato | SQL padrão? |
|---|---|---|
| **Redis Query Engine** (ex-RediSearch, no core do Redis 8) | Índice secundário, full-text, vetorial, agregação — **linguagem própria** (`FT.SEARCH`) | **Não** |
| **RediSQL** (`redisql.com`) | Módulo que **embute o SQLite** dentro do Redis; ~130k tx/s, em memória | Sim (é o SQLite) |
| **redis-sql-trino** (Redis Field Engineering) | Conector **Trino** dando SQL sobre índices do Redis | Sim (via Trino) |
| **Apache Kvrocks + KQIR** | Redis-compatível **sobre disco** (RocksDB), motor de consulta que aceita SQL **e** RediSearch | Sim (parcial) |

Ou seja: **o Redis, sozinho, não tem SQL.** Ou você embute o SQLite (RediSQL),
ou usa uma linguagem que não é SQL (Query Engine), ou põe um motor de consulta
externo por cima (Trino, KQIR). Isso é o ponto que mais decide (§3, item 5).

**Licença (importa para «inspiração, não cópia»):** Redis 8 (mai/2025) é
**tri-licença** — RSALv2, SSPLv1 ou **AGPLv3** (esta é OSI open source; o antirez
voltou à Redis Inc. em nov/2024 e reabriu). RediSQL é produto à parte. Ler
qualquer um é livre e é **inspiração** — nenhum entra como dependência, porque
**zero dependências externas é pétrea** (§3, item 1).

## 3. Contra o nosso crivo — o que passa, o que a pétrea barra, onde já convergimos

1. **Zero dependências: nada disso se importa.** Redis é um servidor C à parte;
   RediSQL embute SQLite; o Query Engine é módulo C. Nenhum vira crate — é o
   mesmo caso do TLS: o **comportamento** (leitura de config muito rápida) pode
   ser meta; o **meio** (linkar Redis/SQLite) não passa sem virar código nosso,
   como o SHA-256. Inspiração, não cópia.

2. **Em memória × em disco — a armadilha da bancada.** O Redis é rápido porque
   **não toca disco na leitura** e **adia o `fsync`** na escrita. A colmeia
   (regedit) é **em disco**, mas `mmap`'d e de flush preguiçoso — mais perto do
   Redis que o nosso Padrão está. **Comparar latência crua colmeia × Redis mede
   o meio (RAM × page cache), não o desenho** — exatamente o erro que a pétrea
   «bancada compara trabalho igual» proíbe. O justo é comparar **padrão de
   acesso** (busca por caminho → busca por caminho) e **modelo de durabilidade**,
   não o número cru de um store em RAM contra um em disco.

3. **Durabilidade — a convergência que vira decisão do DBA.** O Redis default
   (`everysec`) **e** o registro do Windows (lazy flush) fazem a mesma troca:
   «≤1 s de escrita em risco» por velocidade. O nosso Padrão faz `fsync` por
   commit (durável). **Decisão de formato, do papel C/dono:** a colmeia herda o
   flush preguiçoso do regedit (rápida, ≤Xs em risco) ou o `fsync`-por-commit do
   Padrão (durável, mais lenta)? Não se decide calado — e a H3 do backlog existe
   justamente para medir os bytes-ao-disco do REGF real e fechar o «durável ≤
   preguiçosa».

4. **Integridade primordial — o lugar onde ela pode não valer.** O Redis não tem
   FK, nem pai-filho, nem «nunca mata o pai que tem filhos». Config-shaped não
   precisa: um ajuste não é filho de outro. **Decisão explícita, não
   esquecimento:** a colmeia é candidata legítima a NÃO carregar a regra
   primordial — porque não há filho —, mas isso se escreve no formato, do mesmo
   modo que o `ao_excluir só restringir` se escreve. Dispensa registrada, nunca
   silenciosa.

5. **SQL: aqui já estamos à FRENTE do Redis-puro.** O Redis precisa de bolt-on
   para falar SQL (SQLite embutido, ou Trino, ou KQIR); o PhxSql **já fala SQL**
   sobre disco, zero-dep — tradutor, ODBC, `JOIN`/`LEFT JOIN`, `GROUP BY`,
   subconsulta escalar, tudo escrito aqui. A colmeia entra como **sub-tipo
   KV/config sob um motor que já tem SQL** — o inverso do Redis, que é um KV
   tentando ganhar SQL por cima. Se o dono quer «Redis SQL», a leitura honesta é:
   o PhxSql é um SQL-sobre-disco que ganharia um **modo hive** para config, em
   vez de um KV que ganha SQL por fora.

**Onde já convergimos sem saber:** o `mmap`/toque-de-página do REGF é o mesmo
princípio do nosso cache de páginas do `.ndx` (o que comprou 2,40× no pedido
113); e o CRC por página, não por linha, já é o desenho do Cassandra. A colmeia
não inventa o acesso por página — ela o leva ao extremo config-shaped.

## 4. A premissa a medir — e o que já está sendo medido

*Medir a premissa do item vem antes de implementar o item.* Número do Redis
citado de memória (os «130k tx/s», o «sub-ms») é do desenho **deles**, no
gargalo **deles** (em memória, single-thread, sem `fsync` no caminho) — não vira
nosso plano sem medição nossa. O que se mede aqui:

- **(em andamento, H1/H2)** A colmeia-em-disco lê config-shaped N× mais rápido
  que o Padrão, comparando trabalho igual (busca de ponto × busca de ponto)? Se
  não, a premissa morre medida e o `TipoDatabase::Hive` não se justifica.
- **(a medir, H3)** Quanto custa o flush preguiçoso × o `fsync`-por-commit — o
  preço real da durabilidade frouxa que o regedit e o Redis escolheram.
- **(decisão, não medição)** Se a colmeia dispensa a integridade primordial, e
  se herda o flush preguiçoso — ambas de formato, ambas do dono/DBA.

## 5. Veredito preliminar (a fechar com o número do H)

A colmeia como **hive-em-disco config-shaped** é coerente com o regedit e com as
nossas pétreas. O Redis é a prova de que o **padrão de acesso** (caminho→valor,
leitura dominante, escrita adiável) tem mercado enorme — mas o Redis o compra com
três coisas que ou a pétrea barra, ou viram decisão do dono:

- **em memória + dependência** → barrado (zero-deps); a colmeia fica em disco.
- **durabilidade frouxa** (`everysec`) → decisão de formato, não default calado.
- **sem SQL nativo** → aqui o PhxSql já ganha: SQL sobre disco, zero-dep, com
  integridade no Padrão que o Redis não tem.

O que decide se a colmeia nasce é o número do H1/H2 (lê mais rápido que o
Padrão?), não a fama do Redis. Se a colmeia não bater o Padrão em config-shaped,
o modo hive morre medido — e ter perguntado ao Redis primeiro terá poupado o
formato.

## 6. Fontes lidas (12/09/2026)

- Redis Query Engine / RediSearch (core do Redis 8): `redis.io/resources/redis-query-engine/`, `github.com/RediSearch/RediSearch`.
- RediSQL (SQLite embutido, ~130k tx/s): `redisql.com`.
- redis-sql-trino: `github.com/redis-field-engineering/redis-sql-trino`.
- Kvrocks + KQIR (Redis-compatível sobre RocksDB, SQL + RediSearch): `kvrocks.apache.org/blog/kqir-query-engine/`.
- Licença Redis 8 (tri-licença AGPLv3/SSPL/RSALv2): `redis.io/blog/agplv3/`.
- Persistência RDB/AOF, `appendfsync everysec`: `redis.io/docs/latest/operate/oss_and_stack/management/persistence/`.
- Formato REGF / lazy flush do registro do Windows: `docs/propostas/colmeia-estrutura.md` (estudo desta casa).

# O que ainda falta no PhxSql, medido contra quem tem

Medido em **07/09/2026**, uma pergunta de cada vez, contra os motores que
estão **vivos nesta máquina**. Este documento não é o `COMPARACAO.md` (o que
os motores maduros têm e nós **trouxemos**) nem o `CONCORRENTES.md` (o caminho
de inserção deles, lido no fonte). É o outro lado: **o que continua faltando
aqui**, e quem já resolveu.

**18 de 19 capacidades** faltam ou estão pela metade no PhxSql. A única
inteira é um veredito de ausência que esta casa publicou **errado** — o sexto
— e a seção que o conta está abaixo.

> Refaça com `python3 bancada/comparativo/medir.py` e depois
> `python3 bancada/comparativo/documento.py`. **Este arquivo não se
> edita** — a prosa mora no gerador, e a medição, no `resultados.json`.

---

## 1. Como cada célula foi decidida

Três procedências, e a tabela diz qual em cada linha, porque misturá-las
publica um retrato que nunca existiu.

**Perguntando ao motor vivo** — 13 das 19 linhas. A mesma pergunta vai para os
quatro motores na língua de cada um; a instrução passou → tem, recusou → não,
e a mensagem de recusa fica guardada no JSON. Estas versões responderam:

| motor | versão que respondeu |
|---|---|
| PhxSql | `phxsqld 0.18.0 (b63856744b1f-sujo) x86_64-unknown-linux-gnu` |
| PostgreSQL(R) | `16.13 (Ubuntu 16.13-0ubuntu0.24.04.1)` |
| MySQL(R) | `8.0.46-0ubuntu0.24.04.3` |
| SQLite(R) | `3.45.1` |

**Sonda de código** — 6 linhas, só onde SQL não alcança. Cada uma aponta
**arquivo e linha**, e é por isso que ela vale: sonda que aponta para o
repositório se reconfere; sonda que resume, não.

**Citado**, para quem não está aqui:

- **HFSQL(R)** — docs/HFSQL.md, folha de 2013-10 que NÃO está nesta sessão
- **Cassandra(R)** — docs/CASSANDRA.md, fonte da 5.0.10 commit 7b5ab44, lido em sessão anterior

Citado não é mentira; é afirmação de **segunda mão**, e a tabela marca cada
uma dessas células para que ninguém a leia como medida.

### As 5 armadilhas que este medidor pagou

Cada uma virou um portão, e é por isso que estão escritas: portão sem a
história do defeito é portão que alguém remove por parecer exagero.

**1.** Perguntar ao **texto do repositório**. `materializar a linha`, num
comentário, virou «view materializada»; o `ON DUPLICATE KEY UPDATE` que o
DbLink **manda para o MySQL(R)** virou upsert nosso. Sonda por padrão de texto
acha o que não é — e as três células saíram erradas na primeira corrida.

→ Passou a perguntar ao **motor vivo**, e a sonda de código sobrou só onde SQL
não alcança.

**2.** Um portão que exigia **diversidade de resposta**: «um motor não pode
responder a mesma coisa para tudo». Ele reprovou o PostgreSQL(R), que
genuinamente tem todas as capacidades perguntadas por SQL — portão que
confunde motor completo com medidor quebrado.

→ Entrou um **controle positivo** no lugar: uma instrução inválida de
propósito que **todo** motor tem de recusar. Se algum aceitar, quem está
mentindo é o medidor.

**3.** **Duas linhas diziam `tem` por uma recusa.** Recusa não prova nada
sozinha: um índice que devolve zero linhas pode simplesmente não existir, e
uma tabela que recusa o valor proibido pode nunca ter nascido.

→ Cada sonda de efeito ganhou o **caso legítimo** ao lado do proibido. As duas
viraram `não`.

**4.** A sonda da visão lia `r["operacoes"]` do **envelope**, e a resposta vem
dentro de `resultado`. Ela publicou «nenhuma operação de visão entre as 0» —
um `não` certo pelo motivo errado, que é o pior tipo de certo. E o mesmo erro
de nível estava em mais três sondas irmãs.

→ Um só desembrulhador, e **um controle que o prova**: grava `v = 42` e exige
ler 42 de volta antes de qualquer sonda rodar.

**5.** As tabelas das sondas **nunca nasceram**. O índice ia como `[{"coluna":
0}]` e o servidor recusava com «índice pk sem colunas»; ninguém lia a recusa,
e as leituras seguintes achavam vazio e publicavam «o campo foi aceito e
IGNORADO».

→ O `cria()` passou a **exigir** que a tabela nasça, e as duas sondas em que a
recusa é a própria resposta pedem isso pelo nome (`exigir=False`).

---

## 2. A tabela

✅ tem &middot; ◐ pela metade &middot; ❌ não tem &middot; 📄 citado, não medido aqui &middot; — o verbo não existe neste motor

| capacidade | como | PhxSql | HFSQL(R) | PostgreSQL(R) | Cassandra(R) | MySQL(R) | SQLite(R) |
|---|---|---|---|---|---|---|---|
| Visão (`CREATE VIEW`) | SQL | ❌ | ✅ | ✅ | ❌ | ✅ | ✅ |
| Subconsulta e CTE (`WITH … AS`) | SQL | ❌ | 📄 | ✅ | ❌ | ✅ | ✅ |
| Subconsulta no `WHERE` | SQL | ❌ | 📄 | ✅ | ❌ | ✅ | ✅ |
| Função de janela (`OVER`) | SQL | ❌ | 📄 | ✅ | ❌ | ✅ | ✅ |
| `GROUP BY` com agregação | SQL | ❌ | ✅ | ✅ | ◐ | ✅ | ✅ |
| Expressão no `WHERE` (`preco * 1.1 > 100`) | SQL | ❌ | ✅ | ✅ | ❌ | ✅ | ✅ |
| Índice parcial (`CREATE INDEX … WHERE`) | SQL | ❌ | ✅ | ✅ | ❌ | ❌ | ✅ |
| Índice por expressão (`lower(nome)`) | SQL | ❌ | 📄 | ✅ | ❌ | ✅ | ✅ |
| Restrição `CHECK` | SQL | ❌ | 📄 | ✅ | ❌ | ✅ | ✅ |
| `DEFAULT` de coluna | SQL | ❌ | ✅ | ✅ | ❌ | ✅ | ✅ |
| Coluna calculada (`GENERATED ALWAYS AS`) | SQL | ❌ | ✅ | ✅ | ❌ | ✅ | ✅ |
| Upsert (`ON CONFLICT` / `ON DUPLICATE KEY`) | SQL | ❌ | 📄 | ✅ | ✅ | ✅ | ✅ |
| Nível de isolamento acima de `READ COMMITTED` | SQL | ❌ | ✅ | ✅ | ❌ | ✅ | — |
| Trava por linha nas transações | sonda | ✅ | ✅ | 📄 | ❌ | 📄 | 📄 |
| TLS no transporte | sonda | ❌ | ✅ | 📄 | ✅ | 📄 | 📄 |
| Direito por COLUNA | sonda | ❌ | ✅ | 📄 | ❌ | 📄 | 📄 |
| Recuperação a um ponto no tempo (PITR) | sonda | ❌ | 📄 | 📄 | ✅ | 📄 | 📄 |
| Parâmetro em instrução preparada (`?`) | sonda | ❌ | ✅ | 📄 | ✅ | 📄 | 📄 |
| Dizer ONDE duas tabelas diferem | sonda | ◐ | ✅ | 📄 | ◐ | 📄 | 📄 |

Somando as colunas. As marcas de procedência entram na conta em vez de sumir
dela — um ✅ citado e um ✅ medido valem coisas diferentes, e misturá-los numa
coluna só é exatamente o que esta tabela recusa fazer:

| motor | ✅ | ◐ | ❌ | 📄 | — | soma | procedência |
|---|---|---|---|---|---|---|---|
| PhxSql | 1 | 1 | 17 | 0 | 0 | 19 | medido aqui |
| HFSQL(R) | 12 | 0 | 0 | 7 | 0 | 19 | citado |
| PostgreSQL(R) | 13 | 0 | 0 | 6 | 0 | 19 | medido aqui |
| Cassandra(R) | 4 | 2 | 13 | 0 | 0 | 19 | citado |
| MySQL(R) | 12 | 0 | 1 | 6 | 0 | 19 | medido aqui |
| SQLite(R) | 12 | 0 | 0 | 6 | 1 | 19 | medido aqui |

A coluna **soma** existe para a linha fechar em 19: sem ela, uma marca
esquecida some da conta e o leitor não tem como perceber.

---

## 3. O que o SQL vivo respondeu

Cada recusa abaixo é o texto que o próprio motor devolveu, sem retoque. Onde
ela aparece cortada, é porque o medidor guarda só o começo da mensagem — o
resto é o manual do outro motor.

### Visão (`CREATE VIEW`) &mdash; ❌

> nenhuma operacao de visao entre as 119 que o catalogo lista

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) ✅, citado: a folha lista visões.
Cassandra(R) ❌, citado: não há VIEW; há materialized view, com ressalva do próprio projeto.

### Subconsulta e CTE (`WITH … AS`) &mdash; ❌

> [SP000018] esquema invalido: SQL, coluna 1: WITH nao e um comando desta camada

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) 📄, citado: não apurado na folha.
Cassandra(R) ❌, citado: CQL não tem subconsulta nem CTE.

### Subconsulta no `WHERE` &mdash; ❌

> [SP000018] esquema invalido: SQL, coluna 26: IN e uma lista de buscas; o motor faz cada um

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) 📄, citado: não apurado na folha.
Cassandra(R) ❌, citado: CQL não tem subconsulta.

### Função de janela (`OVER`) &mdash; ❌

> [SP000018] esquema invalido: SQL, coluna 18: esperava FROM, e veio "("

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) 📄, citado: não apurado na folha.
Cassandra(R) ❌, citado: CQL não tem função de janela.

### `GROUP BY` com agregação &mdash; ❌

> [SP000018] esquema invalido: SQL, coluna 21: esperava FROM, e veio "("

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) ✅, citado: SQL completo na folha.
Cassandra(R) ◐, citado: GROUP BY só pelo prefixo da chave de partição.

### Expressão no `WHERE` (`preco * 1.1 > 100`) &mdash; ❌

> [SP000018] esquema invalido: SQL, coluna 29: esperava um comparador (=, <>, <, <=, >, >=)

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) ✅, citado: SQL completo na folha.
Cassandra(R) ❌, citado: o WHERE do CQL é sobre a chave, sem expressão.

### Índice parcial (`CREATE INDEX … WHERE`) &mdash; ❌

> o campo `onde` foi aceito e IGNORADO: devolveu 1

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ❌ &middot; SQLite(R) ✅.
HFSQL(R) ✅, citado: a folha lista índice parcial.
Cassandra(R) ❌, citado: índice secundário existe; parcial não.

### Índice por expressão (`lower(nome)`) &mdash; ❌

> o indice aceita NOME de coluna e recusa expressao: [SP000018] esquema invalido: indice usa coluna inexistente: "lower(nom

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) 📄, citado: não apurado.
Cassandra(R) ❌, citado: não há.

### Restrição `CHECK` &mdash; ❌

> o campo `check` foi aceito e IGNORADO: gravou v = -5

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) 📄, citado: não apurado.
Cassandra(R) ❌, citado: não há.

### `DEFAULT` de coluna &mdash; ❌

> o campo `padrao` foi aceito e IGNORADO: v = None

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) ✅, citado: valor padrão no dicionário de dados.
Cassandra(R) ❌, citado: não há DEFAULT em CQL.

### Coluna calculada (`GENERATED ALWAYS AS`) &mdash; ❌

> o campo `calculada` foi aceito e IGNORADO: b = None

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) ✅, citado: item calculado no dicionário.
Cassandra(R) ❌, citado: não há.

### Upsert (`ON CONFLICT` / `ON DUPLICATE KEY`) &mdash; ❌

> [SP000018] esquema invalido: SQL, coluna 1: INSERT ainda nao existe nesta camada -- so SEL

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) 📄, citado: não apurado.
Cassandra(R) ✅, citado: TODO INSERT é upsert: ele não lê antes de gravar.

### Nível de isolamento acima de `READ COMMITTED` &mdash; ❌

> [SP000018] esquema invalido: SQL, coluna 1: SET nao e um comando desta camada

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) —.
HFSQL(R) ✅, citado: a folha anuncia quatro níveis.
Cassandra(R) ❌, citado: não há transação multi-linha; há LWT por partição.

---

## 4. O que a sonda de código achou

Perguntas que SQL não responde, ou porque o verbo não existe em nenhum dos
dialetos, ou porque a resposta está na configuração e não na instrução. Cada
veredito abaixo nomeia onde olhar.

### Trava por linha nas transações &mdash; ✅

> gestor em crates/phxsql-server/src/travas.rs:1; ligado em crates/phxsql-server/src/servidor.rs:573; pedida em crates/phxsql-server/src/servidor.rs:8649

*Vale DENTRO de transação: `esperar_trava` recusa com «sem transação» quem a
pede fora dela, e aí a trava GLOBAL de dados serializa como antes.*

HFSQL(R) ✅, citado: trava por linha automática, na folha.
Cassandra(R) ❌, citado: não há trava: o conflito se resolve por carimbo de hora.

### TLS no transporte &mdash; ❌

> crates/phxsql-server/src/rest.rs:578 — «TLS aqui: a saida honesta e um proxy que termine TLS na»

*A cifra da porta de DADOS é própria (aperto estilo Noise) e existe; o que não
existe é TLS, que exigiria crate — e zero dependências é pétrea.*

HFSQL(R) ✅, citado: a folha anuncia canal cifrado.
Cassandra(R) ✅, citado: client_encryption_options no cassandra.yaml.

### Direito por COLUNA &mdash; ❌

> o portão lê `tabela`, e para aí: crates/phxsql-server/src/usuarios.rs:101

*O direito por tabela existe desde o pedido 124.*

HFSQL(R) ✅, citado: direito por coluna na folha.
Cassandra(R) ❌, citado: GRANT vai até a tabela.

### Recuperação a um ponto no tempo (PITR) &mdash; ❌

> o backup é cópia inteira; o `.log` guarda o evento mas não há quem o reaplique até um instante

*O `.log` por tabela É o diário que um PITR usaria.*

HFSQL(R) 📄, citado: não apurado.
Cassandra(R) ✅, citado: commitlog + snapshot; é o desenho dele.

### Parâmetro em instrução preparada (`?`) &mdash; ❌

> crates/phxsql-odbc/src/lib.rs:617 — «Preparar aqui e guardar o texto: nao ha parametros nem plano no driver, e»

*O driver guarda o texto e o reenvia.*

HFSQL(R) ✅, citado: consulta parametrizada é o normal do WLangage.
Cassandra(R) ✅, citado: prepared statement com bind é o caminho normal.

### Dizer ONDE duas tabelas diferem &mdash; ◐

> o `checksum` diz SE diferem: crates/phxsql-server/src/catalogo.rs:516

*Falta a operação que devolve as linhas divergentes.*

HFSQL(R) ✅, citado: WDHFDiff compara estrutura e dados.
Cassandra(R) ◐, citado: reparo por árvore de Merkle diz o intervalo, não a linha.

---

## 5. O único ✅ do PhxSql, e por que ele é uma lição

**Trava por linha nas transações** — gestor em crates/phxsql-
server/src/travas.rs:1; ligado em crates/phxsql-server/src/servidor.rs:573;
pedida em crates/phxsql-server/src/servidor.rs:8649. Vale DENTRO de transação:
`esperar_trava` recusa com «sem transação» quem a pede fora dela, e aí a trava
GLOBAL de dados serializa como antes.

Esta linha entrou nesta tabela como `não`, escrita de memória, e a sonda de
código a derrubou. Ela é o **sexto** veredito de ausência que esta casa
publicou errado — os cinco anteriores estão na §6 do `HFSQL.md`, e o padrão
dos seis é o mesmo: **ninguém reconfere uma ausência, porque não há o que
olhar.** Um número errado alguém desconfia ao bater o olho; um «não há» fica.

É por isso que este documento sai de um medidor e não de uma leitura:
**veredito de ausência se remede por data, não por suspeita.**

---

## 6. O achado que a pergunta não pedia: campo engolido

**4 campos de esquema desconhecidos são aceitos pelo `criar_tabela` e não
fazem nada.** A tabela nasce, o campo some, e quem escreveu o pedido acha que
declarou uma garantia:

- **Índice parcial (`CREATE INDEX … WHERE`)** — o campo `onde` foi aceito e IGNORADO: devolveu 1
- **Restrição `CHECK`** — o campo `check` foi aceito e IGNORADO: gravou v = -5
- **`DEFAULT` de coluna** — o campo `padrao` foi aceito e IGNORADO: v = None
- **Coluna calculada (`GENERATED ALWAYS AS`)** — o campo `calculada` foi aceito e IGNORADO: b = None

É a mesma lei que o `recursos.cache_paginas` já custou uma vez: **configuração
que não é lida mente**, e campo de esquema sem leitor é pior que campo ausente
— o ausente o servidor recusa, e quem pediu descobre na hora.

E não é falta de rigor geral: na mesma corrida, o índice **recusou** a coluna
inexistente `lower(nome)` com o nome do problema. O motor confere o que ele
conhece e cala sobre o que não conhece — a assimetria é entre saber recusar e
saber que havia algo a recusar.

Este medidor pagou o preço disso na própria carne: a primeira corrida criou as
tabelas com o índice no formato errado, o `criar_tabela` recusou, ninguém leu
a recusa, e as sondas seguintes publicaram «o campo foi aceito e IGNORADO»
sobre tabelas que nunca existiram. Hoje o `cria()` **exige** que a tabela
nasça, e as duas sondas em que a recusa é a própria resposta dizem isso no
nome.

---

## 7. As linhas pela metade

Meia capacidade não é meio caminho andado — é o caminho que PARECE andado, e
por isso ela ganha marca própria em vez de cair para o lado que der jeito:

- **Dizer ONDE duas tabelas diferem** — o `checksum` diz SE diferem: crates/phxsql-server/src/catalogo.rs:516. Falta a operação que devolve as linhas divergentes.

---

## 8. O que esta tabela NÃO diz

Três honestidades, e as três mudam como se lê o resto:

1. **Faltar não é o mesmo que estar errado.** Boa parte destas
   ausências é sequência, não esquecimento: sem nível de isolamento
   acima de `READ COMMITTED` não adianta afinar trava, e sem `INSERT`
   na camada SQL não há upsert para pedir. A ordem está no
   `docs/PENDENCIAS.md`.
2. **Duas colunas são de segunda mão.** 2 motores não
   estão nesta máquina; o que a tabela diz deles saiu de leitura
   anterior, e está marcado 📄 célula a célula. Vantagem nossa contra
   folha velha não é vantagem provada contra o produto de hoje.
3. **Ter não é ter bem.** A tabela pergunta se a instrução passa, não
   se ela é rápida nem se o plano é bom. Quem responde por custo é a
   `bancada/`, e ela mede outra coisa.

---

## 9. Como remedir

```bash
python3 bancada/comparativo/medir.py       # pergunta aos motores vivos
python3 bancada/comparativo/documento.py   # reescreve este arquivo
```

O medidor sobe o PhxSql sozinho e usa os `mysql`, `psql` e `sqlite3` da
máquina. Se algum não estiver no `PATH`, ele **para** em vez de publicar uma
coluna vazia como se fosse ausência — que seria exatamente o erro que este
documento existe para não repetir.

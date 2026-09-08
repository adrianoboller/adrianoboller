# O que ainda falta no PhxSql, medido contra quem tem

Medido em **08/09/2026**, uma pergunta de cada vez, contra os motores que
estão **vivos nesta máquina**. Este documento não é o `COMPARACAO.md` (o que
os motores maduros têm e nós **trouxemos**) nem o `CONCORRENTES.md` (o caminho
de inserção deles, lido no fonte). É o outro lado: **o que continua faltando
aqui**, e quem já resolveu.

**3 de 19 capacidades** faltam ou estão pela metade no PhxSql. A única inteira
é um veredito de ausência que esta casa publicou **errado** — o sexto — e a
seção que o conta está abaixo.

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
| PhxSql | `phxsqld 0.18.0 (51620a0e998f-sujo) x86_64-unknown-linux-gnu` |
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
| Visão (`CREATE VIEW`) | SQL | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ |
| Subconsulta e CTE (`WITH … AS`) | SQL | ✅ | 📄 | ✅ | ❌ | ✅ | ✅ |
| Subconsulta no `WHERE` | SQL | ✅ | 📄 | ✅ | ❌ | ✅ | ✅ |
| Função de janela (`OVER`) | SQL | ✅ | 📄 | ✅ | ❌ | ✅ | ✅ |
| `GROUP BY` com agregação | SQL | ✅ | ✅ | ✅ | ◐ | ✅ | ✅ |
| Expressão no `WHERE` (`preco * 1.1 > 100`) | SQL | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ |
| Índice parcial (`CREATE INDEX … WHERE`) | SQL | ✅ | ✅ | ✅ | ❌ | ❌ | ✅ |
| Índice por expressão (`lower(nome)`) | SQL | ◐ | 📄 | ✅ | ❌ | ✅ | ✅ |
| Restrição `CHECK` | SQL | ✅ | 📄 | ✅ | ❌ | ✅ | ✅ |
| `DEFAULT` de coluna | SQL | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ |
| Coluna calculada (`GENERATED ALWAYS AS`) | SQL | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ |
| Upsert (`ON CONFLICT` / `ON DUPLICATE KEY`) | SQL | ✅ | 📄 | ✅ | ✅ | ✅ | ✅ |
| Nível de isolamento acima de `READ COMMITTED` | SQL | ❌ | ✅ | ✅ | ❌ | ✅ | — |
| Trava por linha nas transações | sonda | ✅ | ✅ | 📄 | ❌ | 📄 | 📄 |
| TLS no transporte | sonda | ❌ | ✅ | 📄 | ✅ | 📄 | 📄 |
| Direito por COLUNA | sonda | ✅ | ✅ | 📄 | ❌ | 📄 | 📄 |
| Recuperação a um ponto no tempo (PITR) | sonda | ✅ | 📄 | 📄 | ✅ | 📄 | 📄 |
| Parâmetro em instrução preparada (`?`) | sonda | ✅ | ✅ | 📄 | ✅ | 📄 | 📄 |
| Dizer ONDE duas tabelas diferem | sonda | ✅ | ✅ | 📄 | ◐ | 📄 | 📄 |

Somando as colunas. As marcas de procedência entram na conta em vez de sumir
dela — um ✅ citado e um ✅ medido valem coisas diferentes, e misturá-los numa
coluna só é exatamente o que esta tabela recusa fazer:

| motor | ✅ | ◐ | ❌ | 📄 | — | soma | procedência |
|---|---|---|---|---|---|---|---|
| PhxSql | 16 | 1 | 2 | 0 | 0 | 19 | medido aqui |
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

### Visão (`CREATE VIEW`) &mdash; ✅

> CREATE VIEW v_c AS SELECT * FROM c e SELECT * FROM v_c devolveram as 1 linha(s) de `c`

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) ✅, citado: a folha lista visões.
Cassandra(R) ❌, citado: não há VIEW; há materialized view, com ressalva do próprio projeto.

### Subconsulta e CTE (`WITH … AS`) &mdash; ✅

> aceitou

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) 📄, citado: não apurado na folha.
Cassandra(R) ❌, citado: CQL não tem subconsulta nem CTE.

### Subconsulta no `WHERE` &mdash; ✅

> aceitou

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) 📄, citado: não apurado na folha.
Cassandra(R) ❌, citado: CQL não tem subconsulta.

### Função de janela (`OVER`) &mdash; ✅

> aceitou

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) 📄, citado: não apurado na folha.
Cassandra(R) ❌, citado: CQL não tem função de janela.

### `GROUP BY` com agregação &mdash; ✅

> aceitou

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) ✅, citado: SQL completo na folha.
Cassandra(R) ◐, citado: GROUP BY só pelo prefixo da chave de partição.

### Expressão no `WHERE` (`preco * 1.1 > 100`) &mdash; ✅

> aceitou

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) ✅, citado: SQL completo na folha.
Cassandra(R) ❌, citado: o WHERE do CQL é sobre a chave, sem expressão.

### Índice parcial (`CREATE INDEX … WHERE`) &mdash; ✅

> guardou a incluida e nao a filtrada

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ❌ &middot; SQLite(R) ✅.
HFSQL(R) ✅, citado: a folha lista índice parcial.
Cassandra(R) ❌, citado: índice secundário existe; parcial não.

### Índice por expressão (`lower(nome)`) &mdash; ◐

> criou -- conferir o que guardou

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) 📄, citado: não apurado.
Cassandra(R) ❌, citado: não há.

### Restrição `CHECK` &mdash; ✅

> recusou -5 e aceitou 5

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) 📄, citado: não apurado.
Cassandra(R) ❌, citado: não há.

### `DEFAULT` de coluna &mdash; ✅

> a linha nasceu com 7

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) ✅, citado: valor padrão no dicionário de dados.
Cassandra(R) ❌, citado: não há DEFAULT em CQL.

### Coluna calculada (`GENERATED ALWAYS AS`) &mdash; ✅

> b saiu 6

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) ✅, citado: item calculado no dicionário.
Cassandra(R) ❌, citado: não há.

### Upsert (`ON CONFLICT` / `ON DUPLICATE KEY`) &mdash; ✅

> aceitou

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) ✅.
HFSQL(R) 📄, citado: não apurado.
Cassandra(R) ✅, citado: TODO INSERT é upsert: ele não lê antes de gravar.

### Nível de isolamento acima de `READ COMMITTED` &mdash; ❌

> [SP000018] esquema invalido: SQL, coluna 1: SET TRANSACTION ISOLATION LEVEL SERIALIZABLE n

Nos vivos: PostgreSQL(R) ✅ &middot; MySQL(R) ✅ &middot; SQLite(R) —.
HFSQL(R) ✅, citado: a folha anuncia quatro níveis.
Cassandra(R) ❌, citado: não há transação multi-linha; há LWT por partição.

---

## 4. O que a sonda de código achou

Perguntas que SQL não responde, ou porque o verbo não existe em nenhum dos
dialetos, ou porque a resposta está na configuração e não na instrução. Cada
veredito abaixo nomeia onde olhar.

### Trava por linha nas transações &mdash; ✅

> gestor em crates/phxsql-server/src/travas.rs:1; ligado em crates/phxsql-server/src/servidor.rs:669; pedida em crates/phxsql-server/src/servidor.rs:12174

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

### Direito por COLUNA &mdash; ✅

> o varrer de ana (salario negado) veio SEM a coluna; o de bea (controle, mesmo cadastro sem `colunas`) veio COM ela

*O direito por TABELA existe desde o pedido 124; o cadastro e o molde de
`testes_direito_por_coluna` em servidor.rs e do MANUAL 14.3.2.*

HFSQL(R) ✅, citado: direito por coluna na folha.
Cassandra(R) ❌, citado: GRANT vai até a tabela.

### Recuperação a um ponto no tempo (PITR) &mdash; ✅

> `ate`=corte devolveu [(1, 'um alterado'), (2, 'dois')] (a 1 alterada, a 2, nada da 3); reaplicados=2, pulados=1

*Sequência de docs/RESTAURACAO.md § 7.10 -- a mesma que bancada/pitr/provar.py
roda com 22 conferências.*

HFSQL(R) 📄, citado: não apurado.
Cassandra(R) ✅, citado: commitlog + snapshot; é o desenho dele.

### Parâmetro em instrução preparada (`?`) &mdash; ✅

> `WHERE id = ?` com `parametros:[1]` devolveu a linha 1; sem `parametros` recusou: [SP000018] esquema invalido: SQL, coluna 1: vieram 0 parametros e o co

*O driver ODBC já liga e manda `parametros` desde o `SQLBindParameter`; a
promoção que faltava era o servidor ler `parametros` na op `sql`, medida aqui
pelo soquete.*

HFSQL(R) ✅, citado: consulta parametrizada é o normal do WLangage.
Cassandra(R) ✅, citado: prepared statement com bind é o caminho normal.

### Dizer ONDE duas tabelas diferem &mdash; ✅

> `diferencas` nomeou a chave [1] e a coluna `nome`: [{'chave': [1], 'colunas': ['nome'], 'a': {'id': 1, 'nome': 'um', 'softdeleted': False, 'rownum': 1}, 'b': {'id': 1, 'nome': 'dois', 'softdeleted': False, 'rownum': 1}}]

*O `checksum` dizia SE diferem; a op `diferencas` é o terceiro irmão da
conferência própria, junto de `juntar` e `unir`.*

HFSQL(R) ✅, citado: WDHFDiff compara estrutura e dados.
Cassandra(R) ◐, citado: reparo por árvore de Merkle diz o intervalo, não a linha.

---

## 5. O que passou a responder `tem`, e por que a lição continua

Até 07/09/2026 esta seção listava **um** `tem` só — a trava por linha — e o
título dizia «único» porque era. **16** das **19** linhas responderam `tem`
nesta remedição (as dezoito do comparativo entraram por contrato, medidas
contra o motor vivo em vez de digitadas): a maioria porque o motor GANHOU a
capacidade nesta rodada, e quatro — coluna, PITR, parâmetro e diferenças —
porque a SONDA deixou de ser código e passou a exercitar o EFEITO pelo
soquete, com o controle na mesma corrida.

**Visão (`CREATE VIEW`)** — CREATE VIEW v_c AS SELECT * FROM c e SELECT * FROM
v_c devolveram as 1 linha(s) de `c`.

**Subconsulta e CTE (`WITH … AS`)** — aceitou.

**Subconsulta no `WHERE`** — aceitou.

**Função de janela (`OVER`)** — aceitou.

**`GROUP BY` com agregação** — aceitou.

**Expressão no `WHERE` (`preco * 1.1 > 100`)** — aceitou.

**Índice parcial (`CREATE INDEX … WHERE`)** — guardou a incluida e nao a
filtrada.

**Restrição `CHECK`** — recusou -5 e aceitou 5.

**`DEFAULT` de coluna** — a linha nasceu com 7.

**Coluna calculada (`GENERATED ALWAYS AS`)** — b saiu 6.

**Upsert (`ON CONFLICT` / `ON DUPLICATE KEY`)** — aceitou.

**Trava por linha nas transações** — gestor em crates/phxsql-
server/src/travas.rs:1; ligado em crates/phxsql-server/src/servidor.rs:669;
pedida em crates/phxsql-server/src/servidor.rs:12174. Vale DENTRO de
transação: `esperar_trava` recusa com «sem transação» quem a pede fora dela, e
aí a trava GLOBAL de dados serializa como antes.

**Direito por COLUNA** — o varrer de ana (salario negado) veio SEM a coluna; o
de bea (controle, mesmo cadastro sem `colunas`) veio COM ela. O direito por
TABELA existe desde o pedido 124; o cadastro e o molde de
`testes_direito_por_coluna` em servidor.rs e do MANUAL 14.3.2.

**Recuperação a um ponto no tempo (PITR)** — `ate`=corte devolveu [(1, 'um
alterado'), (2, 'dois')] (a 1 alterada, a 2, nada da 3); reaplicados=2,
pulados=1. Sequência de docs/RESTAURACAO.md § 7.10 -- a mesma que
bancada/pitr/provar.py roda com 22 conferências.

**Parâmetro em instrução preparada (`?`)** — `WHERE id = ?` com
`parametros:[1]` devolveu a linha 1; sem `parametros` recusou: [SP000018]
esquema invalido: SQL, coluna 1: vieram 0 parametros e o co. O driver ODBC já
liga e manda `parametros` desde o `SQLBindParameter`; a promoção que faltava
era o servidor ler `parametros` na op `sql`, medida aqui pelo soquete.

**Dizer ONDE duas tabelas diferem** — `diferencas` nomeou a chave [1] e a
coluna `nome`: [{'chave': [1], 'colunas': ['nome'], 'a': {'id': 1, 'nome':
'um', 'softdeleted': False, 'rownum': 1}, 'b': {'id': 1, 'nome': 'dois',
'softdeleted': False, 'rownum': 1}}]. O `checksum` dizia SE diferem; a op
`diferencas` é o terceiro irmão da conferência própria, junto de `juntar` e
`unir`.

A **trava por linha** é quem abriu esta seção, e a lição dela continua valendo
sozinha: entrou nesta tabela como `não`, escrita de memória, e uma sonda de
código a derrubou. Foi o **sexto** veredito de ausência que esta casa publicou
errado — os cinco anteriores estão na §6 do `HFSQL.md` — e o padrão dos seis é
o mesmo: **ninguém reconfere uma ausência, porque não há o que olhar.** Um
número errado alguém desconfia ao bater o olho; um «não há» fica.

É por isso que este documento sai de um medidor e não de uma leitura:
**veredito de ausência se remede por data, não por suspeita** — e, como o
título desta seção acabou de provar, **veredito de unicidade também.**

---

## 6. As linhas pela metade

Meia capacidade não é meio caminho andado — é o caminho que PARECE andado, e
por isso ela ganha marca própria em vez de cair para o lado que der jeito:

- **Índice por expressão (`lower(nome)`)** — criou -- conferir o que guardou.

---

## 7. O que esta tabela NÃO diz

Três honestidades, e as três mudam como se lê o resto:

1. **Faltar não é o mesmo que estar errado.** Boa parte destas
   ausências é sequência, não esquecimento: sem nível de isolamento
   acima de `READ COMMITTED` não adianta afinar trava, e sem
   subconsulta correlacionada não há `EXISTS` para pedir. A ordem
   está no `docs/PENDENCIAS.md`.
2. **Duas colunas são de segunda mão.** 2 motores não
   estão nesta máquina; o que a tabela diz deles saiu de leitura
   anterior, e está marcado 📄 célula a célula. Vantagem nossa contra
   folha velha não é vantagem provada contra o produto de hoje.
3. **Ter não é ter bem.** A tabela pergunta se a instrução passa, não
   se ela é rápida nem se o plano é bom. Quem responde por custo é a
   `bancada/`, e ela mede outra coisa.

---

## 8. Como remedir

```bash
python3 bancada/comparativo/medir.py       # pergunta aos motores vivos
python3 bancada/comparativo/documento.py   # reescreve este arquivo
```

O medidor sobe o PhxSql sozinho e usa os `mysql`, `psql` e `sqlite3` da
máquina. Se algum não estiver no `PATH`, ele **para** em vez de publicar uma
coluna vazia como se fosse ausência — que seria exatamente o erro que este
documento existe para não repetir.

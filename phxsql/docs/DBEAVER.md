# DBeaver: o que dá para reaproveitar, legalmente e tecnicamente

**Pergunta:** dá para usar o DBeaver como interface de administração do PhxSql,
e o que do código dele pode ser reaproveitado?

**Resposta curta:** o **código**, quase nada — e não por licença, por
arquitetura. A **interface**, inteira — e por um caminho que não passa pelo
código dele.

---

## 1. A licença, primeiro, porque ela decide o resto

O DBeaver Community é **Apache 2.0**. É uma licença permissiva: dá para copiar,
modificar e redistribuir, inclusive em produto fechado, desde que se preserve o
aviso de copyright, se diga o que foi modificado e se entregue a cópia da
licença. Não há contaminação — Apache 2.0 não obriga a abrir o que você
escreveu em volta.

Mas há duas ressalvas que importam mais do que a licença:

1. **O DBeaver CE é uma aplicação Eclipse RCP, em Java.** Reaproveitar «o
   código» significaria trazer o Eclipse Platform, SWT, JFace e o modelo de
   plugins OSGi. Num projeto cuja regra número um é **zero dependência
   externa**, isso não é uma decisão de licença — é a negação do projeto.
2. **Nem tudo no repositório é Apache 2.0.** O repositório do DBeaver hospeda
   também partes com licenciamento próprio, e os *drivers* JDBC de cada banco
   têm cada um a sua licença, muitas vezes incompatível com redistribuição.
   Copiar «o driver de X que está lá dentro» é onde o problema apareceria.

**Conclusão sobre reaproveitar código: não vale.** O que vale é reaproveitar a
**arquitetura de extensão** — que é ideia, e ideia não tem licença.

---

## 2. Os três caminhos, do mais barato ao mais caro

```
                         ┌──────────────┐
                         │  DBeaver CE  │
                         └──────┬───────┘
                                │ JDBC
              ┌─────────────────┼──────────────────┐
              │                 │                  │
      (A) driver JDBC    (B) protocolo do     (C) plugin nativo
          fino sobre         PostgreSQL(R)        do DBeaver
          o JSON             no phxsqld
              │                 │                  │
        ~2.000 linhas     ~4.000 linhas      ~10.000 linhas
           de Java         de Rust            de Java/Eclipse
```

### (A) Um driver JDBC fino — o caminho recomendado

Um `.jar` que implementa `java.sql.Driver` e traduz JDBC para o JSON por linha
que o PhxSql já fala. É **fora** do repositório do PhxSql (é Java), e não
quebra a regra de dependência zero do motor.

O DBeaver aceita driver genérico: aponta-se o `.jar`, a classe e a URL
(`jdbc:phxsql://host:5000/base`). Com isso funcionam de imediato: a árvore de
objetos, a grade de dados editável, a exportação, a importação e a busca de
metadados.

**O que ele precisaria implementar, e o que já existe do lado do PhxSql:**

| JDBC pede | PhxSql tem |
|---|---|
| `getTables()` | `sistabelas` |
| `getColumns()` | `siscolunas` / `esquema` |
| `getPrimaryKeys()`, `getIndexInfo()` | `esquema` (índices, primária, composta) |
| `getImportedKeys()` (FKs) | `esquema` (chaves estrangeiras declaradas) |
| `ResultSet` paginado | `varrer` com cursor **e** salto por posição |
| `INSERT`/`UPDATE`/`DELETE` | `inserir`, `inserir_lote`, `atualizar`, `excluir` |
| `Statement.executeQuery(sql)` | **nada** — não há SQL |

**A última linha é a dificuldade toda**, e ela é honesta: o DBeaver é um cliente
de SQL. Sem SQL, o driver entrega tudo menos o Editor SQL. Ainda assim entrega
muito — a grade, a estrutura, o diagrama e a exportação valem sozinhas.

### (B) Falar o protocolo de fio do PostgreSQL(R) — o caminho ambicioso

O `phxsqld` abriria uma porta a mais (5432) falando o *wire protocol* v3 do
PostgreSQL(R). Aí o DBeaver — e o `psql`, e o driver JDBC do Postgres, e o
Metabase, e o Power BI — enxergariam o PhxSql como um Postgres.

O protocolo em si é **modesto**: mensagens com tipo de um byte e comprimento,
autenticação (dá para usar só `cleartext` ou SCRAM), `Query`, `Parse/Bind/
Execute`, `RowDescription`, `DataRow`, `CommandComplete`. Escrevê-lo em Rust
sem crate nenhuma é perfeitamente viável — o projeto já escreveu o protocolo do
MySQL(R) do lado cliente, no DbLink.

**O problema não é o protocolo. É o que vem por cima dele:** o cliente vai
mandar `SELECT`, e vai mandar as consultas de catálogo do Postgres
(`pg_catalog.pg_class`, `pg_attribute`, `information_schema`) para descobrir o
que existe. Sem uma camada SQL — nem que seja um subconjunto — a porta responde
o *handshake* e trava na primeira consulta.

Ou seja: **(B) não é um atalho para (A); (B) exige o SQL que (A) também
exige.** A diferença é que (B) entrega dezenas de ferramentas de uma vez, e (A)
entrega uma.

### (C) Plugin nativo do DBeaver

Um `org.jkiss.dbeaver.ext.phxsql` de verdade, com o modelo de metadados, os
editores e os ícones. É o que dá a melhor experiência, e é o mais caro: exige
conhecer o modelo de extensão do Eclipse. Só faz sentido depois de (A) existir
e ser usado.

---

## 3. O que eu faria, e em que ordem

1. **Um subconjunto de SQL**, dentro do PhxSql, em Rust: `SELECT` com `WHERE`,
   `ORDER BY`, `LIMIT`/`OFFSET` e `JOIN`; `INSERT`, `UPDATE`, `DELETE`;
   `CREATE`/`DROP TABLE`. É o pré-requisito de tudo o mais, e já está em
   `docs/PENDENCIAS.md` como «camada SQL».
2. **Driver JDBC fino** sobre o JSON que já existe (caminho A). Sem SQL ele já
   entrega a grade e a estrutura; com SQL, entrega o editor.
3. **Porta com o protocolo do PostgreSQL(R)** (caminho B), reaproveitando o
   mesmo executor SQL do passo 1. É aqui que o retorno explode: um trabalho, e
   o PhxSql aparece em toda ferramenta que fala Postgres.
4. Plugin nativo (C), se e quando alguém pedir.

**O que muda a ordem:** se a prioridade for *administrar*, o Centro de Controle
que já existe faz quase tudo o que o DBeaver faria, e o passo 2 é dispensável.
Se a prioridade for *ser adotado por quem já tem ferramenta*, o passo 3 é o que
importa e o 2 é desvio.

---

## 4. O que copiar do DBeaver sem copiar código

Ideias que valem, e que não custam licença nenhuma:

- **A grade que edita como planilha**, com a alteração virando comando só no
  «aplicar». O Centro de Controle edita por ficha; a grade editável célula a
  célula é melhor para conferência em massa.
- **O `EXPLAIN` visual.** Aqui já há `examinadas` e `us` na resposta da consulta
  em memória — falta desenhar.
- **O diagrama ER gerado do esquema.** As chaves estrangeiras já estão
  declaradas e já são devolvidas pelo `esquema`. O desenho é o que falta, e é
  SVG — o dossiê inteiro é feito disso.
- **A transferência de dados entre bancos diferentes.** O DbLink já lê MySQL(R);
  falta o caminho de escrita e o assistente.
- **Uma conexão por servidor, várias ao mesmo tempo, na mesma árvore.** O Centro
  de Controle já é multi-servidor; falta guardar as conexões.

---

## 5. Resumo em uma linha

Reaproveitar o **código** do DBeaver: não vale, e não por licença — por trazer
o Eclipse inteiro para um projeto que não tem nem uma crate.
Reaproveitar a **ferramenta**: vale muito, e o caminho passa por escrever SQL
aqui dentro — que é pré-requisito de todos os três caminhos, e não desvio de
nenhum.

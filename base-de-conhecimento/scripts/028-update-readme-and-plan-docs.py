# Update README and plan docs
# 27/08 18:34

p='docs/PLANO.md'
s=open(p).read()
s=s.replace('''| 6 | `Tabela.log` — data e hora de toda inclusão, alteração e exclusão | a fazer |
| 7 | Reindex: recriar o `.ndx` do zero | a fazer |
| 8 | Linha de comando | parcial — CLI existe, falta cobrir o resto |
| 9 | Pastas para separar tabelas | a fazer |
| 10 | Separar bancos de dados | a fazer |
| 11 | Hierarquia database → schema → tabela | a fazer |
| 12 | Paginação de tabelas grandes (`_001`, `_002`, ...) | a fazer |
| 13 | Quantidade de registros e de arquivos definida no `CREATE TABLE` | a fazer |''','''| 6 | `Tabela.log` — data e hora de toda inclusão, alteração e exclusão | **pronto** |
| 7 | Reindex: recriar o `.ndx` do zero | **pronto** |
| 8 | Linha de comando | **pronto** |
| 9 | Pastas para separar tabelas | **pronto** |
| 10 | Separar bancos de dados | **pronto** |
| 11 | Hierarquia database → schema → tabela | **pronto** |
| 12 | Paginação de tabelas grandes (`_001`, `_002`, ...) | **pronto** |
| 13 | Quantidade de registros e de arquivos definida no `CREATE TABLE` | **pronto** |
| 14 | Chave estrangeira no esquema (exigida pelo catálogo do FraseSQL) | **pronto** |''')

s=s.replace('''## 4. Questões abertas''','''## 4. Decisões tomadas

As três questões abaixo foram decididas pelo autor:

| Questão | Decisão |
|---|---|
| OLE DB | **ODBC + OLE DB desde já** — aceitando o custo e a restrição a Windows para o OLE DB |
| Direção do ODBC | **Os dois, driver primeiro** — primeiro o driver ODBC do PhxSql (saída), para Excel, Power BI, Crystal e os apps Clarion; depois o cliente (entrada) |
| Camada SQL | **rusqlite atrás de uma *feature* do Cargo** — SQL completo rápido pela tabela virtual, com a dependência de C opcional; depois vira oráculo de teste do parser próprio |

O texto original de cada questão fica abaixo, como registro do raciocínio.

## 4.1 Questões (já respondidas)''')

s=s.replace('''**Não dependem de nenhuma decisão pendente — podem começar já:**

1. Paginação do `.reg`/`.bin`/`.memo`/`.log` e o novo formato do ponteiro
   (é mudança de formato: quanto antes, mais barato)
2. FK no `Schema` (o FraseSQL precisa para gerar JOIN, e o dicionário Clarion
   tem RELATION com CASCADE/RESTRICT)
3. Hierarquia database/schema/tabela em disco
4. `Tabela.log` e o comando `phxsql log`
5. Reindex e compactação
6. `config.json` e o servidor TCP na porta 5000

**Dependem das respostas da seção 4:**

7. Servidor MCP
8. Camada SQL — Fase A ou B, conforme a decisão
9. ODBC — cliente ou driver, conforme a decisão
10. Integração no FraseSQL como `engine = "phxsql"`''','''**Fundação — concluída:**

1. ~~Paginação do `.reg`/`.bin`/`.memo`/`.log` e o novo formato do ponteiro~~
2. ~~FK no `Schema`~~
3. ~~Hierarquia database/schema/tabela em disco~~
4. ~~`Tabela.log` e o comando `phxsql log`~~
5. ~~Reindex~~

**A fazer, nesta ordem:**

6. `config.json` e o servidor TCP na porta 5000
7. Servidor MCP
8. Camada SQL — tabela virtual do SQLite via rusqlite, atrás de uma *feature*
9. Driver ODBC do PhxSql (saída), depois cliente ODBC e OLE DB (entrada)
10. Integração no FraseSQL como `engine = "phxsql"`
11. Compactação, transações, concorrência''')
open(p,'w').write(s)
print("PLANO.md atualizado")

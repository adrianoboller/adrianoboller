# Aviso de corte por teto: 01000, nunca 01004 -- e onde ele mora sozinho

**Estado:** FRUTÍFERO

**Evidência:** `crates/phxsql-odbc/src/lib.rs::sql_truncado_pelo_teto_avisa_01000`;
`crates/phxsql-odbc/src/lib.rs::sql_sem_truncado_continua_sql_success_puro`;
`testes-web/prova-truncado-sql.mjs` (rodada em 24/09/2026: 3/3 passos verdes com
o conserto, 2/3 com a leitura de `r.truncado` removida do `claude.js` -- prova
nos dois sentidos, pelo navegador, contra o `phxsqld` de verdade).

## O que aconteceu

Pedido 438 (seguimento do 419): o servidor diz `"truncado": true` na resposta
de `consultar`/`unir` desde o pedido 419, e esse campo atravessa **sem crivo
nenhum** o envelope da op `sql` (`resposta_do_sql`, `servidor.rs`) -- ou seja,
qualquer SQL que rode pelo console (`ui/claude.js`) ou pelo driver ODBC
(`crates/phxsql-odbc`) já recebia o aviso do servidor. Só que **nenhum dos
dois lia o campo**: o console mostrava `.notas` mas não `.truncado`, e o
driver só sabia SQLSTATE `01004` para truncamento de **valor** de coluna
(`SQLGetData` com buffer curto), nunca para truncamento de **conjunto de
linhas**.

## O que eu concluí primeiro, e estava errado

A primeira leitura do texto do pedido ("SQLSTATE 01004 é o de dado truncado")
me fez supor que a resposta óbvia era reaproveitar o `01004` que o driver já
usa em `SQLGetData`/`SQLDriverConnect` -- afinal "truncado" é a mesma palavra
nos dois lugares. Só que a palavra é a mesma e o **fenômeno** não é: `01004`
no padrão SQL (e em todos os quatro motores que herdam o SQLSTATE do mesmo
padrão ISO/IEC 9075) é `ERRCODE_WARNING_STRING_DATA_RIGHT_TRUNCATION` --
truncamento de uma STRING dentro de uma célula, não de um conjunto de linhas
cortado por um teto de recurso do servidor. Reaproveitar o código faria um
cliente que já trata `01004` como "encolha o buffer e chame `SQLGetData` de
novo" receber esse aviso para uma situação onde não há buffer nenhum para
encolher -- o SQLSTATE mentiria sobre o que aconteceu.

## O que a medição disse

Fonte primária, não humor: `/usr/share/postgresql/16/errcodes.txt` (o
`errcodes.txt` oficial do PostgreSQL, instalado localmente) lista a classe
`01` (warning) inteira, e a linha é literal:

```
01004    W    ERRCODE_WARNING_STRING_DATA_RIGHT_TRUNCATION    string_data_right_truncation
```

Não há, na classe `01` do padrão -- que os quatro motores herdam do mesmo
SQL/CLI, não inventam cada um o seu -- nenhum código dedicado a "linhas
cortadas por um limite do servidor/driver". O `SQL_ATTR_MAX_ROWS` do ODBC (o
mecanismo padrão mais parecido com o nosso `recursos.max_linhas`) é
documentado como não gerando SQLSTATE nenhum quando corta -- e este próprio
driver já tinha o precedente certo: a "01000" (warning genérico) para
"esquema indisponível para esta consulta" (`lib.rs`, comentário original,
`docs/ODBC.md` linha ~410), que é a mesma família de problema -- resposta
"menos completa do que o ideal, mas não errada".

**Convergência pelo SIGNIFICADO do código, não pela pesquisa de comportamento
de cada motor**: como o SQLSTATE nasce do padrão e não de uma escolha de
implementação de cada banco, os quatro concordam por definição em o que
`01004` quer dizer -- não há divergência real para pesar. A decisão não
precisou de média ponderada; precisou de ler a fonte do código.

## A regra

**SQLSTATE nomeia o FENÔMENO, não a palavra em português que o descreve.**
"Truncado" no aviso de corte por teto do servidor usa `01000` (warning
genérico); `01004` fica reservado, neste driver, para truncamento de VALOR
de coluna -- e os dois nunca se misturam no mesmo SQLSTATE, mesmo quando o
texto da mensagem começa com a mesma palavra.

## Como está guardado hoje

Em código: `crates/phxsql-odbc/src/lib.rs`, `executar_sql` -- o comentário
acima de `let cortado = resposta.booleano_ou("truncado", false);` explica a
escolha com a mesma referência (SQL_standard/errcodes.txt) e nomeia as duas
linhas que já usavam `01004` para não confundir quem for mexer de novo. O
segundo aviso (linhas cortadas) some se `sem_tipos` também disparar --
`anotar` empilha os dois registros de diagnóstico, e o teste
`sql_truncado_pelo_teto_avisa_01000` confere que SÓ um diagnóstico aparece
quando o esquema chega (isolando o aviso que a prova mede).

Do lado da tela, `crates/phxsql-server/src/idiomas.rs` (`tela.ia_res_truncado`,
os seis idiomas) e `crates/phxsql-server/ui/claude.js` (`executar()`) --
mesmo campo, mesma caixa `.aviso` (contorno âmbar da casa) das `notas` que já
existiam ali.

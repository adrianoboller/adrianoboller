# Ferramenta «de leitura» que recebe texto escreve pelo texto

**Estado:** PENDENTE

## O que aconteceu

A ponte MCP nasce somente de leitura e recusava as ferramentas com
`escreve()` (`phx_inserir`, `phx_atualizar`). O `phx_sql` não escreve pelo
NOME — `sql` está fora de `OPS_ESCRITA` —, mas escreve pelo TEXTO: um
`DELETE` por ele apagou 1.500 linhas pela ponte «somente de leitura» (pedido
781, medido pela frente P2–P5 com a camada de proteção desligada).

## O que eu concluí primeiro, e estava errado

Que o conserto morava na ponte: olhar o texto do `phx_sql` lá e recusar o que
não começasse por `SELECT`/`SHOW`. Seria um segundo classificador por texto ao
lado do despacho do `op_sql`, que reconhece cinco famílias (cadastro,
diretiva, transação, rotina, comando) por cinco analisadores. O primeiro verbo
novo que o despacho aprendesse e o classificador não divergiria calado — e o
`CALL` cujo corpo grava e o `CREATE VIEW` que carrega um `SELECT` dentro já
mostravam que palavra solta não diz se o comando escreve.

## O que a medição disse

Pelo caminho real da ponte (`ExecutorLocal` → `despachar`), com a camada de
proteção desligada: 12 comandos que escrevem voltam com `erro.mcp_so_leitura`
antes de qualquer trabalho — `DELETE FROM nao_existe` recusa pela ponte, e não
com «tabela não existe» —, e as 1.500 linhas ficam. Sem o carimbo, o `DELETE`
volta com `"afetadas": 1500`. O comportamento velho (SELECT simples, composto,
união, visão, `SHOW`) segue.

## A regra

Quando um portão filtra por NOME de operação, procure a operação cujo efeito
depende do ARGUMENTO — e faça a pergunta a quem já analisa o argumento, não a
um segundo leitor do mesmo texto.

## Como está guardado hoje

`so_le` em `sintaxe::Comando`, `rotina::Comando` e `diretiva::Comando` (sem
`_ =>`: variante nova não compila até alguém decidir); o `op_sql` pergunta em
cada ramo. Guardas `mcp-ponte-de-leitura-sem-carimbo`,
`mcp-sql-dml-passa-na-ponte-de-leitura`, `mcp-sql-rotina-passa-na-ponte-de-leitura`
e `mcp-ponte-de-leitura-recusa-o-select`. **O buraco que fica:** o `so_le` do
`Comando::Selecao` confia que SELECT não grava; se um dia uma função com efeito
colateral entrar no SELECT, a ponte deixa passar.

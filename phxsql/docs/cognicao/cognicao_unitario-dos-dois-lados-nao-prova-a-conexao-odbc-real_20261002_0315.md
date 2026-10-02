# Cognição: o unitário dos dois lados não prova a conexão — o gerenciador de driver real achou quatro defeitos que ninguém via

**Estado:** PENDENTE

**Assunto:** sonda viva do pedido 238 (`SQL_C_WCHAR` e parâmetro de SAÍDA) pelo
unixODBC 2.3.12 + pyodbc 5.3.0 contra um `phxsqld` real.
**Descoberta:** 02/10/2026, ~03h15. **Papéis:** B, F, J.

## 1. O que aconteceu

O pedido 238 estava ◐ porque o `SQL_C_WCHAR` (09/09) e o `OUT`/`INOUT` (12/09)
tinham teste de unidade nos dois lados — driver e servidor — e faltava a
conexão de verdade. A máquina tinha `apt` e `pip` funcionando pelo proxy:
`unixodbc-dev` e `pyodbc` instalaram sem pedir nada. A sonda
(`bancada/odbc/sonda-viva.py`) passou o texto e o `OUT` pelo gerenciador real e
o **primeiro `DELETE` já falhou**, antes de chegar ao `SQL_C_WCHAR`.

Achados, na ordem em que apareceram:

1. `[unixODBC][Driver Manager]Driver returned SQL_ERROR ... but no error
   reporting API found` — todo erro do driver sem SQLSTATE e sem texto.
2. `DELETE FROM clientes` anunciava 4 colunas (o esquema da tabela).
3. `SQLColAttribute(SQL_DESC_UNSIGNED)` recusava com `HYC00`.
4. `CALL p(?, ?)` recusado pelo servidor: «esperava um valor e veio ?».

## 2. O que eu concluí primeiro, e estava errado

- Para o (1) concluí que faltava `SQLGetFunctions` e o gerenciador, sem a
  lista, descartava o `SQLGetDiagRec`. Exportei `SQLGetFunctions`: **nada
  mudou**. A hipótese plausível morreu medida.
- Um `eprintln!` dentro do `SQLGetDiagRec` mostrou que o gerenciador **nem o
  chamava**. A causa só apareceu lendo o fonte do unixODBC (baixado do
  arquivo do Ubuntu, porque o GitHub não passa pelo proxy):
  `CHECK_SQLGETDIAGFIELD && CHECK_SQLGETDIAGREC` — ele exige o **par**, e o
  driver só exportava `SQLGetDiagRec`.
- Antes de rodar, eu esperava que o gargalo fosse o `SQL_C_WCHAR` (a hipótese
  H1: o gerenciador reescreveria o tipo C de um driver só-ANSI). Morta: o
  gerenciador repassa o tipo intacto e o texto com emoji voltou idêntico em
  20 de 20.

## 3. O que a medição disse

`bancada/odbc/resultados.json` (N = 20 por caso, 02/10/2026): `wchar_ida_e_volta`
20/20 (min/mediana/max 131,7/132,0/332,3 ms — o INSERT+SELECT, dominado pelo
`fsync`), `dml_sem_colunas` 20/20, `erro_chega_pelo_gerenciador` 20/20,
`saida_inteiro`, `saida_wchar` e `saida_inout` 20/20 cada, ~44 ms.
Disco: 8,5 GiB livres antes e depois; o build release de `phxsqld` + a `.so`
ocupou 74 MB. Cinco guardas novas no catálogo, todas PROVADAS (o `caem` caiu
com o defeito reposto). A prova de ABI antiga (`provar.py`) segue verde — o
driver nunca falhou ali, porque ela chama as funções dele direto.

## 4. A regra

**Prova de um driver ODBC passa por um gerenciador de driver real, não só pelas
funções do driver chamadas direto:** o gerenciador decide o que chamar a partir
do que o driver exporta (o par de diagnóstico) e o cliente real pergunta o que
quem escreveu a prova não pergunta (`SQL_DESC_UNSIGNED` de toda coluna). E dois
lados com teste de unidade cada um (`OUT` no driver, `OUT` no servidor) podem
não se encontrar: o `?` do `CALL` nunca era resolvido.

## 5. Como está guardado hoje

- `bancada/odbc/sonda-viva.py` + `resultados.json` — exige `unixodbc-dev` e
  `pyodbc` e **diz** quando faltam, sem pular calada.
- Guardas: `odbc-dml-anuncia-colunas-do-esquema`,
  `odbc-colattribute-recusa-unsigned`,
  `odbc-getfunctions-esconde-o-par-de-diagnostico`,
  `sql-call-nao-resolve-interrogacao-do-odbc` e
  `servidor-call-nao-passa-parametros-a-rotina` (as duas últimas são as duas
  pontas do mesmo `?`: o irmão fica).
- **Buraco que fica:** a sonda **não** roda em `./portoes.sh` (depende de
  pacote do sistema e de ~1 min de bancada) — quem a roda é o papel que mexe no
  driver. A existência da exportação `SQLGetDiagField` é guardada só pelo
  teste que a chama (remover a função quebra a compilação); a guarda do
  catálogo cobre a **lista** `FUNCOES_EXPORTADAS`, não o símbolo.
  `{? = call ...}` (valor de retorno) segue sem substrato.

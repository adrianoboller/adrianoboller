# Sonda que cita comentário morre calada quando o comentário é consertado

Frente F-ODBC, 08/09/2026, 17:30 — descoberto ao ligar os parâmetros de
instrução preparada no driver ODBC.

## 1. O que aconteceu

O `docs/COMPARATIVO.md` provava a ausência de «Parâmetro em instrução
preparada (`?`)» citando um **comentário** do driver:

> `crates/phxsql-odbc/src/lib.rs:617` — «Preparar aqui e guardar o texto: nao
> ha parametros nem plano no driver, e»

A sonda que produz essa linha é `bancada/comparativo/medir.py:239`, e ela
casava a frase por expressão regular:

```python
ond = citar("crates/phxsql-odbc/src/lib.rs", r"nao ha parametros nem plano",
            "sem menção no driver")
```

O `SQLBindParameter` entrou. O comentário deixou de ser verdade e foi
reescrito — que é o que a pétrea «conserto entra no caminho que o motivou»
manda fazer. E a sonda, sem uma linha de aviso, passaria a imprimir
**«sem menção no driver»** como se fosse a medida: a mesma célula ❌, com uma
evidência que agora significa o contrário do que parece.

## 2. O que eu concluí primeiro, e estava errado

Concluí que bastava atualizar a citação para a frase nova. Errado por dois
motivos, e o segundo é o que importa:

* a frase nova **não mede a ausência** — ela descreve o que passou a existir;
* e o lado que continua faltando **não é o do driver**. A regex apontava para
  o arquivo errado desde o começo: o que impede um `WHERE id = ?` de funcionar
  não é o driver, é o **léxico do servidor**, que recusa o caractere `?`. A
  sonda media o lado que estava pronto para dizer que o outro faltava, e isso
  funcionou por acidente enquanto os dois faltavam juntos.

Também concluí, ao escrever a prova de ABI nova, que `SQLFreeStmt(SQL_CLOSE)`
desfazia a preparação. Ver a seção 3.

## 3. O que a medição disse

Duas medições, as duas contra código e não contra memória:

**A sonda, medida onde dói.** O `SQLBindParameter` existe em
`crates/phxsql-odbc/src/lib.rs:788`. O `'?'` **não existe** em
`crates/phxsql-sql/src/lexico.rs` — a varredura devolve `None`, e o caractere
cai no ramo «caractere não faz parte da linguagem». A `op_sql` de
`crates/phxsql-server/src/servidor.rs:11795` lê `texto`/`sql` e mais nada.
Rodando a prova de ABI contra um phxsqld 0.18.0 de verdade, `WHERE id = ?`
volta `42000: esquema invalido: SQL, coluna 38`.

**O `SQL_CLOSE`, medido pela prova.** O passo 7c novo da
`bancada/odbc/prova-abi.py` reusou o comando (`stmt`) do passo 7b e esperava
`HY010` de «SQLNumParams sem SQLPrepare». Veio `SQL_SUCCESS` com a contagem
zero: o texto preparado em 7b (`SELECT nome FROM clientes WHERE id = 3`)
continuava vivo — `SQL_CLOSE` fecha o cursor e **não** despreparara, que é o
que a especificação manda. O driver estava certo e o defeito era da prova.
Corrigido, a corrida fecha em **86 conferências, zero falhas, 1 NÃO MEDIDA**
(eram 73 antes do passo 7c).

## 4. A regra

**Sonda de ausência mede o lado que FALTA, e nunca por comentário.** Comentário
é a primeira coisa que o conserto reescreve — casá-lo faz a sonda morrer calada
no dia em que o buraco começa a ser tapado, e imprimir «sem menção» como se
fosse medida. E quando a sonda encontra código dos dois lados, ela para em
**MEIO**: código não é efeito, e quem promove a célula é a sonda viva que
confere a linha que voltou.

## 5. Como está guardado hoje

* A sonda do `medir.py` tem dois lados: cita o `SQLBindParameter` do driver e
  mede o léxico, que é o lado que falta. Enquanto o `?` não entrar no léxico
  ela dá `NAO`; quando entrar ela dá `MEIO`, **nunca** `TEM`.
* O passo 7c da `bancada/odbc/prova-abi.py` é a sonda viva que promove: ele
  tenta a volta de `WHERE id = ?` e, enquanto ela não vier, imprime
  **NÃO MEDIDA** com o motivo e o comando — nunca «ok», nunca sumindo da lista.
  Há um `nao_medida()` próprio para isso, ao lado do `confere()`.
* O comentário do `SQL_CLOSE` está no passo 7c da prova, com o erro registrado
  ao lado, e o aprendizado virou marcador na seção 8 do `docs/ODBC.md`.
* **O buraco que fica**, medido e não suposto: há **dois** `citar()` no
  `medir.py` (linhas 215 e 252) contra **seis** `tem()`, e o outro `citar()`
  casa `"TLS aqui"` em `crates/phxsql-server/src/rest.rs:578` — que é uma
  **frase de texto visível** (a descrição OpenAPI do token), não um
  comentário. É mais durável que comentário, e menos que um `fn` ou um
  `struct`: ninguém a reescreve por conserto, mas quem melhorar a redação a
  quebra igual. E a linha acima dela já registra que essa mesma sonda apontou
  para o arquivo errado em 07/09.

  Não há conferidor que ache isso sozinho, e não escrevi um: com dois casos no
  repositório inteiro, um casador diria mais sobre si do que sobre o código.
  Se aparecer um terceiro, o lugar de registrá-lo é aqui — e aí o casador
  passa a valer a pena.

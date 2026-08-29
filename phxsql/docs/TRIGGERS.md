# Gatilhos e procedimentos

Fecha os pedidos **#49 (triggers)** e **#50 (stored procedures)**. Os dois
estavam parados na mesma pergunta — *em que linguagem o corpo é escrito?* — e
a decisão do Adriano foi: **a do MySQL(R)/MariaDB(R)**, sintaxe *similar*, não
idêntica. O que não couber **recusa dizendo o que não é suportado**, na mesma
regra que a camada `SELECT` já seguia.

Por isso os dois pedidos viraram uma missão só: **há um interpretador, não
dois**. O corpo de um gatilho e o corpo de um procedimento passam pelo mesmo
analisador e pelo mesmo avaliador (`crates/phxsql-sql/src/rotina.rs`); o que
muda entre eles é uma tabela de regras — quem enxerga `NEW`/`OLD`, quem pode
falar com o motor, quem pode `SIGNAL`.

```
crates/phxsql-sql/src/rotina.rs      a linguagem: parser, avaliador, números
crates/phxsql-server/src/rotinas.rs  o registro: carga, gravação, compilação
crates/phxsql-server/src/servidor.rs os pontos de disparo e o portão
```

---

## 1. A linguagem

Tudo entra pela op `sql` que já existia — é SQL, e um driver manda pelo mesmo
campo:

```json
{"op":"sql","database":"loja","texto":"CREATE TRIGGER … "}
```

### Gatilhos

```sql
CREATE TRIGGER nome
  BEFORE|AFTER  INSERT|UPDATE|DELETE
  ON [database.][schema.]tabela
  FOR EACH ROW
  <corpo>

DROP TRIGGER [IF EXISTS] nome
SHOW TRIGGERS
```

### Procedimentos

```sql
CREATE PROCEDURE nome([IN|OUT|INOUT] p TIPO, …) <corpo>
DROP PROCEDURE [IF EXISTS] nome
SHOW PROCEDURES          -- SHOW PROCEDURE STATUS também
CALL nome(arg, …)
```

`CALL` devolve os `OUT`/`INOUT` num objeto `saida`:

```json
{"procedimento":"somar","saida":{"total":5050}}
```

`OUT` **no fim** pode ser omitido na chamada (`CALL somar(100)`), porque quem
chama não tem o que passar ali. `OUT` no meio pede um `NULL` no lugar.

### O corpo

```sql
BEGIN … END                    -- ou uma instrução só, sem BEGIN
DECLARE v TIPO [DEFAULT expr]
SET v = expr,  SET NEW.col = expr
IF … THEN … ELSEIF … ELSE … END IF
WHILE … DO … END WHILE
SIGNAL SQLSTATE '45000' [SET MESSAGE_TEXT = '…']
INSERT INTO [db.][schema.]t (col, …) VALUES (expr, …)
SELECT col, … INTO v, … FROM …          -- uma linha
```

Tipos: `INT`, `DECIMAL(p,s)`, `VARCHAR(n)`, `BOOL`, e `DATE`/`DATETIME`/`TIME`
— que são **texto**, porque é assim que data viaja no protocolo.

Expressões: `+ - * /`, comparações, `AND`/`OR`/`NOT`, `IS [NOT] NULL`, e as
funções `CONCAT`, `UPPER`, `LOWER`, `TRIM`, `LENGTH`/`CHAR_LENGTH`, `ROUND`,
`ABS`, `COALESCE`/`IFNULL`.

### O que `NEW` e `OLD` enxergam

| | `NEW` lê | `NEW` grava | `OLD` lê |
|---|---|---|---|
| `BEFORE INSERT` | sim | **sim** | — |
| `BEFORE UPDATE` | sim | **sim** | sim |
| `BEFORE DELETE` | — | — | sim |
| `AFTER INSERT` | sim | não | — |
| `AFTER UPDATE` | sim | não | sim |
| `AFTER DELETE` | — | — | sim |

---

## 2. As três decisões que moldaram o desenho

### `BEFORE` valida; `AFTER` audita — e a divisão não é estilo, é a trava

O `BEFORE` roda **com a trava de dados na mão**, entre a conversão da linha e a
gravação. É o que permite alterar `NEW` e cancelar por `SIGNAL` sem que ninguém
veja um estado intermediário. E é exatamente por isso que **`BEFORE` não fala
com o motor**: um `INSERT` ali dentro tomaria a mesma trava global de novo — o
abraço mortal com a própria trava, que não é um erro, é um servidor parado.

O `AFTER` roda **depois de a trava ser solta**, e aí sim pode gravar: é o lugar
da auditoria. Em troca, ele **não tem `SIGNAL`** — a escrita já aconteceu e não
há transação que a desfaça. Oferecer o verbo ali seria prometer um cancelamento
que não existe.

O parser recusa cada um dos dois casos com o motivo escrito, e há um segundo
cinto: o corpo `BEFORE` recebe um `MotorNulo` que recusa qualquer pedido. Se um
dia uma instrução nova esquecer a regra, ela recebe um erro em vez de travar o
servidor.

### Falha de `AFTER` é **aviso**, não erro — e isso está escrito na resposta

Não há transação. Devolver erro depois de a linha estar gravada diria «não
gravou» a quem gravou — e o cliente repetiria, duplicando. Então a resposta
continua `ok` e carrega as duas verdades, na ordem certa:

```json
{"rowid": 7, "registros": 4,
 "gatilhos_avisos": ["gatilho \"audita\" falhou: nao encontrado: …"]}
```

É o limite honesto do desenho, e é por isso que o `AFTER` serve para auditoria
e não para regra de negócio. A regra vive no `BEFORE`.

### Gatilho quebrado **barra a escrita**; não é pulado

O disco guarda o corpo como texto; ao carregar, ele é compilado. Um corpo que
não compila mais (arquivo editado à mão, versão antiga) fica marcado e **barra
a escrita da tabela dele**, com o nome e o motivo. Pular seria fingir que a
regra que o dono escreveu não existe — no exato momento em que ela deixou de
valer. O `SHOW TRIGGERS` mostra o campo `quebrado`, que é a chance de consertar
antes de alguém esbarrar.

---

## 3. O portão de permissão

**Criar, excluir e listar** gatilho ou procedimento exigem **`administrar`** na
base. Não `criar`. A razão é escalada: com `criar` bastando, quem pode criar
tabela penduraria um `AFTER INSERT` na tabela alheia e desviaria cada linha
gravada pelos outros para uma tabela sua. É a mesma regra dos *jobs*, e pelo
mesmo motivo — os três são **código guardado que roda depois, sob o poder de
outra pessoa**.

**`CALL` não pede permissão própria.** Cada pedido que o corpo produz sai pelo
`executar_derivado` — o mesmo portão de política e de permissão de um pedido da
rede — com a sessão de **quem chamou**. Então chamar nunca dá poder que a
pessoa já não tinha. É a lição do `juntar`/`unir` aplicada por antecipação: a
rotina **produz** o pedido que o portão já sabe conferir, em vez de ganhar uma
porta própria.

O teste que trava isso é `call_nao_e_a_porta_dos_fundos_para_a_tabela_negada`,
e a prova real dele está na seção 6.

Servidor em `somente_leitura` recusa criar e excluir rotina: a op `sql` não
está em `OPS_ESCRITA` (um `SELECT` não escreve), então a conferência mora no
único lugar por onde esses comandos passam.

---

## 4. O custo do caminho SEM gatilho

A regra do projeto é que instrumentação desligada custe zero, e que **o portão
que decide isso venha antes do trabalho**. Aqui ele é um `AtomicBool` — «existe
algum gatilho neste servidor?» — lido antes de qualquer coisa: sem gatilho
nenhum, o caminho de escrita não toma trava, não monta `String` e não olha o
pedido.

### O que foi medido, e o quanto o número vale

`cargo run --release -p phxsql-server --example custo-do-portao`

20.000 linhas por rodada, 5 rodadas **intercaladas** (1,2,3, 1,2,3, …):

| cenário | mediana | faixa | espalhamento |
|---|---|---|---|
| 1. sem gatilho nenhum | 112,58 µs | 107,32..131,48 | 22,5% |
| 2. gatilho em **outra** tabela | 113,57 µs | 107,93..118,04 | 9,4% |
| 3. gatilho `BEFORE` na própria | 111,71 µs | 109,81..115,72 | 5,4% |

**Diferença 2−1 = +0,99 µs/linha. Maior espalhamento dentro de um cenário
sozinho = 24,16 µs/linha.**

A conclusão honesta é uma comparação, e não um número: **o portão não aparece
acima do ruído**. Dizer «o portão custa 0,99 µs» seria inventar precisão que a
medição não tem — o cenário 3, que faz mais trabalho, saiu *mais rápido* que o
1 nesta rodada, o que só mostra o tamanho do ruído.

A mesma conclusão pelo soquete, com dois binários (HEAD e este), 3 pares
intercalados de 2.000 inserções + 20.000 em lote:

| | mediana antes | mediana depois | diferença | maior espalhamento |
|---|---|---|---|---|
| `inserir` pela rede | 381,10 µs | 373,72 µs | −7,38 µs | 244,76 µs |
| `inserir_lote` | 24,60 µs | 20,96 µs | −3,63 µs | 9,61 µs |

O «depois» saiu nominalmente **mais rápido** nos dois — o que é a prova de que
a diferença é ruído, não ganho. A máquina estava compartilhada com outros
agentes compilando Rust durante a medição, e isso faz parte do método: está
registrado porque um número medido em máquina ocupada não vira, depois, uma
afirmação de que o gatilho acelerou nada.

**O aprendizado de método** (e o erro que ele corrigiu): a primeira versão do
medidor rodava os três cenários **em sequência, na mesma tabela**, e mostrava o
cenário 1 custando 133 µs contra 110 µs dos outros — o portão parecendo *caro*
justamente onde ele não faz nada. Era artefato de ordem: o primeiro cenário
pagava a árvore fria e o crescimento do `.ndx` que os outros herdavam quentes.
Servidor limpo por cenário e rodadas intercaladas desfizeram o fantasma. *Medir
em bloco põe toda a deriva da máquina dentro de um cenário e a chama de custo
dele.*

---

## 5. Onde as rotinas moram

Um arquivo JSON por database, no próprio diretório dele:

```
base/loja/gatilhos.json
base/loja/procedimentos.json
```

JSON e não formato binário porque isto é **cadastro, não dado**: muda por
comando, lê-se no olho e viaja junto com o backup do diretório. O formato está
em `docs/FORMATO.md`, seção 14.

Três regras que valem a pena saber:

* **arquivo ausente = zero rotinas = comportamento de sempre.** E quando a
  última rotina sai, o arquivo sai junto — para o ausente continuar
  significando o que significa;
* **o texto do corpo é guardado verbatim** (com aspas, caixa e espaços). É ele
  que o `SHOW` devolve, e é dele que a compilação sai a cada carga;
* **JSON inválido derruba a subida do servidor**, com o caminho e o motivo.
  Subir sem os gatilhos que o dono escreveu seria gravar sem as regras dele, em
  silêncio — pior do que não subir.

Excluir a tabela leva os gatilhos dela junto, como no MySQL(R): um órfão
dispararia contra uma homônima futura que não tem nada com ele. A resposta do
`excluir_tabela` passa a trazer `gatilhos_apagados`.

---

## 6. As provas

### Teste unitário e de integração

`cargo test --workspace`. Os de integração ficam em `testes_gatilhos`
(`servidor.rs`) e passam pelo `despachar`, que é por onde o pedido entra de
verdade.

### A prova real, nos dois sentidos

A regra da casa é que o teste novo **falhe com o defeito reposto**. Dois foram
repostos à mão, e os dois derrubaram o teste certo:

| defeito reposto | teste que caiu |
|---|---|
| o motor das rotinas chamando `executar` em vez de `executar_derivado` (sem portão) | `call_nao_e_a_porta_dos_fundos_para_a_tabela_negada` — o `CALL` gravou na tabela negada |
| o resultado do corpo `BEFORE` descartado (`let _ = resultado`) | `sinal_cancela_a_escrita_e_a_linha_nao_entra` — a linha recusada entrou |

Com o conserto de volta, os 16 testes do módulo passam.

### A prova por soquete

```bash
cargo build --release
python3 bancada/rotinas/prova-rotinas.py
```

Sobe um phxsqld próprio em **5301/5701**, roda 11 passos com o resultado
esperado escrito antes, e mata **só o processo que criou, pelo PID**. Prova o
que teste unitário não prova: que o `gatilhos.json` sobrevive ao **reinício** do
processo, e que a `MESSAGE_TEXT` chega ao cliente pela rede com o código 3005 e
o nome `SINAL`.

O passo 11 é o que mais importa: **tabela sem gatilho grava exatamente como
antes** — com o gatilho ligado em *outra* tabela, que é justamente quando o
portão está verdadeiro e a procura acontece. Ele confere o dado *e a forma da
resposta*, chave por chave.

---

## 7. Aprendizados

### Frutíferos

**A regra do «não tem substrato? recuse pelo nome» pagou de novo, e mais cedo.**
Cada coisa que não cabe recusa dizendo o próprio nome e o motivo — `CASE`,
`LOOP`, cursor, `HANDLER`, `UPDATE`/`DELETE` no corpo, `DEFINER`, `FOR EACH
STATEMENT`, `CREATE FUNCTION`, as características (`DETERMINISTIC`…). Quem cola
um corpo do MySQL(R) precisa saber **o que trocar**, e «sintaxe inválida» não
diz. São 17 recusas com teste.

**O corpo é conferido no `CREATE`, não no primeiro `INSERT` de produção.** O
erro sai com a coluna do texto, na hora de criar. Um gatilho que só quebra na
primeira gravação de segunda-feira é a pior hora possível para descobrir.

**A conta não passa por `f64`, e isso não foi zelo — foi requisito.** `SET
NEW.preco = NEW.preco * 1.1` é exatamente o corpo que alguém vai escrever. O
`Numero` do interpretador guarda mantissa `i128` e escala: `1.10 * 3` dá `3.30`,
`0.1 + 0.2` dá `0.3`, e `ROUND(1500.00 * 1.1, 2)` dá `1650.00` exato. Seria
absurdo o protocolo exigir decimal como texto para não perder centavo e a
linguagem que mexe nesse número reintroduzir o `f64` no meio do caminho.

**O `AFTER` vê a linha como ela FICOU.** O `NEW` do `AFTER INSERT` é lido de
volta do disco, ainda dentro da trava — com sequência preenchida, `rownum` de
verdade e o que o `BEFORE` alterou. A prova por soquete trava isso: a auditoria
gravou `entrou Maria de JOINVILLE`, com a cidade já normalizada pelo `BEFORE`.

**Coluna de sistema não se toca por gatilho.** `SET NEW.rownum` é recusado na
gravação: *a ordem de digitação é sagrada*, e um gatilho seria o jeito novo de
quebrá-la sem ninguém perceber.

**A lição do `juntar`/`unir`, aplicada antes de doer.** Toda operação que um
corpo produz sai pelo portão que já existe, com o poder de quem chamou. Nenhuma
op nova foi criada — e por isso não há um portão novo para alguém esquecer.

### Infrutíferos — e o que cada um ensinou

**Não há `UPDATE` nem `DELETE` no corpo, e a tentativa morreu medida contra o
motor, não por preguiça.** O motor atualiza e exclui **por `rowid`**; traduzir
um `UPDATE … WHERE` exigiria o planejador que `docs/SQL.md` diz não existir. As
alternativas eram piores que a ausência: aceitar só `WHERE chave = valor`
criaria um `UPDATE` que funciona ou não conforme o índice — e um verbo que às
vezes funciona é pior que um verbo que falta. Fica recusado pelo nome, com o
motivo, e `INSERT` + `SELECT … INTO` cobrem auditoria e leitura, que era o
pedido. *Hipótese que morre também é resultado, e é o que impede a ideia de
voltar sem medição.*

**A primeira medição do custo do portão foi um fantasma, e quase virou tabela.**
Está contada na seção 4: 133 µs contra 110 µs, o portão parecendo caro onde ele
não faz nada — puro artefato de ordem. Se eu tivesse publicado aquele número, o
projeto ganharia um «gatilhos custam 20% na escrita» que ninguém conseguiria
reproduzir. *Medir a premissa vem antes de publicar o número — inclusive quando
o número é contra a gente.*

**A segunda medição, já correta, não conseguiu provar o custo-zero — e o
registro é esse.** Com a máquina compartilhada, o espalhamento dentro de um
cenário (até 24 µs em processo, 244 µs pela rede) engole qualquer diferença
entre cenários. O medidor foi mudado para **dizer isso sozinho**: ele compara a
diferença com o espalhamento e imprime «o portão NÃO aparece acima do ruído do
disco», em vez de um número bonito. *Número citado é número que não se mede — e
número medido em máquina ocupada precisa vir com a régua do ruído ao lado.*

**A `MESSAGE_TEXT` quase apareceu embrulhada.** O primeiro corte punha todo erro
de gatilho como `gatilho "x": <erro>` — inclusive o `SIGNAL`. Só que a
`MESSAGE_TEXT` é a frase que **o dono do banco escreveu para quem esbarrar na
regra**; escondê-la atrás do nosso prefixo troca a mensagem dele pela nossa. O
`SIGNAL` passa intacto; todo o resto leva o nome do gatilho, que aí sim é o que
falta para consertar.

**Um passo da prova por soquete falhou, e o defeito era do teste — de novo.**
Ele comparava o `COUNT(*)` do procedimento com a contagem da varredura. São
perguntas diferentes: o `COUNT(*)` sai do **cabeçalho** em O(1) e conta o slot
da linha excluída **suavemente**; a varredura não a mostra. Comportamento
pré-existente e documentado do `COUNT(*)`, não dos gatilhos — mas a prova
apanhou por comparar duas fontes como se fossem uma. *É a terceira vez que o
projeto anota isto: um teste que passa (ou falha) por engano é pior que um teste
que falta.*

---

## 8. O que ficou de fora, de propósito

| não existe | por quê |
|---|---|
| `UPDATE`/`DELETE` no corpo | sem planejador; o motor escreve por `rowid` |
| `CASE`, `LOOP`, `REPEAT` | `IF/ELSEIF` e `WHILE` cobrem; menos superfície |
| cursor, `HANDLER` | o erro sobe para quem chamou, com código e mensagem |
| `CREATE FUNCTION` | devolveria valor dentro de expressão SQL, e a camada `SELECT` não avalia expressão |
| `CALL` aninhado | nesta versão não |
| `BEGIN`/`COMMIT` no corpo | **não há transação no PhxSql** |
| `DEFINER` | gatilho roda com o poder de quem dispara; `CALL`, de quem chama |
| `FOLLOWS`/`PRECEDES` | disparam na ordem de criação |
| variável de sessão (`@x`) | não há sessão de variáveis; use `DECLARE` |

Duas diferenças de comportamento que **não são omissão** e precisam estar
escritas, porque quem vem do MySQL(R) espera o contrário:

* **`LENGTH` conta caracteres**, não bytes (o `CHAR_LENGTH` é o sinônimo
  honesto). Contar bytes num banco que fala UTF-8 mentiria sobre «Blumenau» ter
  8 letras;
* **comparação de texto é sensível à caixa**. O MySQL(R) usa colação
  case-insensitive por padrão; aqui `'a' = 'A'` é falso. Quem quer o outro
  comportamento escreve `UPPER(…) = UPPER(…)`, que é explícito e não depende de
  configuração invisível.

E o limite que vale repetir: **não há transação**. Um corpo que falha no meio
deixa gravado o que já gravou, e um `AFTER` que falha não desfaz a escrita que o
disparou.

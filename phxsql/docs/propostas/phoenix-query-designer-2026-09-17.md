# Phoenix Studio «IA · Query Designer · Excel Studio» — medido contra o nosso gargalo

**Papel J (pesquisador), 17/09/2026.** Ordem do dono, ~02:40 UTC: *«Arquivos sobre
o SQL query design — verificar se tem algo a ser aproveitado.»*

O material é de um projeto irmão do próprio dono (Phoenix Studio IA v10), então
**não há armadilha jurídica**: o `Cargo.toml` da crate traz
`authors = ["Phoenix Studio IA v10 <adriano@wxsolucoes.com.br>"]`. A lei
«inspiração, não cópia» continua valendo pelo motivo **técnico** e não legal —
*desenho que não passou pela nossa cabeça não sobrevive ao primeiro encontro com
as nossas restrições* —, e a §6 deste parecer nomeia cada divergência com a
restrição nossa que a causa.

O que chegou, em `/root/.claude/uploads/34595649-0af6-575a-8f79-80dbe8cb7a5d/`:

| arquivo | o que é | medida |
|---|---|---|
| `d16fcab9-query-designer.md` | resumo do «Sprint 4b» | 25 linhas |
| `3a9fa903-phoenix_s88_s5124_query_designer.zip` | a crate Rust | `src/lib.rs` **898** linhas + `Cargo.toml` **10** |
| `d2111e3b-…_2.html` | mockup, 1ª geração | **468** linhas |
| `e8d5fd65-…_3.html` / `4376fe36-…_3.html` | mockup, 2ª geração (os dois idênticos) | **667** linhas |
| `c3ddf9a9-…_5.html` | mockup, 3ª geração — **é a da captura de tela** | **833** linhas |
| `50edecf4-…pdf` / `c9a4789d-…pdf` | relatório de uma página, inflado com zlib | 1.096 bytes |
| `94c7f376-image.png` | a captura — é do `_5` | — |

**O primeiro número, e ele é bom:** o `Cargo.toml` da crate **não tem seção
`[dependencies]`** — só `[package]` e `[lib]`. O `lib.rs` importa apenas
`std::collections::HashMap` (linha 13) e `std::fmt`. **Zero dependências
externas**, e `#![forbid(unsafe_code)]` na linha 6. A pétrea desta casa não é
tocada pela crate. O que a desqualifica é outra coisa, e está na §5.

---

## 0. O arco das três gerações diz o que o dono valoriza — e isso é medido

Contado com `grep -c` nos três arquivos:

| termo | `_2` (468) | `_3` (667) | `_5` (833) |
|---|---|---|---|
| `dataBar` | **2** | **2** | **2** |
| `Ollama` | **2** | **2** | **2** |
| `UNION` | 0 | 11 | 19 |
| `HAVING` | 0 | 12 | 13 |
| `CAST` | 0 | 4 | 5 |
| `sub-select` | 0 | 3 | 4 |
| `vassoura` | 0 | 3 | 3 |
| `perfil` | 0 | 5 | 5 |
| `vennJoin` | 0 | 0 | 3 |
| `testarQuery` | 0 | 0 | 2 |
| `analisarTuning` | 0 | 0 | 3 |
| `contarColunas` | 0 | 0 | 3 |

**Duas coisas nasceram na primeira tela e atravessaram as três sem mudar: a
barra de dados do Excel (`dataBar`) e o LLM local (`Ollama`).** Todo o resto —
UNION, HAVING, CAST, sub-select, vassoura, perfil, Venn, tuning — foi
**acrescentado depois**. O que sobrevive a três revisões sem ser reescrito é o
invariante do pedido; é por isso que a ordem de valor da §3 começa pelo `dataBar`
e não pelo construtor visual, que é a parte mais vistosa.

E a segunda leitura do arco: o `_5` acrescenta ao `_3` justamente o que o PhxSql
**já tem** (sete junções com Venn, validação de contagem de colunas do UNION), o
que sugere que as duas casas convergiram sozinhas para o mesmo desenho de tela —
o mesmo padrão do CRC-32 da página do `.ndx` com o Cassandra®.

---

## 1. Matriz de evidência: recurso × já existe aqui × falta × custo × recomendação

Legenda da recomendação: **IDEIA** = aproveitar o conceito, escrever aqui;
**CÓDIGO** = aproveitar linhas da crate; **RECUSAR** = com o número ao lado;
**JÁ TEMOS** = nada a fazer, e o material confirma a escolha.

### 1.1. A tela de consulta

| recurso do material | já existe aqui (arquivo:linha) | falta | custo | recomendação |
|---|---|---|---|---|
| **Construtor visual com N filtros e AND/OR por filtro** | **NÃO EXISTE.** A tela «Consulta» é de **UMA condição só** — uma coluna, um operador, um valor: `ui/index.html:13725–13732` (`#cCol`, `#cOp`, `#cVal`). E ela consulta o `SelectMemory`, não a op `sql`: o próprio texto da tela diz «**Isto não é SQL**» (`:13716`). Busca por `conector`/`" AND "`/`" OR "` no `index.html` inteiro: **1 acerto, e é um comentário sobre célula de grade** (`:2483`) | o construtor multi-condição inteiro | tela nova; o motor **já aceita** (`varrer.expressao` com `AND`/`OR`, `IN`, `BETWEEN`, `LIKE`, `IS NULL` — `docs/STATUS.md` linha G) | **IDEIA — é a maior lacuna real que o material expõe** |
| **A tela web manda a op `sql`** | **Só de um lugar:** `ui/claude.js:1317` (`api("sql", …)`). Medido com `grep -c` em todos os `.js` e no `index.html`: **1 ocorrência no repositório de interface inteiro**, e ela está dentro do painel de IA | um editor SQL de primeira classe fora do painel de IA | — | **IDEIA** (vem de graça junto do item de cima) |
| Sete tipos de junção escolhidos **clicando no diagrama de Venn** | **JÁ TEMOS, e desde o pedido 91.** As sete figuras em `ui/index.html:9851–9866` (`JUNCOES`), o Venn desenhado à mão em `:9873–9906` (`function venn`), e o SQL equivalente **embaixo de cada cartão** com o motivo escrito em `:9843–9849`. O motor: `src/juncao.rs`, **1.159 linhas, 13 testes**, cinco casos reais para sete figuras (`juncao.rs:10–21`: «`direita` é `esquerda` com os lados trocados… a troca decide qual tabela cabe na memória») | nada | — | **JÁ TEMOS — e mais fundo** (ver §1.5) |
| Validação do UNION por **contagem de colunas** | **JÁ TEMOS, e mais forte:** `src/juncao.rs:579` `conferir_uniao` — confere a contagem **e a família de tipo, posição a posição**, contra o esquema real. A frase da recusa (`:589`) é quase a do mockup: «a parte {} tem {} coluna(s) e a primeira tem {}». O comentário `:575–578` explica por que **não** empilha por nome | UNION entre dois `SELECT` **arbitrários** na camada SQL (o nosso valida o `unir` de tabelas) | — | **JÁ TEMOS a validação; o `UNION` de SELECT é outro item (1.2)** |
| Diagrama ER com as caixas e as ligações N:1 | **JÁ TEMOS.** `ui/diagrama-er.js`, **712 linhas** (pedido 127). O do mockup é `renderER()`, ~28 linhas, com as três tabelas **fixas no código** (`QESQUEMA`, `_5:456`) | nada | — | **JÁ TEMOS** |
| `pivotar` com assistente | **JÁ TEMOS.** `ui/index.html:6196` `telaPivot`, «um assistente de três passos» (`:6160`), estado que sobrevive a ir e voltar (`:6190`); motor em `src/pivot.rs`, 850 linhas | nada | — | **JÁ TEMOS** (o mockup não tem pivô) |
| «**A IA propõe; você aprova**» | **JÁ TEMOS, e é o desenho declarado:** `docs/CLAUDE-IA.md` §6 «Modelar: a IA propõe, a pessoa confirma, o PhxSql cria», com «a conferência vem ANTES de a primeira tabela nascer». O editor + botão + grade: `ui/claude.js:1308–1340` — «executa **pelo clique da pessoa, e nunca sozinho**» | nada no conceito | — | **JÁ TEMOS** |
| **LLM local (Ollama) × nuvem** | O `<select>` do mockup (`_3:171–174`) contra a nossa **API da Anthropic chamada do NAVEGADOR** — `docs/CLAUDE-IA.md` §1, `ui/claude.js:92` (`ENDPOINT_OFICIAL`) | o LLM local | **e aqui a premissa do briefing estava errada** — ver §4.2 | **IDEIA, e é barata — decisão do dono** |
| Perfil `usuario` / `dba_administrador` desabilitando INSERT/UPDATE/DELETE | Aqui o portão é **UM e é do servidor**, contra a operação **traduzida**: `docs/SQL.md` §5 «O portão continua sendo UM», teste `o_sql_nao_e_a_porta_dos_fundos_para_a_tabela_negada`. `Atividade::da_operacao("sql")` é `Ler`, e «o de fora só aperta, nunca afrouxa» | **mostrar na tela** o que este usuário pode rodar, antes da ida-e-volta | pequeno, e é **conforto, não segurança** | **IDEIA, com a ressalva da §2.4** |
| «▶ Testar query» | O mockup **não executa**: `testarQuery()` (`_5:753`) chama `problemasQuery()`, validação estática no navegador, e quando passa imprime «✓ Query válida — pronta para executar … **A execução real e o plano (EXPLAIN/tuning do banco) ocorrem no servidor via conector/ODBC**» (`_5:764`). Aqui o botão **executa de verdade** (`claude.js:1317`) | a validação estática *antes* de executar | pequeno | **IDEIA parcial** (ver §2.3) |
| «🛠 Ajuste automático (tuning)» | `analisarTuning()` (`_5:768–782`) são **7 regras de casamento de texto** sobre a string do SQL gerado: `SELECT *`, `DISTINCT` com `GROUP BY`, função sobre coluna no `WHERE`, `LIKE '%…`, `ORDER BY` sem `LIMIT`, `OR` entre condições, `JOIN` sem `WHERE`. `ajusteAutomatico()` (`:783`) aplica **2**: tira o `DISTINCT` e põe `LIMIT 100`. **Zero medição.** Aqui: `estatisticas` devolve **p50/p90/p95 e as 15 mais lentas** (`servidor.rs:19356–19404`), e a escolha de índice é `indices.iter().find(…)` — `traduzir.rs:395`, com a nota `:455` «índice escolhido pelo WHERE — **e não há planejador**» | `EXPLAIN` — **zero ocorrências** em `crates/phxsql-sql/` e no `servidor.rs` | — | **RECUSAR as 7 dicas de texto; o item real é o planejador, que já está no `STATUS.md` G** |

### 1.2. A camada SQL, item a item do mockup

Conferido no fonte de `crates/phxsql-sql/` (12.759 linhas em 10 arquivos).

| o que o mockup gera | o `phxsql-sql` aceita hoje? | onde |
|---|---|---|
| `SELECT col, SUM(x) AS a FROM t JOIN … ON … WHERE … GROUP BY … ORDER BY … LIMIT n` | **SIM, inteiro** | `docs/STATUS.md` G; `LIMIT` em `sintaxe.rs:812` |
| `HAVING SUM(p.valor) > 1000` | **SIM** | `lib.rs:18` da gramática; `sintaxe.rs:154` |
| `INNER JOIN` / `LEFT JOIN` em cadeia | **SIM** | `sintaxe.rs:759–762`; `docs/SQL.md` §7.8 |
| `RIGHT JOIN` / `FULL OUTER JOIN` / `CROSS JOIN` | **NÃO** na camada SQL — mas **SIM** como operação do protocolo (`juncao.rs`) | `docs/STATUS.md` G lista como faltando |
| `SELECT DISTINCT` | **NÃO, e a recusa é nomeada:** `sintaxe.rs:747–751` — *«DISTINCT nao tem substrato: nenhuma operacao do protocolo elimina repetido numa varredura»* | idem |
| `UNION` / `UNION ALL` entre dois SELECT | **NÃO.** A palavra está reservada (`sintaxe.rs:394`, lista `CLAUSULAS`) e o `lib.rs:47` diz que não há. O `unir` **de tabelas** existe e valida (`juncao.rs:579`) | idem |
| `CAST(p.valor AS INTEGER)` | **NÃO.** O avaliador de `phxsql_core::expressao` tem 8 funções — `UPPER`, `LOWER`, `TRIM`, `LENGTH`, `ROUND`, `ABS`, `COALESCE`, `CONCAT` (`expressao.rs:202–209`). **`CAST` não está lá**, e a única ocorrência da palavra no `crates/` é um comentário em `rotina.rs:362` | — |
| `r.nome \|\| ' / ' \|\| v.nome` | **NÃO** — o operador `\|\|` não existe. `CONCAT(...)` existe (`expressao.rs:209`, `:222`) | — |
| `(SELECT AVG(valor) FROM pedidos)` como coluna | Subconsulta **escalar no `WHERE`** existe (`docs/SQL.md` §7.9); **na projeção, não** | — |
| `:nome` (parâmetro nomeado) | **NÃO.** O nosso é `?` **posicional resolvido no LÉXICO** — `docs/SQL.md` §7.1, `lexico.rs:345–351`: *«substituir `?` pelo texto do parâmetro e reanalisar recriaria a injeção»* | §4.3 |
| `-- sem condição` (comentário que a crate emite, §5.4) | O léxico **aceita** `--` e `/* */` (`lexico.rs:185`, teste `comentarios_somem` `:590`) — então o SQL com o comentário **passa** e vira **produto cartesiano** sem ninguém ser avisado | §5.4 |

### 1.3. Excel: o `<dataBar>`

| item | medida |
|---|---|
| escritor XLSX existente | `src/exportar.rs`, **1.163 linhas**; a função `xlsx()` são **88** (`:379–466`) |
| o que já entrega | ZIP+XML escritos aqui, **6 partes** (`:459–464`), faixa de título, **painel congelado** (`:452–454`), **autofiltro** (`:445`), **zebra** (`:435`), largura por coluna medida no conteúdo (`:393–407`), **data como número** (25.569 dias, `:19`), e os índices do `cellXfs` **com nome** porque «número solto aqui já custou caro» (`:34–40`) |
| `<conditionalFormatting>` / `<dataBar>` | **ZERO ocorrências** em `crates/` (`.rs` e `.js`) |
| a norma | ISO/IEC 29500-1: `dataBar` tem pai **`cfRule`** (§18.3.1.10) e filhos **`cfvo`** (§18.3.1.11) e **`color`** (§18.3.1.15); atributos `minLength`, `maxLength`, `showValue`. O exemplo normativo são **três linhas**: dois `cfvo` e um `color`. `conditionalFormatting` é filho legítimo de `worksheet` (§18.3.1.18) |
| custo | **~6 linhas de XML** + a decisão de qual coluna recebe a barra e com que mínimo/máximo + 1 teste da parte + **1 prova real abrindo o arquivo**. Estimativa de 25–40 linhas de Rust. **O risco não é o XML, é a POSIÇÃO dele** no `xsd:sequence` do `CT_Worksheet` — e esta casa já pagou isso uma vez: o comentário `exportar.rs:448` diz «o congelamento vem ANTES do `sheetData` no esquema», e é por isso que a folha é montada em duas partes e emendada |

**Nota do que eu NÃO pude medir:** a posição exata de `conditionalFormatting` no
`xsd:sequence` do `CT_Worksheet` está no §A.2 da norma, e a página de referência
que consultei lista os filhos em ordem **alfabética**, não na da sequência. O
`autoFilter` do nosso escritor é hoje o **último** elemento antes de
`</worksheet>` (`:445–446`), e acrescentar o `conditionalFormatting` logo depois
dele é a aposta certa pela leitura que tenho — mas é **raciocinado, não medido**.
**O que decidiria na bancada:** escrever o arquivo com o elemento nessa posição e
abri-lo; o Excel® «não reclama de índice errado — ele obedece» (`:37`), mas de
elemento fora de sequência ele recusa o arquivo inteiro, e é isso que a prova
mostra em um minuto.

### 1.4. Relatório com bandas — e ele reabre o pedido 161

O mockup e o `.md` giram em torno de **bandas**: título / cabeçalho de grupo /
detalhe / rodapé de grupo / sumário (`_3:193–197`), com **uma query por banda**
(`_3:281–289`) e o `sql_detalhe(coluna, valor)` do `.md` para o mestre-detalhe.

Aqui isso **não existe**, e não existe **por decisão registrada**:

> **Pedido 161 — «Impressão / relatório — RECUSADO por escopo».** *«…não há
> `window.print` nem `@media print` na interface inteira… Impressão só se
> justifica se alguém precisar do **relatório paginado com cabeçalho e rodapé** —
> e não há esse pedido. Decisão do dono: excluído. **Se voltar, volta com quem
> precisa imprimir e o quê.»***

**Está voltando, pela mão do dono, e com o «o quê» anexado.** O PDF de uma
página que veio no lote é exatamente a prova disso: inflado com zlib, ele é o
relatório «Vendas (Query Designer → ODBC)», colunas `regiao`/`vendedor`/`valor`,
três linhas (Sudeste/Mega SE/24010.0; Sul/Acme Sul/6499.0; Sul/Litoral/5210.0) e
«Página 1 de 1». O «quem» é o dono; o «o quê» é relatório agrupado com subtotal e
total, em PDF. Isso **reabre o 161** e é decisão dele, não nossa (§4.1).

### 1.5. Onde o `_5` vai além do nosso, e onde fica atrás

O `_5` liga o tipo de junção **por junção**, numa cadeia `p → v → r`
(`QJOINS`, `_5:456`; `montarCore` em `:655–656`), e traz **sete** tipos com
Venn incluindo os três `EXCLUDING`, que ele resolve com `exclJoin` (`:509`)
virando `LEFT JOIN … WHERE b.chave IS NULL`.

- **A mais:** o tipo é por **elo da cadeia**, e há **galeria de referência** dos
  sete lado a lado (`_5:501–506`). A nossa tela Junção é entre **duas** fontes
  (`JN = { a, b, chaveA, chaveB, tipo }`, `index.html:9908`), uma junção por vez.
- **A menos, e é o que decide:** no `_5` os sete tipos são **texto de SQL**; aqui
  os sete **executam**, com a semântica difícil já resolvida e documentada —
  **`NULO` nunca casa com `NULO`** (`juncao.rs:23–34`: «tratar nulo como um valor
  faria todas as linhas sem chave de A casarem com todas as sem chave de B, e o
  resultado explodiria em produto cartesiano com cara de junção»), a contagem de
  chaves nulas devolvida «para que um `INNER` que devolveu menos do que se
  esperava tenha explicação em vez de mistério» (`:32–34`), e teto de linhas
  (`:40–41`). O `_5` não tem nada disso porque não precisa: ele entrega uma
  string.

**Recomendação:** a **cadeia** (tipo por elo, e mais de duas fontes) é o que vale
olhar do `_5`, e ela é um item de tela sobre motor que já existe. A galeria dos
sete é enfeite bom e barato.

---

## 2. O que RECUSAR, com o número

### 2.1. O CÓDIGO da crate — RECUSADO, e por três números

1. **Ela concatena valor dentro do SQL, sem escapar aspas.** `lib.rs:309`:
   `format!("{} {} '{}'", campo, op.para_sql_op(), self.valor.as_deref().unwrap_or(""))`.
   Idem `:298–303` (`LIKE '%{}%'`, `IN ({})` com a string crua inteira) e `:305–307`
   (`BETWEEN {} AND {}`). **Busca por escape de aspa no arquivo de 898 linhas:
   zero.** O único `replace` (`:553`) troca espaço e hífen por `_` num nome de
   função. Um valor `O'Brien` produz SQL quebrado; um valor `x' OR '1'='1`
   produz injeção. Aqui o `?` é resolvido **no léxico, por token**
   (`lexico.rs:345–351`) e o `Literal::escrever` dobra a aspa
   (`lexico.rs:122`) — adotar o código da crate seria **desfazer** a única
   defesa que temos nesse ponto.
2. **Ela não passa o portão B desta casa.** `#![warn(missing_docs)]` na linha 7,
   **91 itens `pub`** (`fn`/`struct`/`enum`/`const`) e **zero linhas `///`** no
   arquivo. Mais o `use std::collections::HashMap` da linha 13 com **zero usos**
   no resto do arquivo. O portão é `clippy --workspace --all-targets` com **zero
   avisos**.
3. **A API do `.md` não é a API do zip.** O `.md` descreve
   `consulta_designer::Consulta` com `.de()/.campo()/.agregado()/.juntar()/.sql()`
   e **`sql_detalhe(coluna, valor)`** — o mestre-detalhe, que é o recurso de
   manchete. Busca no `lib.rs`: **zero ocorrências** de `sql_detalhe`, `fn sql(`,
   `Juncao`, `fn campo`, `fn agregado`, `struct Consulta`. O zip é
   `s5124_query_designer` com `CanvasQueryDesigner::gerar_sql()`. **São duas
   gerações diferentes, e o código do recurso de manchete não veio.** «Aproveitar
   o código» do mestre-detalhe é impossível: ele não está no pacote.

### 2.2. As 7 dicas de «tuning» — RECUSADAS como estão

São casadores de texto sobre a string do SQL (`_5:770–777`). **Zero delas mede
alguma coisa.** Duas são falsas aqui e uma é contrária ao nosso motor:

- «Liste as colunas em vez de `SELECT *`» — aqui o `SELECT *` pelo direito por
  coluna já recusa nomeando a saída (`direito_coluna::SAIDAS`, pedido 245/O5), o
  que é mais útil que uma dica.
- «`OR` entre condições pode impedir índice; avalie `UNION` ou reescrita» — o
  conselho manda usar exatamente o que **não existe** aqui (`UNION`,
  `sintaxe.rs:394`). Dica que manda fazer o que o motor não faz é pior que
  nenhuma dica; é a mesma lição do pedido 245/O6, em que a recusa passou a dizer
  «declara dois índices **na criação da tabela**» em vez de mandar criar índice
  numa operação que não existe.
- «`ORDER BY` sem `LIMIT` pode ordenar muitas linhas» + o `ajusteAutomatico` que
  **põe `LIMIT 100` sozinho** (`_5:788`): isso **muda o resultado da consulta de
  quem não pediu**. Aqui a lei é «guarda nova entra pedida, não imposta»; um
  ajuste que silenciosamente trunca a resposta é a versão pior disso.

**O que substitui as dicas, e já existe medido:** `estatisticas` com p50/p90/p95
e as 15 mais lentas (`servidor.rs:19356–19404`), e o que **falta de verdade** é o
**planejador** — hoje `indices.iter().find(…)` (`traduzir.rs:395`), com a nota
honesta no próprio código: «índice escolhido pelo WHERE — **e não há
planejador**: se houvesse dois…» (`:455`). Isso já está no `docs/STATUS.md` G e
não precisa deste material para entrar.

### 2.3. O «Testar query» como no mockup — RECUSADO; a metade útil, aceita

Recusado porque aqui o botão **executa de verdade** (`claude.js:1317`) e trocar
execução por simulação seria regressão. A metade útil é a **validação estática
antes de gastar a ida-e-volta** — e mesmo essa, do jeito do mockup, é frágil:
`contarColunas` (`_5:682–698`) conta vírgulas no nível zero de uma **string** e
desiste em `SELECT *` devolvendo `'*'`. O nosso equivalente (`conferir_uniao`,
`juncao.rs:579`) confere contra o **esquema** e ainda compara família de tipo.
Aproveitar a ideia é fácil; aproveitar a implementação seria trocar esquema por
contador de vírgula.

### 2.4. O perfil `usuario`/`dba_administrador` como no mockup — RECUSADO

No mockup a permissão é um `<select>` no navegador que desabilita botões
(`_3:518–531`). Isso é **conforto de tela, não segurança**, e aqui a lei é
explícita: **portão de permissão é UM só**, e ele corre no servidor contra a
operação **traduzida** (`docs/SQL.md` §5). Espalhar a decisão pela tela é o
antipadrão que já produziu a porta dos fundos do `juntar`/`unir`. **A ideia que
passa** é a tela **perguntar ao servidor** o que este usuário pode e desabilitar
o botão — com o portão continuando onde está, e nunca no lugar dele. E o teste
que mais importa nisso é o do comportamento **velho**, no molde de
`sem_regra_de_tabela_nada_muda`.

---

## 3. O que vale aproveitar, em ordem de valor para o dono

A ordem sai da §0: o que atravessou as três gerações sem mudar vem primeiro.

### 3.1. `<dataBar>` no XLSX — o mais barato com o maior retorno

**Por que primeiro:** está nas três gerações, é o único item do lote que o
rodapé do mockup chama de «viável» com a técnica nomeada, e o escritor daqui
está a seis linhas de XML de tê-lo.

**O que mediria a premissa ANTES de construir** — e a premissa aqui não é «dá
para escrever o XML» (dá), é **onde ele entra**: escrever um `.xlsx` com
`<conditionalFormatting><cfRule type="dataBar">` logo **depois** do `autoFilter`
(`exportar.rs:445`), abrir o arquivo num leitor, e ver se abre. Se recusar, a
posição no `xsd:sequence` do `CT_Worksheet` (§A.2 da ISO/IEC 29500-1) tem de ser
lida antes de mais uma linha de Rust. **Uma corrida, cinco minutos, e ela decide
se o item custa 25 linhas ou 25 linhas mais uma remontagem da folha.**

Segunda premissa a medir, e ela é de produto: **qual coluna recebe a barra**. O
mockup decide na tela (é um `switch` numa coluna nomeada «Atingimento»). Aqui a
exportação não tem tela de estilo; a saída honesta é a barra ser **pedida**
(`"barra": {"coluna": "…", "min": 0, "max": 100}`), nunca adivinhada — porque
barra inventada sobre uma coluna numérica qualquer é **estilo mudando o
significado do dado**, que é a linha que não se cruza.

### 3.2. Construtor visual de consulta com N filtros e AND/OR — a lacuna real

**É a maior lacuna medida deste parecer:** a tela de consulta é de **uma
condição** (`index.html:13725–13732`), o motor aceita expressão inteira com
`AND`/`OR`/`IN`/`BETWEEN`/`LIKE`/`IS NULL` (`STATUS.md` G), e a interface web
manda a op `sql` de **um lugar só**, dentro do painel de IA (`claude.js:1317`).
Há motor sobrando e tela faltando.

**O que mediria a premissa ANTES de construir:** pegar as **5 consultas** que o
mockup oferece como exemplo (`_5`/`_3`, o array `EX` e os quatro chips) e as **3
do `_5` na captura**, escrevê-las como texto SQL, e passá-las pelo
`phxsql_sql::analisar` num teste unitário. O número que sai — quantas das oito
**passam hoje** — é o que diz se o construtor deve gerar SQL (e ser barrado por
`CAST`/`||`/`UNION`) ou gerar **pedido do protocolo** (`varrer.expressao`), que é
o caminho que não depende de nada que falta. **Minha leitura, não medida:** o
construtor deve produzir **pedido do protocolo**, e mostrar o SQL equivalente
**embaixo, como texto explicativo** — exatamente o que a tela Junção já faz com
os sete cartões (`index.html:9848`: «a tela ensina o nome enquanto é usada»).
Assim ele nasce inteiro em vez de nascer com quatro recusas.

### 3.3. A galeria dos sete tipos e o tipo por elo da cadeia

Enfeite bom, barato, e com motor pronto (`juncao.rs`, 13 testes). **O que
mediria a premissa:** quantas junções em cadeia o `juntar` aceita hoje num
pedido só — se for uma por vez, o item é de motor e não de tela, e muda de
tamanho.

### 3.4. Validação estática antes de executar

Sem inventar contador de vírgula: reusar `conferir_uniao` (`juncao.rs:579`) e a
recusa nomeada do tradutor, mostrando o erro **antes** do clique. **O que
mediria a premissa:** quantas das recusas do `phxsql-sql` já vêm com frase
própria (o `lib.rs` promete «recusas nomeadas, nunca "sintaxe inválida"») — se
vêm todas, a tela não precisa de validador nenhum: precisa **mostrar** a recusa
que o servidor já sabe dar.

### 3.5. O «HAVING exige agregação» do mockup

É a única das validações do `_5` (`:749`) que **nós não temos na tela** e que o
motor já sabe. Custo: uma conferência. Valor: pequeno, mas é o tipo de coisa que
o mockup acertou e que não custa discussão.

---

## 4. As decisões que são do DONO

### 4.1. Relatório com bandas reabre o pedido 161

O 161 foi recusado **por escopo**, com a condição escrita: *«Se voltar, volta com
quem precisa imprimir e o quê.»* Voltou com os dois. O que a mesa precisa
decidir **não é se é útil** (é), é se **entra no caminho crítico até a 1.0** —
os três motivos medidos da recusa original continuam de pé: não está em nenhuma
das 55 sprints; o caminho crítico é **motor** e relatório é **tela**; e
**exportar já existe** (CSV, XLSX, DOCX). O que mudou é que o dono agora nomeou
o quê. **Papel J não decide isso.** Se entrar, entra como frente própria, e o
primeiro item dela é o **mestre-detalhe** — que é a única peça do material com
desenho de verdade e cujo código **não veio no zip** (§2.1.3).

### 4.2. LLM local: a premissa do briefing estava ERRADA, e isso barateia o item

O briefing que recebi dizia: *«"LLM plugável" exigiria cliente HTTP de saída, que
NÃO existe»*, apontando `docs/SAUDE-DO-DISCO.md`. A primeira metade da frase é
o que eu também teria concluído; **medido, ela vale para a arquitetura do
mockup, não para a nossa.**

- É verdade que **não há cliente HTTP de saída na casa**:
  `docs/SAUDE-DO-DISCO.md:23` — «os únicos `GET`/`POST` escritos num `TcpStream`
  são de **testes** contra o próprio servidor (`servidor.rs:42072`, `:42198`)».
- Mas a chamada de IA daqui **não sai do servidor**: sai do `fetch` da tela
  (`docs/CLAUDE-IA.md` §1, `ui/claude.js:92`). A decisão está registrada com as
  três saídas e o custo de cada uma, e a escolhida foi «não passar pelo
  servidor», porque a `std` do Rust não tem TLS e **crate de TLS quebra a
  pétrea**.
- O Ollama local atende em `http://localhost:11434` — **HTTP simples, sem TLS**,
  e a nossa página também é servida sem TLS. Então o LLM local **não exige
  cliente HTTP de saída nem crate de TLS**: exige **uma origem a mais no
  `connect-src` da página**, que é uma linha em `src/http.rs:317`
  (`connect-src 'self' {ORIGEM_ANTHROPIC}`) — o mesmo mecanismo, a mesma folga
  mínima, travável pelo mesmo tipo de teste que já existe
  (`a_pagina_pode_chamar_a_api_da_anthropic`, e o irmão
  `a_resposta_de_dados_continua_so_com_a_propria_origem`).

**O que é do dono nisso, então, não é a pétrea — é a política:** acrescentar
`http://localhost:11434` (ou uma origem configurável) ao `connect-src` **alarga a
superfície da página**, e alargar CSP é decisão dele. E há um segundo pedaço que
**não é nosso**: o Ollama precisa aceitar a nossa origem no CORS
(`OLLAMA_ORIGINS`), o que é configuração na máquina de quem usa. **Raciocinado,
não medido:** eu não subi nem Ollama nem servidor. **O que decidiria na bancada:**
uma origem a mais no CSP, uma chamada `fetch` para `localhost:11434/api/tags`, e
ver se o navegador deixa passar — se o CORS do Ollama recusar, o item vira
«documentar a variável de ambiente», não «escrever código».

### 4.3. `:nome` da vassoura × `?` posicional nosso

O mockup cria parâmetros `:nome` («vassoura mágica», `_3:539–558`) e o nosso é
`?` posicional (`docs/SQL.md` §7.1). **Os dois estão certos, e a diferença não é
de gosto:**

- O `?` daqui é resolvido **no léxico, por token, antes da sintaxe rodar** —
  `lexico.rs:345–351`: *«substituir `?` pelo texto do parâmetro e reanalisar
  recriaria a injeção que os parâmetros existem para fechar»*. A contagem
  diferente recusa nomeando os dois números.
- O `:nome` é melhor para **tela**, porque a tela precisa rotular o campo, e é
  exatamente por isso que o mockup o escolheu: cada parâmetro aparece como
  `:id → pedidos.id (pk)` (`_3:562`).

**A decisão do dono é se `:nome` entra como SEGUNDO nome do mesmo mecanismo.**
Meu parecer: entra **na tela**, não no léxico — a tela mantém o mapa
`nome → posição` e manda `?` com a lista ordenada. Assim o léxico não ganha um
segundo caminho até o literal, que é a parte que a casa protegeu com um comentário
de sete linhas. E há precedente exato para a forma: o `abertura_do_pedido`
(`servidor.rs:12346`) documenta por que **parâmetros nomeados** ganharam dos
posicionais **no protocolo de transação** — o mesmo par de escolhas, resolvido
para o lado oposto porque o gargalo era outro (extensibilidade, não injeção).

---

## 5. O que a crate do zip NÃO tem

### 5.1. Testes: 56, e nenhum adversarial

`grep -c "#[test]"` = **56** (t01–t55 mais `t85_constitutional_zero_crash`).
Distribuição medida: **11** chamam `gerar_sql()`, o gerador da consulta inteira
(t39–t45, t49 e os do bloco constitucional); os outros exercitam **fragmentos**
(`para_sql` de um filtro, de uma coluna, de um join), **escrituração**
(adicionar/remover/mover) ou **geometria** — t01 mede distância entre dois pontos
e t02 a área de um retângulo, que são 2 de 56 testes de um gerador de SQL.

**E o que falta é o que importa: zero testes com valor hostil.** O valor mais
«difícil» de todos os 56 é `Some("João")` (t22) — um acento, que não quebra nada.
Nenhum teste passa uma aspa, e é coerente: não há código de escape para testar.

### 5.2. Dialeto: ela gera para «SQL genérico», e o nosso recusaria 5 construções

O `.md` diz que o SQL foi validado por **ODBC contra SQLite**. Passado
mentalmente pelo `docs/SQL.md` e pelo fonte de `crates/phxsql-sql/`, o que o
`phxsql-sql` **recusaria hoje** do que a crate sabe gerar:

1. `SELECT DISTINCT …` — `canvas.distinto = true` (`lib.rs:485`) → recusa nomeada
   em `sintaxe.rs:747`.
2. `RIGHT JOIN` / `FULL OUTER JOIN` / `CROSS JOIN` — `TipoJoin` tem os cinco
   (`lib.rs:169`); a camada SQL aceita dois.
3. `COUNT(DISTINCT x)` fora do `GROUP BY`, `FIRST_VALUE`, `LAST_VALUE`,
   `STRING_AGG` — `FuncaoAgregacao` (`lib.rs:346–364`) tem nove; o nosso agregado
   tem cinco mais `COUNT(DISTINCT coluna)`.
4. Coluna calculada arbitrária — `ColunaSaida::calculada` (`lib.rs:389`) aceita
   **qualquer string**, e o teste t37 usa `YEAR(t.criado_em)`, função que o nosso
   avaliador não tem (`expressao.rs:202–209`: oito funções, e `YEAR` não está).
5. `TipoDado::para_sql` emite `VARCHAR(255)`, `DECIMAL(18,4)`, `UUID`, `JSON`,
   `INTEGER[]` (`lib.rs:53–67`) — vocabulário de DDL de outro motor.

### 5.3. Injeção: ela concatena, e a linha é a 309

Já na §2.1.1, e repito aqui porque é a resposta direta à pergunta (e): **sim, ela
concatena valores**, `lib.rs:309`, sem escape, e não há um único `replace` de
aspa no arquivo.

### 5.4. Dois defeitos no fonte dela, e um pode fazer um teste dela falhar

- **Junção emparelhada por POSIÇÃO, ignorando o que a própria junção diz.**
  `lib.rs:505–512`: o laço percorre `self.tabelas[1..]` e usa `self.joins[i]` —
  o índice. A `CondicaoJoin` carrega `tabela_esq`/`tabela_dir`
  (`lib.rs:197–201`) e eles **não são consultados** para decidir a ordem. Trocar a
  ordem das tabelas no canvas troca o `ON` de lugar, calado.
- **`remover_tabela` compara `id` contra nome de tabela, e o teste t47 aparenta
  depender disso.** `lib.rs:439–441` filtra os joins por
  `c.tabela_esq == id || c.tabela_dir == id`, com `id` sendo `"t_pedidos"`
  (`lib.rs:651`); as condições guardam `"clientes"`/`"pedidos"` (`lib.rs:670–673`).
  As duas cadeias são **literais no próprio arquivo**, então a comparação é
  decidível por leitura: nenhuma casa, `any()` é falso, o `retain` **mantém** o
  join, e `joins.len()` fica **1** — enquanto t47 (`lib.rs:844`) afirma
  `assert_eq!(c.joins.len(), 0, "Join deve ser removido junto com tabela")`.
  **LIDO NO FONTE, NÃO EXECUTADO:** tentei rodar `cargo test` na crate isolada
  (target próprio, `nice -n 19`, `flock -n`) e a execução de código externo está
  barrada nesta sessão — corretamente. **O que decidiria na bancada:**
  `cargo test --offline -p phoenix-s5124-query-designer t47` na crate
  descompactada; se ela falhar, o pacote chegou com a suíte vermelha, e isso é
  informação para o dono sobre o projeto irmão, não crítica ao desenho.

### 5.5. O que ela também não tem

Nenhuma banda, nenhum `sql_detalhe`, nenhum mestre-detalhe (§2.1.3); nenhum
`HAVING`; nenhum `UNION`; nenhum parâmetro (`?` ou `:nome`) — o `FiltroVisual`
só tem valor literal; nenhum `OFFSET`; e o `preview()` (`lib.rs:560–574`)
devolve **dados inventados** (`val_0`, `val_1`…), não uma amostra do banco.

---

## 6. Onde a lógica DIVERGIRIA aqui, e por qual restrição nossa

A lei pede a pergunta, e a resposta «em lugar nenhum» significaria que a lógica
passou pelos nossos dedos e não pela nossa cabeça. Seis divergências, cada uma
com a restrição que a causa:

1. **Valor nunca entra no texto da consulta** — a crate monta `'{valor}'`; aqui o
   construtor visual produziria **pedido do protocolo** ou `?` resolvido no
   léxico. *Restrição: a defesa contra injeção é estrutural aqui
   (`lexico.rs:345–351`), e não uma função de escape que alguém pode esquecer de
   chamar.*
2. **O construtor não valida, o servidor recusa e a tela mostra** — a crate tem
   `ValidadorQuery` (`lib.rs:618–631`) com quatro regras próprias; aqui a recusa
   já é nomeada no tradutor e no motor. *Restrição: **portão de permissão é UM
   só**, e a mesma lei vale para a conferência — segunda validação na tela é o
   lugar onde as duas divergem, e a divergência aparece como bug do usuário.*
3. **Sete figuras, cinco implementações** — a crate tem cinco `TipoJoin` e o `_5`
   tem sete rótulos; o `juncao.rs` tem **sete figuras e cinco casos**
   (`:10–21`), porque «`direita` é `esquerda` com os lados trocados… e a troca
   decide qual tabela cabe na memória». *Restrição: nossa junção **executa** e
   tem de escolher quem streama e quem vira mapa — quem só gera texto não paga
   essa conta.*
4. **A ordem sai do `.ndx`, não do `ORDER BY`** — `OrdenacaoVisual` gera
   `ORDER BY t.c ASC` livremente; aqui a direção está **gravada no índice**, e o
   `SELECT` recusa a direção que o `.ndx` não tem (pedido 245/O6). *Restrição: a
   ordem de digitação é sagrada e o `.ndx` decide a ordem; um construtor que
   ofereça `DESC` sem índice `desc` gera consulta que o motor recusa.*
5. **Sem `dataBar` adivinhado** — o Excel Studio decide o estilo na tela; aqui a
   barra teria de vir **pedida** no pedido de exportação. *Restrição: **rótulo se
   estiliza, dado nunca** — barra proporcional inventada sobre uma coluna é
   aparência afirmando coisa sobre o dado que quem olha não pode conferir.*
6. **O direito por COLUNA não existe lá** — a crate seleciona colunas livremente;
   aqui `direito_coluna::SAIDAS` recusa nomeando a saída certa por classe de
   operação (pedido 245/O5). *Restrição: o direito por coluna é do motor, então o
   construtor visual tem de **esconder o que a pessoa não pode ver**, ou monta
   consulta que sempre recusa.*

E uma convergência, que também é resultado: o `_5` acrescentou, sozinho, o Venn
com sete tipos e a validação de contagem de colunas do UNION — **os dois já
existiam aqui** desde o pedido 91 e no `conferir_uniao`. Duas casas chegando ao
mesmo desenho sem se falar é o mesmo sinal do CRC-32 da página do `.ndx` com o
Cassandra®: nesse ponto, o desenho está certo.

---

## 7. Resumo: a recomendação

| # | item | recomendação | número que sustenta |
|---|---|---|---|
| 1 | `<dataBar>` no XLSX | **APROVEITAR A IDEIA** | 0 ocorrências de `conditionalFormatting` em `crates/`; ~6 linhas de XML; norma ISO/IEC 29500-1 §18.3.1.10/11/15; sobreviveu às 3 gerações |
| 2 | Construtor visual com N filtros e AND/OR | **APROVEITAR A IDEIA** | tela atual tem **1** condição (`index.html:13725`); a op `sql` é chamada de **1** lugar na interface (`claude.js:1317`); o motor já aceita a expressão inteira |
| 3 | LLM local (Ollama) | **IDEIA — decisão do dono** | **1 linha** de CSP (`http.rs:317`), não um cliente HTTP nem crate de TLS |
| 4 | Tipo de junção por elo + galeria dos sete | IDEIA, barata | motor pronto: `juncao.rs`, 1.159 linhas, 13 testes |
| 5 | Validação estática antes de executar | IDEIA parcial | `conferir_uniao` (`juncao.rs:579`) já é mais forte que `contarColunas` |
| 6 | Relatório com bandas / mestre-detalhe | **REABRE O 161 — decisão do dono** | o código do `sql_detalhe` **não veio** no zip (0 ocorrências) |
| 7 | O CÓDIGO da crate | **RECUSADO** | `lib.rs:309` concatena sem escape, 0 escapes em 898 linhas; 91 `pub` e 0 `///` sob `#![warn(missing_docs)]`; API do `.md` ≠ API do zip |
| 8 | As 7 dicas de «tuning» | **RECUSADO** | 7 casadores de texto, 0 medições; uma delas manda usar `UNION`, que não existe aqui (`sintaxe.rs:394`) |
| 9 | `ajusteAutomatico` pondo `LIMIT 100` | **RECUSADO** | muda o resultado de quem não pediu — contra «guarda nova entra pedida, não imposta» |
| 10 | Perfil na tela como controle de acesso | **RECUSADO** (a versão informativa passa) | portão é UM e é do servidor (`docs/SQL.md` §5) |
| 11 | Diagrama ER, pivô com assistente, «IA propõe/você aprova», sete Venn, validação de coluna do UNION | **JÁ TEMOS** | `diagrama-er.js` 712 linhas; `pivot.rs` 850; `CLAUDE-IA.md` §6; `index.html:9851`; `juncao.rs:579` |

**Em uma frase:** do material, **duas ideias valem construir** (a barra de dados
no XLSX e o construtor visual multi-condição), **uma é mais barata do que o
briefing supunha** (o LLM local custa uma linha de CSP, não um cliente HTTP),
**seis já existem aqui e às vezes mais fundo**, **quatro se recusam com número**,
e **uma é decisão do dono porque reabre o pedido 161** — que ele próprio fechou
dizendo «se voltar, volta com quem precisa imprimir e o quê», e agora voltou com
os dois.

---

## 8. O que eu NÃO pude medir nesta sessão

Registrado porque «número citado é número que não se mede», e porque a lista do
que falta também é palpite até alguém medir:

1. **A posição de `conditionalFormatting` no `xsd:sequence` do `CT_Worksheet`.**
   A referência que consultei lista os filhos em ordem alfabética. *Decide na
   bancada:* escrever o `.xlsx` com o elemento depois do `autoFilter` e abrir.
2. **Se o teste t47 da crate falha.** Lido no fonte com as duas cadeias literais
   à vista (§5.4); a execução de código externo está barrada nesta sessão.
   *Decide na bancada:* `cargo test` na crate isolada, target próprio.
3. **Se o navegador deixa a página falar com o Ollama.** Não subi servidor nem
   Ollama. *Decide na bancada:* origem a mais no `connect-src` + `fetch` para
   `localhost:11434/api/tags`; se o CORS recusar, o item é documentar
   `OLLAMA_ORIGINS`.
4. **Quantas das 8 consultas de exemplo do mockup o `phxsql_sql::analisar` aceita
   hoje.** Preferi a leitura à corrida porque há bancada de replicação de tempo
   rodando (papel F) e um `cargo test` disputaria o `flock`. *Decide na bancada:*
   um teste unitário em `phxsql-sql` com as oito strings, contando passa/recusa —
   e é esse número que diz se o construtor deve gerar SQL ou pedido do protocolo
   (§3.2).

# Cognição: travar `ALTER` no ledger — a régua do SQL Server não mapeia para o nosso hash

- **Assunto:** ledger — acrescentar coluna quebra o hash retroativo da cadeia
- **Descoberto:** 2026-09-15, ~21:30 (papéis C+B+F, fechando o §2.1 URGENTE da
  pesquisa do DBA sobre as *ledger tables* do SQL Server)
- **Arquivos:** `crates/phxsql-store/src/ledger.rs` (`e_tabela_ledger` e as três
  provas novas), `crates/phxsql-store/src/table.rs`
  (`Table::acrescentar_coluna`, a guarda no topo)

## 1. O que aconteceu

`conteudo_canonico` (`ledger.rs`) hasheia os valores das colunas de dado NA
ORDEM do esquema ATUAL, para QUALQUER linha — inclusive as antigas, relidas para
`verificar_cadeia`. `Table::acrescentar_coluna` já existia (pedido 40, `ALTER
TABLE ADD COLUMN`), e nada nele sabia do modo ledger. Logo: acrescentar uma
coluna a uma tabela em modo ledger com blocos gravados deslocava o conteúdo
canônico de toda linha antiga, o `hash_do_bloco` recalculado deixava de bater o
`hash` gravado, e `verificar_cadeia` passaria a gritar **adulteração numa cadeia
intacta** — falso positivo — ou, no pior caso, a mascarar uma real. Risco atual,
não hipotético: o caminho de escrita já estava no ar.

O conserto: o motor RECUSA `acrescentar_coluna` numa tabela em modo ledger, com a
guarda no TOPO de `Table::acrescentar_coluna` — o único ponto por onde o
`op_acrescentar_coluna` do servidor passa (portão único, não espalhado por
operação, como manda a pétrea do portão de permissão).

## 2. O que eu concluí primeiro, e estava errado

**Errei em DOIS pontos, e os dois vinham de copiar a régua alheia sem medir
contra o nosso desenho.**

- **A régua do SQL Server pareceu adotável, e não é.** O SQL Server permite
  `ALTER` numa *ledger table* desde que a coluna seja *nullable*, entre no FIM e
  seja IGNORADA no hash. Meu primeiro plano foi relaxar a nossa guarda para o
  mesmo — «deixa passar coluna nula no fim». Estava errado, e a razão é do nosso
  conteúdo canônico: ele pula coluna por **NOME** (`hash`, `assinatura`, colunas
  de sistema), **não por posição-no-fim**. Uma coluna nova, mesmo nula e no fim
  da lista do usuário, cai em `posicao_de_coluna_nova` (antes de `softdeleted`) e
  **entra** na varredura do `conteudo_canonico` — some como um `Null` (0x00) no
  meio da série e desloca o hash. A régua do SQL Server só mapearia se o nosso
  hash excluísse a coluna nova por alguma marca, e ele não tem essa marca. A
  proibição INTEIRA é o que casa com o nosso desenho.

- **Achei que a recusa devia depender de a tabela ter blocos.** «Tabela ledger
  vazia não tem passado para quebrar, então deixa alterar.» Errado: o modo ledger
  é DEFINIDO pelo esquema, e a garantia da cadeia é que todo bloco foi hasheado
  sob EXATAMENTE este layout de colunas. Deixar alterar a vazia cria um footgun
  onde a recusa depende de um estado (vazia/cheia) que muda no primeiro
  `inserir`. A recusa é **absoluta** — nem nula, nem com padrão, nem em tabela
  vazia. A imutabilidade do esquema é o que torna a cadeia verificável; mexer no
  esquema é mexer no passado.

## 3. O que a medição disse

- **Prova real nos dois sentidos, medida:** com a guarda desligada
  (`if false && e_tabela_ledger(...)`), `acrescentar_coluna` numa tabela ledger
  com 4 blocos e uma coluna NULA passa em todas as outras conferências e retorna
  `Ok` — o teste `alterar_tabela_em_modo_ledger_e_recusado` entra em pânico no
  `unwrap_err`. Com a guarda, recusa nomeando a tabela e o motivo (`ledger`).
- **O defeito é real, não um medo:** `acrescentar_coluna_deslocaria_o_conteudo_do_bloco`
  mede na função pura — o mesmo bloco, sob um esquema com uma coluna de dado a
  mais na posição de coluna nova, dá `hash_do_bloco` DIFERENTE. É o que
  `verificar_cadeia` veria como adulteração.
- **A guarda é específica:** `tabela_comum_ainda_aceita_coluna_nova` prova o
  comportamento VELHO — uma tabela sem a cadeia continua alterável. Sem esse
  teste, a guarda poderia ter travado todo `ALTER` sem ninguém ver.
- **Reconhecimento por convenção:** `reconhece_o_modo_ledger_pelas_quatro_pecas`
  — as três colunas com os tipos certos MAIS o índice único `porAltura`. Faltando
  uma peça (o índice único), não é ledger: as colunas sozinhas não bastam, porque
  sem o índice a verificação não varre em ordem.
- **181 testes do store + 1037 do servidor** verdes; `fmt` e `clippy
  --all-targets` sem aviso.

## 4. A regra

**Modo ledger é convenção de esquema, não flag: hash+anterior (Uuid256), altura
(Sequence) e o índice único `porAltura`. Uma tabela em modo ledger NÃO aceita
coluna nova — recusa absoluta, no motor, no único portão. Régua de fora só entra
medida contra o NOSSO hash: o conteúdo canônico pula coluna por NOME, então a
relaxação «nula no fim, fora do hash» do SQL Server não vale aqui.**

## 5. Como está guardado hoje

- `ledger::e_tabela_ledger(esquema)` — o predicado das quatro peças, público para
  quem mais precisar reconhecer o modo.
- Guarda no topo de `Table::acrescentar_coluna`, antes de qualquer outra
  conferência; o `op_acrescentar_coluna` do servidor passa por ela sem código
  próprio.
- Quatro provas em `ledger.rs`: `reconhece_o_modo_ledger_pelas_quatro_pecas`,
  `alterar_tabela_em_modo_ledger_e_recusado` (com a cadeia seguindo íntegra depois
  da recusa), `tabela_comum_ainda_aceita_coluna_nova` e
  `acrescentar_coluna_deslocaria_o_conteudo_do_bloco`.
- **Onde o buraco fica:** os outros TRÊS itens do §2.1 do DBA seguem abertos, e
  são endurecimento (decisão do dono sobre escopo): recusar `UPDATE`/`DELETE` no
  MOTOR para tabela ledger (hoje nada em `Table::atualizar` sabe do modo — é o
  caminho que o próprio teste de adulteração explora), âncora externa pela
  replicação (cada réplica testemunha o último `(altura, hash)`), e `DROP` de
  tabela-ledger com confirmação redobrada (uma cadeia com blocos é «um pai que
  sempre tem filhos»). E um limite herdado: `e_tabela_ledger` reconhece pela
  convenção, então uma tabela comum que por acaso reúna as quatro peças seria
  tratada como ledger — o que é aceitável, porque reunir hash/anterior/altura +
  `porAltura` único sem querer uma cadeia é implausível, mas fica nomeado.

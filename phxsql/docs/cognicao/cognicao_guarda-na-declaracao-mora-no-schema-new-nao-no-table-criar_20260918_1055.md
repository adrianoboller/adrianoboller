# A guarda «na declaração» mora no `Schema::new`, não no `Table::criar`

Pedido 355 — modo ledger deixa de aceitar coluna marcada como dado pessoal.
Descoberto em 18/09/2026, ~10h55, enquanto se escolhia onde pôr a recusa.

## 1. O que aconteceu

O modo ledger grava um SHA-256 **sem sal** do conteúdo em claro numa coluna
`hash` (`Uuid256`) que ninguém marca — e, por não ser marcada, ninguém cifra.
Ela sobra em claro ao lado do dado selado, e quem tem a lista dos valores
possíveis confirma qual está ali por tentativa. Decisão do dono: **a combinação
não nasce mais, recusada na declaração**, e **sem desfazer cadeia que já
exista**.

A pergunta que consumiu o trabalho não foi *o quê* — foi **onde**. Há quatro
candidatos, e três deles quebram alguma coisa:

| lugar | o que ele vê | o que ele quebraria |
|---|---|---|
| `Table::criar` (`table.rs:548`) | todo nascimento de tabela | a semeadura de **réplica** |
| `Schema::do_disco` (`schema.rs:744`) | tudo, inclusive o disco | **não abre** a cadeia velha |
| `valores::esquema_de_json` (servidor) | só o pedido JSON | não vê `db.criar_tabela` interno |
| `Schema::new` (`schema.rs:692`) | só quem **declara** | nada |

## 2. O que eu concluí primeiro, e estava errado

«A recusa vai no `Table::criar`: é o funil único — `Instancia::criar_tabela`,
`criar_espelhada`, CLI, exemplos e o servidor passam todos por lá, e um portão
só não se esquece.»

Está errado, e o motivo não aparece lendo o `table.rs`: **a réplica cria a
tabela local a partir do esquema DESSERIALIZADO do source**, e não de uma
remontagem por JSON — `servidor.rs:2871` (`abrir_para_replicar`) e
`servidor.rs:4484` (o caminho bidirecional), os dois chamando
`db.criar_tabela(schema, e.clone())` com o `Schema` que veio do `posicao`. Uma
guarda no `Table::criar` recusaria semear uma réplica de uma cadeia que **já
existe** — e «não desfaz cadeia que já existe» era metade da ordem do dono.

E o alcance disso é maior que a réplica, embora só a réplica esteja medida: todo
caminho que reconstrua uma tabela a partir do **bloco de esquema gravado** cairia
junto. O `restaurar.rs` de hoje não é um deles — conferido, ele não chama
`Table::criar` nem monta `Schema` —, mas o próximo que for nasce quebrado sem
ninguém ver.

## 3. O que a medição disse

O `Schema` já tem a fronteira pronta, e ela está **documentada desde antes**:
`Schema::new` é «esquema de uma tabela NOVA» e diz na própria doc *«quem lê
esquema do disco não passa por aqui — ver `Schema::do_disco`»*. O `do_disco` é
o caminho da leitura, e o `new` termina delegando nele.

Contado no fonte, quem chama cada um:

- `Schema::new` — `valores::esquema_de_json` (`valores.rs:617`, o
  `criar_tabela` do protocolo), CLI, exemplos e testes. **Todo mundo que
  declara.**
- `Schema::do_disco` — `Schema::desserializar` (`schema.rs:1615`, o `PSCH` do
  `.reg`) e `Schema::com_coluna`. **Todo mundo que lê o que já está gravado.**

E o precedente estava a 150 linhas de distância, com o motivo escrito: a guarda
de `id` repetido de coluna mora «nos dois caminhos de DECLARAR: aqui, para a
tabela nova, e em `Schema::com_coluna`, para a coluna que chega depois» — e
**não** no `do_disco`, «que é o de LER». A guarda nova entrou nos **três**
verbos de declarar do mesmo objeto: `new`, `marcar_dado_pessoal` e `com_coluna`.

Prova real nos dois sentidos, medida: com a guarda posta no `do_disco` (o
defeito reposto do lado contrário), o teste do comportamento velho morre **no
`abrir`** —

```
called `Result::unwrap()` on an `Err` value: Esquema("a tabela blocos esta em
modo ledger e a coluna cpf esta marcada como dado pessoal: ...")
```

— ou seja, a cadeia legada sairia do ar. Com a guarda nos verbos de declarar,
ela abre, lê e continua gravando bloco novo.

## 4. A regra

**Guarda que recusa «na declaração» mora no verbo que DECLARA, não no que
grava: procure a fronteira entre construir e reler que o tipo já tem — e se ela
não existir, crie-a antes da guarda.**

Corolário do irmão: os verbos de declarar de um objeto são mais de um. Aqui
eram três, e o segundo (`marcar_dado_pessoal`) é a porta dos fundos exata —
nascer limpo e marcar no pedido seguinte.

E o refinamento que a mesma lei cobrou de volta: no `marcar_dado_pessoal` a
guarda olha a **transição** (não-marcada → marcada), e não o estado. Uma guarda
de estado recusaria **desmarcar** numa cadeia que já nasceu marcada — que é
justamente o único remédio dela.

## 5. Como está guardado hoje

- `crates/phxsql-core/src/schema.rs` — `e_tabela_ledger`,
  `coluna_no_hash_do_ledger`, `pessoal_coberta_pelo_hash`,
  `conferir_ledger_sem_dado_pessoal` e a chamada nos três verbos.
- `crates/phxsql-store/src/ledger.rs` — reexporta os nomes (uma definição só) e
  usa `coluna_no_hash_do_ledger` no `conteudo_canonico`: a lista do que entra no
  hash e a lista do que a guarda protege são **a mesma**.
- Testes: `testes_ledger_com_dado_pessoal` (8, no core) e três no
  `ledger::testes` (o irmão pelo funil `Table::marcar_dado_pessoal`, o
  comportamento velho em disco e a fronteira do sentido contrário).
- `docs/FORMATO.md`, §«A marca de dado pessoal (LGPD / GDPR), v6» — o byte no
  disco **não muda**, e é isso que faz a guarda não quebrar nada.

**Onde o buraco ficou:** o sentido contrário — ligar modo ledger numa tabela que
já tem coluna marcada — não tem guarda própria porque **não tem caminho**: não
há operação de criar índice depois, e `acrescentar_coluna` recusa toda coluna
`Sequence`. O que existe é o teste `nao_ha_caminho_para_virar_ledger_depois`,
que monta a tabela a uma peça de ser cadeia e cai no dia em que qualquer uma das
duas portas abrir.

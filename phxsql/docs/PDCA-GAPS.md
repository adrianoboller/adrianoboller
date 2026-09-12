# PDCA-GAPS — o ciclo dos gaps abertos do PhxSql

**Fases cobertas:** Plan + Check. **Papel:** J (Pesquisador), rodada de
12/09/2026, estruturada aqui pelo papel H (Documentação) no mesmo dia.
**Fases que faltam:** Do + Act — ficam para quando os papéis convocados abaixo
executarem cada recomendação; este documento não implementa nada, só organiza
o que já foi medido para que a implementação não comece sem premissa
conferida.

Este é o documento de conhecimento do PDCA dos gaps: não substitui o
`docs/PENDENCIAS.md` (que registra o pedido, o estado e a data) nem os
documentos de área (`docs/AUTONUMBER.md`, `docs/ODBC.md`,
`docs/TRANSACOES.md`, `docs/propostas/semantica-4-motores.md`…) — ele é o
lugar único onde a fase **Plan** (hipóteses) e a fase **Check** (o que foi
medido, ou marcado para medir) de cada gap ficam lado a lado, com a fonte de
cada número.

**Regra que este documento obedece de ponta a ponta:** todo número aqui ou
vem com o arquivo e a seção de onde o papel J o tirou, ou traz a marca dele,
**«A MEDIR NA FASE DO»**, com o comando exato. Nenhum número foi gerado,
arredondado ou lembrado de memória para este documento — ele estrutura a
pesquisa do J, não a repete de cabeça.

## Divisão Plan / Do / Check / Act nesta rodada

- **Plan** — o papel J levantou, para cada gap aberto, as hipóteses
  concorrentes e a premissa que decide entre elas.
- **Check** — o papel J mediu cada premissa contra o código-fonte (leitura,
  sem `cargo` — a árvore estava ocupada por outro agente), contra os
  documentos de área já existentes, e contra a convergência/divergência dos
  três motores maduros (`docs/propostas/semantica-4-motores.md`). Onde a
  medição fresca exigia compilar ou rodar bancada, ficou marcada para a
  próxima fase.
- **Do** — não ocorreu nesta rodada. É onde B, C, E, F e G (convocados
  abaixo) implementam as recomendações da coluna **Act**.
- **Act** — a recomendação medida de cada gap, já registrada abaixo; vira
  trabalho de verdade só na fase Do.

## Papéis convocados e dispensados nesta pesquisa (pétrea A — dispensa registrada)

- **J** (dono da rodada) — levantou hipótese e mediu a premissa antes de
  qualquer plano.
- **C / DBA** — convocado nos gaps **229** (formato PSCH) e **242** (garantia
  de dado na transação).
- **B / engenharia** — convocado no **238** (o trabalho é no driver ODBC) e no
  **240** (contrato do tradutor SQL).
- **E / designer** — **dispensado do trabalho** nesta rodada, só dimensionado,
  no **230** (é construção de tela, não pesquisa).
- **G / QA, F / prova real** — nomeados onde a fase Do precisa de guarda ou de
  prova real; a medição fresca de cada um está marcada **«A MEDIR NA FASE
  DO»** com o comando exato.
- **D, H, I** — dispensados por não tocarem o domínio de uma pesquisa de
  premissa que não grava nada em disco nem em release.

A régua de decisão usada em todo o documento é a do dono (`CLAUDE.md`):
**convergência dos três motores maduros = aceite automático**; **onde
divergem, média ponderada PostgreSQL 4 / MariaDB 3 / MySQL 2 / SQLite 1**;
**convergência não revoga pétrea nossa** (o choque aparece, não se afoga); **o
que o dono já fechou não se reabre por conta própria** — documenta-se o custo
medido, e é isso que a seção final de gaps fechados faz.

---

## Quadro-resumo

| gap | recomendação em uma linha | classe |
|---|---|---|
| **240** — `[NOT] EXISTS` por apelido de fora | Convergência unânime dos 4 motores + mudança aditiva → **fazer**. | código nosso (B / tradutor) |
| **242** — CHECK só no COMMIT | Os 3 motores maduros recusam CHECK **na instrução** → aceite automático. Mover padrão/calculada/CHECK para o `empilhar`. | código nosso (B, garantia validada pelo C) |
| **229** — auto-number, o que falta | Itens `Uuid`, `verificar`+`reparar` e `Sequence` no INSERT são código sem formato; `inicio`/`passo`, `IDENTITY ALWAYS` e sequência nomeada mudam o PSCH → decisão do dono/DBA, **entram cedo**. | misto: código nosso + formato-dono |
| **238** — ODBC, parâmetro de SAÍDA | Premissa "precisa de operação de protocolo nova" **morreu medida**: o `CALL` já devolve `saida` (valor além da linha). Não é decisão de protocolo — é conserto no driver. | código nosso (B / driver), **não** protocolo |
| **230** — aba de Usuários | As três operações de cadastro já existem (pedido 221); a tela só lê. É construção de tela sobre protocolo pronto + chaves de idioma. | construção de tela (E) |
| **239** — isolamento acima de READ COMMITTED + TLS | Parado por decisão do dono; custo medido e na mesa. | fechado-dono |
| **164** — trava global / MVCC | `RwLock` ganha 2,48×–2,99× em leitura, mas custa o invariante `!Sync`; parado. | fechado-dono |
| **179** — teto do MVCC / Sombra | Ganho de velocidade morreu medido (~1,00×–1,21× no padrão `por_lote`); a Sombra só compraria leitura repetível. Parado. | fechado-dono |
| **207** — quórum de escrita | Rota do canal aberto já **decidida** pelo dono; falta decidir o significado do "ok" — não é reabertura, é o próximo passo do próprio dono. | fechado-dono (rota decidida) |

---

## GAP 240 — `[NOT] EXISTS` correlacionado por apelido de fora recusa quando devia rodar

**Plan (hipóteses):**
- **H1 (a prescrição do PENDENCIAS):** o tradutor passa a emitir o
  `apelido_de` do `de` sempre que `existe`/`escalar`/`em` citarem uma coluna
  dele; o `consultar` passa a aceitar `apelido.coluna` na projeção e na ordem
  **sem junção**; e a `expressao` do sub-pedido do EXISTS perde o
  qualificador interno.
- **H2 (alternativa):** resolver a correlação só no motor, sem tocar o
  tradutor. **Descartada na premissa** — o motor não recebe hoje o apelido de
  fora (`apelido_de` só entra quando há junção, `consulta.rs:176`), então não
  teria como saber que `c.id` é coluna de fora.

**Premissa:** H1 só é a melhor forma se (a) os quatro motores concordarem que
a forma-modelo deve rodar, e (b) a mudança for **aditiva** (aceitar mais),
nunca subtrativa — senão bate na pétrea de não quebrar cliente antigo.

**Check (medido):**
- **Convergência dos 4** — `docs/propostas/semantica-4-motores.md` §2, linha
  «`[NOT] EXISTS` correlacionado por apelido de fora»: PostgreSQL, MariaDB,
  MySQL e SQLite **rodam** todos. Convergência unânime → aceite automático,
  sem conta de média ponderada.
- **Causa medida no código** (leitura, sem `cargo`): a forma-modelo de
  `docs/SQL.md` §10 (`FROM clientes c … WHERE x.cliente_id = c.id`) hoje
  devolve `[]`. O tradutor (`consulta.rs`, campo `apelido_de`, comentado
  «só entra no pedido quando há junção») não emite o apelido do `de`; o
  `consultar` do `servidor.rs` só aceita prefixo de fora quando é o apelido do
  `de` ou o nome da tabela. O substrato de execução (`existe`, com
  `esquerda`=coluna de fora e `direita`=coluna de dentro) já existe desde o
  pedido 236 (`docs/SQL.md` §4 e §10).
- **A MEDIR NA FASE DO:** `cargo test -p phxsql-sql` + `cargo test -p
  phxsql-server` após a mudança; teste do defeito reposto (a forma-modelo
  volta a devolver linhas em vez de `[]`) e teste do comportamento **velho**
  (um `consultar` com junção continua idêntico).

**Choque com pétrea?** Não. A mudança do `consultar` é aditiva: passa a
aceitar `apelido.coluna` sem junção onde antes recusava. O único risco de
contrato seria emitir `apelido_de` "sempre"; a prescrição limita a emissão a
quando `existe`/`escalar`/`em` citam a coluna, preservando o caminho comum.
Guarda do comportamento velho é obrigatória (papéis F/G).

**Act (recomendação medida):** **Fazer a H1.** Código nosso (tradutor +
`consultar`), convergência unânime, mudança aditiva, substrato de execução já
pronto desde o pedido 236 — falta só o tradutor entregar o apelido de fora e
o `consultar` aceitá-lo sem junção. Prova real nos dois sentidos na fase Do.

---

## GAP 242 — CHECK dentro de transação é julgado só no COMMIT e derruba tudo

**Plan (hipóteses):**
- **H1 (prescrição):** aplicar as regras do esquema — padrão, calculada e
  CHECK — sobre a linha tipada no `empilhar` (`servidor.rs`), ao lado da
  unicidade e do gatilho `BEFORE` que já rodam ali, para o CHECK ser recusado
  **na instrução**, não no COMMIT.
- **H2 (alternativa):** manter o julgamento no `t.inserir` do commit e só
  melhorar a mensagem. **Descartada na premissa** — não resolve o defeito de
  produto (a transação inteira ainda cai no COMMIT por erro de uma única
  instrução) e diverge dos três motores maduros.

**Premissa:** H1 só é a melhor forma se os três motores recusarem violação de
CHECK **na instrução** (não só no commit), e se o read-your-own-writes já
existir para a linha empilhada — porque CHECK/calculada podem depender de
outra coluna da mesma linha.

**Check (medido):**
- **Convergência dos 3** — CHECK é constraint imediata em PostgreSQL
  (`IMMEDIATE`, avaliada ao fim de cada `statement`), MySQL 8.0.16+ e MariaDB
  10.2+ (CHECK reprovado rejeita a instrução, não o commit) — mesmo tempo em
  que chave duplicada e `SIGNAL` já são julgados. Convergem, sem divergência a
  ponderar → aceite automático (`docs/propostas/semantica-4-motores.md`
  §1–§2).
- **O que já roda no `empilhar` hoje** — `docs/TRANSACOES.md` §3.3: «o que
  pode falhar é conferido na hora do `INSERT`, não na hora do `COMMIT`. A
  conversão da linha, os gatilhos `BEFORE` e a unicidade rodam ao empilhar».
  Falta só padrão/calculada/CHECK — exatamente o buraco nomeado no
  PENDENCIAS #242 («o empilhar confere chave única mas não roda
  padrão/calculada/CHECK; quem julga é o `t.inserir` no commit»).
- **Read-your-own-writes existe** (pedido 162 / SP000006) —
  `docs/TRANSACOES.md` §4.4.1: «ela enxerga o que ela mesma escreveu». Uma
  calculada/CHECK que consulte a própria linha empilhada tem o dado.
  **Ressalva medida:** `Sequence`/`rownum` só nascem no COMMIT
  (`docs/TRANSACOES.md` §4.4.1, item 2) — um CHECK dependente de `Sequence`
  não pode ser julgado no empilhar com o número final; é o mesmo limite que
  a §B.2.1 do AUTONUMBER (gap 229) ataca em separado.
- **Adjacência que não é este gap:** o defeito grave de FK-em-transação
  (`Table::conferir_fks` lê a mãe do disco e não vê o pai empilhado,
  `docs/propostas/semantica-4-motores.md` §1.1) é outro item —
  read-your-own-writes da **conferência de constraint**, não do CHECK.
- **A MEDIR NA FASE DO:** `cargo test -p phxsql-server` com o reteste
  `p07b_esquema_reteste.py` reposto — o teste do defeito reposto é uma
  transação com CHECK violado numa instrução do meio: hoje só falha no
  COMMIT (derruba tudo); depois tem de falhar **na instrução**, deixando o
  resto da transação de pé.

**Choque com pétrea?** Não. Aplicar a regra do esquema mais cedo fortalece a
garantia de dado (domínio do C/DBA), não a afrouxa. Não muda formato. A
recusa passa a acontecer na instrução, coerente com «recusar cedo custa um
erro lido, recusar tarde custa a transação inteira».

**Act (recomendação medida):** **Fazer a H1** — mover padrão/calculada/CHECK
para o `empilhar`, ao lado de unicidade/BEFORE. Bate com a convergência dos
três (aceite automático). **Nomear** a ressalva do `Sequence`/`rownum`, que
continua só verificável no commit até o item B.2.1 do gap 229 entrar. Papel
B, com a garantia validada pelo C.

---

## GAP 229 — auto-number: o que falta (com chapéu de DBA)

`docs/AUTONUMBER.md` já é uma pesquisa J completa, com a tabela de migração
B.4 medida (custo, muda-formato?, quebra-cliente?). Abaixo, item a item, a
premissa medida «este sub-item exige mudança de formato PSCH?» — porque
formato é do dono/DBA e entra cedo ou vira migração.

**Plan (hipóteses):** por sub-item do "ideal" ainda aberto — `inicio`/`passo`
no esquema; `IDENTITY … GENERATED ALWAYS`; `Uuid` que nasce sozinho;
sequência nomeada (`CREATE SEQUENCE`); alargamento da recusa `>2⁵³` ao
`Int8`/`UInt8`.

**Premissa de cada hipótese:** «muda o formato PSCH?»

**Check (medido, cada premissa contra `docs/AUTONUMBER.md` §B.4 e
`docs/FORMATO.md`):**

| sub-item | muda PSCH? | custo medido | quebra cliente? | fonte |
|---|---|---|---|---|
| `Uuid` nasce sozinho (valor nulo → v7) | não | 1 ramo no `numerar` (`table.rs:2307`) | não | AUTONUMBER §B.2.5, §B.4 #2 |
| recusar valor cru `>2⁵³` (defeito b) | não — **já fechado** pela frente G2 | 2 comparações, em `json_para_valor` | não (hoje já corrompe) | AUTONUMBER §C.1, §B.4 #3 |
| `verificar` reconta `Sequence` + `reparar` empurra | não | dentro de varredura que já existe | não | AUTONUMBER §B.2.7, §C.3, §B.4 #4 |
| `Sequence` numerada no `INSERT` (não no COMMIT) | não | reserva na `Escrita` + devolução | não (hoje lê `null`) | AUTONUMBER §B.2.1, §B.4 #5 |
| `inicio`/`passo` no esquema | **sim** — 2×`u64` por coluna `Sequence` no PSCH | entra cedo, enquanto não houver `Sequence` com faixa em produção | não (ausente = 1 e 1) | AUTONUMBER §B.2.2, §C.2, §B.4 #6 |
| `IDENTITY … GENERATED ALWAYS` | **sim** — 1 byte no PSCH | nasce desligado (guarda pedida) | não (`sem_identity_declarada_nada_muda`) | AUTONUMBER §B.2.3, §B.4 #7 |
| sequência nomeada (`CREATE SEQUENCE`) | arquivo **novo**, não altera PSCH existente | 3 ops + arquivo de 128 B; `fdatasync` 83,5 µs/número (72,6–125,0), 167× o contador atual (0,50 µs) | não | AUTONUMBER §B.2.4, §B.4 #8 |
| alargar recusa `>2⁵³` a `Int8`/`UInt8` | não | 2 comparações | **sim** — muda comportamento de todo cliente que hoje manda `Int8` | AUTONUMBER §C.1 |

**O que os três motores fazem** (consenso, não cópia): o padrão SQL e os três
oferecem `GENERATED ALWAYS`/`BY DEFAULT`, com `BY DEFAULT` sendo o padrão de
fábrica — que é o comportamento de hoje aqui. Ligar `ALWAYS` por padrão
divergiria do consenso **e** quebraria importação/restauração/réplica
bidirecional: convergência e pétrea de "guarda pedida" dizem a mesma coisa,
`ALWAYS` é opt-in. O `auto_increment_increment/_offset` do MariaDB é a origem
do `inicio`/`passo`, mas a restrição nossa (o contador é do `.reg` e viaja com
o arquivo) manda pôr a faixa no esquema da tabela, não em variável de
servidor — divergência justificada, registrada em AUTONUMBER §B.2.2.

**Nota de formato conferida:** o PSCH está hoje na **versão 9**
(`docs/FORMATO.md`, "O bloco de esquema (`PSCH`, versão 9)"); o byte por
chave que o `CLAUDE.md` cita como "v7" continua vindo desde a v7, com as
versões seguintes lendo o antigo com padrões. `inicio`/`passo` e
`IDENTITY ALWAYS` seriam a próxima versão do PSCH, no mesmo molde.

**Choque com pétrea?**
- **Ordem de digitação:** reaproveitar número de `Sequence` de linha excluída
  está recusado (AUTONUMBER §B.3) — a `Sequence` viaja para fora (nota,
  recibo) e reemitir seria dois documentos com a mesma identidade. Nenhum
  sub-item proposto quebra isso.
- **Zero deps:** nenhum sub-item pede crate (`Uuid` v7 e CRC-32 são da casa).
- **Formato entra cedo:** é o ponto central — os itens 6 e 7 têm de entrar
  antes de haver `Sequence` com faixa em produção, senão viram migração.

**Act (recomendação medida):**
- **Código nosso, sem dono (podem entrar já, em qualquer ordem):** `Uuid`
  nasce sozinho (#2), `verificar` reconta + `reparar` (#4), `Sequence`
  numerada no INSERT (#5). O #5 tem premissa a medir antes (AUTONUMBER
  §B.2.1): custo de devolver o contador em ordem no ROLLBACK. **A MEDIR NA
  FASE DO:** `cargo build --release --examples -p phxsql-store` e a bancada
  `--example custo-da-transacao` com reserva de número e rollback fora de
  ordem.
- **Decisão do dono/DBA (formato PSCH, custo na mesa):** `inicio`/`passo`
  (#6) e `IDENTITY ALWAYS` (#7) entram cedo (nova versão de PSCH que lê o
  antigo com padrões) ou viram migração; sequência nomeada (#8) é arquivo
  novo e pode entrar sem migrar os existentes, mas paga `fsync` por número
  (`CACHE` desligado por padrão é decisão de produto).
- **Recusa medida a levar ao dono:** alargar a recusa `>2⁵³` a
  `Int8`/`UInt8` muda o comportamento de todo cliente que hoje manda
  `Int8` — a frente G2 já recusou fazer isso sozinha (AUTONUMBER §C.1); só
  entra como decisão explícita do dono.

---

## GAP 238 — ODBC: parâmetro de SAÍDA/ENTRADA-SAÍDA recusa

**Plan (hipóteses):**
- **H1:** criar uma operação de protocolo nova que devolva "um valor além da
  linha" — a leitura literal do PENDENCIAS #238.
- **H2:** usar o que já existe — o `CALL` de procedimento já devolve os
  `OUT`/`INOUT` num objeto `saida`, transportado no `resultado` do envelope;
  o trabalho que falta é no driver ODBC, não no protocolo.

**Premissa:** H1 só é necessária se nenhuma operação do servidor devolver
valor além da linha. H2 é a melhor forma se já existir uma operação que
devolve OUT, e se os motores de referência também devolverem OUT como
resultado da chamada, não como campo de fio exótico.

**Check — a premissa do PENDENCIAS morreu medida, e a evidência abaixo foi
conferida diretamente contra o código-fonte pelo orquestrador desta rodada
(números de linha exatos, não aproximados):**

- **O `CALL` já devolve valor além da linha.** Em
  `crates/phxsql-server/src/servidor.rs:15062-15078`, a operação
  `chamar_procedimento` (que atende `CALL`) monta o objeto `saida` a partir
  de **todo** parâmetro cujo `modo != Modo::Entrada` (ou seja, `OUT` e
  `INOUT`), lendo `ctx.valor_de(&q.nome)` **depois** de `executar`, e devolve
  `Json::objeto([("procedimento", …), ("saida", Json::Objeto(saida))])`. A
  premissa de que "nenhuma operação devolve valor além da linha" — repetida
  em `docs/PENDENCIAS.md` #238 e `docs/ODBC.md` §2.1.1 — é **falsa**:
  `docs/TRIGGERS.md` §1 já documenta o contrato ("`CALL` devolve os
  `OUT`/`INOUT` num objeto `saida`"). O `CALL` chega pela operação `sql`
  (`{"op":"sql","sql":"CALL somar(?)"}`), é parseado em `Comando::Chamar` e
  roda com o poder de quem chamou (`executar_derivado`, mesmo portão).
- **Onde o driver para hoje** (medido em `crates/phxsql-odbc/`):
  1. `parametro.rs:276-277` recusa parâmetro não-entrada com **`HYC00` na
     ligação** (`SQLBindParameter`), antes mesmo de olhar o tipo C;
  2. `resultado.rs:218-264` (`montar`) consome **só**
     `resposta.campo("linhas")` — o objeto `saida` não é lido em lugar
     nenhum.

  Ou seja: **o vaso já existe no protocolo; o driver é que não liga o OUT nem
  lê o `saida`.**
- **Convergência dos motores de referência (como o protocolo devolve OUT,
  não para copiar código):** PostgreSQL devolve parâmetros OUT de
  função/`CALL` como uma linha de resultado (`RowDescription`+`DataRow`) —
  não há mensagem de fio dedicada a "OUT". MySQL devolve OUT/INOUT de um
  `CALL` (protocolo binário de prepared statement) como um resultset extra
  marcado `SERVER_PS_OUT_PARAMS`. Os dois entregam OUT como conteúdo do
  resultado da chamada, não como campo exótico — o nosso `saida` é o análogo
  direto. Convergência a favor de H2; não há divergência a ponderar (ambos
  "devolvem no resultado").

**Choque com pétrea?** Não.
- **Não quebra cliente antigo:** `saida` é campo adicional no envelope,
  presente só na resposta de um `CALL`; respostas de `SELECT`/`INSERT`
  continuam idênticas. Ligar `SQL_PARAM_OUTPUT` passa a **aceitar** onde
  antes recusava — mudança aditiva.
- **Zero deps:** a conversão de borda UTF-16↔UTF-8 já é `std` (`texto.rs`),
  a mesma máquina do `SQL_C_WCHAR` fechado na metade de 09/09.

**Act (recomendação medida):** **Não é decisão de protocolo — é trabalho do
driver (papel B).** O protocolo já entrega OUT via `resultado.saida` do
`CALL`, na mesma forma que PostgreSQL e MySQL usam. Fazer em
`crates/phxsql-odbc/`:
1. `SQLBindParameter` com `SQL_PARAM_OUTPUT`/`SQL_PARAM_INPUT_OUTPUT` deixa
   de recusar `HYC00` e guarda o ponteiro do buffer do aplicativo
   (`parametro.rs`).
2. o escape ODBC `{call proc(?)}` mapeia para `CALL proc(?)` na operação
   `sql` (`lib.rs::executar_sql`).
3. depois de executar, `resultado.rs` lê o objeto `saida` e escreve cada
   valor OUT no buffer ligado, com a mesma conversão de borda do
   `SQL_C_WCHAR`.

**A MEDIR NA FASE DO:** sonda viva do ODBC estendida a `CALL … (?)` com um
parâmetro OUT — exige compilar `phxsqld` + `libphxsql_odbc.so` (o próprio
PENDENCIAS #238 registra que o disco a 5 GB entre quatro frentes barrou isso,
"não esquecido"): `cargo build --release -p phxsql-server -p phxsql-odbc` e a
sonda de ODBC.

A correção dos dois documentos que ainda repetem a premissa morta está
registrada em separado na última seção deste arquivo, e **não** foi feita
agora — ver motivo lá.

---

## GAP 230 — a aba de Usuários chamar as três operações novas de cadastro (dimensionamento)

**Plan (hipótese única):** construir na aba de Usuários o formulário de
criar/alterar/excluir com poder por base e por tabela, ligado às operações
que já existem no protocolo. É construção de tela (papel E); aqui só o
levantamento de tamanho e chaves, como a pesquisa pediu.

**Check (medido, lido no código, sem `cargo`):**
- **As operações já existem no protocolo** (pedido 221,
  `docs/USUARIOS.md` §"Criar, alterar e excluir pelo protocolo"):
  `usuario_criar` (login, senha, nome, email, telefone, supervisor, ativo,
  `bases`), `usuario_alterar` (idempotente por campo), `usuario_excluir`.
  Aplicação a quente; a senha vira hash antes de a estrutura existir;
  recusam tirar de si mesmo o poder de administrar.
- **A tela só lê hoje** (medido em `crates/phxsql-server/ui/index.html`,
  por volta da linha 3953): a aba `usuarios` monta um `PhxGrid` read-only
  (id, nome, login, email, telefone, supervisor, ativo, e `__bases`
  resumido em texto), com o próprio comentário no código dizendo que criar e
  alterar usuário existe no protocolo desde a 0.18 e ainda não nesta tela —
  falta o formulário de poder por base e por tabela. **Zero** chamadas a
  `usuario_criar`/`alterar`/`excluir` na UI.
- **O que o formulário precisa modelar**
  (`docs/USUARIOS.md` §"O cadastro" e §"O direito no nível da tabela"):
  identidade (nome, login, senha, email, telefone, supervisor, ativo) mais
  editor de poder por base — as **10 atividades**
  (`ler, inserir, alterar, excluir, criar, reindexar, diario, verificar,
  administrar, replicar`) por base, com override por tabela dentro de cada
  base (a regra da tabela substitui a da base; base listada vazia nega
  tudo). `excluir_tabela` exige `administrar` + `confirmar` com o nome.
- **Chaves de idioma — contagem medida:** hoje há **21** ocorrências de
  `usuario`/`usuarios` em `idiomas.rs`, mas são colunas e títulos de leitura
  (`col_supervisor`, `col_ativo`, `col_poder_por_base`, `usuarios_sub` etc.).
  Faltam as do formulário, que não existem: os **10 rótulos de atividade**
  (nenhum tem chave `tela.*` própria hoje) mais os controles do formulário
  (novo usuário, editar, salvar, cancelar, excluir usuário, confirmar
  exclusão, adicionar base, adicionar tabela, senha, repetir senha,
  cabeçalho "poder por tabela"). São **~22 chaves novas × 6 idiomas**
  (o PENDENCIAS #230 estima "~20"; a contagem por rótulo de atividade +
  controles dá 20–24, dependendo de quantas atividades já têm rótulo
  reaproveitável de menu — a fábrica de idiomas é a fonte, não este número).

**Choque com pétrea?** Duas a respeitar na construção (papel E):
- **Rótulo se traduz; dado, nunca** — nomes de base/tabela e valores são
  dado, entram por `${…}` interpolado (some do conferidor de idiomas); só os
  rótulos das 10 atividades e dos botões viram `data-txt`.
- **Senha nunca em texto puro** — o formulário manda a senha no
  `usuario_criar` (a cifra do fio protege); a resposta de
  `config`/`usuarios` nunca traz o hash, e a tela não a exibe de volta.
- **Cores da ação:** verde inclui, amarelo altera, rosa/vermelho excluir,
  azul consulta — contorno, não fundo cheio.

**Act (recomendação medida):** é construção de tela (papel E), não pesquisa.
Tamanho: um formulário mestre-detalhe (usuário → bases → tabelas) sobre as 3
operações que já existem, mais ~22 chaves × 6 idiomas na `FABRICA_TELA`, com
a catraca de `data-txt` descendo no mesmo commit (pétrea de QA). A prova é
exercitar no navegador — o CSS global morde componente novo
(`input{width:100%}`, `label{uppercase}`). Nenhuma mudança de protocolo nem
de formato.

---

## Gaps fechados pelo dono — documentar o custo, não reabrir

### GAP 239 — isolamento acima de READ COMMITTED + TLS no transporte

**Custo medido, na mesa** (`docs/propostas/comparativo-19.md` linhas 53–63):
isolamento acima de READ COMMITTED é a Sombra, parada em 05/09 — aceitar
`SET TRANSACTION ISOLATION LEVEL SERIALIZABLE` sem entregar a garantia real
"seria mentira com aparência de capacidade". TLS no transporte bate na pétrea
de zero dependências externas (TLS 1.3 em casa exigiria X.509/ASN.1 + ECDSA
P-256 + gestão de certificado); a cifra do fio (Noise) já protege a porta de
dados; a auditoria da 0.18 propôs exceção só na camada de rede (proxy que
termina TLS). Choque com pétrea nomeado, não reaberto. **Decisão do dono —
não reabrir.**

### GAP 164 — a trava global e o MVCC

**Custo medido** (`docs/CONCORRENCIA.md` §11, `docs/PENDENCIAS.md` #164):
trava por tabela ≈1,00× (não é a tabela que serializa); `RwLock`
2,48×–2,99× de vazão de leitura — mas `RwLock<Instancia>` "compila de
primeira e está errado" (o `Mutex` é ficha de exclusão global; o estado real
está no disco). O custo é o invariante (`!Sync`, separar caminhos que só
leem dos que escrevem), não uma linha. Duas premissas morreram medidas. **A
ordem é decisão do dono — não reabrir.**

### GAP 179 — o teto do MVCC / a Sombra

**Custo medido** (`docs/SOMBRA.md`, 1ª linha; `docs/CONCORRENCIA.md`
§11.2-bis): no padrão `por_lote` — o mundo escolhido pelo dono — a Sombra
não compra desempenho: ~1,00×–1,21× (a premissa de velocidade morreu medida
em 04/09). O que ela compraria é leitura repetível (fecha leitura-não-
repetível e fantasma; não fecha *write skew*, que piora). "Quem a defender
por velocidade está defendendo um número que morreu medido." **Parada por
decisão do dono, sem urgência de formato (a Sombra é em RAM, zero PSCH) — não
reabrir.**

### GAP 207 — transação com quórum de escrita

**Estado medido** (`docs/propostas/quorum-de-escrita.md`,
`docs/PENDENCIAS.md` #207): não está fechado por recusa — a rota foi
**decidida** pelo dono em 07/09: canal aberto, a réplica conecta e fica,
preservando o firewall; campo `cluster.quorum_minimo` já guardado (tela de
cluster fechada, pedido 208). Medido pelo canal quente: piso 0,089 ms,
empurrar evento 0,466 ms, quórum 2-de-3 0,447 ms, 3-de-3 0,498 ms — o "sono"
era 99,9% do número antigo publicado (826–2014 ms). O que falta não é
transporte: é decidir e escrever o que o "ok" significa (recebeu / aplicou /
aplicou+`fsync` — a decisão 207 diz "aplicou e sincronizou"), o prazo
(`cluster.quorum_prazo_ms`, padrão a medir), e a troca de disponibilidade
(irreversível — decisão de produto). **É o próximo passo já do dono, não um
item a reabrir por conta própria.** Armadilha do Cassandra® já registrada: o
`QUORUM` deles é "N processos copiaram para `mmap`", com `fsync` a cada 10 s
— "ok" tem de significar uma coisa só, aqui.

---

## Ordem de ataque

Critério medido pelo papel J: **mais barato × mais visível × sem formato ×
sem dono, primeiro.**

1. **240 — `[NOT] EXISTS` por apelido de fora.** Convergência unânime dos 4,
   aditivo, substrato `existe` já existe; é tradutor + `consultar`. Corrige
   um bug visível — a própria forma-modelo de `docs/SQL.md` §10 hoje mente
   devolvendo `[]`. Código nosso, sem formato, sem dono.
2. **242 — CHECK na instrução.** Aceite automático (os 3 recusam no
   statement), não muda formato, fecha um defeito de produto (a transação
   inteira cai por erro de uma instrução). Ressalva do `Sequence`/`rownum`
   fica nomeada.
3. **238 — OUT no ODBC.** A premissa "precisa de operação nova de protocolo"
   morreu medida — o `CALL`/`saida` já entrega, na mesma forma de
   PostgreSQL/MySQL. Vira trabalho de driver (barato, aditivo) + correção de
   documento. Depende de compilar o driver (registrar o disco entre
   frentes).
4. **229 — auto-number, os itens sem formato:** `Uuid` nasce sozinho (#2),
   `verificar` reconta (#4), `Sequence` numerada no INSERT (#5, com a
   premissa do rollback-em-ordem a medir antes). Baratos, sem dono.
5. **230 — aba de Usuários.** Construção de tela (papel E) sobre operações
   que já existem, mais ~22 chaves × 6 idiomas e a catraca `data-txt`
   descendo. Sem protocolo.
6. **229 — os itens com formato** (`inicio`/`passo` #6, `IDENTITY ALWAYS`
   #7, sequência nomeada #8): vão ao dono/DBA com o custo na mesa — entram
   cedo (nova versão de PSCH que lê o antigo com padrões) ou viram migração.
7. **239 / 164 / 179 / 207 — nada a fazer nesta rodada:** fechados/parados
   por decisão do dono (239/164/179), ou rota já decidida aguardando o dono
   definir o significado do "ok" (207). Custo documentado acima; não
   reabrir.

**Regra que atravessou toda a pesquisa:** onde o número já estava medido, foi
citado com caminho e seção; onde faltava, ficou marcado **"A MEDIR NA FASE
DO"** com o comando exato — nenhum número foi estimado. E a premissa mais
cara desta rodada morreu medida: o gap 238 não era decisão de protocolo.

---

## Correções de documentação para a fase de integração

Esta rodada **não edita** nenhum documento além deste — `docs/PENDENCIAS.md`
está sob outro agente no momento desta pesquisa, e mexer nele agora seria
colisão de escrita. Registra-se aqui, para quando a árvore liberar, a
correção que a evidência do gap 238 exige — com a fonte exata, para que quem
for editar não precise remedir nada:

- **`docs/PENDENCIAS.md` #238** e **`docs/ODBC.md` §2.1.1** afirmam hoje que
  "nenhuma operação devolve um valor além da linha" e classificam o gap como
  decisão de protocolo. **Isso está desatualizado.** A operação
  `chamar_procedimento` (que atende `CALL`), em
  `crates/phxsql-server/src/servidor.rs:15062-15078`, já monta e devolve o
  objeto `saida` para todo parâmetro `OUT`/`INOUT`, lendo o valor pós-execução
  de `ctx.valor_de(&q.nome)`. O que falta é só do lado do driver:
  `crates/phxsql-odbc/src/parametro.rs:276-277` recusa o bind de
  `SQL_PARAM_OUTPUT`/`SQL_PARAM_INPUT_OUTPUT` com `HYC00`, e
  `crates/phxsql-odbc/src/resultado.rs:218-264` só lê `resposta.campo("linhas")`,
  ignorando `saida`.
- **Ação pendente (não feita agora):** trocar, nos dois arquivos, a frase
  "decisão de protocolo" / "nenhuma operação devolve valor além da linha"
  por algo como "o vaso existe (`CALL`/`saida`, `servidor.rs:15062-15078`);
  faltava ligar no driver ODBC — fechado no pedido/commit que implementou o
  item 1–3 do Act do gap 238 acima". Quem fizer essa edição deve citar o
  commit que efetivamente ligou o OUT no driver, não esta pesquisa.
- **Por que registrar em vez de corrigir direto:** a pétrea de documentação
  ("todo número/afirmação visível sai de fonte medida, ou está marcado")
  vale tanto para não inventar quanto para não pisar em edição concorrente —
  uma correção feita hoje, no arquivo que outro agente está reescrevendo,
  arrisca ser sobrescrita ou gerar conflito de merge maior que o benefício de
  corrigir uma frase agora. A correção não é opcional; só a ordem em que
  acontece foi ajustada a esta rodada.

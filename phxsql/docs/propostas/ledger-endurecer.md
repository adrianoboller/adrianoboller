# Endurecer o ledger — desenho pronto para implementar

**12/09/2026. Papéis C (DBA sênior) + B (Engenheiro de desenvolvimento),
integrando o achado da frente J3 (pesquisa das *ledger tables* do SQL Server)
sobre o modo blockchain do PhxSql (`crates/phxsql-store/src/ledger.rs`,
integrado em `540e5cd`).** Fontes: `docs/propostas/dba-bases-2026-09.md §2.1`,
`docs/cognicao/cognicao_o-crc-do-reg-e-o-hash-do-ledger-sao-camadas-diferentes_20260912_1452.md`,
o próprio `ledger.rs` e os testes de `crates/phxsql-store/tests/acrescentar-coluna.rs`.

**Isto é DOCUMENTO.** Nenhuma linha de Rust foi escrita, nada foi compilado,
nada foi commitado — o motor está com a frente do P0 compilando, e o pipeline
é uma compilação por vez. O que segue é o projeto de cada um dos quatro
endurecimentos pedidos, pronto para um engenheiro implementar sem precisar
redescobrir as decisões.

## Como ler este documento

Cada item traz: o defeito medido, o conserto proposto, o que muda (ou não) no
`PSCH`, e o plano de prova real nos dois sentidos — o teste tem de **falhar
com o defeito reposto** e passar com o conserto. Nenhum teste foi rodado nesta
frente porque não há código ainda; o plano é o que o papel B roda quando
implementar, e o papel G confere a catraca quando a suíte chegar.

### Prioridade e impacto, de relance

| # | Item | Prioridade | Toca o `PSCH`? | Toca só lógica? |
|---|---|---|---|---|
| 1 | Evolução de esquema quebra o hash retroativo | **URGENTE — bug atual** | Não (a trava lê o flag do #2) | Sim |
| 2 | Motor recusa `UPDATE`/`DELETE` numa tabela ledger | Endurecimento | **Sim — PSCH v10, 1 byte** | — |
| 3 | Âncora externa via replicação | Endurecimento | Não (arquivo novo por tabela, fora do `PSCH`) | Parcial |
| 4 | Confirmação redobrada no `DROP` | Endurecimento | Não | Sim (protocolo) |

Os itens 1 e 2 nascem **juntos**: o conserto do 1 é ler o mesmo flag que o 2
introduz. Recomendo implementá-los no mesmo commit — ver §6.

---

## 0. O que já existe, para quem não leu o `ledger.rs`

O modo ledger **não é um `TipoDatabase` novo**: é uma convenção de uso de uma
`Table` comum cujo esquema tem três colunas (`hash` Uuid256, `anterior`
Uuid256, `altura` Sequence) mais um índice único `porAltura`. `preparar_bloco`
calcula o hash do bloco **antes** de gravar (SHA-256 do conteúdo canônico —
os valores das colunas de dado, reserializados por uma regra fixa, nunca os
bytes crus do slot); `verificar_cadeia` varre a árvore e confere três provas,
na ordem barata→cara: altura contígua, ligação (`anterior` aponta pro hash de
baixo) e conteúdo (o hash recalculado bate o gravado).

**Hoje não existe NENHUM flag de tabela.** O motor não sabe que uma tabela
está em modo ledger — quem sabe é o código de aplicação, que decide chamar
`preparar_bloco` em vez de montar a linha à mão. É exatamente essa ausência
que os itens 1 e 2 fecham.

A lição da cognição de 12/09/2026 vale para **todos** os planos de prova real
deste documento: o `.reg` confere o CRC-32 da linha **na leitura**
(`reg.rs:1919`) e recusa uma linha corrompida antes mesmo de o ledger a ver.
Uma prova real de adulteração do ledger tem de entrar pela porta que a camada
de baixo **aceita** — ou o teste credita à cadeia uma detecção que é do CRC.

---

## 1. [URGENTE] Evolução de esquema quebra o hash retroativo

### 1.1 O defeito, com o mecanismo exato

`conteudo_canonico` (`ledger.rs:158`) percorre `esquema.colunas()` **na ordem
atual do esquema**, pulando `hash`, `assinatura` e as colunas de sistema, e
empacota cada valor por posição (`linha[i]`). Ele não sabe, e não tem como
saber, qual era o esquema **no momento em que aquele bloco foi gravado**.

`acrescentar_coluna` (`table.rs:737`) reescreve **todos** os slots do `.reg`
quando uma coluna nova entra — a própria suíte de prova do recurso mede isso:
"todos os slots passam pela reescrita" (`acrescentar-coluna.rs:102`, `n ==
200`). Cada linha antiga passa a ter, fisicamente, um valor a mais (o padrão
declarado, ou nulo) na posição nova.

O encontro dos dois é o bug: depois do `ALTER TABLE ADD COLUMN`, **toda** linha
do ledger — inclusive o bloco gênese — tem um campo a mais na entrada do
SHA-256 que não existia quando o hash gravado foi calculado. `verificar_cadeia`
recalcula com o esquema novo e o resultado **nunca** bate o hash gravado,
para **nenhum** bloco. Não é "as linhas antigas deslocam" no sentido de
misturar bytes de campos vizinhos — cada campo continua com tipo e tamanho
autoexplicativos (`§ do módulo`, byte de tipo + tamanho prefixado) e não há
ambiguidade de parsing. O problema é mais simples e mais grave: **o conteúdo
hasheado mudou**, porque um campo que não fazia parte da série de bytes original
passou a fazer. `verificar_cadeia` devolveria `Falhou { altura: 1, prova:
Conteudo }` — falso positivo de adulteração na cadeia inteira, a partir do
gênese, sem que ninguém tenha tocado um bit de propósito.

Confirmado: `alterar_tabela`/`acrescentar-coluna.rs` já existe e já está
testado para tabelas comuns — o caminho que dispara o bug **já está em
produção do motor**, só falta alguém usá-lo numa tabela ledger. Risco atual,
não hipotético.

### 1.2 Duas propostas de conserto

**Opção A — travar `alterar_tabela` para tabela em modo ledger.**
`Table::acrescentar_coluna` recusa de saída, com um erro nomeando a tabela e o
motivo, quando o esquema está em modo ledger (o flag do item 2). Nenhuma
mudança de formato própria: a trava é um `if` que lê um byte que o item 2 já
precisa gravar.

**Opção B — a régua do SQL Server: só coluna nullable, só no fim, ignorada no
hash.** `acrescentar_coluna` continua permitida numa tabela ledger, mas:

- a coluna nova nasce **fora do hash**, para sempre — inclusive nas linhas
  gravadas **depois** do `ALTER`, não só nas antigas. Se ela entrasse no hash
  das linhas novas e ficasse fora do hash das antigas, `conteudo_canonico`
  precisaria saber "esquema na hora daquele bloco", que é exatamente o dado
  que o motor não guarda hoje — guardá-lo seria uma mudança de formato bem
  maior (um esquema versionado por altura).
- exige um **novo byte por coluna** no `PSCH` (`fora_do_hash: bool`), no
  molde exato do grau de dado pessoal da v6 (`docs/FORMATO.md` linha ~410): um
  byte por coluna, no fim do bloco, nascendo `0`/`false` para toda coluna
  gravada antes desta versão — o que preserva o comportamento atual (colunas
  velhas continuam 100% cobertas pelo hash).
- `acrescentar_coluna`, quando a tabela é ledger, **força** `fora_do_hash =
  true` na coluna nova, sem interruptor para religar — um interruptor "quero
  que esta entre no hash" reabriria o próprio bug que a régua fecha.
- `conteudo_canonico` passa a pular, além de `hash`/`assinatura`/sistema,
  toda coluna com `fora_do_hash = true`.

### 1.3 A recomendação: Opção A (travar), e por quê

**Recomendo travar**, contra as pétreas da casa:

- **Zero dependências / zero formato próprio.** A Opção A não pede nada além
  do flag que o item 2 já vai gravar. A Opção B pede um byte por coluna a
  mais, com uma regra de exceção que sobrevive para sempre em cada tabela que
  a usar.
- **"Recusa cedo custa um erro lido; recusa tarde custa um banco modelado
  errado."** A mesma doutrina que fez `ao_excluir` recusar na declaração e não
  na gravação vale aqui: travar recusa a proposta de `ALTER` na hora, antes de
  reescrever um único slot.
- **"Medir a premissa do item antes de implementar o item" — inclusive quando
  o item é nosso.** O recurso ledger nasceu **hoje**, no mesmo commit em que
  este defeito foi achado. Não há uma linha de PENDENCIAS, um cliente, ou uma
  medição pedindo para evoluir o esquema de uma tabela ledger em produção. A
  Opção B compra uma exceção permanente na garantia de auditoria por uma
  necessidade que ninguém mediu ainda.
- **A régua troca uma garantia forte por uma com exceção que precisa ser
  lembrada.** "Todo byte de toda coluna de dado está sob o hash" é uma frase
  que qualquer um pode auditar sem ler o histórico de `ALTER TABLE` daquela
  tabela. "Todo byte, exceto o das colunas que entraram depois de tal data" só
  é auditável por quem sabe procurar a exceção — é o mesmo formato de risco
  que fez a casa decidir "chave declarada nasce conferida": uma garantia que
  depende de alguém **lembrar** da exceção na hora de interpretar a prova não
  é garantia, é convite ao erro de auditoria.
- **Travar não fecha a porta para sempre.** Se um dia aparecer uma demanda
  real e medida de evoluir esquema de ledger, a Opção B pode entrar depois
  como aditiva (mesma disciplina do byte que nasce `false`) sem quebrar nada
  do que existir até lá.
- **Já existe uma saída para quem precisa mesmo assim:** recriar a tabela como
  um ledger novo (gênese nova), arquivando a cadeia antiga sob outro nome. É
  o mesmo cenário que o item 4 (confirmação redobrada no `DROP`) já protege —
  os dois desenhos se encaixam em vez de se contradizerem.

### 1.4 Onde a trava entra, e o irmão que fica de fora

A trava vai em `Table::acrescentar_coluna` (`table.rs:737`), como a **primeira**
checagem da função — antes até da checagem de coluna de sistema, porque é a
recusa mais barata e a que menos trabalho faz antes de desistir:

> se `self.esquema().modo_ledger()`, recusa com `PhxError::Esquema`, nomeando
> a tabela e explicando: evoluir o esquema de uma tabela em modo ledger
> invalidaria o hash de todo bloco já gravado.

**O único outro ponto do motor que muda esquema de uma tabela viva** é
`Table::redeclarar_chaves_estrangeiras` (`table.rs:692`) — e este **não**
precisa da trava: declarar ou redeclarar uma chave estrangeira não toca a
ordem nem o conteúdo das colunas de dado, então não afeta `conteudo_canonico`
em nada. Registrar isto explicitamente evita a dúvida de quem for implementar:
não é esquecimento, é que o caminho irmão não tem o mesmo risco. Se um dia
nascer uma operação nova que mude posição, tipo ou presença de coluna
(`renomear_coluna`, `remover_coluna`, mudança de tipo — nenhuma existe hoje),
ela **tem** de nascer com a mesma trava.

### 1.5 Impacto no formato

**Nenhum, sozinho.** A trava lê o byte do item 2 (§2). Um banco existente sem
esse byte tem `modo_ledger() == false` para toda tabela, então continua
aceitando `ALTER TABLE ADD COLUMN` exatamente como hoje — nada muda até
alguém ligar o flag numa tabela nova ou existente.

### 1.6 Plano de prova real (dois sentidos)

1. **`acrescentar_coluna_recusa_em_tabela_ledger`.** Cria uma tabela com
   `modo_ledger = true`, grava uma cadeia de N blocos (usando `preparar_bloco`,
   como o `cadeia()` de `ledger.rs` já faz), chama `acrescentar_coluna`, espera
   `Err` nomeando a tabela. **Sem a trava** (defeito reposto), a chamada teria
   sucesso — o teste falha.
2. **`acrescentar_coluna_continua_livre_fora_do_modo_ledger`** — o irmão que
   prova o comportamento velho: a mesma chamada, na mesma tabela mas sem o
   flag, continua tendo sucesso exatamente como toda a suíte de
   `acrescentar-coluna.rs` já prova hoje. É a doutrina "guarda nova entra
   pedida, não imposta" — aplicada ao motor, não à réplica.
3. **Regressão dirigida, para documentar o número do bug antes do conserto**
   (mesma disciplina do "hipótese que morre medida"): antes de aplicar a
   trava, gravar uma cadeia de 5 blocos, chamar `acrescentar_coluna` sem
   guarda nenhuma, e registrar o resultado de `verificar_cadeia` —
   `Falhou { altura: 1, prova: Conteudo { .. } }`, **zero** blocos sobrevivendo
   à verificação, sem nenhum ataque. Este número (não estimado, medido nesta
   implementação) é o que prova que o defeito é um **falso positivo de
   adulteração**, não uma detecção real, e vale registrar no `PENDENCIAS.md`
   junto do fechamento.
4. Rodar a suíte inteira de `acrescentar-coluna.rs` sem modificação — tem de
   continuar 100% verde, porque nenhum dos testes de lá usa esquema em modo
   ledger. Prova que a trava não vazou para tabela comum.

---

## 2. Modo append-only recusa `UPDATE`/`DELETE` no motor

### 2.1 O defeito, com o número que já prova o risco

Nada em `Table::atualizar` (`table.rs:3040`), `Table::excluir_de_vez`
(`table.rs:3242`) ou `Table::excluir_suave` (`table.rs:3310`) sabe que a
tabela em que opera está em modo ledger. A própria suíte de prova do recurso
**já explora isso de propósito**: os testes
`conteudo_adulterado_e_pego_na_altura_certa` e
`conteudo_e_hash_juntos_quebram_a_ligacao_no_seguinte` (`ledger.rs:534` e
`:570`) chamam `t.atualizar(rid3, &linha)` direto, para simular um atacante —
o que é a prova, com número (dois testes hoje, e passando), de que **qualquer
código com uma referência a `Table`** — uma rotina de manutenção, um bug de
aplicação, um operador de console — pode reescrever um bloco sem passar por
`preparar_bloco`, e só uma verificação **a posteriori** (`verificar_cadeia`, se
alguém rodar) descobre.

### 2.2 O conserto: um flag de tabela, `modo_ledger`

**Nome escolhido:** `modo_ledger`, não `anexar_apenas` como o pedido sugeria.
O motivo é que o mesmo byte serve às duas obrigações que uma tabela-ledger
tem — nunca muda de **forma** (item 1) e nunca reescreve **conteúdo** (este
item) — e `anexar_apenas` descreve só a segunda metade. Um nome que cobre as
duas evita a tentação de alguém introduzir dois bytes onde um resolve.

**Onde mora:** um campo novo em `Schema` (`crates/phxsql-core/src/schema.rs`),
`modo_ledger: bool`, no **exato** molde do `motivo_obrigatorio` que já existe
ali (`schema.rs:543`, builder `com_motivo_obrigatorio` em `:834`, acessor em
`:839`, escrita em `:1310`, leitura em `:1480`) — o precedente mais próximo
que a casa já tem de "um byte de comportamento por tabela inteira".

**Validação na declaração** (fail-fast, mesma doutrina de "a recusa acontece
na declaração"): `Schema::com_modo_ledger(true)` devolve `Result<Schema>` e
confere, na hora:

- existe uma coluna `hash` do tipo `Uuid256` (`ledger::COL_HASH`);
- existe uma coluna `anterior` do tipo `Uuid256` (`ledger::COL_ANTERIOR`);
- existe uma coluna `altura` do tipo `Sequence` (`ledger::COL_ALTURA`);
- existe um índice **único**, chamado exatamente `porAltura`
  (`ledger::IDX_POR_ALTURA`), sobre a coluna `altura` —
  porque `topo_da_cadeia`/`verificar_cadeia` procuram esse índice **pelo
  nome**, e um índice com outro nome os deixaria cegos sem erro nenhum.

Faltando qualquer um, `Err(PhxError::Esquema(..))` nomeando o que falta — o
mesmo estilo de mensagem que `posicoes()` (`ledger.rs:184`) já usa.

**Serialização — `PSCH` sobe para v10.** Um byte no **fim** do bloco, depois
do bloco de expressões da v9 — a mesma convenção que a v4
(`motivo_obrigatorio`), a v6 (grau de dado pessoal) e a v8 (índices de texto)
já seguem: quem lê uma versão anterior simplesmente **para antes** deste byte.
Arquivo v9 ou anterior: `modo_ledger` nasce `false`. **Nenhum banco existente
muda de comportamento** — a mesma garantia que o `verificar` da chave
estrangeira (v7) já cumpre, e pelo mesmo motivo: o byte volta com o que foi
gravado nele, e tabela que nunca ligou o flag continua exatamente como está.

**Imposição na gravação, não só na declaração** — a mesma regra primordial
que faz `ao_excluir` recusar em `excluir` e não só no `CREATE TABLE`: a
primeira linha de `Table::atualizar`, `Table::excluir_de_vez` e
`Table::excluir_suave` passa a conferir `self.esquema().modo_ledger()` e
recusar com `PhxError::Integridade`, nomeando a tabela e o rowid que se tentou
tocar, com a frase "tabela em modo ledger: só aceita inserir bloco novo".

**Réplica — defesa em profundidade, não confiança cega.** A origem, tendo o
guard acima, nunca deveria **produzir** um evento de update/delete para uma
tabela ledger. Mas `atualizar_replicado` (`table.rs:3663`),
`excluir_de_vez_replicado` (`table.rs:3646`) e o `aplicar_evento`
(`table.rs:3616`) que os invoca levam **o mesmo guard**, pela mesma doutrina
que já faz a réplica conferir o rowid do evento em vez de aplicar de olhos
fechados: se um evento chegar mesmo assim (bug, ou origem comprometida), a
réplica recusa e **para**, com o mesmo comportamento de "já divergiu, não
adianta seguir" que `aplicar_evento` já tem para rowid errado.

**`inserir`/`inserir_replicado` continuam livres** — é o único caminho
legítimo de um bloco novo. Uma ressalva que fica **fora do escopo deste
item, registrada para o dono decidir depois**: nada aqui confere, dentro do
próprio `inserir`, que quem chamou passou por `preparar_bloco` de verdade
(hash batendo o conteúdo, altura e ligação corretas) — isso pediria mover
parte de `verificar_cadeia` para o caminho quente do `inserir`, com custo por
linha inserida. Não desenho essa conferência aqui porque não foi pedida e
porque tem custo medível que precisa de número antes de entrar — mesma
doutrina de "medir a premissa do item antes de implementar o item".

### 2.3 Uma interação que este desenho precisa fechar: cascata do `ao_alterar`

A pétrea da casa faz `ao_alterar` nascer **cascata**: alterar um pai propaga
um `atualizar` para cada filha que aponta para ele, como parte da **mesma**
operação atômica. Se uma tabela em modo ledger puder ser **filha** de uma
chave estrangeira com cascata, alterar o pai tentaria empurrar um `atualizar`
para a filha ledger — e o guard deste item recusaria, quebrando a alteração
do pai inteira (a cascata não é opcional quando `ao_alterar` é cascata).

**O conserto fecha isso na declaração da chave, não na gravação:** ao
declarar uma `ForeignKey` cuja tabela **filha** está em modo ledger, recusar —
"tabela ledger não aceita ser filha de chave estrangeira: nada pode
sobrescrever uma linha do ledger, nem em cascata". É a mesma doutrina de
"ao_excluir só aceita restringir": fechar o buraco onde a declaração acontece,
não descobrir a contradição no dia em que alguém alterar o pai. Isto não
estava no pedido original — é o "irmão" que a leitura cuidadosa do item 2
achou, e que ficaria esquecido se este item entrasse sem essa checagem.

### 2.4 Consequência nos testes existentes do `ledger.rs`, e por que é bom

Os dois testes de adulteração citados em §2.1 **precisam mudar de vetor de
ataque** assim que este item entra: se a tabela de teste (`esquema_blocos()`)
ganhar `modo_ledger = true`, `t.atualizar(rid3, &linha)` passa a devolver
`Err` — não porque o conserto está errado, mas porque o próprio `setup` do
teste usava um caminho que este item fecha de propósito.

O vetor de ataque correto, pela lição da cognição de 12/09: com update/delete
fechados **no motor**, a única porta que resta para simular um atacante é
escrever bytes **direto** no `.reg`, do jeito que o teste do espelho `.bkp`
já faz (`estragado[alvo] ^= 0xFF`, em `acrescentar-coluna.rs:622`) — só que
**recalculando o CRC-32 da linha** depois de mudar o conteúdo, porque um CRC
inválido seria pego na leitura (`reg.rs:1919`) antes de chegar ao ledger, e o
teste voltaria a medir a camada de baixo. É mais trabalhoso de escrever — e é
esse o ponto: um atacante real com acesso só ao arquivo (não ao processo do
servidor) enfrenta exatamente esse trabalho.

**Recomendo não descartar a cobertura existente — acrescentar a que falta**
(a mesma doutrina de "o caminho irmão fica"): os dois testes atuais continuam
válidos como prova de que `verificar_cadeia` pega adulteração **quando não há
o flag ligado** (o caso de quem usa o padrão ledger sem declarar o modo
formal — que continua possível, porque `modo_ledger` é opt-in). Acrescentam-se
dois testes novos, com `modo_ledger = true` e o ataque por bytes crus + CRC
recalculado, que são os que representam a ameaça **depois** deste item.

**Plano de prova real:**

1. `atualizar_recusa_em_tabela_ledger`, `excluir_suave_recusa_em_tabela_ledger`,
   `excluir_de_vez_recusa_em_tabela_ledger`: dada uma tabela `modo_ledger =
   true` com uma cadeia de N blocos, cada operação num rowid existente devolve
   `Err` nomeando a tabela. Sem a trava, teriam sucesso — falham nos dois
   sentidos.
2. `inserir_continua_livre_em_tabela_ledger` — o irmão que prova que só
   update/delete fecham.
3. `ledger_com_bytes_crus_e_crc_recalculado_e_pego_por_verificar_cadeia`
   (substitui, para o caso `modo_ledger = true`, os dois testes que hoje usam
   `t.atualizar`): corrompe o conteúdo de um bloco escrevendo direto no
   arquivo, recalcula o CRC-32 da linha pela mesma função que o motor usa, e
   confirma que `t.ler` **aceita** a linha (CRC bate) mas `verificar_cadeia`
   pega a adulteração — a prova de que a camada nova (o hash do ledger) ainda
   funciona quando a camada de baixo (o CRC) não ajuda mais.
4. `foreign_key_recusa_filha_em_modo_ledger`: declarar uma FK cascata para uma
   tabela ledger devolve `Err` na declaração, não silenciosamente quebra no
   primeiro `ao_alterar` do pai.
5. `replica_recusa_evento_de_update_em_tabela_ledger`: aplicar (via
   `aplicar_evento`) um evento sintético de update contra uma réplica cuja
   tabela é `modo_ledger`, e confirmar que ela recusa e para — a defesa em
   profundidade de §2.2.

---

## 3. Âncora externa via replicação

### 3.1 O limite que nenhuma prova local resolve

`verificar_cadeia` só confere o arquivo **contra si mesmo**. Um atacante com
acesso de escrita ao `.reg`/`.ndx` de uma tabela ledger pode reescrever
qualquer bloco e recalcular hash, `anterior` e altura de **todos** os blocos
seguintes — uma cadeia inteiramente nova, internamente consistente, que passa
100% em `verificar_cadeia`. É o mesmo motivo estrutural pelo qual uma
blockchain de verdade depende de **muitos nós independentes**, nunca de um nó
conferindo a si mesmo.

### 3.2 O desenho: cada réplica testemunha o que recebeu pelo fio

A ideia central: o valor testemunhado tem de ser gravado **no momento em que
o bloco chega pelo fio**, num lugar **separado** do `.reg` da própria tabela
— senão o mesmo ataque que reescreve o `.reg` reescreveria o testemunho
também, e a defesa não teria comprado nada.

**Arquivo novo por tabela ledger: `<tabela>.ltst`** (ledger-testemunha), ao
lado de `.reg`/`.ndx`/etc. Formato (a documentar em `docs/FORMATO.md` quando
implementado, como novo arquivo por tabela — não como versão nova do `PSCH`):

| Campo | Tam | O que é |
|---|---:|---|
| magia | 4 | `"LTST"` |
| versão | 1 | nasce `1` |
| altura testemunhada | 8 | `u64` BE |
| hash testemunhado | 32 | `Uuid256` |
| quando recebido | 8 | `DateTime` (i64 BE, epoch em ms — mesmo tipo do resto da casa) |
| CRC-32 do registro | 4 | confere as 49 primeiras contra corrupção |

57 bytes fixos, **reescrito por cima** a cada bloco novo (não é um log — só
importa o último valor testemunhado), gravado com o mesmo padrão `*.novo` +
troca atômica que a paginação já usa (`acrescentar-coluna.rs` prova esse
padrão nos testes de queda no meio da reescrita) — para nunca ficar pela
metade numa queda de energia.

**Quem escreve, e quando:**

- **Localmente**, no nó que recebeu um `inserir` legítimo de um bloco nascido
  de `preparar_bloco` — o próprio nó de origem também testemunha a si mesmo.
  Defesa fraca sozinha (é o mesmo disco), mas gratuita e é o primeiro nível de
  autoconferência.
- **Em cada réplica**, dentro de `aplicar_evento`/`aplicar_evento_interno`
  (`table.rs:3616`), depois que um evento de `INSERT` for aceito para uma
  tabela `modo_ledger`: lê altura e hash da linha recém-aplicada e grava no
  `.ltst`, com uma regra de **monotonicidade** — recusa e **alarma** (não
  silencia) se a altura recebida for menor que a já testemunhada, porque isso
  só acontece legitimamente se a réplica foi restaurada de um backup velho.

**Comando de protocolo novo, só-leitura**, no molde do `"posicao"` que já
existe (`servidor.rs:2872`): `"ledger_testemunha"` devolve, para uma tabela,
**os dois valores lado a lado**: `{altura_testemunhada, hash_testemunhado,
quando}` lido do `.ltst`, e `{altura_atual, hash_atual}` recomputado **na
hora** via `topo_da_cadeia()` (`ledger.rs:219`, já existe) sobre o `.reg`
vivo. É a **divergência entre os dois** que denuncia adulteração local — sem
precisar de nenhum outro nó.

**Ferramenta comparadora** (papel J/QA, fora do motor — cabe em
`phxsql/bancada/` ou junto do `dossie/`, zero dependência, reaproveitando o
mesmo cliente texto-por-linha que `replica.rs` já fala): conecta nos N
servidores configurados, pede `ledger_testemunha` de cada um para cada tabela
ledger, e relata dois níveis:

1. **Autoconsistência por nó** — `altura_testemunhada == altura_atual` e
   `hash_testemunhado == hash_atual`? Divergência aqui prova adulteração
   **naquele nó**, sem comparar com ninguém.
2. **Consistência entre nós** — todas as testemunhas concordam na mesma
   altura e hash? Divergência aqui, com (1) OK em todos, indica um nó inteiro
   substituído por um estado antigo (restauração de backup, ou troca do
   arquivo inteiro).

**Por que é barato e sem nuvem:** reaproveita a conexão de replicação que já
existe (4 servidores já rodando), nenhum protocolo de rede novo, nenhuma
dependência externa — só um comando JSON de leitura a mais e um arquivo de 57
bytes por tabela ledger.

**Limite, escrito e não escondido** (a mesma disciplina que já fez a casa
parar de repetir "ACID compliant" sem ressalva): se um atacante comprometer
**todos** os N nós ao mesmo tempo — origem e todas as réplicas — e reescrever
`.reg` **e** `.ltst` de cada um de forma consistente entre si, a testemunha
não pega nada. É "aumentar o custo do ataque", não "torná-lo impossível" —
qualquer esquema de N testemunhas independentes tem esse mesmo limite.

### 3.3 Convergência com o item 4: o `.ltst` como o próprio vestígio do `DROP`

Uma observação que une este item ao item 4: se `<tabela>.ltst` **não** entrar
na lista de extensões que `Catalogo::excluir_tabela` apaga (hoje
`EXTENSOES_TODAS`, `catalogo.rs`), ele **sobrevive** ao `DROP TABLE` como a
prova física de que aquela cadeia existiu até a altura X — sem precisar
inventar uma tabela de sistema nova para o "vestígio" que o item 4 pede.
Detalhe que fecha o caso do nome reaproveitado (a suíte já prova que um nome
apagado fica livre para a próxima tabela,
`excluir_tabela_deixa_o_nome_livre_para_a_proxima`): no `DROP`, em vez de
apagar o `.ltst`, **renomeá-lo** para `<tabela>.ltst.apagado` (ou com um
carimbo de data), para que ele não seja confundido com o testemunho **vivo**
de uma tabela nova que reuse o nome depois. Ver §4.2 para o resto do desenho
do `DROP`.

### 3.4 Impacto no formato

Não toca o `PSCH`. Introduz um arquivo novo por tabela, que só passa a existir
quando a tabela liga `modo_ledger` e recebe o primeiro bloco por replicação —
uma tabela existente sem o flag nunca ganha esse arquivo, e por isso é
compatível por definição: ele simplesmente não existe até alguém optar por
modo ledger. Entra em `docs/FORMATO.md` como arquivo novo por tabela (ao lado
de `.trash`/`.lgpd`/`.pag` já documentados) quando isto for implementado.

### 3.5 Plano de prova real

1. `replica_testemunha_o_topo_ao_aplicar_bloco`: a réplica aplica 5 eventos de
   um ledger; o `.ltst` acompanha altura 5 / hash do bloco 5. Sem o hook
   (defeito reposto), o arquivo não muda ou não existe — falha.
2. `testemunha_recusa_andar_para_tras`: testemunha a altura 5, tenta gravar
   uma altura 3 sintética — recusa e mantém 5, e o alarme é registrado (não
   silenciado).
3. `ferramenta_acha_divergencia_de_autoconsistencia`: cenário sintético — o
   `.ltst` diz altura 5/hash H5, mas o `.reg` foi trocado por fora (o mesmo
   ataque dos itens 1/2, arquivo substituído) de forma que `topo_da_cadeia()`
   devolva hash H5' ≠ H5 na mesma altura. A ferramenta tem de reportar
   **este** nó como suspeito.
4. `ferramenta_acha_divergencia_entre_nos`: réplicas sintéticas (reaproveitar
   a infraestrutura que `replicacao.rs`/`sonda-da-replicacao.rs` já montam,
   sem precisar de 4 processos reais) — 3 concordam, 1 diverge — a ferramenta
   reporta a que diverge, nunca um veredito "tudo bem" por maioria silenciosa.
5. **O que depende do sistema operacional** — 4 servidores TCP de verdade
   respondendo ao comando novo — entra pelo papel F, contra o socket real, não
   por teste unitário simulando rede. É a mesma lição do `BULKINSERT`: um
   teste que finge a rede é pior que um teste que falta.

---

## 4. Proteção do `DROP` de tabela ledger

### 4.1 O que já existe

- `Catalogo::excluir_tabela` (`catalogo.rs:720`) **já** recusa apagar um pai
  com filhos por chave estrangeira, com a frase "nunca se apaga o pai que tem
  filhos" — é o precedente exato para este item.
- `Servidor::op_excluir_tabela` (`servidor.rs:14395`) **já** exige repetir o
  nome da tabela no campo `confirmar` (`servidor.rs:14398`) — uma primeira
  camada de confirmação, pensada para o caso geral ("um nome errado aqui
  perde tudo").

Para uma tabela ledger, as duas não bastam: (a) a checagem de FK não enxerga
a cadeia — uma tabela ledger pode não ter nenhuma FK apontando para ela; (b)
confirmar o **nome** não confirma que quem pediu **sabe quantos blocos** está
destruindo — um operador pode digitar o nome certo sem saber que a tabela tem
dezenas de milhares de blocos de auditoria acumulados.

### 4.2 O conserto: confirmação redobrada por altura, mais o vestígio do §3.3

Quando a tabela é `modo_ledger = true`, `op_excluir_tabela` passa a exigir um
**segundo** campo, `confirmar_altura` (inteiro), que tem de bater com a
altura **atual** do topo da cadeia (`topo_da_cadeia`, lida na hora do pedido —
não a que o cliente lembra de ter visto antes). Faltando o campo, ou o número
não batendo — inclusive porque alguém gravou mais um bloco entre a consulta e
o `DROP` —, recusa nomeando a altura certa, no mesmo estilo do erro que
`confirmar` de nome já usa hoje.

**Por que altura, e não um booleano "tenho certeza":** um booleano prova só
que alguém clicou; a altura prova que alguém **consultou a cadeia** antes de
destruí-la, e amarra o pedido a um instante específico — o mesmo raciocínio
por trás do campo `versao` do controle de concorrência otimista, onde quem
manda o número certo prova que olhou o estado agora, não de memória.

**Não quebra cliente antigo.** Nenhum cliente hoje apaga tabela-ledger,
porque `modo_ledger` ainda não existe — este é um recurso novo protegendo um
recurso novo, e não uma trava nova sobre um comportamento velho. A pétrea
"guarda nova entra pedida, não imposta" protege quem já fazia algo; aqui não
há ninguém fazendo nada porque a coisa protegida ainda não existe.

**Vestígio:** conforme §3.3, o `.ltst` sobrevive ao `DROP` (renomeado, para
liberar o nome para reuso) em vez de virar uma tabela de sistema nova.
Considerei e **rejeito** a alternativa de uma tabela de sistema
`_ledgers_apagados`: ela pede um schema próprio, uma decisão sobre se ELA
também precisa virar ledger (recursão que este documento não tem por que
abrir) e uma tabela que já pode ser feita, mais barato, do arquivo que o
item 3 já cria.

### 4.3 Impacto no formato

Nenhum no `PSCH`. É lógica de protocolo (um campo JSON a mais em
`op_excluir_tabela`) mais a regra de renomear o `.ltst` em vez de apagá-lo.

### 4.4 Plano de prova real

1. `excluir_tabela_ledger_exige_confirmar_altura`: `DROP` com `confirmar`
   certo mas **sem** `confirmar_altura` recusa, nomeando a altura certa; sem a
   trava (defeito reposto), apagaria os arquivos — falha nos dois sentidos.
2. `excluir_tabela_ledger_recusa_altura_desatualizada`: consulta a altura (5),
   uma segunda conexão insere mais um bloco (6), o `DROP` com
   `confirmar_altura = 5` é recusado.
3. `excluir_tabela_ledger_com_confirmacao_certa_apaga` — o caminho feliz
   continua funcionando: o irmão que prova que a trava não trava tudo.
4. `excluir_tabela_comum_continua_so_com_confirmar_nome` — tabela sem
   `modo_ledger` não pede o campo novo. O teste do comportamento **velho**,
   doutrina de "guarda nova entra pedida, não imposta".
5. `excluir_tabela_ledger_deixa_o_ltst_como_vestigio`: depois do `DROP`, o
   `<tabela>.ltst.apagado` existe com a altura e o hash que a cadeia tinha
   antes de sumir, e o nome `<tabela>.ltst` está livre para uma tabela nova.

---

## 5. Resumo: o que toca formato, o que é só lógica

| Item | Formato em disco | Lógica de motor | Lógica de protocolo |
|---|---|---|---|
| 1 — travar `alterar_tabela` | — (lê o byte do #2) | `Table::acrescentar_coluna` | — |
| 2 — flag `modo_ledger` | **`PSCH` v10, 1 byte no fim do bloco** | `Schema`, `atualizar`, `excluir_de_vez`, `excluir_suave`, `aplicar_evento`, declaração de FK | — |
| 3 — testemunha por replicação | **arquivo novo `.ltst` por tabela** (fora do `PSCH`) | `aplicar_evento_interno` | comando `"ledger_testemunha"` |
| 4 — `DROP` redobrado | — (reaproveita o `.ltst` do #3) | — | `op_excluir_tabela`: campo `confirmar_altura` |

**Um único byte novo no `PSCH`** (v9 → v10) resolve os itens 1 e 2 juntos. Os
itens 3 e 4 não tocam o `PSCH` — um introduz um arquivo auxiliar novo, o outro
é protocolo puro.

---

## 6. Ordem de implementação sugerida

1. **Itens 1 + 2, no mesmo commit** — fecham o risco atual (falso positivo de
   adulteração) e são a única mudança de formato deste documento.
2. **Item 4** pode entrar independente dos outros três — é só protocolo, e o
   ganho (confirmação redobrada) vale mesmo antes do item 3 existir (sem o
   `.ltst`, o vestígio fica para depois; a confirmação por altura não depende
   dele).
3. **Item 3** é o maior dos quatro e o único hardening que precisa de um
   arquivo novo e um comando novo — entra depois, com número medido do custo
   de escrever o `.ltst` a cada bloco replicado (deveria ser desprezível: 57
   bytes, uma vez por bloco, não por linha comum).

---

## 7. Dispensa registrada dos papéis não convocados

Cláusula pétrea dos dez papéis: nenhum fica sem dono quando o trabalho toca o
domínio dele, e dispensa tem de ser registrada, não silenciosa.

- **D — Zelador do ambiente.** Não convocado: trabalho de documento, sem
  arquivo temporário além deste `.md`, sem processo para limpar.
- **E — Designer gráfico.** Não convocado: nenhuma tela nova — os quatro itens
  são motor, formato e protocolo.
- **F — Usuários de teste / prova real.** Convocado só para o **plano** (cada
  item traz o roteiro de prova nos dois sentidos); a **execução** — rodar
  `cargo test`, exercitar o socket de verdade para o item 3 — é do papel B
  quando implementar. Nenhum teste foi rodado nesta frente porque não há
  código ainda; registro isto para não sumir do relatório.
- **G — QA (catracas).** Não convocado ainda: não há catraca nova para
  registrar até a suíte dos quatro itens existir. Quando existir, cabe a ele
  provar os testes periodicamente contra o defeito que os motivou (§1.6, §2.4,
  §3.5, §4.4).
- **H — Documentação.** Parcialmente convocado: este documento **é** o
  entregável de documentação desta frente. Quando implementado, faltam ainda
  a atualização de `docs/FORMATO.md` (`PSCH` v10 + o arquivo `.ltst`) e da
  tabela de status — nomeado aqui como pendência de quem implementar, não
  feito nesta rodada.
- **I — Versionador e backup.** Não convocado para commit: nada aqui é
  commitado por este papel — o orquestrador integra, por instrução explícita
  desta tarefa.
- **J — Pesquisador.** Já convocado **antes** desta frente — a leitura das
  *ledger tables* do SQL Server está registrada em
  `docs/propostas/dba-bases-2026-09.md §2.1` e não foi re-rodada aqui, só
  consumida. A régua dos "três motores convergindo" não se aplica a este
  documento: nem PostgreSQL, nem MariaDB, nem MySQL têm ledger tables — é
  recurso exclusivo do SQL Server nesta comparação, então não há convergência
  a invocar, só uma fonte única, medida contra o nosso gargalo (a mesma
  doutrina de "receita de fora se mede antes de virar plano").

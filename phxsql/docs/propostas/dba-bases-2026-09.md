# O que entra no PhxSql — análise do DBA sobre a pesquisa das bases (09/2026)

**12/09/2026. Papel C (DBA sênior), integrando a pesquisa do papel J.** O dono
pediu: baixar/estudar os fontes e manuais das bases (Berkeley/PostgreSQL,
Oracle, Microsoft SQL Server, MySQL, MariaDB, SQLite, Rusqlite, Cassandra) +
SAP HANA, e **elencar o que pode ser adicionado ao PhxSql**. Nove frentes de
pesquisa (papel J) voltaram; esta é a peneira contra o nosso crivo: **zero
dependências externas, integridade primordial, ordem de digitação sagrada, e
número medido antes de plano**.

**Método honesto:** não houve *full-clone* (o disco em 6 GiB com o build vivo
não caberia — a mesma disciplina do `CASSANDRA.md`, «lido no fonte no commit
tal»). Cada frente leu manual + arquivos-fonte em alvo, e cada achado abaixo traz
a frente de origem. **Adoção é decisão do dono/DBA; esta é a lista ordenada por
custo e risco, não uma autorização.**

---

## Tier 1 — ENTRA: baixo risco, sem pétrea contra, sem formato novo

### 1.1 [O ACHADO DA RODADA] Fechar o P0 e o ACID-C juntos — duas fontes independentes apontam o mesmo conserto

O P0 (atomicidade do commit pai+filho: `conferir_fks` lê a mãe do disco e não vê
o pai empilhado) e o ACID-C parcial (a cascata do `ao_alterar` escreve fora do
conjunto de escrita da transação) são **o mesmo problema visto de dois ângulos**,
e SQLite e InnoDB, lidos no fonte, convergem na direção do conserto:

- **SQLite (E2), o *super-journal*:** para commit multi-arquivo (`ATTACH`), o
  SQLite enumera **todos** os arquivos que a transação vai tocar num journal
  mestre, sincroniza, e só então escreve — a recuperação decide tudo-ou-nada
  pela existência da marca. **A nossa marca `.tx` já é meio super-journal (redo,
  não undo)** — o que falta é a cascata **entrar na lista antes do `fsync`**, em
  vez de ser descoberta durante a passada. E nós já conferimos a árvore de
  cascata inteira **antes** de gravar (pedido 169/173), então a informação para
  fechar a lista já existe no momento certo.
- **InnoDB (E1), o read-your-own-writes de graça:** a conferência de FK do
  InnoDB nunca teve esse buraco porque nunca houve **dois objetos** para a mesma
  tabela na mesma transação — há uma cópia física só, e o pai empilhado **é** a
  página. O nosso `conferir_fks` (`table.rs:1205`) abre um **segundo handle** da
  mãe, desconectado da `Sobreposicao` que a transação já mantém. O conserto que
  converge com o mecanismo real não é «olhar mais uma estrutura», é **a transação
  reusar o MESMO handle (a MESMA `Sobreposicao`) para mãe e filha**.

**Veredito:** candidato forte, e o de maior retorno da rodada inteira. Fecha P0 e
ACID-C **sem MVCC, sem Sombra, sem mudar o modelo de escrita** — é lógica de
visibilidade em RAM + escopo da marca `.tx`, não formato em disco. **Ressalva
(pétrea «guarda nova entra pedida, não imposta»):** a consulta à `Sobreposicao`
da mãe tem de nascer atrás do portão barato «há sobreposição pendente?», ou
reintroduz o custo-quando-desligado do Profiler. **É o gate dos motores V e H**
(a recomendação de fechar o P0 antes de erguer Vetorial/Colmeia segue de pé).

### 1.2 Materialized views + reescrita de consulta (J2 Oracle)

Lógica pura de reescrita de plano, **zero-dep**. Adicionar/remover uma MV como um
índice, sem invalidar SQL existente. Ganho real em relatórios/agregações
repetidas. **Antes de qualquer código:** medir se `phxsql-sql` já cacheia plano —
se não cacheia, o item nasce junto com o cache, não depois.

### 1.3 Otimização adaptativa / histogramas por coluna (J2 Oracle, J3 SQL Server)

Algoritmo, zero-dep. Histograma por coluna para cardinalidade não-uniforme, e um
«plano B» trocado em runtime quando a estimativa erra (o *optimizer statistics
collector* do Oracle, o *Automatic Plan Correction* do SQL Server). Candidato de
médio prazo, se o otimizador do PhxSql passar a depender de estatística.

### 1.4 Visão JSON sobre tabela relacional (J2 Oracle, *duality views*)

Serialização é código nosso; a camada REST/OpenAPI já existe. Expor a mesma linha
como documento JSON e como tabela SQL é extensão natural do protocolo, não do
formato em disco. **Nota de convergência:** a concorrência otimista por ETag das
duality views **é exatamente o nosso campo `versao`** — não é candidato a copiar,
é confirmação de que a escolha já feita está alinhada com motor maduro.

---

## Tier 2 — PEDE DECISÃO DO DONO: formato novo, ou reabrir uma pétrea

### 2.1 [URGENTE — risco atual] Endurecer o ledger, a partir das *ledger tables* do SQL Server (J3)

O modo blockchain (E1–E3, `ledger.rs`, integrado em `540e5cd`) tem quatro lacunas
que o SQL Server já resolveu e que a leitura do desenho deles expôs:

- **[URGENTE] Evolução de esquema quebra o hash retroativo.** `conteudo_canonico`
  usa o esquema **atual** para recalcular o hash de **qualquer** linha, inclusive
  antigas. Se alguém inserir uma coluna **no meio** do esquema de uma tabela em
  modo ledger, o layout binário de todas as linhas gravadas antes desloca, e
  `verificar_cadeia` acusa **falso positivo de adulteração** (ou pior, mascara
  uma real). Confirmado: `alterar_tabela`/`acrescentar-coluna.rs` já existe — o
  risco é **atual, não hipotético**. O SQL Server proíbe explicitamente (só
  coluna *nullable*, só no fim, ignorada no hash). → vira item de `PENDENCIAS.md`
  e trava `alterar_tabela` para tabela em modo ledger.
- **Modo append-only deveria recusar `UPDATE`/`DELETE` no MOTOR**, não só detectar
  depois — no molde de `ao_excluir` só aceitar `restringir` («recusa na gravação,
  não só na declaração»). Hoje nada em `Table::atualizar` sabe que a tabela está
  em modo ledger — é o próprio caminho que o teste de adulteração explora.
- **A cadeia não tem âncora fora de si.** Um atacante com o arquivo pode reescrever
  a tabela **e** recalcular a cadeia consistente. A **replicação já existente (4
  servidores)** é uma testemunha pronta: cada réplica registra o último
  `(altura, hash)` que recebeu pelo fio — digest externo barato, sem nuvem, sem
  crate (o meio local, como a pétrea de TLS já fez).
- **`DROP` de tabela ledger apaga a cadeia** sem vestígio. No espírito da regra
  primordial, uma tabela-ledger com blocos é «um pai que sempre tem filhos» —
  exigir confirmação redobrada.

**Veredito:** o primeiro é conserto de bug (entra logo, atrás de uma decisão de
formato do flag `anexar_apenas`); os outros três são endurecimento, decisão do
dono sobre escopo.

### 2.2 Coluna de data/hora de sistema por linha — `rowts` (J1 Berkeley + pedido #189 do dono)

O `tmin`/`tmax` do POSTGRES de 1986 (lido no «Design of Postgres» na íntegra) é
**exatamente** a coluna que o dono já pediu (pai carimbado com instante
estritamente anterior ao do filho). Precedente de 40 anos. **Achado que muda a
implementação:** a ordenação causal estrita **entre nós** (replicação/cluster)
não se resolve com relógio de parede — os sistemas maduros usam **relógio lógico
híbrido (HLC)** ou *causality token* (CockroachDB/YugabyteDB). Mudança de formato
(PSCH novo) → decisão do dono, «entra cedo» (papel C), o formato se decide antes
de gravar.

### 2.3 Web Push (J5) — o único manager de notificação parcialmente nosso

Dos quatro canais que o dono pediu (Push, SMS), o **Web Push (VAPID + RFC 8291)**
é o único parcialmente escrevível em `std`: HKDF-SHA256 já temos, o transporte é
HTTP/1.1 simples. **Mas** pede duas decisões do dono: (a) abrir uma **segunda
curva, P-256** (Weierstrass, do zero, como o Ed25519), e (b) **reabrir a decisão
«nenhum AES»** — a RFC 8291 fixa AES-128-GCM e não aceita ChaCha20. APNs/FCM
colidem de vez (HTTP/2, TLS-cliente, JWT ES256/RSA) — mesma exceção de rede do
item «Ollama/TLS» já pendente. SMS é **gateway pago externo** (decisão comercial),
com o protocolo SMPP cabendo em `std` mas a dependência sendo o contrato, não o
código.

### 2.4 Dictionary encoding por coluna (E5 SAP HANA)

Isolado do column-store (que recusamos, ver 3.1), aplicado **dentro da linha do
`.reg`**: comprimir coluna de baixa cardinalidade (enum de `ao_excluir`/
`ao_alterar`, UF, status) por dicionário, reduzindo bytes por linha. Compatível
com row-store. **Premissa a medir antes de código:** quais colunas reais têm
cardinalidade baixa o bastante, e quanto byte pouparia contra o custo de uma
indireção no `inserir` quente — como o `.tbm` foi medido e recusado.

---

## Tier 3 — RECUSA MEDIDA / não entra, com o número

- **Column-store in-memory (HANA delta/main, Oracle In-Memory) — recusa
  CONFIRMADA (E5, J2).** O HANA prova, na própria produção (KBA de suporte sobre
  lentidão do *delta merge*, trava exclusiva em dois pontos, RAM dobrada na fusão),
  que colunar cobra preço estrutural **mesmo com tudo em RAM**. Em disco nós
  pagaríamos isso **mais** a E/S que o HANA nunca paga. Reforça `MEMORIA.md`
  («seríamos outro banco, e somos row-store») e o `GPU.md §7` (achatar `Vec<Value>`
  custa mais que a varredura que aceleraria).
- **LSM / compaction (E4 Cassandra) — premissa velha, candidato fraco.** Os 83,5%
  do `.ndx` que motivavam a LSM **já caíram para 34,6%** (cache de páginas +
  write-back + CRC slice-by-16, `DESEMPENHO.md §1`). O maior custo hoje é o
  `.reg`+`.log` (60,5%), que **já é append-only** — a LSM não compra o que já
  temos. E a compaction **reescreve e reordena**, quebrando quatro pétreas (ordem
  de digitação, endereço por `rowid`, paginação por cursor, replicação por
  `rowid`). Saída coerente já registrada: um `PHX-LSM` **separado** para
  log/telemetria, onde a ordem de digitação não é sagrada — projeto próprio, não
  ajuste do motor.
- **Paxos / LWT (E4 Cassandra) — não aplicável.** Resolve compare-and-set num
  cluster **sem líder**; nós somos single-leader com trava global. **Zero
  convergência dos quatro motores de referência** (nenhum é leaderless) — pela
  régua ponderada, implementar Paxos seria **divergir**, não convergir. Fechar
  como «estudado e recusado por não-aplicabilidade estrutural».
- **Tombstones (E4) — confirmação, não candidato.** O «tombstone hell» é doença de
  múltiplos arquivos imutáveis que a leitura reconcilia; o nosso soft-delete
  (`.trash`/`softdeleted`, posição fixa) **não herda** esse modo de falha.

---

## Confirmações — onde JÁ convergimos, ou estamos À FRENTE (registro para o dossiê de tecnologias)

- **CRC-32 por página:** o PostgreSQL só ligou por padrão em 2025 (v18, ~3–3,5%
  de TPS); nós fazemos desde a fundação (J1). E o nosso slice-by-16 **dobra** o
  slice-by-8 do InnoDB sem depender de instrução de hardware (o polinômio IEEE
  do PhxSql difere do CRC-32C do InnoDB, e trocá-lo invalidaria todo o disco — E1).
- **Cascata visível à replicação:** o PhxSql já grava a filha por um `atualizar`
  inteiro (índice + `.log` + neta) — **o que o MySQL só corrigiu em 9.6 (jan/2026)**
  (E1). O que nos falta é o oposto: a **atomicidade** (Tier 1.1).
- **Transação multi-tabela** (pedido 162) **ultrapassa** o LWT do Cassandra, que é
  preso a uma partição só (E4).
- **FFI blindado:** o `punho.rs` blinda toda função exportada; o `rusqlite`
  confirma que isso é o certo quando **100% das chamadas são reentrantes** (C
  chamando Rust), não excesso de cautela (E2).
- **QA — cobertura de branch/MC/DC:** o TH3 do SQLite (código de teste 590× o do
  produto, 100% branch+MC/DC) endossa a disciplina do papel G de **aposentar** a
  catraca quando a régua muda, em vez de subir o teto (E2).

---

## Ordem recomendada ao dono

1. **P0 + ACID-C (1.1)** — o núcleo; gate dos motores V/H; maior retorno, sem formato.
2. **Ledger URGENTE (2.1, primeiro item)** — travar `alterar_tabela` em modo ledger; risco atual.
3. **Medições sem formato** — premissa do dictionary encoding (2.4); materialized views se o otimizador cachear plano (1.2).
4. **Formato/decisão do dono** — `rowts`/HLC (2.2), Web Push (2.3), endurecimento restante do ledger (2.1).

**O que NÃO volta a discussão sem número novo:** column-store, LSM no motor
padrão, Paxos — recusas medidas, cada uma com o motivo acima.

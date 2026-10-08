# Parecer do papel C: expurgo do diário (`.log`) do caixa, pedido 706

08/10/2026 · papel C (DBA sênior) · só leitura, sem `cargo` · nível: modelo forte (formato em disco)

## 0. Veredito em uma linha

**Dá para expurgar, mas "por volume, como já existe" não existe para o caixa.** Hoje o desenho
apagaria evento em silêncio de três jeitos, e o expurgo não entra antes de dois consertos de
formato e um pré-requisito de replicação (§4, §6).

Três fatos medidos no código desmontam a premissa do pedido:

1. **A tabela do caixa não vira de volume.** `vendas`, `itens` e `estoque` nascem sem
   `registros_por_arquivo` (`bancada/caixa-offline/medir.py:246-268`). Sem esse campo a paginação é
   `DESLIGADA` (`phxsql-core/src/schema.rs:1329`, teste `valores.rs:2068`), o corte do diário é
   ignorado (`phxsql-store/src/diario.rs:69`: `if corte == 0 || !esquema.ligada()`) e
   `Volumes::candidatos` devolve só `[1]` (`volume.rs:792`). Resultado: **o `.log` do caixa é um
   arquivo único que nunca fecha**. Não sobra volume fechado para expurgar.
2. **A posição é o ordinal do evento contado a partir de zero no PRIMEIRO volume que existe.**
   `LogFile::total` soma os `quantos` dos volumes que existem (`log.rs:1407`), e `percorrer` começa
   `vistos = 0` no primeiro volume existente (`log.rs:1485`). Se o volume 1 sai, **toda posição
   desliza para trás**: a réplica que pede `desde: N` recebe o evento N+k, sem erro. É o defeito do
   pedido 487 (paginar por contagem com expurgo no meio), agora no fio da replicação, e com dado
   gravado errado na ponta.
3. **Refazer a réplica hoje é reaplicar o diário desde o evento 0.** A rota de recuperação da
   tabela rompida é: o operador apaga a tabela na réplica, ela renasce do esquema da origem e puxa
   a partir de `posicao == 0` (`servidor.rs:4916-4930`). Também o bidirecional promete que "perder
   `replicacao-posicoes.json` recomeça do zero e é inofensivo" (`docs/REPLICACAO.md` §12). As duas
   promessas morrem no primeiro expurgo. **"Soltar e refazer a réplica" não tem caminho hoje.**

## 1. Quem lê o `.log` além do fio da réplica

Apagar o que um destes leitores precisa é perda de dado. A coluna da direita diz o que acontece com
cada um depois de um expurgo.

| Leitor | Onde | Lê | Depois do expurgo |
|---|---|---|---|
| Replicação, servir (fiel, espelho, quorum) | `servidor.rs:32107` `op_replicar` → `:32027` `eventos_para_o_fio` (`diario_com_imagem_ate(desde…)` em `:32046`) | de `desde` em diante | **piso** do expurgo: `desde` < base tem de virar recusa explícita |
| Anúncio de posição | `servidor.rs:31842` `op_posicao` (`eventos()`) | total | tem de continuar **monótono**: base + eventos |
| Réplica fiel: a posição dela é o próprio `.log` | `servidor.rs:4406` `abrir_para_replicar`, `:5209`/`:5260` (`eventos() != f.posicao`), `:4521` | total local | o central também pode expurgar o **dele**, desde que `total` siga contando a base |
| Continuidade da réplica | `servidor.rs:4556` `diario_local_continua` (lê `ultimo` em `:4565`) | 1 evento em `posicao-1` | precisa do evento `posicao-1`: o piso é `posicao-1`, não `posicao` |
| Bidirecional: absorver e ancorar o id do grupo | `servidor.rs:7853` `absorver_diario_local` (`:7899`), `:7817` `adotar_o_id_do_grupo` (`:7825`) | de `vistos` em diante, e a cauda | o piso inclui a posição consumida pelo par (`replicacao-posicoes.json`) |
| PITR: reaplicar o diário vivo sobre o backup | `servidor.rs:28190` `reaplicar_diario_ate` (`:28253`), `:28396` `diario_vivo_continua` (`:28412`) | da posição do backup em diante | backup mais velho que a base perde o PITR. Tem de **recusar dizendo "expurgado"**, não "a cópia diverge" |
| Marca da transação da réplica (676/699/701) | `marca.rs:1623` `id_da_metade_que_entrou` (cauda), `:1910` `aplicar_evento_da_marca` (`diario(ev.posicao,1)` em `:1918`), `:2079` `tocada_depois` (`:2093`) | posições guardadas na marca | expurgo **nunca** roda com marca de pé |
| Evento devido (498) | cabeçalho do **volume 1**: `log.rs:855` (`l.cab(1)?`), `:929` | volume 1 obrigatório | a abertura **cai** sem o volume 1. A marca tem de morar no primeiro volume existente |
| Auditoria | `servidor.rs:31552` `op_diario` (rowid em `:31559`, cauda em `:31571`); `log.rs:1663` `historico`; CLI `phxsql-cli/src/main.rs:397-398`; FFI `phxsql-ffi/src/lib.rs:1473`/`:1521` | do 0, ou a cauda | o histórico local encolhe. No espelho a cópia fica no diário do central (a réplica grava o próprio evento) |
| Conferência | `table.rs:9400` `verificar` | todos os volumes | segue certo por volume. O total tem de somar a base |

O `.reg` **não** depende do `.log`: a venda mora no `.reg` do caixa. O que se perde num expurgo
errado é o **caminho** da venda até o central, e para o central isso é venda perdida.

## 2. A regra de quanto se pode tirar

Expurga-se o **prefixo de volumes fechados** v₁…vₖ, e só quando valem as cinco condições:

1. **Vale para todo consumidor que segura**: `base(vₖ₊₁) ≤ min(confirmado_durável) − 1`. O
   `−1` existe porque a continuidade lê o evento `posicao-1` (`:4565`, `:28412`).
2. **Nunca o volume ativo**, e só prefixo (o `existentes()` da trilha já supõe isso, `volume.rs:752`).
3. **Nenhuma marca de pé**: nem evento devido (498) nem marca de transação da réplica.
4. **O piso dos consumidores declarados vem de estado DURÁVEL.** Se esse estado não existe ou não
   se lê, o expurgo não roda. A falha cai sempre para o lado de segurar, nunca de soltar.
5. **Mesmo motor do expurgo da trilha (368)**: três fases, `fsync` do rastro fora da trava,
   `(Result, PorSincronizar)` (`table.rs:9176`/`:9189`, `servidor.rs:27339`/`:27452`/`:27527`). Um
   segundo expurgador seria a decisão escrita duas vezes.

**Quem segura.** Só a réplica **declarada** na origem, no análogo do slot do PostgreSQL. Réplica
não declarada não segura nada: se ficar atrás da base, recebe a recusa explícita. A origem de
pull, hoje, **não guarda** posição de réplica nenhuma de forma durável: o `Cubo` do quorum guarda
em memória (`quorum.rs:217-220`) e perde tudo no reinício.

**O que conta como confirmado.** É a posição que a réplica diz já ter em disco. O `desde` sozinho
não serve: ele é o `eventos()` da réplica, que conta cauda ainda sem `fsync`, e numa queda a
réplica volta pedindo menos. O campo novo `"duravel"` no `replicar` é pedido, não imposto: réplica
antiga que não o manda faz a origem cair no `desde` com **um volume inteiro de margem**. Essa
margem é premissa a medir (§5, prova 6).

## 3. O que os maduros fazem, e a decisão para "réplica fora tempo demais"

| Motor (peso) | Retenção | Réplica atrasada desconectada | Padrão de fábrica |
|---|---|---|---|
| **PostgreSQL (4)** | `wal_keep_size` (padrão 0); o **slot** segura o WAL até o `restart_lsn`; `max_slot_wal_keep_size` põe teto (PG13+) | dentro do teto: segura. Acima do teto: o slot fica `wal_status = lost`, o standby erra e tem de ser **refeito** (`pg_basebackup`) | `max_slot_wal_keep_size = -1`: **segura sem teto**, e o disco enche |
| **MariaDB (3)** | `expire_logs_days` / `binlog_expire_logs_seconds`; `max_binlog_total_size` (11.4); `slave_connections_needed_for_purge` (11.4); o expurgo não apaga binlog que uma réplica conectada está lendo | desconectada não é protegida: na volta, erro 1236, e a réplica tem de ser **refeita** (`mariabackup` + GTID) | `expire_logs_days = 0`, sem teto de tamanho: **segura** |
| **MySQL (2)** | `binlog_expire_logs_seconds`; `PURGE BINARY LOGS` não apaga o arquivo que uma réplica **conectada** está lendo | desconectada: erro 1236 na volta, e a réplica tem de ser **refeita** | 2.592.000 s (30 dias): **solta** por tempo |
| **SQLite (1)** | o checkpoint do WAL não passa do instantâneo de um leitor ativo, e o WAL cresce enquanto o leitor segurar | não há réplica | **segura** |

**Convergência dos três maduros, aceite automático e nada nosso contra.** Os três têm (a) um
teto configurável de retenção, (b) o expurgo nunca apaga o que um consumidor ativo está lendo,
(c) a réplica que fica atrás do teto recebe **erro explícito**, nunca um pulo calado, e (d) a
réplica atrasada se **refaz** a partir de um retrato com posição (`pg_basebackup`,
`mariabackup`/clone + GTID). Isso entra.

**Divergência no padrão de fábrica, decidida pela régua.** Segurar = PG 4 + MariaDB 3 + SQLite 1 =
**8**. Soltar = MySQL 2 = **2**. **Padrão: segurar (8 × 2)**, isto é, expurgo desligado por
omissão e teto ilimitado. Coincide com "guarda nova entra pedida": todo banco de hoje continua
guardando tudo.

**Teto por bytes ou por tempo.** Por bytes = PG 4 + MariaDB 3 = **7**. Por tempo = MySQL 2 +
MariaDB 3 = **5**. **Vence o teto por bytes (7 × 5).** O risco do caixa é o disco, e o tempo pode
vir junto como segunda condição.

**O choque que aparece: o padrão da régua contra a decisão do dono 680.** "Segurar sem teto" no
caixa termina em disco cheio. Disco cheio no `write` do `.log` **derruba o servidor** (498). A
decisão 680 diz que o caixa vende com o central caído. Então **o perfil caixa precisa de teto**, e
no teto a regra é **soltar**: o central recebe "expurgado" e é refeito. Não é choque com pétrea,
porque a convergência dá o teto e a decisão 680 escolhe a ponta. Mas soltar só é entregável quando
o item (d) existir: hoje refazer a réplica é reaplicar do zero (§0.3), e soltar sem retrato deixa o
central **sem aquelas vendas para sempre**.

## 4. Formato: o que existe não basta

| # | Mudança | Onde | Migração |
|---|---|---|---|
| F1 | **Base ordinal do volume** (eventos antes dele), u64 | cabeçalho de 128 bytes do `.log`, bytes **104..112** (hoje reservados e gravados zero; `cofre.rs:857` usa só 96..104). Herdada na virada (`base(v) + quantos(v)`), como o `ultimo_tx` do 684. O expurgo grava a base no primeiro sobrevivente e faz `fsync` **antes** de apagar. `total` = base(primeiro) + Σ quantos; `percorrer` começa `vistos = base`; `pular < base` → erro `DiarioExpurgado{base}` | ausência benigna, **sem subir versão**. Zero no volume 1 é verdade. Zero num primeiro-existente ≠ 1 é **"não sei"** e recusa: volume só fecha cheio, então expurgo verdadeiro dá base > 0. Volume v2 (64 B) não tem o campo e não pode ser o primeiro sobrevivente. Binário anterior: cai em `cab(1)` (`log.rs:855`) com o volume 1 ausente. Falha alta, não desliza, mas a mensagem é ruim. O binário entre este e o seguinte zera 104..112 ao regravar (a mesma armadilha do 684): a recusa do "não sei" cobre esse caso |
| F2 | **O diário vira de volume mesmo com a tabela sem paginação** | `diario.rs:69` e `volume.rs:792`. Adotar o **formato B da trilha** (`Nomes::DaTrilha`, `volume.rs:29-45`): ativo `<t>.log`, fechados `<t>#NNN.log`, **sem teto de volumes** (o teto de 999 do `.log` paginado também sai) | o `vendas.log` de hoje vira o ativo volume 1 e fecha como `vendas#001.log`. Nada se reescreve, e isso já foi provado com fixtures no 368. É mudança de formato: **entra cedo**, antes de haver dado em produção |
| F3 | **Marca do evento devido (498) no primeiro volume EXISTENTE**, não no volume 1 | `log.rs:855`, `:929`, `cofre` `off_marca` | sem migração: a marca só fica de pé entre a queda e a reabertura, e o expurgo não roda com ela de pé |
| F4 | **Posição confirmada por réplica**: estado do servidor, não da tabela | arquivo novo da origem, `replicacao-retencao.json` (réplica declarada → `{db/tabela: duravel, visto_ms}`), gravado pelo `sincronia::gravar_duravel` (padrão do 535/598). **Fora do `.log` e fora do `PSCH`**: regravar cabeçalho a cada pull seria a escrita por commit que o `.log` tirou do caminho | arquivo ausente ou ilegível = expurgo parado (segura) |
| P | `"duravel"` no `replicar` (pedido); `replicacao.retem: [ids]` e `diario.expurgo: {ligado, teto_mib}` no config | protocolo/config | réplica antiga segue funcionando, com a margem de um volume |

`PSCH` não muda. A ordem de digitação não é tocada: nenhum slot do `.reg` sai, nenhum número de
volume se reusa (formato B nunca reusa) e nenhuma posição se reusa (a base só sobe).

**Pré-requisito fora do formato.** Réplica semeada por **retrato + posição** (o item (d) da
convergência) antes de ligar "soltar" em qualquer perfil. Sem ele, o expurgo só pode rodar no modo
"segurar com teto = nunca soltar", que é o que o caixa não pode ter.

## 5. A prova que derrubaria o desenho

Qualquer uma destas vermelha derruba o desenho.

1. **Posição deslizando.** Expurgar v1 e pedir `replicar desde: base−1`: tem de recusar com
   `DiarioExpurgado`. Pedir `desde: base+3` tem de devolver o evento cujo carimbo, rowid e versão
   são os do evento base+3 de antes do expurgo. Com o defeito reposto (`vistos = 0`), a prova tem
   de falhar.
2. **Queda no meio.** `kill -9` (e a falha injetada da `sincronia`) entre gravar a base e apagar, e
   entre apagar e o `fsync` da pasta. Na volta, `total` igual ao de antes e posições idênticas.
3. **Base perdida.** Zerar 104..112 do primeiro sobrevivente: a abertura recusa, não lê como zero.
4. **Estado de retenção perdido.** Apagar `replicacao-retencao.json`: zero volume expurgado até as
   réplicas declaradas reconfirmarem.
5. **Bancada do 678 com expurgo ligado.** Central fora por mais de 1 volume e menos que o teto. O
   `.log` do caixa fica limitado a (volumes não confirmados + ativo). Na volta, SHA-256 linha a
   linha caixa × central e nenhuma venda faltando nem dupla.
6. **A margem do `desde`.** Réplica sem `"duravel"`, morta depois do pedido e antes do `fsync`
   dela. Ela volta pedindo menos, e o pedido tem de cair dentro da margem. Se cair abaixo da base,
   a margem de um volume é falsa e o fallback morre.
7. **Central fora além do teto.** O caixa segue vendendo (zero venda recusada), o central recebe
   `DiarioExpurgado` e se refaz por retrato com SHA igual. **Hoje esta prova não tem como passar**
   (§0.3), e é ela que segura o "soltar".
8. **PITR.** Backup anterior à base: o restaurar recusa nomeando "expurgado", não "a cópia diverge".

## 6. O que sobe ao dono

**Uma coisa, da categoria Produto (SLA):** quanto tempo o central pode ficar fora antes de o caixa
soltar o diário e o central ter de ser refeito por retrato. O número é o `teto_mib` do perfil caixa,
e na prática ele vira dias de venda (1.454–1.629 B por venda, bancada do 678, 08/10/2026).

A direção não sobe. O padrão "segurar" vem da régua, 8 × 2. O "soltar no teto" do caixa vem da
decisão 680 mais a convergência dos três maduros. O teto por bytes vem da régua, 7 × 5.

O resto é engenharia, na ordem: F2 → F1 → F3 → F4/P → retrato da réplica → expurgo ligado.

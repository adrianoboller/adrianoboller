# Recuperação e réplica: um desenho só

08/10/2026 · papel C (DBA sênior) · só leitura, sem `cargo` · nível: modelo forte (formato em disco e
recuperação). Ordem do dono de 08/10/2026: fechar a recuperação e a réplica como **um** desenho,
revisado de ponta a ponta, em vez de mais remendos.

**Base lida:** HEAD `024e42c1` mais a árvore de trabalho do dia (o `servidor.rs` tinha mudança não
comitada de outra frente; as linhas citadas são as dela). **Nada aqui foi medido por mim**: todo
furo novo está marcado **lido**, com a cadeia escrita, e a prova que o confirma ou o mata é a etapa
E0 do plano (§5). Furo lido que a prova desmentir fecha como hipótese morta.

---

## 0. Resumo

1. São **13 caminhos** que gravam ou recuperam estado durável, e **6 motores de recuperação**
   diferentes (marca no store, marca do bidi no servidor, evento devido na abertura, troca de
   volume na abertura, índice marcado, palco da restauração). Os remendos 676–702 consertaram
   cada um pela metade porque **cada motor responde «já foi aplicado?» com uma régua própria**.
2. A régua que funciona já existe e é a da réplica (699/701): **duas faces** — o diário na
   posição conferida **e** o `.reg` com o conteúdo. A do `COMMIT` é mais fraca: «o slot existe»
   para a inclusão e **nenhuma** para alteração, exclusão suave e restauração, que se reaplicam
   sem condição.
3. Daí o furo mais grave, **F1 (lido)**: a marca do `COMMIT` que só esperava a janela é
   reaplicada no arranque **por cima de escrita posterior** na mesma linha. No bidirecional a
   reaplicação sai com o carimbo do arranque e **ganha o «mais recente vence» no par inteiro**.
4. Mais 10 furos lidos, todos irmãos de pedidos já fechados (§3.6). Os principais: a inclusão do
   `COMMIT` com o slot gravado e sem evento nunca completa o diário (F2, irmão do 699); dois
   geradores de id de marca, e o do servidor sem piso do disco (F4, irmão do 684); a restauração
   não completa as marcas e reconstrói o índice antes (F8, inverte o 522); o erro de dado no
   meio do grupo da réplica apaga a marca com meia venda aplicada (F9, contra o 685).
5. **Desenho:** generalizar o `marca.rs` (não criar motor novo): um **bilhete posicional** (marca
   v7/v8, que leva o `tx` e, por operação, a posição esperada no diário e a versão do slot antes),
   **uma** regra de duas faces para toda família e **um** escalonador por id numérico através das
   famílias. Família, chave de identidade e aplicador viram parâmetro.
6. **Os maduros convergem** (PostgreSQL, InnoDB, SQLite): ponto de compromisso único, nunca
   reaplicar por cima de estado mais novo (LSN da página), recuperar antes de servir, réplica sem
   transação parcial. Entra sem pergunta. O **meio** deles (undo, rollback, reuso de espaço)
   bate na ordem de digitação; o comportamento entra pela pré-conferência e por andar só para a
   frente.
7. Plano: **E0 a E9**, a primeira só de prova (F). Formato muda **uma vez** (marca v7/v8 e
   `tx` do evento devido nos bytes 112..120 do `.log`), agora, antes do selo 0.19.0.
8. **Nada sobe ao dono.** O 498 H3 (escrita solta morta entre o `.reg` e o `.log`) continua ⏸ por
   decisão dele, e este desenho não o reabre.

---

## 1. Inventário

Legenda da ordem: **R** = slot do `.reg`, **N** = `.ndx` (byte 52 sobe antes), **T** = `.fts`,
**L** = evento no `.log`, **M** = marca `.tx`, **S** = `fsync`. Toda escrita de linha segue
`R → N → T → L` (`Table::inserir_com_maes_opt`, `table.rs:6219`; o evento entra por
`Table::anotar_imagem`, `table.rs:6430`, e `LogFile::registrar_depois_da_linha`, `log.rs:1155`).
**O dado vai antes do diário**, sem `fsync` por operação; o `fsync` é da janela.

| # | Caminho | Ordem | Ponto de compromisso | Quem recupera, quando | Idempotente por | Motor (arquivo:função:linha) |
|---|---|---|---|---|---|---|
| 1 | **`COMMIT` local** | pré-conferência → M+S (sob a trava, em voo) → passada `R N T L` por escrita → marca vai às pendentes → janela: S das tabelas → apaga M | `fsync` da marca v3/v4 | arranque (`Servidor::novo` → `transacao::recuperar` → `Database::recuperar_marcas`); pânico: `reparar_a_trava` → `completar_marca_em_voo` | **rowid** só na inclusão (`slot_ja_consumido`); alteração, exclusão suave e restauração **sem condição** | `servidor.rs:20669` `op_commit`, `:21385` `depois_da_marca`, `:21474` `passada_sob_a_marca`, `:21638` `aplicar_conjunto`, `:22038` `descarregar_sujas_com`; `marca.rs:615` `gravar_marca`, `:1429` `completar`, `:1807` `aplicar_na_recuperacao` |
| 2 | **Cascata solta** (`ao_alterar` fora de transação, 540) | igual ao 1, lista achatada (`cascata_na_lista`) | idem | idem | idem | `servidor.rs:26611` `atualizar_com_a_marca`, `:26456` `alterar_solto` |
| 3 | **Cascata do embutido** (563) | M+S no diretório da mãe → mãe → elos → S de cada → apaga M | idem | `phx_base_abrir` sem punho vivo, `reindex` da CLI | idem | `table.rs:4148` (`proximo_id_no_diretorio` + `gravar_marca`); `marca.rs:2243` `recuperar_no_diretorio`; **sem unidade de transação** |
| 4 | **Grupo da réplica, pull e quórum** (682) | conferência de continuidade (tomada própria) → M+S v5/v6 **fora** da trava → trava: reconfere posição → em voo → `aplicar_evento` por evento → solta → S das tabelas → apaga M | `fsync` da marca do grupo | arranque, pelo mesmo `recuperar_marcas`; pânico: reparo | **posição do diário conferida** (carimbo, op, rowid) + **`.reg` conferido** (699) + **conteúdo** (701) + `Seguintes` | `servidor.rs:5026` `alcancar_database`, `:32753` `aplicar_database_do_quorum`, `:5183` `aplicar_grupo_da_replica`, `:5400` `marcar_o_grupo`; `marca.rs:654` `gravar_marca_da_replica`, `:1910` `aplicar_evento_da_marca`, `:1990` `conferir_o_reg_do_evento` |
| 5 | **Bidirecional** (698/700) | M+S `bidi_<id>.tx` (mesmo `gravar_com`) **fora** da trava → `aplicar_itens_bidi` pela chave → S → apaga M | `fsync` da marca | arranque, **depois** do 1–4 e do índice: `Servidor::novo` → `completar_marcas_do_bidi` | «mais recente vence» + `slot_sem_inclusao_no_diario` (700) | `servidor.rs:7280` `alcancar_database_bidi`, `:7465` `aplicar_grupo_bidi`, `:7516` `marcar_o_grupo_bidi`, `:7570` `completar_marcas_do_bidi`, `:7712` `aplicar_itens_bidi`, `:7795` `slot_sem_inclusao_no_diario`; `marca.rs:704` `gravar_marca_do_bidi` |
| 6 | **Escrita solta** (inserir/atualizar/excluir sem cascata) e **carga** (`inserir_lote`/`importar`/`carga`, 686) | `R N T L` por linha, numa tomada | **nenhum**: não há bilhete | ninguém completa o par `R`/`L`; o índice volta pelo 522 | — | `servidor.rs:26362` `op_atualizar`, `:26736` `op_excluir`, `:26053` `op_inserir_lote`; `table.rs:6219` |
| 7 | **Evento devido** (498) | `R` gravado → `write` do `L` falha → 16 bytes no cabeçalho do volume 1 (no lugar) → derruba | a marca no cabeçalho | **abertura gravável da tabela**, em qualquer momento | trio (op, rowid, carimbo) na cauda | `log.rs:1208` `marcar_devido`, `:1223` `completar_devido`; `table.rs:2659` → `:7666` `completar_evento_devido` |
| 8 | **Troca de volume em duas fases** (`acrescentar_coluna`, migração da cifra, `migrar_esquema`) | congela → fase A fora da trava: `*.novo` + S → trava: confere retrato → fase B: `rename` duráveis, volume 1 primeiro | `rename` do **volume 1** | abertura gravável: `terminar_troca_interrompida` | geometria do volume + nº de slots igual | `servidor.rs:23332` `op_acrescentar_coluna`, `:23494` `op_migrar_esquema`, `:27053` `op_migrar_cifra`; `reg.rs:757`, `:1794`, `:2385`/`:2491`; `table.rs:1957`/`:2224` |
| 9 | **Índice marcado** (522/533) | byte 52 sobe antes do 1.º `R`; só o `sincronizar` (depois dos dois S) o baixa | o byte 52 | arranque, **depois** das marcas; restauração no palco | reconstrução do `.reg` | `catalogo.rs:1042` `reconstruir_indices_marcados`; chamado em `marca.rs:2190` e `restaurar.rs:636` |
| 10 | **Backup** (513, duas fases) | fase 1 sem escritor excluído; fase 2 sob a trava recopia o que mudou e tira o que sumiu; S das cópias fora da trava; manifesto por último | o manifesto (`backup.json`) / o `rename` do `.zip.part` | — | — | `backup.rs:1282` `copiar_fase_1`, `:1333` `acertar_fase_2`; a marca `.tx` **viaja** (doc do módulo, `backup.rs:49-52`) |
| 11 | **Restauração e PITR** | palco: copia + S → migra separador → **reconstrói índice marcado** → PITR reaplica o diário vivo no palco → trava → `rename` do palco | o `rename` do palco | — (**nenhuma marca é completada**) | — | `restaurar.rs:535` `preparar`, `:655` `confirmar`; `servidor.rs:27872` `op_restaurar_backup`, `:28304` `reaplicar_diario_ate` |
| 12 | **Expurgo do diário** (706, planejado) | grava a base ordinal + S → apaga o prefixo de volumes → S da pasta | a base no 1.º sobrevivente | abertura | base > 0 | `docs/propostas/expurgo-do-diario-706.md` §4 (F1–F4) |
| 13 | **Semeadura da réplica por cópia** (706, pré-requisito) | não existe | — | — | — | o 706 a exige antes de «soltar» |

E o reparo do pânico (451) não é caminho próprio: é o corpo do 1–4 rodado com a trava na mão
(`NoArranque::Gravada`), e para a marca do bidi ele **recusa** e derruba (700).

**O que o inventário mostra:** os caminhos 1–5 já dividem o leitor, o selo, o `create_new` 0600 e
o `fsync` da marca (`gravar_com`, `marca.rs:747`) — e divergem justamente na pergunta que importa,
«esta operação já entrou?». O 7, o 8 e o 9 têm cada um a sua resposta, e estão certos isolados;
os furos aparecem **no encontro** deles com o 1–5 (F6, F8, F11).

---

## 2. As invariantes

Escritas para virar teste. Coluna da direita: quem viola hoje (furo da §3.6) ou **✓** com a prova
que já existe.

| | Invariante (verificável) | Hoje |
|---|---|---|
| **I1** | Toda escrita de mais de uma peça tem **um** ponto de compromisso, durável **antes** do primeiro byte que não se desfaz; antes dele a operação não aconteceu, depois dele aconteceu. | ✓ 1–5, 7, 8, 10, 11 · ✗ 6 (⏸ 498 H3) |
| **I2** | **Duas faces.** Depois da recuperação, para todo rowid: slot vivo ⇔ inclusão no diário sem exclusão posterior; e o conteúdo do slot = a imagem do último evento que a carrega. «Nenhum evento sem a linha, nenhuma linha sem o evento.» | ✓ réplica (699/701), bidi (700), devido (498) · ✗ **F2** (`COMMIT`), ⏸ escrita solta |
| **I3** | **Transação inteira ou nada**, para o leitor local depois do arranque e para o leitor da réplica: nenhum estado observável contém parte de um `tx`. | ✓ fio (676), processo da réplica (682), bidi (698) · ✗ **F8** (restauração), **F9** (erro de dado no meio do grupo) |
| **I4** | **Um `tx` por transação no diário**, inclusive nos eventos que a recuperação completa e no evento devido. | ✓ 701/702 · ✗ **F6** (devido), **F7** (embutido), **F10** (PITR) |
| **I5** | **Ids monotônicos entre arranques**: o `tx` do `.log` e o id da marca nunca saem abaixo do maior já gravado, mesmo com o relógio recuado. | ✓ `tx` (684) · ✗ **F4** (id de marca do servidor) |
| **I6** | **Slot nunca reaproveitado**: a recuperação só anda para a frente; slot livre dentro da faixa recusa nomeando a lacuna; troca de volume preserva rowid e ordem; expurgo não toca o `.reg`. | ✓ (`slot_ja_consumido`, `marca.rs:1786`; troca §1.1 do FORMATO; 706 §4) |
| **I7** | **A posição do diário é ordinal absoluto e nunca desliza** (inclusive depois de expurgo). | ✓ hoje (não há expurgo) · exigência do 706 F1 |
| **I8** | **A recuperação nunca reaplica por cima de estado mais novo.** Cada operação sabe reconhecer que a unidade de dado já passou dela. | ✓ réplica (`tocada_depois`) · ✗ **F1** (`COMMIT`, cascata, embutido) |
| **I9** | **Idempotente e reiniciável**: completar duas vezes = completar uma; completar uma marca cuja passada terminou **grava zero bytes e zero eventos**. | ✓ réplica · ✗ **F3** |
| **I10** | **Recuperar antes de servir**: nenhum leitor vê o intermediário — arranque, palco da restauração, semeadura, abertura do embutido. | ✓ arranque, embutido · ✗ **F8** |
| **I11** | A marca sai só depois de **toda** tabela que ela nomeia ter um `fsync` posterior à última escrita dela. | ✓ (`descarregar_sujas_com`, `:22038`; 536) |
| **I12** | Só `NaoConfere` apaga marca; erro de E/S, falta de chave e linha perdida **retêm**. | ✓ (`tratar_marca`, `marca.rs:1365`; 503, 354, 699) |
| **I13** | **A ordem de completar é a ordem de criação** (id numérico), através de **todas** as famílias. | ✗ **F5**, **F12** |
| **I14** | A recuperação não roda gatilho e não replaneja cascata: o que o bilhete traz é tudo. | ✓ (v3 achatada; 262 H4 põe os `AFTER` antes do ponto de compromisso) |

---

## 3. O desenho único

### 3.1 Qual motor — três hipóteses, escritas antes da escolha

| | Hipótese | Destino |
|---|---|---|
| **H-A** | **Generalizar o `marca.rs`**: um formato, uma regra de «já entrou?», um escalonador; família vira parâmetro. | **vence** |
| **H-B** | Um WAL global (um diário só para o database, redo único, como o PostgreSQL) | **morre**: reabre a posição global que o `REPLICACAO.md` §4 recusou («não precisa inventar um GTID»), põe um segundo `fsync` sequencial no caminho de toda escrita — e o nosso gargalo medido é o `.ndx` (83,5%), não o log —, e não compra nada que o par marca + `.log` com `tx` já não compre. A receita é boa para o gargalo dela. |
| **H-C** | Manter os seis motores e consertar furo a furo | **morre medida pelo histórico**: 676 → 682 → 684 → 698 → 699 → 700 → 701 → 702, oito pedidos em dois dias, cada um abrindo de 1 a 3 irmãos. O motivo é estrutural: a pergunta «já entrou?» está escrita quatro vezes. |

H-A é também o que a lei «função e comando do mesmo motor» pede: a decisão escrita duas vezes é a
régua do «já entrou?», e é ela que vira uma.

### 3.2 O bilhete posicional — marca v7 (em claro) / v8 (cifrada)

O que falta à marca do `COMMIT` para responder como a da réplica responde são três números, e
**os três são conhecidos no instante em que ela é gravada**, porque o `COMMIT` grava a marca com a
trava na mão (`servidor.rs:20856`):

| Campo | Onde | O que responde |
|---|---|---|
| `tx u64` | cabeçalho, depois do `n_operacoes` | o id de transação que **todos** os eventos desta marca levam. Reservado sob a trava por uma função nova do `log` (`reservar_tx_na_unidade`, irmã do `adotar_tx_na_unidade`): a unidade passa de `Some(0)` a `Some(tx)` antes do primeiro evento. Mata a heurística do 702 (`id_da_metade_que_entrou`) e o «não ancora» dela |
| `posicao u64` por operação | fim do payload, depois do byte da cascata (a mesma regra de sempre: campo novo só no fim) | o ordinal do evento desta operação no diário da tabela. É um **atalho**, não a identidade: a identidade é `(tx, op, rowid)` |
| `versao_antes u64` por operação | idem | a versão do slot (FORMATO §1, bytes 8..16) antes da operação — o **LSN da linha**. 0 = inclusão |

A réplica e o bidi continuam gravando v5/v6 (o `tx` deles é o da tomada local, que só existe
depois da trava, e a identidade deles é o carimbo e a origem de lá). Precondição que a regra
assume e que vira `debug_assert` na passada: **uma operação = um evento** na tabela dela (verdade
desde a v3 achatada e o 262; conferir na E3).

**Migração:** a marca é efêmera (vive entre o `COMMIT` e o fecho da janela), então não há dado a
converter. As v1–v6 continuam **lidas** pelo caminho de hoje. O risco é só o de voltar o binário
com marca v7/v8 de pé: o anterior a lê como `NaoConfere` e **apaga uma transação confirmada** —
a mesma armadilha já escrita para a v5/v6 («só vai para frente»). Nenhum binário foi selado, e é
por isso que entra agora.

### 3.3 A regra de duas faces — uma tabela para toda família

Para cada operação, na ordem da marca, com a tabela aberta:

| Face do diário (`L`) | Face do `.reg` (`R`) | Decisão |
|---|---|---|
| evento `(tx, op, rowid)` ausente | slot como antes (`versao == versao_antes`; inclusão: rowid além do fim) | **aplica** pelo aplicador da família |
| ausente | slot já passou dela (`versao == versao_antes+1` com o conteúdo da operação; inclusão: slot vivo com a linha) | **completa só o diário** do payload que está no disco — o `completar_o_diario_da_inclusao`/`_da_exclusao` de hoje, mais o irmão da alteração |
| presente | slot confere | **já estava**: grava zero bytes |
| presente | slot à frente (`versao > versao_antes+1`, ou conteúdo trocado por evento posterior do diário ou operação seguinte da marca) | **já estava**, e houve escrita depois: **não toca** (é o F1) |
| presente | slot atrás (`versao < versao_antes+1`, ou ausente sem exclusão posterior) | **linha perdida** (queda de energia): a marca **fica**, relatório — o 699 de hoje |
| ausente | slot à frente | já estava (a queda levou o evento e a escrita seguinte refez a linha): completa só o diário se a operação seguinte da mesma linha não o tiver; senão recusa nomeando |
| inclusão com slot livre dentro da faixa | — | recusa nomeando a lacuna (I6) |

Para v5/v6 a coluna do diário é a de hoje (posição + carimbo + origem); para v7/v8, `tx` e
posição; para v1–v4, o comportamento de hoje (sem `tx` nem versão não há como saber, e é só o
`COMMIT` que ficou de pé num binário anterior).

### 3.4 O que vira parâmetro

| Parâmetro | Valores | O que muda |
|---|---|---|
| `Familia` | `Commit`, `CascataSolta`, `Embutido`, `Replica`, `Bidi` | quem grava, onde (raiz qualificada / pasta do schema), e o prefixo do nome (`transacao_` / `bidi_`) |
| `Identidade` | `Tx(tx)` · `DeLa{carimbo, origem}` · `Chave` | a coluna «face do diário» da §3.3 |
| `Aplicador` | motor local com FK e cascata sem replanejar · `Table::aplicar_evento` como réplica · `aplicar_por_chave` (servidor) | a ação «aplica» |
| `Completador` | store · servidor (callback) | o bidi completa no servidor pelo mapa de toques; o store **escalona**, não completa |
| `NoArranque` | `Sim`, `Nao`, `Gravada` | já existe (`marca.rs:1335`) |
| `Unidade` | própria (adota o `tx` da marca) · a da tomada | já existe (`UnidadeDaMarca`, `marca.rs:1725`) |

Fica **um** corpo: ler (`ler_marca`), escalonar, abrir e preparar as tabelas, decidir pela
§3.3, parar na primeira recusa, sincronizar, apagar ou reter, relatar.

### 3.5 O escalonador único

Um passe por database, chamado de **quatro** lugares com o mesmo corpo: o arranque do servidor,
a abertura do embutido, o palco da restauração e o palco da semeadura.

1. **Estado físico de cada arquivo**, como hoje, na abertura gravável: troca de volume
   interrompida (`terminar_troca_interrompida`) e evento devido — este com o `tx` dele (§3.6 F6).
2. **Todas as marcas, de todas as famílias, por id numérico** (raiz e schemas): o store chama o
   completador da família; a do bidi é um callback que o servidor passa (no palco, onde não há
   servidor, o bidi roda em modo **só conferir**: passada terminada sai, passada pela metade
   recusa a restauração nomeando a marca).
3. **Índices marcados** (522), depois das marcas — a ordem que impede o passe do índice de calar
   a órfã.
4. Só então a porta abre, ou o palco é renomeado.

O gerador de id de marca é **um**: `max(relógio, maior id de marca em todas as pastas do database
e de todas as famílias + 1)`, que semeia o contador do servidor no arranque e é o mesmo que o
embutido já usa.

### 3.6 Os furos — onde cada caminho de hoje diverge do motor

Todos **lidos, não medidos**; a prova de cada um está na E0. «Na conta» = defeito ativo pela
regra (A) do dono (dado errado, garantia que não vale).

| | Furo | Cadeia (lida) | Irmão de | Inv. | Conta |
|---|---|---|---|---|---|
| **F1** | **A marca do `COMMIT` que só esperava a janela reaplica alteração, exclusão suave e restauração por cima de escrita posterior** | `COMMIT` altera X para «a»; a passada termina e a marca vai às `marcas_pendentes` (`passada_sob_a_marca`, `:21474`); escrita solta muda X para «b» (sem marca); `SIGKILL` antes do fecho da janela — o «b» está no cache do núcleo e sobrevive ao processo; o arranque completa a marca e `aplicar_na_recuperacao` (`marca.rs:1844-1876`) chama `atualizar` sem condição: **X volta a «a»**, com um evento novo de carimbo do arranque. O próprio teste `o_reparo_completa_so_a_marca_em_voo` (`servidor.rs:71919`, testes do 451) monta essa pré-condição e prova só o lado do reparo. A `TRANSACOES.md` §5.4 diz «a recuperação reaplica» e a §5.5 diz que ela «encontra todos os slots já certos e não faz nada»: vale para a inclusão, não para a alteração. **No bidi:** a alteração reaplicada sai com o carimbo do arranque, vence o «mais recente vence» contra o grupo do bidi que tinha entrado depois, e **vai para o par como a mais nova** — o outro lado também volta a «a» | 701 (a), que fez isso para a réplica e não para o `COMMIT`; 503/536, que fecharam outras vias da mesma reaplicação | I8, I9 | **sim** |
| **F2** | **Inclusão do `COMMIT` com o slot gravado e sem o evento: o diário nunca se completa** | `SIGKILL` dentro da passada entre `reg.inserir` e `anotar_imagem` (`table.rs:6219`); no arranque `slot_ja_consumido` dá `Ok(true)` e o caminho do `COMMIT` responde `JaEstava` (`marca.rs:1832`) **sem olhar o diário** — a resposta do 699 («completa só o evento») ficou só no braço da réplica (`:1957`). A linha nunca chega à réplica, e o `verificar` não acusa | 699 (a) | I2 | **sim** |
| **F3** | **Completar uma marca cuja passada terminou acrescenta eventos** | mesmo sem escrita posterior, cada alteração da marca regrava o slot e anexa um evento que nenhum cliente fez; a réplica encadeada o recebe; e uma cópia (backup, semeadura) que leve a marca desloca a posição do diário dela contra a da origem — a semeadura nasceria rompida pela conferência de continuidade | 702 (que já detecta a passada terminada para não adotar o id, e não usa a mesma detecção para não reaplicar) | I9 | **sim** (junto com o F1) |
| **F4** | **Dois geradores de id de marca, e o do servidor sem piso do disco** | `Transacoes::nova(agora_ms())` (`transacao.rs:468`, `servidor.rs:2366`) soma 1 a partir do relógio; o embutido usa `max(relógio, maior marca + 1)` (`marca.rs:1247`). Com o relógio recuado entre arranques e uma marca **retida** da vida anterior (sem chave, linha perdida), a nova nasce com id menor — completada antes da antiga no arranque seguinte — ou igual, e o `create_new` recusa o `COMMIT` | 684 (o piso do `tx`); a lei «do mesmo motor» | I5, I13 | sim (garantia de ordem) |
| **F5** | **Escalonadores separados**: todas as `transacao_*` de todos os databases antes de qualquer `bidi_*`, sem ordem de id entre as famílias | `Servidor::novo`: `transacao::recuperar` (`:2128`) e depois `completar_marcas_do_bidi` (`:2407`). Com o F1, a marca de `COMMIT` 10 é reaplicada antes do grupo do bidi 11 que veio depois dela | 698 | I13 | junto com o F1 |
| **F6** | **O evento devido completado dentro da unidade da recuperação tira um `tx` novo antes da adoção** | o 498 derruba no meio da passada; no arranque `completar` abre a `UnidadeDaMarca` (`marca.rs:1436`) e `id_da_metade_que_entrou` abre as tabelas (`:1635`) — a abertura completa o devido (`table.rs:2659`), que tira `tx` novo (`log.rs:780`, unidade em `Some(0)`); a cauda passa a ter dois ids, a adoção falha e o resto vai com o novo: **a transação parte em duas na réplica encadeada**. O mesmo no braço da réplica: `adotar_tx_na_unidade` (`marca.rs:1926`) chega com a unidade já ocupada. Em serviço, o devido entra no `tx` da primeira tomada que abrir a tabela, de outra transação | 702 pela via do 498 | I4 | sim (réplica partida) |
| **F7** | **A cascata do embutido não abre unidade**: um `tx` por evento | `Table::atualizar` com marca (`table.rs:4148`) roda fora de tomada, e fora de tomada «cada evento ganha o seu» (FORMATO §4). Base escrita pelo embutido e servida depois como origem entrega a cascata em pedaços | 701 (b) | I4 | sim, baixo alcance |
| **F8** | **A restauração não completa as marcas e reconstrói o índice antes delas** | a marca `.tx` viaja no backup de propósito (`backup.rs:49-52`), mas `Preparada::preparar` (`restaurar.rs:535`, `:636`) só reconstrói o índice marcado — a ordem do 522 invertida — e o database restaurado entra na raiz **com a marca de pé**. Ela fica para o próximo arranque do servidor de destino, que a completa por cima do que se escreveu no database restaurado desde então (o F1). Com a cópia de um servidor caído (CLI, fase 1 sozinha), a passada pela metade fica **visível** até esse arranque | 522 (ordem), 682 (recuperar antes de servir) | I3, I8, I10 | **sim** |
| **F9** | **Erro de dado no meio do grupo da réplica apaga a marca com parte aplicada** | `aplicar_grupo_da_replica`, braço `falhou` («fail-stop… a marca sai com o erro»); o bidi que para no meio (`paradas`) e o `ParouNoMeio` do `COMMIT` fazem o mesmo. A venda fica pela metade **para sempre** no central. A decisão do dono no 685 é «inteira ou não chega, **sem exceção**» | 685 | I3 | **sim** |
| **F10** | O PITR grava no palco fora de unidade: um `tx` por evento no database restaurado | `reaplicar_diario_ate` (`:28304`) | 701 (b) | I4 | baixo |
| **F11** | Marca pendente que atravessa uma troca de esquema volta como «impossível» falsa | `op_acrescentar_coluna` não drena a janela antes da fase A; a marca com linhas de N colunas encontra a tabela com N+1 e o `atualizar` recusa pela aridade. O dado está certo (foi copiado do núcleo); o relatório mente | 536 | — | não (relatório) |
| **F12** | `marcas_com_prefixo` ordena por **texto** (`marca.rs:2230`) | inofensivo enquanto os ids tiverem 13 dígitos; vira defeito no dia em que a largura mudar | — | I13 | não (latente) |

**Procurados e não achados** (o irmão foi procurado, e o motor de lá responde certo): a troca de
volume (volume 1 como compromisso, geometria e número de slots, recusa do `*.novo` incompleto); o
evento devido isolado (trio na cauda, I2 do próprio 498); o `fsync` antes de apagar marca nos
quatro chamadores (I11); a marca cifrada sem chave (I12); a posição da réplica atômica com o dado
(a posição **é** o diário dela, `REPLICACAO.md` §13).

**Conhecidos e fora, por decisão:** a escrita solta morta entre o `.reg` e o `.log` (498 H3, ⏸
pelo dono em 30/09); o slot livre dentro da faixa, que não volta para o lugar (lacuna escrita na
`TRANSACOES.md` §5.4).

---

## 4. A convergência com os maduros

Comportamento, não meio. Pesos PG 4, MariaDB 3, MySQL 2, SQLite 1. A matriz é do comportamento
documentado nos manuais dos quatro, **lida, não exercitada nesta rodada**; o papel J confere a
linha 6 (a do PostgreSQL lógico) antes da E7.

| # | Comportamento | PG (4) | MariaDB (3) / MySQL (2) | SQLite (1) | Soma | Decisão |
|---|---|---|---|---|---|---|
| 1 | **Um ponto de compromisso durável** decide se a transação aconteceu | registro de commit no WAL | XID no redo + binlog (2PC interno) | `commit` frame no WAL; apagar o journal no rollback-journal | 10 × 0 | **já é nosso** (`fsync` da marca) |
| 2 | **A recuperação nunca reaplica por cima de estado mais novo** | LSN da página ≥ LSN do registro → pula | idem, LSN da página no InnoDB | o journal se invalida no commit, antes de a próxima transação escrever; o WAL é sempre mais novo que o arquivo | 10 × 0 | **aceite automático** → I8, fecha o F1. O nosso LSN é a **versão do slot** (já no formato desde a v1) mais o `tx` no diário |
| 3 | **Recuperar antes de servir** | o postmaster não aceita conexão antes do fim do redo | o InnoDB recupera antes de abrir | o journal quente se resolve na primeira abertura, antes da leitura | 10 × 0 | **aceite** → I10, fecha o F8 |
| 4 | **Recuperação idempotente e reiniciável** | redo pode recomeçar | idem | idem | 10 × 0 | **aceite** → I9, fecha o F3 |
| 5 | **O mesmo log serve à recuperação e à réplica** | WAL (física e lógica) | binlog amarrado ao redo pelo 2PC | — | 9 × 0 | **já é nosso** (o `.log` é as duas coisas) |
| 6 | **Erro de aplicação na réplica não deixa transação parcial visível** | o replay físico não falha por dado; a réplica lógica desfaz a transação e para o worker | o applier desfaz a transação no InnoDB e para a thread SQL | — | 9 × 0 | **aceite do comportamento** → I3, fecha o F9. **O meio (desfazer) bate na pétrea**: o `.reg` não reaproveita slot, desfazer deixaria buracos. O choque **aparece** e se resolve sem o dono, porque a pétrea deixa um caminho: **pré-conferir o grupo antes da marca** (como o `COMMIT` já faz, 448/567/574) e, depois dela, andar só para a frente |
| 7 | **Log antes do dado** (regra do WAL) | sim | sim | sim no modo WAL | 10 × 0 | **dentro de bilhete, já é nosso** (a marca é o write-ahead). **Na escrita solta não**, e é o 498 H3 que o dono pôs ⏸: inverter a ordem `R → L` exigiria a imagem sempre ligada para refazer o `.reg` do diário — custo, não pétrea —, e a decisão de escopo é dele. Não reabro |
| 8 | **Página rasgada** | `full_page_writes` | doublewrite | o journal guarda a página inteira | 10 × 0 | **já é nosso** por outro meio: CRC por slot e por página + espelho do `.reg` + página selada do `.ndx` (340) |
| 9 | **Posição da réplica gravada atômica com o dado** | LSN de replay no próprio redo | tabela de posição no InnoDB, na mesma transação (réplica *crash-safe*) | — | 9 × 0 | **já é nosso** (a posição é o `.log` da réplica) |

**O que dos maduros não serve aqui, e por qual pétrea:**

- **Undo e rollback** (InnoDB undo, SQLite rollback journal, o *abort* da réplica lógica do PG):
  desfazer devolve slot gravado, e o `.reg` não reaproveita slot. Por isso a recuperação anda
  **só para a frente** e o desenho põe a recusa **antes** do ponto de compromisso.
- **Reuso de espaço** (`VACUUM`, *purge*, *free list*): ordem de digitação. O expurgo do 706 tira
  **volume de diário inteiro**, nunca slot.
- **Truncar o log no *checkpoint* sem perguntar ao consumidor**: o 706 já decidiu segurar pelo
  consumidor declarado (8 × 2).
- **GTID / LSN global**: recusado no `REPLICACAO.md` §4 e no 299; o `tx` por tomada com a posição
  por tabela compra a atomicidade sem a posição global.
- **Visibilidade por MVCC** (o *clog* do PG): a Sombra está parada pelo dono; a atomicidade para o
  leitor vem da trava única, como hoje.

---

## 5. O plano, em etapas pequenas

Cada etapa cita **funções**, não linhas (o `servidor.rs` vai ser dividido antes). Cada prova
tem de **cair com o defeito reposto** e passar com o conserto; a guarda nova vai ao catálogo
(`bancada/guardas/catalogo.py`) com o defeito que a motivou.

| Etapa | O que | Prova (cai com o defeito reposto) | Formato |
|---|---|---|---|
| **E0** (F, antes de qualquer conserto) | Provar ou matar cada furo lido, contra o SO (`SIGKILL`, ganchos `PHXSQL_TESTE_PARAR_*` que já existem) | F1: `COMMIT` X=«a» com a janela parada, solta X=«b», `SIGKILL`, reabre → espera-se «b» (previsto hoje: «a»); F1-bidi: o mesmo com o par, os dois lados terminam em «b». F2: gancho no `InserirDepoisDoContador` dentro de `aplicar_conjunto` → o diário tem a inclusão. F3: marca de passada terminada completada → `eventos()` igual antes e depois. F4: relógio recuado por injeção (o do 684) + marca retida → id novo maior. F6: devido no meio da passada → um `tx` só na transação. F8: backup com marca pendente → depois de restaurar, nenhum `.tx` no destino. F9: imagem com `rowstamp` divergente no 3.º de 7 eventos → nada aplicado | não |
| **E1** | **Um gerador de id de marca** (`proximo_id_no_diretorio` generalizado a todas as pastas e prefixos) semeando o contador de `Transacoes` no arranque; **ordem numérica** em `marcas_com_prefixo` | F4 e F12 | não |
| **E2** | **Formato: marca v7/v8** (`tx` no cabeçalho; `posicao` e `versao_antes` por operação) e **`tx` do evento devido** nos bytes 112..120 do cabeçalho v4 do `.log` (ausência benigna, como o 96..104 do 684; não colide com o 104..112 que o 706 F1 reserva). `FORMATO.md` §4 e §16 no mesmo commit | leitura das v1–v6 inalterada (testes que já existem); volta completa da v7/v8, em claro e cifrada; leitor anterior → `NaoConfere` (escrito, «só para frente») | **sim** |
| **E3** | `log::reservar_tx_na_unidade` chamado sob a trava ao gravar a marca do `COMMIT` e da cascata solta; o embutido abre unidade própria em volta da cascata | F7 (um `tx` só); `debug_assert` «uma operação = um evento» na passada | não |
| **E4** | **A regra de duas faces** (§3.3) no `aplicar_na_recuperacao` para v7/v8; aposentar `id_da_metade_que_entrou` (o `tx` vem da marca); o devido completa com o `tx` dele | F1, F2, F3, F6; os testes do 699/701/702 continuam verdes | não |
| **E5** | **Escalonador único**: `Database::recuperar_marcas` recebe o completador do bidi; todas as famílias por id numérico; `Servidor::novo` deixa de chamar `completar_marcas_do_bidi` à parte | F1-bidi e F5 (`COMMIT` 10 + grupo do bidi 11 na mesma linha, `SIGKILL`) | não |
| **E6** | **Restauração e semeadura chamam o mesmo passe no palco**, antes do `rename`: marcas (bidi em «só conferir»), depois índice | F8; a semeadura nasce com a posição igual à da origem (o que o F3 quebraria) | não |
| **E7** | **Pré-conferência do grupo da réplica e do bidi** antes da marca, sob a mesma tomada (continuidade, identidade pelo `rowstamp`, aridade, unicidade, o vencedor do «mais recente vence»); erro de dado **depois** da marca deixa de apagá-la e vira queda + completar, como o `COMMIT` | F9; **medir** o custo na bancada da réplica (eventos/s, faixa min–max) antes de aceitar | não |
| **E8** | O 706 na ordem do parecer dele (F2 → F1 → F3 → F4 → retrato com posição → expurgo ligado). Com o I7 a posição gravada no bilhete continua valendo depois do expurgo | as 8 provas do 706 §5 | sim (já pareceado) |
| **E9** | Marca pendente atravessando troca de esquema: a fase A drena a janela antes do retrato | F11 | não |

**Ordem e por quê:** E0 primeiro, porque furo lido é hipótese. E1 é pequena e destrava a ordem.
E2 entra **antes** do selo da 0.19.0 porque é formato. E3 e E4 dependem dela. E5 só faz sentido
com o F1 fechado (antes dele, ordenar só muda qual reaplicação estraga). E6 depende do E5. E7 é
independente e pode correr em paralelo depois da E0. E8 é o 706 e espera a E6 (a semeadura).

**Quem:** B escreve; F faz a E0 e confere cada RED; G põe as guardas; C revisa a E2 e a E4
antes do integrador; J confere a linha 6 da §4 antes da E7.

---

## 6. O que sobe ao dono

**Nada.**

- **Choque com pétrea:** dois choques aparecem (§4, linhas 6 e 7) e nenhum vai à mesa. O da linha 6
  tem caminho dentro da pétrea (pré-conferir e andar para a frente). O da linha 7 é o 498 H3, que o
  dono já decidiu deixar ⏸ em 30/09/2026; este desenho não o reabre.
- **Empate real:** nenhum. Todas as linhas da §4 dão 9 × 0 ou 10 × 0.
- **Produto:** o único número de produto deste conjunto (os 30 dias do caixa com o central fora)
  o dono já deu no 706.

O que se decide aqui é do papel C: a marca v7/v8 e o campo 112..120 do `.log` entram agora, antes
do selo, porque mudança de formato entra cedo, e ainda não há dado em produção.

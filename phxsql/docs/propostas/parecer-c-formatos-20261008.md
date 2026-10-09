# Parecer do papel C (DBA) — formatos em disco de 08/10/2026

Escopo: marca v7/v8 (709–716), `.log` 104..112 (706) e 112..120 (715), diário
sem paginação em `t#NNN.log` com expurgo (706), retrato (706), ensaio do bidi
(722), completar marcas, e os campos novos do `dblink.json` §19 (572 T6d,
**ainda não comitados**: revisei a árvore de trabalho). Revisão só de leitura.
Nenhum código foi editado.

**Base factual que muda o peso da pergunta (2).** Nenhuma versão está selada
depois da 0.18.0 (`CHANGELOG.md`: a 0.19.0 está «NÃO SELADA», e 676/706/709/572
estão em «Não lançado»). A 0.18.0 recusa o `.log` v4 alto. Então todo «erra
calado» do `.log` e da marca só alcança **binários de desenvolvimento**. A
exceção é o `dblink.json` (item D1): a 0.18.0 já tinha DbLink MySQL/PostgreSQL.

## Quadro

| # | Achado | Garantia | Ação |
|---|---|---|---|
| D1 | `tls`/`tls_ca`/`pino_tls` (e o `chave_do_fio`, que é anterior) num cadastro **formato 1**: um binário anterior os ignora, liga **sem TLS** e, na primeira gravação dele, regrava o arquivo **sem os campos**. É o mesmo estrago que o §19 já recusou para os envelopes | segurança do fio, calado | **conserto** |
| M1 | Leitor da marca: versão **maior que a conhecida** cai em `NaoConfere` e o arranque **apaga**. É a origem do «só para frente» da v5/v6 e da v7/v8, e vai se repetir na próxima versão se for selado assim | durabilidade | **conserto antes de selar** |
| M2 | v8 (e v6): o `tx` do cabeçalho, o `n_operacoes` e o `carimbo` **não** entram no dado associado. Só o CRC os cobre. Quem edita o arquivo baixa o `n` ou troca o `tx`, refaz o CRC, e a recuperação aplica **meia transação** ou junta eventos de transações diferentes. A FORMATO afirma que o `carimbo` é AAD, e isso é falso | atomicidade sob adulteração | **conserto agora** (v6/v8 nasceram hoje, sem custo de migração) |
| M3 | Doc velha: FORMATO §16 «Marca escrita ANTES…», item 1, e o «atravessa nas duas direções». O doc de `marca::VERSAO` ainda diz «sem cofre a marca continua v3». Desde o 709, sem cofre ela nasce **v7** | doc diz uma garantia que não vale | **conserto de doc** |
| L1 | 706, o caso calado já declarado (`t.log` sem paginação depois de virar em formato B, lido por binário anterior) | posição da réplica | **fechar**, com a doc corrigida |
| L2 | O cabeçalho `.log` v4 está **cheio** (0..120 usados, CRC em 120). O 684 e o 706 provaram que binário que conhece a v4 e não o campo **zera** os bytes ao regravar | evolução do formato | **regra**, sem código |
| L3 | FORMATO §4: «regrava no lugar o cabeçalho do volume 1» (marca 498) e «acima do 1». Desde o 706 é o **primeiro volume que existe** | doc | **conserto de doc** |
| R1 | Retrato: `trocar_pelo_retrato` apaga `.lgpd`, `.trash` e `.reason` da réplica, e o retrato não traz os da origem | trilha LGPD; lixeira e motivo numa réplica promovida | **conserto** |
| R2 | Retrato filtrado por `replica_alcanca`: a filha copiada com a mãe fora do alcance vira órfã **sem** contar. O 300 §2.7 conta as órfãs no caminho do `aplicar`, e o retrato pula esse caminho | visibilidade da regra primordial na réplica | **conserto pequeno** |
| O1 | Ordem de digitação nos caminhos novos | sagrada | **fechar** |
| O2 | Regra primordial nos caminhos novos | primordial | **fechar** |

## Detalhe e ação por item

**D1 — conserto.** `crates/phxsql-server/src/dblink/mod.rs`,
`Registro::gravar_estas`. Quando alguma ligação tem um campo de segurança do fio
(`tls` ≠ desligado, `tls_ca`, `pino_tls` ou `chave_do_fio`), o cadastro tem de
sair num formato que **todo** binário anterior recusa. A forma é a que o próprio
§19 já usa: a lista em `ligacoes` e um `formato` novo (3, ou 2 sem material). O
`Registro::abrir_com` aceita esse formato, e o `FORMATO_DO_CADASTRO` sobe junto.
Quem não pede TLS continua no formato 1, igual byte a byte: a guarda entra
pedida. O teste é o irmão de `CAMPOS_DO_BINARIO_ANTERIOR`: o `para_disco` do
anterior não pode **achar** a lista. Isso não sobe ao dono, porque o precedente
é decisão já escrita no §19: «falha fechada no lugar de destruição silenciosa».

**M1 — conserto, antes de selar a 0.19.0.** `crates/phxsql-store/src/marca.rs`,
`ler_marca`: o `_ => return Ok(Leitura::NaoConfere)` do `match versao` passa a
devolver uma resposta que **para e não apaga**. Pode ser a `SemChave` com o
motivo «versão N, mais nova que este binário», ou uma quarta variante. Versão
desconhecida não é «commit que nunca começou», é marca de binário mais novo. Os
três maduros convergem em não descartar o que não entendem (o PG dá FATAL na
versão do controle, o InnoDB recusa o redo de formato novo). Aceite automático.
Sem isso, toda versão futura da marca repete o «não volte o binário», agora com
um binário selado do outro lado.

**M2 — conserto agora.** No `marca.rs`, o `aad_da_operacao` leva, **só nas
versões 6 e 8**, o `tx`, o `n_operacoes`, o `carimbo` e o índice `i`. O
`ler_marca` passa os mesmos valores. A v4 continua lida como está. As duas
versões nasceram hoje e nenhum binário selado as grava, então mudar o AAD não
custa migração. Depois de selar, custaria uma v9. Na v7, em claro, nada a fazer:
CRC não é selo. Corrija também a linha da FORMATO §16 v4 «tabela, op, rowid
alvo, id, carimbo … dado associado». Hoje o `carimbo` não entra.

**M3 e L3 — conserto de doc.** Arquivos: `docs/FORMATO.md` §16 «Marca escrita
ANTES continua legível», §4 «A marca do evento devido», e o doc de
`marca::VERSAO` no `marca.rs`.

**L1 — fechar.** Medido: o binário que erra calado é **um** de 676 a 705. Todos
eles são de desenvolvimento, de 07–08/10. O selado (0.18.0) recusa a v4 alto. E
mesmo para os de desenvolvimento o estrago é menor que o declarado: a réplica
fiel confere a continuidade (`posição-1`, pedido 295) e **para** a tabela. Só o
bidirecional, que não confere a continuidade (decisão nomeada), receberia
evento errado. Um byte de versão protegeria uma população que não existe fora
deste repositório. Ação: no `docs/FORMATO.md` §4, «A migração só vai para
frente», trocar «binário anterior ao 706» por «binário **não selado** entre o
676 e o 705». Acrescentar que a réplica fiel para pela continuidade e que só o
bidirecional fica calado.

**L2 — regra, para o próximo campo do `.log`.** Não sobra «ausência benigna» no
cabeçalho de 128 bytes. O próximo campo é **versão nova**, com cabeçalho maior.
E o `cofre::gravar_cabecalho` monta o cabeçalho do zero, a partir da struct.
Antes de selar, vale fazê-lo **preservar os bytes reservados que não entende**,
com ler-modificar-gravar. Essa é a causa comum do 684 e do 706, e a regra
deixaria de depender de cada pedido lembrar dela. Fica como pedido
`⏸`, porque não é defeito ativo.

**R1 — conserto.** `crates/phxsql-store/src/catalogo.rs`:
- `Database::trocar_pelo_retrato` **não** apaga o `.lgpd`. A trilha registra
  quem leu dado pessoal **neste** servidor, e o retrato não a substitui. Apagar
  é destruir prova sem ninguém pedir.
- `EXTENSOES_DO_RETRATO` ganha `trash` e `reason`. Com a cópia fiel, a réplica
  refeita, e o cluster a promove automaticamente, continua tendo a lixeira e os
  motivos obrigatórios da origem. Hoje sai uma primária que não restaura nem
  explica nada anterior ao retrato.
- O `.fts`, o `.pag` e o `.bkp` podem continuar saindo: os três se regeneram na
  abertura (`table.rs` refaz o `.fts`, o `espelhar` semeia o `.bkp`, e o `.pag`
  não é fonte de verdade).

**R2 — conserto pequeno.** No `servico_diario_01.rs`, `refazer_por_retrato`,
depois do `trocar_pelo_retrato`, há duas saídas: contar as órfãs das tabelas
recebidas pela mesma conferência do 300 §2.7 (`Table::contar_orfas`) e anotá-las
no estado, ou recusar o retrato quando uma filha recebida tem a mãe fora do
alcance. A primeira é a que os maduros fazem (contar e expor). Não sobe ao dono.

**O1 — fechar.** Ordem de digitação, conferida caminho por caminho:
- **retrato:** é cópia fiel do `.reg`, com os buracos e os rowids da origem.
  Não reusa nada.
- **ensaio do bidi:** só lê. A inclusão prevista cai em `slots() + nascidas`,
  no fim.
- **completar marcas:** a inclusão só entra no rowid da marca. Saiu outro, é
  `Corrompido`. Slot livre na faixa recusa (`slot_ja_consumido`) ou dá «já
  passou» (`a_linha_ja_passou`), e **nunca** se preenche. Os
  `completar_o_diario_da_*` gravam só evento e não tocam no `.reg`.
- **versão do slot:** sobrevive à reescrita do `acrescentar_coluna` (o
  `reg.rs` copia os bytes 8..16), e por isso o bilhete v7/v8 continua valendo
  depois de uma migração.
- **formato B:** o número nunca se reusa (`maior fechado + 1`, e o último
  fechado nunca sai).

**O2 — fechar.** A recuperação reaplica com `*_com_maes`, a regra primordial
com o pai em progresso. A réplica e o bidi aplicam sem julgar e contam as
órfãs, como decidido no 300 §2.7, e o ensaio 722 não muda isso. A única lacuna
é a R2.

## O que sobe ao dono

Nada. D1 e M1 têm precedente escrito (§19) ou convergência dos maduros. Os
outros são conserto sem choque com pétrea.

## Migração, se os consertos entrarem

- **D1:** só para frente, e **só** para quem pediu TLS. Voltar o binário falha
  alto, sem apagar nada.
- **M2:** sem migração, se entrar antes de selar. Marca v6/v8 de pé gravada
  com o AAD antigo deixa de abrir e cai em «não confere». Em desenvolvimento,
  suba uma vez com o binário atual para esvaziar as marcas antes de trocar.
- **R1, R2, M1, M3, L3:** sem mudança de formato em disco.

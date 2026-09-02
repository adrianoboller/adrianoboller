# O roteiro até 1.0, e depois

As **55 sprints** que o dono definiu, na ordem em que ele as definiu. Este
documento é o roteiro **de registro**: as propostas tiradas de manuais de
concorrentes continuam em `docs/SPRINTS.md`, e o `docs/PLANO.md` continua sendo
o documento histórico da leitura do rusqlite e do contrato do FraseSQL.

> **Caminho crítico imediato: SP000001 até SP000016.** Sem contrato, build,
> transações, FK, durabilidade, concorrência, RID, armazenamento e MVCC, não
> faz sentido avançar para SQL ampliado ou recursos cognitivos.

**Toda mensagem de erro diz de qual sprint se trata**, por decisão do dono
(«prefixo em todas, e conserto os clientes»), no molde do MySQL(R) — o
identificador primeiro, entre colchetes, e depois a frase:
`[SP000008] integridade referencial: …`. A tabela mora no `sprint()` do
`crates/phxsql-core/src/error.rs`, com o motivo escrito em cada braço, e a
regra que a escolheu não é «de que área parece» e sim **qual sprint mudaria
este comportamento**. Há teste que lê este arquivo e reprova sprint citada que
não exista aqui — *número citado é número que não se mede*. Detalhes em
[MENSAGENS.md](MENSAGENS.md).

---

## Antes da lista: três decisões que o roteiro exige do dono

Não são objeções ao roteiro. São pontos em que ele **encosta em regra pétrea
desta casa**, e a própria regra manda discutir antes.

### 1. A SP000014 pede o que o `.reg` não faz — e não faz de propósito

> «**A ordem de digitação é sagrada.** O `.reg` nunca reaproveita slot
> excluído. Qualquer proposta que quebre isso precisa ser discutida antes.»
> — `CLAUDE.md`

A SP000014 é «Reuso de espaço, VACUUM e compactação». Reusar slot **é**
reaproveitar slot excluído. E compactar reescreve `rowid`, que aqui **é
endereço**: quem guardou um passa a apontar para outra linha.

Isso já está registrado como **consequência aceita**, não esquecimento
(`docs/COMPARACAO.md`, sobre o `OPTIMIZE TABLE`): uma tabela com muitas
exclusões cresce e não encolhe.

**A decisão é sua**, e ela tem um preço em cada direção. Manter a regra: o
espaço não volta. Relaxá-la: a garantia que o TopSpeed(R) não dava deixa de
existir aqui também.

### 2. A SP000013 é a chave que destrava a SP000016 — e isso é boa notícia

O MVCC está registrado nesta casa como **recusa medida**:

> «**MVCC** — Quebraria o rowid-como-endereço e a replicação por rowid.
> **Decisão pendente do dono**» — `base-de-conhecimento/05-TECNOLOGIAS.md`

A SP000013 é «**RID lógico estável** e formato físico v2». Um RID lógico é
exatamente o que separa «identidade da linha» de «posição da linha no arquivo»
— e é a peça que faz a objeção ao MVCC cair.

Ou seja: **o roteiro já resolve a objeção que esta casa tinha registrado**, e a
ordem SP000013 → SP000016 está certa. O que ele não diz é o custo: RID lógico
é **mudança de formato em disco**, e a regra é que mudança de formato entra
**cedo**, enquanto não há dado em produção.

### 3. A SP000024 troca a cifra do fio por TLS de verdade

Hoje o transporte usa cifra própria estilo Noise (`docs/CIFRA-DO-FIO.md`), e a
recusa de escrever TLS à mão está registrada com motivo: *«TLS mal escrito é
pior que TLS ausente, porque parece seguro»*.

A SP000024 pede TLS 1.3 e mTLS. Isso significa ou **escrever TLS** — o que a
recusa desaconselha — ou **admitir dependência externa**, o que quebra a regra
de zero dependências. Há um terceiro caminho: **proxy obrigatório** (a própria
auditoria externa o lista como alternativa aceitável). Os três têm preço, e
nenhum é gratuito.

---

## PhxSql 0.19 — Fundação

| # | Sprint | Estado medido |
|---|---|---|
| SP000001 | Contrato PhxSql 1.0 e congelamento do escopo | **não iniciado** |
| SP000002 | Build limpo, toolchain fixo e CI obrigatória | **feito, e parado** — `rust-toolchain.toml` pina 1.94.1 (conferido: é o `rustc` que roda aqui, commit `e408947bf`) e `.github/workflows/portoes.yml` roda os três **sem cano**. Nunca correu uma vez: depende do `push`, e o `push` é 403 |
| SP000003 | Proveniência Git, versão e release reproduzível | **feito** — `build.rs` embute commit e árvore suja, `--version` uniforme nos três binários |
| SP000004 | Fonte única de verdade e documentação gerada | **feito** — `CAPABILITIES.json` e os geradores escrevendo README, `TESTES.md` e `REST.md` |
| SP000005 | Decomposição de `servidor.rs` e fronteiras arquiteturais | **não iniciado** — **22.560** linhas (remedido; eram 22.396, e a conversão das telas não passa por aqui — o crescimento é das frentes de servidor) |

## PhxSql 0.20 — Transações e integridade

| # | Sprint | Estado medido |
|---|---|---|
| SP000006 | Visão transacional única e read-your-own-writes | **não iniciado, e agora MEDIDO** — `bancada/transacoes/visibilidade.py`, por soquete: dentro da transação a própria escrita **não** aparece (1→1), o commit aplica (→2) e o rollback descarta (2→2). Os dois últimos são o que separa *modelo de empilhamento coerente* de *transação com defeito*: o empilhamento entrega o **A** do ACID, falta o **I**. E falta por construção — a escrita fica fora da tabela até o commit e a leitura vai na tabela —, então o conserto é no caminho de **leitura**, consultando a pilha pendente |
| SP000007 | Definição e validação estrutural de chaves estrangeiras | **parcial, e andou** — além de declarar (`criar_tabela`, editor ER), a declaração agora **valida**: `ao_excluir` aceita **só** `restringir` e a recusa acontece na criação da tabela, não na gravação (`valores.rs`, com o par de testes que trava os dois sentidos); e a chave conferida **exige índice dos dois lados**, recusando e dizendo qual falta em vez de esconder uma varredura dentro de um `excluir`. Esquema `PSCH` v7 |
| SP000008 | Execução completa de FK e ações referenciais | **parcial** — deixou de ser «não iniciado». Medido: `conferir_fks` roda em **2** pontos de escrita (`inserir` e `atualizar`, `table.rs:1203` e `:1416`) e `conferir_filhas` em **2** de exclusão (de vez e **suave**, `:1574` e `:1634`) — o suave também porque pai logicamente morto deixa filha apontando para linha que a tela não mostra mais. NULL satisfaz (MATCH SIMPLE). **Duas metades faltam, e são nomeadas**: (1) a imposição é **pedida**, pelo interruptor `verificar` da chave — quem não pede continua como antes, e o teste do comportamento velho (`a_chave_e_declarada_mas_ainda_nao_e_imposta_na_gravacao`) segue verde de propósito; (2) `ao_alterar: cascata` é **declarado e nunca lido** — nada cascateia quando a chave da mãe muda. Medido: o campo só aparece em `schema.rs` (guardar/serializar) e no `phxsql-cli` (mostrar), em ponto nenhum de escrita. É a mesma frase do pedido 127 com o alvo deslocado: *declarar não é aplicar* virou *aplicar no excluir não é aplicar no alterar*. `crates/phxsql-store/tests/chave-estrangeira.rs` |
| SP000009 | FK em todos os caminhos e verificador de consistência | **não iniciado** |
| SP000010 | Protocolo de commit e matriz real de durabilidade | **parcial** — durabilidade configurável em três regimes; falta a matriz de falhas |

## PhxSql 0.21 — Concorrência e armazenamento

| # | Sprint | Estado medido |
|---|---|---|
| SP000011 | Remoção do `Mutex<Instancia>` global | **parcial** — a trava virou **ponto único** (catraca `so_um_lugar_toma_a_trava`), mas continua **global** |
| SP000012 | Deadlock, cancelamento e governança de recursos | **parcial** — há prazo e recusa de reentrância |
| SP000013 | RID lógico estável e formato físico v2 | **não iniciado, e REBAIXADO a melhoria** — deixou de ser pré-requisito do MVCC. A premissa foi medida contra um MySQL(R) 8.0.46 de verdade (`bancada/mvcc/premissa-innodb.sh`): o que ancora uma cadeia de versões é a **identidade estável da linha** mais um ponteiro para a versão anterior, e não um identificador novo. O `rowid` daqui já é isso, por construção — o `.reg` nunca reaproveita slot. *A pétrea que parecia atrapalhar é o que torna o MVCC mais fácil aqui* |
| SP000014 | Reuso de espaço, VACUUM e compactação | **RECUSADO POR DECISÃO DO DONO**, e não «hoje» — a pétrea vence: «a ordem de digitação é sagrada, o `.reg` nunca reaproveita slot excluído». O que ela compra é o que se perderia: `rowid` como identidade estável para sempre, que é o que a replicação usa para identificar linha e o que o `.trash`/`.reason` apontam. Espaço de linha excluída volta só em reconstrução explícita. **Não é pendência: é escopo fora**, e a 0.21 fecha sem ela |
| SP000015 | Buffer pool, checkpoints e group commit | **parcial** — cache de páginas (2,40×) e group commit (2,63×) medidos |
| SP000016 | MVCC e níveis de isolamento | **DESBLOQUEADO pela medição** — não depende mais da SP000013. Medido no oráculo: leitura aberta antes da escrita alheia continua vendo a versão velha sem bloquear (100 → 100 → 200, o 200 só após o próprio commit), as versões velhas acumulam enquanto a leitura está aberta (*history list* 7 → 207) e são recolhidas quando ela fecha (26 s → 0, demonstrado uma vez; uma segunda tentativa ficou inconclusiva e isso fica dito). O caminho aqui é cadeia de versões ancorada no `rowid` + área de undo, no molde do InnoDB |

## PhxSql 0.22 — SQL relacional

| # | Sprint |
|---|---|
| SP000017 | Contrato SQL, lexer, parser e AST |
| SP000018 | Binder, catálogo, tipos e coerção |
| SP000019 | Expressões, NULL e funções |
| SP000020 | DDL SQL e catálogo transacional |
| SP000021 | DML SQL, prepared statements e transações SQL |
| SP000022 | Operadores relacionais avançados |
| SP000023 | Estatísticas, otimizador, `EXPLAIN` e conformidade SQL |

Hoje o `phxsql-sql` é o começo da camada: `SELECT` simples traduzido para as
operações que já existem no protocolo.

## PhxSql 0.23 — Segurança

| # | Sprint |
|---|---|
| SP000024 | TLS 1.3 e mTLS em todos os canais — *ver decisão 3 acima* |
| SP000025 | Secure-by-default, identidade e autorização |
| SP000026 | Criptografia em repouso, gestão de chaves e auditoria inviolável |
| SP000027 | Fuzzing, revisão criptográfica e correção de segurança |

A criptografia em repouso **já existe** por coluna marcada (0,10 µs/linha,
escolha (c) do dono entre quatro medidas); o que a SP000026 acrescenta é gestão
de chaves e auditoria inviolável.

## PhxSql 0.24 — Alta disponibilidade e ecossistema

| # | Sprint |
|---|---|
| SP000028 | Replicação de produção em WAN e entre versões |
| SP000029 | Consenso, quorum commit e cluster sem split-brain |
| SP000030 | ODBC completo e drivers oficiais |
| SP000031 | Portabilidade real: Windows, Linux, macOS, ARM, Android e iOS |
| SP000032 | Backup incremental, PITR e upgrades N/N-1 |
| SP000033 | Observabilidade, capacidade e benchmark certificado |

A SP000030 tem uma dívida já nomeada: o driver responde `SQL_TC_NONE` —
«sem transações» — e o servidor **tem** transações desde a frente 37, o que faz
disso lacuna do driver e não limite do motor (`docs/ODBC.md`).

## PhxSql 0.99 e 1.0

| # | Sprint |
|---|---|
| SP000034 | Pilotos reais, UX, documentação e congelamento funcional |
| SP000035 | Auditoria independente e liberação PhxSql 1.0.0 GA |

## PhxSql 2.x — Banco declarativo e adaptativo

| # | Sprint |
|---|---|
| SP000036 | Esquema semântico, LGPD, retenção, residência e acesso |
| SP000037 | Linguagem Phx Contract |
| SP000038 | Compilador de garantias |
| SP000039 | Nó local-first |
| SP000040 | Conflito por tipo de dado |
| SP000041 | Consistência adaptativa |
| SP000042 | Branch, diff, merge e time travel |
| SP000043 | Reactive SQL |
| SP000044 | PhxSql Guardian |
| SP000045 | IA governada |

## PhxSql 2.x — Cognitive Lab

| # | Sprint |
|---|---|
| SP000046 | Arquitetura do Cognitive Lab |
| SP000047 | Telemetria e Evidence Store |
| SP000048 | Anomalias e causa-raiz |
| SP000049 | Soluções candidatas |
| SP000050 | Cognitive Sandbox Copy-on-Write |
| SP000051 | Captura e replay de carga |
| SP000052 | Benchmark comparativo reproduzível |
| SP000053 | Guarantee Gate |
| SP000054 | Cognitive Advisor |
| SP000055 | Change Manager e rollback |

---

## Como este documento não envelhece

A coluna «estado medido» é a que mente primeiro. Ela sai de conferência contra
o código, e cada linha diz **onde** conferir. Quando um sprint fechar, o estado
muda aqui **e** no `docs/PENDENCIAS.md`, que continua sendo a fonte da página
dos pedidos.

E vale a regra que esta casa já pagou duas vezes: *a lista do que falta também
é palpite até alguém medir*. O item da trava ficou dizendo «12 tomadas fora do
ponto único» depois de a frente 29 tê-las zerado — com a receita da medição
escrita ao lado, por refazer.

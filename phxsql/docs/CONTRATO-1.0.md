# Contrato PhxSql 1.0 — o que se promete, o que não se promete, e o que falta decidir

> **Estado: PROPOSTA.** Este documento põe o escopo na mesa decidível. **A
> palavra final é do dono** — as decisões estão marcadas **P** e não foram
> tomadas aqui. Sprint [SP000001](ROTEIRO-1.0.md), «Contrato PhxSql 1.0 e
> congelamento do escopo».

Ele existe porque é o primeiro item do roteiro e porque **sem ele nenhuma das
outras 54 sprints sabe quando parou**. Uma sprint sem linha de chegada fecha
quando alguém se cansa dela.

E ele não é o `README` (que diz como usar), nem o `MANUAL` (que diz o que faz),
nem o `PENDENCIAS.md` (que diz o que falta). É a **promessa**: o que um cliente
escrito hoje pode presumir amanhã.

---

## 0. O crivo, e como ler este documento

Toda linha deste documento é **uma** das três coisas:

| marca | o que é | e o que ela exige |
|---|---|---|
| **G** | **garantia**, com a prova ao lado | arquivo, teste ou bancada que a sustenta — sem isso é folheto |
| **N** | **não-garantia declarada** | dita sem rodeio, com o motivo, para ninguém descobrir sozinho |
| **P** | **pergunta ao dono** | uma decisão que este papel **não** toma |

Se uma frase não for nenhuma das três, ela não pertence a um contrato — e a
regra vale contra este próprio arquivo.

**E os números.** *Número citado é número que não se mede.* Cada número aqui sai
de um documento gerado, de um gerador que se roda, ou de uma medição registrada
com o programa que a refaz. A procedência de cada um está na
[§6](#6-de-onde-sai-cada-número), e nenhum foi digitado de memória.

**O retrato de onde este contrato foi escrito:** `CAPABILITIES.json`, gerado por
`docs/dossie/numeros-do-projeto.py`, medido em **2026-09-03 15:07:55**, commit
`d1e84da`, versão **0.18.0**, árvore suja. Regerar o arquivo é o jeito de
atualizar este parágrafo — reescrevê-lo à mão é o jeito de fazê-lo envelhecer.

---

## 1. O que o PhxSql GARANTE hoje — cada garantia com a prova

### 1.1 O dado gravado

| | garantia | prova |
|---|---|---|
| **G** | **A ordem de digitação.** O `.reg` anexa sempre no fim; excluir marca o slot como livre e o slot **nunca é reaproveitado**. Percorrer o `.reg` do início ao fim devolve os registros na ordem em que foram digitados, e com paginação a garantia continua valendo — o volume N+1 vem sempre depois do N | `FORMATO.md` §1, «Ordem de digitação» |
| **G** | **O `rowid` é identidade estável e para sempre.** É o que a replicação usa para identificar linha, e o que o `.trash`, o `.reason` e o `.lgpd` apontam | `FORMATO.md` §1 e §5–§7; `REPLICACAO.md` §5 |
| **G** | **Escrita rasgada é detectável, nunca silenciosa.** Todo slot leva CRC-32 (IEEE, refletido, `0xEDB88320`); o leitor **recusa** o slot em vez de devolver metade velha e metade nova | `FORMATO.md` §1; `TRANSACOES.md` §11.2 |
| **G** | **O espelho `.bkp` repara**, quando ligado: mesmo slot, mesmo offset, mesmo instante; lido só quando o principal falha, e o `reparar` varre nos dois sentidos | `FORMATO.md`, cabeçalho |
| **G** | **Todo arquivo diz o que é e em que versão.** Assinatura + versão + CRC-32 de cabeçalho, nos nove arquivos | `FORMATO.md`, tabela do cabeçalho |
| **G** | **Índice B+tree com chave composta, ASC/DESC/NOCASE/único**, e `reindex` que recria o `.ndx` do zero a partir do `.reg` | `FORMATO.md` §2 e §12 |
| **G** | **O diário `.log` registra toda inclusão, alteração e exclusão**, com data, hora, usuário e — atrás do interruptor — a imagem da linha | `FORMATO.md` §4 |
| **G** | **A lixeira guarda a linha inteira e o motivo** (`.trash` + `.reason`), e os dois são arquivos **só de administrador**, junto com o `.lgpd` | `FORMATO.md` §5, §6 e §7 |

### 1.2 Integridade referencial

A regra primordial: **nunca se mata o pai que tem filhos.**

| | garantia | prova |
|---|---|---|
| **G** | `ao_excluir` aceita **só** `restringir`. Cascata, anular e nada não existem no lado do excluir, e o par Cascata/Cascata some por consequência | `valores.rs`, com o par `ao_excluir_so_aceita_restringir` e o irmão que impede um portão que recusaria tudo |
| **G** | **Chave declarada nasce conferida.** Quem quer declarar sem conferir manda `"verificar": false` — escolha escrita, não omissão | `a_chave_declarada_nasce_conferida` e `quem_pede_para_nao_conferir_continua_podendo` |
| **G** | **A regra é imposta na gravação, não só na declaração.** `conferir_fks` roda em 3 pontos de escrita (`inserir`, `atualizar`, `restaurar`) e `conferir_filhas` em 2 de exclusão — **de vez e suave** | `INTEGRIDADE.md` §1, com arquivo e linha; o levantamento saiu do código, não de lista |
| **G** | **«Existir» não basta: a mãe tem de estar VIVA.** Mãe excluída de forma suave continua no `.reg` com a chave no índice, e a filha nascia apontando para ela | `INTEGRIDADE.md` §2.1; custo medido: **+7,03 µs/linha (+11,2%)** na chave conferida, **zero** em quem não pediu |
| **G** | **NULO satisfaz** (MATCH SIMPLE), na gravação e no verificador — conferir num e não no outro faria o verificador acusar linha que o motor aceita | `INTEGRIDADE.md` §5 |
| **G** | **`ao_alterar: cascata` executa**, alcança a neta, e a **árvore inteira** é conferida antes da primeira escrita. Teto de 16 níveis | `Table::conferir_a_arvore`; `INTEGRIDADE.md` §7.1 |
| **G** | **A chave conferida exige índice dos dois lados**, e sem um deles o motor **recusa dizendo qual falta** — em vez de esconder uma varredura dentro de um `excluir` que parece barato | `INTEGRIDADE.md` §5 |
| **G** | **Declarar conferida sobre tabela que já tem órfã é recusado**, nomeando a linha: promessa falsa é pior que ausência dela | `table.rs:549`, `redeclarar_chaves_estrangeiras` |
| **G** | **O verificador RELATA e não conserta**, e diz onde está: tabela, chave, rowid e valor | `phxsql-store/src/integridade.rs`, `--example conferir-integridade` |

### 1.3 Transação

| | garantia | prova |
|---|---|---|
| **G** | `BEGIN` / `COMMIT` / `ROLLBACK` / `SAVEPOINT`, pelo protocolo e pelo SQL | `TRANSACOES.md` §14; `bancada/transacoes/provar.py` |
| **G** | **Nada vai a disco antes do `COMMIT`.** O conjunto de escrita fica em RAM, e por isso o `ROLLBACK` não deixa slot, rowid nem evento — a ordem de digitação sai intacta | `TRANSACOES.md` §3.1 |
| **G** | **A transação vê as próprias escritas** (*read-your-own-writes*), num lugar só: a `Sobreposicao` presa ao handle, aplicada no `ler`, no `varrer`, nas cinco paginações, no `contar`, no `filtrar` e no `buscar` | medido por soquete: 1→**2**→2→3→2 (`bancada/transacoes/visibilidade.py`); `TRANSACOES.md` §4.4.1 |
| **G** | **O nível é dito sem enfeite, e é o que o servidor devolve** em `transaction_isolation`: *escrita serializável por tabela, leitura confirmada e não bloqueante, sem leitura repetível* | `TRANSACOES.md` §4.4 |
| **G** | **Leitor nunca espera escritor.** Não há dado não confirmado em lugar nenhum — ele ainda está em RAM | `TRANSACOES.md` §4.4 e §11.1 |
| **G** | **Quem não usa transação não paga.** O único acréscimo é um `AtomicUsize` lido com `load(Relaxed)` **antes** de qualquer trabalho | teste `sem_transacao_nada_muda`; `TRANSACOES.md` §7 |
| **G** | **DDL dentro de transação é RECUSADO, e não silenciosamente confirmado** | `TRANSACOES.md` §3.4 e §11.4 |

### 1.4 Durabilidade e queda do processo

| | garantia | prova |
|---|---|---|
| **G** | **A marca `transacao_<id>.tx` é o ponto de compromisso**, sincronizada antes da passada; uma queda depois dela é completada no arranque | `FORMATO.md` §16; `TRANSACOES.md` §5.1 |
| **G** | **A recuperação anda para a frente, nunca para trás** | `TRANSACOES.md` §5.2 |
| **G** | **A matriz cruza 5 pontos de morte × 3 regimes, por `SIGKILL` de processo real** — nunca por teste unitário | `bancada/durabilidade/prova.py`; `TRANSACOES.md` §5.7 |
| **G** | **Os quatro primeiros pontos têm a mesma garantia nos três regimes:** `gravar_marca` sincroniza sempre, incondicional ao regime, e uma queda de **processo** nunca perde um `write` que o kernel já recebeu. O regime só decide quanto tempo a marca fica pendurada — `por_operacao` 0/0/0, `por_lote` 1/1/0, `sistema` 1/1/1 | `TRANSACOES.md` §5.7 |
| **G** | **Cascata parcial não acontece em silêncio.** Quando o índice da filha está sujo, a recuperação **recusa** cascatear e denuncia em `operacoes IMPOSSIVEIS`: 21 corridas, 1.200 filhas, 9 casos, 9 denunciados, zero calados | `TRANSACOES.md` §5.5.3 |

### 1.5 Replicação e cluster

| | garantia | prova |
|---|---|---|
| **G** | **Replicação assíncrona funcionando, medida com quatro servidores:** master **28.914 linhas/s**, atraso de **1,3 a 2,1 s** com o laço em 2 s, e retrato **SHA-256 de cada linha** idêntico | `REPLICACAO.md` §13 e §14; `bancada/replicacao/` |
| **G** | **A réplica alcança:** 4.273 → **17.450 eventos/s** (4,08×) depois da marca de posição — mais do que o master escreve | `DESEMPENHO.md` §4.5 |
| **G** | **Quatro modos** (source→réplica, multi-master, spare, read replica), **agendamento por origem** e **cascata** (Master→Slave01→Slave03; 2º salto 1.827 ms contra 1.679 do 1º) | `REPLICACAO.md` §9, §11 e §13 |
| **G** | **A réplica APLICA, ela não JULGA.** A garantia de integridade é da origem, que a impôs ao aceitar a escrita; a da réplica é de **fidelidade**. Conferir duas vezes não soma as duas — troca a segunda pela primeira, e isso era **perda de dado**: `pedidos` ficava com **0 de 2** eventos em duas das três ordens | `INTEGRIDADE.md` §3, com a tabela das três ordens |
| **G** | **Uma transação revertida não chega aplicada na réplica** — porque não chega: a transação aberta não produz evento nenhum | `TRANSACOES.md` §6.2 |
| **G** | **O `.log` não mudou de versão para a transação existir.** Réplica de qualquer versão aplica sem saber que houve transação | `TRANSACOES.md` §6.1 |
| **G** | **Cluster com eleição e promoção automática**, provado com três servidores e um SMTP falso na bateria | `CLUSTER.md` §2; parte `cluster` do `provar.py` |
| **G** | **Escrita na réplica recebe `REDIRECIONA`** (HTTP 421 pelo REST), com o endereço do primário | `REPLICACAO.md` §10; `REST.md` §6 |

### 1.6 Segurança

| | garantia | prova |
|---|---|---|
| **G** | **Senha nunca em texto puro** — nem em arquivo, nem em log, nem em resposta do protocolo. PBKDF2-HMAC-SHA256, 210.000 voltas; login por desafio-resposta, em que a senha não sai da máquina | `SEGURANCA.md` §2; teste que falha se a ficha de usuário vazar o hash |
| **G** | **Criptografia conferida contra vetor oficial**, e não «parece certo»: SHA-256/HMAC/PBKDF2 (FIPS 180-4, RFC 2104, 2898, 4231), Ed25519 (RFC 8032), X25519 (RFC 7748), HKDF (RFC 5869), ChaCha20-Poly1305 (RFC 8439), Base64 (RFC 4648), UUID v7 (RFC 9562), SCRAM-SHA-256 (RFC 5802/7677) | `TECNOLOGIAS.md` §3, com o nome do teste de cada vetor |
| **G** | **O portão de permissão é UM só** — o `despachar` —, com direito até a tabela e conferência própria nas operações que não têm o campo `"tabela"` (`juntar`, `unir`, `pivotar`) | guardas `pivotar-sem-portao`, `posicao-sem-portao`, `sequencias-sem-portao`, `duplicar-sem-destino` |
| **G** | **Regra de tabela ausente não muda nada** para quem já configurou | teste `sem_regra_de_tabela_nada_muda`; guarda `regra-de-tabela-imposta` |
| **G** | **Cifra do fio na porta 5000**, aperto de mão estilo Noise (`Noise_NX_25519_ChaChaPoly_SHA256`), selando **inclusive o token de serviço** | `CIFRA-DO-FIO.md`; `SEGURANCA.md` §7 |
| **G** | **Cifra em repouso dos diários** (`.log`, `.trash`, `.reason` na versão 3) e **do dado por coluna marcada** (no `.reg`, `.memo`, `.bin` e `.bkp`), a **0,10 µs/linha**, com **duas** fechaduras — o AAD e o endereço dentro do nonce | `SEGURANCA.md` §8 e §11; guarda `endereco-fora-da-amarracao` |
| **G** | **O Profiler redige ANALISANDO, nunca recortando** — o que não se analisa não vira texto, vira o tamanho em bytes | `SEGURANCA.md` §10.1; guardas `profiler-recorta` e `profiler-recorta-largo` |
| **G** | **Blacklist com bloqueio automático, gancho de firewall e log de acessos por IP** em JSON Lines | `SEGURANCA.md` §3, §4 e §5 |
| **G** | **Restaurar backup confere SHA-256 antes de tocar o destino** | `RESTAURACAO.md`; guarda `backup-sem-sha256` |

### 1.7 As interfaces

| | garantia | prova |
|---|---|---|
| **G** | **121 operações no protocolo**, e o catálogo **é** o `despachar` — travado por teste, não por disciplina | teste `o_catalogo_e_o_despachar_sao_a_mesma_lista`; número gerado em `CAPABILITIES.json` |
| **G** | **A especificação OpenAPI sai da tabela de despacho**, com guarda nos **dois** lados do laço: operação sem rota e rota sem operação | `REST.md` §1, com o defeito reposto de cada uma |
| **G** | **No REST, o caminho manda sobre o corpo.** Um `"op"` no corpo diferente do caminho é **recusado**, não ignorado — senão um `POST /v1/ping` seria um `excluir` no servidor e continuaria um `ping` em tudo o que observa de fora | `REST.md` §6 |
| **G** | **As duas portas REST nascem DESLIGADAS** | `REST.md` §3 |
| **G** | **Driver ODBC 3.x de verdade**, `cdylib` de ABI C, provado por **73 conferências** pela ABI literal mais `isql` — e com o defeito reposto (truncamento calado no `SQLGetData`) a prova falha em 4 | `ODBC.md`; `bancada/odbc/provar.py` |
| **G** | **Servidor MCP** (`phxsqld --mcp`), com o `tools/list` lendo o catálogo em vez de uma segunda lista | `MCP.md`; `tests/mcp_stdio.rs` roda o binário de verdade |
| **G** | **Zero dependências externas.** Só a `std` — o que faz `cargo build --offline` funcionar e o que fez a compilação cruzada para Windows sair de primeira | `CAPABILITIES.json`: `dependencias_externas: 0`; `TECNOLOGIAS.md` §2 |

### 1.8 O que sustenta tudo isso: as provas, contadas

| | garantia | prova |
|---|---|---|
| **G** | **1.547 testes, 0 falhas** — somados dos `test result:` de uma rodada de verdade, por um gerador que **aborta se a suíte falhar** | `docs/dossie/numeros-do-projeto.py`; `TESTES.md` §1 |
| **G** | **Três famílias de prova, porque uma não cobre a outra:** motor (`cargo test`), tela (`testes-web/`) e soquete (`bancada/`) — *teste unitário não prova queda de conexão, e teste de motor não prova formulário* | `TESTES.md`, cabeçalho |
| **G** | **Um comando roda tudo**, com veredito por parte e o **motivo de cada pulo** no relatório: `python3 provar.py --construir` | `TESTES.md` §7 |
| **G** | **77 guardas — 73 provadas, 4 redundantes.** Cada defeito que esta casa já pagou é **reposto** numa cópia da árvore, e o teste que o motivou tem de cair | tabela gerada em `TESTES.md` §8 por `bancada/guardas/tabela-no-testes.py` |
| **G** | **Toolchain pinado e CI que correu de verdade:** `rust-toolchain.toml` fixa a versão, e o workflow `Portoes` teve **11 corridas** em 03/09 — *CI que nunca correu não é CI* | `PORTOES.md`; `ROTEIRO-1.0.md`, SP000002 |
| **G** | **Catracas que só descem:** `TETO_ROTULOS_E_CRASE` = 1.720, `TETO_COLADO` = 0, `TETO_FRASE_REPETIDA` = 0 | `conferidor.rs` |

---

## 2. O que o PhxSql NÃO garante

Esta seção é a que separa contrato de folheto. Nada aqui é esquecimento.

### 2.1 **Não é «ACID compliant»** — e a frase não se escreve

**N** — A folha de marca afirma *ACID compliant*. **É falso, e continua falso.**
A resposta precisa, letra por letra, está em `TRANSACOES.md` §12:

| letra | estado | com precisão |
|---|---|---|
| **A** | **entregue** | o conjunto de escrita é aplicado inteiro ou não é aplicado; o `ROLLBACK` não deixa slot, rowid nem evento |
| **I** | **entregue, com o nome certo** | escrita serializável por tabela, leitura confirmada e não bloqueante, **sem leitura repetível**. **Não é ANSI SERIALIZABLE** e não pode ser chamado assim |
| **C** | **PARCIAL** | tipo, unicidade, gatilhos e integridade referencial são conferidos. O que falta: **a cascata escreve em tabela que a transação não declarou**, então um `ROLLBACK` não alcança a filha. Enquanto isso valer, o **C** não está inteiro |
| **D** | **entregue, e configurável** | com `durabilidade: sistema` quem abre mão é quem configurou, e está escrito |

**N** — **A 1.0 não usará a expressão «ACID compliant» em documento técnico**, com
ou sem qualificação, e isso é regra da casa e não estilo. O que se pode escrever,
e é verdade: *atomicidade e durabilidade entregues, isolamento entregue no nível
declarado acima, consistência dependente do escopo da cascata.*

### 2.2 Onde a decisão foi **NÃO conferir** — escolhas, não buracos

`INTEGRIDADE.md` §3 e §4. Cada uma tem o motivo e, quando há, o custo medido.

| | não-garantia | motivo |
|---|---|---|
| **N** | **A réplica não confere chave estrangeira e não refaz a cascata** | a replicação anda **por tabela**, cada uma com a sua posição, e não existe ordem global entre tabelas. Conferir na réplica **causava a perda que existe para impedir**: `pedidos` com 0 de 2 eventos em duas das três ordens |
| **N** | **`copiar_tabela_para` deixa nascer órfã** | colar a filha num banco onde a mãe ainda não está é ordem legítima de trabalho, e a cópia é **byte a byte** — reinserir linha a linha para conferir perderia a ordem de digitação e os rowids. O custo está **em teste**, não suposto: `colar_a_filha_sem_a_mae_passa_e_o_verificador_acha_a_orfa`. As duas saídas: o motor recusa a próxima gravação, e o verificador acha a órfã |
| **N** | **Restaurar backup não confere integridade referencial** | um backup é o retrato de um database inteiro; recusar aqui trocaria «restaure e confira» por «não restaure», que é a pior das duas na hora em que se precisa de um backup |
| **N** | **`reparar` e `reindexar` não conferem** | consertam arquivo, não modelo; negar o reparo trocaria um arquivo consertado por um arquivo quebrado |
| **N** | **`acrescentar_coluna` e `duplicar_tabela` não conferem** | não precisam, e o motivo é estrutural em cada caso (`INTEGRIDADE.md` §4.1 e §4.2) |
| **N** | **Filha em outro schema não é vista** pelo `excluir_tabela`, pelo `renomear_tabela` nem pelo verificador — todos varrem **um diretório** | `INTEGRIDADE.md` §6 |
| **N** | **A exigência de índice dos dois lados é imposta na gravação, não na declaração** — dá para declarar a chave conferida sem os índices e só descobrir no primeiro `excluir` | `INTEGRIDADE.md` §6 |
| **N** | **A auto-referência passa em silêncio**, e os dois grandes recusam. Está registrado como **defeito**, não como decisão | `INTEGRIDADE.md` §7.4 |

### 2.3 Concorrência

| | não-garantia | número / prova |
|---|---|---|
| **N** | **A trava de dados é única e global.** Com 2 clientes e metade da máquina ociosa, o mesmo caminho entrega **1,99×** no `ping` (que não a toma) e **1,51–1,59×** no `varrer` — ela come ~20% do paralelismo na leitura e ~25% na escrita já com dois clientes | `DESEMPENHO.md` §14, **com controle** |
| **N** | **Uma leitura segura a trava 23× mais tempo que uma gravação** no padrão `por_lote`: 3.122 µs contra 137 µs | `CONCORRENCIA.md` §7.1 |
| **N** | **Mandar parar não para.** São **4 de 76** seções críticas com ponto de cancelamento; nas outras 72 o pedido de cancelamento não é atendido | `CONCORRENCIA.md` §7.2; `bancada/concorrencia/mapa-da-trava.py` |
| **N** | **Sem MVCC e sem leitura repetível.** Entre duas leituras da mesma transação, outra pode ter confirmado | `TRANSACOES.md` §11.1 |
| **N** | **Não há trava de arquivo nem de registro: um processo por diretório**, e **nada impede o segundo**. O caso fácil de acontecer é a CLI `phxsql` num diretório que o `phxsqld` está servindo | `FORMATO.md` §17, e conferido nesta rodada: uma varredura por `flock`, `LOCK_EX`, `libc::open`, `custom_flags` e `create_new(true)` nos oito crates devolve **dois** acertos, e nenhum é trava de instância — a criação de um volume novo (`volume.rs:258`) e a gravação atômica do `config.json` (`config.rs:1219`) |
| **N** | **Transação entre databases não existe** (*two-phase commit*), e a recusa é fundamentada | `TRANSACOES.md` §2.3 e §11.5 |

### 2.4 Replicação

| | não-garantia | número / prova |
|---|---|---|
| **N** | **Não é síncrona.** A réplica fica atrás: **1,3 a 2,1 s** com o laço em 2 s | `REPLICACAO.md` §15 |
| **N** | **Não substitui backup.** A réplica repete o `DELETE` errado, e repete rápido | `REPLICACAO.md` §15 |
| **N** | **A posição é por tabela, e não há ordem global entre tabelas.** *Consequência que este documento nomeia e que ainda **não foi medida**:* um `COMMIT` que escreve em duas tabelas produz eventos em dois `.log`, puxados por posições independentes — logo **a transação chega parcelada na réplica**, e há uma janela em que a réplica mostra metade dela. No source a atomicidade vale; entre servidores, não. A medição que fecharia isto: `bancada/transacoes/` escrevendo em duas tabelas num `COMMIT` e a réplica lida entre os dois lotes | deduzido de `INTEGRIDADE.md` §3 + `TRANSACOES.md` §6.2. **`REPLICACAO.md` §15 envelheceu neste ponto**: ele diz «não há transação, então não há ordem global a preservar» — e agora há transação |
| **N** | **O lote é buscado com a trava de dados na mão.** Medido: `varrer` esperou **30,7 s** numa réplica cortada em silêncio, e no bidirecional os dois lados se trancam por 30 s com a rede sã | `REPLICACAO.md` §13 e §17 |
| **N** | **`replicacao_estado` não conta nada durante um corte silencioso** — o monitoramento não distingue «nada a replicar» de «cego» | `REPLICACAO.md` §13 |
| **N** | **O pulso do cluster vai em claro**, mesmo com a cifra do fio ligada na replicação: cifrar metade do tráfego do cluster é pior que não cifrar nenhuma, porque parece protegido | `REPLICACAO.md` §13 |
| **N** | **O bidirecional só foi provado com dois servidores** | `REPLICACAO.md` §13 |
| **N** | **`replica.rs` não tem nenhum teste no `cargo test`.** O laço que faz a replicação andar é provado só por `bancada/replicacao/`, que precisa de quatro servidores e **não roda no portão** | `TESTES.md` §1 e §5.2 |

### 2.5 Segurança

| | não-garantia | motivo |
|---|---|---|
| **N** | **Não é TLS.** Não há certificado, cadeia, autoridade nem revogação. A confiança é o **pino**; sem pino é TOFU, e quem estiver no meio na primeira conexão vence para sempre | `SEGURANCA.md` §7 |
| **N** | **`cifra_fio.exigir` nasce DESLIGADA**, e com ela desligada a proteção vale contra **escuta passiva e nada mais**: o atacante ativo corta o `cifrar` e o cliente rebaixa para claro. Nasce desligada porque guarda nova entra pedida — ligá-la quebraria todo cliente que não fala o aperto, o driver ODBC inclusive | `SEGURANCA.md` §7 |
| **N** | **A interface web não tem cifra própria, e não pode ter.** O navegador fala TLS ou fala claro; um aperto em JavaScript seria teatro, porque o próprio script chega pelo canal que se quer proteger. As saídas honestas são proxy com TLS à frente, ou túnel | `SEGURANCA.md` §6 e §7 |
| **N** | **A cifra do fio não interopera** com outras implementações de Noise: os tijolos são de norma e conferidos, a composição não foi rodada contra os vetores do *cacophony* | `SEGURANCA.md` §7 |
| **N** | **O `.ndx` sobre a coluna marcada continua em claro**, e há teste que **prova o vazamento**: um índice guarda a chave para poder comparar, e cifrá-la destruiria a ordem. Quem precisa dos dois tira o índice da coluna sensível | `SEGURANCA.md` §11.3; teste `o_indice_sobre_a_coluna_marcada_continua_em_claro` |
| **N** | **Ligar a cifra não cifra o que já existe**, e não há comando de recifragem: vale do volume seguinte em diante | `SEGURANCA.md` §8 e §11.6 |
| **N** | **Sem troca de senha pelo protocolo** (muda no `config.json` e reinicia), **sem bloqueio por faixa** (é IP a IP), e **as tentativas vivem em memória** — reiniciar zera os contadores | `SEGURANCA.md` §7 |
| **N** | **Não protege de quem lê o `config.json`.** Nunca protegeu | `SEGURANCA.md` §7 |

### 2.6 Espaço, SQL, plataformas e tela

| | não-garantia | número / prova |
|---|---|---|
| **N** | **O espaço de linha excluída não volta.** É a consequência aceita da ordem de digitação: uma tabela com muitas exclusões cresce e não encolhe | `COMPARACAO.md`, sobre o `OPTIMIZE TABLE` |
| **N** | **E a saída que o formato promete NÃO EXISTE.** O `FORMATO.md` §1 diz que o espaço «só volta com uma compactação explícita»; o §17 do mesmo arquivo diz que **o comando ainda não foi escrito**. Hoje não há nenhuma forma suportada de recuperar o espaço — ver [P4](#54-as-perguntas-que-este-documento-acrescenta) | `FORMATO.md` §1 e §17 |
| **N** | **O disco custa mais:** 253,6 MiB contra 57,3 do SQLite(R) e 104,0 do MySQL(R) para a mesma tabela de 1.000.000 de linhas — **4,42×** e **2,44×**. É o preço do modelo de arquivos separados | `DESEMPENHO.md` §13 |
| **N** | **A camada SQL é um começo:** `SELECT` simples traduzido para as operações que já existem. **Não há planejador, e não há expressão em `WHERE`** | `SQL.md`; `FORMATO.md` §17 |
| **N** | **A inserção perde para o SQLite(R) por 3,88×** (2,557 s contra 9,928 s no milhão) e **o excluir por 1,83×**. O `buscar` **empata** (164 contra 166 ms, faixas sobrepostas) e o `atualizar` ganha por 3,72× | `DESEMPENHO.md` §13, `bancada/comparacao/um-milhao.json` |
| **N** | **O driver ODBC responde `SQL_TC_NONE`** — «sem transações» — e o servidor **tem** transações. É lacuna do driver, e é uma **mentira sobre a garantia** | `ODBC.md`; `ROTEIRO-1.0.md`, SP000030 |
| **N** | **macOS não tem alvo** e não dá para compilar aqui; **Android e iOS**: o REST não é a forma certa, e isso não é limitação nossa. Provados: Linux x86-64, Windows e ARM (este último **executado** sob `qemu-aarch64-static`, não só compilado) | `REST.md` §7 |
| **N** | **A tela não é inteiramente traduzível.** Medido: **1.175** textos na fábrica de idiomas contra **1.720** ainda cravados no fonte — **40%**. A máquina existe e a catraca segura o número, mas ele só desce com trabalho | `CAPABILITIES.json`, bloco `idiomas`; `conferidor.rs` |
| **N** | **Sem CORS, sem `GET` para leitura e sem métricas em formato Prometheus** no REST, cada um com motivo escrito | `REST.md` §10 |

---

## 3. O contrato de compatibilidade

O que **1.0** quer dizer nesta casa: **um cliente escrito contra a 1.0 continua
funcionando em toda 1.x, sem recompilar e sem mudar de código.** Quebrar
qualquer item da §3.1 exige **2.0**.

### 3.1 O que a 1.0 se compromete a NÃO quebrar

| | congelado | o alcance exato |
|---|---|---|
| **G** | **A ordem de digitação** | rowid nunca reaproveitado, nunca renumerado; `varrer` **sem índice declarado** devolve na ordem de digitação; o rowid identifica a linha para a replicação, o `.trash`, o `.reason` e o `.lgpd`. **É pétrea**, e é a promessa mais cara que este produto faz — ver §2.6 |
| **G** | **O formato em disco: LER** | um motor 1.x lê tudo que um motor 1.y escreveu (y ≤ x) **e** continua lendo as versões antigas que já lê hoje. O que **não** se promete é escrever na versão antiga. Versões de hoje: `.reg` **4** (5 quando cifrado), esquema `PSCH` **7** (lê a partir da 2), `.ndx` **1**, `.bin`/`.memo` **2**, `.log`/`.trash`/`.reason`/`.lgpd` **2** (3 quando cifrados), marca `.tx` **2** (lê a 1), `.pag` e `backup.json` em JSON |
| **G** | **O protocolo** | o **nome** de uma operação, seus campos obrigatórios e o **significado** da resposta. Campo novo pode aparecer — o cliente ignora o que não conhece. Apelido que existe continua atendendo (`systables`/`sistabelas`). **Remover ou renomear operação é 2.0** |
| **G** | **O REST** | `POST /v1/<operação>`; o **caminho manda** sobre o corpo; o envelope (`ok`, `op`, `resultado`/`erro`, `codigo`, `nome`, `classe`, `sprint`, `repetir`, `ms`, `sessao`); o mapa faixa-de-erro → HTTP; e o `openapi.json` **gerado do catálogo**, nunca digitado |
| **G** | **Os códigos de erro** | o **número** e o que ele significa. Erro novo toma número novo, dentro da faixa. Quem ramifica em `codigo`/`nome`/`classe` não quebra |
| **G** | **O `config.json`** | campo que existe mantém nome e significado; campo com nome errado avisa; **campo novo nasce com o comportamento velho por padrão** |
| **G** | **O portão de permissão** | continua sendo **um só**, e regra nova de direito **só estreita** — nunca tira direito de quem não pediu (`sem_regra_de_tabela_nada_muda`) |
| **G** | **A regra da guarda nova** | *guarda nova entra pedida, não imposta*: quem manda o campo ganha a garantia, quem não manda continua como antes. **Proteção que quebra todo cliente antigo não é proteção, é estrago** |
| **G** | **Zero dependências externas** | `cargo build --offline` continua funcionando na 1.x |

### 3.2 O que fica LIVRE para mudar dentro da 1.x

| | livre | e por que dizer isso importa |
|---|---|---|
| **N** | **O texto de qualquer mensagem**, inclusive o prefixo `[SPxxxxxx]` | mensagem é texto, e texto se traduz e se reescreve. **Quem programa contra a frase quebra calado.** O contrato está no `codigo`, no `nome` e na `classe` — o `sprint` é campo, mas o **valor** dele muda quando o roteiro muda |
| **N** | **O HTML, o CSS e o layout da interface** | a tela é produto, não protocolo |
| **N** | **A decomposição interna** — o `servidor.rs` de 22.560 linhas vai ser partido (SP000005), e nenhum cliente sente | módulo é organização, não contrato |
| **N** | **Desempenho, uso de memória e as escolhas de cache** | cache de páginas, group commit, write-back: nada disso é promessa de número |
| **N** | **A versão de qualquer arquivo em disco pode SUBIR** | o que não pode é deixar de ler o que já está gravado |
| **N** | **A ordem de `varrer` COM índice** é a do índice, e pode mudar se o índice mudar | só a ordem **sem** índice é congelada |
| **N** | **Os números da bancada** | eles medem esta máquina neste dia, e o gráfico recusa desenhar sem o JSON da rodada |

### 3.3 A regra que atravessa as duas listas

**Mudança de formato entra CEDO.** Enquanto não há dado em produção é barata;
depois vira migração. **A 1.0 é o último momento barato** — e é por isso que a
decisão sobre o RID lógico ([§5.2](#52-sp000013--o-rid-lógico-e-o-último-momento-barato))
não pode ser adiada para «depois da 1.0»: depois da 1.0, por definição, há dado
em produção.

---

## 4. A linha de corte: onde a 1.0 termina

### 4.1 O critério, dito antes da lista

**P0** — *O critério abaixo é proposta. Se o dono trocar o critério, a lista
inteira se refaz sozinha — e é para isso que ele vem antes dela.*

**1.0 não quer dizer «pronto». Quer dizer «não quebra».** Todo recurso admitido
na 1.0 vira uma linha da §3.1 e passa a ser pago por anos. **A 1.0 menor que um
usuário de produção consegue confiar vale mais que a 1.0 maior.**

Então **entra na 1.0 o que, faltando, faz um usuário de produção:**

1. **perder dado** que ele acredita gravado;
2. **vazar dado** para quem não devia vê-lo;
3. **ficar preso** — não conseguir atualizar, voltar atrás, diagnosticar ou sair.

E **sai da 1.0** tudo o que é conveniência, alcance de plataforma, qualidade de
plano ou pesquisa — **não porque não valha**, mas porque congelar cedo custa mais
do que entregar tarde. O critério não é «o que é interessante de construir»: é
**o que dói em produção quando falta**.

### 4.2 O que ENTRA na 1.0

| bloco | sprints | qual dos três testes |
|---|---|---|
| Fundação | SP000001–005 | (3) — sem build reproduzível e sem fonte única de verdade ninguém consegue voltar atrás |
| Transações e integridade | SP000006–010 | (1) |
| Concorrência e armazenamento | SP000011–016 | (1) e (3) — a trava global é o teto de crescimento; SP000013 e SP000014 conforme a decisão do dono (§5) |
| SQL relacional, **parte** | SP000017–021 | (3) — sem contrato SQL, DDL e DML com *prepared statements*, o usuário não migra para cá e não sai daqui |
| Segurança | SP000024–027 | (2) |
| Alta disponibilidade | SP000028, 029, 032 | (1) e (3) — WAN e **entre versões**, split-brain, e backup incremental/PITR/upgrade N/N-1 |
| Ecossistema, **reduzido** | SP000030 e SP000033 | ver §4.3 |
| GA | SP000034–035 | pilotos, congelamento funcional e auditoria independente |

### 4.3 O que SAI, e para onde — com o motivo

| sprint | proposta | motivo, pelo critério |
|---|---|---|
| **SP000022** — operadores relacionais avançados | **1.1** | não passa em nenhum dos três. Quem precisa de janela e CTE hoje faz duas consultas; quem perde dado por falta delas não existe |
| **SP000023** — estatísticas, otimizador, `EXPLAIN`, conformidade | **partida**: fica na 1.0 **só o `EXPLAIN` que diz qual índice foi usado e quantas linhas foram tocadas**; o otimizador por estatísticas e a conformidade vão para a 1.1 | `EXPLAIN` é **diagnóstico** e passa no teste (3): sem ele o usuário não descobre por que a consulta está lenta nem se declarou o índice certo. Otimizador é **qualidade de plano** — melhora sem quebrar, e melhorar depois é barato |
| **SP000030** — ODBC completo e drivers oficiais | **reduzida**: fica na 1.0 **o ODBC que não mente** — hoje ele responde `SQL_TC_NONE` e o servidor tem transações. Os «drivers oficiais» (JDBC, .NET, DBAPI) vão para a 1.1 | mentira sobre transação é **perda de dado por confiança errada** — teste (1). Driver novo é alcance, e alcance não congela nada |
| **SP000031** — Windows, Linux, macOS, ARM, Android, iOS | **1.1 / 2.x** | Linux, Windows e ARM já estão provados e são o alvo de produção. O próprio `REST.md` §7 diz que **o REST não é a forma certa** em Android e iOS: é produto novo, não portabilidade |
| **SP000033** — observabilidade, capacidade e benchmark certificado | **reduzida**: observabilidade e capacidade ficam na 1.0; **«benchmark certificado» sai** | observabilidade é teste (3). Benchmark certificado é **marketing**, não contrato — e esta casa já publicou três números que a comparação desmentiu |
| **SP000036–045** — Phx Contract | **2.x**, como o roteiro já diz | uma linguagem declarativa admitida na 1.0 vira **sintaxe que não se pode mais mudar** — o pior tipo de congelamento cedo |
| **SP000046–055** — Cognitive Lab | **2.x**, como o roteiro já diz | pesquisa. Nada nela impede perder dado, vazar dado ou ficar preso |

**O que este documento NÃO faz é tirar sprint por tirar.** O roteiro já cortou
bem: o bloco 1.0 dele tem 35 sprints e o 2.x tem 20, e a divisão está certa. O
que se acrescenta aqui é **o preço de cada uma que fica** — e três reduções onde
uma sprint inteira estava carregando junto uma parte que congela sem precisar.

### 4.4 O que não tem sprint e passa nos três testes

Cada um destes passa no critério da §4.1 e **hoje não tem dono no roteiro**. São
candidatos, e por isso são perguntas — a coluna da direita diz qual.

| | candidato | teste | pergunta |
|---|---|---|---|
| **P** | **Trava de diretório**: recusar subir quando outro processo já serve aquele caminho | (1) | [P4](#54-as-perguntas-que-este-documento-acrescenta) |
| **P** | **A transação chega parcelada na réplica** (§2.4) — medir antes de decidir | (1) | P5 |
| **P** | **A cascata escreve fora do escopo declarado da transação** — é o que falta do **C** | (1) | P6 |
| **P** | **`replica.rs` sem teste no portão** — o laço que faz a replicação andar | (1) | P7 |
| **P** | **Filha em outro schema** e **auto-referência em silêncio** (§2.2) | (1) | P8 |
| **P** | **A reconstrução explícita que devolva espaço** — prometida no `FORMATO.md` §1 e inexistente no §17 | (3) | [P1](#51-sp000014--o-espaço-que-não-volta) |

### 4.5 O que a 1.0 custa, dito para não haver surpresa

**N** — Cada linha da §3.1 é uma promessa que se paga em toda versão 1.x. A mais
cara é a primeira: **a ordem de digitação**. Ela compra o `rowid` como identidade
estável para sempre — que é o que faz a replicação ser fiel, o `.trash` e o
`.reason` apontarem para linha certa, e o MVCC ser **mais fácil** aqui do que nos
outros. E cobra: **o espaço não volta.**

---

## 5. As decisões que o roteiro exige do dono

As três primeiras são as que o `ROTEIRO-1.0.md` já nomeia na seção «Antes da
lista». Cada uma vem em um parágrafo, terminando na pergunta.

### 5.1 SP000014 — o espaço que não volta

**P1** — A SP000014 é «reuso de espaço, `VACUUM` e compactação», e ela pede
exatamente o que a pétrea proíbe: reusar slot **é** reaproveitar slot excluído, e
compactar reescreve o `rowid`, que aqui **é endereço** — quem guardou um passa a
apontar para outra linha. O roteiro já registra a decisão tomada: **recusada, a
pétrea vence, não é pendência e sim escopo fora.** O que sobra para a 1.0 não é
reabrir a recusa: é que **a saída de escape que o próprio formato promete não
existe.** O `FORMATO.md` §1 diz que o espaço «só volta com uma compactação
explícita, que renumera os rowids e reconstrói os índices»; o §17 do mesmo
arquivo diz que **o comando ainda não foi escrito**. Hoje, um banco com muitas
exclusões cresce e não há nada suportado a fazer. **A pergunta: a 1.0 sai
dizendo «o espaço nunca volta, ponto» — e aí a frase da §1 do `FORMATO.md` tem de
sair — ou sai com uma reconstrução explícita, offline, que renumera e quebra a
identidade do rowid para aquela tabela (e portanto o que a replicação, o
`.trash`, o `.reason` e o `.lgpd` apontam), com esse preço escrito ao lado do
comando?**

### 5.2 SP000013 — o RID lógico, e o último momento barato

**P2** — A SP000013 é «RID lógico estável e formato físico v2», e o roteiro a
**rebaixou a melhoria** com número: medido contra um MySQL(R) 8.0.46 de verdade,
o que ancora uma cadeia de versões é a identidade estável da linha mais um
ponteiro, e não um identificador novo — e o `rowid` daqui já é isso, por
construção, justamente porque o `.reg` nunca reaproveita slot. Ou seja, a pétrea
que parecia atrapalhar o MVCC é o que o torna **mais fácil** aqui. O que continua
verdadeiro é o custo: RID lógico é **mudança de formato em disco**, e a SP000016
(MVCC) já pede a sua própria — `.reg` v6 mais uma área de *undo* fora do `.reg`.
A regra da casa é que mudança de formato entra **cedo**, e a §3.3 deste contrato
diz por que isso vira urgência: **depois da 1.0, por definição, existe dado em
produção — a 1.0 é o último momento barato.** **A pergunta: a mudança de formato
que a SP000013 e a SP000016 pedem entra ANTES da 1.0, pagando agora o que depois
vira migração — ou a 1.0 congela o formato de hoje e assume que MVCC e RID lógico
serão 2.0, com migração de banco?**

### 5.3 SP000024 — TLS, e a única promessa que a 1.0 não pode fazer pela metade

**P3** — A SP000024 pede TLS 1.3 e mTLS em todos os canais, e o roteiro registra
que ela foi **adiada por palavra do dono** («pule o TLS, vemos depois»). O
problema é que ela encosta em duas coisas ao mesmo tempo: escrever TLS à mão está
recusado com motivo (*«TLS mal escrito é pior que TLS ausente, porque parece
seguro»* — seria X.509, ASN.1 e uma pilha de *handshake* aqui dentro), e admitir
uma crate quebra a pétrea de zero dependências, que é o que faz o
`cargo build --offline` e a compilação cruzada funcionarem. Existe um terceiro
caminho, que a auditoria externa lista como aceitável: **proxy obrigatório à
frente**. E há um dado que torna a decisão urgente e não estética: a cifra do fio
**não vale para a interface web** — o navegador fala TLS ou fala claro —, e na
porta 5000 ela nasce com `exigir` **desligada**, o que a limita a escuta passiva.
**A pergunta: a 1.0 declara o proxy TLS como PARTE DO PRODUTO — documentado, com
receita de implantação e prova na bateria, e o `config.json` recusando subir a
porta web sem ele em modo de produção — ou a 1.0 admite uma dependência externa
só para o transporte, ou promete TLS nativo e aceita o risco que a recusa
descreve?**

### 5.4 As perguntas que este documento acrescenta

Não estavam no roteiro, e cada uma passa no critério da §4.1.

**P4 — A trava de diretório.** Não há trava de arquivo nem de registro
(`FORMATO.md` §17), e conferido no código não há arquivo de trava: **nada impede
dois processos de abrirem o mesmo diretório de dados.** O caso fácil de acontecer
não é exótico — é a CLI `phxsql reindex` rodando num diretório que o `phxsqld`
está servindo. **Entra na 1.0 uma trava de diretório que faz o segundo processo
recusar subir dizendo quem já está lá?**

**P5 — A transação parcelada na réplica.** A posição de replicação é por tabela
e não existe ordem global entre tabelas (`INTEGRIDADE.md` §3); um `COMMIT` que
escreve em duas tabelas produz eventos em dois `.log` puxados independentemente.
Isto está **deduzido do desenho e não medido**, e a regra da casa é que
diagnóstico plausível não é diagnóstico medido. **A 1.0 mede isso antes de
prometer qualquer coisa sobre transação em cluster — e, medido, o que ela promete:
«atômico no source, parcelado na réplica» dito no contrato, ou um número de
sequência do database inteiro (que muda o `.log` e quebra réplica antiga)?**

**P6 — O que fecha o C.** A cascata escreve em tabela que a transação não
declarou, então um `ROLLBACK` não alcança a filha (`TRANSACOES.md` §4.6 e §12).
É a única coisa que falta para o **C**, e hoje não tem sprint. **Ela entra na
1.0 — e portanto o **C** fica inteiro — ou a 1.0 sai com o **C** parcial e a
frase escrita?**

**P7 — A cobertura do que a 1.0 promete.** A replicação é uma garantia da §1.5,
e `replica.rs` **não tem nenhum teste no `cargo test`**: a prova mora numa
bancada que precisa de quatro servidores e não roda no portão. **Garantia da
1.0 pode ser sustentada só por bateria que não roda no portão, ou a 1.0 exige que
tudo o que ela promete tenha prova no caminho obrigatório?**

**P8 — Os dois buracos de alcance da integridade.** O `excluir_tabela`, o
`renomear_tabela` e o verificador varrem **um diretório**, então **filha em outro
schema não é vista** — e a busca reversa é por varredura por escolha, porque
excluir é raro e inserir é o laço quente (`INTEGRIDADE.md` §6). E a
**auto-referência passa em silêncio**, onde os dois grandes recusam, o que está
registrado como **defeito** e não como decisão (§7.4). Os dois deixam nascer
órfã sob chave que o esquema declara conferida — que é a promessa falsa que a
SP000007 fechou por outro caminho. **A 1.0 fecha os dois, ou declara no contrato
que «conferida» quer dizer «conferida dentro de um schema, e sem auto-referência»
— e nesse caso o verificador tem de dizer isso ao lado de cada chave que ele
aprova?**

---

## 6. De onde sai cada número

Nenhum número deste documento foi digitado de memória. A tabela é o crivo:
quem quiser conferir um, roda o gerador.

| número | de onde sai |
|---|---|
| 1.547 testes, **121 operações**, 8 crates, 128.745 linhas Rust, 39.349 de doc, 0 dependências, 1.175/1.720/2.895 idiomas (40%), commit `d1e84da`, versão 0.18.0 | `CAPABILITIES.json`, gerado por `python3 docs/dossie/numeros-do-projeto.py`, medido em 2026-09-03 15:07:55 |
| 77 guardas, 73 provadas, 4 redundantes | tabela gerada entre `<!-- guardas:inicio/fim -->` no `TESTES.md` §8, por `bancada/guardas/provar-guardas.py --json` + `tabela-no-testes.py` |
| 175 pedidos: 168 feitos, 4 parciais, 3 planejados | saída de `python3 docs/dossie/pagina-dos-pedidos.py`, **rodado nesta sessão**; o `git diff` ficou vazio, então o número no repositório é o de hoje |
| cobertura por área (transações 16 testes / 1,0%; replicação 11 / 0,7%) e os arquivos sem `#[test]` | tabela gerada entre `<!-- cobertura:inicio/fim -->` no `TESTES.md` §1, por `docs/dossie/cobertura-por-area.py` |
| +7,03 µs/linha (+11,2%) da chave conferida; 46,8 µs para abrir a mãe | `DESEMPENHO.md` §15, `--example onde-doi` |
| 2,40× do cache de páginas; 44,4 → 15,9 µs (2,79×); 83,5% → 63,6% no `.ndx` | `DESEMPENHO.md` §1 e §2 |
| 1,99× no `ping` contra 1,51–1,59× no `varrer`, com controle | `DESEMPENHO.md` §14, `bancada/concorrencia/a-trava-serializa.py` |
| 3.122 µs de leitura contra 137 µs de gravação (23×); `fsync` sob a trava 1.267–1.371 µs (10,3×–12,3×) | `CONCORRENCIA.md` §7.1 |
| 76 seções críticas; 4 de 76 com ponto de cancelamento; 0 de 76 na classe `rede-ou-espera` | `bancada/concorrencia/mapa-da-trava.py`, citado em `CONCORRENCIA.md` §1.2 e §7.2 |
| matriz de durabilidade 0/0/0, 1/1/0, 1/1/1; 21 corridas, 1.200 filhas, 9 casos denunciados | `bancada/durabilidade/prova.py`; `TRANSACOES.md` §5.7 e §5.5.3 |
| 1→2→2→3→2 do *read-your-own-writes* | `bancada/transacoes/visibilidade.py`; `TRANSACOES.md` §4.4.1 |
| 28.914 linhas/s, atraso 1,3–2,1 s, 4.273 → 17.450 eventos/s (4,08×), cascata 1.679 → 1.827 ms | `bancada/replicacao/medir.py`; `REPLICACAO.md` §13–§15, `DESEMPENHO.md` §4.5 |
| `pedidos` com 0 de 2 eventos em duas das três ordens | `INTEGRIDADE.md` §3, tabela das três ordens |
| `varrer` esperando 30,7 s numa réplica cortada | `REPLICACAO.md` §17 |
| medianas do milhão (9,928 / 2,557 / 12,342 s etc.), 253,6 vs 57,3 vs 104,0 MiB, piso de 59,6% | `bancada/comparacao/medir.py` → `um-milhao.json`; `DESEMPENHO.md` §13 |
| 0,10 µs/linha da cifra por coluna; 10,4 µs/linha da base | `SEGURANCA.md` §11.1, `--example onde-doi` |
| 210.000 voltas do PBKDF2; os RFC/FIPS de cada primitiva | `TECNOLOGIAS.md` §3, com o nome do teste de cada vetor |
| 73 conferências do ODBC | `bancada/odbc/provar.py`; `ODBC.md` |
| 14m35s da bateria inteira; os três códigos de saída | `provar.py --listar`; `TESTES.md` §7 |
| catracas 1.720 / 0 / 0 | `crates/phxsql-server/src/conferidor.rs` |
| versões de arquivo em disco (`.reg` 4/5, `PSCH` 7, `.ndx` 1, `.bin`/`.memo` 2, `.tx` 2/1) | constantes no código: `reg.rs:115` e `:129`, `schema.rs:42`, `ndx.rs:67`, `blob.rs:39`, `transacao.rs:54` e `:57` |
| 11 corridas do workflow `Portoes`; 22.560 linhas do `servidor.rs` | `ROTEIRO-1.0.md`, SP000002 e SP000005 (medidos lá, e este documento não os remede) |
| **a ausência de trava de instância** (2 acertos, nenhum deles trava) | medição **desta rodada**: um `grep -rnE` nos oito crates pelas cinco formas de tomar arquivo em exclusivo — `flock`, `LOCK_EX`, `libc::open`, `custom_flags` e `create_new(true)` |

**Um número que este documento se recusou a citar:** quantas das 76 seções
críticas fazem `fsync`. O `CONCORRENCIA.md` §1.2 diz **24** e o `ROTEIRO-1.0.md`
diz **23**, com a observação de que o gerador não vinha sendo rodado. Quem decide
é `bancada/concorrencia/mapa-da-trava.py`, e um contrato não cita número em
disputa — cita o gerador.

---

## 7. Como este documento não envelhece

Ele envelhece pela §1 e pela §2, que são as que mentem primeiro: uma garantia
some quando alguém a quebra sem perceber, e uma não-garantia vira mentira no dia
em que alguém a conserta e não vem apagá-la daqui. Foi o que aconteceu com
`REPLICACAO.md` §15, que ainda diz «não há transação» — e é por isso que a §2.4
o nomeia em vez de repetir o erro em silêncio.

Três regras, e elas são de manutenção e não de estilo:

1. **Toda linha continua sendo G, N ou P.** Frase que não é nenhuma das três sai
   do documento — e é assim que ele não vira folheto de novo.
2. **Número novo entra com a linha da §6 no mesmo commit.** Sem a procedência, o
   número não entra.
3. **Quando uma sprint fechar, a não-garantia que ela matou sai daqui** — e a
   garantia que ela criou entra na §1 com a prova ao lado. Lacuna que muda de
   nome sem ninguém reescrever vira documento que envelhece dizendo a verdade de
   ontem.

E a decisão de fundo, que este papel não toma: **as oito perguntas da §5 são o
que falta para a SP000001 fechar.** Enquanto elas estiverem abertas, este
documento é proposta; respondidas, ele vira o contrato — e é a partir daí que as
outras 54 sprints sabem quando pararam.

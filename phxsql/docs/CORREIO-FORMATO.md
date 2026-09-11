# Correio nativo do PhxSql — proposta de formato em disco

**Papel:** C (DBA sênior). **Data:** 11/09/2026. **Estado:** PROPOSTA para o dono
aprovar — **nada foi gravado**.

Esta é uma proposta de formato, e por lei do projeto **formato em disco se
decide com o dono ANTES de gravar**. Portanto aqui não há código, não há PSCH
alterado no fonte, não há versão de formato bumpada, não há commit. O que segue
são tabelas propostas, as pétreas aplicadas a elas, o custo da mudança de
formato e — o mais importante — **as perguntas que travam o gravar**. Enquanto
elas não tiverem resposta, o formato não congela.

## De onde isto sai

O modelo já está provado ponta a ponta, **em memória**, no protótipo
`crates/phxsql-core/examples/correio-e2e.rs` (24 checagens, PROVA VERDE), com as
três cognições que o acompanham:

- `docs/cognicao/cognicao_correio-e2e-a-senha-do-outro-e-ecdh_20260911_1445.md`
- `docs/cognicao/cognicao_anti-spam-do-correio-e-o-canal-de-pedido_20260911_1452.md`
- `docs/cognicao/cognicao_status-precisa-de-estado-nao-de-bool_20260911_1630.md`

O protótipo **não é o formato em disco** — é o modelo. Ele guarda tudo em
`Vec` na RAM. Esta proposta é a tradução daquele modelo para tabelas PSCH, no
modelo de arquivos separados do resto do motor (`docs/FORMATO.md`).

O que o protótipo prova e esta proposta preserva:

1. Identidade **X25519**; a privada mora **cifrada sob a senha** (PBKDF2 →
   ChaCha20-Poly1305) — não abre sem a senha.
2. Só os domínios `phxsql.com.br` e `phxmail.com.br` (e subdomínios) mandam/recebem.
3. Sem **confiança aceita** não há contato — nos **dois** canais: o da mensagem
   e o do **pedido** (repetido recusado, bloqueado não pede, teto de pendentes).
4. Mensagem cifrada pelo segredo **ECDH**: cada um decifra com a **própria**
   senha (que abre a própria privada) + a chave **pública** do outro. Nunca se
   pede a senha do outro, porque correio é assíncrono.
5. Três camadas encaixadas: E2E (sempre) + alto segredo (frase por telefone/SMS)
   + Masson (flag X). **Prioridade e tipo entram no AAD** — rebaixar um alerta
   grava não decifra.
6. Anexos num armazém **à parte**; a mensagem guarda só a referência.
7. Moderação (banir/quarentena/multa) com **motivo obrigatório**, e a **decisão**
   (deferir/indeferir) também carrega motivo.
8. Estado das solicitações: confiança `pendente/aceita/recusada`, moderação
   `aberta/deferida/indeferida`. Recusar **não** é bloquear.

## Papéis desta rodada (a cláusula pétrea cobra a dispensa registrada)

- **C (DBA)** — dono deste documento. Escolhe chaves, índices, integridade
  referencial, ordem de digitação, mudança de formato.
- **A (orquestrador)** — dispensado do documento, **convocado obrigatório na
  integração**: a garantia "pai estritamente antes do filho" (§5) mora no
  gestor de transações, não no formato, e é ali que ela se cruza com o que a
  frente das transações já fez.
- **B (engenheiro)** — **não** convocado: não se escreve código antes do dono
  aprovar o formato. Entra depois.
- **F (prova real)** — **não** convocado agora, **obrigatório** quando gravar:
  cada garantia abaixo precisa do teste que falha com o defeito reposto.
- **J (pesquisador)** — o consenso dos motores maduros não alcança um correio
  E2E proprietário; dispensado com motivo.
- **D, E, G, H, I** — fora do domínio desta proposta; dispensados.

---

## 1. Visão geral

Seis tabelas PSCH, no modelo de arquivos separados. Cada uma é uma tabela comum
do motor — nasce com `.reg`, `.ndx` e, quando tem coluna externa, `.bin`. Nada
de formato especial: o correio é **dado do PhxSql guardado no PhxSql**, e é essa
a força da proposta — ele herda CRC, espelho, diário, replicação e as pétreas de
integridade sem uma linha nova de motor de arquivo.

| Tabela | Papel | Externo (`.bin`) |
|---|---|---|
| `correio_contas` | uma linha por caixa (endereço, pública em claro, privada selada) | privada selada |
| `correio_confiancas` | solicitações de relação de confiança, com estado | — |
| `correio_bloqueios` | o "nunca mais" (distinto de recusar) | — |
| `correio_moderacoes` | banir / quarentena / multa, com motivo e decisão | — |
| `correio_mensagens` | o envelope cifrado, de/para/prioridade/tipo em claro | envelope cifrado |
| `correio_anexos` | o armazém à parte; conteúdo cifrado, um por linha | conteúdo cifrado |

Duas decisões de base, e o porquê de cada uma:

- **A chave estrangeira aponta para `correio_contas.id` (Uuid), não para o
  endereço.** O `id` é imutável e estreito (16 bytes); o endereço é a identidade
  visível (largo, e — ver §6 — usado no AAD do selo). Amarrar a integridade ao
  `id` deixa a relação independente de qualquer futura troca de endereço. O
  endereço fica em coluna própria, com índice único, e é ele que responde "quem
  é você?".
- **O material de cifra opaco (privada selada, envelope da mensagem, conteúdo do
  anexo) vai em coluna `Bin`, não inline.** Duas razões em §3: o motor não tem
  tipo inline de bytes crus de largura fixa, e o `.bin` é separado do `.reg`, o
  que mantém as varreduras de caixa (listar, filtrar, paginar) rápidas — que é o
  que o dono pediu com *"os anexos devem ficar em outro arquivo para não ficar
  lento"*.

---

## 2. Tabelas propostas

Legenda de tipos (todos já existem no motor, `crates/phxsql-core/src/types.rs`):
`Uuid` = 16 bytes crus (v7 crescente, ótimo para índice), `Uuid256` = 32 bytes
crus, `Str(n)` = UTF-8 de largura fixa, `UInt1` = 1 byte, `Int8` = i64,
`DateTime` = i64 **milissegundos**, `Bool` = 1 byte, `Bin` = ponteiro de 16
bytes para o `.bin`. As colunas de sistema `softdeleted`, `rownum` e a **nova**
`rowts` (§5) entram no fim de toda tabela; abaixo elas ficam implícitas.

### 2.1 `correio_contas`

Uma linha por caixa de correio.

| Coluna | Tipo | Nulo | O que é |
|---|---|---|---|
| `id` | `Uuid` | não | **PK**. UUID v7 sorteado na criação. Alvo das FKs das outras tabelas |
| `endereco` | `Str(320)` | não | `usuario@empresa.dominio`. **UNIQUE**. 320 = limite clássico (local 64 + `@` + domínio 255); RFC 5321 fixa 254, a folga cobre variações |
| `empresa` | `Str(255)` | não | o host da conta. **Indexada** — serve à regra "mesma empresa" e à política de domínio |
| `publica` | `Uuid256` | não | chave pública X25519, **32 bytes em claro** (pública não é segredo) |
| `priv_selada` | `Bin` | não | a privada X25519 selada sob a senha. Bloco opaco `[sal 16][nonce 12][ct 32][tag 16]` = **76 bytes**, no `.bin` |
| `criada_em` | `DateTime` | não | quando a conta nasceu (dado da conta, não a coluna de sistema) |

**Chaves e índices:**

- PK `id` (único).
- UNIQUE `endereco` — alvo (lado mãe) de todas as FKs `de`/`para`/`alvo`. Sem
  ela, nenhuma FK para endereço poderia ser conferida; como as FKs apontam para
  `id`, esta continua obrigatória por ser a chave de busca de login.
- Índice `empresa`.

**O que NÃO se grava:** a senha. Nunca. Ela só é usada (no cliente ou no login)
para derivar a chave PBKDF2 que sela/abre `priv_selada`. Ver §3 e §4.

### 2.2 `correio_confiancas`

Uma linha por par ordenado (quem pede → quem recebe).

| Coluna | Tipo | Nulo | O que é |
|---|---|---|---|
| `id` | `Uuid` | não | **PK** |
| `de_id` | `Uuid` | não | **FK → `correio_contas.id`**. Quem pede |
| `para_id` | `Uuid` | não | **FK → `correio_contas.id`**. Quem recebe |
| `estado` | `UInt1` | não | 0 pendente, 1 aceita, 2 recusada |
| `respondida_em` | `DateTime` | sim | quando o `para` respondeu (nulo enquanto pendente) |

**Chaves e índices:**

- PK `id`.
- **UNIQUE (`de_id`, `para_id`)** — um par tem uma linha só. O re-pedido depois
  de recusada é um **`atualizar`** que volta o estado a pendente, não uma linha
  nova (respeita a ordem de digitação: não se cria slot para reabrir um pedido).
- Índice `de_id` (lado filho da FK; serve ao "o que eu pedi").
- Índice (`para_id`, `estado`) (lado filho da outra FK; serve ao "quem eu tenho
  a aprovar" e ao teto de pendentes em tempo de índice).

**Nota de modelagem, para o dono decidir em §6:** UNIQUE(`de_id`,`para_id`) +
atualizar-no-lugar **descarta o histórico** de recusas anteriores. Se o dono
quiser histórico ("já me recusou três vezes"), a alternativa é tabela
append-only sem o UNIQUE, com o estado corrente derivado pela `rowts` mais nova.

### 2.3 `correio_bloqueios`

O "nunca mais", distinto de recusar (que é "não, obrigado — pode pedir depois").

| Coluna | Tipo | Nulo | O que é |
|---|---|---|---|
| `id` | `Uuid` | não | **PK** |
| `dono_id` | `Uuid` | não | **FK → `correio_contas.id`**. Quem bloqueia |
| `quem_id` | `Uuid` | não | **FK → `correio_contas.id`**. Quem foi bloqueado |
| `bloqueado_em` | `DateTime` | não | quando |

- PK `id`; **UNIQUE (`dono_id`, `quem_id`)**; índice `quem_id`.
- Bloquear um `quem` **retira** o pedido pendente dele (o protótipo faz isso), e
  passa a recusar novos pedidos — a conferência do canal de pedido lê esta
  tabela antes de aceitar um `pendente`.
- **Aberto para o dono (§6):** bloqueio de **domínio inteiro** (`golpe.phxsql.com.br`
  de uma vez) não cabe aqui — seria uma tabela `correio_bloqueios_dominio(dono_id,
  dominio)` ou uma política. Não proponho agora sem a palavra dele.

### 2.4 `correio_moderacoes`

| Coluna | Tipo | Nulo | O que é |
|---|---|---|---|
| `id` | `Uuid` | não | **PK** |
| `tipo` | `UInt1` | não | 0 banir (alto prejuízo), 1 quarentena (baixo), 2 multa |
| `de_id` | `Uuid` | não | **FK → `correio_contas.id`**. Quem abriu |
| `alvo_id` | `Uuid` | não | **FK → `correio_contas.id`**. Contra quem |
| `motivo` | `Memo` | não | **obrigatório** — texto livre, no `.memo` |
| `estado` | `UInt1` | não | 0 aberta, 1 deferida, 2 indeferida |
| `decisao_motivo` | `Memo` | sim | o motivo de quem deferiu/indeferiu (nulo enquanto aberta) |
| `link_pagamento` | `Str(512)` | sim | só na multa |
| `decidida_em` | `DateTime` | sim | quando o moderador decidiu |

**Chaves e índices:**

- PK `id`.
- Índice `de_id` (filho da FK; "as que eu abri").
- Índice (`alvo_id`, `estado`) (filho da FK; "o que pesa contra este alvo").

**Regras que o motor pode impor sozinho (expressões de esquema, PSCH v9):**

- `motivo` **não nulo** + `CHECK length(trim(motivo)) > 0` — o motivo vazio é
  recusado na gravação, não só na tela. É a mesma lei do `.reason`: quem julga
  diz por quê.
- Multa exige link: `CHECK tipo <> 2 OR link_pagamento IS NOT NULL`. Atenção à
  semântica v9 — *falso recusa, nulo passa* —, então a expressão precisa devolver
  falso (e não nulo) quando `tipo = 2` e o link falta. A conferência "só decide
  o que está aberta" e "não se re-julga" é lógica de aplicação, não de formato.

### 2.5 `correio_mensagens`

O envelope cifrado. As colunas que o motor **filtra, ordena e o AEAD autentica**
ficam inline, em claro; o pacote opaco vai ao `.bin`.

| Coluna | Tipo | Nulo | O que é |
|---|---|---|---|
| `id` | `Uuid` | não | **PK**, v7 crescente (a folha mais à direita da B+tree, insert barato) |
| `de_id` | `Uuid` | não | **FK → `correio_contas.id`** |
| `para_id` | `Uuid` | não | **FK → `correio_contas.id`** |
| `prioridade` | `UInt1` | não | 0 alta, 1 média, 2 baixa. **Vai no AAD** (ver abaixo) |
| `tipo` | `UInt1` | não | 0 alerta, 1 aviso, 2 normal. **Vai no AAD** |
| `camadas` | `UInt1` | não | bitmap: bit 0 alto segredo, bit 1 Masson. Diz o que abrir |
| `envelope` | `Bin` | não | `[sal_kdf 16][sal_extra 16][sal_masson 16][nonce 12][tag 16][ct …]`, no `.bin` |
| `lida` | `Bool` | não | conveniência da tela; nasce `false`. Opcional |
| `enviada_em` | `DateTime` | não | carimbo de envio (dado; distinto da `rowts` de sistema) |

**Chaves e índices:**

- PK `id`.
- Índice `de_id` (filho da FK; "enviadas por mim").
- Índice (`para_id`, `lida`) (filho da FK; a caixa de entrada e o "não lidas").
- A **linha do tempo da caixa** sai de graça da `rownum`/`rowts` — o `.reg`
  guarda na ordem de chegada, e paginar por chegada é bissecção sem índice novo.

**Por que `prioridade` e `tipo` ficam em claro E no AAD:** o motor precisa deles
inline para listar e ordenar sem decifrar; e eles entram no dado associado do
AEAD para que **rebaixar um alerta gravado** (mexer na cópia inline) faça a
etiqueta não conferir e a leitura recusar. Metadado que decide não viaja fora do
selo — é a cognição do anti-spam. A cópia inline é conveniência; a verdade é
autenticada.

**A referência aos anexos NÃO mora aqui.** É 1-para-muitos, e pela regra
primordial *a chave se declara na filha*: a FK vive em `correio_anexos.mensagem_id`.
O protótipo guardava `Vec<u64>` na mensagem porque estava em memória; no disco,
inverte — a mensagem não carrega lista, os anexos apontam para ela.

### 2.6 `correio_anexos` — o armazém à parte

Tabela **própria**, com `.reg` e `.bin` próprios. É isto que atende ao *"outro
arquivo para não ficar lento"*: a `.reg` da mensagem não guarda um byte de
anexo, e a varredura da caixa nunca toca no `.bin` dos anexos.

| Coluna | Tipo | Nulo | O que é |
|---|---|---|---|
| `id` | `Uuid` | não | **PK** |
| `mensagem_id` | `Uuid` | não | **FK → `correio_mensagens.id`**, `ao_excluir: restringir` |
| `nome` | `Str(255)` | não | nome do arquivo. **É o AAD do selo do anexo** (autenticado) |
| `conteudo` | `Bin` | não | `[sal_kdf 16][nonce 12][tag 16][ct …]` — o anexo cifrado, no `.bin` |
| `tamanho_claro` | `Int8` | não | tamanho do conteúdo em claro (para a tela, sem decifrar) |

**Chaves e índices:**

- PK `id`.
- Índice `mensagem_id` (**lado filho** da FK) — e `correio_mensagens.id` PK é o
  **lado mãe**. Os dois lados indexados: é a lei "chave conferida precisa de
  índice dos dois lados" (§4).

---

## 3. A cifra: o que o motor faz e o que o correio faz (e por que não se misturam)

Esta é a decisão de DBA que mais importa não errar, porque errá-la dá **falsa
sensação de segurança** sem quebrar nenhum teste.

O motor tem uma cifra de coluna (o "cofre", PSCH `.reg` v5): marca-se uma coluna
como dado pessoal, liga-se `cifra.ligada`, e o motor sela aquela faixa **sob uma
única chave derivada da senha do banco** (`docs/FORMATO.md` §1, `docs/SEGURANCA.md`).
Isso **não serve** para o correio, e a razão é o modelo:

- O cofre do motor sela sob **uma** chave (a do banco). O correio precisa selar a
  privada de cada conta sob a senha **daquele usuário** — chaves diferentes por
  linha. Nenhuma coluna marcada faz isso.
- Marcar `priv_selada` como pessoal **cifraria de novo**, sob a chave do banco,
  um dado que já está selado sob a senha do usuário — dupla cifra sem ganho, e
  pior: quem tem a senha do banco (o administrador) passaria a poder **abrir a
  camada de fora** de um material que o desenho quer opaco até para ele.

Portanto: **as colunas `priv_selada`, `envelope` e `conteudo` são `Bin` opacas,
NUNCA marcadas como dado pessoal.** A selagem é feita pela camada do correio, com
a `phxsql_core::cifra` já conferida contra vetor (ChaCha20-Poly1305 RFC 8439,
PBKDF2, HKDF, X25519). O motor guarda os bytes e não sabe o que são — é o certo.

**Contraste de nonce, que confirma a separação:** o cofre do motor usa nonce de
**24 bytes** por valor (`docs/FORMATO.md` §1) porque reusa a chave da tabela em
muitas linhas e precisa da folga de 192 bits. O correio usa nonce de **12 bytes**
(RFC 8439) porque **cada mensagem/anexo/camada tem chave própria**, derivada por
HKDF com sal de 16 bytes sorteado por item — sob uma chave usada uma vez, 96 bits
de nonce bastam. São dois mecanismos, e é por isso que um não empresta força ao
outro.

**Sem tipo inline de bytes crus.** Uma consequência de formato que o dono deve
saber: o motor não tem tipo inline de **bytes crus de largura fixa**. `Str(n)` é
UTF-8 (sal e nonce aleatórios não são UTF-8 válido); `Uuid`/`Uuid256` são crus
mas de 16/32 bytes fixos, sem um tamanho de 24 para o nonce do cofre nem
variável para o `ct`. Por isso o material de cifra vai empacotado em **um** `Bin`
por linha, com layout interno documentado acima. A alternativa — propor um tipo
`Bytes(n)` inline — é mudança de formato maior e fica como pergunta menor em §6;
`Bin` resolve hoje sem ela.

---

## 4. As pétreas de integridade aplicadas ao correio

**Nunca se mata o pai que tem filhos (Cascade/Restrict).** Todas as FKs nascem
`ao_excluir: restringir` (é o único que o motor aceite no excluir) e conferidas
(`verificar: true`, e "chave declarada nasce conferida"). Consequências:

- Não se apaga uma `correio_conta` que tem mensagem, confiança, bloqueio ou
  moderação — o motor recusa. Para encerrar uma caixa: limpar os filhos primeiro,
  ou (o caminho natural) **exclusão suave** (`softdeleted`), que some da lista e
  continua restringindo — mãe marcada não aceita filha nova, filha marcada
  continua segurando a mãe (medido, `docs/ACID.md` / pendência 189).
- Não se apaga uma `correio_mensagem` que ainda tem anexo: `ao_excluir:
  restringir` em `correio_anexos.mensagem_id`. Para apagar mensagem **e** anexos,
  apaga-se os anexos e depois a mensagem. **Cascata no excluir não existe em
  PhxSql** — a limpeza é filho-primeiro, de propósito, para que nunca haja anexo
  órfão que ninguém vê.
- `ao_alterar` nasce `cascata` (o padrão), mas como as FKs apontam para `id`
  (imutável, UUID v7), a cascata de alteração é **inerte** aqui — o `id` nunca
  muda. É a forma de deixar a porta fechada sem depender de ninguém lembrar.

**Chave conferida precisa de índice dos dois lados.** Para cada Fk, o motor
recusa a declaração se faltar índice na filha (para responder "alguém aponta para
esta linha?" ao excluir a mãe) ou na mãe (para responder "existe este pai?" ao
gravar a filha). O inventário dos índices propostos, lado a lado:

| FK (filha → mãe) | índice na filha | índice na mãe |
|---|---|---|
| `confiancas.de_id → contas.id` | `de_id` | PK `id` |
| `confiancas.para_id → contas.id` | (`para_id`,`estado`) | PK `id` |
| `bloqueios.dono_id → contas.id` | UNIQUE(`dono_id`,`quem_id`) | PK `id` |
| `bloqueios.quem_id → contas.id` | `quem_id` | PK `id` |
| `moderacoes.de_id → contas.id` | `de_id` | PK `id` |
| `moderacoes.alvo_id → contas.id` | (`alvo_id`,`estado`) | PK `id` |
| `mensagens.de_id → contas.id` | `de_id` | PK `id` |
| `mensagens.para_id → contas.id` | (`para_id`,`lida`) | PK `id` |
| `anexos.mensagem_id → mensagens.id` | `mensagem_id` | PK `id` |

**A ordem de digitação é sagrada; o `.reg` nunca reusa slot excluído.** Cai como
uma luva no correio, que é por natureza um diário: mensagem apagada libera o slot
mas nunca o reaproveita, e a `rownum` (ordem de chegada, nunca reaproveitada)
dá a linha do tempo da caixa e a paginação por bissecção, sem índice novo. O
único ponto que **atualiza no lugar** (não anexa) é `correio_confiancas` no
re-pedido — e isso não fere a ordem de digitação, que é sobre slots físicos, não
sobre o estado lógico de uma linha que já existe.

**Senha nunca em texto puro.** Não está em coluna, log, nem resposta de
protocolo. O que existe no disco: `publica` em claro (não é segredo) e a
`priv_selada` (só abre com a senha, via AEAD). É o análogo do teste da casa "a
ficha de usuário não vaza o hash": o teste de prova real do correio (papel F,
quando gravar) tem de **falhar** se algum caminho gravar a privada em claro ou o
`ct` de uma mensagem contiver o texto claro (a cognição já mediu: 77 bytes de
ciphertext, o claro não aparece nos bytes).

---

## 5. A coluna de data/hora de sistema por linha (proposta de PSCH v10)

Decisão do dono, 11/09/2026: *nasce uma coluna de data/hora de sistema por linha,
e no commit o pai é carimbado com instante estritamente anterior ao do filho.*
É mudança de formato — entra cedo, é do DBA.

**Proposta concreta:**

- Nasce a coluna de sistema **`rowts`**, `Int8` (i64), **não nula**, em
  **microssegundos** desde 1970-01-01T00:00:00Z, preenchida pelo motor (como a
  `rownum`), **no fim da lista de colunas**, depois da `rownum`.
- Ela **não** é o tipo `DateTime` do usuário — e isto é decisão, não descuido: o
  `DateTime` do motor é i64 em **milissegundos** (`crates/phxsql-core/src/datahora.rs`).
  O microssegundo é preciso demais para o milissegundo do `DateTime`, e reusar o
  `DateTime` reinterpretando a unidade quebraria toda a conversão de calendário
  que já existe. `Int8` cru, com a unidade documentada na coluna de sistema, é o
  mesmo padrão da `rownum` (um `UInt8` com significado de "ordem de chegada").
- **PSCH v10 = v9 + a coluna de sistema `rowts` no fim.** A leitura passa a
  aceitar 2..=10; **escrever, só na 10**.

**O que custa:**

- **+8 bytes por linha, em TODA tabela** (não só as do correio): `slot_size`
  cresce 8, uma vez. O endereço continua saindo de conta
  (`offset = data_offset + (rowid−1)·slot_size`).
- **Não é migração automática.** Pela mesma convenção que `softdeleted` (v4) e
  `rownum` (v5): tabela gravada em v9 **continua v9**, sem `rowts` — sintetizar a
  coluna na leitura deslocaria os offsets de toda linha antiga, silenciosamente,
  porque o CRC do slot continuaria batendo. A garantia "pai antes do filho no
  dado" vale para o que nasce em v10 daqui em diante. `acrescentar_coluna` numa
  tabela v9 é caminho separado, se o dono quiser.
- O `.log`, a réplica e o PITR **não** ganham nada novo: a `rowts` é dado inline
  do slot, viaja no payload como qualquer coluna.

**A garantia "pai estritamente antes do filho", e onde ela mora:**

- O **formato** (papel C) entrega a coluna. A **garantia** é do gestor de
  transações (papel A/B) no commit — por isso A é convocado na integração.
- Proposta de regra, para não repetir um erro **já medido**: o carimbo de parede
  **não é monotônico** (medido na pendência 232: um `[t, t−25anos, t]` real,
  porque o bidirecional carimba com o relógio do outro servidor). Se `rowts`
  fosse `agora_us` puro, dois relógios ou um salto de NTP dariam pai e filho
  fora de ordem. Então o motor deve atribuir `rowts` como
  **`max(agora_us, ultimo_rowts_do_servidor + 1)`** — um contador monotônico por
  servidor, preso ao relógio de parede quando ele avança. Como o PhxSql já impõe
  *o pai empilhado é visível antes do filho* dentro da transação, o commit já
  processa pais antes de filhos; atribuir `rowts` estritamente crescente nessa
  ordem dá **pai < filho** sempre, mesmo dois na mesma microssegundo de relógio.
- **Na réplica**, `rowts` de uma escrita que veio de outro servidor é o valor
  **de origem** (como o carimbo do `.log`), não o relógio local — senão o "mais
  recente vence" se decidiria pelo relógio errado. É item a confirmar com o dono
  junto do desenho da réplica.

---

## 6. Perguntas ao dono — as que travam o gravar

Enquanto estas não tiverem resposta, o formato **não congela**, porque cada uma
muda uma coluna, uma tabela ou uma unidade — e mudar depois de haver dado vira
migração.

1. **Origem da chave Masson.** O protótipo trata Masson como uma frase (string →
   PBKDF2) fornecida **na leitura**, e no disco só guarda o `sal_masson`. Preciso
   saber o que Masson é de verdade, porque decide se nasce **coluna ou tabela**:
   - É segredo do **segundo usuário** (algo que só o destinatário sabe)? Então
     nada novo no disco além do sal — igual ao alto segredo.
   - É chave **da empresa** (compartilhada, custodiada)? Então precisa de um lugar
     selado para ela (uma `correio_chaves_empresa`?), e isso é formato novo.
   - É um **algoritmo/KDF** específico (não o PBKDF2 da casa)? Então muda a
     derivação, e "zero dependências" pede que seja escrito e conferido contra
     vetor antes de entrar.

2. **Existe recuperação de senha?** Hoje "sem senha não abre" é literal: a
   `priv_selada` só abre com a senha, e senha perdida = **histórico perdido**,
   conta nova. Se o produto precisa recuperar senha, isso **enfraquece a
   garantia** e exige desenho agora (não depois): uma chave de recuperação/escrow
   (custodiada pela empresa? por N administradores?) que também sela a privada —
   o que é **coluna nova em `correio_contas`**, portanto decisão de formato cedo.

3. **Sigilo futuro (forward secrecy / ratchet)?** A identidade X25519 é
   **estática**: vazou a privada de um lado, vaza **todo** o histórico daquele
   par. Um ratchet estilo Signal compra sigilo futuro, mas muda o formato de
   fundo — chaves efêmeras por mensagem e **estado de ratchet por conversa**
   (uma `correio_ratchet(par, estado)`), além de outras colunas em
   `correio_mensagens`. Migrar isso depois de haver mensagem é caro. Decidir
   agora: aceita-se estático na v1 (marcado, como a cognição já registra) ou já
   nasce com ratchet?

4. **Revogação de confiança, e o histórico já trocado.** Solicitar e aceitar
   existem; **retirar** a confiança, não. Duas decisões acopladas:
   - O estado é um 4º valor (`revogada`) na mesma linha (preserva histórico, não
     fere a ordem de digitação) ou apaga a linha?
   - E o que acontece com o que **já foi trocado**? As chaves E2E não mudam, então
     tecnicamente as mensagens antigas continuam decifráveis pelos dois — revogar
     é uma regra de **aplicação** (não entrega novas), não uma que reescreve o
     passado. Confirmar que é isso, e não "apagar o histórico".

5. **O teto de 50 pendentes.** É número **escolhido, não medido** (a cognição diz
   isso). Antes de virar constante no fonte: é por conta, fixo? por domínio?
   configurável no `config.json`? Proponho **configurável** (campo de config, com
   padrão), para não repetir "número citado é número que não se mede" — e medir
   com uso real depois.

**Perguntas menores (não travam o formato, mas quero registrado):**

6. **Bloqueio de domínio inteiro** — abrir `correio_bloqueios_dominio` ou tratar
   como política? (fica fora até a palavra do dono).
7. **O AAD do selo usa endereço ou `id`?** O modelo provado usa o **endereço** no
   AAD. Como as FKs apontam para `id`, o endereço é resolvido da conta carregada
   (custo zero — o remetente/leitor já carrega as duas contas para o ECDH). Mas
   isso **fixa o endereço como identidade imutável**: renomear endereço quebraria
   a decifra do histórico. Confirmar que endereço não renomeia (renomear = caixa
   nova), ou migrar o AAD para `id` antes de gravar qualquer mensagem.
8. **Tipo `Bytes(n)` inline?** Só se o dono quiser material de cifra inline em vez
   de `Bin`. Não é preciso; `Bin` resolve. Registro por completude.

---

## 7. Resumo

**As seis tabelas, em uma frase cada:**

- `correio_contas` — uma linha por caixa: `id` (PK), `endereco` (único), `empresa`
  (índice), a pública X25519 em claro (`Uuid256`) e a privada selada sob a senha
  (`Bin` opaco, 76 bytes).
- `correio_confiancas` — o pedido de confiança por par (`de_id`,`para_id` únicos),
  com estado `pendente/aceita/recusada`, atualizado no lugar no re-pedido.
- `correio_bloqueios` — o "nunca mais" por par (`dono_id`,`quem_id` únicos),
  distinto de recusar.
- `correio_moderacoes` — banir/quarentena/multa com `motivo` obrigatório (CHECK),
  estado `aberta/deferida/indeferida`, decisão com motivo e link só na multa.
- `correio_mensagens` — de/para/prioridade/tipo em claro (prioridade e tipo no
  AAD), e o envelope cifrado num `Bin`; os anexos apontam para ela, ela não os
  lista.
- `correio_anexos` — o armazém à parte, tabela própria: `mensagem_id` (FK,
  restringir), `nome` (AAD) e o conteúdo cifrado num `Bin` no `.bin` próprio.

**As decisões que travam o gravar** (nenhum byte antes destas):

1. **O que é a chave Masson** — segredo do segundo usuário, chave da empresa ou
   algoritmo? Decide se há coluna/tabela nova.
2. **Recuperação de senha existe?** Se sim, exige chave de escrow em
   `correio_contas` — coluna de formato, cedo.
3. **Forward secrecy agora ou depois?** Ratchet muda o formato de fundo
   (estado por conversa, chaves por mensagem).
4. **Revogação de confiança**: 4º estado `revogada` (preserva histórico) e a
   regra do que já foi trocado.
5. **A unidade e a regra da `rowts`** (PSCH v10): `Int8` micros, monotônica por
   servidor (`max(agora, último+1)`), valor de origem na réplica — confirmar.

Os itens menores (bloqueio de domínio, AAD por endereço vs `id`, tipo `Bytes(n)`)
não travam o formato, mas ficam registrados para não voltarem sem decisão.

Ao aprovar, o passo seguinte é B escrever o PSCH v10 e as seis tabelas, com F
provando cada garantia nos dois sentidos, e A integrando a `rowts` com o gestor
de transações. Antes disso, nada se grava.

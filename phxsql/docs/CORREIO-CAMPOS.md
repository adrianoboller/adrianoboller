# CORREIO — campos dos registros (papel C — DBA)

Spec dos registros do correio, aterrada no que já roda (`correio-e2e`/`niveis`/
`masson`, bateria `servermail-ciclo`, `CLUSTER.md`, `REPLICACAO.md`,
`CORREIO-DNS.md`). **Esta é a fonte canônica**; o mapa visual
(`docs/dossie/mapa-campos-correio.html`) é um retrato dela.

> **Mudança de formato entra cedo.** Enquanto não há dado em produção, mudar é
> barato; depois vira migração. Campos marcados **[decidido]** já rodam;
> **[a decidir]** pedem sua palavra antes de gravar byte (o PSCH a congelar).

## Convenções que valem para todos os registros

- **`id` = UUID v7** em **empresa, servermail, cluster, usuário/cliente,
  e‑mail, anexo, confiança**. Ordem temporal embutida; é o id guardado na WX.
  *(Decisão do dono: «id server e cliente = uuid v7».)*
- **`sistema_dt` = data/hora completa em GMT (UTC).** Uma coluna de data/hora de
  sistema por linha; no commit o **pai é carimbado estritamente anterior ao
  filho**. **[a decidir]** o encode em disco (epoch em ms UTC, ou ISO‑8601 `Z`).
- **RESTRICT** em toda chave: nunca se mata o pai com filhos; a chave **nasce
  conferida**; **índice dos dois lados**.

## Onde mora cada registro (topologia) — decisão do dono, 12/09

> *«No servidor phxmail.com.br só guarda os dados do servermail. Os usuários
> cadastrados nos servermail não ficam armazenados no servidor phxmail, apenas
> entre os servermails pela relação de confiança.»*

O servidor central **`phxmail.com.br` guarda só os dados de servermail** — o
**diretório** dos nós da rede (`servermail` + `cluster`) e, no Cloudflare, o DNS
que mapeia `empresa.phxmail.com.br → IP`. **Não** guarda usuário, nem
empresa‑com‑dados, nem e‑mail.

Os **usuários, empresas, caixas e e‑mails moram em cada servermail** — não no
central. Um usuário existe no servermail que o cadastrou, e em nenhum outro
lugar. Entre servermails, a ligação é a **relação de confiança** (federação): o
server X acha o Y pelo DNS, conecta direto na porta 8000 e entrega o **blob
cifrado**; Y guarda na caixa do destinatário e também não lê. **Não há repasse
central** de usuário nem de mensagem.

| Registro | Onde mora |
|---|---|
| `servermail`, `cluster` | **diretório central** (`phxmail.com.br`) |
| DNS `empresa.phxmail.com.br → IP` | Cloudflare (gerido pelo binário) |
| `empresa`, `usuario`, `email`, `anexo`, `confiança` | **em cada servermail** (nunca no central) |
| `coligação` (federação servermail↔servermail) | nos dois servermails que se ligam |

**Consequência de privacidade:** não há **honeypot central**. Quem invade o
`phxmail.com.br` acha a lista de servidores, **não** a de usuários — que nem
está lá. É a mesma lei do «servidor não lê o conteúdo», agora no eixo de *onde o
dado mora*, não só de *quem consegue lê‑lo*.

**Dois níveis de confiança, não um.** A `confiança` (§6) é **usuário↔usuário**
(anti‑spam, com pix/categoria). A **`coligação`** é **servermail↔servermail (ou
empresa↔empresa)** — a federação que deixa dois servidores trocarem mensagem;
foi provada na bateria `servermail-ciclo` (tabela `coligacoes`, RESTRICT dos
dois lados). Sem coligação entre os servidores, nem a confiança entre usuários
entrega.

## Três pétreas que mandam nestes campos

1. **O servidor não lê o conteúdo.** O e‑mail se parte em **metadado** (o
   servidor vê para rotear/cobrar/moderar) e **blob E2E** (guarda e nunca
   decifra). Corpo e assunto não são colunas legíveis — são ciphertext.
2. **Senha nunca em claro.** O usuário **não tem coluna de senha**: só o `sal` e
   o material **selado** por ela (PBKDF2, 200k). Autenticar é abrir esse material.
3. **RESTRICT** (regra primordial da integridade), como acima.

---

## 1. Empresa (tenant)

| Campo | Tipo | Chave / índice | Nota |
|---|---|---|---|
| `id` | UUID v7 | **PK** | |
| `nome` | texto | | razão social (DADO — nunca estilizado) |
| `cnpj` | texto (14 díg) | **única** | pessoa jurídica; **validado (mod‑11)** — obrigatório |
| `cpf_responsavel` | texto (11 díg) | | pessoa física responsável; **validado (mod‑11)** — obrigatório |
| `rotulo` | texto | **única** | rótulo DNS (`prado`, `timeagil`); vira o hostname |
| `cidade` / `uf` | texto | | metadado |
| `servermail_id` | UUID v7 | **FK → servermail (RESTRICT)** | qual nó hospeda |
| `estado` | enum | índice | `ativa` / `implantando` / `suspensa` |
| `armazenado` | inteiro | | bytes (blobs cifrados) |
| `contas` | inteiro | | contagem derivada (funcionários) |
| `sistema_dt` | data/hora GMT | | criação |

RESTRICT: **empresa com contas não se apaga** (já provado em `servermail-ciclo`).
O registro DNS da empresa (`rotulo.phxmail.com.br`) vive no `CORREIO-DNS.md`.
*(O `phxsql.com.br` foi removido em 12/09 — só o `phxmail.com.br`.)*

**Cadastro (decisão do dono, 12/09):** empresa **não** cadastra sem **CNPJ e CPF
válidos** (dígito verificador conferido, mod‑11) **e** com **qualquer campo
vazio**. A recusa é na **declaração** (cedo), como «chave nasce conferida». CPF e
CNPJ são **PII que o servidor vê** (precisa validar) — vivem no servermail, na
trilha LGPD, **não** são E2E. Provado em `crates/phxsql-core/examples/
correio-documentos.rs` (22/22 VERDE, vetores conferidos à mão).

## 2. Servermail (nó da rede)

| Campo | Tipo | Chave / índice | Nota |
|---|---|---|---|
| `id` | UUID v7 | **PK** | «id server = uuid v7» |
| `host` | texto | **única** | `sm-sp-01` |
| `ip_fixo` | IPv4 | **única** | cada servermail num IP fixo (ver CORREIO‑DNS) |
| `porta` | inteiro | | 8000 |
| `pub_no` | 32 bytes | | chave pública pinável do nó (aperto Noise / cifra do fio) |
| `cluster_id` | UUID v7 | **FK → cluster (RESTRICT)** | a que cluster pertence (chave na filha) |
| `papel` | enum | | `lider` / `seguidor` (cluster) · `master` / `slave` (replicação) |
| `cadastro_estado` | enum | índice | `pendente` / `liberado` — o fluxo por e‑mail (§ CORREIO‑DNS) |
| `estado` | enum | | `no_ar` / `fora` / `implantando` |
| `versao` | texto | | versão do binário |
| `sistema_dt` | data/hora GMT | | |

RESTRICT: **servermail com empresas não se apaga**.

## 3. Cluster

| Campo | Tipo | Chave / índice | Nota |
|---|---|---|---|
| `id` | UUID v7 | **PK** | |
| `nome` | texto | | |
| `lider_id` | UUID v7 | **FK → servermail** | o líder eleito |
| `mandato` | inteiro | | termo da eleição (sobe a cada eleição) |
| `modo` | enum | | síncrono ao quórum / assíncrono / … (os 4 modos de `REPLICACAO.md`) |
| `quorum` | inteiro | | nº mínimo para decidir |
| `estado` | enum | | `saudavel` / `sem_lider` / `failover` |
| `ultima_eleicao` | data/hora GMT | | |
| `sistema_dt` | data/hora GMT | | |

A **filiação** é `servermail.cluster_id` (chave na filha, como manda a casa): o
cluster pergunta aos nós «quem é meu?», não guarda a lista embutida.

---

## 4. Usuário (a conta / o cliente)

### 4.1 Identidade e chaves

| Campo | Tipo | Chave / índice | Nota |
|---|---|---|---|
| `id` | UUID v7 | **PK** | «cliente = uuid v7» |
| `endereco` | texto | **única** | `local@empresa.dominio` — é o login |
| `cpf` | texto (11 díg) | | **validado (mod‑11)** — obrigatório; **nenhum user sem CPF** |
| `empresa` | UUID v7 | **FK → empresas (RESTRICT)** | índice dos dois lados |
| `estado` | enum | índice | `ativo` / `banido` / `quarentena` |
| `idioma` | enum | | pt/en/es/fr/de/it |
| `sistema_dt` | data/hora GMT | | |

### 4.2 Segurança — nada em claro

| Campo | Tipo | Nota |
|---|---|---|
| `sal` | 16 bytes | sal do PBKDF2 (ITER=200.000) |
| `verificador_login` | 32 bytes | verificador PBKDF2 do login/canal — **não** a senha; teste falha se vazar. **[a decidir]** |

**Não existe** campo `senha`.

### 4.3 Certificados selados (a leiga nunca vê o p12)

Pública clara, privada **selada sob a senha** (PBKDF2 → ChaCha20‑Poly1305):
`identidade` (X25519), `p12` (nível 2), `p12‑b` (nível 3), `assinatura`
(Ed25519, respostas assinadas).

### 4.4 Maçonaria (opcional)

`masson_loja` (FK/índice) · `masson_id` (a chave da 3ª camada) ·
`masson_credencial` (Ed25519 da Loja, validada contra a pública conhecida).

---

## 5. E‑mail (a mensagem)

### 5.1 Metadado — o servidor VÊ

| Campo | Tipo | Chave / índice | Nota |
|---|---|---|---|
| `id` | UUID v7 | **PK** | |
| `de` | UUID v7 | **FK → usuarios (RESTRICT)** | remetente |
| `para` | UUID v7 | **FK → usuarios (RESTRICT)** | destinatário; **índice** da caixa |
| `prioridade` | enum | | alta / media / baixa |
| `tipo` | enum | | alerta 🚨 / aviso 🔔 / normal |
| `nivel` | enum | | 0 / 1 / 2 / 3 / masson — **autenticado no AAD** (`de\|para\|nivel`) |
| `tamanho` | inteiro | | bytes do blob (o servidor mede, não lê) |
| `estado` | enum | índice | `na_caixa` / `lida` / `na_lixeira`. **Não existe `rascunho`**; esvaziar a lixeira é **purga** total (sem `.trash`/`.reason`/log) — ver `CORREIO-PRIVACIDADE.md` |
| `sistema_dt` | **data/hora completa GMT** | índice | enviado; pai estritamente anterior ao filho |
| `anexos` | lista de refs | | ids de anexos (armazém à parte) |

> **NÃO existe `cc` nem `cco`.** *(Decisão do dono.)* O e‑mail é **de → para**
> (um destinatário). É coerente com o E2E: cada destinatário precisa do seu
> próprio blob selado por ECDH; uma lista de cópias seria N blobs distintos, e o
> «oculto» (cco) num sistema fim‑a‑fim é ilusão de privacidade. Quem quer mandar
> para vários manda N e‑mails, cada um cifrado para o seu dono.

### 5.2 Conteúdo E2E — o servidor GUARDA mas NÃO decifra

| Campo | Tipo | Nota |
|---|---|---|
| `assunto_ct` (+ nonce/tag) | bytes | **assunto cifrado** — decifrado no cliente para a lista e para a busca (§5.4) |
| `nonce` / `sal_kdf` | bytes | abrem a camada E2E (HKDF sobre o segredo ECDH) |
| `sal_extra` / `sal_masson` | bytes | camadas dos níveis 1 e Masson |
| `ct` | bytes | **corpo cifrado** (ChaCha20‑Poly1305) — corpo já **ASCII** antes de cifrar |
| `tag` | 16 bytes | tag AEAD — pega adulteração de prioridade/nível pelo AAD |

### 5.3 Anexo

`id` (UUID v7, PK) · `nome` (**acentos preservados**, byte a byte) · `tamanho` ·
`sal_kdf`/`nonce`/`ct`/`tag` (cifrado à parte, mesma disciplina E2E).

### 5.4 Assunto e busca — «ter assunto e ser pesquisável»

O **assunto** é campo próprio, **E2E** (`assunto_ct`), para o cliente mostrá‑lo
na lista sem abrir o corpo inteiro. **A busca é no CLIENTE**: ele decifra e
alimenta o `.fts` local (a casa já tem FTS). Assim é pesquisável **sem** o
servidor ler nada — a pétrea 1 fica de pé. Busca no servidor exigiria assunto em
claro (quebra a pétrea) e **não** é o caminho recomendado.

---

## 6. Confiança (a relação)

| Campo | Tipo | Chave / índice | Nota |
|---|---|---|---|
| `id` | UUID v7 | **PK** | |
| `de` | UUID v7 | **FK → usuarios (RESTRICT)** | quem pede; **índice** «pedi» |
| `para` | UUID v7 | **FK → usuarios (RESTRICT)** | quem aceita; **índice** «recebi» |
| `estado` | enum | | `pendente` / `aceita` / `recusada` |
| `bloqueada` | bool | | o «nunca mais» (recusar ≠ bloquear) |
| `categoria` | enum | | familia/amigos · negocios · atenção baixa/moderada/total |
| `pix_chave` | texto | | chave pix de quem cobra |
| `pix_valor` | **inteiro (centavos)** | | pix é inteiro, **nunca `f64`**; split Masson também em centavos |
| `seguro_alto` | bool | | pediu p12 (nível ≥ 2) |
| `sistema_dt` | data/hora GMT | | criação / aceite |

---

## 7. O que falta você decidir (antes de gravar byte)

1. **Encode do `sistema_dt`** em GMT: epoch‑ms UTC ou ISO‑8601 `Z` — e o
   carimbo pai‑antes‑do‑filho no commit.
2. **`verificador_login`** entra agora ou fica para o #159.
3. **Congelar o PSCH** dos sete registros (empresa, servermail, cluster,
   usuário, e‑mail, anexo, confiança).
4. **O «meio» do cadastro via Cloudflare** (o binário falando HTTPS) — a pétrea
   zero‑deps × TLS, agora **viva**. Ver `CORREIO-DNS.md` §5 e o fluxo de cadastro.
5. **O modo «purga» por tabela** (e‑mail e anexo: sem `.trash`, sem `.reason`,
   sem log; `excluir` zera os bytes) — é o oposto do modelo forense padrão, uma
   exceção declarada. Ver `CORREIO-PRIVACIDADE.md`.

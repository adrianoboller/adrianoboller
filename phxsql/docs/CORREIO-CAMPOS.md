# CORREIO — campos do usuário e do e-mail (papel C — DBA)

Spec dos dois registros centrais do correio, aterrada no modelo que já roda nos
exemplos `correio-e2e`/`correio-niveis`/`correio-masson` e na bateria
`servermail-ciclo`. **Esta é a fonte canônica**; o mapa visual
(`docs/dossie/mapa-campos-correio.html`) é um retrato dela.

> **Mudança de formato entra cedo.** Enquanto não há dado em produção, mudar é
> barato; depois vira migração. Por isso os campos abaixo estão marcados
> **[decidido]** (já roda) ou **[a decidir com o dono]** (formato PSCH a
> congelar antes de gravar byte). O `id` UUID v7, a data/hora de sistema por
> linha e o verificador de login são os que ainda pedem sua palavra.

## Três pétreas que mandam nestes campos

1. **O servidor não lê o conteúdo.** O e-mail se parte em **metadado** (o
   servidor vê para rotear, cobrar e moderar) e **conteúdo E2E** (um blob que o
   servidor guarda e **nunca** decifra). O corpo não é coluna legível — é
   ciphertext. O que não se analisa vira `tamanho em bytes`.
2. **Senha nunca em claro.** O usuário **não tem coluna de senha**. Guarda-se o
   `sal` (PBKDF2, ITER=200_000) e o material **selado** pela senha; autenticar é
   conseguir abrir esse material. Há teste que falha se a ficha vazar hash/chave.
3. **RESTRICT (regra primordial da integridade).** `usuario.empresa` → empresas
   e `email.de`/`email.para` → usuarios são chaves conferidas: **nunca se mata o
   pai que tem filhos**, a chave **nasce conferida**, e há **índice dos dois
   lados** (na mãe para «existe este pai?», na filha para «alguém aponta para
   esta linha?»).

---

## 1. Usuário (a conta)

### 1.1 Identidade e chaves de tabela

| Campo | Tipo | Chave / índice | Nota |
|---|---|---|---|
| `id` | UUID v7 | **PK** | ordem temporal; é o id guardado na WX. **[a decidir]** (hoje o exemplo usa `endereco` como chave natural) |
| `endereco` | texto | **única** | `local@empresa.dominio`; é o login |
| `empresa` | texto/uuid | **FK → empresas (RESTRICT)** | índice dos dois lados; chave nasce conferida |
| `estado` | enum | índice | `ativo` / `banido` / `quarentena` (moderação) |
| `idioma` | enum | — | preferência multilíngua (pt/en/es/fr/de/it) |
| `sistema_dt` | data/hora | — | data/hora de sistema por linha (decisão 11/09). **[a decidir]** o formato |

### 1.2 Segurança — nada em claro

| Campo | Tipo | Nota |
|---|---|---|
| `sal` | 16 bytes | sal do PBKDF2 da senha (ITER=200_000) |
| `verificador_login` | 32 bytes | verificador PBKDF2 para o login/amarração ao canal — **não** a senha. **[a decidir]**: o exemplo autentica abrindo a chave; um login explícito pede este campo, com teste que falha se ele vazar no protocolo |

> **Não existe** campo `senha`. Quem quer autenticar prova que abre a chave
> privada (ou bate no `verificador_login`). A senha nunca é gravada, nem em log,
> nem em resposta do protocolo.

### 1.3 Certificados selados (a leiga nunca vê o p12)

Todos gerados sozinhos e **selados sob a senha** (PBKDF2 → ChaCha20-Poly1305).
Da ficha só sai a **pública**; a privada só existe cifrada.

| Par | Pública (clara) | Privada (selada) | Para quê |
|---|---|---|---|
| identidade | `pub_id` (X25519) | `priv_id_cif` + `nonce_id` + `tag_id` | E2E padrão (ECDH) |
| p12 | `pub_p12` (X25519) | `priv_p12_cif` + `nonce`/`tag` | nível 2 (segurança alta) |
| p12-b | `pub_p12b` (X25519) | `priv_p12b_cif` + `nonce`/`tag` | nível 3 (p12 + p12) |
| assinatura | `pub_sig` (Ed25519) | `priv_sig_cif` + `nonce`/`tag` | respostas assinadas |

### 1.4 Maçonaria (opcional)

| Campo | Tipo | Nota |
|---|---|---|
| `masson_loja` | texto/uuid | qual Loja emitiu — `None` se não é Masson |
| `masson_id` | bytes | o **id da Maçonaria** (chave da 3ª camada no nível Masson) |
| `masson_credencial` | assinatura Ed25519 | emitida pela Loja; validada contra a **pública conhecida** da loja |

---

## 2. E-mail (a mensagem)

Parte-se em duas metades por causa da pétrea 1.

### 2.1 Metadado — o servidor VÊ (rotear, cobrar, moderar)

| Campo | Tipo | Chave / índice | Nota |
|---|---|---|---|
| `id` | UUID v7 | **PK** | ordem temporal |
| `de` | texto/uuid | **FK → usuarios (RESTRICT)** | remetente |
| `para` | texto/uuid | **FK → usuarios (RESTRICT)** | destinatário; **índice** para a caixa de entrada |
| `prioridade` | enum | — | `alta` / `media` / `baixa` |
| `tipo` | enum | — | `alerta` 🚨 / `aviso` 🔔 / `normal` |
| `nivel` | enum | — | `0` padrão / `1` frase OOB / `2` p12 / `3` p12+p12 / `masson` — **autenticado no AAD** (`de\|para\|nivel`): não se rebaixa calado |
| `tamanho` | inteiro | — | bytes do blob (o que o servidor mede em vez de ler) |
| `estado` | enum | índice | `na_caixa` / `lida` / `na_lixeira` (exclusão suave — a marca rosa que volta) |
| `sistema_dt` | data/hora | — | data/hora de sistema por linha (pai **estritamente anterior** ao filho). **[a decidir]** o formato |
| `anexos` | lista de refs | — | ids de anexos (armazém à parte) |

### 2.2 Conteúdo E2E — o servidor GUARDA mas NÃO decifra (blob)

| Campo | Tipo | Nota |
|---|---|---|
| `nonce` | 12 bytes | nonce da camada E2E |
| `sal_kdf` | 16 bytes | deriva a chave da camada (HKDF sobre o segredo ECDH) |
| `sal_extra` | 16 bytes | camada do nível 1 (frase por telefone/SMS) |
| `sal_masson` | 16 bytes | camada Masson (3ª camada = id da Maçonaria) |
| `ct` | bytes | **corpo cifrado** (ChaCha20-Poly1305). O corpo já é **ASCII** (sem acento) antes de cifrar |
| `tag` | 16 bytes | tag AEAD — pega adulteração de prioridade/nível pelo AAD |

> A senha do destinatário **nunca vai ao fio**. Só ela + a pública do remetente
> (ECDH) abrem o blob; a assinatura Ed25519 do remetente é conferida na leitura.

### 2.3 Anexo (referenciado pelo e-mail)

| Campo | Tipo | Nota |
|---|---|---|
| `id` | UUID v7 / u64 | **PK**, referenciado por `email.anexos` |
| `nome` | texto | **acentos preservados** — byte a byte (ao contrário do corpo) |
| `tamanho` | inteiro | bytes |
| `sal_kdf` / `nonce` / `ct` / `tag` | bytes | cifrado à parte, mesma disciplina E2E |

---

## 3. Confiança (relação — onde moram pix e categoria)

Não é o e-mail, mas o dono citou pix/categoria, então fica o vizinho:

| Campo | Tipo | Nota |
|---|---|---|
| `de` / `para` | uuid | o par da relação |
| `estado` | enum | `pendente` / `aceita` / `recusada` |
| `categoria` | enum | `familia/amigos` / `negocios` / `atencao baixa` / `moderada` / `total` |
| `pix_chave` | texto | chave pix de quem cobra |
| `pix_valor` | **inteiro (centavos)** | **corrige o `f64` do exemplo** — pix é valor inteiro, nunca ponto flutuante; no Masson, split para a loja também em centavos |
| `seguro_alto` | bool | a relação pediu p12 (nível ≥ 2) |

---

## 4. O que falta você decidir (antes de gravar byte)

1. **`id` UUID v7 como PK** dos três registros (hoje o exemplo usa chave
   natural). É o que o `servermail-ciclo` já provou guardando o v7 no `.reg`.
2. **`sistema_dt`** — a coluna de data/hora de sistema por linha (decisão sua de
   11/09): tipo e tamanho no PSCH, e o carimbo pai-antes-do-filho no commit.
3. **`verificador_login`** — se o login explícito entra agora ou fica para o
   #159 (o servidor de rede na 8000).
4. **Congelar o PSCH** destes campos — é a decisão de formato que, por pétrea,
   se toma **com você** e **cedo**.

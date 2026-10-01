# Plano do TLS 1.3 escrito aqui — pedido 572 (papel J, 01/10/2026)

Para a engenharia começar sem perguntar. Nenhum código aqui; cada número diz se
foi **medido** (comando ao lado) ou **raciocinado**.

---

## 0. A premissa do pedido, medida antes de planejar

A encomenda pedia «o que já existe: SHA-256, HMAC, PBKDF2, a cifra do fio…
Ed25519/X25519? AEAD? HKDF?» — como se o TLS estivesse por começar. **Não está.**
A linha do 572 no `PENDENCIAS.md` está `◐`, e o código confirma:

| fatia | estado | commit | onde |
|---|---|---|---|
| T1 ECDSA/ECDH P-256 | feito 30/09 | `7793945f` | `phxsql-core/src/p256.rs` (797 l.) |
| T2 key schedule + registro | feito 30/09 | `94feb521` | `phxsql-core/src/tls13.rs` (474 l.) |
| T3 X.509 P-256, PEM/SEC1/PKCS#8 | feito 30/09 | (no T4a) | `phxsql-core/src/x509.rs` (714 l.) |
| T4a aperto do **servidor** | feito 30/09 | `c2cb6660` | `phxsql-core/src/tls.rs` (1.441 l.) |
| T4b três portas HTTP | feito 30/09 | `c0f9fa30` | `phxsql-server/src/http.rs` |
| T5 AES-128-GCM tempo constante | feito 30/09 | `51c81401` | `phxsql-core/src/aes.rs` (326 l.) |
| T6a porta de dados 5000 (servidor) | feito 30/09 | na árvore | `phxsql-server/src/fio_dados.rs` |
| **T6b lado CLIENTE** | **não existe** | — | `tls.rs` só exporta `aceitar`; zero `conectar` |

**Medido agora:** `cargo test --offline -p phxsql-core --lib -- tls aes p256 x509 x25519 ed25519 hkdf cifra asn1 sha`
→ **102 passaram, 0 falharam, 1 ignorado** (3,11 s), incluindo os que chamam o
`openssl` do contêiner.

Logo, o plano útil é o **que falta**: o cliente, a verificação de certificado
alheio, os adaptadores de protocolo, a convivência com o Noise — e um defeito de
aleatoriedade que o TLS agravou (§5.1).

---

## 1. Inventário medido no código

| peça | arquivo | norma conferida | estado |
|---|---|---|---|
| SHA-256 | `hash.rs` | FIPS 180-4 | existe |
| SHA-512 | `sha512.rs` | FIPS 180-4 | existe — **SHA-384 não** (outro IV + truncar) |
| HMAC, PBKDF2 | `hash.rs`, `senha.rs` | RFC 4231, vetores PBKDF2 | existe |
| HKDF | `hkdf.rs` | RFC 5869 | existe |
| X25519 | `x25519.rs` | RFC 7748 §5.2 (iterações) | existe |
| Ed25519 | `ed25519.rs` | RFC 8032 | existe |
| ChaCha20-Poly1305 | `cifra.rs` | RFC 8439 | existe (234 MiB/s, registros 16 KiB) |
| AES-128-GCM | `aes.rs` | FIPS 197, GCM casos 1–2, RFC 8448 | existe (5,3 MiB/s — 44× mais lento, 2ª escolha) |
| ECDSA/ECDH P-256 | `p256.rs` | RFC 6979 A.2.5 + openssl nos 2 sentidos | existe |
| DER (ler/escrever) | `asn1.rs` | — | existe (elemento genérico) |
| X.509 **emitir** P-256/Ed25519 | `x509.rs` | openssl x509/verify | existe |
| X.509 **validar cadeia alheia** | — | RFC 5280 §6 | **não existe** |
| RSA (verificar PKCS#1 v1.5 / PSS) | — | RFC 8017 | **não existe** (grep `rsa` em `core/src`: zero) |
| ECDSA P-384 (verificar) | — | FIPS 186-4 | **não existe** |
| TLS 1.3 servidor | `tls.rs` `aceitar`, `FluxoTls` | RFC 8446, RFC 8448 §3 | existe |
| TLS 1.3 **cliente** | — | — | **não existe** |
| Cifra do fio (Noise NX) | `fio.rs` (1.286 l.), `Iniciador`/`Canal` | composição própria, sem vetor de interop | existe; 3 iniciadores: `replica.rs:177`, `servidor.rs:795` (Remoto), `odbc/conexao.rs:486` |
| Aleatoriedade | `senha::bytes_aleatorios`, `cifra::sortear` | — | **Linux ok; Windows degrada — §5.1** |

Consumidores de saída que hoje vão **em claro** e pedem TLS-cliente (grep `TcpStream::connect_timeout`):
`dblink/mysql.rs:484` («Sem TLS», linha 34), `pg/mod.rs:685` («Sem TLS», linha 41; `SCRAM-SHA-256-PLUS` recusado por falta de TLS), `email.rs:180` («nao ha TLS», linha 9).

---

## 2. O mínimo de TLS 1.3 que serve ao PhxSql

### 2.1 O que a RFC obriga (texto oficial, RFC 8446 §9.1–9.2)

> «MUST implement the TLS_AES_128_GCM_SHA256 … SHOULD implement … TLS_CHACHA20_POLY1305_SHA256 …
> MUST support digital signatures with rsa_pkcs1_sha256 (for certificates), rsa_pss_rsae_sha256
> (for CertificateVerify and certificates), and ecdsa_secp256r1_sha256 … MUST support key exchange
> with secp256r1 (NIST P-256) and SHOULD support key exchange with X25519.»
> Extensões obrigatórias: supported_versions, **cookie**, signature_algorithms, **signature_algorithms_cert**,
> supported_groups, key_share, server_name.

Servidor: cumpre tudo **menos RSA**, que no servidor é dispensável (o certificado
dele é P-256 e só ele assina). Cliente: **RSA PKCS#1 v1.5 + PSS é obrigatório**,
e não é luxo — medido no contêiner, as 153 raízes de `/etc/ssl/certs/ca-certificates.crt`
são **113 RSA (74%), 36 P-384 (24%), 4 P-256 (3%)**. Um cliente só-P-256 não
valida 97% da PKI pública. (Comando: separar o PEM e `openssl x509 -text | grep Public-Key`.)

### 2.2 Entra

- Cliente 1-RTT: ChaCha20 (preferido) e AES-128-GCM; X25519 e P-256; HRR **com eco do `cookie`**.
- Tolerar o que servidor honesto manda depois do aperto: **`NewSessionTicket` descartado** (o
  `openssl s_server` manda 2 por padrão — raciocinado da doc do OpenSSL 3.0, conferir na bancada),
  `KeyUpdate`, `ChangeCipherSpec` de compatibilidade (§D.4) ignorado, `CertificateRequest`
  respondido com `Certificate` vazio (§4.4.2).
- Verificação: ECDSA P-256 e Ed25519 (existem), **RSA PKCS#1 v1.5 e PSS-RSAE com SHA-256/384, ECDSA P-384, SHA-384** (novos).

### 2.3 Fica de fora — com a régua

| item | régua | decisão |
|---|---|---|
| **0-RTT** | nenhum dos três liga `early_data` (grep no fonte: PG `be-secure-openssl.c`, MySQL `vio/viosslfactories.cc`, MariaDB idem — zero ocorrências) → **convergência**. E RFC 8446 §8: 0-RTT não tem proteção contra repetição — um `inserir` repetido é dado errado | **fora, definitivo** |
| **Retomada (PSK/tickets)** | PG **desliga** (`SSL_CTX_set_num_tickets(0)` + `SSL_OP_NO_TICKET`, `be-secure-openssl.c:263-276`) = 4; MySQL liga (`ssl_session_cache_mode` padrão ON, doc 8.4 «reusing-ssl-sessions») = 2; MariaDB liga (`SSL_CTX_sess_set_cache_size(…,128)`, `viosslfactories.c:527`) = 3 → **5 × 4, oferecer** | **entra pela régua, nasce `⏸`** (não é defeito). Antes dela: medir *keep-alive* na web, que hoje é `Connection: close` e paga o aperto (2,9 ms mediana, T4b) **por pedido** — keep-alive compra o mesmo sem guardar segredo de ticket |
| **mTLS (certificado do cliente)** | os três autenticam por certificado (PG `clientcert=verify-full`, MySQL/MariaDB `REQUIRE X509`) → **convergência** | **entra por aceite automático, nasce `⏸`** — a encomenda dizia «fora»; a régua desmente, só o prazo é depois |
| Servidor com chave **RSA** | assinar RSA exige exponenciação de tempo constante com segredo — risco alto, e o certbot ≥ 2.0 já emite ECDSA P-256 por padrão (raciocinado, não conferido) | **fora**; recusa nomeando «use P-256» |
| TLS 1.2 | pedido diz «TLS 1.3 escrito aqui» | fora |
| TLS_AES_256_GCM_SHA384 | SHOULD, não MUST | fora; servidor que só aceite AES-256 é lacuna declarada |
| CRL/OCSP | PG só confere CRL se o arquivo existir; MySQL `--ssl-crl` opcional → nenhum confere por padrão | fora por padrão |

---

## 3. Vetores oficiais, peça por peça

| peça | vetor | situação |
|---|---|---|
| key schedule, registro | RFC 8448 §3 (23 constantes, já extraídas por script) | **existe** |
| **cliente**: transcrição e `Finished` | RFC 8448 §3: dado o `ClientHello` e a privada X25519 do cliente **do traço**, alimentar o voo do servidor e conferir o `Finished` do cliente byte a byte | novo (T6b-1) |
| **cliente**: HRR | RFC 8448 §5 | novo |
| X25519 | RFC 7748 §5.2, §6.1 | existe |
| Ed25519 | RFC 8032 §7.1 | existe |
| HKDF | RFC 5869 apêndice A | existe |
| ChaCha20-Poly1305 | RFC 8439 §2.8.2, A.5 | existe |
| AES-GCM | McGrew–Viega casos 1–2 (existem); **casos 3–4 (com AAD)** e o `gcmtestvectors.zip` do NIST CAVP | completar |
| P-256 | RFC 6979 A.2.5 | existe |
| **P-384** | RFC 6979 **A.2.6**; Wycheproof `ecdsa_secp384r1_sha384_test.json` | novo |
| **SHA-384** | FIPS 180-4, exemplos do NIST CSRC (`SHA384.pdf`) | novo |
| **RSA PKCS#1 v1.5 / PSS** | NIST CAVP FIPS 186-4 `SigVer15_186-3.rsp`, `SigVerPSS_186-3.rsp`; Wycheproof `rsa_signature_2048_sha256_test.json`, `rsa_pss_2048_sha256_mgf1_32_test.json` (casos-limite: enchimento, comprimento) | novo |
| **Validação de cadeia** | **x509-limbo** (C2SP, Apache 2.0) — o conjunto moderno; NIST PKITS como segunda via | novo |
| Nome do servidor | RFC 9525 (substitui a 6125): SAN obrigatório, curinga só no rótulo mais à esquerda | novo |
| interop | `openssl s_server -tls1_3` (OpenSSL **3.0.13 existe** no contêiner, `-groups`, `-sigalgs`); **PostgreSQL 16.13** (`/usr/lib/postgresql/16/bin`, existe) com `ssl=on`; `python3` com `ssl` (TLS 1.3 = True) como servidor SMTP STARTTLS de prova | conferido agora |

Vetor se copia **do texto oficial baixado por script**, como no T2 — nunca de memória.
Wycheproof e x509-limbo são **dados de teste**, não dependência: entram como arquivo em `tests/`, com a licença ao lado.

---

## 4. Certificado: pino ou cadeia? Decidido pela régua

**Hipóteses escritas antes:** (H1) só pino de chave, como o cluster; (H2) só cadeia + nome;
(H3) os dois, por destino.

| comportamento | PG | MySQL | MariaDB | resultado |
|---|---|---|---|---|
| validar cadeia contra arquivo de CAs + nome | `sslrootcert`, `verify-full` | `--ssl-ca`, `VERIFY_IDENTITY` | `--ssl-ca` | **convergência → entra** |
| modo cifrado **sem** verificar | `require` | `REQUIRED` | `--disable-ssl-verify-server-cert` | **convergência → entra** |
| loja do sistema | `sslrootcert=system` (força `verify-full`) | — | Connector/C | não converge; PG 4 + MariaDB 3 = 7 × 2 → **entra** (`"sistema"` = caminhos Linux conhecidos; Windows exige arquivo, sem API da loja só com a `std`) |
| verificar **por padrão** | `prefer` (não verifica) | `PREFERRED` (não verifica) | 11.4: verifica | 4+2 = **6 × 3 → não verificar por padrão** — e casa com «guarda nova entra pedida» |
| pino por impressão digital | — | — | `ssl-fp` / verificação por senha (11.4.1) | só MariaDB; mas é o **comportamento que já temos** (pino do Noise, TOFU) e não se tira de quem usa |

**Decisão (H3), sem subir:**
- **PhxSql ↔ PhxSql** (cmd, ODBC, réplica, cluster, DbLink phx, Remoto): **pino SHA-256 do SPKI**
  (`sha256//<base64>`, a forma do `--pinnedpubkey` do curl), TOFU quando não há pino — o mesmo contrato
  do `chave_do_fio` hoje. Não exige RSA nem cadeia: entra primeiro e barato.
- **Terceiros** (MySQL, PostgreSQL, SMTP): modos `desligado | exigir | verificar`, **padrão `desligado`**
  (configuração existente não muda), `verificar` = cadeia + nome (RFC 9525), `tls_ca` = arquivo PEM ou `"sistema"`.
- A **mesma** função de verificação atende os dois (pino é um atalho antes da cadeia) — não se escreve duas.

---

## 5. Riscos

### 5.1 Aleatoriedade no Windows — defeito ATIVO, e o TLS o agravou

`senha::bytes_aleatorios` lê `/dev/urandom`; **onde ele não existe (Windows, alvo de `empacotar.sh:36`)**
cai em `SHA-256(nanos ‖ contador ‖ pid ‖ endereço de heap)` (`senha.rs:214-260`). Dali saem
`p256::gerar_privada` (`p256.rs:435`, **efêmera do TLS e a chave do autoassinado do T4b**) e a semente
de `cifra::sortear` (`cifra.rs:623`), que gera a **efêmera X25519 do TLS e do Noise** — inclusive no
**driver ODBC**, cujo uso típico é Windows. É o achado **A3** de `auditoria-sec-cripto-2026-09.md`
(«bloqueia para o pacote Windows»), que **não está em pedido nenhum** do `PENDENCIAS.md` (grep: zero).
Com o TLS, quem estima instante de arranque e PID **decifra tráfego gravado**. Raciocinado do código, não
medido no Windows (não há `wine` no contêiner; `bancada/windows/provar.sh` diz como instalar).

Hipóteses: (H1) FFI para `ProcessPrng` — é a mesma chamada que a `std` faz, mas é um `extern` que esta
casa nunca escreveu (grep `#[link`: zero) e a letra da pétrea é «só a `std`»; (H2) **pela própria `std`**:
`HashMap` promete semente «from a high quality, secure source of randomness provided by the host»
(doc da `std`), e no fonte da 1.94.1 a primeira `RandomState::new()` de **cada thread** chama
`hashmap_random_keys()` → `ProcessPrng` (`library/std/src/hash/random.rs:68`, `sys/random/windows.rs`).
Duas threads novas, hashes de entradas fixas, SHA-256 → 256 bits do sistema, só `std`; (H3) recusar
gerar chave no Windows. **Decisão: H2 para semear, H3 quando a extração se repete** (duas threads com
o mesmo resultado = fonte morta → erro, nunca mistura de relógio). A mistura continua servindo **só ao sal**,
que exige unicidade e não segredo. H1 morre por exigir decisão do dono que H2 dispensa. Risco nomeado da H2:
a renovação por thread é detalhe de implementação, não contrato — o teste das duas threads o vigia.

### 5.2 Tempo constante

- Já coberto: P-256 (escada com troca por máscara), AES (S-box calculada, GHASH de 128 passos), X25519, ChaCha.
  Escrito nos módulos que **não é auditoria**.
- **RSA, P-384 e cadeia mexem só em dado público** (verificar, nunca assinar) → aritmética de tempo
  variável é aceitável; isto é o que mantém a fatia pequena. A regra que impede o erro: **nenhum**
  segredo entra no `bigint` novo — conferidor/guarda que reprova `assinar`/`privada` no módulo.
- Comparação de `Finished`/etiqueta/pino: igualdade de tempo constante (já existe no `cifra.rs`; reusar, não reescrever).

### 5.3 Outros

- Teto antes da credencial no cliente também: `Certificate` do servidor com teto (a lição do 434), cadeia com profundidade máxima.
- Extensão **crítica desconhecida → recusa** (RFC 5280 §4.2) — é como `nameConstraints` sai barato.
- Prova contra cliente/servidor real: `openssl s_client`/`s_server` 3.0.13 **existem**; `curl` existe; PG 16 existe; **MySQL não está no contêiner** (as provas de `bancada/dblink/prova-mysql.py` usaram 8.0.46 e não o sobem) — lacuna da T6d.

---

## 6. Fatias, ordem, aceite e prova

Ritmo medido: T1→T6a, seis fatias, saíram em **um dia** (30/09). Estimativa abaixo é **raciocinada** a partir dele.

| # | fatia | aceite | prova real (com defeito reposto) | rodadas |
|---|---|---|---|---|
| **T0** | aleatoriedade sem mistura para chave (§5.1) | material de chave nunca vem da mistura; fonte morta = erro | teste das duas threads; guarda que reprova chamada à mistura fora do sal; `wine` + dois `phxsqld.exe` no mesmo ms → efêmeras distintas | 0,5 |
| **T6b-1** | **cliente TLS 1.3** no core (`tls::conectar`, espelho do `aceitar`), autenticação **só por pino SPKI** | fecha com o nosso servidor, com `openssl s_server` (X25519, P-256, HRR via `-groups P-384:X25519`, tickets, KeyUpdate) | `Finished` do cliente = RFC 8448 §3; HRR = §5; repostos: pino ignorado, `CertificateVerify` sem conferir, `Finished` do servidor sem conferir, rótulo do contexto, ticket tratado como erro, `cookie` sem eco | 1 |
| **T6b-2** | trocar Noise→TLS nos **3 iniciadores** (`replica::Cliente` — que serve réplica, cluster, DbLink phx e `phxsqlcmd` —, Remoto, ODBC) | config `pino_tls`; servidor **continua aceitando Noise** | `tests/cifra-do-fio.rs`, `cluster-cifrado.rs`, `identidade-do-pulso.rs` repassados em TLS; `bancada/cifra-do-fio/prova.py` passo 9 medindo os bytes em TLS | 1 |
| **T6c-1** | `bigint` público + RSA PKCS#1 v1.5/PSS + SHA-384 + ECDSA P-384 (verificar) | os vetores da §3 | CAVP + Wycheproof + RFC 6979 A.2.6; repostos: comprimento de enchimento, `salt_len` do PSS, `s ≥ n` aceito, hash errado no DigestInfo | 1 |
| **T6c-2** | validação de cadeia (RFC 5280 §6, subconjunto) + nome (RFC 9525) + `tls_ca` (arquivo/`"sistema"`) | `openssl s_server` com cadeias raiz→intermediária→folha em RSA e P-384 | x509-limbo (subconjunto declarado); repostos: intermediária com `cA=false`, validade vencida, curinga cobrindo dois rótulos, IP casando `dNSName`, crítica desconhecida aceita | 1–1,5 |
| **T6d** | adaptadores: PG (`SSLRequest`; `SCRAM-SHA-256-PLUS` com `tls-server-end-point`, RFC 5929 §4), MySQL (`CLIENT_SSL`; caminho completo do `caching_sha2_password` por dentro), SMTP (`STARTTLS`, RFC 3207; 465 implícito, RFC 8314) | modos `desligado/exigir/verificar` | PG 16 local com `ssl=on`; SMTP contra servidor `python3 ssl`; MySQL 8 **depende de subir um mysqld** | 1,5 |
| **T6e** | fecho do servidor: 2º `ClientHello` mudando o conjunto após HRR (lacuna declarada no T5), `signature_algorithms_cert`, cadeia com intermediárias no PEM | — | cliente cru do teste; `openssl s_client -showcerts` | 0,5 (paralela à T6b-1) |

**Ordem:** T0 → T6b-1 (‖ T6e) → T6b-2 → T6c-1 → T6c-2 → T6d. **Total raciocinado: 6–7 rodadas.**
T0 primeiro porque é defeito ativo e porque toda chave que as fatias seguintes gerarem no Windows
herdaria a mistura. T6b antes de T6c porque PhxSql↔PhxSql não precisa de RSA nem de cadeia, e já
aposenta a duplicação da §7.

---

## 7. Convivência com a cifra do fio (Noise NX) e o «TLS no transporte»

**Hipóteses:** (H1) TLS substitui o Noise em tudo, com convivência no servidor; (H2) Noise fica para
sempre entre PhxSql, TLS só para terceiros; (H3) os dois opcionais indefinidamente.

- **Régua:** os três maduros cifram cliente **e** replicação por TLS (PG `primary_conninfo`/`sslmode`,
  MySQL `SOURCE_SSL`, MariaDB `MASTER_SSL`) — convergência no meio, e agora nenhuma pétrea se opõe,
  porque o TLS é escrito aqui.
- **Lei da casa:** «função e comando vêm do mesmo motor» — duas cifras de fio são a mesma decisão
  (cifrar, autenticar o servidor, pinar) escrita duas vezes, com duas disciplinas de nonce e dois formatos de pino.
- **Bytes:** o Noise custa **+78,8% / +48,2% / +33,8% / +33,3%** (ping / inserção / lote 5 KiB / resposta
  200 KiB — **medido**, `CIFRA-DO-FIO.md` §6, porque a moldura é Base64). O registro TLS custa **22 bytes
  fixos por até 16 KiB** (5 de cabeçalho + 1 de tipo + 16 de etiqueta, RFC 8446 §5.2) → **+42% / +13% /
  +0,4% / +0,14%** — **calculado da norma, não medido**; decide o passo 9 da `prova.py` rodado em TLS (T6b-2).
- **Interop:** driver de terceiros (Python `ssl`, JDBC, .NET) só fala TLS; Noise é protocolo nosso.

**Decisão: H1.** Clientes novos falam TLS com pino; o servidor **continua aceitando Noise** (o primeiro
byte já separa: `0x16` TLS, `{` JSON — T6a), porque «proteção que quebra todo cliente antigo é estrago».
H2 morre pela lei da duplicação e pelos bytes; H3 morre porque é H2 sem data.
O **«TLS no transporte» das portas**: HTTP (T4b) e dados (T6a) já estão; réplica e cluster usam a porta
de dados e entram pela T6b-2; DbLink/SMTP pela T6d. O **proxy do pedido 239** deixa de ser a resposta,
mas `atras_de_proxy` continua válido para quem já usa.

---

## 8. Matriz de evidência

| fonte | o que resolve | custo |
|---|---|---|
| RFC 8446 §9.1–9.2 (`rfc-editor.org/rfc/rfc8446.txt`, linhas 5688–5760) | o que o cliente tem de saber verificar (RSA!) e as extensões obrigatórias (`cookie`) | T6c-1 inteira |
| RFC 8446 §4.2.10 / §8 | 0-RTT sem proteção contra repetição | nenhum (fica fora) |
| RFC 8448 §3, §5 | prova byte a byte do cliente e do HRR | barato: constantes já extraídas no T2 |
| `/etc/ssl/certs/ca-certificates.crt` (medido) | 113 RSA / 36 P-384 / 4 P-256 | justifica RSA e P-384 |
| PG `be-secure-openssl.c:263-276` (REL_17_STABLE) | tickets desligados | voto 4 contra retomada |
| MySQL doc 8.4 «reusing-ssl-sessions»; `vio/viosslfactories.cc` | retomada ligada no servidor; sem `early_data` | voto 2 |
| MariaDB `vio/viosslfactories.c:527` (11.4); blog «mission impossible: zero-configuration SSL»; doc Connector/C | cache de sessão; verificação por padrão; pino por senha | voto 3 |
| libpq doc (`libpq-ssl`, `libpq-connect`) | `prefer` padrão; `verify-full`; `sslrootcert=system` | régua do §4 |
| MySQL doc 8.4 `connection-options` | `PREFERRED` padrão; `VERIFY_IDENTITY` + `--ssl-ca` | régua do §4 |
| Rust std 1.94.1 `hash/random.rs:68`, `sys/random/windows.rs` | entropia do Windows só com a `std` | T0, meia rodada |
| `auditoria-sec-cripto-2026-09.md` A3 | o defeito já visto e não aberto como pedido | — |

## 9. Recusas medidas (para não voltarem)

- **0-RTT**: repetição de escrita; nenhum dos três liga. Definitivo.
- **Servidor com chave RSA**: assinatura RSA com segredo em tempo constante, sem demanda medida.
- **FFI direto para `ProcessPrng`** (H1 da §5.1): a `std` já entrega a mesma fonte; o `extern` abriria discussão de pétrea sem comprar nada.
- **Noise para sempre** (H2 da §7): duas cifras de fio = decisão duplicada; e +33% de bytes medidos contra ~0,14% calculados.
- **Verificar por padrão contra terceiros**: régua 6 × 3, e guarda nova entra pedida.

## 10. Lacunas

1. Windows não medido (sem `wine`); a §5.1 é leitura de código.
2. Sobrecarga do TLS na porta de dados é **calculada**; o número vem da T6b-2.
3. MySQL 8 não está no contêiner; a T6d-MySQL depende de alguém subir um.
4. Comportamento do `openssl s_server` quanto a tickets por padrão: raciocinado, conferir na T6b-1.
5. Padrão de chave do certbot (P-256 desde 2.0): não conferido em fonte primária.
6. A3 (§5.1) não tem número de pedido — o integrador abre.

## 11. O que sobe ao dono

**Nada bloqueia a engenharia.** Um item de **produto**, não urgente: **quando o servidor deixa de aceitar
o Noise** — é promessa de compatibilidade ao cliente, e a pesquisa não a decide. Até lá, convivência.

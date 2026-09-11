# Correio nativo `.p12` — chaves por par no modelo ECDH

**Papéis:** B (engenheiro), C (DBA/formato), F (prova real).
**Data:** 11/09/2026. **Estado: EM CONSTRUÇÃO.**

Este documento é o plano do arquivo de chaves do correio nativo. A **fatia 1**
(a fundação: codec ASN.1 DER + certificado X.509 Ed25519/X25519, provado contra
o OpenSSL) **está pronta e medida**; a **fatia 2** (o PKCS#12 propriamente dito)
**ainda não existe** — este documento é onde ela se decide antes de gravar.

Lei do projeto: **zero dependências externas.** Tudo abaixo é `std` mais a
cripto escrita e conferida à mão nesta casa. Onde a fatia 2 vai exigir cripto
nova (AES), isto está dito em letra grande na §6 — não é crate, é código a
escrever e provar contra vetor.

## 1. O que o dono decidiu

Cada par de correspondentes tem um arquivo de chaves no formato **PKCS#12
(`.p12`) DE VERDADE** — para que o `openssl`, um chaveiro de sistema ou um
cliente qualquer consigam abrir — mas com **chaves Ed25519/X25519 (RFC 8410)**
em vez de RSA/ECDSA. É `.p12` real e interoperável **sem** quebrar a pétrea das
zero dependências, porque ASN.1/DER, X.509, PKCS#8 e PKCS#12 são só formato, e
formato a gente escreve.

## 2. Duas chaves por dono, cada uma no seu papel

O protótipo em memória (`examples/correio-e2e.rs`) usava **uma** chave X25519
por conta. O modelo `.p12` separa em **duas**, porque assinar e trocar são
coisas diferentes:

| Chave | Norma | Para quê | OID (RFC 8410) |
|-------|-------|----------|-----------------|
| **Ed25519** | RFC 8032 | **assinar** — identidade, avaliza os certificados | `1.3.101.112` (`id-Ed25519`) |
| **X25519** | RFC 7748 | **trocar** (ECDH) — deriva o segredo que cifra a mensagem | `1.3.101.110` (`id-X25519`) |

X25519 não assina nada. Por isso quem assina os **dois** certificados de um dono
é a **Ed25519 dele**: o de identidade é autoassinado de verdade (a chave que
assina é a que ele publica); o de troca publica a X25519 e é assinado pela mesma
Ed25519 — a identidade avaliza a chave de troca.

As duas privadas moram **cifradas sob a senha do dono** (PBKDF2 →
cifra simétrica), como já era no protótipo: sem a senha, não abrem.

## 3. O arquivo por par: `<A>_x_<B>.p12`

Na máquina de **A**, o arquivo `A_x_B.p12` guarda:

1. O **certificado de A** (identidade Ed25519) e o **certificado de troca de A**
   (X25519), públicos.
2. O **certificado de B** (identidade + troca), públicos — é como A conhece a
   chave pública com que deriva o segredo ECDH para falar com B.
3. A **privada do DONO LOCAL** (aqui, de A), empacotada em **PKCS#8** e
   embrulhada sob **a senha de A** (`pkcs8ShroudedKeyBag`).

Na máquina de **B**, o arquivo espelho `B_x_A.p12` guarda os mesmos
certificados públicos e a **privada de B** sob a **senha de B**.

**A regra que isto honra:** a privada nunca sai selada e **cada um abre a sua**.
Nenhum arquivo carrega a privada do outro; ninguém precisa da senha do outro. É
o mesmo princípio do protótipo (correio é assíncrono: nunca se pede a senha do
outro), agora em disco e num formato que ferramentas de fora leem.

O nome `<A>_x_<B>` é o par ordenado do ponto de vista do dono local; o `_x_`
lembra que é a aresta entre dois, não a caixa de um.

## 4. OIDs que a fatia 2 vai usar

Além dos três da fatia 1 (`id-Ed25519`, `id-X25519`, e `commonName` = `2.5.4.3`):

| OID | Nome | Onde entra |
|-----|------|-------------|
| `1.2.840.113549.1.7.1` | `data` (ContentType) | o `AuthenticatedSafe` externo |
| `1.2.840.113549.1.7.6` | `encryptedData` | o cofre dos certificados, cifrado |
| `1.2.840.113549.1.12.10.1.2` | `pkcs8ShroudedKeyBag` | a privada do dono, cifrada |
| `1.2.840.113549.1.12.10.1.3` | `certBag` | cada certificado |
| `1.2.840.113549.1.9.20` | `friendlyName` | rótulo do bag |
| `1.2.840.113549.1.9.21` | `localKeyId` | casa a privada ao seu certificado |
| `1.2.840.113549.1.5.13` | `PBES2` | esquema de cifra do shrouding |
| `1.2.840.113549.1.5.12` | `PBKDF2` | derivação de chave da senha |
| `2.16.840.1.101.3.4.2.1` | `sha-256` | hash do PBKDF2 e do MAC |

## 5. As RFCs em jogo

- **RFC 8032** — Ed25519 (assinatura). Já escrita e conferida:
  `crates/phxsql-core/src/ed25519.rs`.
- **RFC 7748** — X25519 (ECDH). Já escrita e conferida:
  `crates/phxsql-core/src/x25519.rs`.
- **RFC 8410** — como Ed25519/X25519 entram em X.509/PKCS#8: os OIDs curtos e,
  a armadilha, os **parâmetros AUSENTES** no `AlgorithmIdentifier` (não `NULL`).
- **RFC 5280** — X.509 v3 (o certificado). Fatia 1.
- **RFC 5958 / PKCS#8** — `OneAsymmetricKey`, o envelope da privada.
- **RFC 7292 / PKCS#12** — o `.p12`: `PFX`, `AuthenticatedSafe`, os `SafeBag`, e
  o `MacData` de integridade (com a KDF própria do Apêndice B).
- **RFC 8018 / PKCS#5** — PBES2 e PBKDF2, o shrouding da privada.
- **FIPS 180-4 / RFC 4231** — SHA-256/512 e HMAC, já conferidos aqui.

## 6. O que a fatia 1 JÁ ENTREGA, e o que a fatia 2 AINDA FALTA

### Pronto e medido (fatia 1)

- **Codec ASN.1 DER** — `crates/phxsql-core/src/asn1.rs`. Codifica e decodifica
  SEQUENCE, SET, INTEGER (com o `0x00` de sinal quando o topo é ≥ 0x80), OID,
  OCTET STRING, BIT STRING, BOOLEAN, NULL, PrintableString/UTF8String,
  UTCTime/GeneralizedTime e tags de contexto `[n]` explícitas. O leitor **recusa
  o que o DER proíbe**: comprimento indefinido, forma longa onde cabia a curta,
  `INTEGER`/arco de OID com byte sobrando. 11 testes de round-trip.
- **Certificado X.509 v3 autoassinado Ed25519** — `crates/phxsql-core/src/x509.rs`.
  Monta a TBSCertificate (v3, serialNumber, signature = `id-Ed25519` sem
  parâmetros, issuer=subject com CN, validity, SubjectPublicKeyInfo), assina a
  DER da TBS com `ed25519::assinar`, e embrulha em Certificate. **Duas
  variantes de SPKI:** (a) Ed25519 — o cert de identidade; (b) X25519 — o cert
  de troca, assinado pela Ed25519 do dono. 4 testes.
- **Prova real contra a ferramenta** — `crates/phxsql-core/examples/cert-openssl.rs`.
  Gera os dois certificados, e contra o **OpenSSL 3.0.13**:
  - `openssl x509 -text` parseia e reconhece **ED25519** e **X25519**, e a
    pública lida é o vetor conhecido `d75a9801…` da RFC 8032;
  - `openssl pkeyutl -verify -rawin` e `openssl verify` (do autoassinado)
    **aceitam a nossa assinatura Ed25519** — o verificador do OpenSSL concorda
    com o nosso assinante;
  - **falha-com-defeito nos dois sentidos:** um bit virado na assinatura faz o
    OpenSSL **recusar** (e o nosso `ed25519::conferir` também); trocar o OID
    `id-Ed25519` por um não registrado faz o OpenSSL **não reconhecer mais
    ED25519**.
  - 14 checagens, **PROVA VERDE**.

Por que a prova cripto usa `pkeyutl -verify` e não o `x509 -text`, e por que o
cert de troca não se prova por cadeia: ver
`docs/cognicao/cognicao_x509-ed25519-a-prova-e-pkeyutl-nao-o-x509-text_20260911_1820.md`.

### Falta (fatia 2 — PKCS#12/PFX)

1. **PKCS#8 `OneAsymmetricKey`** — envelopar a privada Ed25519/X25519 crua
   (RFC 8410 §7): `AlgorithmIdentifier` sem parâmetros + a privada como OCTET
   STRING **dentro de outro** OCTET STRING (o `CurvePrivateKey`).
2. **PBES2 shrouding** — cifrar esse PKCS#8 sob a senha do dono (PBKDF2-SHA-256
   + um cifrador simétrico) → `pkcs8ShroudedKeyBag`.
3. **`SafeBag`s** — os `certBag` dos certificados e o key bag, com `localKeyId`
   casando privada e certificado.
4. **`AuthenticatedSafe`** — os cofres (um em claro para os certs, ou também
   cifrado via `encryptedData`), embrulhados no `ContentInfo` `data`.
5. **`PFX` + `MacData`** — o envelope externo e o HMAC-SHA-256 de integridade,
   com a **KDF própria do PKCS#12 (RFC 7292 Apêndice B)**, não a PBKDF2.
6. Provar **`openssl pkcs12 -info`** lendo o nosso `.p12`, e o `.p12` do
   `openssl` lendo pelo nosso decodificador — nos dois sentidos.

### ⚠️ O nó da fatia 2 a decidir com o dono (papel C + cripto)

O cifrador que o `openssl pkcs12` entende no `pkcs8ShroudedKeyBag` via PBES2 é,
no OpenSSL 3.0 moderno, **AES-256-CBC** (ou o legado 3DES/RC2). Esta casa **não
tem AES escrito** — tem ChaCha20-Poly1305, que **não é** um cifrador PBES2 que o
OpenSSL leia ali. Então, para um `.p12` que o `openssl pkcs12` abra, a fatia 2
precisa de **AES-256-CBC escrito à mão e conferido contra os vetores FIPS-197 +
NIST CBC**, mais o `PKCS#7` padding. Isso é **cripto nova, sem crate**, e é a
parte de projeto e risco da fatia 2 — não uma tarde de trabalho mecânico.

Alternativa a pôr na mesa: um `.p12` **só nosso** (shrouding com
ChaCha20-Poly1305), que perde a interoperabilidade do cofre da privada com o
`openssl pkcs12` mas mantém os **certificados** legíveis por qualquer
ferramenta. A decisão é do dono, porque troca interoperabilidade por trabalho de
AES — e é medida, não palpite: os certificados já são interoperáveis hoje (fatia
1); o que está em jogo é só o embrulho da privada.

## 7. Rodar a prova da fatia 1

```bash
cargo test -p phxsql-core                       # asn1 + x509 (round-trip e cripto)
cargo run --example cert-openssl -p phxsql-core # prova real contra o OpenSSL
```

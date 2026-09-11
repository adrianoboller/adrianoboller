# Cognição: a prova de um cert Ed25519 contra o OpenSSL é `pkeyutl -verify`, não `x509 -text`

**Assunto:** interop de certificado X.509 v3 Ed25519/X25519 (RFC 8410) contra o
OpenSSL 3.0.13 — fatia 1 do correio `.p12`.
**Descoberta:** 11/09/2026, ~18h20. **Papéis:** B, C, F.

## 1. O que aconteceu

Escrevi um codec ASN.1 DER (`crates/phxsql-core/src/asn1.rs`) e um emissor de
certificado X.509 v3 autoassinado com Ed25519 (`crates/phxsql-core/src/x509.rs`),
tudo só com `std`, reaproveitando o `ed25519`/`x25519` já conferidos contra
vetor. A prova real contra a ferramenta está em
`crates/phxsql-core/examples/cert-openssl.rs` (14 checagens, PROVA VERDE).

## 2. O que eu concluí primeiro, e estava errado

Concluí duas coisas plausíveis, e as duas furaram:

- **«`openssl x509 -inform DER -text -noout` prova a assinatura.»** Não prova.
  Esse comando **só parseia e imprime** — ele nunca confere a assinatura. Um
  byte virado na assinatura passa por ele sem reclamação nenhuma. Quem confia no
  `-text` para provar cripto tem um teste que passa por engano.
- **«O cert de troca X25519 é autoassinado, então `openssl verify` fecha.»**
  Não fecha. O cert de troca carrega uma chave X25519 (que não assina nada) e é
  assinado pela Ed25519 do dono — não é autoassinado no sentido criptográfico. E
  ao tentar `openssl verify` dele contra o cert de identidade como CA, o OpenSSL
  recusa com **erro 79, «invalid CA certificate»**: sem a extensão
  `basicConstraints CA:TRUE`, o cert de identidade não serve de âncora de
  cadeia. O erro não tinha nada a ver com a assinatura — era constraint de CA.

## 3. O que a medição disse

Medido contra o OpenSSL 3.0.13 desta máquina:

- `openssl x509 -text` sobre a assinatura corrompida: **exit 0** (aceitou —
  porque não confere).
- `openssl pkeyutl -verify -pubin -inkey pub.pem -rawin -in tbs -sigfile sig`
  sobre a boa: **«Signature Verified Successfully», exit 0**; sobre a torta:
  **«Signature Verification Failure», exit 1**. Este é o verificador Ed25519 do
  OpenSSL conferindo a NOSSA assinatura sobre a NOSSA TBS.
- `openssl verify -CAfile ident.pem ident.pem` (o cert de **identidade**, esse
  sim autoassinado de verdade): **OK, exit 0**. Sem precisar de `basicConstraints`,
  porque uma âncora carregada no `-CAfile` é confiada como raiz.
- `openssl verify` do cert de **troca** contra a identidade como CA: **erro 79**.
- A pública Ed25519 impressa pelo OpenSSL a partir do nosso DER é o vetor
  conhecido `d75a9801…` da RFC 8032 — prova de que o OpenSSL leu os mesmos bytes
  que embutimos.
- Trocar o OID `id-Ed25519` (1.3.101.112) por um não registrado (1.3.101.99) faz
  o `x509 -text` deixar de imprimir «ED25519» — o OID caído dentro da TBS também
  derruba a nossa própria conferência, porque a assinatura deixa de cobrir os
  bytes lidos. Já trocar 112→113 vira **Ed448** (algoritmo válido!), então esse
  não serve de defeito.

Detalhe da RFC 8410 que morde antes de tudo: o `AlgorithmIdentifier` de Ed25519
e X25519 tem os **parâmetros AUSENTES**, não `NULL`. Pôr um `NULL` ali faz o
OpenSSL recusar. Por isso o `alg_id` escreve só o OID.

## 4. A regra

**Certificado de assinatura se prova com `openssl pkeyutl -verify -rawin` (ou
`verify` do autoassinado de verdade), nunca com `x509 -text`, que só parseia.**
E o cert que uma chave-não-assinante (X25519) publica confere pela chave do
**assinante** (a Ed25519 do dono), não por cadeia — cadeia exigiria
`basicConstraints CA:TRUE`, que a fatia 1 não emite.

## 5. Como está guardado hoje

- Codec DER: `crates/phxsql-core/src/asn1.rs` (11 testes de round-trip; o leitor
  recusa o que o DER proíbe: comprimento não-mínimo, `INTEGER`/arco de OID com
  byte sobrando, comprimento indefinido).
- Emissor X.509: `crates/phxsql-core/src/x509.rs` (4 testes; o de identidade
  confere a própria assinatura, o de troca confere pela Ed25519 do dono).
- Prova contra ferramenta: `crates/phxsql-core/examples/cert-openssl.rs`
  (`cargo run --example cert-openssl -p phxsql-core`), com falha-com-defeito nos
  dois sentidos (assinatura torta e OID trocado).
- **Buraco deixado de propósito (fatia 2):** o `.p12` interoperável precisa
  embrulhar a privada em `pkcs8ShroudedKeyBag`/PBES2, e o cifrador que o
  `openssl pkcs12` entende ali é **AES-256-CBC**, que esta casa **não tem**
  escrito. É crypto nova a escrever à mão e conferir contra FIPS-197 — sem
  crate, mas não é de graça. Registrado em `docs/CORREIO-P12.md`.

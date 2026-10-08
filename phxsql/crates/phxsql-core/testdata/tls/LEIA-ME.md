# Vetores oficiais da verificacao de assinatura (pedido 572, T6c-1)

Dados de teste, nao dependencia: entram por `include_str!` nos testes de
`src/rsa/testes.rs` e `src/p384/testes.rs`, e nao vao para o binario.

| arquivo | origem | o que foi cortado | licenca |
|---|---|---|---|
| `SigVer15_186-3.rsp` | NIST CAVP, `186-3rsatestvectors.zip` (csrc.nist.gov, FIPS 186-3 RSA) | os 54 casos de SHA-224 (nao implementado); 216 restam | obra do governo dos EUA, dominio publico |
| `SigVerPSS_186-3.rsp` | idem | os 54 casos de SHA-224; 216 restam | idem |
| `ecdsa_SigVer_P-384.rsp` | NIST CAVP ECDSA `SigVer.rsp` (FIPS 186-3), copia em `go/src/crypto/ecdsa/testdata/SigVer.rsp.bz2` | so as secoes `[P-384,SHA-1/256/384/512]`; 60 casos | dominio publico |
| `pss-vect.txt` | RSA Laboratories, vetores do PKCS#1 v2.1 (RSASSA-PSS, SHA-1), copia em `go/src/crypto/rsa/testdata/pss-vect.txt.bz2` | nada; 60 exemplos | distribuidos pela RSA Labs para teste |
| `rsa_signature_2048_sha256_test.json` | Wycheproof (C2SP), `testvectors_v1/` | nada; 259 casos (1 `acceptable` pulado) | Apache 2.0 |
| `rsa_pss_2048_sha256_mgf1_32_test.json` | idem | nada; 108 casos | Apache 2.0 |
| `ecdsa_secp384r1_sha384_test.json` | idem | nada; 504 casos | Apache 2.0 |

Fora daqui, e tambem oficiais: o SHA-384 confere contra a RFC 6234 §8.5
(`sha512.rs`), o ECDSA P-384 contra a RFC 6979 A.2.6 (constantes extraidas por
script, em `src/p384/testes.rs`), e o RSA-PSS do `CertificateVerify` contra os
tracos das secoes 3 e 5 da RFC 8448 (`src/tls/cliente.rs`), agora pelo
conferidor de producao.

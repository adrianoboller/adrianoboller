# NIST PKITS -- recorte da validacao de caminho (pedido 572, T6c-2)

Origem: o *Path Validation Testing Program* do NIST PKI Testing
(https://csrc.nist.gov/projects/pki-testing), vetores extraidos do `PKITS.pdf`,
na copia que acompanha o Go do conteiner (`go/src/crypto/x509/testdata/nist-pkits/`).
Licenca: dominio publico (obra do governo dos EUA, 17 U.S.C. 105) -- dito no
`README.md` de la. **So os dados entram aqui; codigo do Go, nenhum.**

`vetores.json` e o recorte das secoes 4.1, 4.2, 4.3, 4.5, 4.6, 4.7, 4.13 e
4.16 (95 vetores, sem o campo `CRLPath`); `certs/` sao os 159 certificados que
eles citam. O teste (`src/cadeia/testes.rs`) roda 86 e deixa 9 de fora, cada
um com o motivo no `FORA`: os tres de DSA e os seis que so sao invalidos por
REVOGACAO (CRL), que este validador nao faz -- decisao escrita no cabecalho de
`src/cadeia.rs`. As secoes inteiras fora (4.4, 4.14, 4.15: CRL; 4.8 a 4.12:
politicas) estao fora pelo mesmo motivo.

# Dados dos testes dos cofres externos (`tests/cofres.rs`)

## `aws4/` — suite oficial da AWS Signature Version 4

Os 31 casos do `aws-sig-v4-test-suite` da AWS (`.req` o pedido, `.creq` a requisição
canônica, `.sts` o texto a assinar, `.authz` o `Authorization`, `.sreq` o pedido assinado),
com a credencial de exemplo da própria suite (`AKIDEXAMPLE`,
`wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY`, região `us-east-1`, serviço `service`).

Fonte: o endereço original do pacote
(`https://docs.aws.amazon.com/general/latest/gr/samples/aws-sig-v4-test-suite.zip`)
respondeu **404** em 09/10/2026. Os arquivos foram baixados, sem alteração, da cópia que a
própria AWS mantém no SDK de Python — `boto/botocore`, ramo `develop`,
`tests/unit/auth/aws4_testsuite/` (`raw.githubusercontent.com`), em 09/10/2026. Dois casos
da lista conhecida da suite (`post-vanilla-query-nonunreserved` e
`post-vanilla-query-space`) não existem nessa cópia e não entram.

## `rfc7515_a2_pkcs8.pem` — a chave RSA do Apêndice A.2 da RFC 7515

A chave do exemplo JWS RS256 da RFC 7515 (Apêndice A.2.1), em PEM PKCS#8. Montada da JWK
do apêndice pelo `openssl asn1parse -genconf` (a `RSAPrivateKey` com `n, e, d, p, q, dp,
dq, qi`) e convertida por `openssl pkey`. Conferida por implementação independente: o
`openssl dgst -sha256 -sign` com este PEM sobre a entrada do apêndice reproduz, byte a byte,
a assinatura publicada na RFC (`cC4hiUPoj9Eetdgtv3hF80EGrhuB…`).

`rfc7515_a2_pkcs1.pem` é a mesma chave em PKCS#1 (`RSA PRIVATE KEY`), saída de
`openssl pkey -traditional` sobre o PEM acima, conferida do mesmo jeito.

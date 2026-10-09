# A marca `// REPOSTO` no fim do trecho mutado comentou o parêntese que fechava a expressão

**Estado:** INFRUTÍFERO (falha observada; causa e prevenção abaixo)

**Evidência:** a primeira corrida do repositor de defeitos da frente Nostr: `M01 sal do HKDF`
saiu «NAO DERRUBA» com `error: mismatched closing delimiter` em `nip44.rs:155`, e a interrupção
deixou `hmac_sha256(ikm, sal) // REPOSTO` na árvore até ser restaurado por escrita.

## O que aconteceu

O repositor trocava o trecho e acrescentava `// REPOSTO` ao fim do texto novo. O trecho
`hkdf_extrair(b"nip44-v1", &x)` vivia dentro de `Ok(...)`: o comentário engoliu o `)` final e a
mutação não compilou. O laço leu «sem resultado» como «não derruba».

## O que eu concluí primeiro, e estava errado

Que «fim do trecho» e «fim da linha» eram o mesmo lugar. Só são quando o trecho fecha a linha.

## A causa e a prevenção

- **Causa:** marca posta na coluna do trecho, não numa linha própria; e compilação quebrada
  contada como veredito.
- **Prevenção:** a marca vai numa **linha só dela, antes** da linha mutada (comentário de linha
  é válido no meio de expressão, de cadeia de método e de literal); compilação quebrada que
  aponta o arquivo mutado é **erro do repositor**, nunca «não derruba»; e o `SIGTERM` restaura o
  arquivo, porque matar o laço no meio deixou uma mutação viva.

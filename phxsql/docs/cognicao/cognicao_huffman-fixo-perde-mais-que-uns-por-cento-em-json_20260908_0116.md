# Huffman fixo perde bem mais que "uns por cento" contra o dinâmico, em JSON

## 1. O que aconteceu

O pedido 226 (compressão no fio) tinha a premissa medida pela F3 com o `zlib`
do Python: uma resposta de `varrer` com 5.000 linhas cai de **535.870** para
**55.284 bytes — 9,69×**. Ao fechar o pedido (negociação por pedido +
enquadramento em Base64, só no caminho claro), medi de novo — agora com o
`deflate` **desta casa** (`phxsql_core::zip::deflate`, Huffman **fixo**), na
mesma forma de dado (5.000 linhas, `id`+`nome`+`cidade`):

**535.871 → 94.694 bytes — 5,66×.**

(Um byte de diferença no tamanho de entrada é o `\n` do fio; irrelevante para
a comparação.) Medido pelo soquete, em
`crates/phxsql-server/tests/compressao-do-fio.rs::medir_o_ganho_do_deflate_desta_casa`,
rodável com `--nocapture` para reproduzir o número.

## 2. O que eu concluí primeiro, e estava errado

O próprio `phxsql_core::zip` documenta a escolha de Huffman fixo assim: *"Não
é o melhor compressor que existe: Huffman dinâmico (BTYPE=10) ganharia mais
alguns por cento, ao custo de montar e serializar duas árvores."* Lendo isso
antes de medir, esperei que o número da casa ficasse **perto** de 9,69× —
talvez 8× a 9×, "uns por cento" abaixo do zlib.

Errado: a diferença entre os dois é de **42%** no tamanho final (94.694 contra
55.284 bytes para o mesmo conteúdo) — quase o dobro da razão de compressão
perdida, não "uns por cento".

## 3. O que a medição disse

| compressor | árvore | bytes de saída | razão |
|---|---|---:|---:|
| `zlib` (Python, nível 6) | dinâmica | 55.284 | 9,69× |
| `phxsql_core::zip::deflate` | fixa | 94.694 | 5,66× |

A causa provável (não medida separadamente nesta rodada, então fica como
hipótese e não como fato): um JSON de protocolo repete um vocabulário curto e
previsível — as MESMAS chaves (`"id"`, `"nome"`, `"cidade"`) em cada uma das
5.000 linhas, e o alfabeto de caracteres do valor (nomes, um sobrenome fixo)
é bem mais estreito que um texto genérico. A árvore de Huffman **fixa** da
RFC 1951 foi desenhada para ser boa **em média**, sobre texto genérico — ela
não pode se especializar num vocabulário tão enviesado quanto este. A árvore
**dinâmica** constrói os códigos a partir das frequências REAIS do bloco, e
um vocabulário tão repetitivo é exatamente o caso em que ela ganha mais do
que "uns por cento".

O ganho do LZ77 (as referências para trás) é o mesmo nos dois — quem muda é
só a codificação de entropia por cima. Isso também não foi isolado nesta
rodada.

## 4. A regra

**Para JSON de protocolo (ou qualquer texto com vocabulário repetido e
estreito), meça com o compressor DESTA casa antes de citar a razão de outro
compressor — Huffman fixo x dinâmico diverge muito mais aqui do que diverge
no dado binário de tamanho fixo para o qual o `zip.rs` foi desenhado.**

Não é uma contradição da pétrea *"número citado é número que não se mede"* —
é a mesma lei, aplicada: a F3 já tinha registrado a premissa como vindo do
`zlib`, e não como "razão do motor"; o erro foi meu, ao esperar que os dois
números ficassem próximos sem medir.

## 5. Como está guardado hoje

- `docs/CIFRA-DO-FIO.md` §10 registra os dois números lado a lado, no bullet
  que também explica por que a compressão fica de fora do túnel cifrado.
- O teste que produz o número da casa é
  `compressao-do-fio.rs::medir_o_ganho_do_deflate_desta_casa`, com o limiar de
  aprovação deliberadamente baixo (`> 2,0×`) para não travar numa comparação
  com um compressor que não é o nosso.
- **O que NÃO mudou:** o comentário do `phxsql_core::zip` ("ganharia mais
  alguns por cento") continua como está — ele fala do formato de **disco**
  (`.reg`/`.ndx`, slot de tamanho fixo com preenchimento), um padrão de
  redundância diferente do JSON de protocolo, e não foi medido de novo nesta
  rodada. Corrigi-lo exigiria medir a mesma comparação sobre um `.reg`/`.ndx`
  real, o que fica para quem mexer no backup em disco, não para esta frente.
  O buraco, se houver um, é desse escopo — não deste documento.

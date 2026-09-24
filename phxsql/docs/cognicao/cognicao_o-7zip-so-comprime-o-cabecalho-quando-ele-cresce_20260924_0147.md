# O 7-Zip só comprime o cabeçalho quando ele cresce — o caminho do LZMA puro não aparece num `.phz`

**Descoberta:** 24/09/2026, 01:47, pedido 450 (PhxZip).

## 1. O que aconteceu

A premissa do contrato dizia que o 7-Zip grava, **por padrão**, «LZMA2 no
conteúdo e cabeçalho codificado (EncodedHeader, LZMA)». Gravei os arquivos de
um `config.json` com o 7-Zip 23.01 e li os bytes do cabeçalho antes de
escrever o leitor.

## 2. O que eu concluí primeiro, e estava errado

Que bastava um fixture `7z a -p... -mhe=on config.json` para exercitar o
decodificador de LZMA puro (método `030101`) no cabeçalho. Não exercita: nesse
arquivo o cabeçalho sai **só com o 7zAES**, sem LZMA nenhum. Um teste montado
em cima dele passaria sem ter lido um byte de LZMA — teste que passa por
engano.

## 3. O que a medição disse

- `7z a -p... -mhe=on` com **uma** entrada, e com uma entrada de nome de 200
  caracteres: o bloco do cabeçalho tem **um** coder, `06F10701` (7zAES).
- `7z a -p...` sem `-mhe`: o cabeçalho vem **em claro** (`kHeader`), sem
  `kEncodedHeader`.
- `7z a -mhe=on` com **duas** entradas, e com uma pasta de 60 arquivos: o
  cabeçalho vem com **dois** coders, 7zAES + `030101` (LZMA, propriedades
  `5d 00100000`). Sem senha, uma árvore de 8 entradas também sai em LZMA.
- O LZMA puro, então, só é exercitado pelos fixtures `duas-entradas.phz`,
  `muitos-cabecalho-cifrado.phz` e `arvore-*.7z` — e é por isso que o teste
  `mais_de_uma_entrada_e_recusada_contando` existe: chegar a contar as entradas
  prova que o LZMA do cabeçalho decodificou.

## 4. A regra

Antes de montar o fixture que prova um caminho, confira nos bytes que o
caminho está nele. O padrão documentado de uma ferramenta não é o que ela grava
em todo tamanho.

## 5. Como está guardado hoje

A tabela dos fixtures e a nota do que cada um exercita estão no topo de
`crates/phxzip/tests/phz.rs`; o LZMA puro também tem vetor do liblzma em
`crates/phxzip/src/lzma.rs` (`lzma1_do_liblzma_com_a_marca_de_fim`), que não
depende do 7-Zip decidir comprimir.

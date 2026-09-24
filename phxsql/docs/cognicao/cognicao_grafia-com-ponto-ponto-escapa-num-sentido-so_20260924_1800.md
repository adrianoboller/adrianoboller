# A grafia com `..` escapa da recusa num sentido só — a prova de grafia é a MATRIZ, não uma linha

**Estado:** PENDENTE

Data da descoberta: 24/09/2026, ~18:00 UTC (frente «fáceis D», pedido 523).

## 1. O que aconteceu

O pedido 523 dizia: a recusa do `fsync` (pedido 509) é guardada pela chave
léxica do diretório, e «pelo symlink ou por `dir/../dir` o mesmo diretório
sincroniza Ok». O conserto decidido pelo papel C foi resolver a grafia contra o
disco (`canonicalize`) na gravação da recusa e, só com recusa de pé, na
consulta (`crates/phxsql-store/src/volume.rs::chaves_reais`).

A primeira prova que escrevi recusava por `real/` e sincronizava pelas três
grafias (`real/`, `link/`, `real/../real/`).

## 2. O que eu concluí primeiro, e estava errado

Com o defeito reposto, a primeira prova acusou **só o `link/`**. A leitura
plausível seria «o `..` não é defeito: a chave léxica já o segura», e o teste
ficaria com uma grafia a menos do que o pedido nomeava.

## 3. O que a medição disse

O `..` escapa num sentido só. Recusado por `real/`, o caminho
`real/../real/t.log` ainda **começa** com `real/` componente a componente — o
`Path::starts_with` casa por acaso. Recusado por `real/../real/`, o `real/t.log`
não começa com ele. A matriz 3×3 (recusa por cada grafia, sincroniza por cada
uma, um diretório novo por linha) mediu, com o defeito reposto, **5 dos 6 pares
cruzados** sincronizando Ok, e a tabela aberta pelo symlink fechando Ok com o
byte 52 em **0**. Com o conserto: 0 de 6, byte 52 em 1. O parecer do papel C
tinha visto o `..` no sentido certo (`b/../b` recusa, `b` sincroniza); a minha
primeira prova, no sentido errado.

## 4. A regra

Prova de «a mesma coisa por outra grafia» é a matriz inteira — cada grafia
gravando e cada grafia consultando —, porque a comparação por prefixo não é
simétrica.

## 5. Como está guardado hoje

`crates/phxsql-store/tests/recusa-por-outra-grafia.rs` (a matriz, a tabela pelo
symlink e o relativo↔absoluto que já segurava) e a guarda
`recusa-do-fsync-por-grafia` no `bancada/guardas/catalogo.py`. O atestado do
`.ndx` (pedido 522) continua com a chave léxica de propósito: lá errar a grafia
é perder um atestado, que é o lado seguro — ou deixar um atestado órfão que só o CRC do cabeçalho separa (lido pelo papel C, não medido), e ele está no caminho de todo pedido.

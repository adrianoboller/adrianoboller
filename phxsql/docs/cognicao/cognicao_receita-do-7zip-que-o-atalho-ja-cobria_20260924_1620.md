# A receita do 7-Zip que o nosso atalho já cobria

## 1. O que aconteceu

A próxima lacuna do PhxZip era a velocidade de compactação, num fio e com
vários. O plano escrito era a busca em 2 estágios do 7-Zip (hash numa thread,
árvore noutra) e dois cortes do `GetOptimum` dele: `numFastBytes` e
`startLen`.

## 2. O que eu concluí primeiro, e estava errado

Que o gargalo com vários fios era a busca, e que os dois cortes do 7-Zip
comprariam velocidade do planejador. Os dois estavam errados. A busca já tinha
fio próprio, e o limite passou a ser o planejador. Os dois cortes quase nunca
disparavam, porque o atalho do «bom» já encerrava o plano antes de o laço
enumerar comprimentos longos.

## 3. O que a medição disse

- **Divisão do tempo** (callgrind, texto de 1,3 MB, nível 5): busca 43%,
  planejador 46%, resto 11%.
- **Corte no «bom»:** +0,3% de instruções.
- **`startLen`:** −0,8% de instruções, mesmo tamanho.
- **Relógio:** a oscilação de cerca de 10% escondia essas diferenças, e só a
  contagem de instruções decidiu.
- **Comparar 8 bytes por vez:** −0,6% no texto e −5,9% no binário, porque ali
  os casamentos são longos.

## 4. A regra

Receita de fora se confere contra o que o nosso código JÁ faz no mesmo ponto,
e não só contra o gargalo. Um corte do 7-Zip pode ser a mesma decisão que um
atalho nosso tomou antes, com outro nome. E numa diferença de 1%, o relógio
não decide: decide a contagem de instruções.

## 5. Como está guardado hoje

- `docs/PHXZIP.md` §3d-bis: a tabela das três hipóteses, com o número.
- `crates/phxzip/examples/perfil.rs`: o medidor, que roda num fio, uma vez, e
  serve ao callgrind.

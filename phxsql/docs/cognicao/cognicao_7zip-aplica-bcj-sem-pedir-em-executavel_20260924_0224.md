# O 7-Zip aplica BCJ sem pedir em executável — e a fixture tem de desligar

## 1. O que aconteceu

Primeira prova 7-Zip → PhxZip, 24/09/2026 02:24: cinco arquivos gravados com
`7z a -m0=lzma`, `-m0=lzma2 -mx=9`, `-m0=copy`, `-ms=off` e `-mhe=on`, todos
com a mesma pasta de entrada. Os cinco recusados pelo `phxzipcmd` com
«metodo BCJ (x86) recusado» (e um com BCJ2).

## 2. O que eu concluí primeiro, e estava errado

Que `-m0=lzma2` fixava a corrente inteira da pasta, e que portanto a recusa
era defeito do leitor — ligação entre coders lida ao contrário, ou o
identificador do LZMA2 casando com o do BCJ.

## 3. O que a medição disse

A pasta de entrada tinha um executável ELF (o próprio `phxzipcmd`, copiado
para ter dado binário). O 7-Zip detecta executável pelo conteúdo e acrescenta
o filtro BCJ/BCJ2 **por conta própria**, mesmo com `-m0` dado; só `-mf=off`
desliga. Refeito com `-mf=off`: seis de seis arquivos abertos, conteúdo igual
por `diff -r`. Até o `-m0=copy` levou BCJ.

## 4. A regra

Fixture de interoperabilidade gravada pelo 7-Zip leva `-mf=off`, e a
comparação de compressão também: sem isso o 7-Zip faz trabalho diferente do
nosso (filtro + compressor) e o número compara coisas desiguais.

## 5. Como está guardado hoje

As fixtures de `crates/phxzip/tests/dados/` foram gravadas com `-mf=off`, e a
tabela do `docs/PHXZIP.md` §3 diz que os filtros estavam desligados. A recusa
do BCJ em si está **certa** (pedido 454: filtros BCJ são recusa nomeada) — e o
buraco que fica é de produto: um `.7z` comum com executável dentro, gravado
com as opções padrão do 7-Zip, o PhxZip não abre. Se isso pesar, é decisão do
dono reabrir os filtros; hoje a mensagem diz o nome e o `-m0=lzma2 -mf=off`
refaz.

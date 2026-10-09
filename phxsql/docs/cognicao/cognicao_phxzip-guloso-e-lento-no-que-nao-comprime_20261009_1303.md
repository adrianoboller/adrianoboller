# O codificador do PhxZip é lento justamente no que não comprime

## 1. O que aconteceu

A bancada do PhxZip contra o `7z` (pedido 455, fatia Z11,
`bancada/phxzip/medir.py`) mediu dois conjuntos com trabalho igual — LZMA2 não
sólido, guloso, `mc=48`, uma thread dos dois lados. No `fontes` (858 arquivos de
texto, 19,6 MB) a compactação empatou dentro do ruído. No `aleatorio` (8 MiB por
SHA-256 encadeado, que não comprime) o `phxzipcmd compactar` levou 33,06 s de
mediana contra 4,05 s do 7z igual: **8,16×**, faixas sem cruzar, 5 rodadas
intercaladas, 09/10/2026 13:02, commit `c70e99ee`. Os números estão no
`bancada/phxzip/resultados.json` e no `docs/MANUAL-PHXZIP.md` §6.

## 2. O que eu concluí primeiro, e estava errado

Que o conjunto aleatório seria o caso **fácil**: o `lzma_compressor.rs` tem o
«pedaço cru quando comprimir não paga», e eu li isso como atalho — dado que não
comprime iria cru e rápido. Só pus o conjunto na bancada para exercitar o
pedaço cru, não para medir tempo. O atalho decide o que vai para o DISCO; o
trabalho de procurar casamento em cada posição acontece antes, inteiro, e só
então é jogado fora.

## 3. O que a medição disse

- `fontes`: compactar 1,28 s × 1,09 s (empate, faixas se cruzam); tamanho
  +2,12% (126.575 bytes) do PhxZip.
- `aleatorio`: compactar 33,06 s × 4,05 s (8,16×); extrair 0,08 s × 0,02 s.
- A causa **não foi medida**. Hipóteses, ainda sem número: (a) a cadeia de
  dispersão visita as 48 posições em toda posição quando nenhum casamento
  aparece, sem a saída precoce que o `hc4` do 7-Zip tem; (b) o pedaço é
  codificado inteiro e só depois comparado com o cru, e o trabalho da tentativa
  se paga a cada 64 KiB empacotados.

## 4. A regra

Conjunto «que só exercita um caminho» também se mede no tempo: o caminho que
parece atalho no código pode ser o pior caso no relógio.

## 5. Como está guardado hoje

A bancada mede os dois conjuntos a cada corrida, e o manual publica o número
pelo gerador, com o vencedor só quando as faixas não se cruzam. **O defeito de
desempenho não tem pedido aberto nem conserto**: a frente Z11/Z12 não mexe no
codificador. Fica para quem abrir o pedido medir (a) contra (b) antes de
consertar.

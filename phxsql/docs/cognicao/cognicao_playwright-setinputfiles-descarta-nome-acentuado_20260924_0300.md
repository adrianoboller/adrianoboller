# O `setInputFiles` por caminho descarta, calado, o arquivo de nome acentuado

## 1. O que aconteceu

Primeira gravação do vídeo PhxZip × 7-Zip (`testes-web/video-phxzip.mjs`),
24/09/2026 03:00: cinco arquivos escolhidos pelo `page.setInputFiles` com os
caminhos; o `.7z` gravado pela tela saiu com **quatro**. Faltou
`relatório-ação.txt`. O `7z t` deu «Everything is Ok» — o arquivo estava
íntegro, só incompleto —, e quem acusou foi o `diff -r` contra os originais.

## 2. O que eu concluí primeiro, e estava errado

Que o defeito era do servidor: o nome acentuado quebrando o JSON do envelope,
ou o `Escritor` recusando o nome. Os dois tinham teste passando, e a suspeita
parecia natural porque o nome era justamente o único com UTF-8.

## 3. O que a medição disse

O mesmo envelope mandado pelo Python, com os cinco nomes, voltou com cinco
(`7z l`: 5 files). Conferido dentro do navegador, depois do `setInputFiles`
por caminho, `input.files` tinha **4** — o arquivo acentuado nunca chegou à
página. Passando o conteúdo como buffer (`{name, mimeType, buffer}`), vieram
os cinco. O servidor estava certo; a ferramenta de teste mentia.

## 4. A regra

Todo roteiro que escolhe arquivo pelo navegador confere a contagem de
`input.files` logo depois de escolher, e não confia no êxito da chamada. E
prova de interoperabilidade termina num `diff` contra os originais, não num
«teste ok» — o teste confere integridade, não completude.

## 5. Como está guardado hoje

`escolherArquivo` no `video-phxzip.mjs` entra por buffer e **lança erro** se o
navegador recebeu menos arquivos do que foram escolhidos. A cena 2 do vídeo
termina em `diff -r`. Os outros roteiros de `testes-web/` que usam
`setInputFiles` por caminho **não foram revistos** — buraco registrado.

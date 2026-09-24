# Busca melhor expõe o atalho do planejador: o «bom» forçava casamento novo por cima da repetição

## 1. O que aconteceu

Frente do PhxZip, fechar a diferença do LZMA2 contra o `7z -mf=off -mmt=1`.
A árvore binária (`Buscador`, `crates/phxzip/src/lzma/enc.rs`) entrou nos
níveis 5–9 e o teste `o_preco_ganha_do_guloso_em_texto` **caiu**: no texto de
tabela (4.000 linhas quase iguais), o guloso com a árvore deu 17.049 bytes e a
análise ótima por preço, com a MESMA árvore, 17.725. O preço perdendo do
guloso.

## 2. O que eu concluí primeiro, e estava errado

Que o defeito era da árvore (casamentos com distância ruim, fora da ordem que
o preço espera) ou das tabelas de preço envelhecidas (`REFAZER`). Nenhum dos
dois: com «bom» 273 o mesmo planejador dava 13.649 — o problema aparecia só
com o «bom» de 32 do nível 5.

## 3. O que a medição disse

O atalho do laço (`otimo.rs`, casamento ≥ «bom» adiante) **forçava o
casamento novo** no nó `cur + l`, sem olhar a repetição que começava ali. Na
tabela, a repetição longa da distância de sempre sai sem pagar distância; o
casamento novo paga a distância cheia. O `GetOptimum` do 7-Zip faz outra
coisa: **termina o plano em `cur`** e deixa o próximo começar ali, onde os
atalhos conferem a repetição ANTES do casamento. Trocado para isso (com a
lista de casamentos guardada, porque a árvore não se consulta duas vezes):

| | antes | depois | 7z |
|---|---|---|---|
| tabela sintética, nível 5 | 17.887 | 15.889 | 15.971 |
| corpus da §3, nível 5 | 2.896.986 | 2.896.482 | 2.896.132 |

Com a cadeia, o defeito existia e não aparecia no teste (ótimo 18.353 contra
guloso 20.342). A explicação provável — a cadeia raramente achava casamento de
32+ no meio da tabela, então o atalho quase nunca disparava — **não foi
medida** (não contei os disparos).

## 4. A regra

Quando uma peça de BAIXO melhora (a busca), re-meça as decisões de CIMA que
dependiam da peça velha ser fraca — um atalho que nunca disparava não estava
certo, estava dormindo.

## 5. Como está guardado hoje

`o_preco_ganha_do_guloso_em_texto` cai com o atalho antigo reposto (medido:
17.725 contra 17.049). O teste já existia; o que mudou foi a busca passar a
alcançá-lo.

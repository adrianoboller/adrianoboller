# Um caractere na receita, e uma corrida no lugar de três

*Descoberto em 07/09/2026, 05h20, ao montar a bancada do índice de texto.*

## 1. O que aconteceu

O medidor que fundamentou a decisão do dono sobre o `.fts` gerava o texto de
teste assim:

```rust
/// ~14 palavras distintas, como o texto que a §20 mediu.
s.push_str(RECHEIO[...]);
s.push('_');
s.push_str(&((i + k) % 5_000).to_string());
```

**O `_` não é alfanumérico.** A quebra em termos parte em tudo o que não é
letra nem dígito (`termo.rs::quebrar`), então `pedido_0` são **dois** termos, e
o texto tinha **~26**, não 14. O `6,70×` que eu levei ao dono é o custo de 26
chaves por linha, com a prosa ao lado dizendo 14.

E o achado veio pelo **medidor irmão**: escrevi a bancada de ponta a ponta
reusando o mesmo gerador, procurei `pedido_0`, e ela anunciou **39.730×** com
`linhas achadas: 0`. As duas faixas concordavam em **nada**.

## 2. O que eu concluí primeiro, e estava errado

**Duas vezes, e a segunda depois de já ter aprendido a lição uma vez neste
mesmo dia.**

Primeiro: li `39.730×` e quase escrevi o número. O `linhas achadas: 0` estava
impresso na linha de cima, na minha própria saída.

Segundo, e pior: consertei o `_`, remedi, deu **4,49×**, e escrevi isso no
`FTS.md` como «a terceira medição, que é a que vale». Uma corrida. Rodei de
novo por outro motivo e deu **5,79×** — 29% acima. O número que eu tinha
acabado de publicar como conserto de um erro de medição era ele próprio um
número não medido.

## 3. O que a medição disse

A cadeia inteira, e cada elo derrubou o anterior:

| medição | forma | B/A |
|---|---|---:|
| `custo-da-chave-a-mais` | 15 árvores separadas | 9,05× |
| `custo-do-fts-de-verdade` | 1 árvore, mas 26 termos/linha | 6,70× |
| a mesma, com o `_` corrigido — **uma corrida** | 1 árvore, 14 termos | 4,49× |
| a mesma — **três corridas** | 1 árvore, 14 termos | **5,67×** (5,38–6,06) |

E no ganho da busca a faixa é ainda mais larga: **21.165–33.444×**, mediana
31.399. Publicar a primeira corrida seria publicar sorte nos dois casos — e nos
dois a sorte veio **a nosso favor**, que é o lado em que ela não se percebe.

## 4. A regra

**Uma corrida não é medição, e isso vale para a bancada inteira — não só para o
gráfico.** E: quando o medidor souber contar o que está medindo, faça-o
**imprimir a contagem**, em vez de deixá-la num comentário.

## 5. Como está guardado hoje

`bancada/fts/medir.py` roda **três vezes** e grava `{min, mediana, max}` de cada
número; o `resultados.json` não tem mais um escalar. Os dois medidores do motor
ganharam portões próprios:

| portão | o que ele impede |
|---|---|
| os conjuntos de rowids têm de bater, busca a busca | número que sai de trabalhos diferentes |
| nenhuma corrida pode achar **zero** linha | as duas faixas concordarem em nada |
| a contagem de chaves por linha é **impressa** | a prosa dizer 14 enquanto são 26 |

**O alcance que este arquivo registra**, porque a lei já existia: a regra da
faixa min–max estava escrita no pedido 155 **para os gráficos** — *«cada barra
traz a faixa, e o vencedor só é contornado quando as faixas não se cruzam»*.
Ela nasceu na página que desenha, e ficou morando lá. O que faltava era ela
valer onde o número é **produzido**, e não só onde ele é desenhado: um gerador
de gráfico não consegue desenhar a faixa de um `resultados.json` que guardou um
escalar.

**O buraco que fica:** as outras bancadas desta casa continuam gravando escalar.
Nenhuma delas foi refeita nesta rodada — só a do `.fts` nasce com faixa —, e
isso não é conserto pendente escondido: é o tamanho do trabalho, e ele está dito.

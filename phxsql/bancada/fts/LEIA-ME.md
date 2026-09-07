# A bancada do índice de texto

O `.fts` contra a varredura, na **mesma pergunta** e com o **mesmo resultado**.

```bash
bancada/esta-medindo.sh && echo "há medição em curso -- espere"
cargo build --release --examples -p phxsql-store
python3 bancada/fts/medir.py 1000000 20
```

## Os dois portões do medidor

Esta bancada tem duas coisas que as outras não precisavam ter, e as duas saem
de erro pago aqui.

**1. Os conjuntos de rowids têm de bater, busca por busca.** Não as contagens —
os conjuntos. É a regra da casa (*bancada compara trabalho igual, e não só
pergunta igual*) transformada em `assert`: se a varredura e o índice
responderem diferente para qualquer palavra, o medidor **aborta** em vez de
imprimir um número. Um número que sai de trabalhos diferentes é pior que número
nenhum, e esta casa já publicou dois.

**2. Nenhuma busca pode achar zero.** A primeira corrida deste medidor anunciou
**39.730×** com `linhas achadas: 0` — as duas faixas concordavam em **nada**,
porque a palavra procurada (`pedido_0`) não era um termo: o `_` não é
alfanumérico e a quebra parte ali. *Concordar em zero não é prova.* Hoje o
medidor aborta se o total de achados for zero.

## Metade das palavras não existe, e é de propósito

Palavra inexistente é o caso em que o índice ganha mais — desce a árvore e
volta com zero, enquanto a varredura lê a tabela inteira. Só medir palavra que
existe seria honesto; só medir palavra que não existe inflaria o ganho a nosso
favor. A bancada usa metade e metade, e diz isso na saída.

## O lado da escrita entra no mesmo arquivo

O ganho da busca é pago na gravação. Uma bancada que publicasse só o ganho
contaria a metade que nos favorece, então o `resultados.json` carrega as duas:
o `escrita_vezes` é o preço, e o `ganho` é o que ele compra.

E o `escrita_chaves_por_linha` sai **medido**, e não do comentário acima do
gerador de texto — foi exatamente aí que a medição anterior se enganou, e o
`docs/FTS.md` §4.1.1 conta como.

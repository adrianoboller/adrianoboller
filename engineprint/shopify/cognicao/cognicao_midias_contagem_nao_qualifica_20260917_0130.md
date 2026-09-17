# Contagem de midias nao qualifica a pagina do produto

**Descoberto em** 17/09/2026, 01:30. **Produto:** Bambu Lab H2Ds
(`gid://shopify/Product/8017152540715`).

## O pedido

*«as H2Ds quero 6 imagens»*. Tinha 3, subiram 3, `mediaCount` devolveu **6**,
todas `READY` e com `mediaErrors` vazio. Pelo criterio do pedido, pronto.

## O que eu concluiu primeiro, e estava errado

Concluiu que contar resolvia. `mediaCount: 6` e seis `status: READY` sao o
numero que o pedido pediu, e ele fecha sem mentir — por isso passa. So que
`READY` quer dizer «o Shopify processou o arquivo», nao «a imagem serve para
esta pagina». Sao perguntas diferentes, e a API so responde a primeira.

Medido depois, nas mesmas seis:

| # | dimensao | canto rgb |
|---|---|---|
| 1 | 389x392   | (0, 0, 0)       |
| 2 | 2400x2400 | (0, 0, 0)       |
| 3 | 451x450   | (0, 0, 0)       |
| 4 | 2400x2400 | (247, 247, 247) |
| 5 | 2400x2400 | (247, 247, 247) |
| 6 | 3800x3800 | (247, 247, 247) |

As tres antigas tem **fundo preto puro** e a capa era a pior das seis: preta
**e** 389 px, num palco `#F7F7F7` de canto arredondado. O dono ja tinha
reclamado exatamente disso — *«esta muito feio a borda da imagem e o fundo»* —
e o defeito continuou de pe atras de um numero que dizia sucesso.

E o agravante e o proprio trabalho da rodada: a transicao suave que entrou
(`opacity .26s`, no lugar do corte seco) faz o carrossel **piscar** do preto
para o claro tres vezes. A melhoria anterior deixou o defeito mais visivel,
nao menos.

## A causa

O criterio de aceite veio do enunciado (*seis*) em vez de vir do lugar onde a
imagem aparece (*o palco*). `mediaCount` e `status` sao os dois numeros que a
API oferece de graca, e por isso foram os dois que eu olhei.

## O conserto

`productReorderMedia`, sem apagar nada: capa passa a ser a de 2400 px com
fundo claro, a do laser (preta, mas 2400 px e o diferencial do produto) fica no
meio, e as duas pequenas de fundo preto vao para o fim. Conferido depois —
`featuredMedia` e 2400x2400.

Reordenar e reversivel; trocar as duas pequenas por candidatas claras de
2400 px e apagar midia, e isso se pergunta antes.

## A lei que sai daqui

**Midia se confere onde ela aparece, nao no contador.** Para toda imagem que
entra numa vitrine, alem de `status: READY`:

- **dimensao** contra o tamanho renderizado (o palco pede >= 2x o CSS);
- **canto rgb** contra o fundo do palco — um so canto fora ja denuncia;
- **as irmas juntas**, desenhadas no palco de verdade, porque o que estraga
  um carrossel e a **diferenca** entre as imagens, e essa nenhuma delas
  carrega sozinha.

E o corolario, que e a lei do projeto por outro caminho: *numero que o pedido
pediu nao e numero que a pagina precisa.* Aceite que sai do enunciado fecha
tarefa; aceite que sai do palco fecha defeito.

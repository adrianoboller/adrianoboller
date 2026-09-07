# O controle prova o achado; falta o que prova o LEITOR

**07/09/2026, 03:10** — descoberto ao montar o `bancada/comparativo/`, o
medidor que responde «o que ainda falta no PhxSql, contra quem tem».

## 1. O que aconteceu

O medidor tem **sondas de efeito**: em vez de perguntar «o motor aceitou o
campo `padrao`?», ele cria a tabela com `padrao: 7`, insere sem a coluna e
**lê de volta**. Aceitar é barato; produzir efeito, não. Escrevi o controle
positivo em quatro delas, e me achei rigoroso.

Quatro sondas publicaram, no `resultados.json` da primeira corrida:

```
default_de_coluna    nao   o campo `padrao` foi aceito e IGNORADO: v = None
coluna_calculada     nao   o campo `calculada` foi aceito e IGNORADO: b = None
indice_parcial       nao   veredito ANULADO pelo controle: nem a linha INCLUIDA
                           o indice devolveu (dentro=None)
view                 nao   nenhuma operacao de visao entre as 0
```

As quatro frases são falsas sobre o **motivo**, e as quatro têm o veredito
certo. As tabelas **nunca nasceram**: o índice ia como `[{"coluna": 0}]`, o
servidor recusava com «indice pk sem colunas», e ninguém lia a recusa. E as
leituras seguintes liam `r["linha"]` do **envelope**, quando a resposta vem
dentro de `r["resultado"]` — então achariam `None` mesmo que a tabela
existisse. Dois erros empilhados, cada um bastando sozinho para produzir a
mesma frase.

## 2. O que eu concluí primeiro, e estava errado

**Primeiro:** «o campo é aceito e ignorado». Plausível, e o pior é que era
**verdade** — só que por acaso. O motor de fato engole `padrao`, `check`,
`calculada` e `onde` (virou achado próprio, `docs/COMPARATIVO.md` §6). A frase
certa nasceu de um medidor quebrado, e isso é o caso mais difícil de pegar:
*o errado sobrevive melhor quando o conserto funcionou por outro motivo.*

**Segundo:** ao ver o catálogo devolver zero operação, escrevi que a causa era
não ter mandado `database` no pedido. Pus um portão e, junto, o interruptor
que repõe o defeito. **A prova real desmentiu:** com o defeito reposto o
medidor **passou** — sem `database` o servidor responde igual. O zero vinha só
do envelope. Sem a prova real, o portão teria ficado guardando a causa que eu
imaginei, com o comentário afirmando a causa errada para sempre.

**Terceiro:** achei que o controle positivo que já tinha escrito me protegia.
Ele prova o **achado** — «recusou o proibido E aceitou o permitido». Nenhum
deles pergunta se o instrumento que lê a resposta funciona.

## 3. O que a medição disse

- **5** sondas irmãs liam o nível errado da resposta; **4** publicaram
  evidência falsa, e **1** — a do `check` — deu o motivo certo («a tabela não
  nasceu») porque tinha controle no lugar certo.
- **123** operações no catálogo; a primeira corrida publicou **118**, porque
  subiu um `phxsqld` anterior ao `procurar_texto`. Recompilado, **119** visíveis
  + 4 ocultas = 123.
- O controle novo — gravar `v = 42` e exigir ler 42 — derruba as cinco de uma
  vez, com uma linha.
- **3** portões provados nos dois sentidos por
  `bancada/comparativo/prova-dos-portoes.py`, e o quarto morreu na prova (o do
  `database`), que é resultado tão válido quanto os três.

## 4. A regra

**Antes de a sonda medir o efeito, prove o instrumento que lê o efeito.** Grave
um valor que você conhece e exija lê-lo de volta. Sem isso, «não achei» e «não
sei olhar» produzem a mesma frase — e a frase é convincente nas duas.

E o corolário, que é o que faz esta cognição não ser só mais uma cópia do
controle positivo: **um controle prova o que está DEPOIS dele.** O controle
positivo do achado vive dentro da sonda e não alcança a mesa posta nem o
leitor. Cada camada que a sonda atravessa — a tabela nasceu, a leitura lê, o
binário é de hoje — quer o seu.

## 5. Como está guardado hoje

- `bancada/comparativo/medir.py`: `corpo()` desembrulha num lugar só;
  `linha_de()` é o leitor único; o controle `v = 42` roda **antes** de qualquer
  sonda; `cria()` **exige** que a tabela nasça, e as duas sondas em que a recusa
  é a resposta pedem isso pelo nome (`exigir=False`); `compila_o_nosso()`
  recompila o `phxsqld` antes de medir.
- `bancada/comparativo/prova-dos-portoes.py`: repõe cada defeito por
  `PHX_CMP_DEFEITO` e exige a parada certa — **e prova o sentido contrário**,
  porque um medidor que parasse sempre passaria nos três.
- `bancada/comparativo/LEIA-ME.md`: a tabela defeito → portão, e a história do
  diagnóstico que morreu medido.

**O buraco que fica:** os portões são de Python e o catálogo de guardas
(`bancada/guardas/catalogo.py`) só sabe repor defeito em Rust. O
`prova-dos-portoes.py` não roda junto da bateria única — quem mexer no medidor
tem de chamá-lo à mão. Papel que não está cumprindo aparece como não cumprindo.

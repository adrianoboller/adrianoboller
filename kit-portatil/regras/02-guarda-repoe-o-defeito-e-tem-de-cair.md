# A guarda repõe o defeito, e o teste tem de cair

**Regra.** Todo teste que guarda um defeito tem de **falhar com o defeito
reposto** e passar com o conserto. Isso não se faz uma vez à mão e se esquece:
vira um **catálogo** (arquivo, trecho de hoje, trecho do dia do estrago, testes
que caem, testes que seguem) e um **executor** que repõe cada defeito numa
cópia da árvore, roda só os testes nomeados, desfaz, e dá o veredito:
PROVADA, NÃO PEGOU, ESTRAGOU, QUEBRADA ou REDUNDANTE.

E há um terceiro sentido: **o mesmo comando, sem defeito nenhum, tem de sair
verde na mesma corrida.** Vermelho sem base verde ao lado não prova nada.

**Cicatrizes.**
- Uma prova passou com o defeito reposto porque conferia o veredito, e a
  conferência acontecia **depois** do dano. O número certo só saiu quando ela
  passou a medir *quanto* foi lido, não *se* recusou.
- Ninguém sabia dizer quais de 1.242 asserções ainda pegariam o defeito que as
  motivou — a prova feita uma vez à mão tinha se perdido.
- Onze mutantes deram «vermelho» numa rodada: o comando de teste tinha dois
  filtros que a ferramenta recusava por erro de uso. Todo mutante saía ≠ 0. A
  base sem mutante também saía ≠ 0, e ninguém a rodou.
- Defeito reposto no fim de um arquivo caiu dentro do bloco de teste, não da
  produção: a prova ficou verde com o defeito «na árvore».

**Como aplicar.** `scripts/provar-guardas.py` deste kit é o esqueleto
(catálogo → troca → caem/seguem, cópia isolada, veredito). Nunca na árvore de
verdade; desfazer em `finally` e em `atexit`; prazo em toda rodada.

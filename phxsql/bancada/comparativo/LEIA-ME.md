# A bancada comparativa — o que falta aqui, contra quem tem

Três arquivos, e a ordem importa:

```bash
python3 bancada/comparativo/medir.py             # mede e grava resultados.json
python3 bancada/comparativo/documento.py         # escreve docs/COMPARATIVO.md
python3 bancada/comparativo/prova-dos-portoes.py # prova os portões do medidor
```

O `docs/COMPARATIVO.md` **não se edita**: a prosa mora no `documento.py` e a
medição no `resultados.json`. Documento gerado que alguém edita à mão volta a
ser documento digitado no commit seguinte, e ninguém percebe qual das duas
versões é a verdadeira.

## As três procedências, e por que a tabela as separa

**Motor vivo.** Quatro respondem nesta máquina — PhxSql (soquete, op `sql`),
MySQL(R), PostgreSQL(R) e SQLite(R) —, e a mesma pergunta vai para os quatro na
língua de cada um. Nenhuma célula dessas colunas é escrita à mão.

**Sonda de código**, só onde SQL não alcança. Cada uma aponta arquivo, linha
**e o trecho citado** — caminho pelado não é evidência, e foi assim que a sonda
do TLS ficou três rodadas apontando para o cliente de e-mail com o veredito
certo.

**Citado**, para HFSQL(R) e Cassandra(R), que não têm motor nem folha nesta
sessão. Citado não é mentira; é segunda mão, e a tabela marca 📄 célula a
célula para que ninguém a leia como medida.

## Os portões, e o defeito de cada um

Cada portão abaixo existe porque o defeito passou. E cada um se prova com o
defeito **reposto**, por `PHX_CMP_DEFEITO=<nome> python3 …/medir.py` — é o que
o `prova-dos-portoes.py` faz, mais o sentido contrário: sem defeito, o medidor
tem de ir até o fim. Sem essa segunda metade, um medidor que parasse sempre
passaria nos três.

| defeito reposto | o portão que tem de disparar |
|---|---|
| `indice-velho` | `MESA NAO POSTA` — o índice volta ao formato `[{"coluna": 0}]`, o servidor recusa, e a tabela não nasce |
| `envelope` | `LEITOR QUEBRADO` — as sondas voltam a ler o envelope em vez do `resultado` |
| `catalogo-vazio` | `SONDA QUEBRADA` — a lista de operações chega vazia, e lista vazia não é ausência de visão |

E a prova real já derrubou um diagnóstico meu aqui: o interruptor começou
sendo «pedir o `catalogo` sem `database`», porque foi assim que eu expliquei o
zero. Reposto, o medidor **passou** — sem `database` o servidor responde igual.
O zero vinha do envelope lido no nível errado, e só. *Diagnóstico plausível não
é diagnóstico medido*, e o errado sobrevive melhor quando o conserto funcionou
por outro motivo.

Há mais dois portões que não têm interruptor porque não dependem do nosso
motor: o **controle positivo** (`CREATE ZZZZ nao_existe_de_proposito` tem de
ser recusado por **todos**; se algum aceitar, quem mente é o medidor) e a
**recompilação do `phxsqld` antes de medir** — a primeira corrida de 07/09
subiu binário anterior ao `procurar_texto` e publicou 118 operações onde havia
123.

## As cinco armadilhas já pagas

Estão contadas no `docs/COMPARATIVO.md` §1, com o que cada uma virou. O resumo
para quem for mexer aqui:

1. **Perguntar ao texto do repositório** achou o que não é — três células
   erradas na primeira corrida.
2. **Um portão que exigia diversidade de resposta** reprovou o PostgreSQL(R),
   que genuinamente tem tudo o que se perguntou por SQL.
3. **Recusa sem controle que passa** não prova nada: índice que devolve zero
   pode não existir, tabela que recusa o proibido pode nunca ter nascido.
4. **Ler o envelope** em vez do `resultado` deu «nenhuma operação entre as 0» —
   um `não` certo pelo motivo errado, que é o pior tipo de certo. Estava em
   quatro sondas irmãs de uma vez.
5. **Tabelas que nunca nasceram**: o índice ia no formato errado, a recusa não
   era lida, e as leituras seguintes achavam vazio e publicavam «o campo foi
   aceito e IGNORADO».

## Ao acrescentar uma capacidade à tabela

- Se ela se pergunta por SQL, entre em `PERGUNTAS` com a instrução **na língua
  de cada motor** — a mesma pergunta, não perguntas parecidas.
- Se não, entre em `sonda_codigo()` com `citar()`, que traz o trecho junto.
- Preencha as duas colunas citadas em `CITACOES`, ou marque `CITADO` com
  «não apurado» — célula em branco vira ausência aos olhos de quem lê.
- **Se a sonda mede EFEITO, escreva o controle antes do veredito.** É a regra
  que este diretório mais pagou para aprender.

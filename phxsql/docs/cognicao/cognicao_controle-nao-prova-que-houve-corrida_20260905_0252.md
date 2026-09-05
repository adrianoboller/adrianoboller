# O controle prova que o instrumento enxerga; ele não prova que houve corrida

- **Quando:** 2026-09-05, 02:52
- **Onde:** `bancada/acid/prova.py`, a matriz da leitura consistente da letra I
- **Custo:** zero, porque as duas células deram zero ao mesmo tempo e isso não
  fechava; teria sido a afirmação mais forte do `docs/ACID.md` publicada com
  nada por baixo

## O que aconteceu

A bancada do ACID mede se uma leitura enxerga o banco no meio de duas
gravações. O desenho já nascia com a pétrea do controle: duas linhas com soma
constante, um escritor transferindo, e o leitor perguntando de **duas** formas —
uma instrução (`varrer`, que devolve as duas linhas) e duas (`ler` + `ler`).
A célula de duas instruções era o **controle positivo**: se ela não quebrasse,
o medidor estaria cego e a outra não valeria.

Rodou. **As duas deram zero.** O controle não me salvou: ele deu zero junto.

Fui olhar. O leitor terminava as 400 voltas em **225 ms**; o escritor levava
**296 ms** só para abrir o soquete e fazer o login. Ele **nunca entrou no laço**
— zero gravações, e nem exceção houve, porque a thread só encontrou
`parar.is_set()` já verdadeiro e saiu limpa pelo `while`.

Os dois números eram de uma corrida em que só um dos lados existiu.

## O que eu concluí primeiro, e estava errado

Concluí que **as duas leituras eram atômicas** — que o `RwLock` da onda 1 estava
segurando a ficha compartilhada por tempo bastante para o par de `ler`
atravessar coerente, e comecei a escrever o parágrafo explicando o mecanismo.
A explicação era plausível, encaixava com o pedido 187, e estava inteiramente
errada: não havia mecanismo nenhum a explicar, porque não havia escritor.

O erro é do mesmo naipe do «o mutex serializa» do Profiler: **diagnóstico
plausível não é diagnóstico medido**, e o errado sobrevive melhor quando o
número que ele explica é o número que se esperava ver.

## O que a medição disse

Com o leitor esperando o escritor entrar no laço, e só parando depois de ele ter
dado no mínimo 40 voltas, os mesmos 400 giros contra ~300 gravações deram:

| o leitor pergunta | escritor sem transação | escritor em transação |
|---|---:|---:|
| **uma** instrução | 94–99 de 400 | **0** de 400 |
| **duas** instruções | 0–1 de 400 | 48–66 de 400 |

Três das quatro células mudaram de zero para um número, e a que **continuou**
zero passou a valer, porque agora existe o oposto ao lado.

E fica o registro do que a corrida vazia produzia: **zero nas quatro**, que é
exatamente o desenho «tudo consistente, isolamento perfeito» — o resultado mais
lisonjeiro possível, saído de nada.

## A regra

**Medição de concorrência publica o número do OUTRO lado.** Antes de acreditar
num zero, mostre quantas voltas o concorrente deu enquanto o instrumento
media — e faça o instrumento **esperar** por ele em vez de torcer.

## Como está guardado hoje

`transferir()` em `bancada/acid/prova.py` espera um `threading.Event` que o
escritor levanta depois da primeira volta completa, e só encerra quando ele já
deu `min_escritas` voltas. O número de voltas dele vai para o `resultado.json` e
para o `docs/ACID.md` («a corrida não foi vazia: o escritor deu N voltas…»), e a
afirmação `os_dois_lados_rodaram_juntos` reprova a corrida em que ele deu zero.

**Onde o buraco fica:** a guarda é desta bancada, e não da casa. As outras
medições de concorrência — `bancada/concorrencia/`, `bancada/mvcc/` — não têm
uma afirmação equivalente, e nada impede que uma delas já tenha publicado um
número de uma corrida em que o vizinho não chegou a rodar. Não foi medido nesta
rodada, e fica nomeado em vez de suposto resolvido.

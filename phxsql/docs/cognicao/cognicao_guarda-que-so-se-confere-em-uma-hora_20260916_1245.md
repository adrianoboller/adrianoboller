# Guarda que só se confere em uma hora é guarda que não se confere

**Descoberto:** 16/09/2026, 12:45
**Papéis:** G (QA, dona das catracas), F (prova real), A (integração)
**Pedido:** 263

## 1. O que aconteceu

A corrida completa do `provar-guardas.py` (começou 11:38, JSON gravado 12:41 —
**63 minutos**) devolveu:

```
142 guardas: 128 provadas, 3 redundantes, 0 nao pegaram, 0 estragaram, 11 quebradas
```

Zero reprovadas é a leitura fácil, e é a errada. **QUEBRADA não é uma nota
melhor que REPROVADA — é a ausência de nota.** A guarda não rodou: o `trecho`
que ela manda substituir para repor o defeito não existe mais no arquivo,
então o executor não tem onde aplicar a troca. E o catálogo continua contando
a entrada como cobertura, o que é exatamente a forma de mentira que uma
guarda deveria impedir.

## 2. O número, e o que ele separa

Medi commit a commit, e as onze viraram três coisas diferentes:

| o que é | quantas | quais |
|---|---:|---|
| quebrou **hoje**, consertável agora | 2 | `set-do-on-conflict-ignorado` (a frente U acrescentou o gancho BEFORE ao `upsert::aplicar`), `leitura-sem-recuo-para-a-exclusiva` (teste renomeado em `f2b87aa`) |
| **não estava quebrada** | 1 | `trava-sem-guarda-de-reentrancia` — trecho no lugar, defeito reposto derrubou o teste (1/1); estourou o **prazo de 420 s** com a suíte rodando ao lado |
| **dívida velha** | 8 | ver abaixo |

E a dívida velha tem um dono só, quase inteira:

| commit | data | guardas que aposentou |
|---|---|---:|
| `2fe8658` «a conferência de FK dentro da transação vê o pai empilhado» | 12/09 | **5** |
| `7d29f5f` ACID-C | 15/09 | 1 |
| `d59967a` pedido 227 | 08/09 | 1 |
| `2d33c5c` saúde do disco | 16/09 | 1 |

**Um commit aposentou cinco guardas de uma vez, e ninguém viu por quatro
dias.**

## 3. O que eu concluí primeiro, e estava errado

**Errei duas vezes, e as duas por medir de um jeito que não podia dar certo.**

**Primeiro erro — a busca binária.** Para achar quando cada trecho sumiu,
rodei uma busca binária sobre os commits do arquivo. Ela respondeu «nunca
casou em nenhum commit deste arquivo» para seis das oito, e eu quase escrevi
isso no pedido: teria virado uma acusação («o trecho foi digitado à mão e
nunca existiu») bem mais grave do que o fato.

Busca binária exige **monotonicidade**, e presença de um trecho no tempo não é
monótona: o código vai e volta. O que eu testava de fato era «o commit mais
antigo tem o trecho?» — e não tem, porque a entrada do catálogo é mais nova
que o arquivo. A varredura linear, que é o que a pergunta pedia desde o
início, achou o commit exato de cada uma.

**Segundo erro — varrer um terço da caixa.** A primeira versão do conferidor
procurava os testes nomeados só em `crates/<pacote>/src/` e acusou **154**
nomes mortos. Eram 154 falsos: o teste de integração mora em
`crates/<pacote>/tests/`. Se eu tivesse acreditado no número, teria «achado»
um desastre inexistente — e, pior, a catraca teria nascido em 154, um teto tão
frouxo que jamais seguraria nada.

É a mesma lei do KiB da interface (780 publicado, 1.032 medido) por outro
caminho: **quando um conferidor depende de uma lista, a lista tem de sair do
código** — e «os arquivos da caixa» é uma lista tanto quanto «os arquivos
embutidos no `http.rs`».

**Terceiro, e é o mais barato de errar:** escrevi no cabeçalho do conferidor
que ele roda «em menos de um segundo», porque *parecia* barato — não compila
nada. Medido: **31,6 s**, três corridas. Guardava o fonte inteiro numa string
e corria mais de quatrocentas buscas de expressão regular sobre os mesmos
megabytes. Uma passagem só, com o nome virando chave de conjunto, dá o mesmo
veredito em **0,18 s** — 180×. *Número citado é número que não se mede*, e eu
citei um dentro do arquivo cuja razão de existir é não deixar número
envelhecer.

## 4. A lei que sai disto

**Guarda que só se confere em uma hora é guarda que não se confere.**

O catálogo não falhou por desleixo de ninguém. Falhou por **custo**: 63
minutos, porque repõe o defeito e roda `cargo test` para cada uma das 142
entradas. Ninguém roda isso a cada commit, então o envelhecimento acontece
entre duas corridas e o intervalo é de dias.

A saída não é rodar o caro com mais frequência — é ter uma régua **barata**
que pegue a *classe* de envelhecimento, e dizer com todas as letras o que ela
**não** prova. `trecho-vivo.py` responde «o trecho ainda existe?» e «o teste
ainda existe?» em 0,18 s. **Ela não prova que repor o defeito derruba o
teste** — só o provador prova isso, e continua sendo ele a autoridade. Régua
barata que se vende como prova é pior que régua nenhuma.

E o corolário sobre a catraca: ela nasce em **8**, não em 0. Oito é o número
medido hoje; zero é o desejado. Catraca que nasce no desejado nasce vermelha,
e catraca vermelha vira catraca desligada na semana seguinte.

## 5. O que fica aberto, e por que não fechei

As **oito** entradas velhas, uma a uma. Não as consertei nesta rodada, e a
recusa é a parte que importa: cada uma pede ler o código de hoje, achar para
onde o ponto de reposição andou e **provar** que o defeito reposto derruba o
teste nomeado. Remendar o `trecho` para o que o compilador aceita produz uma
guarda que passa por engano — e *teste que passa por engano é pior que teste
que falta* vale para a guarda também, com o agravante de ela existir para
provar que os outros não passam por engano.

Fica aberto também o **prazo do provador**. `trava-sem-guarda-de-reentrancia`
não está quebrada; estourou 420 s com a suíte do workspace ao lado. Subir o
número no chute é o mesmo erro que a casa já pagou ao subir um teto porque a
régua mudou. O que isso pede é medir quanto ela custa com a máquina parada e
com a máquina carregada, e decidir com os dois números — não com a
impaciência de uma corrida.

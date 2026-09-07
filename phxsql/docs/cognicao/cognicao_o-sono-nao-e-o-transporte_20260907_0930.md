# O sono não é o transporte — e o número somava os dois

**07/09/2026, 09:30 UTC.**

## 1. O que aconteceu

O dono perguntou se dá para gravar por **quórum**, replicando para outros
servidores sem usar a replicação. Antes de responder qualquer coisa de
projeto, era preciso saber **quanto custa** esperar uma réplica confirmar.

O número que estava à mão era o da `bancada/replicacao/`: **826 a 2014 ms** de
atraso por operação. Com esse número, a resposta seria curta e errada: um
commit síncrono a 826 ms não se propõe a ninguém.

## 2. O que eu concluí primeiro, e estava errado

Que aquele atraso media o **transporte** — o tempo de o dado sair do master e
chegar na réplica. É a leitura natural do rótulo «atraso», e ela estava a um
passo de virar a resposta ao dono.

Aquela bancada roda com `reconectar_em: 2`, e o laço da réplica **dorme** esse
tempo quando não acha nada (`servidor.rs`: `Ok(0) => sleep(espera)`). O
atraso publicado é, quase todo, **sono** — e sono é escolha de configuração,
não custo de mecanismo.

O erro tem nome e já é lei aqui: *número citado é número que não se mede.* O
agravante é que este número **foi** medido, e corretamente — só que ele mede
**duas coisas somadas**, e o nome dele só anuncia uma. Medição honesta com
rótulo incompleto engana melhor que palpite, porque vem com autoridade.

## 3. O que a medição disse

`bancada/quorum/medir.py`, 60 voltas, master e duas réplicas em `127.0.0.1`,
com `reconectar_em` de **uma hora** nas réplicas para o laço delas não competir
com o cronômetro:

| | mediana | faixa |
|---|---:|---|
| gravar no master (o que se paga hoje) | **0,209 ms** | 0,171 – 3,357 |
| levar até UMA réplica, chamado na hora | **0,475 ms** | 0,386 – 4,000 |
| commit esperando 2 de 3 | **0,661 ms** | **3,16×** |
| commit esperando 3 de 3 | **0,704 ms** | **3,37×** |

**O transporte custa 0,475 ms, e não 826.** O sono era **99,9%** do número
publicado. Um quórum síncrono custaria **3,2×** o commit — caro, e a ordem de
grandeza de uma decisão de produto, não de uma impossibilidade.

Tudo em localhost, e o medidor carrega o aviso junto do número: é o **piso**.

## 4. E a medição me contou a arquitetura, que eu não tinha perguntado

O medidor teve de puxar os eventos **à mão, do Python**, e eu escrevi isso como
comodidade de bancada. Não era. A replicação daqui é **pull** — *«quem procura
é a réplica; o source não empurra nada»*, por firewall —, e um quórum síncrono
exige o master saber, **no instante do commit**, que N réplicas têm o dado.
Com pull ele não tem como fazer ninguém buscar.

Ou seja: **o obstáculo ao quórum nunca foi o custo; é a direção da conexão.** E
quem me disse isso foi a forma do medidor, não a leitura do código — eu li o
`replica.rs` antes e não tinha tirado a conclusão.

## 5. A regra

**Antes de usar um número medido por outra bancada, pergunte o que ele SOMA.**
Rótulo de medida nomeia a intenção de quem mediu, não o conteúdo do número — e
duas grandezas somadas sob um nome só passam por uma delas.

E o corolário: **a forma que um medidor precisa ter para funcionar é
informação sobre o sistema.** Quando escrever um medidor exigir um andaime que
o produto não tem, o andaime é o achado.

## 6. Como está guardado hoje

- `bancada/quorum/medir.py` separa gravar, levar e o quórum de 2-de-3 e 3-de-3,
  grava `resultados.json`, e **para** se uma réplica puxar sozinha (zero
  eventos significa que ela chegou antes, e o medidor estaria cronometrando o
  próprio concorrente) ou se o esquema não a alcançar antes da primeira volta.
- A bancada está **declarada** na `pagina-dos-testes.py`, com a nota de que ela
  **não prova recurso** — mede o preço de uma decisão que ainda não foi tomada.
- `docs/REPLICACAO.md` §19 e o pedido 207 carregam a análise inteira.
- **Onde o buraco fica:** os 826 ms continuam publicados na bancada de
  replicação **sem dizer que são sono**. O nome do campo é `atraso_ms`, e ele
  vai enganar o próximo que o ler — inclusive eu, de novo. Separar aquele
  número na origem é trabalho que não fiz nesta rodada.

# Instrumento quebrado não soma ruído — ele ESCONDE, e escolhe o que esconder

**04/09/2026, 08:52.** Descoberto uma hora depois de eu consertar o
instrumento, ao refazer a medição que eu **não** achava que precisasse ser
refeita.

## 1. O que aconteceu

Às 06:12 publiquei, com número, que o teto exclusivo do MVCC — o que **só** ele
compra, que é a diferença entre ter um **escritor** ao lado e ter outro
**leitor** ao lado — dava **1,00× · 0,91× · 1,13× · 1,02×** em duas baterias
limpas. Escrevi que ele *«não se distingue do ruído desta bancada»*, e que a
Sombra teria de se justificar por leitura repetível, não por velocidade.

Às 07:52 descobri que a bancada mandava `"limite"`, campo que o `op_varrer` não
lê: **toda leitura era de 1.000 linhas**, não das 50 que o rótulo dizia.

Refeito com o campo certo, na carga de **50 linhas** (a de uma página de
grade) e durabilidade `por_operacao`, o mesmo teto exclusivo dá **3,23× e
2,77×**.

## 2. O que eu concluí primeiro, e estava errado

**Duas vezes, e a segunda é a que ensina de verdade.**

**Primeiro:** que um instrumento errado *adiciona ruído*. É o modelo mental
natural — medida suja, número disperso, conclusão insegura. Aqui aconteceu o
contrário: os quatro números eram **coerentes entre si** (1,00 · 0,91 · 1,13 ·
1,02, quatro medições dentro de 22%) e apontavam com firmeza para uma
conclusão **falsa**. Um instrumento quebrado pode ser **preciso e errado**, e
a precisão é justamente o que faz ninguém desconfiar.

**Depois, e este é o pecado:** ao consertar o instrumento, refiz a §13 —
a medição cuja conclusão eu **duvidava**, porque a tinha acabado de publicar e
retratar. **Não refiz a §11 no mesmo movimento**, e a §11 era a que sustentava
uma recomendação de arquitetura. Só a refiz uma hora depois, por completude.

O critério que eu usei sem perceber foi *«refazer o que parece errado»*. O
critério certo é **«refazer tudo o que o instrumento tocou»** — porque o que
parece certo é exatamente onde um instrumento preciso-e-errado se esconde.

## 3. O que a medição disse

O mesmo teto exclusivo, nas duas cargas, em baterias limpas:

| carga da leitura | `por_lote` (padrão) | `por_operacao` |
|---|---:|---:|
| 1.000 linhas (o instrumento quebrado) | 1,00× e 0,91× | 1,13× e 1,02× |
| **50 linhas** (o certo, a carga da tela) | 1,21× e 1,00× | **3,23× e 2,77×** |

E o mecanismo, das medianas da própria corrida:

```
por_operacao, leitor com pagina de 50 linhas:
  sozinho ............ p99    738 us
  com outro LEITOR ... p99    911 us   (+23%  -- o que o RwLock recupera)
  com um ESCRITOR .... p99  2.527 us   (3,4x  -- o resto so o MVCC tira)
```

A diferença entre 911 e 2.527 µs é o `fsync` sob a trava global — **1.267 a
1.371 µs medidos na §7.1**. Com a leitura de 1.000 linhas custando 6.500 µs,
esse `fsync` era **20% do numerador e 20% do denominador**, e a razão dava
~1,0. **A leitura cara afogava o efeito que se procurava.**

O custo do protocolo, medido e não citado: **duas baterias limpas em seis
tentativas.** Três reprovações foram vizinho meu — o aviso de comunicação, um
`push` que rodei logo depois de lançar, e o próprio batimento de 15 minutos
disparando dentro da janela. As outras duas foram um pico numa única amostra
entre 38.

## 4. A regra

**Instrumento quebrado não soma ruído: ele esconde, e o que ele esconde
depende do defeito.** Um denominador inflado não deixa a medição imprecisa —
deixa-a precisamente errada, e no sentido mais traiçoeiro: fazendo um efeito
real parecer ruído.

E o corolário do erro nº 2, que é o operacional: **consertado o instrumento,
refaz-se TUDO o que ele tocou — não só o que parecia errado.** O que parecia
certo é onde ele está escondido.

## 5. Como está guardado hoje

* **A §11.2-bis** traz as quatro medições nas duas durabilidades, e a **§11.3**
  deixou de dizer «a Sombra não compra desempenho»: hoje diz que ela **compra
  onde o `fsync` acontece em toda gravação e não compra onde ele acontece uma
  vez por janela** — e que qual dos dois mundos vale é o
  `recursos.durabilidade`, escolha do dono do banco.
* **O aviso no topo do `PESQUISA-MVCC-E-FORMATO.md` foi refeito.** Ele tinha
  sido escrito às 06:17 com a metade que eu media então, e é a primeira coisa
  que alguém lê ao chegar para justificar a Sombra.
* **O pedido 179 leva a inversão**, e o 183 leva o defeito do campo.
* **As corridas cruas das duas cargas estão versionadas** lado a lado, com
  `CERTO` no nome das boas. As invalidadas ficam: apagá-las perderia a série e
  a lição.
* **Onde o buraco ficou:** a §7.1 — o µs de trava presa por operação — ainda
  não foi refeita na carga certa. É a última das três, está nomeada na §8, e
  uma corrida curta de fumaça sugere que a trava presa lendo cai de ~3.150 µs
  para a ordem de 500 µs. **Não medido em bateria limpa, e não estimado.**
* **E uma guarda que NÃO foi mexida, de propósito:** o `quieta.Vigia` reprovou
  quatro das seis tentativas, e em duas delas eu já tinha a hipótese de que o
  desconto `meus` não conta o processo-pai amostrador. **Não subi o desconto.**
  Afrouxar a catraca para o próprio número passar é o que esta casa proíbe com
  mais clareza; se o ponto cego for real, ele se mede pelo
  `ruido-do-controle.py`, que existe para isso — e aí a catraca **aposenta e
  renasce**, não sobe.

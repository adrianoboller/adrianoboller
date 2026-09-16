# A catraca conta o DEFEITO; quem conta a COBERTURA é outro número

*16/09/2026, 16h15 — frente G (QA). Alcance de duas pétreas já escritas:
«catraca só desce» e «lei que lista menos casos do que existem».*

## 1. O que aconteceu

Três buracos, medidos e nomeados na mesma manhã de 16/09 e consertados à
tarde. Os três parecem independentes e têm **uma raiz só**.

**(a) A régua nova via uma das cinco formas de quebra.**
`bancada/guardas/trecho-vivo.py` (pedido 263) pergunta se o `trecho` de cada
entrada do catálogo ainda existe no arquivo. O `provar-guardas.py` conhece
**cinco** motivos de `QUEBRADA`: trecho ausente, trecho em dobro, o código
trocado não compila, abortou sem ser esperado, e estourou o prazo. Medido no
dia: a catraca dizia `ok 0` enquanto o provador dizia **1 QUEBRADA** — a
`trava-sem-guarda-de-reentrancia`, que estoura os 420 s.

**(b) A catraca não distinguia conserto de apagamento.**
`TETO_TRECHO_MORTO` desceu de 8 para 0 quando as oito entradas velhas foram
consertadas. **Apagar as oito teria medido exatamente o mesmo 0** — e apagar é
o caminho barato.

**(c) A tabela que diz quantas catracas há contava menos do que existe.**
`docs/qa/medir.py` varria `crates/*/examples/*.rs` e `crates/*/src/**`. As
catracas que moram em Python, em `bancada/`, não apareciam. O
`docs/QA-PDCA.md` publicado trazia **8**; o gerador consertado mede **20**.

## 2. O que eu concluí primeiro, e estava errado

**Errei duas vezes, e a segunda é a que ensina.**

**Primeiro erro.** Li o parágrafo do `docs/CATRACAS.md` §13 que já denunciava
o buraco (c) — *«são **sete** catracas vivas que a tabela gerada não conta»* —
e tratei o sete como inventário: bastava fazer o gerador enxergar aquelas
sete. Fui contar uma a uma antes de escrever o código, por hábito, e eram
**oito**: o parágrafo esquecia a `TETO_PKILL_SEM_PID` da §11, que é da mesma
família e mora na mesma pasta. **O parágrafo que denunciava uma lista curta
era, ele próprio, uma lista curta.** Se eu tivesse confiado nele, teria
consertado o gerador deixando uma catraca de fora — e com a sensação de ter
fechado o buraco, que é o pior estado.

**Segundo erro, sobre o buraco (a).** Concluí que a saída era **alargar a
régua** do `TETO_TRECHO_MORTO` para contar também o trecho em dobro: é a mesma
pergunta, é o mesmo arquivo, é uma linha. Só que régua que passa a medir mais
**aposenta** a catraca antiga — é a lei, e ela custaria a série do `8 → 0`
medido na mesma manhã. A saída certa é outra e é mais barata: **catraca nova
ao lado**, no número medido do dia, com a régua velha intocada. Alargar a
velha custa a série; a nova ao lado custa um nome.

E o corolário que só aparece depois de ver os dois: **quando as duas saídas
existem, a que não mexe na régua é a certa** — não por elegância, mas porque a
aposentadoria é um preço que só se paga quando não há alternativa.

## 3. O que a medição disse

**A raiz comum dos três.** Um teto conta **ocorrências do defeito**. Zero
ocorrências é compatível com dois mundos opostos:

| o mundo | `TETO_TRECHO_MORTO` |
|---|---|
| as oito entradas consertadas e provadas | **0** |
| as oito entradas apagadas do catálogo | **0** |

E o mesmo vale para (a) e (c): `ok 0` numa régua que vê uma das cinco formas é
compatível com «o catálogo está inteiro» e com «há uma quebrada que esta régua
não vê»; «8 catracas» numa tabela que varre só Rust é compatível com «há oito»
e com «há vinte». **Em todos os três casos o número está certo e a leitura
está errada** — e a leitura errada é a que alguém vai fazer, porque o número é
o que está à mão.

**Os números do dia, medidos:**

- formas de `QUEBRADA` que a régua barata passou a ver: **2 de 5** → **4 de 7**
  motivos (contando arquivo ausente e teste ausente separados), e as três que
  sobram custam compilar ou rodar;
- catracas na tabela gerada: **8 → 20** (nove em Rust, onze em `bancada/`);
- entradas do catálogo de guardas: **143**, que viraram o `PISO_DAS_ENTRADAS`;
- custo da régua: **0,171–0,183 s** com uma pergunta, **0,47 s** com quatro
  escritas do jeito óbvio, **0,200–0,206 s** depois de três consertos medidos
  um a um (passagem única guardada por arquivo; catálogo lido uma vez; o crivo
  de `mod x;` só nos 57 arquivos de `tests/`, dos 276).

**A prova de que a classe nova é a mesma do provador foi exercitada, não
afirmada**, e sem compilar nada: a própria função `Arvore.repor` do
`provar-guardas.py` foi chamada sobre o arquivo mutado numa cópia de um
arquivo só, e devolveu *«o trecho aparece 2 vezes … trocar a errada provaria
outra coisa»* — literalmente o motivo que vira `QUEBRADA`. Quando a régua
barata afirma cobrir uma classe da régua cara, a função da régua cara é quem
tem de dizer isso.

## 4. A regra

**Toda catraca que conta defeito pede um piso que conte cobertura — e toda
régua que cobre parte de uma classe imprime, junto do número, a parte que não
cobre.**

E a regra da aposentadoria, que sai do segundo erro: **pergunta nova sobre o
mesmo alvo nasce em catraca nova ao lado, não alargando a régua da velha.**
Aposentar é o preço de mudar a régua, e só se paga quando não há alternativa.

## 5. Como está guardado hoje, e onde o buraco ficou

**Guardado:**

- as quatro perguntas e o piso em `bancada/guardas/trecho-vivo.py`
  (`TETO_TRECHO_MORTO`, `TETO_TRECHO_AMBIGUO`, `TETO_TESTE_MORTO`,
  `TETO_TESTE_FORA_DO_BINARIO`, `PISO_DAS_ENTRADAS`), com as cinco provadas
  nos dois sentidos e a lista «o provador continua dono de: …» impressa em
  toda corrida;
- a aposentadoria **escrita** (`APOSENTADAS`, com id, data e motivo) como
  única saída legítima para tirar uma entrada — e uma aposentada que volte ao
  catálogo reprova, porque contaria dos dois lados;
- o inventário em `docs/qa/medir.py`, que agora varre `bancada/**/*.py` pelo
  **mesmo crivo** do Rust (quem escreve `catraca:nome=` responde a
  `--numeros`), com `tipo=teto`/`tipo=piso` **opcional** — quem não diz
  continua sendo teto, e os nove conferidores em Rust não mudaram uma linha;
- o gerador dessa tabela no `PLANO` do `portao-dos-geradores.py`, que não o
  tinha: foi por isso que o `docs/QA-PDCA.md` publicava 8 catracas e um teto
  de idiomas em 1.050 quando já era 1.049.

**Onde o buraco ficou, nomeado:**

- as **três** formas de `QUEBRADA` que só existem depois de compilar e rodar
  continuam só com o provador, e ele continua custando cerca de uma hora. A
  régua barata não as tem e **diz que não as tem**;
- a correspondência da forma `faltando` (teste fora do binário) com o veredito
  do provador foi **lida do fonte**, não medida de ponta a ponta: a cópia dele
  estava vazia e uma corrida `--so` custaria uma compilação fria de ~1,2 GB
  numa árvore com 7,9 GB livres. A do trecho em dobro foi exercitada;
- a varredura de órfãs em Rust continua só `pub const TETO*`: sete `const`
  privados e dezenove `MAX_*`/`LIMITE_*` continuam invisíveis a ela. A régua
  **não** mudou nesta frente — por isso nenhuma catraca de lá se aposentou —, e
  o buraco segue medido na tabela de limites do `docs/CATRACAS.md`;
- a `alcancam-fsync` continua **23 com teto 22**, vermelha e parada com o dono
  (#252 metade 1). Nada aqui a tocou; ela apenas passou a **aparecer na tabela
  gerada**, como `REPROVANDO — 1 acima`, em vez de não constar.

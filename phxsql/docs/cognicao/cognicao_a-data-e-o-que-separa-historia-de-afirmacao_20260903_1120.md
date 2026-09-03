# A data ao lado é o que separa história de afirmação sobre o presente

**Descoberto:** 03/09/2026, 11:20.
**Onde:** `docs/PENDENCIAS.md` (pedido 145), `docs/QA-PDCA.md`; a pétrea
«todo número visível sai de um gerador».

## 1. O que aconteceu

Três coisas na mesma rodada apontaram para o mesmo lugar:

* a frente da concorrência achou **três números do `CONCORRENCIA.md` velhos** —
  e o gerador que os produz **existia**, ninguém o rodava de novo;
* ela achou o `PENDENCIAS.md` dizendo **«37 guardas»** com o catálogo em **60**;
* eu tentei contar as sprints do `ROTEIRO-1.0.md` por script e **ele errou duas
  linhas**, porque a coluna «estado medido» é prosa, não dado.

Parecia uma classe de defeito pedindo um conferidor: varrer os documentos atrás
de quantidades que um gerador já produz e reprovar quem divergir.

## 2. O que eu concluí primeiro, e estava errado

Que valia escrever esse conferidor. Fui medir o tamanho da classe antes — e ela
**não existe no tamanho que eu supunha**.

`grep` por «N guardas» em `docs/` dá **seis** ocorrências. Lidas uma a uma:

| onde | o que é |
|---|---|
| `PENDENCIAS.md:158` | **defeito real** — 37 sem data, com o catálogo em 60 |
| `QA-PDCA.md:257` | história correta: *«(medido em 2026-08-30 17:28)»* |
| `QA-PDCA.md:469` | afirmação corrente e **certa** (60/56) |
| `QA-PDCA.md:847` | idem, e diz *«nesta mesma rodada»* |
| `QA-PDCA.md:862` | história de uma passada específica |
| `QA-PDCA.md:889` | `~20`, estimativa de **subconjunto**, não total |

**Uma em seis.** Um conferidor ingênuo reprovaria as outras cinco — e o meu
próprio `grep` já tinha provado isso ao capturar «**15 pedidos**», que é prosa
dentro de uma entrada e não a contagem. Máquina que acusa cinco inocentes para
achar um culpado não é catraca: é ruído que se aprende a ignorar, e catraca
ignorada não segura nada.

## 3. O que a medição disse

O que separa as cinco legítimas da defeituosa **não é o número** — é a
**atribuição**, e ela tem três formas nesta casa:

1. **data explícita** — «medido em 2026-08-30 17:28»;
2. **âncora de rodada** — «nesta mesma rodada», «fresca desta rodada»;
3. **til de estimativa** — `~20`, que declara subconjunto e não total.

A do `PENDENCIAS.md` não tinha nenhuma das três. E ela tinha um segundo
defeito, que só apareceu lendo: a frase estava **truncada por um merge** —
sobrava `…as seis frentes.md\` sai de um gerador, não da mão`, que é a cauda de
«o número do `docs/TESTES.md` sai de um gerador». O número errado escondia uma
frase quebrada, e nenhum conferidor de número teria achado a segunda.

## 4. A regra

**Número em documento ou traz atribuição, ou é afirmação sobre o presente e tem
de sair de gerador.** Data, âncora de rodada ou til de estimativa — uma das
três. Sem nenhuma, o leitor lê como «é assim agora», e é assim que 37 vira
mentira sem ninguém digitar nada de novo.

E a regra de método, que é a que me segurou: **medir o tamanho da classe antes
de escrever a máquina que a caça.** Seis ocorrências se leem em dois minutos;
um conferidor com 83% de falso positivo custa uma frente e um teto que ninguém
respeita.

## 5. Como está guardado hoje

* A frase do `PENDENCIAS.md` foi **reparada e datada**: «Medido em 02/09/2026:
  37 guardas», mais o comando que dá o número de agora
  (`provar-guardas.py`) e a frase que restou do merge, remontada.
* **O conferidor NÃO foi escrito**, e esta é a recusa medida: 1 defeito em 6,
  com 5 que um matcher ingênuo reprovaria. Se a classe crescer — e o jeito de
  saber é refazer este `grep` —, o conferidor certo não procura números: procura
  **número sem atribuição**, que é o crivo que classificou as seis aqui.
* O parente disto já tem item próprio: o **pedido 167** separa «versão corrente»
  de «história» pelo mesmo motivo, e mediu **seis menções históricas** que um
  `sed` global transformaria em afirmações falsas sobre o passado. É a mesma
  doutrina em dois lugares, e agora com o discriminador nomeado.

**Onde o buraco ficou:** a coluna «estado medido» do `ROTEIRO-1.0.md` continua
sendo **prosa**, e foi ela que fez o meu script errar duas sprints. Transformá-la
em algo que um gerador leia é item próprio, e não cabia nesta rodada porque três
frentes escrevem nesse arquivo agora.

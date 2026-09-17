# A premissa do integrador é a mais perigosa — três invertidas no mesmo dia

**Descoberto em 17/09/2026, 14:50 UTC**, ao integrar a frente do `Uuid` que
nasce sozinho e ver a terceira premissa minha cair medida em nove horas.

## 1. O que aconteceu

Três vezes hoje eu entreguei uma premissa técnica como se fosse fato, e três
vezes o papel dono do domínio a derrubou **com número**:

| hora | o que eu afirmei | quem derrubou | com o quê |
|---|---|---|---|
| ~11:30 | adiar o `.ndx` compra «5 a 6×» | a própria medição | comparei custo proporcional a **M** com custo proporcional a **N** |
| 14:0x | o `Uuid` v7 é a identidade que resolve 20 caixas | papel **C** (DBA) | +47,5% no `.ndx` (239 → 162 chaves/folha) e perda de localidade **na reconexão** |
| 14:4x | fechar o `Uuid` nulo «custa uma linha», como o `AUTONUMBER.md` diz | papel **B** (engenheiro) | a linha inventaria pai inexistente **depois** da conferência de FK |

A terceira é a que dói, porque ela **não era um número errado: era um defeito
pronto para entrar.** `conferir_fks_com` roda em `table.rs:3225` e o `numerar`
em `:3241` — dezesseis linhas depois —, e a conferência tem escrito no corpo
*«NULO satisfaz: nada a procurar»*. Gerar um v7 em toda coluna `Uuid`, como eu
mandei, poria um id inventado numa coluna de referência **depois** de o portão
ter deixado o nulo passar. A órfã entra e ninguém vê, e a pétrea primordial cai
pelas mãos do próprio motor.

E o agravante: **esta casa já tinha pago esse defeito, do outro lado.** O
`PENDENCIAS.md` registra que a primeira versão do conserto na tela *«preenchia
toda coluna `Uuid`, inclusive a estrangeira, o que geraria um id sorteado
apontando para nada — quem viu isso foi a captura de tela»*.

## 2. O que eu concluí primeiro, e estava errado

Concluí que a cláusula dos dez papéis é **divisão de trabalho**: eu recorto, eles
executam, eu integro. Sob essa leitura, a minha premissa é o enunciado do
problema e o trabalho do papel é resolvê-lo.

Está errado, e as três linhas da tabela acima são a prova. Sob essa leitura,
nada teria pego o erro do `Uuid`: o engenheiro teria escrito a linha que eu
mandei, os portões teriam passado — **`fmt`, `clippy` e a suíte inteira passam
com esse defeito**, porque não há teste para uma órfã que ainda não existe —, e
o defeito entraria assinado por mim.

## 3. O que a medição disse

O que salvou não foi portão nenhum: foi o papel **recusar a premissa**. E isso
só aconteceu porque o contrato dizia, na letra, que ele podia — duas frases que
eu escrevi por hábito e que hoje viraram a peça mais importante do contrato:

> *«Se durante o trabalho você concluir que o item não é tão simples quanto a
> premissa diz, **pare e diga**.»*
>
> *«Premissa que morre medida é resultado tão válido quanto o conserto.»*

Contado no que entrou: dos **664** acrescentados, **414 são teste** (254 no
`identificadores.rs`, 160 no `servidor.rs`). O código que fecha o item cabe numa
função de vinte linhas; o resto é a prova de que ele não faz o que eu tinha
mandado fazer. **A guarda que mais importa é a que impede a minha instrução.**

E o vermelho que eu mesmo repus, para não acreditar no relatório: tirada a
guarda da coluna de referência, cai **exatamente um** teste, com a asserção
medindo o **efeito** — «a primaria que tambem e' referencia nao pode se
inventar: **1**», que conta a linha que entrou, e não se o motor recusou.

## 4. A regra

**A premissa do orquestrador não é enunciado: é hipótese, e quem a mede é o
papel dono do domínio.** Todo contrato de frente carrega, explícita, a licença
de recusar — sem ela a cláusula dos dez papéis vira divisão de trabalho, e
divisão de trabalho propaga o erro de quem divide.

E o corolário que a terceira vez ensinou: **quando eu cito um documento nosso
como premissa, eu ainda não medi nada.** O «custa uma linha» estava escrito no
`AUTONUMBER.md`, de boa-fé, por quem mediu outra coisa. *A receita de um número
envelhece* — e a receita de um **plano** também.

## 5. Como está guardado hoje

**Guardado no hábito e em três contratos; não há guarda nenhuma.**

- As duas frases de licença estão nos contratos que escrevi hoje, e é a elas
  que eu credito o conserto. **Não estão em lugar nenhum que sobreviva a mim**:
  não há molde de contrato versionado, nem conferidor que reprove um contrato
  de frente sem a licença de recusa. Um `.claude/agents/*` não cobre isso —
  ele descreve o papel, não o contrato da tarefa.
- **Os portões não pegariam.** Vale escrever por extenso, porque é
  contraintuitivo: `fmt`, `clippy --all-targets` e a suíte de 2.499 testes
  passariam limpos com o defeito que eu mandei fazer. Guarda só pega defeito
  que alguém já imaginou; a premissa errada do orquestrador é, por construção,
  a que ninguém imaginou ainda.
- O que caberia como guarda de verdade é do lado do processo, não do código:
  **todo contrato de frente termina com a licença de recusar**, e o relatório
  que volta diz **o que da premissa morreu**. Fica nomeado, não feito.

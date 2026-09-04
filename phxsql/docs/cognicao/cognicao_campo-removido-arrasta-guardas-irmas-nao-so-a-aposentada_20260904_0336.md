# Campo removido arrasta as guardas IRMÃS do catálogo, não só a que se aposentou

*Descoberto às 03:36 de 04/09/2026, consertando as duas entradas QUEBRADAS que
`python3 provar.py --so guardas` apontou na corrida de 04/09.*

## 1. O que aconteceu

`Table::inserir` e `Table::restaurar`, em `table.rs`, confериam chave
estrangeira lendo `!self.fks_conferidas.is_empty()` — um campo do struct,
lista em cache. Numa rodada anterior (03/09), esse campo foi **removido** e
substituído por uma função calculada na hora, `fks_que_conferem(&self.esquema)`
(`docs/PESQUISA-ESTADO-DERIVADO.md`: a lista comprava 0,28-0,86 ns e calcular
na hora custa 0,92-1,37 ns — diferença menor que 1 ns não paga o preço de ter
estado derivado que pode envelhecer).

Essa mesma refatoração já tinha **aposentado corretamente** uma terceira
guarda, `portao-de-fk-com-esquema-velho`, com um comentário de trinta linhas no
próprio `catalogo.py` explicando o porquê (o defeito que ela guardava — o
`redeclarar_chaves_estrangeiras` deixando um índice velho apontando para a
lista — deixou de poder existir, porque não há mais lista para desalinhar).

O que essa rodada **não fez** foi procurar as outras entradas que citavam o
mesmo campo. `replica-julga-fk` (em `inserir`) e
`restaurar-nao-pergunta-pela-mae` (em `restaurar`) continuaram com o texto
velho, e só apareceram **QUEBRADA** na corrida seguinte de
`provar-guardas.py` — dias depois, não na hora da refatoração.

## 2. O que eu concluí primeiro, e estava errado

Pensei que o conserto era trocar só o nome do campo dentro do `trecho` e da
`troca`, mantendo o resto do texto idêntico — inclusive o comentário curto
`// Numerar ANTES das chaves` que fechava o trecho de `replica-julga-fk`.

Não bastava. O comentário **também tinha crescido**
(`// Numerar ANTES das chaves, pela mesma razao da sequencia: se a coluna`), e
o `if` novo (`fks_que_conferem(&self.esquema).next().is_some() &&
self.julga_integridade()`) aparece **três vezes** no arquivo — em `inserir`,
em `atualizar` e numa terceira função. Cortando o trecho na forma antiga eu
teria produzido uma entrada que, ou não bate em lugar nenhum, ou bate mais de
uma vez — e o próprio executor recusa isso (`"o trecho aparece %d vezes...
trocar a errada provaria outra coisa"`).

Eu tratei "o campo mudou de nome" como se fosse a única coisa que mudou. O
texto vizinho — o comentário que dava unicidade ao trecho — tinha mudado
junto, e só a contagem por `grep -c` contra o `table.rs` de hoje revelou isso;
ler o diff mental do campo não mostrava.

## 3. O que a medição disse

| checagem | resultado |
|---|---|
| `self.conferir_aridade(valores)?;` no arquivo | 2 ocorrências (`inserir`, `atualizar`) |
| `fks_que_conferem(&self.esquema).next().is_some() && self.julga_integridade()` | 3 ocorrências |
| `// Numerar ANTES das chaves` (a forma NOVA, mais longa) | 1 ocorrência — foi ela que devolveu a unicidade |
| `fks_conferidas` em `catalogo.py`, fora do comentário da aposentada | 2 entradas (as duas quebradas) |

Depois do conserto: `provar-guardas.py --so replica-julga-fk --so
restaurar-nao-pergunta-pela-mae` → as duas **PROVADA**, 2/2 e 1/1 caíram. A
corrida cheia fechou em **77 guardas: 73 provadas, 4 redundantes, 0
quebradas** (era 71 provadas + 2 quebradas antes deste conserto).

## 4. A regra

**Quando um campo que uma guarda cita for removido ou renomeado, procure TODAS
as entradas do catálogo que citam esse campo — não só a que motivou a
aposentadoria de outra guarda.** E ao remendar um trecho, não presuma que só a
linha do campo mudou: confira por contagem (`grep -c` contra o arquivo de
verdade) que o trecho novo continua único, porque o texto vizinho — aqui, um
comentário — pode ter crescido e ser exatamente o que hoje dá a unicidade.

## 5. Como está guardado hoje

Os comentários novos junto às duas entradas em `bancada/guardas/catalogo.py`
explicam a origem do envelhecimento e apontam para a entrada `APOSENTADA em
03/09/2026: portao-de-fk-com-esquema-velho` como a irmã que já tinha sido
tratada na mesma refatoração. **Não há verificação automática** que, ao
aposentar uma guarda por causa de um campo removido, liste as outras entradas
que citam o mesmo campo — isso continua dependendo de rodar
`provar-guardas.py` até o fim e ler quem saiu `QUEBRADA`, o que só acontece na
corrida seguinte e não no commit que fez a remoção.

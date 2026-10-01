# Um integrador só comita

**Regra.** Frentes paralelas escrevem e provam; **só o integrador comita**, por
caminho explícito (nunca `git add -A`), e só depois dos portões na **árvore
exata** do commit. Frente que terminou devolve a cópia e o relatório, não o
commit.

**Cicatrizes.**
- Numa rodada de seis frentes, **três defeitos só apareceram no encontro
  delas**: um teto de memória que uma frente pôs e a outra apagaria sem
  conflito nenhum aparecer; uma bateria com dezoito partes quando cada frente
  contou dezessete; uma seção inteira de documento perdida porque o merge
  escolheu o lado de quem não a tinha. Nenhuma frente sozinha via isso.
- Um número digitado envelheceu em noventa minutos porque duas frentes mexeram
  na mesma catraca sem se verem.
- Conflito em arquivo de catálogo (lista de entradas com `id`) não se resolve
  escolhendo um lado: parte do lado integrado, traz da frente as entradas
  novas e as que **só ela** mudou, e manda para a mesa a entrada mudada pelos
  dois lados (`scripts/mesclar_catalogo.py`).
- Rascunho compartilhado entre agentes: um `tar -x` num diretório de nome
  genérico sobrescreveu a árvore de outra frente. Cada frente escreve só em
  subdiretório com o nome do pedido.
- Portões verdes não substituem o parecer de revisão num lote de risco: o
  revisor mediu 5 de 5 rodadas com o defeito na árvore integrada, com todos os
  portões verdes.

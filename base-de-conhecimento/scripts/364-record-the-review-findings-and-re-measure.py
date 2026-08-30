# Record the review findings and re-measure
# 28/08 13:18

import pathlib
p = pathlib.Path('docs/PENDENCIAS.md')
s = p.read_text()
v = '''- **A árvore roubava a tela de quem pintasse depois dela.**'''
n = '''### O que a revisão do dossiê achou

Duas coisas erradas na própria página, nenhuma no código.

- **Os números do índice lateral estavam fora de ordem.** Do item 4 ao 10 eles
  ficaram um atrás — «04 Paginação» quando Paginação é a 05 —, e o item 4
  perdeu o zero à esquerda. Os *links* apontavam certo o tempo todo; só o número
  exibido divergia. Aconteceu quando uma seção entrou no meio e os
  `<section>` foram renumerados sem o índice. Agora o número sai do próprio
  alvo (`#s7` mostra 07), então não tem como divergir de novo.

- **Duas afirmações defasadas**: «9 comandos» na linha de comando (são 11) e
  «30 das 33 operações na tela» (são 33 das 36). As duas em dois lugares cada.

E uma correção de arrumação: o texto sobre os metadados de campo, a chave
primária e a partição por calendário tinha entrado dentro de *Estado e
roteiro*, que é o roteiro — não o lugar de explicar formato. Foi para onde
pertence: campo e chave na seção 3 (*A tabela, peça a peça*), partição na 5
(*Paginação*). As figuras foram renumeradas em ordem de leitura.

- **A árvore roubava a tela de quem pintasse depois dela.**'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')

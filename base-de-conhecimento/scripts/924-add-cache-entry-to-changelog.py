# Add cache entry to CHANGELOG
# 29/08 00:23

import pathlib
p = pathlib.Path("CHANGELOG.md")
s = p.read_text()
alvo = '''### Mudado

- **O erro do protocolo chega inteiro à tela.**'''
novo = '''- **Cache de páginas no `.ndx`** (pedido 113, e não pelo caminho que o pedido
  supunha). A inserção com dois índices caiu de **44,4 para 18,5 µs por linha —
  2,40×** —, e a carga em lote pela rede subiu de **25.985 para 37.021
  linhas/s**. Sem mudar formato, sem mudar garantia e sem tocar na B+tree.

  O pedido dizia «ordene as chaves do lote, para chaves vizinhas caírem na mesma
  folha». Medi antes: **a desordem custava 1,06×**. O custo não era de
  localidade — era de **reler do arquivo e recalcular o CRC-32 da mesma página**
  a cada descida da árvore, e a raiz é a mesma página em todas as inserções da
  carga. O medidor agora **conta** os toques em vez de citar um `strace`
  antigo: 8,80 páginas servidas de RAM, 2,06 gravadas, 10,86 no total — não os
  ~20 que estavam escritos. A 2,34 µs de CRC por página, eram **25,4 µs por
  linha só de CRC**, de 44,4 medidos.

  Com isso, a linha que mais mudou é a que confirma o diagnóstico: **conferir a
  chave única caiu de 20,5% para 2,3%** do tempo de uma inserção. É uma descida
  na árvore que não escreve nada — exatamente o trabalho que o cache serve de
  graça. E o `.ndx` caiu de 83,5% para 63,6% do total.

  **O cache é de leitura.** Toda gravação atravessa para o arquivo na hora.
  Segurar página suja daria mais e trocaria uma garantia por desempenho sem
  avisar: hoje só uma queda da máquina atrasa o `.ndx` em relação ao `.reg`, e
  não uma queda do processo. O despejo é por segunda chance, senão a raiz — a
  página mais visitada — sairia junto com as outras assim que o teto enchesse.
  O teto de 2.048 páginas (8 MiB) saiu de uma varredura de quatro tamanhos, em
  `docs/DESEMPENHO.md` §2.1.

  **Ordenar as chaves continua não feito**, agora com número: depois do cache a
  desordem passou a custar **1,19×** (a localidade só importa quando não se está
  pagando CRC de qualquer jeito). Implementar exige gravar o `.reg` antes de
  indexar, e aí uma falha no meio deixa linha sem chave, sem como desfazer.
  Está registrado com o preço para a decisão ser tomada com ele na mão.

- **`bancada/carga/medir.py`**, para a carga pela rede parar de ser um número
  medido à mão. As duas metades fazem o mesmo trabalho e a contagem é conferida
  no fim — a armadilha que esta bancada já caiu duas vezes.

- **`--example ordem-da-chave`**, que mede quanto a ordem das chaves custa. Foi
  ele que reprovou a hipótese do pedido 113 antes de ela virar código.

### Mudado

- **O erro do protocolo chega inteiro à tela.**'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")

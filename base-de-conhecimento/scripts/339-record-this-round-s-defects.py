# Record this round's defects
# 28/08 11:49

import pathlib
p = pathlib.Path('docs/PENDENCIAS.md')
s = p.read_text()
v = '''- **A árvore roubava a tela de quem pintasse depois dela.**'''
n = '''### O que a rodada da gestão do banco achou

Dois defeitos, e o segundo é uma armadilha que qualquer tela nova podia repetir.

- **Um `onclick` no `#painel` vazava para a tela seguinte.** A gestão do banco
  pendurou o clique no próprio painel; o `folha()` troca o *conteúdo* do painel,
  não o *elemento*, então o tratador sobreviveu à troca de tela e disparava na
  próxima — clicar em «Configurações e diretivas» abria SysColumns. Corrigido
  em dois lugares: o tratador foi para o container das operações, e o `folha()`
  passou a limpar o `onclick` do painel por garantia.

- **O botão primário ocupava a linha inteira.** O `.botao` nasceu com
  `width:100%` para o cartão de entrada, onde é o único da linha. Numa barra de
  ações ele empurrava os outros para baixo. Agora `.acoes .botao` cabe no
  próprio texto.

- **O volume 1 nascia sem período.** Na partição por calendário o volume 1 é
  criado antes da primeira linha, então não havia período para gravar — e
  reabrir a tabela recusava com «não tem fronteira gravada». Agora ele nasce com
  uma sentinela e a primeira linha **adota** o volume em vez de cortar um novo,
  senão a tabela nasceria com um arquivo vazio.

- **A tela de partições calculava por divisão.** Ela dividia `slots` por
  `registros_por_arquivo` — a conta certa para a partição por faixa, e errada
  para a por período, onde o corte depende do calendário. Quatro meses apareciam
  como um volume só. Agora ela lê as fronteiras que o `esquema` devolve.

- **A árvore roubava a tela de quem pintasse depois dela.**'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')

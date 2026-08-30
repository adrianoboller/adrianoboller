# Add the bulk-load subsection
# 28/08 20:45

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
antigo = """  <h3>O oitavo arquivo: o espelho <code>.bkp</code></h3>"""
novo = """  <h3>Uma linha por vez, ou mil de uma vez</h3>

  <p>Gravar mil linhas com mil pedidos custava mil aberturas de tabela — sete
  arquivos cada —, mil travas e mil <em>fsync</em>. <code>inserir_lote</code> faz
  tudo uma vez só: <strong>2.715 → 25.985 linhas/s, 9,6×</strong>, medido com
  20.000 linhas pela rede contra o mesmo trabalho linha a linha.</p>

  <p>O ganho <strong>não é do disco</strong>. Cada linha custa exatamente o mesmo
  lá dentro — montar o payload, conferir a unicidade, gravar o slot, manter cada
  índice —, e a inserção já era o caminho mais caro do motor, com 65% do tempo na
  manutenção do <code>.ndx</code>. O ganho é de tudo que <em>acontecia por
  linha</em> e passou a acontecer uma vez.</p>

  <div class="nota">
    <span class="t">Não há transação, e o lote não muda isso</span>
    <p>Se a linha 700 de mil falhar, as 699 anteriores <strong>ficam
    gravadas</strong>. Não há como desfazer: o <code>.reg</code> não reaproveita
    slot, então «desfazer» seria deixar 699 buracos. Por isso o padrão é
    <strong>parar</strong> na primeira recusada — entre uma carga que para na
    linha 700 e uma que grava 999 com uma faltando no meio, a primeira é a que dá
    para consertar. Quem está importando dado sujo de propósito passa
    <code>parar_no_erro: false</code> e recebe a lista do que ficou de fora, com o
    <em>número da linha</em> no arquivo dele.</p>
  </div>

  <p>O mesmo pedido aceita texto colado em <strong>JSON, CSV, TXT, XML ou
  HTML</strong>, e adivinha o formato pelo conteúdo. A primeira linha manda: as
  colunas casam pelo <strong>nome</strong>, não pela posição — coluna que a tabela
  não tem é recusada com o nome dela, coluna que falta fica nula. E o número vem
  no formato daqui: <code>1.500,50</code> vira 1500,50 e <code>1,500.50</code>
  também, porque o <em>último</em> separador é o decimal. <code>1.500</code> é
  ambíguo — mil e quinhentos ou um e meio? — e fica como está, em vez de o motor
  escolher por conta própria.</p>

  <p><code>importar_conferir</code> lê e devolve o que entendeu — quantas linhas,
  quais colunas, uma amostra — <strong>sem gravar nada</strong>. É o que a tela de
  Importar usa: o botão de gravar só acende depois que a conferência passa.</p>

  <h3>O oitavo arquivo: o espelho <code>.bkp</code></h3>"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")

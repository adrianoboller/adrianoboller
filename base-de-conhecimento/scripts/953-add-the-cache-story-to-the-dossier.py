# Add the cache story to the dossier
# 29/08 00:44

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
alvo = '''  <h3>A inserção é o buraco, e o diagnóstico é incômodo</h3>

<!-- bancada:diagnostico:inicio -->'''
novo = '''  <h3>O CRC de página inteira, e o cache que o tirou do caminho</h3>

  <p>A rodada anterior parou num lugar desconfortável: <strong>83,5% do tempo de
  uma inserção estava no <code>.ndx</code></strong>, e a conta do CRC
  <em>não fechava</em> — o medidor estimava, por um <code>strace</code> de outro dia,
  ~20 toques de página por linha, e a 2,34&#8201;µs de CRC por página de 4&#8201;KiB
  isso daria ~47&#8201;µs, mais do que os 44,4&#8201;µs medidos no total. Ficou
  registrado como <em>pista aberta</em>, e não como conclusão.</p>

  <p>A conta não fechava porque o número era <strong>citado, e não medido</strong>.
  Hoje o medidor conta os toques por dentro:</p>

  <div class="rolo">
    <table>
      <thead><tr><th>Por linha inserida, com dois índices</th><th class="num">páginas</th></tr></thead>
      <tbody>
        <tr><td>servidas pelo cache</td><td class="num">8,80</td></tr>
        <tr><td>lidas do arquivo</td><td class="num">0,00</td></tr>
        <tr><td>gravadas</td><td class="num">2,06</td></tr>
      </tbody>
    </table>
  </div>

  <p>São <strong>10,86</strong>, e não 20. Antes do cache, as 10,86 passavam
  <em>todas</em> pelo CRC: 25,4&#8201;µs de 44,4 — <strong>57% do tempo de uma
  inserção era CRC-32 de página</strong>. E a raiz da árvore é a mesma página em
  todas as inserções da carga.</p>

  <p>Um cache de páginas <strong>de leitura</strong> no <code>.ndx</code>, com
  despejo por segunda chance, levou a inserção de <strong>44,4 para 18,5&#8201;µs
  por linha (2,40&#215;)</strong> sem mudar formato, sem mudar garantia e sem tocar
  na B+tree. A linha que mais mudou diz o que aconteceu: <em>conferir a chave
  única</em> caiu de 20,5% para 2,3% do tempo — é uma descida na árvore que não
  escreve nada, exatamente o trabalho que o cache serve de graça.</p>

  <div class="nota">
    <span class="t">O cache é de leitura, e isso é escolha</span>
    <p>Toda gravação atravessa para o arquivo na hora. Segurar página suja em RAM
    daria mais e trocaria uma garantia por desempenho <strong>sem avisar</strong>:
    hoje uma queda do <em>processo</em> não atrasa o <code>.ndx</code> em relação ao
    <code>.reg</code>, porque o <code>write</code> já entregou a página ao núcleo. Só
    uma queda da <em>máquina</em> faz isso.</p>
    <p>O despejo é por <strong>segunda chance</strong>, e não fila simples: a raiz,
    a página mais visitada de todas, sairia junto com as outras assim que o teto
    enchesse. O teto — 2.048 páginas, 8&#8201;MiB por tabela aberta — saiu de uma
    varredura de quatro tamanhos, e não do chute. É o campo
    <code>recursos.cache_paginas</code> do <code>config.json</code>, que existia
    desde a 0.13.0 <strong>sem nada por trás</strong>.</p>
  </div>

  <div class="nota">
    <span class="t">O pedido pedia outra coisa, e a medição mandou</span>
    <p>O item da lista dizia «ordene as chaves do lote antes de inserir no
    <code>.ndx</code>, para chaves vizinhas caírem na mesma folha». O alvo estava
    certo; o mecanismo, não. Medindo antes de escrever código: <strong>a desordem
    das chaves custava 1,06&#215;</strong>. Ordenar teria comprado quase nada — o
    custo não era de localidade, era de reler e recalcular CRC da mesma página.</p>
    <p>Depois do cache a desordem passou a custar <strong>1,19&#215;</strong>: a
    localidade só importa quando não se está pagando CRC de qualquer jeito.
    Ordenar continua não feito, agora com o preço na mesa — implementá-lo exige
    gravar o <code>.reg</code> antes de indexar, e aí uma falha no meio deixa linha
    sem chave, sem como desfazer.</p>
  </div>

  <h3>A inserção ainda é onde o MySQL(R) ganha — e 2,9&#215; já voltaram</h3>

<!-- bancada:diagnostico:inicio -->'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")

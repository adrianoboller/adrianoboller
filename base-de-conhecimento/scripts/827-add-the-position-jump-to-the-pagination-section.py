# Add the position jump to the pagination section
# 28/08 20:45

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()

antigo = """  <p class="leg">O que ainda não existe: <strong>salto para «a página 500»</strong>.
  O cursor sabe ir e voltar uma página; ir direto a um ponto exigiria contar a
  tabela — que é justamente o que foi removido. Quem precisa de um ponto certo
  usa o <code>rownum</code> com a bissecção. E por <em>índice</em> o cursor não
  vale: ali os rowids vêm na ordem da chave, e «depois do rowid X» não quer
  dizer nada — a resposta declara que paginou por posição.</p>
</section>"""
novo = """  <h3>E o salto para «a página 500», que faltava</h3>

  <p>O cursor sabe ir e voltar uma página. Ir <em>direto</em> à milésima parecia
  exigir contar a tabela — que era justamente o que tinha sido removido. Não
  exige, e a saída estava na coluna acima.</p>

  <p>Se ninguém apagou de vez e ninguém marcou como excluída, a <strong>posição
  de uma linha na lista é o <code>rownum</code> dela menos um</strong>. As duas
  condições se conferem em tempo constante, no cabeçalho: <code>proximo_rownum −
  1 == registros</code> diz que nada saiu de vez, e o contador
  <code>marcadas</code> diz que nada está escondido. Quando valem, o começo da
  página sai de uma bissecção.</p>

  <div class="rolo">
    <table>
      <thead><tr><th class="num"><code>pular</code></th><th class="num">bissecção</th><th class="num">passo a passo</th></tr></thead>
      <tbody>
        <tr><td class="num">200</td><td class="num">7 ms</td><td class="num">6 ms</td></tr>
        <tr><td class="num">20.000</td><td class="num">7 ms</td><td class="num">18 ms</td></tr>
        <tr><td class="num">100.000</td><td class="num">6 ms</td><td class="num">72 ms</td></tr>
        <tr><td class="num">199.800</td><td class="num">6 ms</td><td class="num"><b>131 ms</b></td></tr>
      </tbody>
    </table>
  </div>

  <p>Tabela de 200.000 linhas, pelo protocolo, pedindo 200 linhas. A bissecção é
  <strong>plana</strong> — e os 6 ms dela são decodificar e serializar as 200
  linhas, não achar o começo. Dentro do motor, sem a rede e sem a serialização:
  <strong>164 µs contra 246 ms</strong> no meio de uma tabela de 800.000. Na
  tela, com o desenho junto, o salto para a página 500 levou <strong>116
  ms</strong>.</p>

  <p>Os dois caminhos devolvem a <strong>mesma página</strong> — o exemplo
  <code>custo-da-pagina</code> afirma isso e falha se deixar de ser verdade. O
  que muda é o preço, e a resposta declara qual pagou em <code>salto</code>:
  <code>"bisseccao"</code> ou <code>"passo"</code>.</p>

  <p>E o contador de marcadas devolveu de graça o número que tinha sumido:
  <code>visiveis = registros − marcadas</code>, os dois do cabeçalho. «Página 3
  de 40» voltou para a grade sem custar varredura, e com ele a caixa <em>ir para
  a página</em>.</p>

  <div class="nota">
    <span class="t">Duas coisas que o salto não faz</span>
    <p><strong>Com um buraco, ele volta a andar.</strong> Uma única linha
    excluída — de vez ou marcada — derruba a igualdade na tabela inteira, e o
    <code>pular</code> volta aos 131 ms. É correto: a posição realmente mudou.
    Mas é uma degradação <em>em degrau</em>, e não gradual — quem paginava a 6 ms
    passa a 131 com uma exclusão. Um índice de posição resolveria, ao preço de
    mantê-lo.</p>
    <p><strong>Na partição alfanumérica ele nunca vale</strong>, e a razão é
    bonita: ali o <code>rownum</code> não cresce com o <code>rowid</code>. A Silva
    digitada primeiro mora no <code>_S</code>, com rowid alto; a Alves digitada
    depois mora no <code>_A</code>, com rowid 1 — número de ordem 1 num rowid
    maior que o do número 2. Bissetar uma sequência que não está ordenada
    devolveria a linha errada <em>em silêncio</em>, que é pior que devolver
    devagar. Ali o motor varre, e há teste que falha se os rowids um dia saírem
    crescentes — para não continuar provando outra coisa.</p>
    <p>Por <em>índice</em> o cursor também não vale: ali os rowids vêm na ordem da
    chave, e «depois do rowid X» não quer dizer nada — a resposta declara que
    paginou por posição.</p>
  </div>
</section>"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")

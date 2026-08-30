# Add table rights to the dossier
# 29/08 00:39

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
alvo = '''  <div class="nota">
    <span class="t">Nega por omissão, em toda parte</span>
    <p>Atividade que não aparece vale <code>false</code>. A base listada manda — o
    curinga <code>"*"</code> não completa o que faltou nela. Base ausente e sem curinga
    nega tudo. E <strong>operação desconhecida exige <code>administrar</code></strong>:
    uma op nova que alguém esqueça de mapear é negada, não liberada.</p>
  </div>'''
novo = '''  <div class="nota">
    <span class="t">Nega por omissão, em toda parte</span>
    <p>Atividade que não aparece vale <code>false</code>. A base listada manda — o
    curinga <code>"*"</code> não completa o que faltou nela. Base ausente e sem curinga
    nega tudo. E <strong>operação desconhecida exige <code>administrar</code></strong>:
    uma op nova que alguém esqueça de mapear é negada, não liberada.</p>
  </div>

  <h3>E o direito desce até a tabela</h3>

  <p>A folha de pagamento e a tabela de clientes moram no mesmo banco porque o negócio
  é um só, e o direito de ler as duas não é o mesmo. Dentro do objeto da base,
  <code>"tabelas"</code> escreve a regra de cada uma:</p>

  <pre class="codigo"><code>"bases": {
  "Z": {
    "ler": true, "inserir": true, "alterar": true,
    "tabelas": {
      "folha":    { },
      "clientes": { "ler": true, "inserir": true, "alterar": true }
    }
  }
}</code></pre>

  <p>A regra da tabela <strong>substitui</strong> a da base ali — não soma nem corta,
  do mesmo jeito que a base já fazia com o <code>"*"</code>. Substituir, e não
  interceder, é o que permite as duas coisas que a prática pede: <strong>tirar</strong>
  <code>folha</code> de quem lê o banco inteiro, e <strong>dar</strong>
  <code>clientes</code> a quem não lê o banco nenhum. Uma regra de interseção resolveria
  só a primeira.</p>

  <div class="nota">
    <span class="t">O portão é um só, e o campo que ele lê é onde mora o furo</span>
    <p>A conferência de tabela entrou no <em>mesmo</em> portão, que lê o campo
    <code>"tabela"</code> do pedido — espalhada por quarenta operações, a que alguém
    esquecesse viraria a porta dos fundos, e ninguém acharia isso por leitura.</p>
    <p>Duas operações não têm esse campo: <strong><code>juntar</code></strong>, cujas
    tabelas moram em <code>a.tabela</code> e <code>b.tabela</code>, e
    <strong><code>unir</code></strong>, cuja lista está em <code>"tabelas"</code>. Sem
    conferência própria, bastaria pedir a tabela negada como o lado B de uma junção. As
    duas conferem cada tabela do pedido, e há um teste com esse nome para cada uma.</p>
  </div>

  <p>A árvore e o catálogo — <code>tabelas</code>, <code>sistabelas</code>,
  <code>siscolunas</code> — passaram a listar <strong>só o que dá para abrir</strong>: o
  nome de uma tabela já conta parte da história, e descobrir a recusa só ao clicar é
  pior do que não ver. O que ainda não desce é o direito por <strong>coluna</strong>.</p>'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")

# Shorten the figure text and re-render
# 27/08 22:46

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
velho = '''          <text x="16" y="200" font-size="11.5" opacity=".75">
            <tspan font-weight="600">O portão fica antes da conta, não depois.</tspan> Um número agregado vaza tanto quanto uma linha: saber que existe uma base de dez milhões
          </text>
          <text x="16" y="218" font-size="11.5" opacity=".75">de registros já é saber alguma coisa. Por isso a base sem permissão de leitura não entra na soma — e não aparece diminuída, aparece ausente.</text>
          <text x="16" y="244" font-size="11" opacity=".6">Uma chamada em vez de dez: a ida e volta custa mais do que a conta. E uma passada por fonte, não uma por pergunta — ler o log cinco vezes</text>
          <text x="16" y="262" font-size="11" opacity=".6">para responder cinco perguntas seria o painel ficando lento junto com o log.</text>'''
novo = '''          <text x="16" y="200" font-size="11.5" opacity=".75">
            <tspan font-weight="600">O portão fica antes da conta, não depois.</tspan> Número agregado vaza tanto quanto linha.
          </text>
          <text x="16" y="218" font-size="11.5" opacity=".75">Saber que existe uma base de dez milhões de registros já é saber alguma coisa: ela não entra na soma.</text>
          <text x="16" y="244" font-size="11" opacity=".6">Uma chamada em vez de dez: a ida e volta custa mais do que a conta.</text>
          <text x="16" y="262" font-size="11" opacity=".6">E uma passada por fonte, não uma por pergunta — ler o log cinco vezes seria o painel lento junto com ele.</text>'''
assert s.count(velho)==1
open(p,'w').write(s.replace(velho,novo))

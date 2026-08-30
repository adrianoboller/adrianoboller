# Update dossier numbers
# 27/08 20:00

s=open('docs/dossie/dossie-phxsql.html').read()

# nova linha na tabela de estado, logo depois da politica/blacklist
s = s.replace('''<tr><td>Política, blacklist e gancho de firewall</td><td><span class="pino ok">pronto</span></td><td class="num">14</td></tr>''',
'''<tr><td>Política, blacklist e gancho de firewall</td><td><span class="pino ok">pronto</span></td><td class="num">12</td></tr>
        <tr><td>Centro de Controle — interface web embutida no <code>phxsqld</code></td><td><span class="pino ok">pronto</span></td><td class="num">13</td></tr>''')

# numeros medidos no rodape
s = s.replace('''<p>PhxSql 0.2.0 · 13.334 linhas de Rust em 4 crates · 166 testes · nenhuma dependência
  externa.''','''<p>PhxSql 0.1.0 · 14.171 linhas de Rust em 4 crates, mais 34 KB de interface ·
  214 testes · nenhuma dependência externa.''')

# config.json: 23 -> 13 na linha do servidor? o numero antigo cobria config+servidor.
s = s.replace('''<tr><td>Servidor TCP na porta 5000 · <code>config.json</code></td><td><span class="pino ok">pronto</span></td><td class="num">23</td></tr>''',
'''<tr><td>Servidor TCP na porta 5000 · <code>config.json</code></td><td><span class="pino ok">pronto</span></td><td class="num">13</td></tr>''')
open('docs/dossie/dossie-phxsql.html','w').write(s)

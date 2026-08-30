# Complete the screen inventory table
# 28/08 13:11

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()

v = '''        <tr><td>Bloqueios</td><td>quem está barrado, por quê e até quando</td><td><code>bloqueios</code></td></tr>
        <tr><td>Tabelas</td><td>as tabelas do banco e as oito operações sobre cada uma</td><td><code>tabelas</code>, <code>esquema</code></td></tr>
        <tr><td>Nova tabela</td><td>colunas, índices, schema e paginação, montados linha a linha</td><td><code>criar_tabela</code></td></tr>
        <tr><td>Partições</td><td>em que volume cada faixa de rowid cai, e o nome do arquivo</td><td><code>esquema</code> + conta</td></tr>
      </tbody>
    </table>
  </div>'''

n = '''        <tr><td>Bloqueios</td><td>quem está barrado, por quê e até quando</td><td><code>bloqueios</code></td></tr>
      </tbody>
    </table>
  </div>

  <p>E, sobre a tabela escolhida ou sobre o banco inteiro:</p>

  <div class="rolo">
    <table>
      <thead><tr><th>Onde</th><th>O que traz</th><th>De onde vem</th></tr></thead>
      <tbody>
        <tr><td>Tabelas</td><td>as tabelas do banco, e onze operações sobre a escolhida</td><td><code>tabelas</code>, <code>esquema</code></td></tr>
        <tr><td>Nova tabela</td><td>campos, índices, schema e partição, montados linha a linha</td><td><code>criar_tabela</code></td></tr>
        <tr><td>Partições</td><td>em que volume cada faixa de rowid cai, e o nome do arquivo</td><td><code>esquema</code> — conta, ou as fronteiras</td></tr>
        <tr><td>Configurações da tabela</td><td>a geometria decidida na criação, chaves, volumes, e o que ela herda</td><td><code>esquema</code>, <code>config</code>, <code>memoria</code></td></tr>
        <tr><td>Copiar e colar</td><td>leva uma tabela para outro banco ou schema</td><td><code>copiar_tabela</code></td></tr>
        <tr><td>Gerir banco</td><td>quinze itens sobre o database — onze funcionam, quatro dizem o que falta</td><td><code>sistabelas</code></td></tr>
        <tr><td>SysTables</td><td>o catálogo de tabelas em forma de dado</td><td><code>sistabelas</code></td></tr>
        <tr><td>SysColumns</td><td>o dicionário de dados: caption, máscara, tipo, papel na chave</td><td><code>siscolunas</code></td></tr>
        <tr><td>Configurações do servidor</td><td>cada campo do <code>config.json</code> e para que serve</td><td><code>config</code>, <code>ping</code></td></tr>
        <tr><td>Configurações do banco</td><td>onde ele mora, o que tem dentro, o que herda</td><td><code>config</code>, <code>tabelas</code>, <code>sistabelas</code></td></tr>
        <tr><td>Configurações dos usuários</td><td>o cadastro e o poder de cada um sobre cada base</td><td><code>usuarios</code></td></tr>
        <tr><td>Diretivas do banco</td><td>os seis portões na ordem em que fecham, e quem alcança</td><td><code>config</code>, <code>usuarios</code>, <code>quem_sou</code></td></tr>
        <tr><td>Backup e restauração</td><td>copiar com manifesto, conferir uma cópia</td><td><code>backup</code>, <code>conferir_backup</code></td></tr>
        <tr><td>Transações</td><td>o que existe hoje e o que falta — <em>mostra a ausência</em></td><td><code>config</code></td></tr>
        <tr><td>Editor de menu</td><td>troca o nome exibido de qualquer item</td><td>o navegador de quem mexeu</td></tr>
        <tr><td>Painel</td><td>o servidor inteiro: sete gráficos e oito números</td><td><code>painel</code></td></tr>
        <tr><td>Consulta</td><td><code>SelectMemory</code> sobre uma tabela residente</td><td><code>memoria</code>, <code>SelectMemory</code></td></tr>
      </tbody>
    </table>
  </div>

  <p><strong>33 das 36 operações do protocolo têm tela.</strong> Fora:
  <code>buscar</code>, <code>desbloquear</code> e <code>criar_schema</code> — este
  último acontece sozinho quando a tela cria uma tabela dentro de um schema que
  ainda não existe.</p>'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('tabela de telas completa')

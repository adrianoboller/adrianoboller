# Complete the state table
# 28/08 13:12

import pathlib
p = pathlib.Path('docs/dossie/dossie-phxsql.html')
s = p.read_text()
v = '''        <tr><td>Gerir banco · configurações, diretivas, conexões, backup</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>'''
n = '''        <tr><td>Gerir banco · quinze itens, onze vivos e quatro dizendo o que falta</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>
        <tr><td>Configurações do servidor, do banco e dos usuários · <em>leem, não gravam</em></td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>
        <tr><td>Diretivas de acesso ao banco · os seis portões na ordem em que fecham</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>
        <tr><td>Configurações e diretivas da tabela · geometria, chaves e volumes</td><td><span class="pino ok">pronto</span></td><td class="num">—</td></tr>'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''        <tr><td>Parar e subir a porta de dados pela interface</td><td><span class="pino nao">a fazer</span></td><td class="num">—</td></tr>'''
n = '''        <tr><td>Parar e subir a porta de dados pela interface</td><td><span class="pino nao">a fazer</span></td><td class="num">—</td></tr>
        <tr><td>Modo exclusivo · reservar uma tabela por um período</td><td><span class="pino nao">a fazer</span></td><td class="num">—</td></tr>
        <tr><td>Restaurar um backup por cima do que está lá</td><td><span class="pino nao">a fazer</span></td><td class="num">—</td></tr>
        <tr><td>Editar <code>config.json</code> e usuários pela web</td><td><span class="pino nao">a fazer</span></td><td class="num">—</td></tr>'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('quadro de estado completo')

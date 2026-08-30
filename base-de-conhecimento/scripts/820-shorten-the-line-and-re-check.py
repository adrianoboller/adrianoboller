# Shorten the line and re-check
# 28/08 20:40

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
antigo = '''          <text x="16" y="530" font-size="10.5" opacity=".55">Alterar segue o mesmo caminho: herda o rownum e a marca, remove a chave antiga só quando ela mudou, e libera os blocos antigos no fim.</text>
          <text x="16" y="548" font-size="10.5" opacity=".55">Excluir tem dois caminhos, e o padrão é o reversível — a figura seguinte mostra os dois.</text>'''
novo = '''          <text x="16" y="530" font-size="10.5" opacity=".55">Alterar segue o mesmo caminho: herda o rownum e a marca, e só troca a chave do índice quando ela mudou.</text>
          <text x="16" y="548" font-size="10.5" opacity=".55">Excluir tem dois caminhos, e o padrão é o reversível — a figura seguinte mostra os dois.</text>'''
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)

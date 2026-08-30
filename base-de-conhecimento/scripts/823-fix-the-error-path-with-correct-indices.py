# Fix the error path with correct indices
# 28/08 20:42

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
linhas = p.read_text().split("\n")
i0, i1 = 1191, 1199   # 0-based: linha 1192 ate 1200
assert 'M623 158' in linhas[i0], linhas[i0]
assert 'M580 240 L560 240' in linhas[i1], linhas[i1]
novo = '''          <path d="M560 186 L560 288" fill="none" stroke="var(--log)" stroke-width="1.4" stroke-dasharray="5 3" marker-end="url(#setaErro)"/>
          <text x="572" y="224" fill="var(--log)" font-size="10">se um índice</text>
          <text x="572" y="237" fill="var(--log)" font-size="10">falhar</text>

          <rect x="512" y="292" width="196" height="54" rx="4" fill="none" stroke="var(--log)" stroke-width="1.5" stroke-dasharray="5 3"/>
          <text x="610" y="312" text-anchor="middle" fill="var(--log)" font-size="11">desfaz tudo</text>
          <text x="610" y="327" text-anchor="middle" font-size="9" opacity=".62">tira as chaves já postas,</text>
          <text x="610" y="340" text-anchor="middle" font-size="9" opacity=".62">exclui o slot, libera os blocos</text>'''.split("\n")
linhas[i0:i1+1] = novo
p.write_text("\n".join(linhas))
print("ok")

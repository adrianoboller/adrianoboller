# Simplify the error path
# 28/08 20:41

import pathlib
p = pathlib.Path("docs/dossie/dossie-phxsql-0.15.html")
s = p.read_text()
antigo = '''          <path d="M623 158 L660 158 L660 240 L580 240" fill="none" stroke="var(--log)" stroke-width="1.4" stroke-dasharray="5 3" marker-end="url(#setaErro)"/>
          <text x="672" y="204" fill="var(--log)" font-size="10">se um índice</text>
          <text x="672" y="217" fill="var(--log)" font-size="10">falhar</text>

          <rect x="516" y="284" width="180" height="52" rx="4" fill="none" stroke="var(--log)" stroke-width="1.5" stroke-dasharray="5 3"/>
          <text x="606" y="304" text-anchor="middle" fill="var(--log)" font-size="11">desfaz tudo</text>
          <text x="606" y="319" text-anchor="middle" fill="var(--log)" font-size="9" opacity=".62">tira as chaves já postas,</text>
          <text x="606" y="331" text-anchor="middle" fill="var(--log)" font-size="9" opacity=".62">exclui o slot, libera os blocos</text>
          <path d="M580 240 L560 240 L560 284" fill="none" stroke="var(--log)" stroke-width="1.4" stroke-dasharray="5 3"/>'''
novo = '''          <path d="M560 186 L560 284" fill="none" stroke="var(--log)" stroke-width="1.4" stroke-dasharray="5 3" marker-end="url(#setaErro)"/>
          <text x="572" y="222" fill="var(--log)" font-size="10">se um índice</text>
          <text x="572" y="235" fill="var(--log)" font-size="10">falhar</text>

          <rect x="512" y="288" width="196" height="54" rx="4" fill="none" stroke="var(--log)" stroke-width="1.5" stroke-dasharray="5 3"/>
          <text x="610" y="308" text-anchor="middle" fill="var(--log)" font-size="11">desfaz tudo</text>
          <text x="610" y="323" text-anchor="middle" font-size="9" opacity=".62">tira as chaves já postas,</text>
          <text x="610" y="336" text-anchor="middle" font-size="9" opacity=".62">exclui o slot, libera os blocos</text>'''
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)

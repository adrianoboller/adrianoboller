# Regenerate and document the badge
# 28/08 20:48

import pathlib
p = pathlib.Path("docs/dossie/numeros-do-projeto.py")
s = p.read_text()
s = s.replace('''    html = trocar(html, ABRE, FECHA, painel, "painel da capa")
    html = trocar(html, ABRE_RODAPE, FECHA_RODAPE, rodape, "rodapé")''',
'''    selo = f'\\n  <div class="selo">Dossiê técnico · versão {n["versao"]}</div>\\n  '

    html = trocar(html, ABRE, FECHA, painel, "painel da capa")
    html = trocar(html, ABRE_RODAPE, FECHA_RODAPE, rodape, "rodapé")
    html = trocar(html, ABRE_SELO, FECHA_SELO, selo, "selo da capa")''')
p.write_text(s)

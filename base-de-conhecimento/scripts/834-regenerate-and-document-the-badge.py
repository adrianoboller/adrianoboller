# Regenerate and document the badge
# 28/08 20:48

import pathlib
p = pathlib.Path("docs/dossie/LEIA-ME.md")
s = p.read_text()
s = s.replace("""que mede tudo e reescreve os dois blocos entre as marcas `<!-- projeto:… -->`
e `<!-- rodape:… -->`.""",
"""que mede tudo e reescreve os três blocos entre as marcas `<!-- projeto:… -->`,
`<!-- rodape:… -->` e `<!-- selo:… -->` — o selo entrou porque a versão na capa
ficou **quatro lançamentos** dizendo 0.11.0.""")
p.write_text(s)

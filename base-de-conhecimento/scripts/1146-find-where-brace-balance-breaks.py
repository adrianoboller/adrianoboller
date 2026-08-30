# Find where brace balance breaks
# 29/08 17:23

import pathlib, re
linhas = pathlib.Path("crates/phxsql-server/src/servidor.rs").read_text().splitlines()
nivel = 0; dentro_str = False
suspeitas = []
for i, l in enumerate(linhas, 1):
    # ignora linhas de comentario simples para reduzir ruido
    codigo = re.sub(r'//.*$', '', l)
    codigo = re.sub(r'"(\\.|[^"\\])*"', '""', codigo)   # tira literais
    antes = nivel
    nivel += codigo.count("{") - codigo.count("}")
    # dentro de `impl Servidor` (nivel 1) toda `fn` de metodo deve comecar em 1
    if re.match(r"^    (pub )?fn ", l) and antes != 1:
        suspeitas.append((i, antes, l.strip()[:70]))
print("nivel final:", nivel)
for s in suspeitas[:6]: print("linha %d nivel %d :: %s" % s)

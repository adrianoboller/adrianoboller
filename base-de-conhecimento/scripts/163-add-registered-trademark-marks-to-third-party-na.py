# Add registered trademark marks to third-party names
# 27/08 20:57

import re, pathlib

MARCAS = ["SQLite", "MySQL", "HFSQL", "Redis", "TopSpeed", "Clarion", "Oracle"]

# Onde NAO mexer: identificador, caminho, URL, nome de crate, nome de arquivo.
# "rusqlite" e nome de pacote, nao marca -- fica como esta.
def marcar(texto, ext):
    for m in MARCAS:
        # ja marcado, dentro de URL/caminho, ou colado em outra palavra: pula
        padrao = re.compile(
            r'(?<![\w/.-])' + m + r'(?!\(R\))(?![\w/.-])'
        )
        def troca(mo):
            i = mo.start()
            antes = texto[max(0, i-80):i]
            # dentro de link, de caminho ou de bloco de codigo inline curto
            if 'http' in antes[-60:] or antes.rstrip().endswith('/'):
                return m
            return m + '(R)'
        texto = padrao.sub(troca, texto)
    return texto

alvos = []
for p in ['README.md', 'MANUAL.txt']:
    alvos.append(pathlib.Path(p))
alvos += list(pathlib.Path('docs').rglob('*.md'))
alvos += list(pathlib.Path('marca').rglob('*.md'))
alvos.append(pathlib.Path('docs/dossie/dossie-phxsql.html'))
alvos += list(pathlib.Path('crates').rglob('*.rs'))

mudados = []
for p in alvos:
    if not p.is_file():
        continue
    t = p.read_text()
    n = marcar(t, p.suffix)
    if n != t:
        p.write_text(n)
        mudados.append(str(p))
print(f"{len(mudados)} arquivo(s) marcados:")
for m in mudados: print("  ", m)

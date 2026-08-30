# Fix the verbatim quote and mark remaining names
# 27/08 20:58

import pathlib, re
p = pathlib.Path('docs/PLANO.md')
s = p.read_text()
# Citacao literal da descricao do rusqlite: nao se altera o texto de outro.
s = s.replace('| "Ergonomic wrapper for SQLite(R)" |', '| "Ergonomic wrapper for SQLite" (citacao literal do repositorio) |')
# As outras marcas da mesma linha do ODBC
for m in ["DB2", "AS400", "Informix", "Sybase", "Teradata", "Access", "Firebird"]:
    s = re.sub(r'(?<![\w/.-])' + m + r'(?!\(R\))(?![\w/.-])', m + '(R)', s)
p.write_text(s)
print([l for l in s.split('\n') if 'ODBC do PhxSql' in l or 'Ergonomic' in l])

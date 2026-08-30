# Fix the server call site
# 28/08 11:17

import pathlib
p = pathlib.Path('crates/phxsql-server/src/valores.rs')
s = p.read_text()
v = '''        esquema.com_paginacao(
            Paginacao::nova(por_arquivo as u64, 1)?
                .com_digitos(digitos)?
                .com_max_arquivos(max)?,
        )'''
n = '''        esquema.com_paginacao(
            Paginacao::nova(por_arquivo as u64, 1)?
                .com_digitos(digitos)?
                .com_max_arquivos(max)?
                .com_modo(modo)?,
        )?'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))

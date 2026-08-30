# Wire permissions and the read-only guard
# 28/08 11:25

import pathlib
p = pathlib.Path('crates/phxsql-server/src/usuarios.rs')
s = p.read_text()
v = '''            "bancos" | "tabelas" | "esquema" | "ler" | "varrer" | "buscar" => Atividade::Ler,'''
n = '''            "bancos" | "tabelas" | "esquema" | "ler" | "varrer" | "buscar" => Atividade::Ler,
            // O catalogo e leitura: quem pode ler a tabela pode saber que ela
            // existe e que colunas tem.
            "sistabelas" | "systables" | "siscolunas" | "syscolumns" => Atividade::Ler,'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))

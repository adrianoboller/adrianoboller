# Wire permissions and verify
# 28/08 13:28

import pathlib
p = pathlib.Path('crates/phxsql-server/src/usuarios.rs')
s = p.read_text()
v = '''            "sistabelas" | "systables" | "siscolunas" | "syscolumns" => Atividade::Ler,'''
n = '''            "sistabelas" | "systables" | "siscolunas" | "syscolumns" => Atividade::Ler,
            // O pivot resume o que a varredura leria: quem pode ler a tabela
            // pode ver o total dela.
            "pivotar" | "pivot" => Atividade::Ler,'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))

p = pathlib.Path('crates/phxsql-server/src/servidor.rs')
s = p.read_text()
v = '''            "ping", "config", "bancos", "tabelas", "esquema", "ler", "varrer", "buscar", "diario",
            "verificar",'''
n = '''            "ping", "config", "bancos", "tabelas", "esquema", "ler", "varrer", "buscar", "diario",
            "pivotar", "verificar",'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')

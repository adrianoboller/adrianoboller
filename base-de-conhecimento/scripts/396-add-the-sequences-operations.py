# Add the sequences operations
# 28/08 13:59

import pathlib
p = pathlib.Path('crates/phxsql-server/src/usuarios.rs'); s = p.read_text()
v = '''            "pivotar" | "pivot" => Atividade::Ler,'''
n = '''            "pivotar" | "pivot" => Atividade::Ler,
            "sequencias" | "sequences" => Atividade::Ler,
            // Mexer no contador pode fazer a proxima insercao repetir numero.
            "ajustar_sequencia" => Atividade::Administrar,'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))

p = pathlib.Path('crates/phxsql-server/src/servidor.rs'); s = p.read_text()
v = '''    "copiar_tabela",
];'''
n = '''    "copiar_tabela",
    "ajustar_sequencia",
];'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''            "copiar_tabela",
        ] {'''
n = '''            "copiar_tabela",
            "ajustar_sequencia",
        ] {'''
assert s.count(v) == 1
s = s.replace(v, n)
v = '''            "sistabelas",
            "siscolunas",
            "pivotar",
        ] {'''
n = '''            "sistabelas",
            "siscolunas",
            "pivotar",
            "sequencias",
        ] {'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('permissoes')

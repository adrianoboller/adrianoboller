# Add activity mapping
# 28/08 20:13

import pathlib
p = pathlib.Path("crates/phxsql-server/src/usuarios.rs")
s = p.read_text()
antigo = """            "diario" => Atividade::Diario,"""
novo = """            "diario" => Atividade::Diario,
            // O fluxo de replicacao E o diario, com a linha inteira dentro.
            // Quem pode ler o diario pode ler isto -- e quem nao pode ler o
            // diario nao pode receber a copia de cada linha por outra porta.
            "posicao" | "replicar" => Atividade::Diario,
            // Aplicar GRAVA na tabela, e grava por fora das conferencias
            // normais: rowid escolhido, payload cru. Nao e insercao comum, e
            // por isso pede o poder de administrar e nao o de inserir.
            "aplicar" => Atividade::Administrar,"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")

# Use the existing Replicar activity
# 28/08 20:15

import pathlib
p = pathlib.Path("crates/phxsql-server/src/usuarios.rs")
s = p.read_text()
antigo = """            "diario" => Atividade::Diario,
            // O fluxo de replicacao E o diario, com a linha inteira dentro.
            // Quem pode ler o diario pode ler isto -- e quem nao pode ler o
            // diario nao pode receber a copia de cada linha por outra porta.
            "posicao" | "replicar" => Atividade::Diario,
            // Aplicar GRAVA na tabela, e grava por fora das conferencias
            // normais: rowid escolhido, payload cru. Nao e insercao comum, e
            // por isso pede o poder de administrar e nao o de inserir.
            "aplicar" => Atividade::Administrar,
            "verificar" => Atividade::Verificar,"""
novo = """            "diario" => Atividade::Diario,
            // Aplicar GRAVA na tabela, e grava por fora das conferencias
            // normais: rowid escolhido do evento, payload cru vindo de fora.
            // Nao e insercao comum, e por isso pede o poder de administrar e
            // nao o de inserir.
            "aplicar" => Atividade::Administrar,
            "verificar" => Atividade::Verificar,"""
assert antigo in s
s = s.replace(antigo, novo)
antigo = """            "posicao" | "replicar" => Atividade::Replicar,"""
novo = """            // O fluxo de replicacao e o diario com a linha inteira dentro:
            // permissao propria, para poder dar a uma replica sem dar mais
            // nada -- e para nao sair de graca junto com `ler`.
            "posicao" | "replicar" => Atividade::Replicar,"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")

# Add the profiler operations
# 28/08 22:57

import pathlib
p = pathlib.Path("crates/phxsql-server/src/usuarios.rs")
s = p.read_text()
antigo = """            "acessos" | "ips" | "config" | "usuarios" | "bloqueios" | "desbloquear" => {
                Atividade::Administrar
            }"""
novo = """            "acessos" | "ips" | "config" | "usuarios" | "bloqueios" | "desbloquear" => {
                Atividade::Administrar
            }
            // O profiler mostra o TEXTO dos pedidos de todo mundo, com os
            // dados que estao sendo gravados dentro. Quem pode ler uma tabela
            // nao ganha por isso o direito de ver o que os outros escrevem
            // nela -- nem de mandar o servidor escrever um arquivo no disco.
            "profiler" | "profiler_ligar" | "profiler_desligar" | "profiler_limpar" => {
                Atividade::Administrar
            }"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)

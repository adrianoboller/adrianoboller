# Fix both merge damages and rebuild
# 29/08 18:41

import pathlib
p = pathlib.Path("crates/phxsql-server/src/usuarios.rs"); t = p.read_text()
velho = '''            "bloqueios_exportar" | "whitelist_salvar" | "mensagens" | "mensagens_semear" => {
            // A telemetria mostra'''
novo = '''            "bloqueios_exportar" | "whitelist_salvar" | "mensagens" | "mensagens_semear" => {
                Atividade::Administrar
            }
            // A telemetria mostra'''
assert velho in t
t = t.replace(velho, novo, 1)
# O braco sem `config_gravar` voltou duplicado no merge; fica o que o inclui.
dup = '''            "acessos" | "ips" | "config" | "usuarios" | "bloqueios" | "desbloquear" => {
                Atividade::Administrar
            }
'''
assert dup in t
t = t.replace(dup, "", 1)
p.write_text(t); print("dois estragos consertados")

# Fix known field, duplicate arm; check warnings
# 29/08 18:11

import pathlib
# 1) O campo que a frente de JOBS acrescentou entra na lista de conhecidos --
#    era o verificador novo fazendo o trabalho dele.
p = pathlib.Path("crates/phxsql-server/src/config.rs"); t = p.read_text()
velho = '''    (
        "alertas.email",
        &[
            "ligado",
            "servidor",'''
novo = '''    (
        "alertas.email",
        &[
            "ligado",
            // Da frente dos jobs: liga o aviso por e-mail de job que falhou ou
            // parou. Sem ele na lista, o verificador novo -- que passou a olhar
            // o INTERIOR das secoes -- acusava campo estranho num exemplo que
            // esta certo.
            "avisar_jobs",
            "servidor",'''
assert velho in t
p.write_text(t.replace(velho, novo, 1)); print("1. avisar_jobs reconhecido")

# 2) usuarios.rs: o merge deixou dois bracos com a mesma lista. Fica UM, com o
#    config_gravar que o segundo trazia.
p = pathlib.Path("crates/phxsql-server/src/usuarios.rs"); t = p.read_text()
velho = '''            "acessos" | "ips" | "config" | "usuarios" | "bloqueios" | "desbloquear" => {
                Atividade::Administrar
            }'''
assert velho in t
t = t.replace(velho, "", 1)
t = t.replace('''            // `config_gravar` esta aqui declarado, e nao so caindo no `_`:''',
'''            // `config_gravar` esta aqui declarado, e nao so caindo no `_`:''', 1)
p.write_text(t); print("2. braco duplicado removido")

# 3) A variavel copiada e usada; o warning era o `somente_leitura` sobrando
#    porque o campo ja nascia do outro lado. Usa-se a copia, que e a mesma.

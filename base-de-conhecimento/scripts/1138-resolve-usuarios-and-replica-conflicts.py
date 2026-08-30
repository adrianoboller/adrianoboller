# Resolve usuarios and replica conflicts
# 29/08 17:20

import re, pathlib

# usuarios.rs e replica.rs: aditivos -- ficam os dois lados.
p = pathlib.Path("crates/phxsql-server/src/usuarios.rs"); t = p.read_text()
t = re.sub(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", lambda m: m.group(1)+m.group(2), t, flags=re.S)
p.write_text(t); print("usuarios.rs: os dois lados")

# replica.rs: o nome ESCRITA_NA_REPLICA vinha de servidor remoto; agora ele e
# REDIRECIONA. Mantem os dois nomes na traducao -- um servidor de versao
# anterior ainda pode mandar o nome velho, e o cliente novo tem de entender.
p = pathlib.Path("crates/phxsql-server/src/replica.rs"); t = p.read_text()
t = t.replace('''<<<<<<< HEAD
                "REDIRECIONA" => PhxError::Redireciona(texto),
=======
                "ESCRITA_NA_REPLICA" => PhxError::EscritaNaReplica(texto),
                "SPARE_EM_ESPERA" => PhxError::SpareEmEspera(texto),
>>>>>>> worktree-agent-aeba5ba7fe4b19f92
''', '''                // Os dois nomes caem no MESMO erro: "escrita na replica" e
                // "redireciona" sempre foram o mesmo evento com nomes
                // diferentes, e um servidor de versao anterior ainda manda o
                // nome antigo.
                "REDIRECIONA" | "ESCRITA_NA_REPLICA" => PhxError::Redireciona(texto),
                "SPARE_EM_ESPERA" => PhxError::SpareEmEspera(texto),
''')
p.write_text(t); print("replica.rs: os dois nomes no mesmo erro")

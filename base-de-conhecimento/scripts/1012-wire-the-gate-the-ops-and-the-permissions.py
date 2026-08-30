# Wire the gate, the ops and the permissions
# 29/08 02:55

import pathlib
p = pathlib.Path("crates/phxsql-server/src/usuarios.rs")
s = p.read_text()
alvo = '''            "inserir" => Atividade::Inserir,'''
novo = '''            "inserir" => Atividade::Inserir,
            // Reservar a tabela para carga exige o poder de INSERIR nela, e
            // nao mais: quem pode gravar mil linhas pode pedir a tabela para
            // gravar mil linhas. Ja `cargas` -- a lista de quem reservou o que
            // -- mostra o movimento dos outros, e por isso pede administrar,
            // pela mesma razao que `sessoes` pede.
            "bulkinsert" | "carga" => Atividade::Inserir,
            "cargas" => Atividade::Administrar,'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)

# Wire the flag and inspect op_diario
# 28/08 20:12

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
antigo = """        // Quem alterar assina o evento no .log da tabela.
        t.definir_usuario(sessao.id());
        Ok(t)
    }"""
novo = """        // Quem alterar assina o evento no .log da tabela.
        t.definir_usuario(sessao.id());
        // A imagem da linha no diario e decisao do servidor, como o espelho:
        // um source grava, um servidor isolado nao paga por ela.
        t.ligar_imagem_no_diario(self.config.replicacao.imagem_da_linha);
        Ok(t)
    }"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")

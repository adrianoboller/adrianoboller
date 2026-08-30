# Fix the name clash, the lint and the test's socket close
# 29/08 02:58

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
# "carga" ja e apelido de inserir_lote -- nao roubar o nome
s = s.replace('''            "bulkinsert" | "carga" => self.op_bulkinsert(p, sessao),''',
              '''            "bulkinsert" => self.op_bulkinsert(p, sessao),''',1)
s = s.replace('''    "bulkinsert",
    "carga",''', '''    "bulkinsert",''',1)
# a linha em branco depois do doc comment
s = s.replace('''    // ------------------------------------------------------------- carga
    //
    // `BULKINSERT`: a tabela reservada para quem esta carregando, e so para
    // ele. Ver `crate::carga` para o desenho e para as duas redes de protecao
    // contra reserva orfa.

    /// Solta o que esta ligacao reservou''','''    // --- carga -----------------------------------------------------------
    // `BULKINSERT`: a tabela reservada para quem esta carregando, e so para
    // ele. Ver `crate::carga` para o desenho e para as duas redes de protecao
    // contra reserva orfa.
    /// Solta o que esta ligacao reservou''',1)
p.write_text(s)

p = pathlib.Path("crates/phxsql-server/src/usuarios.rs")
s = p.read_text()
s = s.replace('''            "bulkinsert" | "carga" => Atividade::Inserir,''',
              '''            "bulkinsert" => Atividade::Inserir,''',1)
p.write_text(s)

# Resolve gate 2b conflict preserving both
# 29/08 17:14

import pathlib
p = pathlib.Path("phxsql/crates/phxsql-server/src/servidor.rs")
t = p.read_text()
velho = """<<<<<<< HEAD
        // Portao 2b -- a escrita. Com cluster, quem decide e o papel VIVO: a
        // replica redireciona para o master (`REDIRECIONA host:porta`), e um
        // master sem maioria visivel recusa, para conter o split-brain. O
        // `somente_leitura` do config -- que toda replica de cluster liga --
        // deixa de valer no no PROMOVIDO, senao a promocao nao promoveria
        // nada. Sem o bloco `cluster`, a regra e a de sempre.
        if OPS_ESCRITA.contains(&op) {
            if let Some(estado) = &self.cluster {
                if let Some(recusa) = estado.recusa_de_escrita() {
                    return Err(recusa);
                }
            } else if self.config.somente_leitura {
                return Err(PhxError::Autorizacao(
                    "servidor em modo somente leitura".into(),
                ));
            }
=======
        // Portao 2b -- o servidor inteiro em somente leitura.
        if self.config.somente_leitura && OPS_ESCRITA.contains(&op) {
            return Err(PhxError::Autorizacao(self.msg("erro.somente_leitura", &[])));
>>>>>>> worktree-agent-af10b6f797860b6a7
        }"""
novo = """        // Portao 2b -- a escrita. Com cluster, quem decide e o papel VIVO: a
        // replica redireciona para o master (`REDIRECIONA host:porta`), e um
        // master sem maioria visivel recusa, para conter o split-brain. O
        // `somente_leitura` do config -- que toda replica de cluster liga --
        // deixa de valer no no PROMOVIDO, senao a promocao nao promoveria
        // nada. Sem o bloco `cluster`, a regra e a de sempre.
        //
        // A recusa do somente-leitura sai pela tabela de mensagens: e texto
        // que o usuario final le, entao acompanha o idioma configurado. A do
        // cluster ja vem pronta de `recusa_de_escrita`, que precisa do
        // `REDIRECIONA host:porta` no comeco para o cliente se reapontar.
        if OPS_ESCRITA.contains(&op) {
            if let Some(estado) = &self.cluster {
                if let Some(recusa) = estado.recusa_de_escrita() {
                    return Err(recusa);
                }
            } else if self.config.somente_leitura {
                return Err(PhxError::Autorizacao(self.msg("erro.somente_leitura", &[])));
            }
        }"""
assert velho in t, "trecho nao casou"
p.write_text(t.replace(velho, novo))
print("resolvido")

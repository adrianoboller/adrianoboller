# Combine cluster exemption with expanded role validation
# 29/08 17:20

import pathlib
p = pathlib.Path("crates/phxsql-server/src/config.rs"); t = p.read_text()
velho_ini = """<<<<<<< HEAD
        // Com cluster, a origem e o master CORRENTE, descoberto pelo pulso --
        // uma lista fixa de origens apontaria para o master de ontem.
        if self.cluster.is_none()
            && self.replicacao.papel == Papel::Replica
            && self.replicacao.origens.is_empty()
        {
            return Err(PhxError::Esquema(
                "papel replica exige ao menos uma origem em replicacao.origens".into(),
            ));
=======
        if self.replicacao.papel.puxa_de_origem() && self.replicacao.origens.is_empty() {
            return Err(PhxError::Esquema(format!(
                "papel {} exige ao menos uma origem em replicacao.origens",
                self.replicacao.papel.nome()
            )));
        }"""
novo_ini = """        // Duas regras que se somam: a lista de origens e exigida de TODO papel
        // que puxa (replica, read replica, spare, multi) -- e nao so do
        // `replica`, como era antes de os papeis novos existirem --, MENOS
        // quando ha cluster, porque ai a origem e o master CORRENTE descoberto
        // pelo pulso, e uma lista fixa apontaria para o master de ontem.
        if self.cluster.is_none()
            && self.replicacao.papel.puxa_de_origem()
            && self.replicacao.origens.is_empty()
        {
            return Err(PhxError::Esquema(format!(
                "papel {} exige ao menos uma origem em replicacao.origens",
                self.replicacao.papel.nome()
            )));
        }"""
assert velho_ini in t
t = t.replace(velho_ini, novo_ini, 1)
t = t.replace(""">>>>>>> worktree-agent-aeba5ba7fe4b19f92
""", "", 1)
p.write_text(t)
print("config.rs resolvido; marcas:", t.count("<<<<<<<"), t.count(">>>>>>>"))

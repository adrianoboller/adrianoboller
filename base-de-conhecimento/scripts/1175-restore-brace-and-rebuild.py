# Restore brace and rebuild
# 29/08 18:08

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()
velho = """    fn anotar_estado(&self, origem: &str, f: impl FnOnce(&mut EstadoOrigem)) {
        if let Ok(mut e) = self.estado_replicacao.lock() {
            f(e.entry(origem.to_string()).or_default());
        }

    /// O teto de linhas por resposta que vale AGORA."""
novo = """    fn anotar_estado(&self, origem: &str, f: impl FnOnce(&mut EstadoOrigem)) {
        if let Ok(mut e) = self.estado_replicacao.lock() {
            f(e.entry(origem.to_string()).or_default());
        }
    }

    /// O teto de linhas por resposta que vale AGORA."""
assert velho in t
p.write_text(t.replace(velho, novo, 1)); print("fechamento reposto")

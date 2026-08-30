# Restore brace and rebuild
# 29/08 18:40

import pathlib
p = pathlib.Path("crates/phxsql-server/src/servidor.rs"); t = p.read_text()
velho = """    pub fn espelho(&self) -> bool {
        self.espelho_vivo.load(Ordering::Relaxed)

    /// Toma a trava unica de dados"""
novo = """    pub fn espelho(&self) -> bool {
        self.espelho_vivo.load(Ordering::Relaxed)
    }

    /// Toma a trava unica de dados"""
assert velho in t
p.write_text(t.replace(velho, novo, 1)); print("fechamento reposto")

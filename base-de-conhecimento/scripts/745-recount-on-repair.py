# Recount on repair
# 28/08 19:43

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()
antigo = """    /// Confere os dois lados e conserta o que der. Ver `RegFile::reparar`.
    pub fn reparar(&mut self) -> Result<(u64, u64, u64)> {
        self.reg.reparar()
    }"""
novo = """    /// Confere os dois lados e conserta o que der. Ver `RegFile::reparar`.
    ///
    /// Reconta as marcadas no fim: o reparo pode ter trazido de volta um slot
    /// que estava ilegivel, e o contador do cabecalho nao sabia dele.
    pub fn reparar(&mut self) -> Result<(u64, u64, u64)> {
        let r = self.reg.reparar()?;
        self.recontar_marcadas()?;
        Ok(r)
    }"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")

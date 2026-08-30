# Guard rowid zero and retest
# 28/08 19:45

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()
antigo = """    pub fn rownum_de(&mut self, rowid: RowId) -> Result<u64> {
        if self.esquema.coluna_rownum().is_none() {
            return Ok(0);
        }"""
novo = """    pub fn rownum_de(&mut self, rowid: RowId) -> Result<u64> {
        // Rowid zero e o «pagina vazia» de quem chama, e nao um erro de faixa.
        if rowid == 0 || self.esquema.coluna_rownum().is_none() {
            return Ok(0);
        }"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")

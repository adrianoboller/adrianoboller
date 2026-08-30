# Add rownum_de and build
# 28/08 19:45

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()
antigo = """    /// Quantas linhas a visao enxerga, SEM varrer."""
novo = """    /// O `rownum` desta linha, sem decodificar o resto dela.
    ///
    /// Zero quando a tabela nao tem a coluna ou o slot esta livre -- e o mesmo
    /// «nao ha numero» dos dois lados, porque a tela trata os dois igual.
    pub fn rownum_de(&mut self, rowid: RowId) -> Result<u64> {
        if self.esquema.coluna_rownum().is_none() {
            return Ok(0);
        }
        match self.reg.ler(rowid)? {
            Some(p) => self.rownum_do_payload(&p),
            None => Ok(0),
        }
    }

    /// Quantas linhas a visao enxerga, SEM varrer."""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)

p = pathlib.Path("crates/phxsql-server/src/servidor.rs")
s = p.read_text()
antigo = """        let primeiro = rowids.first().copied().unwrap_or(0);
        let ultimo = rowids.last().copied().unwrap_or(0);"""
novo = """        let primeiro = rowids.first().copied().unwrap_or(0);
        let ultimo = rowids.last().copied().unwrap_or(0);
        let rownum_inicio = t.rownum_de(primeiro)?;
        let rownum_fim = t.rownum_de(ultimo)?;"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")

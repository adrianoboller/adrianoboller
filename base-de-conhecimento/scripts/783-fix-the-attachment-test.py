# Fix the attachment test
# 28/08 20:11

import pathlib
p = pathlib.Path("crates/phxsql-store/src/table.rs")
s = p.read_text()
antigo = """    /// Desmonta a imagem. Inversa exata de [`Table::imagem_da_linha`]."""
novo = """    /// A imagem da linha de um rowid, lendo o payload do `.reg`.
    pub fn imagem_da_linha_do_rowid(&mut self, rowid: RowId) -> Result<Vec<u8>> {
        let payload = self
            .reg
            .ler(rowid)?
            .ok_or_else(|| PhxError::NaoEncontrado(format!("registro {rowid} esta excluido")))?;
        self.imagem_da_linha(&payload)
    }

    /// Desmonta a imagem. Inversa exata de [`Table::imagem_da_linha`]."""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)

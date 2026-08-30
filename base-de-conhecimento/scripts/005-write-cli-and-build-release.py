# Write CLI and build release
# 27/08 17:57

p='crates/phxsql-store/src/table.rs'
s=open(p).read()
s=s.replace('''    pub fn sincronizar(&mut self) -> Result<()> {''','''    /// Ocupacao dos arquivos externos: `(.bin, .memo)`.
    pub fn estatisticas_externas(&self) -> (crate::blob::EstatisticaBlob, crate::blob::EstatisticaBlob) {
        (self.bin.estatistica(), self.memo.estatistica())
    }

    /// Paginas ocupadas pelo `.ndx`, incluindo a pagina 0 de cabecalho.
    pub fn paginas_indice(&self) -> u64 {
        self.ndx.paginas()
    }

    /// Descritores dos indices como estao gravados no `.ndx`.
    pub fn descritores_indices(&self) -> &[crate::ndx::DescritorIndice] {
        self.ndx.indices()
    }

    pub fn sincronizar(&mut self) -> Result<()> {''')
open(p,'w').write(s)

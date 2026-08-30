# Fix duplicate function and run tests
# 27/08 18:27

p='crates/phxsql-store/src/table.rs'
s=open(p).read()
s=s.replace('''    /// Ocupacao dos arquivos externos: `(.bin, .memo)`.
    pub fn estatisticas_externas(
        &self,
    ) -> (crate::blob::EstatisticaBlob, crate::blob::EstatisticaBlob) {
        (self.bin.estatistica(), self.memo.estatistica())
    }
''','''    /// Ocupacao dos arquivos externos: `(.bin, .memo)`.
    pub fn estatisticas_externas(
        &mut self,
    ) -> Result<(crate::blob::EstatisticaBlob, crate::blob::EstatisticaBlob)> {
        Ok((self.bin.estatistica()?, self.memo.estatistica()?))
    }

    /// Volumes existentes de cada arquivo paginado.
    pub fn volumes_por_arquivo(&self) -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>) {
        (
            self.reg.volumes(),
            self.bin.volumes(),
            self.memo.volumes(),
            self.log.volumes(),
        )
    }
''',1)
open(p,'w').write(s)

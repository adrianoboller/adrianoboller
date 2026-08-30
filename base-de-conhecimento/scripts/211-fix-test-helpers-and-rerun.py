# Fix test helpers and rerun
# 27/08 21:45

p='crates/phxsql-store/src/reg.rs'
s=open(p).read()
s=s.replace('temp("espelho")','dir_temp("espelho")').replace('temp("reparar")','dir_temp("reparar")')
s=s.replace('temp("perdidos")','dir_temp("perdidos")').replace('temp("semespelho")','dir_temp("semespelho")')
# o teste precisa escrever so no principal; em vez de furar o encapsulamento,
# usa um metodo proprio para isso
s=s.replace('''                // escrever() duplica no espelho; aqui queremos so o principal.
                let f = r.volumes.arquivo(v, true).unwrap();
                use std::io::{Seek, SeekFrom, Write};
                f.seek(SeekFrom::Start(off)).unwrap();
                f.write_all(&slot).unwrap();''',
'''                // escrever() duplica no espelho; aqui queremos SO o principal.
                r.volumes.escrever_so_no_principal(v, off, &slot).unwrap();''')
open(p,'w').write(s)

p='crates/phxsql-store/src/volume.rs'
s=open(p).read()
s=s.replace('''    /// Copia um trecho do principal para o espelho, para o reparo inverso.''',
'''    /// Escreve SO no principal, sem tocar no espelho.
    ///
    /// Existe para o reparo e para o teste que precisa estragar um lado so.
    /// Fora disso ninguem deve usar: escrita que nao vai aos dois lugares e
    /// exatamente o que o espelho existe para evitar.
    pub fn escrever_so_no_principal(
        &mut self,
        volume: u32,
        offset: u64,
        buf: &[u8],
    ) -> Result<()> {
        let f = self.arquivo(volume, true)?;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(buf)?;
        Ok(())
    }

    /// Copia um trecho do principal para o espelho, para o reparo inverso.''')
open(p,'w').write(s)

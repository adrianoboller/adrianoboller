# Fix the seeding bug and retest
# 27/08 21:46

p='crates/phxsql-store/src/volume.rs'
s=open(p).read()
s=s.replace('''    /// Copia um trecho do principal para o espelho, para o reparo inverso.''',
'''    /// Tamanho do volume no espelho. Zero quando ele ainda nao existe.
    pub fn tamanho_do_espelho(&mut self, volume: u32) -> Result<u64> {
        match &mut self.espelho {
            None => Ok(0),
            Some(e) => {
                if e.existe(volume) {
                    e.tamanho(volume)
                } else {
                    Ok(0)
                }
            }
        }
    }

    /// Copia um trecho do principal para o espelho, para o reparo inverso.''')
open(p,'w').write(s)

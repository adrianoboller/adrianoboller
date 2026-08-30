# Fix borrow errors and rebuild
# 27/08 17:52

p='crates/phxsql-store/src/reg.rs'
s=open(p).read()
s=s.replace("use std::io::{Seek, SeekFrom, Write};","use std::io::Write;")
s=s.replace("""        let mut slot = vec![0u8; self.slot_size];
        ler_exato(&mut self.arquivo, self.offset(rowid), &mut slot)?;
        if slot[0] != STATUS_ATIVO {""","""        let offset = self.offset(rowid);
        let mut slot = vec![0u8; self.slot_size];
        ler_exato(&mut self.arquivo, offset, &mut slot)?;
        if slot[0] != STATUS_ATIVO {""")
s=s.replace("""        let mut b = [0u8; 1];
        ler_exato(&mut self.arquivo, self.offset(rowid), &mut b)?;""","""        let offset = self.offset(rowid);
        let mut b = [0u8; 1];
        ler_exato(&mut self.arquivo, offset, &mut b)?;""")
open(p,'w').write(s)

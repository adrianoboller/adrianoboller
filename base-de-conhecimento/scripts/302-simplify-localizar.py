# Simplify localizar
# 28/08 11:18

import pathlib
p = pathlib.Path('crates/phxsql-store/src/reg.rs')
s = p.read_text()
v = '''    fn localizar(&self, rowid: RowId) -> (u32, u64) {
        let slot = match self.volume_por_fronteira(rowid) {
            None => return self.localizar_por_conta(rowid),
            Some(v) => rowid - self.fronteiras[v as usize - 1].primeiro_rowid + 1,
        };
        let volume = self.volume_por_fronteira(rowid).unwrap();
        (
            volume,
            self.data_offset + (slot - 1) * self.slot_size as u64,
        )
    }

    fn localizar_por_conta(&self, rowid: RowId) -> (u32, u64) {
        let (volume, slot) = self.esquema.paginacao().localizar(rowid);
        (
            volume,
            self.data_offset + (slot - 1) * self.slot_size as u64,
        )
    }'''
n = '''    fn localizar(&self, rowid: RowId) -> (u32, u64) {
        let (volume, slot) = match self.volume_por_fronteira(rowid) {
            Some(v) => (v, rowid - self.fronteiras[v as usize - 1].primeiro_rowid + 1),
            None => self.esquema.paginacao().localizar(rowid),
        };
        (
            volume,
            self.data_offset + (slot - 1) * self.slot_size as u64,
        )
    }'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')

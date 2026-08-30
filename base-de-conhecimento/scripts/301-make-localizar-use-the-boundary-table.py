# Make localizar use the boundary table
# 28/08 11:18

import pathlib
p = pathlib.Path('crates/phxsql-store/src/reg.rs')
s = p.read_text()

# ------------------------------------------------------------- localizar
v = '''    /// Volume e offset em que um rowid mora.
    fn localizar(&self, rowid: RowId) -> (u32, u64) {
        let (volume, slot) = self.esquema.paginacao().localizar(rowid);
        (
            volume,
            self.data_offset + (slot - 1) * self.slot_size as u64,
        )
    }'''
n = '''    /// Volume e offset em que um rowid mora.
    ///
    /// Na particao por quantidade e uma divisao. Na particao por periodo o
    /// volume nao sai de conta -- ele depende de quando o periodo virou --,
    /// entao sai de uma busca binaria na tabela de fronteiras. Nos dois casos
    /// o offset dentro do volume continua sendo multiplicacao.
    fn localizar(&self, rowid: RowId) -> (u32, u64) {
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
    }

    /// O ultimo volume que comeca em rowid menor ou igual ao pedido.
    ///
    /// `None` quando nao ha fronteiras -- ou seja, quando a particao e por
    /// quantidade e o volume sai de divisao.
    fn volume_por_fronteira(&self, rowid: RowId) -> Option<u32> {
        if self.fronteiras.is_empty() {
            return None;
        }
        let i = self
            .fronteiras
            .partition_point(|f| f.primeiro_rowid <= rowid);
        Some(i.max(1) as u32)
    }'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')

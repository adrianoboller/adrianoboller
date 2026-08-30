# Ligar o reindexar a construcao em lote
# 29/08 03:25

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()
velho = '''        let quantos_indices = self.esquema.indices().len();
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            let valores = self.decodificar(&payload, false)?;
            for i in 0..quantos_indices {
                let chave = self.codificar_chave(i, &valores)?;
                self.ndx.inserir(i, &chave, id)?;
            }
            rowid = id + 1;
        }
        self.ndx.verificar()'''
novo = '''        // Uma varredura do `.reg` para TODOS os indices, e depois uma
        // construcao em lote por indice -- em vez de uma descida na arvore por
        // chave, que e o mesmo trabalho do caminho de dentro feito de novo.
        let quantos_indices = self.esquema.indices().len();
        let mut lotes: Vec<Vec<u8>> = vec![Vec::new(); quantos_indices];
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            let valores = self.decodificar(&payload, false)?;
            for (i, lote) in lotes.iter_mut().enumerate() {
                let chave = self.codificar_chave(i, &valores)?;
                lote.extend_from_slice(&NdxFile::chave_completa(&chave, id));
            }
            rowid = id + 1;
        }
        for (i, lote) in lotes.into_iter().enumerate() {
            self.ndx.construir_em_lote(i, lote)?;
        }
        self.ndx.verificar()'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')

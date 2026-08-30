# Clean warning and run full tests
# 28/08 17:33

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''        self.conferir_motivo(motivo)?;
        let quantos = self.lixeira.total()?;
        self.motivos.registrar(Tipo::Expurgo, 0, motivo, "")?;''',
'''        self.conferir_motivo(motivo)?;
        self.motivos.registrar(Tipo::Expurgo, 0, motivo, "")?;''',1)
io.open(p,'w',encoding='utf-8').write(s)

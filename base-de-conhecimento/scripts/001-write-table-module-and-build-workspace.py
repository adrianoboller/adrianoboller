# Write table module and build workspace
# 27/08 17:51

import re
p='phxsql/crates/phxsql-store/src/ndx.rs'
s=open(p).read()
s=s.replace('''
impl Clone for DescritorIndice {
    fn clone(&self) -> Self {
        DescritorIndice {
            nome: self.nome.clone(),
            unico: self.unico,
            key_len: self.key_len,
            raiz: self.raiz,
            qtd_chaves: self.qtd_chaves,
        }
    }
}
''','')
s=s.replace('const TIPO_LIVRE: u8 = 0;','#[allow(dead_code)]\nconst TIPO_LIVRE: u8 = 0;')
s=s.replace('''        let mut pos = 0usize;
        for _ in 0..qtd_indices {
            let nl = u16::from_le_bytes(dir[pos..pos + 2].try_into().unwrap()) as usize;''','''        let mut pos = 0usize;
        for _ in 0..qtd_indices {
            if pos + 2 > dir.len() {
                return Err(PhxError::Corrompido(format!(
                    "diretorio de indices de {nome} truncado"
                )));
            }
            let nl = u16::from_le_bytes(dir[pos..pos + 2].try_into().unwrap()) as usize;
            if pos + 2 + nl + 21 > dir.len() {
                return Err(PhxError::Corrompido(format!(
                    "diretorio de indices de {nome} truncado"
                )));
            }''')
open(p,'w').write(s)
print("ndx ajustado")

# Ler a marca na abertura e compilar
# 29/08 06:01

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        let dir_crc = c.u32(40);

        let mut dir = vec![0u8; dir_len];'''
novo='''        let dir_crc = c.u32(40);
        // Byte 52: a marca de sujo. Arquivo escrito antes da 0.18.0 tem zero
        // ali, e zero e "limpo" -- que e a verdade para quem so escrevia
        // atraves. Nao ha migracao a fazer.
        let sujo = cab[52] != 0;

        let mut dir = vec![0u8; dir_len];'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')

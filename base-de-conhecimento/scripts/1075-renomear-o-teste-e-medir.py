# Renomear o teste e medir
# 29/08 05:37

import io
p='crates/phxsql-core/src/crc.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('fn slice8_concorda_com_o_laco_byte_a_byte','fn a_versao_rapida_concorda_com_o_laco_byte_a_byte')
io.open(p,'w',encoding='utf-8').write(s)

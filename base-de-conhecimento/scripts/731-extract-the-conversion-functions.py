# Extract the conversion functions
# 28/08 19:29

import io, re
sv='crates/phxsql-server/src/valores.rs'
s=io.open(sv,encoding='utf-8').read()

# recorta as tres funcoes que vao para o nucleo
def recortar(marca_ini, marca_fim):
    global s
    i=s.index(marca_ini)
    j=s.index(marca_fim)
    bloco=s[i:j]
    s=s[:i]+s[j:]
    return bloco

bloco_texto = recortar('/// Converte um valor que veio de um formato de TEXTO',
                       'pub fn json_para_valor(j: &Json, ty: &ColumnType) -> Result<Value> {')
# texto_para_decimal fica no servidor tambem? move para o nucleo.
i=s.index('pub fn texto_para_decimal(')
# acha o fim da funcao pelo fecha-chaves na coluna 0
j=s.index('\n}\n', i)+3
bloco_dec = s[i:j]
s=s[:i]+s[j:]
io.open(sv,'w',encoding='utf-8').write(s)
io.open('/tmp/bloco_texto.rs','w',encoding='utf-8').write(bloco_texto)
io.open('/tmp/bloco_dec.rs','w',encoding='utf-8').write(bloco_dec)
print('recortado:', len(bloco_texto), len(bloco_dec))

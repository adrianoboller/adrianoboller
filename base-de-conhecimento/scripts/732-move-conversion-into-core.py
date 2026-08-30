# Move conversion into core
# 28/08 19:29

import io
# 1. o nucleo recebe as funcoes
p='crates/phxsql-core/src/carga.rs'
s=io.open(p,encoding='utf-8').read()
texto=io.open('/tmp/bloco_texto.rs',encoding='utf-8').read()
dec=io.open('/tmp/bloco_dec.rs',encoding='utf-8').read()

# adapta: os nomes do nucleo
texto = texto.replace('json_para_valor_de_texto','valor_de_texto')
texto = texto.replace('json_para_linha_de_texto','linha_de_texto')
texto = texto.replace('return json_para_valor(j, ty);','return crate::valores_json::json_para_valor(j, ty);')
texto = texto.replace('_ => json_para_valor(&Json::texto_de(t), ty)?,','_ => crate::valores_json::json_para_valor(&Json::texto_de(t), ty)?,')
texto = texto.replace('phxsql_core::schema::COLUNA_ROWNUM','crate::schema::COLUNA_ROWNUM')

cabecalho = '''
// ---------------------------------------------------- do texto para o valor

use crate::schema::Schema;
use crate::types::ColumnType;
use crate::value::Value;

'''
s = s.replace('#[cfg(test)]\nmod testes {', cabecalho + dec + "\n" + texto + '\n#[cfg(test)]\nmod testes {',1)
io.open(p,'w',encoding='utf-8').write(s)
print('nucleo ok')

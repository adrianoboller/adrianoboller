# Validate the primary key in Schema::new
# 28/08 11:14

import pathlib
p = pathlib.Path('crates/phxsql-core/src/schema.rs')
s = p.read_text()

# validacao da chave primaria dentro do Schema::new
v = '''        // Uma sequencia por tabela. O contador mora no cabecalho do `.reg`, e
        // e um so: duas colunas Sequence dividiriam o mesmo numerador, o que
        // ninguem espera ao escrever o esquema.'''
n = '''        // So uma chave primaria, e ela e unica. Duas primarias seriam duas
        // identidades para a mesma linha, e uma primaria que aceita duplicata
        // nao identifica nada -- os dois casos sao erro de esquema, nao
        // preferencia.
        let primarias: Vec<&str> = indices
            .iter()
            .filter(|i| i.primario)
            .map(|i| i.nome.as_str())
            .collect();
        if primarias.len() > 1 {
            return Err(PhxError::Esquema(format!(
                "a tabela {nome} tem {} chaves primarias ({}); pode ter no maximo uma",
                primarias.len(),
                primarias.join(", ")
            )));
        }
        if let Some(idx) = indices.iter().find(|i| i.primario && !i.unico) {
            return Err(PhxError::Esquema(format!(
                "a chave primaria {} nao esta marcada como unica",
                idx.nome
            )));
        }
        // Coluna de chave primaria nao pode ser nula: uma identidade nula nao
        // identifica.
        if let Some(idx) = indices.iter().find(|i| i.primario) {
            for ic in &idx.colunas {
                if colunas[ic.coluna].nullable {
                    return Err(PhxError::Esquema(format!(
                        "a coluna {} faz parte da chave primaria {} e aceita nulo",
                        colunas[ic.coluna].nome, idx.nome
                    )));
                }
            }
        }

        // Uma sequencia por tabela. O contador mora no cabecalho do `.reg`, e
        // e um so: duas colunas Sequence dividiriam o mesmo numerador, o que
        // ninguem espera ao escrever o esquema.'''
assert s.count(v) == 1
p.write_text(s.replace(v, n))
print('ok')

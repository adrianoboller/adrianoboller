# Refuse updates that would change the bucket
# 28/08 18:48

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()
velho='''        let ponteiros_antigos = self.ponteiros(&antigo)?;
        let payload = self.montar_payload(valores)?;
        let versao = self.reg.atualizar(rowid, &payload)?;'''
novo='''        // Na particao alfanumerica, o balde e o ENDERECO: mudar a coluna de
        // referencia de «Silva» para «Andrade» mudaria o arquivo em que a
        // linha mora, e com ele o rowid -- que e a identidade dela e esta em
        // todo indice. Mover nao e opcao; deixar a linha no balde errado
        // tambem nao, porque ai o `_S` deixa de conter os S e a particao para
        // de valer. Entao a alteracao e RECUSADA, com o caminho escrito.
        if let (Some(a), Some(b)) = (
            self.balde_da_linha(&valores_antigos)?,
            self.balde_da_linha(valores)?,
        ) {
            if a != b {
                let baldes = phxsql_core::paginacao::BALDES;
                return Err(PhxError::Esquema(format!(
                    "a alteracao mudaria o balde de {} para {}, e o balde e o \\
                     endereco fisico da linha em {}. Exclua e insira de novo: \\
                     a linha nova nasce no balde certo, com outro rowid",
                    baldes[a as usize - 1],
                    baldes[b as usize - 1],
                    self.nome
                )));
            }
        }

        let ponteiros_antigos = self.ponteiros(&antigo)?;
        let payload = self.montar_payload(valores)?;
        let versao = self.reg.atualizar(rowid, &payload)?;'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)

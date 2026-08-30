# Reconciliar o contador e testar
# 29/08 05:18

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()
velho='''            if total != d.qtd_chaves {
                return Err(PhxError::Corrompido(format!(
                    "indice {}: diretorio diz {} chaves, varredura achou {total}",
                    d.nome, d.qtd_chaves
                )));
            }
            saida.push((d.nome, total));'''
novo='''            // Os dois sentidos NAO sao a mesma coisa, e confundi-los faria
            // esta conferencia gritar corrupcao numa arvore sadia.
            //
            // Varredura MAIOR que o contador: o contador ficou para tras, que e
            // o que uma queda entre dois `sincronizar` deixa -- a arvore tem
            // todas as chaves, e o numero e que esta velho. Conserta-se.
            //
            // Varredura MENOR: falta chave na arvore. Isso e corrupcao, e
            // continua parando aqui.
            if total < d.qtd_chaves {
                return Err(PhxError::Corrompido(format!(
                    "indice {}: diretorio diz {} chaves e a varredura achou so {total}",
                    d.nome, d.qtd_chaves
                )));
            }
            if total > d.qtd_chaves {
                self.indices[i].qtd_chaves = total;
                self.estrutura_mudou = true;
            }
            saida.push((d.nome, total));'''
assert s.count(velho)==1
s=s.replace(velho,novo)
io.open(p,'w',encoding='utf-8').write(s)
print('ok')

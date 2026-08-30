# Add binary search by rownum and test it
# 28/08 18:36

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    /// A pagina ANTERIOR ao cursor, para o botao de voltar.'''
novo='''    /// O rowid da primeira linha cujo `rownum` e >= `alvo`.
    ///
    /// # Por que isto e uma busca binaria, e nao uma varredura
    ///
    /// O `rownum` cresce com a ordem de chegada, e o `.reg` guarda as linhas
    /// na ordem de chegada. Entao o `rownum` **cresce com o rowid**, e uma
    /// sequencia crescente num arquivo de acesso aleatorio se procura por
    /// bisseccao: log2 de um milhao sao vinte leituras.
    ///
    /// Nao ha indice envolvido, e nao ha indice a manter. E o mesmo motivo de
    /// o endereco sair de uma conta: a ordem logica e a ordem fisica.
    ///
    /// Slot excluido nao tem `rownum` para comparar; a bisseccao anda para o
    /// vizinho vivo mais proximo, o que custa alguns passos a mais num trecho
    /// muito esburacado e nao muda a resposta.
    ///
    /// `None` quando nenhuma linha tem `rownum` >= alvo, ou quando a tabela
    /// nao tem a coluna.
    pub fn rowid_do_rownum(&mut self, alvo: u64) -> Result<Option<RowId>> {
        if self.esquema.coluna_rownum().is_none() {
            return Ok(None);
        }
        let (mut baixo, mut alto) = (1u64, self.reg.slots());
        if alto == 0 {
            return Ok(None);
        }
        let mut achado = None;
        while baixo <= alto {
            let meio = baixo + (alto - baixo) / 2;
            // Anda para a frente ate achar um slot vivo, sem passar do alto.
            let Some((id, payload)) = self.reg.proximo_ativo(meio)? else {
                // So ha buraco daqui para a frente: o alvo esta atras.
                if meio == 0 {
                    break;
                }
                alto = meio - 1;
                continue;
            };
            if id > alto {
                if meio == 0 {
                    break;
                }
                alto = meio - 1;
                continue;
            }
            if self.rownum_do_payload(&payload)? >= alvo {
                achado = Some(id);
                if id == 0 {
                    break;
                }
                alto = id - 1;
            } else {
                baixo = id + 1;
            }
        }
        Ok(achado)
    }

    /// A pagina que comeca no numero de ordem `alvo`, inclusive.
    ///
    /// E o cursor da tela quando quem pagina guarda o `rownum` e nao o rowid --
    /// que e o caso da particao alfanumerica, onde o rowid de volumes
    /// diferentes nao se compara.
    pub fn pagina_desde_rownum(
        &mut self,
        alvo: u64,
        limite: u64,
        visao: Visao,
    ) -> Result<Vec<RowId>> {
        let Some(inicio) = self.rowid_do_rownum(alvo)? else {
            return Ok(Vec::new());
        };
        // `depois_de` exclui o proprio cursor, e aqui o inicio ENTRA.
        self.pagina_depois_de(inicio.saturating_sub(1), limite, visao)
    }

    /// A pagina ANTERIOR ao cursor, para o botao de voltar.'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)

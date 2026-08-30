# Add page and cursor reads
# 28/08 18:23

import io
p='crates/phxsql-store/src/table.rs'
s=io.open(p,encoding='utf-8').read()

velho='''    /// Tira da lista os rowids que a visao nao enxerga.'''
novo='''    // ------------------------------------------------------------- paginas

    /// Uma pagina de rowids, sem decodificar linha nenhuma.
    ///
    /// # Por que isto existe separado da varredura
    ///
    /// `varrer_com` decodifica CADA linha da tabela -- com os anexos do `.bin`
    /// e do `.memo` -- e devolve tudo. Quem quer duzentas linhas de um milhao
    /// pagava um milhao de decodificacoes e um milhao de leituras de anexo,
    /// para jogar 999.800 fora. O custo crescia com a TABELA, e nao com a
    /// pagina, que e o defeito que o `LIMIT`/`OFFSET` de qualquer motor tem --
    /// so que aqui era pior, porque o `OFFSET` ao menos nao carrega o blob.
    ///
    /// Aqui a leitura para no teto, e nada e decodificado: para decidir se um
    /// slot entra basta o byte da coluna de sistema.
    ///
    /// `pular` continua existindo porque tela pequena precisa dele, e porque
    /// nem toda ordenacao tem cursor. Mas ele e o modo de compatibilidade --
    /// quem tem tabela grande usa [`Table::pagina_depois_de`].
    pub fn pagina(&mut self, pular: u64, limite: u64, visao: Visao) -> Result<Vec<RowId>> {
        let mut saida = Vec::new();
        let mut vistos = 0u64;
        let mut rowid = 1;
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            rowid = id + 1;
            if !self.visao_aceita_payload(&payload, visao)? {
                continue;
            }
            if vistos >= pular {
                saida.push(id);
                if limite > 0 && saida.len() as u64 >= limite {
                    break;
                }
            }
            vistos += 1;
        }
        Ok(saida)
    }

    /// A pagina que vem DEPOIS do rowid `cursor`. O *keyset* do PhxSql.
    ///
    /// # Por que aqui ele sai de graca
    ///
    /// Num motor relacional, pular para o meio da tabela exige um indice: a
    /// ordem logica nao tem nada a ver com a posicao fisica. Aqui tem --
    /// `offset = data_offset + (rowid-1) x slot_size`. Continuar depois do
    /// rowid 500.000 nao e procurar: e uma conta.
    ///
    /// O custo e o da PAGINA, e nao o da tabela. E a diferenca entre uma tela
    /// que abre igual na pagina 1 e na pagina 10.000, e uma que vai ficando
    /// lenta conforme o usuario desce.
    ///
    /// Cursor zero comeca do inicio. A pagina nunca inclui o proprio cursor,
    /// para o cliente poder mandar de volta o ultimo rowid que recebeu sem
    /// receber a mesma linha duas vezes.
    pub fn pagina_depois_de(
        &mut self,
        cursor: RowId,
        limite: u64,
        visao: Visao,
    ) -> Result<Vec<RowId>> {
        let mut saida = Vec::new();
        let mut rowid = cursor.saturating_add(1);
        while let Some((id, payload)) = self.reg.proximo_ativo(rowid)? {
            rowid = id + 1;
            if !self.visao_aceita_payload(&payload, visao)? {
                continue;
            }
            saida.push(id);
            if limite > 0 && saida.len() as u64 >= limite {
                break;
            }
        }
        Ok(saida)
    }

    /// A pagina ANTERIOR ao cursor, para o botao de voltar.
    ///
    /// Devolve em ordem crescente, como a de ir: quem chama nao deveria ter de
    /// saber que a leitura veio de tras para a frente.
    pub fn pagina_antes_de(
        &mut self,
        cursor: RowId,
        limite: u64,
        visao: Visao,
    ) -> Result<Vec<RowId>> {
        if cursor <= 1 {
            return Ok(Vec::new());
        }
        let mut saida = Vec::new();
        let mut rowid = cursor - 1;
        while rowid >= 1 {
            if let Some(payload) = self.reg.ler(rowid)? {
                if self.visao_aceita_payload(&payload, visao)? {
                    saida.push(rowid);
                    if limite > 0 && saida.len() as u64 >= limite {
                        break;
                    }
                }
            }
            if rowid == 1 {
                break;
            }
            rowid -= 1;
        }
        saida.reverse();
        Ok(saida)
    }

    /// A visao aceita este payload? Le SO o byte da coluna de sistema.
    ///
    /// Decodificar a linha inteira para olhar um bit seria pagar o `.memo` e o
    /// `.bin` de cada linha percorrida -- que e justamente o que a paginacao
    /// existe para nao fazer.
    fn visao_aceita_payload(&self, payload: &[u8], visao: Visao) -> Result<bool> {
        if visao == Visao::Todas {
            return Ok(true);
        }
        let Some(i) = self.esquema.coluna_softdeleted() else {
            return Ok(visao != Visao::Excluidas);
        };
        // Nulo no bitmap nao acontece nesta coluna, que e obrigatoria -- mas
        // se acontecer, «nao marcada» e a leitura segura.
        let excluida = if payload[i / 8] & (1 << (i % 8)) != 0 {
            false
        } else {
            let off = self.esquema.offset_coluna(i)?;
            payload[off] != 0
        };
        Ok(visao.aceita(excluida))
    }

    /// Tira da lista os rowids que a visao nao enxerga.'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)

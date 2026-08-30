# Implement the volume-cut decision
# 28/08 11:19

import pathlib
p = pathlib.Path('crates/phxsql-store/src/reg.rs')
s = p.read_text()

v = '''    /// Anexa um registro no fim e devolve seu rowid.
    pub fn inserir(&mut self, payload: &[u8]) -> Result<RowId> {
        if payload.len() != self.esquema.payload_len() {
            return Err(PhxError::Corrompido(format!(
                "payload de {} bytes, esperado {}",
                payload.len(),
                self.esquema.payload_len()
            )));
        }
        let rowid = self.slot_count + 1;
        let paginacao = self.esquema.paginacao();
        if !paginacao.cabe(rowid) {
            return Err(PhxError::LimiteExcedido(format!(
                "tabela {} cheia: capacidade de {} registros ({} por arquivo x {} arquivos)",
                self.volumes.nome(),
                paginacao.capacidade(),
                paginacao.registros_por_arquivo,
                paginacao.max_arquivos
            )));
        }

        let (volume, offset) = self.localizar(rowid);
        if self.volumes.garantir(volume)? {
            // Volume novo: ganha cabecalho e esquema proprios.
            self.gravar_cabecalho(volume)?;
        }
'''
n = '''    /// Anexa um registro no fim e devolve seu rowid.
    pub fn inserir(&mut self, payload: &[u8]) -> Result<RowId> {
        self.inserir_no_periodo(payload, None)
    }

    /// Anexa, dizendo em que periodo a linha cai.
    ///
    /// A chave do periodo vem de cima porque o `.reg` so conhece bytes: quem
    /// sabe ler a coluna de data e a `Table`, que tem o esquema e os valores.
    /// Na particao por quantidade a chave e ignorada.
    pub fn inserir_no_periodo(&mut self, payload: &[u8], chave: Option<i64>) -> Result<RowId> {
        if payload.len() != self.esquema.payload_len() {
            return Err(PhxError::Corrompido(format!(
                "payload de {} bytes, esperado {}",
                payload.len(),
                self.esquema.payload_len()
            )));
        }
        let rowid = self.slot_count + 1;
        let paginacao = self.esquema.paginacao();
        let por_periodo = paginacao.modo.periodo().is_some();

        if !por_periodo && !paginacao.cabe(rowid) {
            return Err(PhxError::LimiteExcedido(format!(
                "tabela {} cheia: capacidade de {} registros ({} por arquivo x {} arquivos)",
                self.volumes.nome(),
                paginacao.capacidade(),
                paginacao.registros_por_arquivo,
                paginacao.max_arquivos
            )));
        }

        let (volume, offset) = if por_periodo {
            self.abrir_faixa_do_periodo(rowid, chave.unwrap_or(0))?
        } else {
            self.localizar(rowid)
        };
        if self.volumes.garantir(volume)? {
            // Volume novo: ganha cabecalho e esquema proprios.
            self.gravar_cabecalho(volume)?;
        }
'''
assert s.count(v) == 1
s = s.replace(v, n)

# --------------------------------------------- a decisao de cortar o volume
v = '''    /// O ultimo volume que comeca em rowid menor ou igual ao pedido.'''
n = '''    /// Decide em que volume a linha nova entra, cortando se preciso.
    ///
    /// Corta em dois casos, e o segundo e o que a particao por periodo existe
    /// para fazer:
    ///
    /// 1. o volume corrente encheu (`registros_por_arquivo` continua sendo
    ///    teto, senao um mes movimentado estouraria o arquivo);
    /// 2. o periodo virou.
    ///
    /// O que ele NAO faz e mandar a linha para um volume anterior. Um
    /// lancamento de janeiro digitado em marco entra no volume de marco: a
    /// ordem de digitacao manda, e voltar significaria escrever no meio de um
    /// arquivo ja fechado.
    fn abrir_faixa_do_periodo(&mut self, rowid: RowId, chave: i64) -> Result<(u32, u64)> {
        let paginacao = self.esquema.paginacao();
        let corta = match self.fronteiras.last() {
            None => true,
            Some(f) => {
                let no_volume = rowid - f.primeiro_rowid;
                no_volume >= paginacao.registros_por_arquivo || f.chave_periodo != chave
            }
        };
        if corta {
            if self.fronteiras.len() as u64 >= paginacao.max_arquivos as u64 {
                return Err(PhxError::LimiteExcedido(format!(
                    "tabela {} cheia: {} volumes, o teto do sufixo de {} digitos",
                    self.volumes.nome(),
                    paginacao.max_arquivos,
                    paginacao.digitos
                )));
            }
            self.fronteiras.push(Fronteira {
                primeiro_rowid: rowid,
                chave_periodo: chave,
            });
        }
        let volume = self.fronteiras.len() as u32;
        let f = self.fronteiras[volume as usize - 1];
        Ok((
            volume,
            self.data_offset + (rowid - f.primeiro_rowid) * self.slot_size as u64,
        ))
    }

    /// O ultimo volume que comeca em rowid menor ou igual ao pedido.'''
assert s.count(v) == 1
s = s.replace(v, n)

# ------------------------------------------------ o cabecalho grava a fronteira
v = '''        por_i64(&mut buf, 60, self.criado_em);
        por_i64(&mut buf, 68, agora());'''
n = '''        por_i64(&mut buf, 60, self.criado_em);
        por_i64(&mut buf, 68, agora());
        // A fronteira deste volume, na particao por periodo. Cada volume
        // carrega a sua, e por isso a tabela se remonta lendo os cabecalhos --
        // sem arquivo extra e sem bloco que cresce.
        if let Some(f) = self.fronteiras.get(volume as usize - 1) {
            por_u64(&mut buf, 76, f.primeiro_rowid);
            por_u64(&mut buf, 84, f.chave_periodo as u64);
        }'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')

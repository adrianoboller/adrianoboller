# Add insert-by-bucket to the .reg
# 28/08 18:46

import io
p='crates/phxsql-store/src/reg.rs'
s=io.open(p,encoding='utf-8').read()

velho='''    pub fn inserir_no_periodo(&mut self, payload: &[u8], chave: Option<i64>) -> Result<RowId> {
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

        if !por_periodo && !paginacao.cabe(rowid) {'''

novo='''    /// Insere no BALDE da particao alfanumerica.
    ///
    /// O rowid nao vem de `slot_count + 1`: vem da conta que poe a linha no
    /// arquivo dela.
    ///
    /// ```text
    /// rowid = (balde - 1) x registros_por_arquivo + slot_no_balde
    /// ```
    ///
    /// E a inversa exata do que `Paginacao::localizar` ja fazia, e por isso
    /// nenhum caminho de LEITURA precisou mudar: `localizar` continua
    /// devolvendo (volume, offset) por divisao, e o `.ndx` continua guardando
    /// rowid sem saber que balde existe.
    ///
    /// `registros_por_arquivo` passa a ser um teto POR LETRA, e nao da tabela.
    /// Numa base brasileira o `_S` enche muito antes do `_K`, e o erro diz qual
    /// balde encheu -- porque «tabela cheia» com 3% de ocupacao seria uma
    /// mensagem que nao ajuda ninguem.
    pub fn inserir_no_balde(&mut self, payload: &[u8], balde: u32) -> Result<RowId> {
        self.conferir_payload(payload)?;
        let paginacao = self.esquema.paginacao();
        let i = balde as usize;
        if i == 0 || i > self.baldes.len() {
            return Err(PhxError::Esquema(format!(
                "balde {balde} fora da faixa 1..={}",
                self.baldes.len()
            )));
        }

        let usados = self.baldes[i - 1];
        if usados >= paginacao.registros_por_arquivo {
            return Err(PhxError::LimiteExcedido(format!(
                "o balde {} de {} encheu: {} registros, o teto por letra",
                BALDES[i - 1],
                self.volumes.nome(),
                paginacao.registros_por_arquivo
            )));
        }

        let rowid = (balde as u64 - 1) * paginacao.registros_por_arquivo + usados + 1;
        let (volume, offset) = paginacao.localizar(rowid);
        debug_assert_eq!(volume, balde, "a conta do rowid nao bate com o balde");
        let offset = self.data_offset + (offset - 1) * self.slot_size as u64;

        if self.volumes.garantir(volume)? {
            self.gravar_cabecalho(volume)?;
        }
        self.escrever_slot(volume, offset, payload)?;

        self.baldes[i - 1] = usados + 1;
        self.live_count += 1;
        // `slot_count` vira a MARCA D'AGUA: o maior rowid que ja existiu. Ele
        // deixa de ser "quantos slots" -- com baldes, a tabela tem buracos
        // enormes entre um balde e o seguinte -- e continua servindo para o
        // que `conferir_faixa` precisa: recusar rowid que nunca foi gravado.
        self.slot_count = self.slot_count.max(rowid);
        self.gravar_cabecalho(volume)?;
        if volume != 1 {
            self.gravar_cabecalho(1)?;
        }
        Ok(rowid)
    }

    fn conferir_payload(&self, payload: &[u8]) -> Result<()> {
        if payload.len() != self.esquema.payload_len() {
            return Err(PhxError::Corrompido(format!(
                "payload de {} bytes, esperado {}",
                payload.len(),
                self.esquema.payload_len()
            )));
        }
        Ok(())
    }

    fn escrever_slot(&mut self, volume: u32, offset: u64, payload: &[u8]) -> Result<()> {
        let mut slot = vec![0u8; self.slot_size];
        slot[0] = STATUS_ATIVO;
        por_u32(&mut slot, 4, crc32(payload));
        por_u64(&mut slot, 8, 1); // versao do registro
        slot[SLOT_CAB..].copy_from_slice(payload);
        self.volumes.escrever(volume, offset, &slot)
    }

    pub fn inserir_no_periodo(&mut self, payload: &[u8], chave: Option<i64>) -> Result<RowId> {
        self.conferir_payload(payload)?;
        let rowid = self.slot_count + 1;
        let paginacao = self.esquema.paginacao();
        let por_periodo = paginacao.modo.periodo().is_some();

        if !por_periodo && !paginacao.cabe(rowid) {'''
assert velho in s
s=s.replace(velho,novo,1)

# o corpo antigo do inserir_no_periodo usa o slot montado a mao; troca pelo helper
velho2='''        let mut slot = vec![0u8; self.slot_size];
        slot[0] = STATUS_ATIVO;
        por_u32(&mut slot, 4, crc32(payload));
        por_u64(&mut slot, 8, 1); // versao do registro
        slot[SLOT_CAB..].copy_from_slice(payload);
        self.volumes.escrever(volume, offset, &slot)?;

        self.slot_count += 1;
        self.live_count += 1;
        self.gravar_cabecalho(1)?;
        Ok(rowid)
    }'''
novo2='''        self.escrever_slot(volume, offset, payload)?;

        self.slot_count += 1;
        self.live_count += 1;
        self.gravar_cabecalho(1)?;
        Ok(rowid)
    }'''
assert velho2 in s
s=s.replace(velho2,novo2,1)
io.open(p,'w',encoding='utf-8').write(s)

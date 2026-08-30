# Make scans bucket-aware
# 28/08 18:46

import io
p='crates/phxsql-store/src/reg.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    pub fn proximo_ativo(&mut self, desde: RowId) -> Result<Option<(RowId, Vec<u8>)>> {
        let mut rowid = desde.max(1);
        while rowid <= self.slot_count {
            if let Some(p) = self.ler(rowid)? {
                return Ok(Some((rowid, p)));
            }
            rowid += 1;
        }
        Ok(None)
    }'''
novo='''    pub fn proximo_ativo(&mut self, desde: RowId) -> Result<Option<(RowId, Vec<u8>)>> {
        if !self.baldes.is_empty() {
            return self.proximo_ativo_por_balde(desde);
        }
        let mut rowid = desde.max(1);
        while rowid <= self.slot_count {
            if let Some(p) = self.ler(rowid)? {
                return Ok(Some((rowid, p)));
            }
            rowid += 1;
        }
        Ok(None)
    }

    /// O proximo ativo quando a tabela e alfanumerica.
    ///
    /// Aqui `slot_count` e uma marca d'agua, e nao uma contagem: entre o fim do
    /// balde `_A` e o comeco do `_B` ha `registros_por_arquivo` menos os usados
    /// de puro vazio. Andar de um em um por esse vazio faria uma varredura de
    /// mil linhas custar milhoes de leituras -- que e exatamente o defeito que
    /// a paginacao acabou de tirar do caminho.
    ///
    /// Entao a varredura anda POR BALDE: dentro do balde vai ate `usados`, e
    /// no fim dele salta direto para o inicio do proximo. A tabela e percorrida
    /// na ordem dos baldes, que e a ordem alfabetica -- e nao na ordem de
    /// chegada, que na alfanumerica mora no `rownum`.
    fn proximo_ativo_por_balde(&mut self, desde: RowId) -> Result<Option<(RowId, Vec<u8>)>> {
        let rpa = self.esquema.paginacao().registros_por_arquivo;
        let desde = desde.max(1);
        let mut balde = ((desde - 1) / rpa) as usize;
        let mut slot = (desde - 1) % rpa;

        while balde < self.baldes.len() {
            let usados = self.baldes[balde];
            while slot < usados {
                let rowid = balde as u64 * rpa + slot + 1;
                if let Some(p) = self.ler(rowid)? {
                    return Ok(Some((rowid, p)));
                }
                slot += 1;
            }
            balde += 1;
            slot = 0;
        }
        Ok(None)
    }

    /// Quantos slots cada balde ja usou. Vazio fora da particao alfanumerica.
    pub fn baldes(&self) -> &[u64] {
        &self.baldes
    }'''
assert velho in s
s=s.replace(velho,novo,1)

# conferir_faixa: na alfanumerica a faixa e a capacidade
velho2='''    fn conferir_faixa(&self, rowid: RowId) -> Result<()> {
        if rowid == 0 || rowid > self.slot_count {'''
novo2='''    fn conferir_faixa(&self, rowid: RowId) -> Result<()> {
        if !self.baldes.is_empty() {
            // Na alfanumerica o rowid diz o balde: a faixa valida e a
            // capacidade da tabela, e o que decide se a linha existe e o slot
            // estar dentro do `usados` daquele balde.
            let rpa = self.esquema.paginacao().registros_por_arquivo;
            let balde = ((rowid.max(1) - 1) / rpa) as usize;
            let slot = (rowid.max(1) - 1) % rpa;
            if rowid == 0 || balde >= self.baldes.len() || slot >= self.baldes[balde] {
                return Err(PhxError::NaoEncontrado(format!(
                    "rowid {rowid} nao existe em {}",
                    self.volumes.nome()
                )));
            }
            return Ok(());
        }
        if rowid == 0 || rowid > self.slot_count {'''
assert velho2 in s
s=s.replace(velho2,novo2,1)

# verificar: percorre por balde tambem
velho3='''    pub fn verificar(&mut self) -> Result<u64> {
        let mut vivos = 0u64;
        for rowid in 1..=self.slot_count {
            if self.ler(rowid)?.is_some() {
                vivos += 1;
            }
        }'''
novo3='''    pub fn verificar(&mut self) -> Result<u64> {
        let mut vivos = 0u64;
        // Pelo `proximo_ativo`, que sabe saltar os vazios entre baldes: um
        // `for` de 1 ate a marca d'agua percorreria os buracos da alfanumerica.
        let mut rowid = 1;
        while let Some((id, _)) = self.proximo_ativo(rowid)? {
            vivos += 1;
            rowid = id + 1;
        }'''
assert velho3 in s
s=s.replace(velho3,novo3,1)
io.open(p,'w',encoding='utf-8').write(s)

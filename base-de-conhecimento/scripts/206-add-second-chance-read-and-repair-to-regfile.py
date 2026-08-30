# Add second-chance read and repair to RegFile
# 27/08 21:43

p='crates/phxsql-store/src/reg.rs'
s=open(p).read()

# leitura com segunda chance
velho = '''        let payload = slot[SLOT_CAB..].to_vec();
        if crc32(&payload) != Campos(&slot).u32(4) {
            return Err(PhxError::Corrompido(format!(
                "CRC do registro {rowid} em {} nao confere",
                self.volumes.caminho(volume).display()
            )));
        }
        Ok(Some(payload))'''
novo = '''        let payload = slot[SLOT_CAB..].to_vec();
        if crc32(&payload) != Campos(&slot).u32(4) {
            // A segunda chance: se ha espelho, o outro lado pode estar bom.
            if self.volumes.tem_espelho() {
                let mut copia = vec![0u8; self.slot_size];
                if self.volumes.ler_do_espelho(volume, offset, &mut copia).is_ok()
                    && copia[0] == STATUS_ATIVO
                {
                    let dele = copia[SLOT_CAB..].to_vec();
                    if crc32(&dele) == Campos(&copia).u32(4) {
                        self.recuperados += 1;
                        return Ok(Some(dele));
                    }
                }
            }
            return Err(PhxError::Corrompido(format!(
                "CRC do registro {rowid} em {} nao confere{}",
                self.volumes.caminho(volume).display(),
                if self.volumes.tem_espelho() {
                    " -- e o espelho tambem nao tem uma copia boa"
                } else {
                    ""
                }
            )));
        }
        Ok(Some(payload))'''
assert s.count(velho)==1
s=s.replace(velho,novo)

# contador de recuperacoes
s=s.replace('''    pub fn esquema(&self) -> &Schema {''','''    /// Quantas leituras foram salvas pelo espelho desde que a tabela abriu.
    ///
    /// Nao e curiosidade: recuperacao silenciosa e a pior especie. Se este
    /// numero sobe, alguma coisa esta estragando dado, e alguem precisa saber.
    pub fn recuperados(&self) -> u64 {
        self.recuperados
    }

    /// Percorre todos os slots e conserta os que o espelho consegue salvar.
    ///
    /// Devolve (conferidos, reparados, perdidos). Repara nos DOIS sentidos:
    /// se o principal esta bom e o espelho nao, o espelho e reescrito -- senao
    /// a segunda chance de amanha ja nasceria queimada.
    pub fn reparar(&mut self) -> Result<(u64, u64, u64)> {
        if !self.volumes.tem_espelho() {
            return Err(PhxError::Esquema(
                "esta tabela nao tem espelho: ligue \\"espelho\\" no config.json antes".into(),
            ));
        }
        let (mut conferidos, mut reparados, mut perdidos) = (0u64, 0u64, 0u64);
        let bom = |slot: &[u8]| -> bool {
            slot[0] != STATUS_ATIVO
                || crc32(&slot[SLOT_CAB..]) == Campos(slot).u32(4)
        };
        for rowid in 1..=self.slot_count {
            let (volume, offset) = self.localizar(rowid);
            let mut principal = vec![0u8; self.slot_size];
            let mut copia = vec![0u8; self.slot_size];
            if self.volumes.ler(volume, offset, &mut principal).is_err() {
                perdidos += 1;
                continue;
            }
            conferidos += 1;
            let copia_ok = self
                .volumes
                .ler_do_espelho(volume, offset, &mut copia)
                .is_ok()
                && bom(&copia);
            match (bom(&principal), copia_ok) {
                (true, true) => {}
                // O principal quebrou e o espelho salvou.
                (false, true) => {
                    self.volumes.escrever(volume, offset, &copia)?;
                    reparados += 1;
                }
                // O espelho quebrou; o principal reescreve o espelho.
                (true, false) => {
                    self.volumes.escrever_no_espelho(volume, offset, &principal)?;
                    reparados += 1;
                }
                (false, false) => perdidos += 1,
            }
        }
        self.volumes.sincronizar()?;
        Ok((conferidos, reparados, perdidos))
    }

    pub fn esquema(&self) -> &Schema {''')
open(p,'w').write(s)

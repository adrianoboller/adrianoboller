# Rewrite the log read/write paths
# 28/08 20:08

import pathlib
p = pathlib.Path("crates/phxsql-store/src/log.rs")
s = p.read_text()

antigo = """    /// Registra um evento com o carimbo do relogio.
    pub fn registrar(&mut self, operacao: Operacao, rowid: RowId, versao: u64) -> Result<Evento> {
        let evento = Evento {
            carimbo: agora_ms(),
            operacao,
            rowid,
            versao,
            usuario: self.usuario,
        };
        self.anexar(evento)?;
        Ok(evento)
    }

    fn anexar(&mut self, evento: Evento) -> Result<()> {
        let paginacao = self.volumes.paginacao();
        let atual = self.cab(self.volume_atual)?;
        let vazio = atual.fim <= CAB_LEN as u64;
        let (volume, virou) =
            paginacao.volume_externo(self.volume_atual, atual.fim, EVENTO_LEN as u64, vazio);
"""
novo = """    /// Registra um evento com o carimbo do relogio, sem imagem.
    pub fn registrar(&mut self, operacao: Operacao, rowid: RowId, versao: u64) -> Result<Evento> {
        self.registrar_com_imagem(operacao, rowid, versao, &[])
    }

    /// Registra um evento levando junto a imagem da linha.
    ///
    /// Imagem vazia grava o evento como sempre foi -- e e o que a exclusao
    /// manda, porque ali o rowid basta.
    pub fn registrar_com_imagem(
        &mut self,
        operacao: Operacao,
        rowid: RowId,
        versao: u64,
        imagem: &[u8],
    ) -> Result<Evento> {
        if imagem.len() as u64 > IMAGEM_MAX as u64 {
            return Err(PhxError::LimiteExcedido(format!(
                "imagem de {} bytes passa do teto de {IMAGEM_MAX} do diario",
                imagem.len()
            )));
        }
        let evento = Evento {
            carimbo: agora_ms(),
            operacao,
            rowid,
            versao,
            usuario: self.usuario,
            tam_imagem: imagem.len() as u32,
        };
        self.anexar(evento, imagem)?;
        Ok(evento)
    }

    fn anexar(&mut self, evento: Evento, imagem: &[u8]) -> Result<()> {
        let paginacao = self.volumes.paginacao();
        let atual = self.cab(self.volume_atual)?;
        let vazio = atual.fim <= CAB_LEN as u64;
        let (volume, virou) =
            paginacao.volume_externo(self.volume_atual, atual.fim, evento.ocupa(), vazio);
"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """        let mut buf = [0u8; EVENTO_LEN];
        evento.escrever(&mut buf);
        self.volumes.escrever(volume, cab.fim, &buf)?;
        self.gravar_cab(Cabecalho {
            volume,
            fim: cab.fim + EVENTO_LEN as u64,
            qtd_eventos: cab.qtd_eventos + 1,
        })
    }"""
novo = """        let mut buf = [0u8; EVENTO_CAB];
        evento.escrever(&mut buf, imagem);
        self.volumes.escrever(volume, cab.fim, &buf)?;
        if !imagem.is_empty() {
            self.volumes
                .escrever(volume, cab.fim + EVENTO_CAB as u64, imagem)?;
        }
        self.gravar_cab(Cabecalho {
            volume,
            fim: cab.fim + evento.ocupa(),
            qtd_eventos: cab.qtd_eventos + 1,
        })
    }"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """    pub fn ler(&mut self, pular: u64, limite: u64) -> Result<Vec<Evento>> {
        let mut saida = Vec::new();
        let mut vistos = 0u64;
        for volume in self.volumes.existentes() {
            let cab = self.cab(volume)?;
            let mut offset = CAB_LEN as u64;
            while offset + EVENTO_LEN as u64 <= cab.fim {
                if vistos >= pular {
                    let mut buf = [0u8; EVENTO_LEN];
                    self.volumes.ler(volume, offset, &mut buf)?;
                    saida.push(Evento::ler(&buf)?);
                    if limite > 0 && saida.len() as u64 >= limite {
                        return Ok(saida);
                    }
                }
                vistos += 1;
                offset += EVENTO_LEN as u64;
            }
        }
        Ok(saida)
    }"""
novo = """    pub fn ler(&mut self, pular: u64, limite: u64) -> Result<Vec<Evento>> {
        Ok(self
            .percorrer(pular, limite, false)?
            .into_iter()
            .map(|(e, _)| e)
            .collect())
    }

    /// O mesmo que [`LogFile::ler`], trazendo a imagem de cada evento.
    ///
    /// E o que a replicacao usa. Eventos gravados sem imagem voltam com o
    /// vetor vazio -- e ai a replica sabe que aquele evento nao da para
    /// aplicar, em vez de aplicar bytes que nao existem.
    pub fn ler_com_imagem(&mut self, pular: u64, limite: u64) -> Result<Vec<(Evento, Vec<u8>)>> {
        self.percorrer(pular, limite, true)
    }

    /// A varredura unica dos dois caminhos.
    ///
    /// Desde que o evento deixou de ter largura fixa, chegar ao evento N e
    /// caminhar pelos anteriores. O que ainda se pula de graca e o VOLUME
    /// inteiro: o `qtd_eventos` do cabecalho diz quantos ele tem, e se todos
    /// eles estao antes do `pular` o arquivo nem se abre.
    fn percorrer(
        &mut self,
        pular: u64,
        limite: u64,
        com_imagem: bool,
    ) -> Result<Vec<(Evento, Vec<u8>)>> {
        let mut saida = Vec::new();
        let mut vistos = 0u64;
        for volume in self.volumes.existentes() {
            let cab = self.cab(volume)?;
            if vistos + cab.qtd_eventos <= pular {
                vistos += cab.qtd_eventos;
                continue;
            }
            let mut offset = CAB_LEN as u64;
            while offset + EVENTO_CAB as u64 <= cab.fim {
                let mut buf = [0u8; EVENTO_CAB];
                self.volumes.ler(volume, offset, &mut buf)?;
                let evento = Evento::ler(&buf)?;
                if vistos >= pular {
                    let mut imagem = Vec::new();
                    if evento.tam_imagem > 0 {
                        imagem = vec![0u8; evento.tam_imagem as usize];
                        self.volumes
                            .ler(volume, offset + EVENTO_CAB as u64, &mut imagem)?;
                        evento.conferir(&buf, &imagem)?;
                        if !com_imagem {
                            imagem.clear();
                        }
                    }
                    saida.push((evento, imagem));
                    if limite > 0 && saida.len() as u64 >= limite {
                        return Ok(saida);
                    }
                }
                vistos += 1;
                offset += evento.ocupa();
            }
        }
        Ok(saida)
    }"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """            let mut offset = CAB_LEN as u64;
            let mut no_volume = 0u64;
            while offset + EVENTO_LEN as u64 <= cab.fim {
                let mut buf = [0u8; EVENTO_LEN];
                self.volumes.ler(volume, offset, &mut buf)?;
                Evento::ler(&buf)?; // confere CRC e operacao
                no_volume += 1;
                offset += EVENTO_LEN as u64;
            }"""
novo = """            let mut offset = CAB_LEN as u64;
            let mut no_volume = 0u64;
            while offset + EVENTO_CAB as u64 <= cab.fim {
                let mut buf = [0u8; EVENTO_CAB];
                self.volumes.ler(volume, offset, &mut buf)?;
                let evento = Evento::ler(&buf)?; // confere a operacao, e o CRC se nao ha imagem
                if evento.tam_imagem > 0 {
                    // Com imagem o CRC so fecha depois de le-la. Conferir so o
                    // cabecalho aqui deixaria de fora justamente os bytes que
                    // a replica grava como dado.
                    let mut imagem = vec![0u8; evento.tam_imagem as usize];
                    self.volumes
                        .ler(volume, offset + EVENTO_CAB as u64, &mut imagem)?;
                    evento.conferir(&buf, &imagem)?;
                }
                no_volume += 1;
                offset += evento.ocupa();
            }"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")

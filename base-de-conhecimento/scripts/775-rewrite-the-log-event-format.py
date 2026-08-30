# Rewrite the log event format
# 28/08 20:07

import pathlib
p = pathlib.Path("crates/phxsql-store/src/log.rs")
s = p.read_text()

antigo = """//! # Evento (36 bytes)
//!
//! ```text
//! [carimbo i64 ms][operacao u8][flags u8][res u16]
//! [rowid u64][versao u64][usuario u32][crc32 u32]
//! ```
//!
//! O carimbo e em milissegundos desde 1970-01-01T00:00:00Z, o que da
//! resolucao suficiente para ordenar operacoes dentro do mesmo segundo.
//!
//! Como o `.log` cresce para sempre, ele tambem e paginado em
//! `Tabela_001.log`, `Tabela_002.log`, ... pelo tamanho de volume do esquema."""
novo = """//! # Evento: 44 bytes de cabecalho, e talvez um corpo
//!
//! ```text
//! [carimbo i64 ms][operacao u8][flags u8][res u16]
//! [rowid u64][versao u64][usuario u32]
//! [tam_imagem u32][crc32 u32][res u32]
//! [imagem ... tam_imagem bytes]
//! ```
//!
//! O carimbo e em milissegundos desde 1970-01-01T00:00:00Z, o que da
//! resolucao suficiente para ordenar operacoes dentro do mesmo segundo.
//!
//! # A imagem da linha, e por que ela e opcional
//!
//! Sem imagem o evento diz que o rowid 42 mudou; nao diz PARA QUE. Isso basta
//! para auditoria e nao basta para replicar -- uma replica precisa dos bytes.
//!
//! Com a imagem, um registro de 200 bytes gasta ~244 bytes de diario por
//! alteracao em vez de 36. E caro para quem so quer auditoria, e por isso o
//! interruptor esta no `config.json`: `replicacao.imagem_da_linha`.
//!
//! A imagem NAO e o texto do registro -- e o payload cru do `.reg`, os mesmos
//! bytes que a replica vai gravar, mais o CONTEUDO dos externos. Os ponteiros
//! do `.bin` e do `.memo` sao offsets locais e nao valem na outra maquina; e a
//! mesma razao de o `.trash` guardar conteudo e nao ponteiro.
//!
//! Exclusao nao leva imagem: o rowid basta.
//!
//! # O preco de o evento deixar de ter largura fixa
//!
//! Ate a versao 1 o evento N morava no offset `CAB_LEN + N x 36`, e pular era
//! uma conta. Agora nao e: para chegar ao evento N e preciso caminhar pelos
//! anteriores lendo o tamanho de cada um. O `qtd_eventos` de cada volume no
//! cabecalho e o que salva a leitura -- um volume inteiro se pula sem abrir.
//!
//! Como o `.log` cresce para sempre, ele tambem e paginado em
//! `Tabela_001.log`, `Tabela_002.log`, ... pelo tamanho de volume do esquema."""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """const CAB_LEN: usize = 64;
/// Bytes de cada evento.
pub const EVENTO_LEN: usize = 36;
const VERSAO: u16 = 1;"""
novo = """const CAB_LEN: usize = 64;
/// Bytes do CABECALHO de cada evento. O corpo vem depois, se houver.
pub const EVENTO_CAB: usize = 44;
/// Teto da imagem de uma linha, para um tamanho corrompido nao pedir 4 GiB.
///
/// Uma linha com anexos grandes pode passar disto; ai o evento vai sem imagem
/// e a replica busca a linha pelo `ler`. Perder a replicacao de uma linha
/// gigante e melhor que abrir espaco para um `tam_imagem` inventado alocar a
/// memoria toda da maquina.
pub const IMAGEM_MAX: u32 = 64 * 1024 * 1024;
/// Bit 0 do byte de flags: este evento tem imagem.
const FLAG_IMAGEM: u8 = 1;
const VERSAO: u16 = 2;"""
assert antigo in s
s = s.replace(antigo, novo)

# --- Evento: campos novos
antigo = """    /// Identificacao de quem fez. Zero = nao informado.
    pub usuario: u32,
}"""
novo = """    /// Identificacao de quem fez. Zero = nao informado.
    pub usuario: u32,
    /// Bytes da imagem que vem depois deste cabecalho. Zero = sem imagem.
    pub tam_imagem: u32,
}"""
assert antigo in s
s = s.replace(antigo, novo)

antigo = """    fn escrever(&self, dst: &mut [u8; EVENTO_LEN]) {
        dst.fill(0);
        por_i64(dst, 0, self.carimbo);
        dst[8] = self.operacao.tag();
        por_u64(dst, 12, self.rowid);
        por_u64(dst, 20, self.versao);
        por_u32(dst, 28, self.usuario);
        let crc = crc32(&dst[..32]);
        por_u32(dst, 32, crc);
    }

    fn ler(src: &[u8]) -> Result<Evento> {
        if src.len() < EVENTO_LEN {
            return Err(PhxError::Corrompido("evento de log truncado".into()));
        }
        let c = Campos(src);
        if crc32(&src[..32]) != c.u32(32) {
            return Err(PhxError::Corrompido(
                "evento de log com CRC invalido".into(),
            ));
        }
        Ok(Evento {
            carimbo: c.u64(0) as i64,
            operacao: Operacao::de_tag(src[8])?,
            rowid: c.u64(12),
            versao: c.u64(20),
            usuario: c.u32(28),
        })
    }
}"""
novo = """    /// O evento ocupa isto no arquivo, cabecalho mais corpo.
    pub fn ocupa(&self) -> u64 {
        EVENTO_CAB as u64 + self.tam_imagem as u64
    }

    /// O CRC cobre o cabecalho E a imagem.
    ///
    /// Cobrir so o cabecalho deixaria a imagem sem conferencia -- e a imagem e
    /// justamente o que a replica vai gravar como dado. Um byte trocado ali
    /// entraria na replica sem ninguem notar.
    fn escrever(&self, dst: &mut [u8; EVENTO_CAB], imagem: &[u8]) {
        dst.fill(0);
        por_i64(dst, 0, self.carimbo);
        dst[8] = self.operacao.tag();
        dst[9] = if imagem.is_empty() { 0 } else { FLAG_IMAGEM };
        por_u64(dst, 12, self.rowid);
        por_u64(dst, 20, self.versao);
        por_u32(dst, 28, self.usuario);
        por_u32(dst, 32, imagem.len() as u32);
        let mut crc = crc32(&dst[..36]);
        if !imagem.is_empty() {
            crc ^= crc32(imagem);
        }
        por_u32(dst, 36, crc);
    }

    /// Le o cabecalho. `imagem` e `None` quando quem chama ainda nao a leu --
    /// e ai o CRC so pode ser conferido depois, com [`Evento::conferir`].
    fn ler(src: &[u8]) -> Result<Evento> {
        if src.len() < EVENTO_CAB {
            return Err(PhxError::Corrompido("evento de log truncado".into()));
        }
        let c = Campos(src);
        let tam_imagem = c.u32(32);
        if tam_imagem > IMAGEM_MAX {
            return Err(PhxError::Corrompido(format!(
                "evento de log diz ter imagem de {tam_imagem} bytes, acima do teto"
            )));
        }
        let evento = Evento {
            carimbo: c.u64(0) as i64,
            operacao: Operacao::de_tag(src[8])?,
            rowid: c.u64(12),
            versao: c.u64(20),
            usuario: c.u32(28),
            tam_imagem,
        };
        if tam_imagem == 0 {
            evento.conferir(src, &[])?;
        }
        Ok(evento)
    }

    /// Confere o CRC do par cabecalho + imagem.
    fn conferir(&self, cab: &[u8], imagem: &[u8]) -> Result<()> {
        let mut crc = crc32(&cab[..36]);
        if !imagem.is_empty() {
            crc ^= crc32(imagem);
        }
        if crc != Campos(cab).u32(36) {
            return Err(PhxError::Corrompido(
                "evento de log com CRC invalido".into(),
            ));
        }
        Ok(())
    }
}"""
assert antigo in s
s = s.replace(antigo, novo)
p.write_text(s)
print("ok")

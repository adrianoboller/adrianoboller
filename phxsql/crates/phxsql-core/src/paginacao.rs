//! Paginacao de tabelas grandes.
//!
//! Uma tabela grande se parte em volumes numerados:
//!
//! ```text
//! cadastroClientes_001.reg
//! cadastroClientes_002.reg
//! cadastroClientes_003.reg
//! ```
//!
//! A quantidade de registros por arquivo e a quantidade de arquivos sao
//! definidas na criacao da tabela e ficam gravadas no esquema.
//!
//! # A propriedade que faz isso sair barato
//!
//! O endereco de um registro continua saindo de uma CONTA, nao de uma busca:
//!
//! ```text
//! volume = (rowid - 1) / registros_por_arquivo + 1
//! slot   = (rowid - 1) % registros_por_arquivo + 1
//! ```
//!
//! Com isso:
//!
//! * o `rowid` continua GLOBAL e imutavel -- ele nao e "posicao no volume",
//!   e posicao na tabela, e o volume sai dele por divisao;
//! * a ordem de digitacao continua garantida, porque o volume N+1 vem sempre
//!   depois do N;
//! * o `.ndx` NAO MUDA EM NADA. Ele ja guarda rowid, e nenhuma linha do codigo
//!   de indice precisa saber que volume existe.

use crate::error::{PhxError, Result};
use crate::RowId;

/// Bytes por volume adotados por padrao nos arquivos externos (1 GiB).
pub const BYTES_POR_ARQUIVO_PADRAO: u64 = 1024 * 1024 * 1024;

/// Largura padrao do sufixo numerico (`_001`).
pub const DIGITOS_PADRAO: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paginacao {
    /// Registros em cada volume do `.reg`. Zero = sem paginacao.
    pub registros_por_arquivo: u64,
    /// Teto de volumes da tabela.
    pub max_arquivos: u32,
    /// Largura do sufixo numerico. Zero = arquivo unico, sem sufixo.
    pub digitos: u8,
    /// Bytes por volume nos arquivos externos (`.bin`, `.memo`, `.log`).
    pub bytes_por_arquivo: u64,
}

impl Paginacao {
    /// Tabela em arquivo unico: `cadastroClientes.reg`, sem sufixo.
    pub const DESLIGADA: Paginacao = Paginacao {
        registros_por_arquivo: 0,
        max_arquivos: 0,
        digitos: 0,
        bytes_por_arquivo: 0,
    };

    /// Paginacao com os dois numeros que o `CREATE TABLE` define.
    pub fn nova(registros_por_arquivo: u64, max_arquivos: u32) -> Result<Paginacao> {
        Paginacao {
            registros_por_arquivo,
            max_arquivos,
            digitos: DIGITOS_PADRAO,
            bytes_por_arquivo: BYTES_POR_ARQUIVO_PADRAO,
        }
        .validada()
    }

    /// Muda a largura do sufixo (por exemplo 4, para passar de 999 volumes).
    pub fn com_digitos(mut self, digitos: u8) -> Result<Paginacao> {
        self.digitos = digitos;
        self.validada()
    }

    /// Muda o tamanho de cada volume dos arquivos externos.
    pub fn com_bytes_por_arquivo(mut self, bytes: u64) -> Result<Paginacao> {
        self.bytes_por_arquivo = bytes;
        self.validada()
    }

    fn validada(self) -> Result<Paginacao> {
        if self.registros_por_arquivo == 0 {
            return Err(PhxError::Esquema(
                "registros_por_arquivo precisa ser maior que zero".into(),
            ));
        }
        if self.max_arquivos == 0 {
            return Err(PhxError::Esquema(
                "max_arquivos precisa ser maior que zero".into(),
            ));
        }
        if self.digitos == 0 || self.digitos > 9 {
            return Err(PhxError::Esquema(format!(
                "digitos precisa estar entre 1 e 9, recebido {}",
                self.digitos
            )));
        }
        let teto = 10u64.pow(self.digitos as u32) - 1;
        if self.max_arquivos as u64 > teto {
            return Err(PhxError::Esquema(format!(
                "max_arquivos {} nao cabe em {} digitos (maximo {teto})",
                self.max_arquivos, self.digitos
            )));
        }
        if self.bytes_por_arquivo == 0 {
            return Err(PhxError::Esquema(
                "bytes_por_arquivo precisa ser maior que zero".into(),
            ));
        }
        Ok(self)
    }

    pub fn ligada(&self) -> bool {
        self.digitos > 0 && self.registros_por_arquivo > 0
    }

    /// Volume e slot de um rowid. Sem paginacao, o volume e sempre 1 e o slot
    /// e o proprio rowid.
    pub fn localizar(&self, rowid: RowId) -> (u32, u64) {
        if !self.ligada() {
            return (1, rowid);
        }
        let base = rowid - 1;
        (
            (base / self.registros_por_arquivo) as u32 + 1,
            base % self.registros_por_arquivo + 1,
        )
    }

    /// Quantos registros a tabela comporta.
    pub fn capacidade(&self) -> u64 {
        if !self.ligada() {
            return u64::MAX;
        }
        self.registros_por_arquivo
            .saturating_mul(self.max_arquivos as u64)
    }

    /// Um rowid ainda cabe na tabela?
    pub fn cabe(&self, rowid: RowId) -> bool {
        rowid >= 1 && rowid <= self.capacidade()
    }

    /// Sufixo do nome do arquivo: `_001` com paginacao, vazio sem.
    pub fn sufixo(&self, volume: u32) -> String {
        if !self.ligada() {
            String::new()
        } else {
            format!("_{:0largura$}", volume, largura = self.digitos as usize)
        }
    }

    /// Volume que deve receber o proximo bloco externo, dado quanto ja foi
    /// escrito no volume atual e o tamanho do bloco novo.
    ///
    /// Um bloco nunca e partido entre volumes: se nao couber no volume atual,
    /// vai inteiro para o proximo.
    ///
    /// `volume_vazio` diz se o volume atual ainda nao tem nenhum bloco. Um
    /// bloco maior que `bytes_por_arquivo` fica sozinho no seu volume, em vez
    /// de ser recusado -- caso contrario uma foto de 2 MB seria impossivel de
    /// gravar num arquivo de 1 MB, e trocar de volume nao resolveria nada.
    pub fn volume_externo(
        &self,
        volume_atual: u32,
        usado: u64,
        novo: u64,
        volume_vazio: bool,
    ) -> (u32, bool) {
        if !self.ligada() || volume_vazio || usado + novo <= self.bytes_por_arquivo {
            (volume_atual, false)
        } else {
            (volume_atual + 1, true)
        }
    }
}

impl Default for Paginacao {
    fn default() -> Self {
        Paginacao::DESLIGADA
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desligada_mantem_arquivo_unico() {
        let p = Paginacao::DESLIGADA;
        assert!(!p.ligada());
        assert_eq!(p.localizar(1), (1, 1));
        assert_eq!(p.localizar(999_999), (1, 999_999));
        assert_eq!(p.sufixo(1), "");
        assert_eq!(p.capacidade(), u64::MAX);
    }

    #[test]
    fn fronteiras_de_volume() {
        let p = Paginacao::nova(1_000, 999).unwrap();
        assert_eq!(p.localizar(1), (1, 1));
        assert_eq!(p.localizar(1_000), (1, 1_000));
        assert_eq!(p.localizar(1_001), (2, 1));
        assert_eq!(p.localizar(2_000), (2, 1_000));
        assert_eq!(p.localizar(2_001), (3, 1));
    }

    #[test]
    fn o_rowid_e_global_e_a_ordem_de_digitacao_se_mantem() {
        let p = Paginacao::nova(10, 99).unwrap();
        // Percorrer rowids em ordem crescente percorre volumes em ordem
        // crescente, e dentro de cada volume os slots em ordem crescente.
        let mut ultimo = (0u32, 0u64);
        for rowid in 1..=100u64 {
            let atual = p.localizar(rowid);
            assert!(atual > ultimo, "rowid {rowid} quebrou a ordem");
            ultimo = atual;
        }
    }

    #[test]
    fn sufixo_e_zero_a_esquerda() {
        let p = Paginacao::nova(100, 999).unwrap();
        assert_eq!(p.sufixo(1), "_001");
        assert_eq!(p.sufixo(42), "_042");
        assert_eq!(p.sufixo(999), "_999");
        let q = p.com_digitos(4).unwrap();
        assert_eq!(q.sufixo(7), "_0007");
    }

    #[test]
    fn capacidade_e_tabela_cheia() {
        let p = Paginacao::nova(1_000, 3).unwrap();
        assert_eq!(p.capacidade(), 3_000);
        assert!(p.cabe(3_000));
        assert!(!p.cabe(3_001));
        assert!(!p.cabe(0));
    }

    #[test]
    fn max_arquivos_precisa_caber_nos_digitos() {
        assert!(
            Paginacao::nova(100, 1_000).is_err(),
            "1000 nao cabe em 3 digitos"
        );
        assert!(Paginacao::nova(100, 999).is_ok());
        assert!(Paginacao::nova(100, 1_000).is_err());
        assert!(Paginacao::nova(100, 999).unwrap().com_digitos(4).is_ok());
        assert!(Paginacao::nova(0, 10).is_err());
        assert!(Paginacao::nova(10, 0).is_err());
    }

    #[test]
    fn bloco_externo_nao_e_partido_entre_volumes() {
        let p = Paginacao::nova(1_000, 999)
            .unwrap()
            .com_bytes_por_arquivo(1_000)
            .unwrap();
        // Cabe: fica no volume atual.
        assert_eq!(p.volume_externo(1, 900, 100, false), (1, false));
        // Nao cabe por um byte: vai inteiro para o proximo.
        assert_eq!(p.volume_externo(1, 900, 101, false), (2, true));
        // Sem paginacao nunca troca de volume.
        assert_eq!(
            Paginacao::DESLIGADA.volume_externo(1, u64::MAX / 2, 1_000, false),
            (1, false)
        );
    }

    #[test]
    fn bloco_maior_que_o_volume_fica_sozinho_em_vez_de_ser_recusado() {
        let p = Paginacao::nova(1_000, 999)
            .unwrap()
            .com_bytes_por_arquivo(1_000)
            .unwrap();
        // Volume ainda vazio e o bloco e maior que o volume inteiro:
        // grava assim mesmo, senao nunca caberia em lugar nenhum.
        assert_eq!(p.volume_externo(5, 64, 50_000, true), (5, false));
        // Com o volume ja ocupado, ele rola para o proximo.
        assert_eq!(p.volume_externo(5, 500, 50_000, false), (6, true));
    }
}

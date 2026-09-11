//! O TIPO de um database. Decisao do dono, 11/09/2026: o PhxSql passa a ter
//! TRES tipos de database, cada um com um motor de armazenamento proprio.
//!
//! - **Padrao**: o modelo de arquivos separados do HFSQL -- tabelas
//!   relacionais, indices, integridade referencial, transacoes. E o unico que
//!   existia, e o **default de todo database sem marca**: banco que ja existe
//!   nasceu padrao e continua padrao, sem migracao.
//! - **Hive (colmeia)**: armazem hierarquico chave->valor, mapeado em memoria,
//!   inspirado no REGF do Registro do Windows. Bom para configuracao. Desenho
//!   em `docs/propostas/colmeia.md`. **MOTOR EM CONSTRUCAO.**
//! - **Vetorial**: armazem de vetores (embeddings) com busca por similaridade.
//!   Bom para IA/semantica. **MOTOR EM CONSTRUCAO.**
//!
//! Este modulo e' so o TIPO: o enum e a conversao de/para texto estavel. O
//! MOTOR de cada tipo e' outra coisa; a infraestrutura roteia por este enum, e
//! enquanto o motor de hive/vetorial nao existe, quem rotear por aqui diz "em
//! construcao" -- nunca finge que gravou (metade pior que nada).

use crate::error::{PhxError, Result};

/// O tipo de armazenamento de um database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TipoDatabase {
    /// Relacional, arquivos separados -- o motor que sempre existiu.
    #[default]
    Padrao,
    /// Colmeia hierarquica chave->valor (inspirada no REGF). Motor em obra.
    Hive,
    /// Vetores/embeddings com busca por similaridade. Motor em obra.
    Vetorial,
}

impl TipoDatabase {
    /// O texto que vai no marcador do database e no protocolo. **Estavel** --
    /// e formato em disco; mudar um destes nomes renomeia o que ja foi gravado.
    pub fn como_texto(self) -> &'static str {
        match self {
            TipoDatabase::Padrao => "padrao",
            TipoDatabase::Hive => "hive",
            TipoDatabase::Vetorial => "vetorial",
        }
    }

    /// Le o tipo de um texto. **Ausencia/vazio -> Padrao** (compatibilidade:
    /// quem nunca declarou tipo e' padrao). Texto desconhecido e' **ERRO**, nao
    /// um palpite -- um tipo que o motor nao conhece nao pode nascer, senao a
    /// infraestrutura rotearia para lugar nenhum.
    pub fn de_texto(s: &str) -> Result<TipoDatabase> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "padrao" | "padrão" | "standard" => Ok(TipoDatabase::Padrao),
            "hive" | "colmeia" => Ok(TipoDatabase::Hive),
            "vetorial" | "vector" => Ok(TipoDatabase::Vetorial),
            outro => Err(PhxError::Esquema(format!(
                "tipo de database desconhecido: {outro:?} -- use padrao, hive ou vetorial"
            ))),
        }
    }

    /// O MOTOR deste tipo ja esta implementado? So o padrao, por enquanto. A
    /// infraestrutura existe para os tres; o motor de hive e vetorial e' frente
    /// aberta. Quem rotear por aqui usa isto para dizer "em construcao".
    pub fn motor_pronto(self) -> bool {
        matches!(self, TipoDatabase::Padrao)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn de_texto_conhece_os_tres_e_o_vazio() {
        assert_eq!(TipoDatabase::de_texto("").unwrap(), TipoDatabase::Padrao);
        assert_eq!(
            TipoDatabase::de_texto("padrao").unwrap(),
            TipoDatabase::Padrao
        );
        assert_eq!(TipoDatabase::de_texto("HIVE").unwrap(), TipoDatabase::Hive);
        assert_eq!(
            TipoDatabase::de_texto("colmeia").unwrap(),
            TipoDatabase::Hive
        );
        assert_eq!(
            TipoDatabase::de_texto(" vetorial ").unwrap(),
            TipoDatabase::Vetorial
        );
    }

    #[test]
    fn tipo_desconhecido_e_erro_nao_palpite() {
        assert!(TipoDatabase::de_texto("grafo").is_err());
    }

    #[test]
    fn texto_e_volta_sao_estaveis() {
        for t in [
            TipoDatabase::Padrao,
            TipoDatabase::Hive,
            TipoDatabase::Vetorial,
        ] {
            assert_eq!(TipoDatabase::de_texto(t.como_texto()).unwrap(), t);
        }
    }

    #[test]
    fn so_o_padrao_tem_motor_pronto_hoje() {
        assert!(TipoDatabase::Padrao.motor_pronto());
        assert!(!TipoDatabase::Hive.motor_pronto());
        assert!(!TipoDatabase::Vetorial.motor_pronto());
    }

    #[test]
    fn o_default_e_padrao() {
        assert_eq!(TipoDatabase::default(), TipoDatabase::Padrao);
    }
}

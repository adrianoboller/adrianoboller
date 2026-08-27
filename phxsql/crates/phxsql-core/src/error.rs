//! Erros do PhxSql.

use std::fmt;

/// Resultado padrao do PhxSql.
pub type Result<T> = std::result::Result<T, PhxError>;

#[derive(Debug)]
pub enum PhxError {
    /// Falha de entrada/saida no sistema de arquivos.
    Io(std::io::Error),
    /// Assinatura ("magic") do arquivo nao confere com a esperada.
    BadMagic {
        arquivo: String,
        esperado: &'static [u8; 8],
        encontrado: [u8; 8],
    },
    /// Versao de formato nao suportada por esta build.
    VersaoNaoSuportada {
        arquivo: String,
        encontrada: u16,
        suportada: u16,
    },
    /// Estrutura interna inconsistente (CRC, offset fora do arquivo, etc).
    Corrompido(String),
    /// Esquema invalido ou incompativel com o arquivo aberto.
    Esquema(String),
    /// Valor incompativel com o tipo declarado da coluna.
    Tipo(String),
    /// Registro, indice ou chave inexistente.
    NaoEncontrado(String),
    /// Violacao de indice unico.
    Duplicado(String),
    /// Credencial invalida ou poder insuficiente.
    Autorizacao(String),
    /// Valor excede o limite fisico do formato.
    LimiteExcedido(String),
}

impl fmt::Display for PhxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PhxError::Io(e) => write!(f, "erro de E/S: {e}"),
            PhxError::BadMagic {
                arquivo,
                esperado,
                encontrado,
            } => write!(
                f,
                "assinatura invalida em {arquivo}: esperado {:?}, encontrado {:?}",
                String::from_utf8_lossy(esperado.as_slice()),
                String::from_utf8_lossy(encontrado.as_slice())
            ),
            PhxError::VersaoNaoSuportada {
                arquivo,
                encontrada,
                suportada,
            } => write!(
                f,
                "versao de formato {encontrada} nao suportada em {arquivo} (esta build le ate {suportada})"
            ),
            PhxError::Corrompido(m) => write!(f, "arquivo corrompido: {m}"),
            PhxError::Esquema(m) => write!(f, "esquema invalido: {m}"),
            PhxError::Tipo(m) => write!(f, "tipo invalido: {m}"),
            PhxError::NaoEncontrado(m) => write!(f, "nao encontrado: {m}"),
            PhxError::Duplicado(m) => write!(f, "chave duplicada: {m}"),
            PhxError::Autorizacao(m) => write!(f, "acesso negado: {m}"),
            PhxError::LimiteExcedido(m) => write!(f, "limite excedido: {m}"),
        }
    }
}

impl std::error::Error for PhxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PhxError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PhxError {
    fn from(e: std::io::Error) -> Self {
        PhxError::Io(e)
    }
}

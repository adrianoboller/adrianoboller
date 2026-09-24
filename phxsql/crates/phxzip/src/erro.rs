//! Os erros do PhxZip, cada um com nome.
//!
//! Erro com nome e o que deixa quem chama decidir: a tela diz «senha errada»
//! em vez de «falhou», e o terminal devolve codigo de saida diferente para
//! arquivo corrompido e para metodo recusado.

use core::fmt;

/// Resultado do PhxZip.
pub type Resultado<T> = core::result::Result<T, Erro>;

/// Tudo o que pode dar errado ao ler ou gravar um 7z.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Erro {
    /// Nao comeca com a assinatura do 7z.
    NaoE7z,
    /// Versao maior do formato que esta leitura nao conhece.
    VersaoDesconhecida(u8, u8),
    /// Bytes que nao fecham com o formato -- o `onde` diz o que se lia.
    Corrompido(&'static str),
    /// CRC-32 gravado nao bate com o calculado.
    CrcNaoBate(&'static str),
    /// Metodo que o PhxZip decidiu nao implementar (pedido 454). O nome e o
    /// do 7-Zip, para quem le saber o que refazer: `7z a -m0=lzma2`.
    MetodoRecusado {
        /// Identificador do metodo no 7z.
        id: u64,
        /// Nome conhecido, ou «desconhecido».
        nome: &'static str,
    },
    /// Recurso do formato que existe mas nao e suportado aqui (dito qual).
    NaoSuportado(&'static str),
    /// O cabecalho ou os dados estao cifrados e nenhuma senha veio.
    SenhaNecessaria,
    /// Havia senha, e com ela o dado nao fecha: ou a senha esta errada, ou o
    /// arquivo esta corrompido. O AES-CBC do 7z nao tem etiqueta de
    /// autenticacao, entao os dois casos nao se distinguem -- e dizer so
    /// «senha errada» seria mentir num deles.
    SenhaErradaOuCorrompido,
    /// Um teto de `Limites` foi atingido (nomeado): protege contra bomba.
    Teto(&'static str),
    /// Nome de entrada que sairia da pasta de destino, ou invalido.
    CaminhoInseguro,
    /// Tamanho que nao cabe no `usize` deste alvo (32 bits).
    GrandeDemaisParaEsteAlvo,
    /// Uso errado da API (dito como).
    Uso(&'static str),
}

impl fmt::Display for Erro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Erro::NaoE7z => write!(f, "nao e um arquivo 7z (assinatura ausente)"),
            Erro::VersaoDesconhecida(a, b) => {
                write!(f, "versao {a}.{b} do formato 7z nao suportada")
            }
            Erro::Corrompido(onde) => write!(f, "arquivo corrompido: {onde}"),
            Erro::CrcNaoBate(onde) => write!(f, "CRC nao bate: {onde}"),
            Erro::MetodoRecusado { id, nome } => {
                write!(f, "metodo {nome} (id {id:X}) recusado: o PhxZip le e grava so Copy, LZMA, LZMA2 e 7zAES")
            }
            Erro::NaoSuportado(o) => write!(f, "nao suportado: {o}"),
            Erro::SenhaNecessaria => write!(f, "arquivo cifrado: informe a senha"),
            Erro::SenhaErradaOuCorrompido => write!(f, "senha errada ou arquivo corrompido"),
            Erro::Teto(o) => write!(f, "teto atingido: {o}"),
            Erro::CaminhoInseguro => write!(
                f,
                "nome de entrada inseguro (absoluto, com '..' ou invalido)"
            ),
            Erro::GrandeDemaisParaEsteAlvo => {
                write!(f, "tamanho nao cabe na memoria enderecavel deste alvo")
            }
            Erro::Uso(o) => write!(f, "uso invalido: {o}"),
        }
    }
}

/// Converte um tamanho do arquivo (sempre `u64` no 7z) para `usize`, com erro
/// nomeado no alvo de 32 bits em vez de truncar calado.
pub(crate) fn para_usize(v: u64) -> Resultado<usize> {
    usize::try_from(v).map_err(|_| Erro::GrandeDemaisParaEsteAlvo)
}

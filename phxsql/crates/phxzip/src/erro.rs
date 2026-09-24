//! Os erros do PhxZip, um por pergunta que quem recebe precisa responder.
//!
//! # Por que tantos
//!
//! Quem abre um arquivo que recusou precisa saber O QUE fazer, e cada variante
//! pede uma coisa diferente: senha errada pede a senha certa; arquivo
//! corrompido pede o backup; metodo legado pede regravar com o 7-Zip atual;
//! nome perigoso pede NAO extrair. Um erro so, com texto, obrigaria a web e o
//! terminal a adivinhar pela frase -- e texto se resolve por chave, nunca por
//! comparacao da frase (licao da fabrica de idiomas).
//!
//! # Senha errada ou arquivo corrompido: quando o formato nao separa
//!
//! O 7zAES e AES-CBC sem autenticacao: uma chave errada nao e recusada pela
//! cifra, ela so produz lixo -- e lixo e exatamente o que um byte trocado no
//! dado cifrado tambem produz. O que separa os dois e um CRC do dado CIFRADO:
//! se ele bate, o que esta gravado chegou intacto e o lixo so pode ser a senha.
//! O escritor do PhxZip grava esse CRC (o formato preve, `kCRC` no `PackInfo`);
//! o 7-Zip nao grava. Por isso ha DUAS variantes: [`Erro::SenhaErrada`] quando
//! o arquivo permite afirmar, e [`Erro::SenhaErradaOuCorrompido`] quando nao --
//! em vez de afirmar o que nao se sabe.

use alloc::string::String;
use core::fmt;

/// Por que o arquivo foi recusado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Erro {
    /// Nao comeca com a assinatura do 7z.
    NaoE7z,
    /// A versao maior do formato nao e 0.
    VersaoNaoSuportada(u8),
    /// A estrutura nao fecha: cabecalho cortado, tamanho que aponta para fora
    /// do arquivo, campo fora da faixa. O texto diz onde.
    Estrutura(String),
    /// Um CRC que a senha nao alcanca nao bateu: o arquivo mudou depois de
    /// gravado.
    Corrompido(String),
    /// O dado cifrado chegou intacto (o CRC do que esta gravado bateu) e o que
    /// saiu da decifracao nao fecha: so pode ser a senha.
    SenhaErrada,
    /// A decifracao nao fechou e o arquivo nao guarda o CRC do dado cifrado --
    /// e o que o 7-Zip grava --, entao o formato nao separa senha errada de
    /// byte trocado.
    SenhaErradaOuCorrompido,
    /// Ha coisa cifrada e nao veio senha.
    SenhaAusente,
    /// Metodo que existe no 7z e o PhxZip recusa de proposito, por ser
    /// legado (decisao do dono, 24/09/2026: so Copy, LZMA, LZMA2 e 7zAES).
    MetodoLegado(String),
    /// Identificador de metodo que nem consta da lista do 7-Zip.
    MetodoDesconhecido(String),
    /// Nome de entrada que, extraido, escreveria fora do destino (caminho
    /// absoluto, `..`, letra de unidade, NUL) -- o «zip-slip».
    NomePerigoso(String),
    /// O tamanho declarado passa do teto de quem chamou.
    GrandeDemais {
        /// O que foi medido: `"entrada"`, `"bloco"` ou `"cabecalho"`.
        oque: &'static str,
        declarado: u64,
        teto: u64,
    },
    /// Um tamanho do arquivo nao cabe no endereco desta plataforma (alvo de
    /// 32 bits com arquivo acima de 4 GiB). Recusado em vez de truncado.
    NaoCabe { oque: &'static str, valor: u64 },
    /// A derivacao de chave pede mais rodadas do que o teto (`2^ciclos`).
    CiclosDemais { pedidos: u8, teto: u8 },
    /// Indice de entrada que nao existe.
    EntradaInexistente(usize),
    /// `.phz`: o arquivo nao tem entrada nenhuma.
    SemEntrada,
    /// `.phz`: o arquivo tem mais de uma entrada.
    MaisDeUmaEntrada(u64),
    /// `.phz`: nada passou pelo 7zAES -- e 7z, mas nao e um `.phz` protegido.
    SemCifra,
    /// `.phz`: a unica entrada e uma pasta.
    EntradaEPasta(String),
    /// O mesmo nome duas vezes no que se esta gravando.
    NomeRepetido(String),
}

impl Erro {
    /// O nome estavel do erro, para quem precisa decidir por codigo -- a web, o
    /// terminal, a fabrica de idiomas. A frase do `Display` pode melhorar de
    /// redacao; este nome nao muda.
    pub fn nome(&self) -> &'static str {
        match self {
            Erro::NaoE7z => "NAO_E_7Z",
            Erro::VersaoNaoSuportada(_) => "VERSAO_NAO_SUPORTADA",
            Erro::Estrutura(_) => "ESTRUTURA",
            Erro::Corrompido(_) => "CORROMPIDO",
            Erro::SenhaErrada => "SENHA_ERRADA",
            Erro::SenhaErradaOuCorrompido => "SENHA_ERRADA_OU_CORROMPIDO",
            Erro::SenhaAusente => "SENHA_AUSENTE",
            Erro::MetodoLegado(_) => "METODO_LEGADO",
            Erro::MetodoDesconhecido(_) => "METODO_DESCONHECIDO",
            Erro::NomePerigoso(_) => "NOME_PERIGOSO",
            Erro::GrandeDemais { .. } => "GRANDE_DEMAIS",
            Erro::NaoCabe { .. } => "NAO_CABE",
            Erro::CiclosDemais { .. } => "CICLOS_DEMAIS",
            Erro::EntradaInexistente(_) => "ENTRADA_INEXISTENTE",
            Erro::SemEntrada => "SEM_ENTRADA",
            Erro::MaisDeUmaEntrada(_) => "MAIS_DE_UMA_ENTRADA",
            Erro::SemCifra => "SEM_CIFRA",
            Erro::EntradaEPasta(_) => "ENTRADA_E_PASTA",
            Erro::NomeRepetido(_) => "NOME_REPETIDO",
        }
    }
}

impl fmt::Display for Erro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Erro::NaoE7z => write!(f, "nao e um arquivo 7z: a assinatura nao confere"),
            Erro::VersaoNaoSuportada(v) => write!(f, "versao {v} do formato 7z nao suportada"),
            Erro::Estrutura(onde) => write!(f, "estrutura do 7z invalida: {onde}"),
            Erro::Corrompido(onde) => write!(f, "arquivo corrompido: {onde}"),
            Erro::SenhaErrada => write!(f, "senha errada"),
            Erro::SenhaErradaOuCorrompido => write!(
                f,
                "senha errada ou arquivo corrompido: este arquivo nao guarda o CRC \
                 do dado cifrado, e sem ele o formato nao separa os dois"
            ),
            Erro::SenhaAusente => write!(f, "o arquivo e cifrado e nao veio senha"),
            Erro::MetodoLegado(nome) => {
                write!(f, "metodo legado, nao suportado pelo PhxZip: {nome}")
            }
            Erro::MetodoDesconhecido(id) => write!(f, "metodo desconhecido: {id}"),
            Erro::NomePerigoso(nome) => write!(
                f,
                "nome de entrada perigoso, recusado: {nome:?} escreveria fora do destino"
            ),
            Erro::GrandeDemais {
                oque,
                declarado,
                teto,
            } => write!(
                f,
                "{oque} declara {declarado} bytes, acima do teto de {teto}"
            ),
            Erro::NaoCabe { oque, valor } => {
                write!(f, "{oque} de {valor} nao cabe no endereco desta plataforma")
            }
            Erro::CiclosDemais { pedidos, teto } => write!(
                f,
                "a derivacao da chave pede 2^{pedidos} rodadas, acima do teto de 2^{teto}"
            ),
            Erro::EntradaInexistente(i) => write!(f, "nao ha entrada {i} neste arquivo"),
            Erro::SemEntrada => write!(f, "o arquivo nao tem entrada nenhuma"),
            Erro::MaisDeUmaEntrada(n) => {
                write!(f, "um .phz tem uma entrada so, e este tem {n}")
            }
            Erro::SemCifra => write!(
                f,
                "o conteudo nao passou pelo 7zAES: e 7z, mas nao e um .phz (a barreira \
                 contra editor)"
            ),
            Erro::EntradaEPasta(nome) => {
                write!(f, "a unica entrada do .phz e uma pasta: {nome}")
            }
            Erro::NomeRepetido(nome) => write!(f, "o nome {nome:?} ja foi acrescentado"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Erro {}

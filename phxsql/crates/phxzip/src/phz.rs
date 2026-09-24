//! `.phz`: um 7z de UMA entrada, sempre cifrado -- o arquivo de configuracao
//! que o administrador nao abre num editor (pedido 450).
//!
//! # O que isto E e o que NAO E
//!
//! Decisao do dono, 24/09/2026, com o custo apresentado antes: os JSON de
//! configuracao passam a ser gravados como `.phz`, com uma senha fixa no
//! binario. Isso e barreira contra quem abre o arquivo num editor ou num
//! visualizador. **Nao e cifra contra quem tem o binario ou o repositorio**: o
//! repositorio e publico e `strings` no binario entrega a senha. Nenhum
//! documento pode chamar isto de cifra -- a mesma regra do
//! `encryption_exigida` (pedido 366). A senha NAO mora aqui: e parametro. A do
//! `config.phz` do servidor mora em `phxsql-server/src/config_phz.rs`
//! (`SENHA_DO_PHZ`, etapa 2), ao lado do comentario que cita o pedido.
//!
//! # Com a senha constante, o `.phz` nao da sigilo NEM integridade
//!
//! Dito pelo parecer SEC de 24/09/2026 (pedido 471), e vale para todo leitor
//! deste modulo:
//!
//! * **Sigilo zero.** O escritor grava sal vazio (como o 7-Zip), entao a chave
//!   AES e funcao pura da senha -- a MESMA em todo servidor, e derivavel por
//!   qualquer um que leia o repositorio.
//! * **Integridade zero.** O que confere o conteudo e o CRC-32, que nao e
//!   criptografico, e o AES-CBC do 7z nao autentica. Quem consegue ESCREVER o
//!   `.phz` (no disco, num backup, num canal de sincronia) forja uma
//!   configuracao com todos os CRCs certos, e [`desempacotar`] a aceita. A
//!   autenticidade da configuracao tem de vir de FORA -- permissao do arquivo,
//!   assinatura destacada --, nunca do `.phz`.
//! * **Sem oraculo, por construcao.** Como nao ha segredo, separar «senha
//!   errada» de «corrompido» nao revela nada que ja nao seja publico; e o CBC
//!   do 7z nao tem preenchimento PKCS (completa com zero e guarda o tamanho no
//!   cabecalho), entao nao ha oraculo de preenchimento.
//! * **Sem zeroizacao.** A crate e `no_std` sem crate de fora: a chave fica no
//!   cache de chaves e a senha em UTF-16 dentro do `Arquivo` ate eles serem
//!   soltos, sem sobrescrita. Irrelevante com senha publica; quem reusar este
//!   AES ou esta derivacao para segredo de verdade tem de rever isto antes.
//!
//! # Por que 7z, e por que por dentro
//!
//! O dono escolheu o formato do 7-Zip e escreve-lo dentro do PhxSql: o
//! administrador abre o `.phz` no 7-Zip com a senha, confere, e o PhxSql le o
//! que o 7-Zip gravar de volta (LZMA2 e cabecalho cifrado, os padroes dele).

use alloc::string::String;
use alloc::vec::Vec;

use crate::chave::CICLOS_PADRAO;
use crate::erro::Erro;
use crate::escritor::{Escritor, Metodo, Opcoes};
use crate::leitor::{Arquivo, Limites};

/// A extensao do arquivo, sem o ponto. Por dentro e 7z; o nome proprio e
/// decisao do dono (24/09/2026) e nao muda o formato.
pub const EXTENSAO: &str = "phz";
/// O maior conteudo que [`desempacotar`] aceita: 16 MiB. Um JSON de
/// configuracao tem KiB; o teto existe para a bomba, nao para o JSON.
pub const TETO_PADRAO: u64 = 16 << 20;

/// Grava `conteudo` como a unica entrada `nome`, comprimido em LZMA2 e cifrado
/// com o 7zAES -- conteudo e cabecalho --, com as 2^19 rodadas do 7-Zip.
pub fn empacotar(nome: &str, conteudo: &[u8], senha: &str) -> Result<Vec<u8>, Erro> {
    empacotar_com_ciclos(nome, conteudo, senha, CICLOS_PADRAO)
}

/// Como [`empacotar`], escolhendo o `NumCyclesPower`. A senha do `.phz` e
/// publica, entao as rodadas nao compram protecao e so custam tempo -- num
/// ESP32, o que decide. O 7-Zip le de 0 a 24.
pub fn empacotar_com_ciclos(
    nome: &str,
    conteudo: &[u8],
    senha: &str,
    ciclos: u8,
) -> Result<Vec<u8>, Erro> {
    let mut e = Escritor::novo(Opcoes {
        metodo: Metodo::Lzma2,
        senha: Some(String::from(senha)),
        ciclos,
        cifrar_cabecalho: true,
    })?;
    e.arquivo(nome, conteudo, None)?;
    Ok(e.terminar())
}

/// Le um `.phz` e devolve o nome da entrada e o conteudo, com o teto padrao.
///
/// O nome vem de fora e ja passou pela conferencia de caminho, mas quem chama
/// nao deve usa-lo como caminho: o `.phz` tem um conteudo, e o lugar dele e
/// decidido por quem chama.
pub fn desempacotar(arquivo: &[u8], senha: &str) -> Result<(String, Vec<u8>), Erro> {
    desempacotar_com_teto(arquivo, senha, TETO_PADRAO)
}

/// Como [`desempacotar`], com o teto do conteudo descompactado.
pub fn desempacotar_com_teto(
    arquivo: &[u8],
    senha: &str,
    teto: u64,
) -> Result<(String, Vec<u8>), Erro> {
    let limites = Limites {
        entrada: teto,
        bloco: teto,
        cabecalho: teto.min(1 << 20),
        ..Limites::default()
    };
    desempacotar_com_limites(arquivo, senha, limites)
}

/// Como [`desempacotar`], com os [`Limites`] inteiros de quem chama.
///
/// Existe para quem sabe o TAMANHO do que espera: o `.phz` de configuracao do
/// servidor (pedido 450, etapa 2) tem uma entrada e um cabecalho de centenas
/// de bytes, e abri-lo com o cabecalho de 1 MiB e as 65.536 contagens do
/// padrao seria dar ao arquivo mais folga do que ele jamais precisa. A regra
/// do `.phz` -- uma entrada, cifrada, que nao e pasta -- continua morando so
/// aqui: quem aperta os limites nao reescreve a conferencia.
pub fn desempacotar_com_limites(
    arquivo: &[u8],
    senha: &str,
    limites: Limites,
) -> Result<(String, Vec<u8>), Erro> {
    let mut a = Arquivo::abrir(arquivo, Some(senha), limites)?;
    let e = match a.entradas() {
        [] => return Err(Erro::SemEntrada),
        [e] => e.clone(),
        v => return Err(Erro::MaisDeUmaEntrada(v.len() as u64)),
    };
    if e.pasta {
        return Err(Erro::EntradaEPasta(e.nome));
    }
    // Protegido e o que passou pelo 7zAES. Conteudo vazio nao tem bloco, e ai
    // o que conta e o cabecalho ter vindo cifrado.
    if !(e.cifrada || (e.tamanho == 0 && a.cabecalho_cifrado())) {
        return Err(Erro::SemCifra);
    }
    let dados = a.extrair(0)?;
    Ok((e.nome, dados))
}

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
//! `encryption_exigida` (pedido 366). A senha NAO mora aqui: e parametro, e a
//! etapa 2 do pedido decide onde ela fica.
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

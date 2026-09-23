//! Guarda de senha: hash em vez de texto puro.
//!
//! O `config.json` guarda o HASH da senha, nunca a senha. O arquivo de
//! configuracao vai para backup, para o Git e para o suporte -- e um hash
//! nesses lugares e um aborrecimento, enquanto uma senha e um incidente.
//!
//! # Formato
//!
//! ```text
//! pbkdf2-sha256$210000$<sal em hex>$<hash em hex>
//!               ^        ^            ^
//!               |        |            derivado da senha com o sal
//!               |        16 bytes, unico por senha
//!               iteracoes (o custo)
//! ```
//!
//! Tudo o que e preciso para conferir esta na propria linha, entao mudar o
//! custo no futuro nao invalida as senhas antigas: cada uma carrega o numero
//! de iteracoes com que foi criada.

use crate::error::{PhxError, Result};
use crate::hash::{de_hex, iguais_em_tempo_constante, para_hex, pbkdf2_sha256};

/// Iteracoes adotadas para senhas novas.
///
/// E a recomendacao da OWASP para PBKDF2-HMAC-SHA256. Conferir uma senha custa
/// da ordem de 100 ms -- irrelevante uma vez por conexao, caro para quem tenta
/// adivinhar em massa. E por isso que a autenticacao acontece uma vez por
/// conexao, e nao a cada pedido.
pub const ITERACOES_PADRAO: u32 = 210_000;

const SAL_LEN: usize = 16;
const HASH_LEN: usize = 32;
const ALGORITMO: &str = "pbkdf2-sha256";

/// Gera o hash de uma senha, com sal novo.
pub fn cifrar(senha: &str) -> String {
    cifrar_com(senha, ITERACOES_PADRAO)
}

pub fn cifrar_com(senha: &str, iteracoes: u32) -> String {
    let sal = sal_novo();
    let mut derivado = [0u8; HASH_LEN];
    pbkdf2_sha256(senha.as_bytes(), &sal, iteracoes, &mut derivado);
    format!(
        "{ALGORITMO}${iteracoes}${}${}",
        para_hex(&sal),
        para_hex(&derivado)
    )
}

/// Confere uma senha contra o hash guardado.
///
/// Devolve `false` para hash malformado, em vez de erro: um `config.json`
/// estragado nao pode virar porta de entrada.
pub fn conferir(senha: &str, guardado: &str) -> bool {
    let Ok((iteracoes, sal, esperado)) = destrinchar(guardado) else {
        return false;
    };
    let mut derivado = vec![0u8; esperado.len()];
    pbkdf2_sha256(senha.as_bytes(), &sal, iteracoes, &mut derivado);
    iguais_em_tempo_constante(&derivado, &esperado)
}

/// A linha e um hash no formato deste modulo?
pub fn e_hash(texto: &str) -> bool {
    destrinchar(texto).is_ok()
}

fn destrinchar(guardado: &str) -> Result<(u32, Vec<u8>, Vec<u8>)> {
    let partes: Vec<&str> = guardado.trim().split('$').collect();
    let ruim = || PhxError::Esquema("hash de senha malformado".to_string());
    if partes.len() != 4 || partes[0] != ALGORITMO {
        return Err(ruim());
    }
    let iteracoes: u32 = partes[1].parse().map_err(|_| ruim())?;
    if iteracoes == 0 {
        return Err(ruim());
    }
    let sal = de_hex(partes[2]).ok_or_else(ruim)?;
    let hash = de_hex(partes[3]).ok_or_else(ruim)?;
    if sal.is_empty() || hash.is_empty() {
        return Err(ruim());
    }
    Ok((iteracoes, sal, hash))
}

/// Bytes aleatorios da fonte do sistema operacional, para sal, nonce e CHAVE.
///
/// # Falha fechado
///
/// Sem fonte de entropia do sistema, isto entra em panico -- nunca devolve
/// bytes de uma mistura de relogio, PID e endereco, como fazia ate 23/09/2026.
/// Aquela mistura servia a um sal (que so precisa ser unico), mas esta mesma
/// funcao semeia o `cifra::sortear` e gera chave Ed25519, X25519, AC do
/// phxvpn e token de sessao: chave adivinhavel e pior que servico parado
/// (achado C2 da revisao de seguranca do phxvpn). No Windows, onde nao ha
/// `/dev/urandom` e a mistura rodava SEMPRE, a fonte agora e o
/// `BCryptGenRandom` do proprio sistema.
pub fn bytes_aleatorios(quantos: usize) -> Vec<u8> {
    let mut saida = vec![0u8; quantos];
    if let Err(e) = entropia_do_sistema(&mut saida) {
        panic!("sem fonte de entropia do sistema ({e}): recuso gerar bytes previsiveis");
    }
    saida
}

/// Le do `/dev/urandom` por um descritor aberto UMA vez e guardado.
///
/// Abrir a cada chamada fazia o sorteio depender de haver descritor livre: com
/// a tabela esgotada (uma avalanche de conexoes basta), o `open` falhava e o
/// codigo antigo caia na mistura em silencio.
#[cfg(unix)]
fn entropia_do_sistema(saida: &mut [u8]) -> std::io::Result<()> {
    use std::io::Read;
    use std::sync::OnceLock;
    static URANDOM: OnceLock<std::fs::File> = OnceLock::new();
    let arquivo = match URANDOM.get() {
        Some(f) => f,
        None => {
            let f = std::fs::File::open("/dev/urandom")?;
            // Duas threads podem abrir juntas; fica o primeiro, o outro fecha.
            let _ = URANDOM.set(f);
            URANDOM.get().expect("acabou de ser posto")
        }
    };
    // ATENCAO: /dev/urandom e INFINITO -- le exatamente o pedido.
    (&*arquivo).read_exact(saida)
}

#[cfg(windows)]
fn entropia_do_sistema(saida: &mut [u8]) -> std::io::Result<()> {
    use std::ffi::c_void;
    #[link(name = "bcrypt")]
    extern "system" {
        fn BCryptGenRandom(alg: *mut c_void, buf: *mut u8, tamanho: u32, bandeiras: u32) -> i32;
    }
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;
    for pedaco in saida.chunks_mut(u32::MAX as usize) {
        // SAFETY: o ponteiro e o tamanho sao do proprio pedaco, valido e
        // exclusivo durante a chamada; o algoritmo nulo e o que a bandeira
        // BCRYPT_USE_SYSTEM_PREFERRED_RNG exige.
        let st = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                pedaco.as_mut_ptr(),
                pedaco.len() as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        if st != 0 {
            return Err(std::io::Error::other(format!(
                "BCryptGenRandom devolveu {st:#x}"
            )));
        }
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn entropia_do_sistema(_saida: &mut [u8]) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "plataforma sem fonte de entropia conhecida",
    ))
}

/// O material derivado que esta guardado dentro de um hash de senha.
///
/// E o que o desafio-resposta usa como chave: o servidor ja tem, e o cliente
/// chega nele a partir da senha, do sal e das iteracoes.
pub fn derivado_do_hash(guardado: &str) -> Result<Vec<u8>> {
    destrinchar(guardado).map(|(_, _, hash)| hash)
}

/// Sal e iteracoes de um hash guardado, para mandar ao cliente no desafio.
pub fn sal_e_iteracoes(guardado: &str) -> Result<(Vec<u8>, u32)> {
    destrinchar(guardado).map(|(it, sal, _)| (sal, it))
}

/// Sal novo de 16 bytes, da mesma fonte que as chaves.
fn sal_novo() -> [u8; SAL_LEN] {
    let mut sal = [0u8; SAL_LEN];
    sal.copy_from_slice(&bytes_aleatorios(SAL_LEN));
    sal
}

#[cfg(test)]
mod tests {
    use super::*;

    // Iteracoes baixas nos testes: o que se testa aqui e a logica, nao o custo.
    const RAPIDO: u32 = 64;

    #[test]
    fn cifra_e_confere() {
        let h = cifrar_com("Senha Forte 123", RAPIDO);
        assert!(conferir("Senha Forte 123", &h));
        assert!(!conferir("senha forte 123", &h), "maiuscula conta");
        assert!(!conferir("Senha Forte 124", &h));
        assert!(!conferir("", &h));
    }

    #[test]
    fn o_formato_e_o_documentado() {
        let h = cifrar_com("x", RAPIDO);
        let partes: Vec<&str> = h.split('$').collect();
        assert_eq!(partes.len(), 4);
        assert_eq!(partes[0], "pbkdf2-sha256");
        assert_eq!(partes[1], RAPIDO.to_string());
        assert_eq!(partes[2].len(), 32, "sal de 16 bytes em hex");
        assert_eq!(partes[3].len(), 64, "hash de 32 bytes em hex");
        assert!(e_hash(&h));
    }

    #[test]
    fn duas_senhas_iguais_dao_hashes_diferentes() {
        let a = cifrar_com("a mesma senha", RAPIDO);
        let b = cifrar_com("a mesma senha", RAPIDO);
        assert_ne!(a, b, "o sal precisa ser novo a cada vez");
        assert!(conferir("a mesma senha", &a));
        assert!(conferir("a mesma senha", &b));
    }

    #[test]
    fn o_custo_viaja_junto_com_o_hash() {
        // Um hash criado com custo antigo continua conferindo depois que o
        // padrao muda, porque as iteracoes estao na propria linha.
        let antigo = cifrar_com("legado", 32);
        assert!(conferir("legado", &antigo));
        let novo = cifrar_com("legado", 128);
        assert!(conferir("legado", &novo));
        assert_ne!(antigo, novo);
    }

    #[test]
    fn hash_estragado_nunca_deixa_entrar() {
        for ruim in [
            "",
            "senha-em-texto-puro",
            "pbkdf2-sha256$0$aa$bb",
            "pbkdf2-sha256$100$$bb",
            "pbkdf2-sha256$100$aa$",
            "pbkdf2-sha256$abc$aa$bb",
            "md5$100$aa$bb",
            "pbkdf2-sha256$100$aa",
            "pbkdf2-sha256$100$zz$bb",
        ] {
            assert!(!e_hash(ruim), "deveria recusar o formato: {ruim:?}");
            assert!(
                !conferir("qualquer coisa", ruim),
                "hash estragado deixou entrar: {ruim:?}"
            );
        }
    }

    #[test]
    fn senha_com_acento_e_espaco() {
        let h = cifrar_com("Ação do José 2026!", RAPIDO);
        assert!(conferir("Ação do José 2026!", &h));
        assert!(!conferir("Acao do Jose 2026!", &h));
    }

    #[test]
    fn urandom_le_so_o_que_precisa_e_nao_trava() {
        // /dev/urandom e infinito: se a leitura nao for limitada, isto nunca
        // retorna. O teste existe para travar essa regressao.
        assert_ne!(sal_novo(), sal_novo(), "duas leituras nao podem coincidir");
    }

    /// Prova do C2 contra o SISTEMA OPERACIONAL: um processo filho ESGOTA a
    /// propria tabela de descritores (como uma avalanche de conexoes faria) e
    /// so entao sorteia pela primeira vez. Tem de morrer em panico -- nunca
    /// devolver bytes. Com a mistura antiga, o filho saia 0 com bytes
    /// previsiveis. (`ulimit -n 3` direto nao serve: o carregador dinamico
    /// precisa de descritor para o executavel sequer subir.)
    #[cfg(target_os = "linux")]
    #[test]
    fn sem_descritor_o_sorteio_falha_fechado() {
        if std::env::var_os("PHX_FILHO_SEM_DESCRITOR").is_some() {
            let mut presos = Vec::new();
            while let Ok(f) = std::fs::File::open("/dev/null") {
                presos.push(f);
            }
            let b = bytes_aleatorios(16);
            println!("BYTES {}", crate::hash::para_hex(&b));
            return;
        }
        let exe = std::env::current_exe().unwrap();
        let saida = std::process::Command::new("sh")
            .arg("-c")
            .arg("ulimit -n 64 && exec \"$0\" --exact senha::tests::sem_descritor_o_sorteio_falha_fechado --test-threads=1 --nocapture")
            .arg(exe)
            .env("PHX_FILHO_SEM_DESCRITOR", "1")
            .output()
            .unwrap();
        let texto = String::from_utf8_lossy(&saida.stdout);
        assert!(
            !texto.contains("BYTES "),
            "o filho sem descritor devolveu bytes: {texto}"
        );
        assert!(
            !saida.status.success(),
            "o filho sem descritor tinha de falhar"
        );
        // E tem de falhar PELO sorteio -- nao porque o executor de testes nao
        // conseguiu subir com tres descritores, o que passaria por engano.
        let erro = String::from_utf8_lossy(&saida.stderr);
        assert!(
            erro.contains("sem fonte de entropia"),
            "o filho falhou por outro motivo: {erro}"
        );
    }

    #[test]
    fn bytes_aleatorios_no_tamanho_pedido() {
        for n in [0usize, 1, 15, 16, 17, 64, 100] {
            assert_eq!(bytes_aleatorios(n).len(), n);
        }
        assert_ne!(bytes_aleatorios(32), bytes_aleatorios(32));
    }

    #[test]
    fn extrai_o_derivado_e_o_sal_do_hash() {
        let h = cifrar_com("segredo", RAPIDO);
        let dk = derivado_do_hash(&h).unwrap();
        assert_eq!(dk.len(), 32);
        let (sal, it) = sal_e_iteracoes(&h).unwrap();
        assert_eq!(sal.len(), 16);
        assert_eq!(it, RAPIDO);
        // Refazer a conta a partir da senha da o mesmo derivado.
        let mut refeito = vec![0u8; 32];
        crate::hash::pbkdf2_sha256(b"segredo", &sal, it, &mut refeito);
        assert_eq!(refeito, dk);
        assert!(derivado_do_hash("nao-e-hash").is_err());
    }

    #[test]
    fn sal_nunca_repete_em_sequencia() {
        let mut vistos = std::collections::HashSet::new();
        for _ in 0..200 {
            assert!(vistos.insert(sal_novo()), "sal repetiu");
        }
    }
}

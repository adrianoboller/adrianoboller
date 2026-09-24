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
use crate::hash::{de_hex, iguais_em_tempo_constante, para_hex, pbkdf2_sha256, sha256};

/// Iteracoes adotadas para senhas novas.
///
/// ATENCAO, conferido em 24/09/2026 (pedido 521): esta linha dizia que 210.000
/// e «a recomendacao da OWASP para PBKDF2-HMAC-SHA256», e a folha da OWASP
/// (Password Storage Cheat Sheet) pede 600.000 para SHA-256; 210.000 foi o
/// numero dela para SHA-512 e hoje e 220.000. O valor nao mudou aqui -- mudar
/// e decisao de custo por login, e nao do 521. Conferir uma senha custa da
/// ordem de 100 ms (numero antigo, nao remedido depois de a chave do HMAC
/// passar a ser preparada uma vez, que dividiu o custo por ~2 em debug) --
/// irrelevante uma vez por conexao, caro para quem tenta adivinhar em massa.
/// E por isso que a autenticacao acontece uma vez por conexao, e nao a cada
/// pedido.
pub const ITERACOES_PADRAO: u32 = 210_000;

const SAL_LEN: usize = 16;
const HASH_LEN: usize = 32;
const ALGORITMO: &str = "pbkdf2-sha256";

/// O maior tamanho de senha, em BYTES, que entra numa conta -- pedido 521.
///
/// # Por que 65.535
///
/// Decidido pela regua dos motores, e nao escolhido. Pergunta: acima de que
/// tamanho o servidor recusa a senha em claro que recebe? No fonte de cada um:
///
/// | motor (peso) | teto da senha em claro |
/// |---|---|
/// | PostgreSQL (4) | 65.535 B no login (`PG_MAX_AUTH_TOKEN_LENGTH`, `libpq/auth.h`, lido em `recv_password_packet`); o de 1.024 do SASLprep saiu na 14 |
/// | MariaDB (3) | nenhum proprio (`parsec`, `ed25519` e nativo recebem o tamanho que vier); so o pacote limita |
/// | MySQL (2) | 256 B (`MAX_PLAINTEXT_LENGTH`, recusado no `caching_sha2_password` e no `sha256_password`, ao criar e ao entrar) |
/// | SQLite (1) | nao tem usuario; nao vota |
///
/// Nao ha convergencia, e a regua ponderada decide degrau a degrau: «teto
/// ate 256?» perde de 2 a 7; «teto ate 65.535?» ganha de 6 (PG + MySQL) a 3.
/// O numero que sobra e o do PostgreSQL. A hipotese «sem teto proprio»
/// (MariaDB) perdeu do mesmo jeito, 3 a 6.
///
/// E o numero conversa com o resto da casa: a linha anonima da porta de dados
/// ja nao passa de 64 KiB (`fio::TETO_DO_APERTO`), entao ali ele quase nao
/// morde -- quem morde e a porta web, cujo corpo aceita 4 MiB antes da
/// credencial, e o `CREATE USER` depois dela. Com a chave preparada uma vez
/// (`hash::ChaveHmac`), uma senha no teto paga 1.025 compressoes de SHA-256
/// a mais que uma de 8 B, sobre 420.002 das 210.000 iteracoes: 0,24%.
pub const TETO_DA_SENHA: usize = 65_535;

/// A senha cabe no teto? Recusa ANTES de qualquer PBKDF2.
///
/// A mensagem diz o tamanho e nunca o conteudo: e a mesma regra do resto da
/// casa para o que nao se pode mostrar -- vira o tamanho em bytes.
pub fn caber_no_teto(senha: &str) -> Result<()> {
    if senha.len() > TETO_DA_SENHA {
        return Err(PhxError::LimiteExcedido(format!(
            "a senha tem {} bytes e o teto e {TETO_DA_SENHA}; nenhuma conta foi feita com ela",
            senha.len()
        )));
    }
    Ok(())
}

/// O hash do usuario que NAO existe -- pedido 520.
///
/// O login de quem nao existe confere a senha contra ESTE hash, pela mesma
/// [`conferir`] de quem existe e com as [`ITERACOES_PADRAO`] com que todo
/// usuario novo nasce: e o que faz «nao existe» e «senha errada» custarem o
/// mesmo. A fachada de antes era um `cifrar_com("nao-existe", 1_000)` -- 1.000
/// iteracoes para fabricar o hash e mais 1.000 para conferir, contra 210.000
/// de quem existe: medido em debug, 25,7 ms contra 2.671 ms. O relogio dizia
/// quem existe.
///
/// E o mesmo usuario de mentira que o `desafio` apresenta a quem nao existe
/// (sal falso, `ITERACOES_PADRAO`). O `derivado` e um resumo fixo: ninguem
/// precisa acertar esta senha, e acertar nao adiantaria -- nao ha usuario
/// para devolver.
pub fn hash_de_fachada() -> &'static str {
    static FACHADA: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    FACHADA.get_or_init(|| {
        let sal = sha256(b"phxsql: sal do usuario que nao existe");
        let derivado = sha256(b"phxsql: derivado do usuario que nao existe");
        format!(
            "{ALGORITMO}${ITERACOES_PADRAO}${}${}",
            para_hex(&sal[..SAL_LEN]),
            para_hex(&derivado[..HASH_LEN])
        )
    })
}

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
///
/// E `false` tambem para senha acima do [`TETO_DA_SENHA`], sem conta nenhuma:
/// quem chama e esquece o teto nao reabre o 521 por aqui.
pub fn conferir(senha: &str, guardado: &str) -> bool {
    if caber_no_teto(senha).is_err() {
        return false;
    }
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

/// Bytes aleatorios, para sal e para nonce.
///
/// Tenta `/dev/urandom`; onde ele nao existe, cai na mistura descrita em
/// [`sal_novo`].
pub fn bytes_aleatorios(quantos: usize) -> Vec<u8> {
    let mut saida = Vec::with_capacity(quantos);
    while saida.len() < quantos {
        match sal_do_urandom() {
            Some(b) => saida.extend_from_slice(&b),
            None => saida.extend_from_slice(&sal_por_mistura()),
        }
    }
    saida.truncate(quantos);
    saida
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

/// Sal novo de 16 bytes.
///
/// Tenta `/dev/urandom` primeiro. Onde ele nao existe (Windows), cai numa
/// mistura de relogio em nanossegundos, PID, endereco de heap (que o ASLR
/// muda a cada execucao) e um contador -- passada por SHA-256.
///
/// O que um sal exige e ser UNICO por senha, nao imprevisivel, e a mistura
/// garante isso. Ainda assim, `/dev/urandom` e o caminho preferido e o que
/// roda em Linux.
fn sal_novo() -> [u8; SAL_LEN] {
    // ATENCAO: /dev/urandom e um dispositivo INFINITO. Ler o "arquivo inteiro"
    // nunca termina -- tem de ser exatamente SAL_LEN bytes.
    if let Some(sal) = sal_do_urandom() {
        return sal;
    }
    sal_por_mistura()
}

fn sal_do_urandom() -> Option<[u8; SAL_LEN]> {
    use std::io::Read;
    let mut arquivo = std::fs::File::open("/dev/urandom").ok()?;
    let mut sal = [0u8; SAL_LEN];
    arquivo.read_exact(&mut sal).ok()?;
    Some(sal)
}

fn sal_por_mistura() -> [u8; SAL_LEN] {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static CONTADOR: AtomicU64 = AtomicU64::new(0);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let sequencia = CONTADOR.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id() as u64;
    // Endereco de uma alocacao: varia por execucao por causa do ASLR.
    let caixa = Box::new(0u8);
    let endereco = (&*caixa as *const u8) as u64;

    let mut entrada = Vec::with_capacity(32);
    entrada.extend_from_slice(&nanos.to_le_bytes());
    entrada.extend_from_slice(&sequencia.to_le_bytes());
    entrada.extend_from_slice(&pid.to_le_bytes());
    entrada.extend_from_slice(&endereco.to_le_bytes());

    let resumo = sha256(&entrada);
    let mut sal = [0u8; SAL_LEN];
    sal.copy_from_slice(&resumo[..SAL_LEN]);
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
        if let Some(a) = sal_do_urandom() {
            let b = sal_do_urandom().expect("segunda leitura tambem deve funcionar");
            assert_ne!(a, b, "duas leituras do urandom nao podem coincidir");
        }
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
            assert!(vistos.insert(sal_por_mistura()), "sal repetiu na mistura");
        }
    }

    // ------------------------------------------------ pedidos 520 e 521

    /// **O teto recusa antes da conta, e a recusa nao carrega a senha.** No
    /// teto a senha confere; um byte acima, nem a senha certa entra -- e o
    /// contador de iteracoes prova que o PBKDF2 nao rodou.
    #[test]
    fn a_senha_acima_do_teto_nao_chega_ao_pbkdf2() {
        let no_teto = "s".repeat(TETO_DA_SENHA);
        let acima = "s".repeat(TETO_DA_SENHA + 1);
        assert!(caber_no_teto(&no_teto).is_ok());
        let erro = caber_no_teto(&acima).unwrap_err().to_string();
        assert!(erro.contains(&(TETO_DA_SENHA + 1).to_string()), "{erro}");
        assert!(!erro.contains("sss"), "a recusa carregou a senha: {erro}");

        let h_no_teto = cifrar_com(&no_teto, RAPIDO);
        assert!(conferir(&no_teto, &h_no_teto));
        // Um hash feito para a senha longa demais -- o `cifrar` nao recusa, e
        // quem recusa e a porta; aqui ele serve para mostrar que nem a senha
        // CERTA faz o `conferir` gastar uma iteracao.
        let h_acima = cifrar_com(&acima, RAPIDO);
        let antes = crate::hash::iteracoes_pagas_nesta_thread();
        assert!(!conferir(&acima, &h_acima));
        assert_eq!(crate::hash::iteracoes_pagas_nesta_thread(), antes);
    }

    /// **O teto conta BYTES, e nao caracteres**, na borda exata e com
    /// caracteres de 2 e de 3 bytes (condicao C2 do parecer do SEC,
    /// `docs/propostas/parecer-sec-520-521-2026-09-24.md`). O teste de cima e
    /// todo ASCII, onde byte e caractere coincidem: trocar o `len()` do
    /// `caber_no_teto` por `chars().count()` passava nele -- e deixaria entrar
    /// 65.535 caracteres de 4 bytes, 262.140 B.
    ///
    /// Vermelho medido com essa troca reposta: «65536 B em 32768 caracteres
    /// de 2 bytes cabia no teto» -- a primeira acima da borda passa.
    #[test]
    fn o_teto_conta_bytes_e_nao_caracteres() {
        let casos = [
            // (senha, bytes, cabe?)
            (format!("{}a", "é".repeat(32_767)), 65_535, true),
            ("é".repeat(32_768), 65_536, false),
            ("€".repeat(21_845), 65_535, true),
            (format!("{}a", "€".repeat(21_845)), 65_536, false),
        ];
        for (senha, bytes, cabe) in &casos {
            assert_eq!(senha.len(), *bytes, "o caso foi montado errado");
            let largura = senha.chars().next().unwrap().len_utf8();
            let caracteres = senha.chars().count();
            assert_eq!(
                caber_no_teto(senha).is_ok(),
                *cabe,
                "{bytes} B em {caracteres} caracteres de {largura} bytes {} no teto",
                if *cabe { "nao cabia" } else { "cabia" }
            );
        }
        // E o `conferir` segue o mesmo teto: a acima, nem certa, e sem conta;
        // a da borda confere.
        let (acima, borda) = (&casos[1].0, &casos[2].0);
        let h_acima = cifrar_com(acima, RAPIDO);
        let antes = crate::hash::iteracoes_pagas_nesta_thread();
        assert!(!conferir(acima, &h_acima));
        assert_eq!(crate::hash::iteracoes_pagas_nesta_thread(), antes);
        assert!(conferir(borda, &cifrar_com(borda, RAPIDO)));
    }

    /// **A fachada do 520 e um usuario novo de mentira**: o mesmo formato, o
    /// mesmo custo de quem nasce pelo `cifrar`, e estavel entre chamadas.
    #[test]
    fn a_fachada_tem_o_custo_de_um_usuario_novo() {
        let fachada = hash_de_fachada();
        assert!(e_hash(fachada));
        let (sal, it) = sal_e_iteracoes(fachada).unwrap();
        assert_eq!(
            it, ITERACOES_PADRAO,
            "a fachada custa menos que um usuario de verdade"
        );
        assert_eq!(sal.len(), SAL_LEN);
        assert_eq!(derivado_do_hash(fachada).unwrap().len(), HASH_LEN);
        assert!(std::ptr::eq(fachada, hash_de_fachada()));
    }
}

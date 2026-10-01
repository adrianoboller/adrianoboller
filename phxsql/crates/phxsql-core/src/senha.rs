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
/// **O nome carrega segredo?** -- a lista UNICA da casa, por conteudo e sem
/// caixa: `alertas.email.senha`, `rest.token`, `cifra_fio.chave_privada_env`.
///
/// Morava no `diretivas.rs` do servidor, e o `phxsql-sql` -- que redige o SQL
/// do Profiler -- nao a enxergava: `ALTER SERVER SET alertas.email.senha =
/// x` saia cru, e a copia da lista la seria o defeito da lei «vem do mesmo
/// motor» (pedido 560). Falso positivo custa um valor escondido; falso
/// negativo, uma credencial em texto puro. A duvida vai para esconder.
pub fn nome_sigiloso(nome: &str) -> bool {
    let c = nome.to_ascii_lowercase();
    ["token", "senha", "password", "secret", "chave_privada"]
        .iter()
        .any(|s| c.contains(s))
}

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

/// Bytes aleatorios -- sal, nonce e MATERIAL DE CHAVE (a efemera do TLS e do
/// Noise, a privada do autoassinado, a semente de [`crate::cifra::sortear`]).
///
/// Tenta `/dev/urandom`, que e o caminho de Linux e macOS, como sempre foi.
/// Onde ele nao existe (Windows), vem de [`bytes_sem_urandom`] -- o gerador
/// do sistema pela propria `std` (pedido 606). NUNCA da mistura de relogio,
/// PID e endereco: essa e boa para sal, que pede so unicidade, e ruim para
/// chave, que pede segredo -- quem estima o instante do arranque e o PID
/// refazia a chave efemera e decifrava o trafego gravado.
pub fn bytes_aleatorios(quantos: usize) -> Vec<u8> {
    match ler_urandom(quantos) {
        Some(b) => b,
        None => bytes_sem_urandom(quantos),
    }
}

/// `quantos` bytes do `/dev/urandom`, ou `None` onde ele nao existe.
///
/// ATENCAO: o dispositivo e INFINITO -- ler "o arquivo inteiro" nunca
/// termina; e exatamente `quantos`.
fn ler_urandom(quantos: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut arquivo = std::fs::File::open("/dev/urandom").ok()?;
    let mut saida = vec![0u8; quantos];
    arquivo.read_exact(&mut saida).ok()?;
    Some(saida)
}

/// O caminho de [`bytes_aleatorios`] onde nao ha `/dev/urandom` -- pedido
/// 606. Separado para a prova rodar no Linux, onde o `/dev/urandom` existe e
/// esconderia este caminho.
///
/// Fonte morta (a `std` devolvendo a mesma semente a threads diferentes)
/// DERRUBA a chamada em vez de cair na mistura: chave adivinhavel servida
/// calada e pior que pedido recusado -- e e o mesmo que a `std` faz quando o
/// gerador do sistema falha ao semear um `HashMap` (`assert!` no
/// `RtlGenRandom`; o `ProcessPrng` e documentado como sempre `TRUE`).
fn bytes_sem_urandom(quantos: usize) -> Vec<u8> {
    let mut saida = Vec::with_capacity(quantos);
    while saida.len() < quantos {
        let semente = colher_da_std().unwrap_or_else(|| {
            panic!(
                "sem /dev/urandom e sem gerador do sistema pela std (pedido 606): \
                 o PhxSql recusa gerar chave de relogio e PID"
            )
        });
        saida.extend_from_slice(&semente);
    }
    saida.truncate(quantos);
    saida
}

/// Threads novas por colheita. Cada uma traz os 128 bits que a `std` pediu ao
/// sistema para ela; quatro dao 512 bits de entrada para os 256 da saida --
/// folga para a parte que o SipHash nao deixa passar.
const THREADS_DA_COLHEITA: usize = 4;

/// 32 bytes do gerador do SISTEMA, so com a `std` e sem `unsafe` -- pedido
/// 606.
///
/// # O que a `std` garante, e o que e detalhe dela
///
/// **Garantido (documentado):** o `RandomState` e semeado de «a high quality,
/// secure source of randomness provided by the host» -- e o que protege o
/// `HashMap` contra quem escolhe chaves para colidir.
///
/// **Detalhe de implementacao (lido no fonte da 1.94.1, nao contrato):**
/// `library/std/src/hash/random.rs` guarda as chaves num `thread_local!`
/// iniciado por `hashmap_random_keys()` -- uma vez POR THREAD, so somando 1
/// ao `k0` a cada `RandomState` seguinte da mesma thread. No Windows,
/// `sys/random/mod.rs` faz dela 16 bytes do `fill_bytes`, que e o
/// `ProcessPrng` (`RtlGenRandom` no alvo `win7`) -- a mesma chamada que um
/// FFI faria, sem o FFI que a petrea «so a `std`» nao deixa escrever. No
/// Linux e o `getrandom`.
///
/// Entao: cada thread NOVA traz 128 bits do sistema, e o hash de entradas
/// fixas com eles e funcao so desses bits. O SHA-256 dos hashes de
/// [`THREADS_DA_COLHEITA`] threads e a semente.
///
/// **O risco nomeado:** se a `std` passar a semear uma vez por PROCESSO, as
/// threads viram a mesma semente com o `k0` somado -- ainda do sistema, ainda
/// sem relogio, so com 128 bits em vez de 512 -- e nenhum teste ve isso (o
/// SipHash esconde a relacao entre as chaves). Se passar a nao semear do
/// sistema (o `unsupported.rs` dela usa enderecos), a conferencia de fonte
/// morta abaixo tambem nao pega: o caminho so vale onde a `std` tem gerador
/// (Windows aqui; Unix tem o `/dev/urandom` antes).
fn colher_da_std() -> Option<[u8; 32]> {
    use std::hash::{BuildHasher, Hasher, RandomState};
    let mut fios = Vec::with_capacity(THREADS_DA_COLHEITA);
    for _ in 0..THREADS_DA_COLHEITA {
        // `Builder::spawn`, e nao `thread::spawn`: sem thread, a colheita
        // falha como fonte morta, em vez de derrubar com outra mensagem.
        let fio = std::thread::Builder::new()
            .spawn(|| {
                let estado = RandomState::new();
                let mut colhido = [0u64; 4];
                for (i, c) in colhido.iter_mut().enumerate() {
                    let mut h = estado.build_hasher();
                    h.write_u64(i as u64);
                    *c = h.finish();
                }
                colhido
            })
            .ok()?;
        fios.push(fio);
    }
    let mut colheitas = Vec::with_capacity(THREADS_DA_COLHEITA);
    for fio in fios {
        colheitas.push(fio.join().ok()?);
    }
    // Fonte morta: duas threads novas com a mesma colheita -- a semente nao
    // veio do sistema, e o que sairia daqui seria previsivel.
    for (i, a) in colheitas.iter().enumerate() {
        if colheitas[i + 1..].contains(a) {
            return None;
        }
    }
    let mut entrada = Vec::with_capacity(THREADS_DA_COLHEITA * 32);
    for c in &colheitas {
        for x in c {
            entrada.extend_from_slice(&x.to_le_bytes());
        }
    }
    Some(sha256(&entrada))
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
    let mut sal = [0u8; SAL_LEN];
    sal.copy_from_slice(&ler_urandom(SAL_LEN)?);
    Some(sal)
}

/// Relogio, contador, PID e endereco: SO para sal (unicidade), nunca para
/// chave -- ver [`bytes_aleatorios`]. Previsivel por construcao: dados os
/// quatro, sai sempre o mesmo, e e isso que a prova do 606 mostra.
fn sal_por_mistura() -> [u8; SAL_LEN] {
    let (nanos, sequencia, pid, endereco) = ambiente_da_mistura();
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

/// O que a mistura do sal le do ambiente: nanossegundos do relogio, um
/// contador do processo, o PID e o endereco de uma alocacao (que o ASLR muda
/// a cada execucao). Nos testes, uma thread pode fixa-los -- e o retrato de
/// quem ADIVINHA o instante e o PID, que e o ataque do 606.
fn ambiente_da_mistura() -> (u64, u64, u64, u64) {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static CONTADOR: AtomicU64 = AtomicU64::new(0);

    #[cfg(test)]
    if let Some(fixo) = tests::AMBIENTE_FIXO.with(|a| a.get()) {
        return fixo;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let sequencia = CONTADOR.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id() as u64;
    let caixa = Box::new(0u8);
    let endereco = (&*caixa as *const u8) as u64;
    (nanos, sequencia, pid, endereco)
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

    thread_local! {
        /// Relogio, contador, PID e endereco fixados NESTA thread -- ver
        /// [`ambiente_da_mistura`].
        pub(super) static AMBIENTE_FIXO: std::cell::Cell<Option<(u64, u64, u64, u64)>> =
            const { std::cell::Cell::new(None) };
    }

    /// Relogio e PID de um arranque que o atacante estimou -- os mesmos nos
    /// dois processos da prova.
    const AMBIENTE_ADIVINHADO: (u64, u64, u64, u64) =
        (1_790_000_000_000_000_000, 0, 4242, 0x5555_0000_1000);

    /// **Pedido 606: sem `/dev/urandom`, a chave nao sai do relogio nem do
    /// PID.** Com relogio, contador, PID e endereco FIXOS -- o que quem
    /// estima o arranque tem na mao --, duas chamadas tem de dar bytes
    /// diferentes. Com o defeito reposto (`bytes_sem_urandom` pela mistura),
    /// as duas saem iguais, e iguais a mistura refeita por fora.
    #[test]
    fn sem_urandom_a_chave_nao_sai_do_relogio_nem_do_pid() {
        AMBIENTE_FIXO.with(|a| a.set(Some(AMBIENTE_ADIVINHADO)));
        let refeita = sal_por_mistura();
        let a = bytes_sem_urandom(32);
        let b = bytes_sem_urandom(32);
        AMBIENTE_FIXO.with(|a| a.set(None));
        assert_eq!(
            refeita,
            sal_por_mistura_fixa(),
            "a premissa: a mistura e funcao so do ambiente"
        );
        assert_ne!(
            a, b,
            "com relogio e PID fixos a chave repetiu: ela sai do relogio e do PID"
        );
        assert_ne!(&a[..SAL_LEN], &refeita[..], "a chave E a mistura refeita");
    }

    fn sal_por_mistura_fixa() -> [u8; SAL_LEN] {
        AMBIENTE_FIXO.with(|a| a.set(Some(AMBIENTE_ADIVINHADO)));
        let s = sal_por_mistura();
        AMBIENTE_FIXO.with(|a| a.set(None));
        s
    }

    const FILHO_606: &str = "PHX_606_FILHO";

    /// O corpo do filho de [`dois_processos_no_mesmo_instante_dao_chaves_diferentes`]:
    /// fora dele, nao faz nada.
    #[test]
    fn filho_606_imprime_a_chave_e_a_mistura() {
        if std::env::var_os(FILHO_606).is_none() {
            return;
        }
        AMBIENTE_FIXO.with(|a| a.set(Some(AMBIENTE_ADIVINHADO)));
        let chave = para_hex(&bytes_sem_urandom(32));
        let mistura = para_hex(&sal_por_mistura());
        AMBIENTE_FIXO.with(|a| a.set(None));
        println!("606-chave={chave}");
        println!("606-mistura={mistura}");
    }

    /// **Pedido 606, contra o SO: dois PROCESSOS no mesmo instante e com o
    /// mesmo PID dao chaves diferentes.** Os dois filhos sobem juntos, com
    /// relogio, contador, PID e endereco fixados iguais. A mistura velha da o
    /// MESMO nos dois -- e e o que um atacante refaz --; a chave tem de dar
    /// diferente, porque vem do gerador do sistema. O caminho e o generico da
    /// `std` (o `RandomState` de threads novas), o mesmo do Windows; no
    /// Windows mesmo, NAO MEDIDO (sem Windows nem `wine` no conteiner).
    #[test]
    fn dois_processos_no_mesmo_instante_dao_chaves_diferentes() {
        if std::env::var_os(FILHO_606).is_some() {
            return;
        }
        let eu = std::env::current_exe().unwrap();
        let subir = || {
            std::process::Command::new(&eu)
                .args([
                    "--exact",
                    "senha::tests::filho_606_imprime_a_chave_e_a_mistura",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(FILHO_606, "1")
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        };
        let (a, b) = (subir(), subir());
        let ler = |f: std::process::Child| {
            let saida = f.wait_with_output().unwrap();
            assert!(saida.status.success());
            let texto = String::from_utf8_lossy(&saida.stdout).into_owned();
            let campo = |nome: &str| {
                texto
                    .lines()
                    .find_map(|l| l.split_once(nome).map(|(_, v)| v.trim()))
                    .unwrap_or_else(|| panic!("o filho nao imprimiu {nome}:\n{texto}"))
                    .to_string()
            };
            (campo("606-chave="), campo("606-mistura="))
        };
        let ((chave_a, mistura_a), (chave_b, mistura_b)) = (ler(a), ler(b));
        assert_eq!(
            mistura_a, mistura_b,
            "a premissa: com relogio e PID iguais a mistura velha repete"
        );
        assert_ne!(
            chave_a, chave_b,
            "dois processos com o mesmo relogio e o mesmo PID deram a MESMA \
             chave: ela sai da mistura, e quem adivinha o arranque a refaz"
        );
    }

    /// **Pedido 606: threads novas colhem sementes diferentes, e duas
    /// colheitas nunca coincidem.** E o que a conferencia de fonte morta de
    /// [`colher_da_std`] supoe. O que este teste NAO ve, dito: se a `std`
    /// passar a semear uma vez por processo e so somar 1 ao `k0` por
    /// `RandomState`, os hashes continuam diferentes (o SipHash nao deixa ver
    /// a relacao entre as chaves) -- a entrada cai de 512 para 128 bits do
    /// sistema, ainda sem relogio. Esse risco fica nomeado no comentario de
    /// [`colher_da_std`], nao vigiado.
    #[test]
    fn duas_threads_novas_colhem_diferente() {
        use std::hash::{BuildHasher, RandomState};
        let semente_da_thread = || {
            std::thread::spawn(|| RandomState::new().hash_one(0u64))
                .join()
                .unwrap()
        };
        let vistos: std::collections::HashSet<_> = (0..16).map(|_| semente_da_thread()).collect();
        assert_eq!(vistos.len(), 16, "threads novas repetiram a semente da std");
        let a = colher_da_std().expect("a std tem gerador neste alvo");
        let b = colher_da_std().expect("a std tem gerador neste alvo");
        assert_ne!(a, b);
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

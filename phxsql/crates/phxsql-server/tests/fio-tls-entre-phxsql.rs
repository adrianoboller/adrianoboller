//! O fio TLS entre dois PhxSql (pedido 572, T6b-2) -- pelo soquete.
//!
//! O `replica::Cliente` e o iniciador da replica, do pulso e da propagacao do
//! cluster, da sonda, do DbLink `phx` e do console. Com `pino_tls` escrito ele
//! fala TLS 1.3 no lugar do Noise, e o vinculo ao canal (o `amarrar_canal` do
//! login e a prova do pulso) passa a ser o `tls-exporter` da RFC 9266.
//!
//! 1. o pulso de dois nos atravessa com o TLS como UNICA cifra (`cluster.cifra`
//!    desligada), os dois exigindo cifra no fio E prova do pulso. Defeitos
//!    repostos que este teste derruba: `proteger` ignorando o `pino_tls` (o
//!    pulso sai em claro e o `exigir` recusa); o servidor sem guardar o vinculo
//!    do TLS na sessao, ou o cliente sem manda-lo (a prova nao fecha);
//! 2. o pino errado recusa ANTES de qualquer byte da aplicacao, dizendo o pino
//!    que o servidor mostrou; o certo atende;
//! 3. login com `exigir_amarra`: pelo TLS o cliente amarra a prova ao vinculo
//!    e entra; o servidor confere contra o DELE.

mod comum;
use comum::DirTemp;

use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::hash::para_hex;
use phxsql_core::json::Json;
use phxsql_server::replica::Cliente;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token do fio tls deste teste";

/// O silencio que o cliente do teste da amarra tolera numa resposta. NAO e o
/// criterio do teste -- o criterio e a resposta chegar; isto so impede um
/// servidor pendurado de travar a suite.
///
/// Pedido 757, medido: o `usuario_criar` e o `autenticar` pagam um PBKDF2 de
/// 210.000 iteracoes cada, em debug, e o servidor fica mudo enquanto calcula.
/// Com 5 s aqui, sob 8 `yes` em 4 nucleos, 13 de 20 corridas cairam com
/// `WouldBlock` entre 5,03 e 5,23 s (as verdes, 4,16 a 4,73 s); sob 4 `yes`
/// com a carga dos vizinhos, 20 de 50. Era o relogio medindo a CPU da
/// maquina, nao a amarra.
const SILENCIO_DO_PBKDF2: Duration = Duration::from_secs(60);

/// O par TLS do no, gravado onde o servidor procura o autoassinado
/// (`tls-dados-*.pem` ao lado do `config.json`), e o pino dele em texto.
/// Gerado AQUI, e nao pelo servidor, porque cada no precisa do pino do outro
/// antes de qualquer um subir.
fn par_tls(base: &Path) -> String {
    let privada = phxsql_core::p256::gerar_privada();
    let cert = phxsql_core::x509::cert_tls_p256(
        &["localhost", "127.0.0.1"],
        &phxsql_core::x509::Validade::de_agora_por_anos(1),
        &[0x01, 0x72],
        &privada,
    )
    .unwrap();
    std::fs::write(
        base.join("tls-dados-certificado.pem"),
        phxsql_core::x509::para_pem(&cert, "CERTIFICATE"),
    )
    .unwrap();
    std::fs::write(
        base.join("tls-dados-chave.pem"),
        phxsql_core::x509::pem_da_chave_p256(&privada).unwrap(),
    )
    .unwrap();
    let spki = phxsql_core::x509::spki_do_certificado(&cert).unwrap();
    phxsql_core::tls::pino_em_texto(&phxsql_core::hash::sha256(spki))
}

fn pino(texto: &str) -> [u8; 32] {
    phxsql_core::tls::pino_de_texto(texto).unwrap()
}

/// O `chave_do_fio` (a IDENTIDADE do no para a prova do pulso) da privada.
fn publica_noise(privada_hex: &str) -> String {
    let b = phxsql_core::hash::de_hex(privada_hex).unwrap();
    let mut k = [0u8; 32];
    k.copy_from_slice(&b);
    para_hex(&phxsql_core::x25519::chave_publica(&k))
}

fn bar(p: std::path::PathBuf) -> String {
    p.display().to_string().replace('\\', "/")
}

/// Um servidor sozinho, com TLS na porta de dados e o resto de `extra`.
fn subir_so(base: &Path, extra: &str) -> (Arc<Servidor>, u16) {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let caminho = base.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:0", "base": "{}", "token": "{TOKEN}",
              "log_acessos": "{}", "seguranca": {{ "blacklist": "{}" }},
              "dblink": "{}", "jobs": "{}", "tls": true{extra}
            }}"#,
            bar(base.join("base")),
            bar(base.join("acessos.log")),
            bar(base.join("blacklist.json")),
            bar(base.join("dblink.json")),
            bar(base.join("jobs.json")),
        ),
    )
    .unwrap();
    let s = Servidor::novo(Config::ler(&caminho).unwrap()).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    (s, porta)
}

/// Um no do cluster de dois. A cifra do CLUSTER fica desligada de proposito:
/// o TLS do `pino_tls` e a unica coisa entre o pulso e o `exigir` do outro.
#[allow(clippy::too_many_arguments)]
fn subir_no(
    base: &Path,
    este: &str,
    papel: &str,
    privada_este: &str,
    ouvinte: TcpListener,
    outro: &str,
    porta_outro: u16,
    noise_outro: &str,
    tls_outro: &str,
) -> Arc<Servidor> {
    let porta_este = ouvinte.local_addr().unwrap().port();
    std::fs::create_dir_all(base.join("base")).unwrap();
    let caminho = base.join("config.json");
    let noise_este = publica_noise(privada_este);
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:{porta_este}",
              "base": "{base_dir}",
              "token": "{TOKEN}",
              "log_acessos": "{log}",
              "seguranca": {{ "blacklist": "{bl}" }},
              "dblink": "{dblink}",
              "jobs": "{jobs}",
              "web": {{ "ligado": false }},
              "tls": true,
              "cifra_fio": {{ "ligada": true, "exigir": true, "chave_privada": "{privada_este}" }},
              "replicacao": {{ "papel": "{papel}", "id_servidor": "{este}", "imagem_da_linha": true }},
              "cluster": {{
                "id": "{este}",
                "token": "{TOKEN}",
                "janela_inatividade_s": 3,
                "pulso_s": 1,
                "cifra": false,
                "exigir_prova_do_pulso": true,
                "nos": [
                  {{ "id": "{este}", "endereco": "127.0.0.1", "porta": {porta_este},
                     "chave_do_fio": "{noise_este}" }},
                  {{ "id": "{outro}", "endereco": "127.0.0.1", "porta": {porta_outro},
                     "chave_do_fio": "{noise_outro}", "pino_tls": "{tls_outro}" }}
                ]
              }}
            }}"#,
            base_dir = bar(base.join("base")),
            log = bar(base.join("acessos.log")),
            bl = bar(base.join("blacklist.json")),
            dblink = bar(base.join("dblink.json")),
            jobs = bar(base.join("jobs.json")),
        ),
    )
    .unwrap();
    let s = Servidor::novo(Config::ler(&caminho).unwrap()).unwrap();
    comum::no_ar_no_ouvinte(&s, ouvinte);
    s
}

/// Pergunta ao no, PELO TLS, se ele ve `alvo` vivo. `None` = ainda nao deu.
fn ve_vivo(porta: u16, pino_tls: &str, alvo: &str) -> Option<bool> {
    let mut c = Cliente::conectar("127.0.0.1", porta, TOKEN, Duration::from_secs(3)).ok()?;
    c.cifrar_tls(pino(pino_tls)).ok()?;
    let r = c
        .pedir(vec![("op", Json::texto_de("cluster_estado"))])
        .ok()?;
    let nos = r.campo("nos").and_then(Json::lista)?;
    Some(
        nos.iter()
            .any(|n| n.texto_ou("id", "") == alvo && n.booleano_ou("vivo", false)),
    )
}

#[test]
fn o_pulso_do_cluster_atravessa_pelo_tls_com_a_prova_amarrada_ao_exporter() {
    let priv_a = "31".repeat(32);
    let priv_b = "42".repeat(32);
    let base_a = DirTemp::novo("fio-tls-a");
    let base_b = DirTemp::novo("fio-tls-b");
    let tls_a = par_tls(&base_a);
    let tls_b = par_tls(&base_b);
    let (ouvinte_a, porta_a) = comum::ouvinte_reservado();
    let (ouvinte_b, porta_b) = comum::ouvinte_reservado();
    let _a = subir_no(
        &base_a,
        "noA",
        "source",
        &priv_a,
        ouvinte_a,
        "noB",
        porta_b,
        &publica_noise(&priv_b),
        &tls_b,
    );
    let _b = subir_no(
        &base_b,
        "noB",
        "replica",
        &priv_b,
        ouvinte_b,
        "noA",
        porta_a,
        &publica_noise(&priv_a),
        &tls_a,
    );

    let ate = Instant::now() + Duration::from_secs(15);
    let (mut a_ve_b, mut b_ve_a) = (false, false);
    while Instant::now() < ate {
        a_ve_b = ve_vivo(porta_a, &tls_a, "noB").unwrap_or(false);
        b_ve_a = ve_vivo(porta_b, &tls_b, "noA").unwrap_or(false);
        if a_ve_b && b_ve_a {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(
        a_ve_b && b_ve_a,
        "os nos nao se enxergaram pelo TLS (A ve B: {a_ve_b}, B ve A: {b_ve_a}): \
         o pulso nao foi por TLS, ou a prova nao fechou contra o vinculo"
    );
}

#[test]
fn o_pino_errado_recusa_dizendo_o_que_o_servidor_mostrou_e_o_certo_atende() {
    let d = DirTemp::novo("fio-tls-pino");
    let certo = par_tls(&d);
    let (_s, porta) = subir_so(&d, "");

    let mut c = Cliente::conectar("127.0.0.1", porta, TOKEN, Duration::from_secs(5)).unwrap();
    let e = c
        .cifrar_tls([0x5a; 32])
        .expect_err("o pino errado passou")
        .to_string();
    assert!(e.contains(&certo), "a recusa nao diz o pino visto: {e}");

    let mut c = Cliente::conectar("127.0.0.1", porta, TOKEN, Duration::from_secs(5)).unwrap();
    // `proteger` com os dois: o pino TLS decide, a `cifra` nao.
    c.proteger(true, None, Some(pino(&certo))).unwrap();
    assert!(c.tls(), "com pino_tls a conexao nao foi TLS");
    let r = c.pedir(vec![("op", Json::texto_de("ping"))]).unwrap();
    assert!(!matches!(r, Json::Nulo), "o ping pelo TLS nao respondeu");
    assert!(
        c.cifrar(None).is_err(),
        "um Noise por dentro do TLS foi aceito: uma cifra por conexao"
    );
}

#[test]
fn com_amarra_exigida_o_login_pelo_tls_amarra_ao_vinculo_e_entra() {
    let d = DirTemp::novo("fio-tls-amarra");
    let certo = par_tls(&d);
    // O `usuario_criar` de administrador e da lista de perigo (765/767); a
    // prova e da amarra do login ao vinculo do TLS.
    let (_s, porta) = subir_so(
        &d,
        r#", "cifra_fio": { "exigir_amarra": true }, "protecao": { "ligada": false }"#,
    );
    const SENHA: &str = "Senha-Do-Teste-572-T6b2";

    let mut admin = Cliente::conectar("127.0.0.1", porta, TOKEN, SILENCIO_DO_PBKDF2).unwrap();
    admin.cifrar_tls(pino(&certo)).unwrap();
    admin
        .pedir(vec![
            ("op", Json::texto_de("usuario_criar")),
            ("login", Json::texto_de("ana")),
            ("senha", Json::texto_de(SENHA)),
            // O primeiro usuario tem de ser supervisor: o cadastro recusa
            // ficar sem nenhum.
            ("supervisor", Json::Bool(true)),
        ])
        .unwrap();

    let mut c = Cliente::conectar("127.0.0.1", porta, TOKEN, SILENCIO_DO_PBKDF2).unwrap();
    c.cifrar_tls(pino(&certo)).unwrap();
    assert!(c.transcricao().is_some(), "o TLS nao deu vinculo ao canal");
    c.autenticar("ana", "", SENHA)
        .expect("o login amarrado ao tls-exporter foi recusado");
    let quem = c.pedir(vec![("op", Json::texto_de("quem_sou"))]).unwrap();
    assert!(
        quem.escrever().contains("ana"),
        "a sessao nao e da ana: {}",
        quem.escrever()
    );
}

//! A cifra do CLUSTER, provada PELO SOQUETE.
//!
//! # Por que soquete, e nao teste de unidade
//!
//! Os testes de unidade provam que o `Cluster` le o campo `cifra` e o pino, e
//! que a `origem_do_master` os carrega. Nenhum deles prova o que este arquivo
//! prova: que o PULSO da eleicao, que sobe numa thread e fala pela `Cliente`
//! de replicacao, mesmo APERTA a mao antes de mandar o `cluster_pulso`. Isso
//! depende do laco vivo e do sistema operacional, e a licao da casa e uma so:
//! teste de unidade nao prova queda nem tunel de conexao, soquete prova.
//!
//! # O desenho da prova, e por que os DOIS nos exigem tunel
//!
//! Dois nos, os dois com `cifra_fio.exigir: true` -- recusam qualquer pedido
//! em claro -- e `cluster.cifra: true`. Com a cifra do pulso no lugar, cada no
//! aperta a mao antes de pulsar o outro, o pulso atravessa, e cada um ve o
//! outro VIVO no `cluster_estado`. Com o defeito reposto (o pulso saindo em
//! claro), o pulso de A bate no `exigir` de B e cai, o de B bate no de A e
//! cai, e NENHUM dos dois enxerga o outro. Se so um no exigisse, o pulso em
//! claro do outro sentido ainda registraria o par -- e o defeito passaria
//! despercebido. Por isso os dois exigem.
//!
//! O pino de cada no e a chave publica que sai da sua privada estatica, e a
//! privada esta no `config.json` do teste (`cifra_fio.chave_privada`) para o
//! pino ser DETERMINISTICO -- senao cada arranque geraria uma estatica nova e
//! nao haveria o que pinar.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::base64;
use phxsql_core::fio::{Canal, Iniciador, Recebido};
use phxsql_core::hash::para_hex;
use phxsql_core::json::Json;
use phxsql_core::x25519;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token do cluster cifrado deste teste";

/// Faixa PROPRIA (7250-7299): a `cifra-do-fio` mora em 7200-7249, e dois
/// binarios de teste rodando juntos nao podem disputar a mesma porta.
static PROXIMA: AtomicU16 = AtomicU16::new(7250);

fn porta_livre() -> u16 {
    loop {
        let porta = PROXIMA.fetch_add(1, Ordering::SeqCst);
        assert!(porta < 7299, "acabaram as portas entre 7250 e 7298");
        if let Ok(l) = TcpListener::bind(("127.0.0.1", porta)) {
            drop(l);
            return porta;
        }
    }
}

/// O pino de um no: a publica que corresponde a esta privada estatica.
fn pino_de(privada_hex: &str) -> String {
    let bytes = phxsql_core::hash::de_hex(privada_hex).unwrap();
    let mut k = [0u8; 32];
    k.copy_from_slice(&bytes);
    para_hex(&x25519::chave_publica(&k))
}

/// Escreve o `config.json` de UM no do cluster de dois e sobe o servidor.
///
/// `este`/`outro` sao os ids, e cada no leva no `cluster.nos` o pino do OUTRO
/// (e o proprio, que nao atrapalha). Os dois exigem tunel.
#[allow(clippy::too_many_arguments)]
fn subir_no(
    base: &std::path::Path,
    este: &str,
    papel: &str,
    privada_este: &str,
    porta_este: u16,
    outro: &str,
    porta_outro: u16,
    pino_outro: &str,
) -> Arc<Servidor> {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let caminho = base.join("config.json");
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    let pino_este = pino_de(privada_este);
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
              "cifra_fio": {{ "ligada": true, "exigir": true, "chave_privada": "{privada_este}" }},
              "replicacao": {{ "papel": "{papel}", "id_servidor": "{este}", "imagem_da_linha": true }},
              "cluster": {{
                "id": "{este}",
                "token": "{TOKEN}",
                "janela_inatividade_s": 3,
                "pulso_s": 1,
                "cifra": true,
                "nos": [
                  {{ "id": "{este}", "endereco": "127.0.0.1", "porta": {porta_este}, "chave_do_fio": "{pino_este}" }},
                  {{ "id": "{outro}", "endereco": "127.0.0.1", "porta": {porta_outro}, "chave_do_fio": "{pino_outro}" }}
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
    no_ar(
        Servidor::novo(Config::ler(&caminho).unwrap()).unwrap(),
        porta_este,
    )
}

/// Poe o servidor no ar e espera a porta atender -- por CONDICAO, nao por
/// tempo fixo.
fn no_ar(s: Arc<Servidor>, porta: u16) -> Arc<Servidor> {
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return s;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("o no nao subiu na porta {porta}");
}

/// Pergunta ao no da `porta`, POR DENTRO DO TUNEL (o no exige), se ele enxerga
/// `alvo` vivo. `pino` e a chave publica esperada deste no.
///
/// Devolve `None` quando o aperto ou a consulta falham -- o chamador esta num
/// laco de espera, e "ainda nao" nao e "nunca".
fn ve_vivo(porta: u16, pino: &str, alvo: &str) -> Option<bool> {
    let alvo_addr: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo_addr, Duration::from_secs(2)).ok()?;
    fluxo.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    fluxo.set_write_timeout(Some(Duration::from_secs(3))).ok()?;
    let mut escrita = fluxo.try_clone().ok()?;
    let mut leitor = BufReader::new(fluxo);

    // O aperto: mesma coreografia de um cliente qualquer.
    let bytes = phxsql_core::hash::de_hex(pino)?;
    let mut p = [0u8; 32];
    p.copy_from_slice(&bytes);
    let (iniciador, m1) = Iniciador::comecar(Some(p));
    writeln!(
        escrita,
        r#"{{"op":"cifrar","e":"{}"}}"#,
        base64::codificar(&m1)
    )
    .ok()?;
    escrita.flush().ok()?;
    let mut resp = String::new();
    leitor.read_line(&mut resp).ok()?;
    let j = Json::analisar(&resp).ok()?;
    if !j.booleano_ou("ok", false) {
        return None;
    }
    let m2 = base64::decodificar(
        j.campo("resultado")
            .map(|r| r.texto_ou("m2", ""))
            .unwrap_or(""),
    )
    .ok()?;
    let (transporte, _) = iniciador.terminar(&m2).ok()?;
    let mut canal = Canal::Cifrado(Box::new(transporte));

    // A consulta, ja por dentro do tunel.
    canal
        .escrever(
            &mut escrita,
            &format!(r#"{{"token":"{TOKEN}","op":"cluster_estado"}}"#),
        )
        .ok()?;
    let linha = match canal.ler(&mut leitor).ok()? {
        Recebido::Linha(l) => l,
        Recebido::Fim => return None,
    };
    let r = Json::analisar(&linha).ok()?;
    let nos = r
        .campo("resultado")
        .and_then(|res| res.campo("nos"))
        .and_then(Json::lista)?;
    for n in nos {
        if n.texto_ou("id", "") == alvo {
            return Some(n.booleano_ou("vivo", false));
        }
    }
    Some(false)
}

/// **A prova real.** Dois nos que exigem tunel se enxergam VIVOS -- e so se
/// enxergam porque o pulso da eleicao apertou a mao antes de falar. Com o
/// defeito reposto (a `guarda pulso-do-cluster-em-claro` tira o `cifrar` do
/// `pulsar`), o pulso sai em claro, cada `exigir` o recusa, e nenhum no
/// aparece vivo no outro: este teste entao ESTOURA o prazo e cai.
#[test]
fn pulso_do_cluster_cifrado_atravessa_no_que_exige_tunel() {
    let priv_a = "11".repeat(32);
    let priv_b = "22".repeat(32);
    let pino_a = pino_de(&priv_a);
    let pino_b = pino_de(&priv_b);

    let base_a = DirTemp::novo("cluster-cif-a");
    let base_b = DirTemp::novo("cluster-cif-b");
    let porta_a = porta_livre();
    let porta_b = porta_livre();

    // A = master (source), B = replica; os dois cifram e exigem.
    let _a = subir_no(
        &base_a, "noA", "source", &priv_a, porta_a, "noB", porta_b, &pino_b,
    );
    let _b = subir_no(
        &base_b, "noB", "replica", &priv_b, porta_b, "noA", porta_a, &pino_a,
    );

    // Espera a convergencia: cada no pulsa o outro a cada 1s. Bem dentro do
    // prazo quando a cifra do pulso esta no lugar; estoura quando nao esta.
    let ate = Instant::now() + Duration::from_secs(15);
    let mut a_ve_b = false;
    let mut b_ve_a = false;
    while Instant::now() < ate {
        a_ve_b = ve_vivo(porta_a, &pino_a, "noB").unwrap_or(false);
        b_ve_a = ve_vivo(porta_b, &pino_b, "noA").unwrap_or(false);
        if a_ve_b && b_ve_a {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    assert!(
        a_ve_b,
        "noA nao enxergou noB vivo: o pulso cifrado nao atravessou o `exigir`"
    );
    assert!(
        b_ve_a,
        "noB nao enxergou noA vivo: o pulso cifrado nao atravessou o `exigir`"
    );
}

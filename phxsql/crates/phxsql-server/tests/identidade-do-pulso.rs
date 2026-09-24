//! A identidade de quem manda o pulso do cluster -- pedido 278 (SEC A1).
//!
//! # Por que soquete, e nao teste de unidade
//!
//! O dano do A1 nao esta numa funcao: esta na corrente inteira -- o pulso
//! entra pela porta de dados, o arbitro le o mapa meio segundo depois,
//! `rebaixar` grava o `cluster.estado.json`, e esse arquivo GANHA do
//! `config.json` no arranque seguinte. Uma prova de unidade sobre
//! `EstadoCluster::registrar` veria o mapa mudar e nao veria o master cair
//! nem a paralisia sobreviver ao reinicio, que e o que o pedido descreve.
//!
//! # O vermelho, medido antes do conserto
//!
//! Com o mesmo desenho deste arquivo, uma linha so --
//! `{"token":…,"op":"cluster_pulso","id":"noB","papel":"master","epoca":9}` --
//! bastava. O log do `noA` dizia, em 0,53 s:
//!
//! ```text
//! cluster: ha epoca 9 no ar e a minha e 0 -- houve eleicao enquanto este no
//!          esteve fora; REBAIXANDO a replica
//! MEDIDO cluster.estado.json: {"papel":"replica","epoca":9}
//! ```
//!
//! # O desenho, e por que UM no de pe
//!
//! A lista tem tres nos e so o `noA` sobe. E deliberado: sobra UM vivo de
//! tres, nunca ha maioria, e nenhuma eleicao legitima devolve ou tira o papel
//! do `noA` no meio da medida. O teste mede o que quer medir -- o efeito do
//! PULSO -- sem depender de tempo.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::hash::para_hex;
use phxsql_core::json::Json;
use phxsql_core::x25519;
use phxsql_server::pulso::{self, Assinado};
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "o token do cluster desta prova";

/// A privada estatica do `noA` -- fixa para o pino dele ser deterministico.
const PRIV_A: &str = "aa";
/// A privada do `noB`, que aqui e o no LEGITIMO simulado pelo teste.
const PRIV_B: &str = "bb";
/// A privada de um terceiro que tem a credencial do cluster e quer se passar
/// pelo `noB`.
const PRIV_INTRUSO: &str = "cc";

/// Faixa PROPRIA (7400-7449): dois binarios de teste rodando juntos nao podem
/// disputar porta.
static PROXIMA: AtomicU16 = AtomicU16::new(7400);

fn porta_livre() -> u16 {
    loop {
        let porta = PROXIMA.fetch_add(1, Ordering::SeqCst);
        assert!(porta < 7449, "acabaram as portas entre 7400 e 7448");
        if let Ok(l) = TcpListener::bind(("127.0.0.1", porta)) {
            drop(l);
            return porta;
        }
    }
}

fn privada(semente: &str) -> [u8; 32] {
    let bytes = phxsql_core::hash::de_hex(&semente.repeat(32)).unwrap();
    let mut k = [0u8; 32];
    k.copy_from_slice(&bytes);
    k
}

fn publica(semente: &str) -> [u8; 32] {
    x25519::chave_publica(&privada(semente))
}

/// Escreve o `config.json` do `noA` -- master de um cluster de TRES -- e sobe.
///
/// `exigir` liga `cluster.exigir_prova_do_pulso`; `pino_b` e o `chave_do_fio`
/// do `noB` na lista do `noA` (vazio = no sem pino, como um cluster que nunca
/// configurou um).
fn subir_no_a(base: &std::path::Path, porta: u16, exigir: bool, pino_b: &str) -> Arc<Servidor> {
    subir_no_a_com_b_em(base, porta, exigir, pino_b, 7498)
}

/// O mesmo `noA`, com o `noB` num endereco escolhido -- o do par FALSO que
/// o teste do pedido 441 poe para responder ao pulso.
fn subir_no_a_com_b_em(
    base: &std::path::Path,
    porta: u16,
    exigir: bool,
    pino_b: &str,
    porta_b: u16,
) -> Arc<Servidor> {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let caminho = base.join("config.json");
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    let campo_pino = if pino_b.is_empty() {
        String::new()
    } else {
        format!(r#", "chave_do_fio": "{pino_b}""#)
    };
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:{porta}",
              "base": "{base_dir}",
              "token": "{TOKEN}",
              "log_acessos": "{log}",
              "seguranca": {{ "blacklist": "{bl}" }},
              "dblink": "{dblink}",
              "jobs": "{jobs}",
              "web": {{ "ligado": false }},
              "cifra_fio": {{ "ligada": true, "exigir": false, "chave_privada": "{priv_a}" }},
              "replicacao": {{ "papel": "source", "id_servidor": "noA", "imagem_da_linha": true }},
              "cluster": {{
                "id": "noA",
                "token": "{TOKEN}",
                "janela_inatividade_s": 3,
                "pulso_s": 1,
                "cifra": false,
                "exigir_prova_do_pulso": {exigir},
                "nos": [
                  {{ "id": "noA", "endereco": "127.0.0.1", "porta": {porta} }},
                  {{ "id": "noB", "endereco": "127.0.0.1", "porta": {porta_b}{campo_pino} }},
                  {{ "id": "noC", "endereco": "127.0.0.1", "porta": 7499 }}
                ]
              }}
            }}"#,
            base_dir = bar(base.join("base")),
            log = bar(base.join("acessos.log")),
            bl = bar(base.join("blacklist.json")),
            dblink = bar(base.join("dblink.json")),
            jobs = bar(base.join("jobs.json")),
            priv_a = PRIV_A.repeat(32),
        ),
    )
    .unwrap();
    no_ar(
        Servidor::novo(Config::ler(&caminho).unwrap()).unwrap(),
        porta,
    )
}

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

/// Manda UMA linha do protocolo, em claro, e devolve a resposta analisada.
fn falar(porta: u16, pedido: &str) -> Json {
    Json::analisar(&falar_cru(porta, pedido)).unwrap()
}

/// A resposta CRUA, como o fio a entrega -- o tamanho dela e um canal (435
/// reaberto), e so a linha crua o mede.
fn falar_cru(porta: u16, pedido: &str) -> String {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
    fluxo
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{pedido}").unwrap();
    escrita.flush().unwrap();
    let mut resp = String::new();
    leitor.read_line(&mut resp).unwrap();
    resp.trim_end().to_string()
}

/// O papel VIVO que o no declara, e a epoca dele.
fn papel_e_epoca(porta: u16) -> (String, i64) {
    let r = falar(
        porta,
        &format!(r#"{{"token":"{TOKEN}","op":"cluster_estado"}}"#),
    );
    let res = r.campo("resultado").expect("sem resultado");
    (
        res.texto_ou("papel", "?").to_string(),
        res.inteiro_ou("epoca", -1),
    )
}

/// Um pulso de `noB` para `noA`, com ou sem prova.
///
/// `assinante` e a privada de quem assina -- e e o parametro que separa o no
/// legitimo do intruso: os dois montam o MESMO corpo, e so a chave difere.
///
/// `nonce` so viaja com a prova, e tem de ser do formato do emissor
/// (`pulso::nonce()`) desde o pedido 436: um nonce torto e recusado pela
/// antirrepeticao, e o teste que o usasse passaria a medir o crivo do M1 em
/// vez do que diz medir.
fn pulso_de_b(papel: &str, epoca: u64, assinante: Option<&str>, nonce: &str) -> String {
    pulso_de("noB", papel, epoca, assinante, nonce)
}

/// O mesmo pulso, de QUALQUER par da lista -- o `pulso_de_b` e este com o id
/// preso. Um motor so: dois montadores de pulso divergiriam no dia em que um
/// campo entrasse na mensagem assinada, e a prova de um passaria a nao fechar
/// pelo motivo errado.
fn pulso_de(id: &str, papel: &str, epoca: u64, assinante: Option<&str>, nonce: &str) -> String {
    let mut campos = format!(
        r#""op":"cluster_pulso","id":"{id}","papel":"{papel}","epoca":{epoca},"posicao":0,"incompleta":false,"prioridade":0"#
    );
    if let Some(semente) = assinante {
        let quando = phxsql_server::agora_ms();
        let prova = pulso::assinar(
            &privada(semente),
            &publica(PRIV_A),
            &Assinado {
                de: id,
                para: "noA",
                papel,
                epoca,
                posicao: 0,
                incompleta: false,
                prioridade: 0,
                quando,
                nonce,
            },
            None,
        )
        .unwrap();
        campos.push_str(&format!(
            r#","para":"noA","quando":{quando},"nonce":"{nonce}","prova":"{prova}""#
        ));
    }
    format!(r#"{{"token":"{TOKEN}",{campos}}}"#)
}

/// O `noB` aparece vivo no mapa do `noA`?
fn ve_vivo(porta: u16, alvo: &str) -> bool {
    let r = falar(
        porta,
        &format!(r#"{{"token":"{TOKEN}","op":"cluster_estado"}}"#),
    );
    r.campo("resultado")
        .and_then(|res| res.campo("nos"))
        .and_then(Json::lista)
        .map(|nos| {
            nos.iter()
                .any(|n| n.texto_ou("id", "") == alvo && n.booleano_ou("vivo", false))
        })
        .unwrap_or(false)
}

/// **O conserto do 278.** Um pulso que se DECLARA `noB` -- sem provar nada --
/// nao derruba mais o master, e a epoca forjada nao entra em lugar nenhum.
///
/// Com o `conferir_identidade` tirado do `op_cluster_pulso`, este teste volta
/// a ver `replica` e `epoca 9`, que foi o que a medida de antes registrou.
#[test]
fn um_pulso_forjado_nao_destrona_o_master() {
    let base = DirTemp::novo("identidade-pulso-forjado");
    let porta = porta_livre();
    let _a = subir_no_a(&base, porta, true, &para_hex(&publica(PRIV_B)));

    assert_eq!(papel_e_epoca(porta), ("master".into(), 0));

    // (1) sem prova nenhuma, com a credencial do cluster na mao.
    let r = falar(porta, &pulso_de_b("master", 9, None, "n1"));
    assert!(
        !r.booleano_ou("ok", true),
        "o pulso SEM prova passou: {}",
        r.escrever()
    );
    assert_eq!(r.texto_ou("nome", ""), "ACESSO_NEGADO", "{}", r.escrever());

    // (2) com uma prova assinada por QUEM NAO E o noB -- o intruso tem a
    // credencial do cluster e esta no cluster, e ainda assim nao fecha.
    let r = falar(
        porta,
        &pulso_de_b("master", 9, Some(PRIV_INTRUSO), &pulso::nonce()),
    );
    assert!(
        !r.booleano_ou("ok", true),
        "o pulso com prova de outra chave passou: {}",
        r.escrever()
    );

    // (3) o master continua de pe, e o arquivo que ganha do config tambem.
    std::thread::sleep(Duration::from_millis(1_500));
    let (papel, epoca) = papel_e_epoca(porta);
    assert_eq!(
        papel, "master",
        "o master caiu com um pulso que nao provou nada"
    );
    assert_eq!(
        epoca, 0,
        "a epoca forjada entrou mesmo com o pulso recusado"
    );
    let estado = base.join("base").join("cluster.estado.json");
    if estado.exists() {
        let gravado = std::fs::read_to_string(&estado).unwrap();
        assert!(
            !gravado.contains("\"epoca\":9"),
            "a epoca forjada foi parar no arquivo: {gravado}"
        );
    }
}

/// O caminho LEGITIMO continua andando: o `noB` de verdade assina, o pulso
/// conta, e o mapa do `noA` passa a ve-lo vivo.
///
/// Sem este teste, «recusar tudo» passaria como conserto.
#[test]
fn o_pulso_com_prova_valida_passa_e_conta() {
    let base = DirTemp::novo("identidade-pulso-legitimo");
    let porta = porta_livre();
    let _a = subir_no_a(&base, porta, true, &para_hex(&publica(PRIV_B)));

    let r = falar(
        porta,
        &pulso_de_b("replica", 0, Some(PRIV_B), &pulso::nonce()),
    );
    assert!(
        r.booleano_ou("ok", false),
        "o pulso legitimo foi recusado: {}",
        r.escrever()
    );
    assert!(ve_vivo(porta, "noB"), "o noB nao entrou no mapa do noA");

    // E a RESPOSTA tambem prova quem a manda -- o irmao da conferencia, para
    // o outro lado nao registrar um `noA` que nao e o noA.
    let res = r.campo("resultado").expect("sem resultado");
    assert_eq!(res.texto_ou("id", ""), "noA");
    assert!(
        !res.texto_ou("prova", "").is_empty(),
        "a resposta do pulso saiu SEM prova: {}",
        res.escrever()
    );
    assert_eq!(res.texto_ou("para", ""), "noB");
}

/// Gravar um pulso legitimo e toca-lo de novo nao conta duas vezes.
///
/// E o que impede um atacante de renovar "vi o master agora" para sempre com
/// uma gravacao -- a prova diz QUEM mandou, e o nonce diz que nao e de antes.
#[test]
fn um_pulso_repetido_nao_conta_duas_vezes() {
    let base = DirTemp::novo("identidade-pulso-repetido");
    let porta = porta_livre();
    let _a = subir_no_a(&base, porta, true, &para_hex(&publica(PRIV_B)));

    let gravado = pulso_de_b("replica", 0, Some(PRIV_B), &pulso::nonce());
    let primeiro = falar(porta, &gravado);
    assert!(primeiro.booleano_ou("ok", false), "{}", primeiro.escrever());

    let repetido = falar(porta, &gravado);
    assert!(
        !repetido.booleano_ou("ok", true),
        "o MESMO pulso passou duas vezes: {}",
        repetido.escrever()
    );
    assert!(
        repetido.texto_ou("erro", "").contains("repete"),
        "a recusa nao disse que era repeticao: {}",
        repetido.escrever()
    );
}

/// **O COMPORTAMENTO VELHO, e e o teste que mais importa aqui.**
///
/// Um no de versao anterior nao sabe assinar pulso nenhum. Sem
/// `exigir_prova_do_pulso` e sem ninguem ter provado nada ainda, o pulso dele
/// continua passando exatamente como passava -- inclusive movendo a epoca,
/// que e o comportamento de sempre. Guarda nova entra PEDIDA.
#[test]
fn sem_exigencia_o_no_de_versao_anterior_continua_pulsando() {
    let base = DirTemp::novo("identidade-pulso-velho");
    let porta = porta_livre();
    // Sem pino do noB e sem exigencia: o cluster de ontem, inteiro.
    let _a = subir_no_a(&base, porta, false, "");

    let r = falar(porta, &pulso_de_b("master", 9, None, "n1"));
    assert!(
        r.booleano_ou("ok", false),
        "o pulso sem prova de um no sem pino foi recusado -- isto quebraria \
         todo cluster escrito antes desta guarda: {}",
        r.escrever()
    );

    // E ele continua CONTANDO como contava: o master se rebaixa diante de uma
    // epoca maior. E o comportamento de sempre, com todos os riscos de sempre
    // -- que e por isso que o interruptor existe.
    let ate = Instant::now() + Duration::from_secs(5);
    let mut papel = String::new();
    while Instant::now() < ate {
        papel = papel_e_epoca(porta).0;
        if papel == "replica" {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        papel, "replica",
        "o comportamento velho mudou sem ninguem pedir"
    );
}

/// O TOFU do pulso: sem interruptor nenhum, um no que JA provou uma vez nao
/// volta a ser aceito sem prova.
///
/// E o que fecha o buraco para quem nunca ligou o interruptor -- bastaria
/// OMITIR o campo `prova` para desligar a guarda, e omitir e exatamente o que
/// um forjador faz.
#[test]
fn depois_que_o_no_provou_o_pulso_sem_prova_e_recusado() {
    let base = DirTemp::novo("identidade-pulso-tofu");
    let porta = porta_livre();
    // Interruptor DESLIGADO de propósito: o que morde aqui e so o TOFU.
    let _a = subir_no_a(&base, porta, false, &para_hex(&publica(PRIV_B)));

    // (1) antes de qualquer prova, o pulso sem prova passa -- comportamento
    // velho intacto.
    let r = falar(porta, &pulso_de_b("replica", 0, None, "n0"));
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());

    // (2) o noB prova uma vez.
    let r = falar(
        porta,
        &pulso_de_b("replica", 0, Some(PRIV_B), &pulso::nonce()),
    );
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());

    // (3) e dali em diante, pulso sem prova daquele id nao entra mais.
    let r = falar(porta, &pulso_de_b("master", 9, None, "n2"));
    assert!(
        !r.booleano_ou("ok", true),
        "o rebaixamento silencioso passou: {}",
        r.escrever()
    );
    assert_eq!(papel_e_epoca(porta), ("master".into(), 0));
}

// ---------------------------------------------------------------------------
// O cluster de verdade, com a exigencia LIGADA
// ---------------------------------------------------------------------------

/// Sobe UM no de um cluster de DOIS, cifrado, com pino do outro e a exigencia
/// de prova ligada.
#[allow(clippy::too_many_arguments)]
fn subir_par(
    base: &std::path::Path,
    este: &str,
    papel: &str,
    priv_este: &str,
    porta_este: u16,
    outro: &str,
    porta_outro: u16,
    pino_outro: &str,
) -> Arc<Servidor> {
    std::fs::create_dir_all(base.join("base")).unwrap();
    let caminho = base.join("config.json");
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    let pino_este = para_hex(&publica(priv_este));
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
              "cifra_fio": {{ "ligada": true, "exigir": true, "chave_privada": "{priv_hex}" }},
              "replicacao": {{ "papel": "{papel}", "id_servidor": "{este}", "imagem_da_linha": true }},
              "cluster": {{
                "id": "{este}",
                "token": "{TOKEN}",
                "janela_inatividade_s": 3,
                "pulso_s": 1,
                "cifra": true,
                "exigir_prova_do_pulso": true,
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
            priv_hex = priv_este.repeat(32),
        ),
    )
    .unwrap();
    no_ar(
        Servidor::novo(Config::ler(&caminho).unwrap()).unwrap(),
        porta_este,
    )
}

/// **A prova que impede o conserto de virar estrago.** Dois nos de verdade,
/// cifrados, com pino um do outro e `exigir_prova_do_pulso` ligado nos dois:
/// eles continuam se enxergando vivos.
///
/// # O que so este teste alcanca
///
/// A prova do pulso entra na mensagem amarrada a TRANSCRICAO do tunel quando
/// ha tunel. Se a transcricao que o iniciador usa para assinar nao fosse
/// byte a byte a que o respondedor usa para conferir, nenhuma prova fecharia
/// -- e com a exigencia ligada o cluster cifrado, que e o padrao desde
/// 18/09/2026, pararia INTEIRO. Nem o teste de unidade da prova nem o do
/// pulso forjado (que fala em claro) veem isso: so dois nos de pe veem.
///
/// E ele cobre os DOIS sentidos de uma vez: o pedido que sai do `pulsar` e a
/// RESPOSTA que volta, que o `pulsar` tambem confere antes de registrar.
#[test]
fn dois_nos_cifrados_com_exigencia_ligada_continuam_se_enxergando() {
    let base_a = DirTemp::novo("identidade-par-a");
    let base_b = DirTemp::novo("identidade-par-b");
    let porta_a = porta_livre();
    let porta_b = porta_livre();
    let pino_a = para_hex(&publica(PRIV_A));
    let pino_b = para_hex(&publica(PRIV_B));

    let _a = subir_par(
        &base_a, "noA", "source", PRIV_A, porta_a, "noB", porta_b, &pino_b,
    );
    let _b = subir_par(
        &base_b, "noB", "replica", PRIV_B, porta_b, "noA", porta_a, &pino_a,
    );

    let ate = Instant::now() + Duration::from_secs(20);
    let mut a_ve_b = false;
    let mut b_ve_a = false;
    while Instant::now() < ate {
        a_ve_b = ve_vivo_no_tunel(porta_a, PRIV_A, "noB");
        b_ve_a = ve_vivo_no_tunel(porta_b, PRIV_B, "noA");
        if a_ve_b && b_ve_a {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(
        a_ve_b,
        "o noA nao enxergou o noB: a prova do pulso nao atravessou o tunel"
    );
    assert!(
        b_ve_a,
        "o noB nao enxergou o noA: a prova da RESPOSTA nao fechou deste lado"
    );
}

/// `cluster_estado` por DENTRO do tunel -- os nos do par exigem aperto.
fn ve_vivo_no_tunel(porta: u16, priv_do_no: &str, alvo: &str) -> bool {
    use phxsql_core::base64;
    use phxsql_core::fio::{Canal, Iniciador, Recebido};

    let endereco: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let Ok(fluxo) = TcpStream::connect_timeout(&endereco, Duration::from_secs(2)) else {
        return false;
    };
    let _ = fluxo.set_read_timeout(Some(Duration::from_secs(3)));
    let Ok(mut escrita) = fluxo.try_clone() else {
        return false;
    };
    let mut leitor = BufReader::new(fluxo);
    let (iniciador, m1) = Iniciador::comecar(Some(publica(priv_do_no)));
    if writeln!(
        escrita,
        r#"{{"op":"cifrar","e":"{}"}}"#,
        base64::codificar(&m1)
    )
    .is_err()
    {
        return false;
    }
    let _ = escrita.flush();
    let mut resp = String::new();
    if leitor.read_line(&mut resp).is_err() {
        return false;
    }
    let Ok(j) = Json::analisar(&resp) else {
        return false;
    };
    let Ok(m2) = base64::decodificar(
        j.campo("resultado")
            .map(|r| r.texto_ou("m2", ""))
            .unwrap_or(""),
    ) else {
        return false;
    };
    let Ok((transporte, _)) = iniciador.terminar(&m2) else {
        return false;
    };
    let mut canal = Canal::Cifrado(Box::new(transporte));
    if canal
        .escrever(
            &mut escrita,
            &format!(r#"{{"token":"{TOKEN}","op":"cluster_estado"}}"#),
        )
        .is_err()
    {
        return false;
    }
    let Ok(Recebido::Linha(linha)) = canal.ler(&mut leitor) else {
        return false;
    };
    let Ok(r) = Json::analisar(&linha) else {
        return false;
    };
    r.campo("resultado")
        .and_then(|res| res.campo("nos"))
        .and_then(Json::lista)
        .map(|nos| {
            nos.iter()
                .any(|n| n.texto_ou("id", "") == alvo && n.booleano_ou("vivo", false))
        })
        .unwrap_or(false)
}

/// A resposta sem o `ms`, que e RELOGIO e nao texto -- os dois se comparam
/// separados, porque um cai por carga da maquina e o outro nunca.
fn sem_o_relogio(j: &Json) -> String {
    let texto = j.escrever();
    let Some(inicio) = texto.find(r#","ms":"#) else {
        return texto;
    };
    let resto = &texto[inicio + 6..];
    let fim = resto
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(resto.len());
    format!("{}{}", &texto[..inicio], &resto[fim..])
}

/// **Prova real do pedido 435 (SEC A2, 23/09/2026).** A recusa da prova do
/// pulso nao diz QUAIS nos tem pino neste no -- nem pela frase, nem pelo
/// relogio que vem ao lado dela.
///
/// # Por que soquete
///
/// O que o pedido acusa e o que chega AO FIO: o texto sai de `cluster.rs`,
/// atravessa `texto_do_erro` e a montagem da resposta, e so ali vira o `erro`
/// que o atacante le -- e so ali ganha o campo `ms`, que um teste de unidade
/// sobre `conferir_identidade` nao veria existir.
///
/// # O desenho
///
/// O `noA` sobe com a lista de sempre: o `noB` COM `chave_do_fio` e o `noC`
/// SEM. Os dois recebem um pulso com prova assinada por quem nao e eles. As
/// duas respostas tem de ser a mesma, trocado o id -- que e o unico campo que
/// o proprio remetente mandou.
///
/// # A metade que quase escapou, medida
///
/// Colapsar so a frase comprava ZERO: com o texto ja unico, `ms` saiu **1
/// contra 0 em 40 de 40** corridas, porque o caminho sem pino voltava antes do
/// X25519 e do HMAC. O mapa tinha mudado de campo, nao sumido. Por isso o
/// `assert` do relogio esta aqui ao lado do da frase, e por isso o pino cego
/// existe no `cluster.rs`.
///
/// # O defeito reposto
///
/// Duas formas, e a guarda cai nas duas: devolver as duas frases separadas, ou
/// voltar a recusar o no sem pino ANTES do `pulso::conferir`.
#[test]
fn o_pulso_nao_diz_quais_nos_tem_pino() {
    let base = DirTemp::novo("identidade-pulso-mapa-do-pino");
    let porta = porta_livre();
    // Fabrica: `exigir_prova_do_pulso` desligado. E a configuracao em que o
    // mapa vale ouro, porque e nela que o no sem pino ainda passa sem prova.
    let _a = subir_no_a(&base, porta, false, &para_hex(&publica(PRIV_B)));

    let com_pino = falar(
        porta,
        &pulso_de("noB", "master", 9, Some(PRIV_INTRUSO), &pulso::nonce()),
    );
    let sem_pino = falar(
        porta,
        &pulso_de("noC", "master", 9, Some(PRIV_INTRUSO), &pulso::nonce()),
    );
    assert!(
        !com_pino.booleano_ou("ok", true) && !sem_pino.booleano_ou("ok", true),
        "alguma das duas passou -- a medida perde o sentido:\n  {}\n  {}",
        com_pino.escrever(),
        sem_pino.escrever()
    );
    // A resposta INTEIRA, e nao so o campo `erro`: um `codigo` ou um `nome`
    // diferente seria o mesmo mapa por outro campo.
    assert_eq!(
        sem_o_relogio(&com_pino).replace("noB", "{id}"),
        sem_o_relogio(&sem_pino).replace("noC", "{id}"),
        "as duas recusas diferem, e a diferenca e o mapa de quais nos tem pino"
    );

    // O RELOGIO, que e o canal para onde o mapa se mudou quando a frase
    // fechou. Nao se afirma igualdade de uma amostra -- isso cairia por carga
    // da maquina. Afirma-se que a separacao SUMIU: com o defeito, 40/40
    // separavam; um classificador que ainda acerte 34 das 40 nao e ruido.
    let mut separadas = 0;
    for _ in 0..40 {
        let b = falar(
            porta,
            &pulso_de("noB", "master", 9, Some(PRIV_INTRUSO), &pulso::nonce()),
        );
        let c = falar(
            porta,
            &pulso_de("noC", "master", 9, Some(PRIV_INTRUSO), &pulso::nonce()),
        );
        if b.inteiro_ou("ms", -1) != c.inteiro_ou("ms", -2) {
            separadas += 1;
        }
    }
    assert!(
        separadas <= 34,
        "o `ms` da resposta separa o no com pino do sem pino em {separadas} de \
         40: a frase fechou e o relogio reabriu o mesmo mapa"
    );

    // O que o mapa VALIA: o `noC`, sem pino, continua sendo aceito sem prova
    // nenhuma -- e exatamente onde o 278 nao pega. Enquanto isto for verdade,
    // a frase unica nao e zelo, e a guarda.
    let sem_prova = falar(porta, &pulso_de("noC", "replica", 0, None, "m3"));
    assert!(
        sem_prova.booleano_ou("ok", false),
        "o no sem pino passou a recusar pulso sem prova: reveja o porque desta \
         guarda antes de apaga-la\n  {}",
        sem_prova.escrever()
    );
}

// ---------------------------------------------------------------------------
// A guarda inerte se anuncia -- pedido 436 (SEC M3)
// ---------------------------------------------------------------------------

/// A sonda do `aceitar_pulso_sem_prova_deixa_rastro`. So faz sentido como
/// processo FILHO: o que se mede e o stderr do servidor, e dentro da bateria
/// o `libtest` o captura.
///
/// Fabrica inteira -- `exigir_prova_do_pulso` desligado --, com o `noB` COM
/// pino aqui (e o lado de la que nao assina) e o `noC` sem. Tres pulsos sem
/// prova do `noB` e um do `noC`, todos ACEITOS: o comportamento velho e a
/// metade desta prova que nao pode mudar.
#[test]
#[ignore = "sonda: roda so reexecutada por aceitar_pulso_sem_prova_deixa_rastro"]
fn sonda_pulso_sem_prova() {
    // Do alto da faixa: a sonda roda num processo FILHO, com o contador de
    // portas zerado, ao mesmo tempo que a bateria do pai sobe nos de 7400 em
    // diante. Comecar embaixo disputaria a mesma porta com um deles.
    PROXIMA.store(7440, Ordering::SeqCst);
    let base = DirTemp::novo("identidade-pulso-sonda-inerte");
    let porta = porta_livre();
    let _a = subir_no_a(&base, porta, false, &para_hex(&publica(PRIV_B)));
    for id in ["noB", "noB", "noB", "noC"] {
        let r = falar(porta, &pulso_de(id, "replica", 0, None, ""));
        assert!(
            r.booleano_ou("ok", false),
            "o pulso sem prova de {id} foi recusado -- o comportamento velho \
             mudou: {}",
            r.escrever()
        );
    }
}

/// **Prova real do pedido 436, M3.** A guarda do 278 inerte para um par --
/// pulso sem prova, interruptor no padrao, par que nunca provou -- deixa
/// RASTRO no log do processo, e uma vez por par.
///
/// # Por que o stderr de um processo filho
///
/// O achado e de EVIDENCIA EM EXECUCAO: o `docs/CLUSTER.md` diz o alcance, e
/// documento nao e evidencia de instalacao. O que quem opera tem na mao e o
/// log do processo, entao e ele que se le -- e nao um contador que so o teste
/// enxergaria. O proprio binario roda de novo filtrado na sonda.
///
/// # Os dois vermelhos
///
/// Zero linhas e o defeito do achado: o `return Ok(())` mudo. Tres linhas
/// para o `noB` e o aviso por pulso -- o aviso perpetuo que ninguem le, e que
/// gasta a confianca do aviso verdadeiro. O certo e uma por par.
#[test]
fn aceitar_pulso_sem_prova_deixa_rastro() {
    let saida = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "sonda_pulso_sem_prova",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .output()
        .expect("reexecutar o proprio binario de teste");
    let erro = String::from_utf8_lossy(&saida.stderr);
    assert!(
        saida.status.success(),
        "a sonda nao terminou limpa:\n{erro}"
    );
    let linhas_de = |id: &str| -> Vec<&str> {
        let marca = format!("{id:?}");
        erro.lines()
            .filter(|l| l.contains("INERTE") && l.contains(&marca))
            .collect()
    };
    let (b, c) = (linhas_de("noB"), linhas_de("noC"));
    assert_eq!(
        (b.len(), c.len()),
        (1, 1),
        "a guarda inerte foi anunciada {} vez(es) para o noB (3 pulsos) e {} \
         para o noC (1 pulso) -- o certo e uma por par. stderr da sonda:\n{erro}",
        b.len(),
        c.len()
    );
    // E o diagnostico aponta o lado certo: o noB TEM pino aqui, o noC nao.
    assert!(b[0].contains("do lado de la"), "noB: {}", b[0]);
    assert!(c[0].contains("vazio NESTE no"), "noC: {}", c[0]);
}

// ---------------------------------------------------------------------------
// A RESPOSTA do pulso passa pelo mesmo crivo -- pedido 441 (SEC A1)
// ---------------------------------------------------------------------------

/// Um par FALSO no endereco do `noB`: responde a toda linha com um pulso SEM
/// prova, de papel `master` e epoca 1, dizendo-se `id_falso`. E o que um no
/// comprometido -- ou quem esta no meio de um enlace sem pino -- faz.
///
/// Devolve a porta e quantos pulsos ele recebeu: o CANARIO de que o laco do
/// `noA` chegou mesmo a ele. Sem o canario, um laco que nunca pulsasse deixaria
/// o teste verde pelo motivo errado.
fn par_que_responde_como(id_falso: &'static str) -> (u16, Arc<AtomicUsize>) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let vistos = Arc::new(AtomicUsize::new(0));
    let contador = Arc::clone(&vistos);
    std::thread::spawn(move || {
        for conexao in ouvinte.incoming() {
            let Ok(conexao) = conexao else { return };
            let contador = Arc::clone(&contador);
            std::thread::spawn(move || {
                let Ok(mut escrita) = conexao.try_clone() else {
                    return;
                };
                for linha in BufReader::new(conexao).lines() {
                    if linha.is_err() {
                        return;
                    }
                    contador.fetch_add(1, Ordering::SeqCst);
                    let resposta = format!(
                        r#"{{"ok":true,"op":"cluster_pulso","resultado":{{"id":"{id_falso}","papel":"master","epoca":1,"posicao":0,"incompleta":false,"prioridade":0}}}}"#
                    );
                    if writeln!(escrita, "{resposta}").is_err() {
                        return;
                    }
                }
            });
        }
    });
    (porta, vistos)
}

/// O corpo dos dois testes do 441: o `noA` master pulsa um `noB` que e o par
/// falso, e o master tem de continuar master, na epoca 0, no mapa E no disco.
fn o_master_resiste_a_resposta_de(id_falso: &'static str, rotulo: &str) {
    let base = DirTemp::novo(rotulo);
    let porta = porta_livre();
    let (porta_b, vistos) = par_que_responde_como(id_falso);
    // Fabrica: sem exigencia, sem pino -- o `fantasma.py` da revisao.
    let _a = subir_no_a_com_b_em(&base, porta, false, "", porta_b);
    assert_eq!(papel_e_epoca(porta), ("master".into(), 0));

    // Uma janela e meia: com o defeito, o rebaixamento veio em menos de
    // 4,5 s na medida da revisao. Cai no PRIMEIRO sinal, dizendo quantos
    // pulsos o par falso ja tinha respondido.
    let ate = Instant::now() + Duration::from_millis(4_500);
    while Instant::now() < ate {
        let (papel, epoca) = papel_e_epoca(porta);
        assert!(
            papel == "master" && epoca == 0,
            "a RESPOSTA do pulso com id {id_falso:?}, sem prova, rebaixou o \
             master: papel={papel} epoca={epoca}, depois de {} pulso(s) \
             respondido(s) pelo par falso",
            vistos.load(Ordering::SeqCst)
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    let respondidos = vistos.load(Ordering::SeqCst);
    assert!(
        respondidos >= 2,
        "o laco do noA respondeu so {respondidos} pulso(s) ao par falso: a \
         medida nao mediu nada"
    );
    assert!(
        !ve_vivo(porta, id_falso) || id_falso == "noA",
        "o id {id_falso:?} entrou no mapa como vivo"
    );
    let estado = base.join("base").join("cluster.estado.json");
    if estado.exists() {
        let gravado = std::fs::read_to_string(&estado).unwrap();
        assert!(
            !gravado.contains("replica"),
            "o rebaixamento foi parar no arquivo que ganha do config: {gravado}"
        );
    }
}

/// **Prova real do pedido 441 (SEC A1).** A resposta de um par com id que
/// NAO esta na lista, sem prova, nao rebaixa o master.
///
/// O vermelho medido pela revisao, com o crivo so no `op_cluster_pulso`:
/// `papel=replica epoca=1`, e `{"papel":"replica","epoca":1}` no
/// `cluster.estado.json`.
#[test]
fn a_resposta_de_um_no_fantasma_nao_rebaixa_o_master() {
    o_master_resiste_a_resposta_de("fantasma", "identidade-pulso-resposta-fantasma");
}

/// O irmao obrigatorio do 441: a resposta que diz ser ESTE no. O id esta na
/// lista -- e o proprio `noA` --, entao so o «e este no» do crivo a pega.
#[test]
fn a_resposta_com_o_id_deste_no_nao_rebaixa_o_master() {
    o_master_resiste_a_resposta_de("noA", "identidade-pulso-resposta-eu");
}

// ---------------------------------------------------------------------------
// O ramo SEM prova nao diz quem tem pino -- pedido 435 reaberto (SEC A2)
// ---------------------------------------------------------------------------

/// Quantas de 40 sondas pares o `ms` pode separar no ramo sem prova antes de
/// virar mapa. MEDIDO em 24/09/2026, no binario de teste: com o conserto,
/// **1, 0, 0 e 0 de 40** em quatro corridas; com a resposta que assina e so
/// esconde os campos (a variante que o `resposta-sem-prova-assina-e-esconde`
/// do catalogo repoe), **40, 40 e 40**. Dez fica dez vezes acima do pior
/// ruido visto e trinta abaixo do defeito -- e nao os 34 do teste do 435, que
/// a revisao SEC chamou de frouxos (B3).
const LIMITE_DO_RELOGIO_SEM_PROVA: usize = 10;

/// A sonda do A2, como a revisao a mandou: pulso SEM prova com uma posicao
/// que o `registrar` descarta (>= 2^53). O veredito e a resposta voltam
/// inteiros, e o mapa do cluster nao muda.
fn sonda_sem_prova(id: &str) -> String {
    format!(
        r#"{{"token":"{TOKEN}","op":"cluster_pulso","id":"{id}","papel":"replica","epoca":0,"posicao":1e16,"incompleta":false,"prioridade":0}}"#
    )
}

/// **Prova real do 435 reaberto (SEC A2).** A resposta de SUCESSO a um pulso
/// sem prova e a mesma para o no COM pino e para o SEM -- em campos, em
/// tamanho e no relogio.
///
/// O vermelho medido pela revisao: 291 B com `prova`/`nonce`/`quando`/`para`
/// para o `noB` (com pino), 137 B sem eles para o `noC`. O 435 tinha fechado
/// esse bit so no ramo de ERRO.
///
/// O relogio entra pela regua do proprio 435 -- antes de fechar um canal,
/// medir se o vizinho entrega o mesmo bit: assinar a resposta e um X25519 a
/// mais so para quem tem pino.
#[test]
fn o_pulso_sem_prova_nao_diz_quais_nos_tem_pino() {
    let base = DirTemp::novo("identidade-pulso-mapa-sem-prova");
    let porta = porta_livre();
    let _a = subir_no_a(&base, porta, false, &para_hex(&publica(PRIV_B)));

    let com_pino = falar_cru(porta, &sonda_sem_prova("noB"));
    let sem_pino = falar_cru(porta, &sonda_sem_prova("noC"));
    let (jb, jc) = (
        Json::analisar(&com_pino).unwrap(),
        Json::analisar(&sem_pino).unwrap(),
    );
    assert!(
        jb.booleano_ou("ok", false) && jc.booleano_ou("ok", false),
        "a sonda sem prova foi recusada -- a medida perde o sentido:\n  {com_pino}\n  {sem_pino}"
    );
    // A resposta INTEIRA, trocado o id e tirado o relogio: um campo a mais,
    // ou o mesmo campo com outro tamanho, e o mapa de novo.
    let (b, c) = (
        sem_o_relogio(&jb).replace("noB", "{id}"),
        sem_o_relogio(&jc).replace("noC", "{id}"),
    );
    assert_eq!(
        b,
        c,
        "a resposta de SUCESSO a um pulso sem prova difere entre o no com pino \
         ({} B) e o sem ({} B): e o bit do 435 saindo pelo ramo sem prova",
        com_pino.len(),
        sem_pino.len()
    );

    // O RELOGIO. Com as respostas iguais em forma, o que sobraria e o custo de
    // assinar uma e nao a outra.
    let mut separadas = 0;
    for _ in 0..40 {
        let b = falar(porta, &sonda_sem_prova("noB"));
        let c = falar(porta, &sonda_sem_prova("noC"));
        if b.inteiro_ou("ms", -1) != c.inteiro_ou("ms", -2) {
            separadas += 1;
        }
    }
    assert!(
        separadas <= LIMITE_DO_RELOGIO_SEM_PROVA,
        "o `ms` da resposta sem prova separa o no com pino do sem pino em \
         {separadas} de 40"
    );
}

//! As operacoes do PhxZipWeb provadas PELO SOQUETE (pedido 454, fatias Z6 a
//! Z8): envelope `PZW1`, as recusas antes do corpo (com dreno), o
//! `simultaneas`, o zip-slip e a extracao na pasta do operador.
//!
//! Como no `soquete.rs`, o cliente manda o pedido INTEIRO e so depois le --
//! e assim que o navegador faz, e e o que torna a falta do dreno visivel.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use phxsql_core::json::Json;
use phxzip_web::{Config, Servidor};

fn subir(cfg: Config) -> SocketAddr {
    let s = Servidor::escutar(cfg).expect("a porta abre");
    let end = s.endereco().unwrap();
    std::thread::spawn(move || s.servir());
    end
}

fn subir_simples() -> SocketAddr {
    subir(Config {
        porta: 0,
        ..Config::default()
    })
}

fn envelope(cabeca: &str, carga: &[u8]) -> Vec<u8> {
    let mut v = b"PZW1".to_vec();
    v.extend_from_slice(&(cabeca.len() as u32).to_le_bytes());
    v.extend_from_slice(cabeca.as_bytes());
    v.extend_from_slice(carga);
    v
}

/// Um `POST` como a tela manda, com `Origin` opcional.
fn post(end: SocketAddr, rota: &str, corpo: &[u8], extra: &str) -> (u16, String, Vec<u8>) {
    let mut bruto = format!(
        "POST {rota} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\
         Content-Type: application/octet-stream\r\nContent-Length: {}\r\n{extra}\r\n",
        end.port(),
        corpo.len()
    )
    .into_bytes();
    bruto.extend_from_slice(corpo);
    partir(&conversar(end, &bruto).expect("resposta"))
}

fn conversar(end: SocketAddr, bruto: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut c = TcpStream::connect(end)?;
    c.set_read_timeout(Some(Duration::from_secs(30)))?;
    c.write_all(bruto)?;
    let mut r = Vec::new();
    c.read_to_end(&mut r)?;
    Ok(r)
}

fn partir(r: &[u8]) -> (u16, String, Vec<u8>) {
    let fim = r.windows(4).position(|j| j == b"\r\n\r\n").expect("cabeca") + 4;
    let cabeca = String::from_utf8_lossy(&r[..fim]).into_owned();
    (cabeca[9..12].parse().unwrap(), cabeca, r[fim..].to_vec())
}

fn erro(corpo: &[u8]) -> String {
    let j = Json::analisar(std::str::from_utf8(corpo).unwrap()).unwrap();
    j.campo("erro")
        .and_then(Json::texto)
        .unwrap_or("")
        .to_string()
}

/// Uma pasta `0755` com guarda: mora no `CARGO_TARGET_TMPDIR` (dentro de
/// `target/`, e nao no `/tmp`) e se apaga no `Drop`, inclusive quando o teste
/// cai no meio (pedido 150, `conferidor_temporarios`).
struct DirTemp(PathBuf);

impl Drop for DirTemp {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl std::ops::Deref for DirTemp {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

fn pasta_nova(nome: &str) -> DirTemp {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let p = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("phxzipweb-{nome}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    DirTemp(p)
}

/// Um pacote pelo proprio servidor: duas entradas numa pasta.
fn pacote(end: SocketAddr, senha: Option<&str>) -> Vec<u8> {
    let s = senha
        .map(|s| format!(r#","senha":"{s}""#))
        .unwrap_or_default();
    let cab = format!(
        r#"{{"formato":"7z","nivel":"lzma2"{s},"itens":[
        {{"nome":"rel","pasta":true}},
        {{"nome":"rel/jan.txt","tamanho":5,"modificado":1790000000}},
        {{"nome":"rel/fev.txt","tamanho":3}}]}}"#
    );
    let (c, cab, corpo) = post(end, "/api/compactar", &envelope(&cab, b"janeifev"), "");
    assert_eq!(c, 200, "{cab}");
    assert!(cab.contains("Content-Type: application/x-7z-compressed\r\n"));
    corpo
}

fn nomes_em(p: &Path) -> Vec<String> {
    let mut v = Vec::new();
    for e in walk(p) {
        v.push(
            e.strip_prefix(p)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/"),
        );
    }
    v.sort();
    v
}

fn walk(p: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(p).unwrap() {
        let e = e.unwrap().path();
        if e.is_dir() {
            v.extend(walk(&e));
        }
        v.push(e);
    }
    v
}

// ------------------------------------------------------------------ a volta

#[test]
fn compactar_listar_testar_extrair_pelo_soquete() {
    let end = subir_simples();
    let p = pacote(end, Some("s3nh@"));
    let (c, _, corpo) = post(
        end,
        "/api/listar",
        &envelope(r#"{"senha":"s3nh@"}"#, &p),
        "",
    );
    assert_eq!(c, 200);
    let j = Json::analisar(std::str::from_utf8(&corpo).unwrap()).unwrap();
    assert_eq!(j.campo("entradas").and_then(Json::lista).unwrap().len(), 3);
    let (c, _, corpo) = post(
        end,
        "/api/testar",
        &envelope(r#"{"senha":"s3nh@"}"#, &p),
        "",
    );
    assert_eq!(c, 200, "{}", String::from_utf8_lossy(&corpo));
    let (c, cab, corpo) = post(
        end,
        "/api/extrair",
        &envelope(r#"{"senha":"s3nh@","indices":[1]}"#, &p),
        "",
    );
    assert_eq!((c, corpo.as_slice()), (200, &b"janei"[..]));
    assert!(cab.contains("filename=\"jan.txt\""), "{cab}");
    let (c, _, corpo) = post(end, "/api/listar", &envelope("{}", &p), "");
    assert_eq!((c, erro(&corpo)), (422, "SENHA_AUSENTE".into()));
}

// ------------------------------------------------------- as recusas, com dreno

/// RED: sem a conferencia da origem nas rotas `/api/*`, um site de fora (com
/// `no-cors`, ou um cliente qualquer) faria a porta extrair. A recusa chega
/// LEGIVEL mesmo com o corpo inteiro no fio -- o dreno.
#[test]
fn origem_alheia_num_post_e_recusada_e_legivel() {
    let end = subir_simples();
    let corpo = vec![0x5Au8; 4 << 20];
    for (rota, cab) in [
        ("/api/extrair", "Origin: http://evil.com\r\n"),
        ("/api/compactar", "Origin: null\r\n"),
        ("/api/listar", "Sec-Fetch-Site: cross-site\r\n"),
    ] {
        let (c, cabeca, r) = post(end, rota, &corpo, cab);
        assert_eq!((c, erro(&r)), (403, "ORIGEM_RECUSADA".into()), "{rota}");
        assert!(cabeca.contains("Content-Security-Policy: "));
    }
    // O irmao: a origem da casa passa.
    let origem = format!(
        "Origin: http://127.0.0.1:{}\r\nSec-Fetch-Site: same-origin\r\n",
        end.port()
    );
    let (c, _, r) = post(end, "/api/listar", &envelope("{}", b"x"), &origem);
    assert_eq!((c, erro(&r)), (422, "NAO_E_7Z".into()));
}

/// 415 e 411 pelo soquete, com o corpo no fio: o cliente le a recusa.
#[test]
fn tipo_e_tamanho_ausentes_chegam_legiveis() {
    let end = subir_simples();
    let corpo = vec![1u8; 2 << 20];
    let mut bruto = format!(
        "POST /api/listar HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: text/plain\r\n\
         Content-Length: {}\r\n\r\n",
        end.port(),
        corpo.len()
    )
    .into_bytes();
    bruto.extend_from_slice(&corpo);
    let (c, _, r) = partir(&conversar(end, &bruto).unwrap());
    assert_eq!((c, erro(&r)), (415, "TIPO_DE_CONTEUDO".into()));

    let bruto = format!(
        "POST /api/listar HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\
         Content-Type: application/octet-stream\r\nTransfer-Encoding: chunked\r\n\r\n\
         5\r\nabcde\r\n0\r\n\r\n",
        end.port()
    );
    let (c, cab, r) = partir(&conversar(end, bruto.as_bytes()).unwrap());
    assert_eq!((c, erro(&r)), (411, "TAMANHO_AUSENTE".into()));
    assert!(cab.starts_with("HTTP/1.1 411 Length Required\r\n"));
}

/// O 413 da cabeca do envelope (`limites.cabeca`) e o do envio (`--envio`)
/// pelo soquete.
#[test]
fn os_413_dos_post() {
    let end = subir(Config {
        porta: 0,
        envio: 3 << 20,
        ..Config::default()
    });
    let mut env = b"PZW1".to_vec();
    env.extend_from_slice(&(2u32 << 20).to_le_bytes());
    env.resize(env.len() + 64, b' ');
    let (c, _, r) = post(end, "/api/listar", &env, "");
    assert_eq!((c, erro(&r)), (413, "GRANDE_DEMAIS".into()));
    assert!(String::from_utf8_lossy(&r).contains(r#""oque":"cabeca""#));
    let grande = vec![0u8; 5 << 20];
    let (c, _, r) = post(end, "/api/compactar", &grande, "");
    assert_eq!((c, erro(&r)), (413, "GRANDE_DEMAIS".into()));
    assert!(String::from_utf8_lossy(&r).contains(r#""oque":"envio""#));
}

/// O `simultaneas`: com as duas vagas tomadas por `POST` que ainda mandam o
/// corpo, o terceiro leva 503 ANTES de mandar o dele -- o pacote nao entra
/// na memoria sem vaga.
#[test]
fn o_terceiro_post_simultaneo_e_ocupado() {
    let end = subir_simples();
    let cabeca = format!(
        "POST /api/listar HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\
         Content-Type: application/octet-stream\r\nContent-Length: 1000000\r\n\r\n",
        end.port()
    );
    let presos: Vec<TcpStream> = (0..2)
        .map(|_| {
            let mut c = TcpStream::connect(end).unwrap();
            c.write_all(cabeca.as_bytes()).unwrap();
            c
        })
        .collect();
    std::thread::sleep(Duration::from_millis(300));
    let (c, cab, r) = post(end, "/api/listar", &envelope("{}", b"x"), "");
    assert_eq!((c, erro(&r)), (503, "OCUPADO".into()), "{cab}");
    // O GET nao disputa a vaga do `simultaneas`.
    let (c, _, _) = partir(
        &conversar(
            end,
            format!(
                "GET /api/estado HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
                end.port()
            )
            .as_bytes(),
        )
        .unwrap(),
    );
    assert_eq!(c, 200);
    drop(presos);
    // Soltas as vagas, o POST volta a passar.
    std::thread::sleep(Duration::from_millis(300));
    let (c, _, r) = post(end, "/api/listar", &envelope("{}", b"x"), "");
    assert_eq!((c, erro(&r)), (422, "NAO_E_7Z".into()));
}

// ---------------------------------------------------------------- zip-slip

/// O 7z hostil: o nome `XXfora.txt` do fixture do motor trocado por outro de
/// mesmo tamanho, com os dois CRCs refeitos -- o que um atacante faria.
fn hostil(nome: &str) -> Vec<u8> {
    let mut d = include_bytes!("../../phxzip/tests/fixtures/base-zip-slip.7z").to_vec();
    let de: Vec<u8> = "XXfora.txt"
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    let para: Vec<u8> = nome.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    assert_eq!(de.len(), para.len());
    let onde = d
        .windows(de.len())
        .position(|w| w == de.as_slice())
        .unwrap();
    d[onde..onde + de.len()].copy_from_slice(&para);
    let off = u64::from_le_bytes(d[12..20].try_into().unwrap()) as usize;
    let tam = u64::from_le_bytes(d[20..28].try_into().unwrap()) as usize;
    let crc = phxsql_core::crc32(&d[32 + off..32 + off + tam]);
    d[28..32].copy_from_slice(&crc.to_le_bytes());
    let crc_inicio = phxsql_core::crc32(&d[12..32]);
    d[8..12].copy_from_slice(&crc_inicio.to_le_bytes());
    d
}

/// RED zip-slip pela web: o pacote com `../ora.txt` e recusado ao listar, ao
/// baixar e ao extrair na pasta -- e nada aparece fora da pasta (nem dentro).
/// E na IDA: compactar um item `../x` e recusado antes de comprimir.
#[test]
fn zip_slip_pela_web_e_recusado() {
    let pasta = pasta_nova("slip");
    let dentro = pasta.join("d");
    std::fs::create_dir(&dentro).unwrap();
    let end = subir(Config {
        porta: 0,
        pasta: Some(dentro.clone()),
        ..Config::default()
    });
    let p = hostil("../ora.txt");
    for (rota, cab) in [
        ("/api/listar", "{}"),
        ("/api/extrair", r#"{"todas":true}"#),
        ("/api/extrair", r#"{"todas":true,"na_pasta":true}"#),
    ] {
        let (c, _, r) = post(end, rota, &envelope(cab, &p), "");
        assert_eq!((c, erro(&r)), (422, "NOME_PERIGOSO".into()), "{rota} {cab}");
    }
    assert!(!pasta.join("ora.txt").exists());
    assert!(nomes_em(&dentro).is_empty());

    let cab = r#"{"itens":[{"nome":"../fora.txt","tamanho":1}]}"#;
    let (c, _, r) = post(end, "/api/compactar", &envelope(cab, b"x"), "");
    assert_eq!((c, erro(&r)), (422, "NOME_PERIGOSO".into()));
}

// ------------------------------------------------- a pasta do operador

/// A extracao na pasta: pelo `Destino` do motor, com o conteudo cifrado
/// nascendo `0600` e nada se sobrescrevendo.
#[test]
fn extrair_na_pasta_grava_pelo_destino() {
    let pasta = pasta_nova("extrair");
    let end = subir(Config {
        porta: 0,
        pasta: Some(pasta.to_path_buf()),
        ..Config::default()
    });
    let p = pacote(end, Some("s"));
    let (c, _, r) = post(
        end,
        "/api/extrair",
        &envelope(r#"{"senha":"s","todas":true,"na_pasta":true}"#, &p),
        "",
    );
    assert_eq!(c, 200, "{}", String::from_utf8_lossy(&r));
    let j = Json::analisar(std::str::from_utf8(&r).unwrap()).unwrap();
    assert_eq!(j.campo("gravadas").and_then(Json::inteiro), Some(3));
    assert_eq!(nomes_em(&pasta), ["rel", "rel/fev.txt", "rel/jan.txt"]);
    assert_eq!(std::fs::read(pasta.join("rel/jan.txt")).unwrap(), b"janei");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let modo = std::fs::metadata(pasta.join("rel/jan.txt"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(modo & 0o777, 0o600, "cifrado nasce privado: {modo:o}");
    }
    // De novo: o arquivo ja existe, e nada se sobrescreve sem pedir.
    let (c, _, r) = post(
        end,
        "/api/extrair",
        &envelope(r#"{"senha":"s","indices":[1],"na_pasta":true}"#, &p),
        "",
    );
    assert_eq!((c, erro(&r)), (422, "DESTINO_INSEGURO".into()));
}

/// Sem `--pasta`, pedir a extracao na pasta e recusa nomeada -- e o estado
/// diz que ela nao existe.
#[test]
fn sem_pasta_nao_ha_extracao_no_disco() {
    let end = subir_simples();
    let p = pacote(end, None);
    let (c, _, r) = post(
        end,
        "/api/extrair",
        &envelope(r#"{"todas":true,"na_pasta":true}"#, &p),
        "",
    );
    assert_eq!((c, erro(&r)), (422, "SEM_PASTA".into()));
}

/// RED da revisao SEC da Z9 pela web: pasta onde outros escrevem e recusada
/// -- ao subir a porta, e a cada extracao (a pasta pode mudar depois de a
/// porta subir). Prova real: sem o `Destino::novo` no caminho da web, o
/// pedido grava na pasta `0777`.
#[cfg(unix)]
#[test]
fn pasta_gravavel_por_outros_e_recusada() {
    use std::os::unix::fs::PermissionsExt;
    let aberta = pasta_nova("aberta");
    std::fs::set_permissions(&*aberta, std::fs::Permissions::from_mode(0o777)).unwrap();
    let r = Servidor::escutar(Config {
        porta: 0,
        pasta: Some(aberta.to_path_buf()),
        ..Config::default()
    });
    let motivo = r
        .err()
        .expect("a porta nao sobe com pasta 0777")
        .to_string();
    assert!(motivo.contains("escrita de outros"), "{motivo}");

    // Subiu com a pasta certa; alguem a abre depois.
    let pasta = pasta_nova("depois");
    let end = subir(Config {
        porta: 0,
        pasta: Some(pasta.to_path_buf()),
        ..Config::default()
    });
    let p = pacote(end, None);
    std::fs::set_permissions(&*pasta, std::fs::Permissions::from_mode(0o777)).unwrap();
    let (c, _, r) = post(
        end,
        "/api/extrair",
        &envelope(r#"{"todas":true,"na_pasta":true}"#, &p),
        "",
    );
    assert_eq!((c, erro(&r)), (422, "DESTINO_INSEGURO".into()));
    assert!(nomes_em(&pasta).is_empty(), "{:?}", nomes_em(&pasta));
    // O irmao: a mesma pasta fechada de novo volta a receber.
    std::fs::set_permissions(&*pasta, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (c, _, _) = post(
        end,
        "/api/extrair",
        &envelope(r#"{"todas":true,"na_pasta":true}"#, &p),
        "",
    );
    assert_eq!(c, 200);
}

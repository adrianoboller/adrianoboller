//! A porta do PhxZip web provada pelo SOQUETE, com o servidor de pe: o que
//! depende de rede se prova contra a rede, nao chamando funcao por dentro.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use phxzip::{Arquivo7z, Escritor, Limites, Opcoes};
use phxzip_web::{servir_em, Config};

const USUARIO: &str = "adriano";
const SENHA: &str = "senha-do-teste-web";

fn subir(com_login: bool, max_corpo: usize) -> u16 {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let mut c = Config {
        max_corpo,
        // Dois fios fixos: o padrao sai dos nucleos da maquina, e o teste do
        // corte em blocos nao pode depender de onde roda.
        fios: 2,
        ..Config::default()
    };
    if com_login {
        c.usuario = Some(USUARIO.into());
        // Poucas voltas so para o teste nao pagar o PBKDF2 inteiro.
        c.senha_hash = Some(phxsql_core::senha::cifrar_com(SENHA, 1000));
    }
    std::thread::spawn(move || servir_em(ouvinte, c));
    porta
}

struct Resposta {
    codigo: u16,
    cabecalhos: String,
    corpo: Vec<u8>,
}

impl Resposta {
    fn texto(&self) -> String {
        String::from_utf8_lossy(&self.corpo).into_owned()
    }
    fn cookie(&self) -> Option<String> {
        self.cabecalhos.lines().find_map(|l| {
            let v = l.strip_prefix("Set-Cookie: ")?;
            Some(v.split(';').next()?.to_string())
        })
    }
}

fn pedir(porta: u16, metodo: &str, rota: &str, extras: &[(&str, &str)], corpo: &[u8]) -> Resposta {
    let mut s = TcpStream::connect(("127.0.0.1", porta)).unwrap();
    let mut cab = format!(
        "{metodo} {rota} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n",
        corpo.len()
    );
    for (k, v) in extras {
        cab.push_str(&format!("{k}: {v}\r\n"));
    }
    cab.push_str("\r\n");
    s.write_all(cab.as_bytes()).unwrap();
    let _ = s.write_all(corpo);
    let mut tudo = Vec::new();
    let _ = s.read_to_end(&mut tudo);
    let fim = tudo
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("resposta sem cabecalho");
    let cabecalhos = String::from_utf8_lossy(&tudo[..fim]).into_owned();
    let codigo = cabecalhos
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    Resposta {
        codigo,
        cabecalhos,
        corpo: tudo[fim + 4..].to_vec(),
    }
}

fn envelope(meta: &str, bytes: &[u8]) -> Vec<u8> {
    let mut v = meta.as_bytes().to_vec();
    v.push(b'\n');
    v.extend_from_slice(bytes);
    v
}

fn api(porta: u16, rota: &str, cookie: Option<&str>, corpo: &[u8]) -> Resposta {
    let mut h = vec![("X-PhxZip", "1")];
    let c;
    if let Some(k) = cookie {
        c = k.to_string();
        h.push(("Cookie", c.as_str()));
    }
    pedir(porta, "POST", rota, &h, corpo)
}

fn um_7z(senha: Option<&str>) -> Vec<u8> {
    let mut e = Escritor::novo(Opcoes {
        senha: senha.map(String::from),
        acaso: [3; 32],
        ..Opcoes::default()
    });
    e.arquivo(
        "docs/leia.txt",
        b"PhxZip na porta 4000\n".repeat(50),
        None,
        None,
    )
    .unwrap();
    e.arquivo("b.bin", (0..=255u8).collect(), None, None)
        .unwrap();
    e.gravar().unwrap()
}

#[test]
fn sem_login_compacta_lista_testa_e_extrai() {
    let p = subir(false, 1 << 20);
    let r = pedir(p, "GET", "/api/estado", &[], b"");
    assert!(r.texto().contains("\"exige_login\":false"), "{}", r.texto());
    // A tela recusa acima do teto ANTES de enviar; para isso ela precisa do
    // teto verdadeiro desta porta, e nao de um numero copiado no JavaScript.
    assert!(
        r.texto().contains(&format!("\"max_corpo\":{}", 1 << 20)),
        "{}",
        r.texto()
    );
    let pagina = pedir(p, "GET", "/", &[], b"");
    assert_eq!(pagina.codigo, 200);
    assert!(pagina.texto().contains("PhxZip"));
    assert!(pagina.cabecalhos.contains("frame-ancestors 'none'"));

    let dados = b"conteudo que vai e volta pela porta\n".repeat(30);
    let meta = format!(
        r#"{{"nivel":9,"senha":"s3gr3do","arquivos":[{{"nome":"pasta/a.txt","tamanho":{}}}]}}"#,
        dados.len()
    );
    let r = api(p, "/api/compactar", None, &envelope(&meta, &dados));
    assert_eq!(r.codigo, 200, "{}", r.texto());
    assert!(r.cabecalhos.contains("application/x-7z-compressed"));
    // O que a porta devolveu e 7z de verdade, com nomes cifrados.
    let arq = r.corpo;
    let a = Arquivo7z::abrir(&arq, Some("s3gr3do"), Limites::default()).unwrap();
    assert!(a.cabecalho_cifrado());
    assert_eq!(a.extrair(0).unwrap(), dados);
    drop(a);
    let r = api(
        p,
        "/api/listar",
        None,
        &envelope(r#"{"senha":"s3gr3do"}"#, &arq),
    );
    assert!(r.texto().contains("pasta/a.txt"), "{}", r.texto());
    let r = api(
        p,
        "/api/testar",
        None,
        &envelope(r#"{"senha":"s3gr3do"}"#, &arq),
    );
    assert!(r.texto().contains("\"testado\":true"), "{}", r.texto());
    let r = api(
        p,
        "/api/extrair",
        None,
        &envelope(r#"{"senha":"s3gr3do","indice":0}"#, &arq),
    );
    assert_eq!(r.corpo, dados);
    let r = api(
        p,
        "/api/listar",
        None,
        &envelope(r#"{"senha":"errada"}"#, &arq),
    );
    assert_eq!(r.codigo, 400);
    assert!(r.texto().contains("senha_errada"));
}

#[test]
fn com_login_nada_de_arquivo_sem_sessao() {
    let p = subir(true, 1 << 20);
    let arq = um_7z(None);
    let corpo = envelope("{}", &arq);
    assert!(pedir(p, "GET", "/api/estado", &[], b"")
        .texto()
        .contains("\"exige_login\":true"));
    assert_eq!(api(p, "/api/listar", None, &corpo).codigo, 401);

    let errada = format!(r#"{{"usuario":"{USUARIO}","senha":"nao-e"}}"#);
    let r = api(p, "/api/entrar", None, errada.as_bytes());
    assert_eq!(r.codigo, 401);
    assert!(r.cookie().is_none());

    let certa = format!(r#"{{"usuario":"{USUARIO}","senha":"{SENHA}"}}"#);
    let r = api(p, "/api/entrar", None, certa.as_bytes());
    assert_eq!(r.codigo, 200, "{}", r.texto());
    let cookie = r.cookie().expect("sessao");
    assert!(r.cabecalhos.contains("HttpOnly") && r.cabecalhos.contains("SameSite=Strict"));
    assert!(!r.texto().contains(SENHA), "a senha voltou na resposta");

    let r = api(p, "/api/listar", Some(&cookie), &corpo);
    assert_eq!(r.codigo, 200, "{}", r.texto());
    assert!(r.texto().contains("docs/leia.txt"));
    let e = pedir(p, "GET", "/api/estado", &[("Cookie", &cookie)], b"");
    assert!(e.texto().contains("\"logado\":true"));
    assert!(!e.texto().contains("pbkdf2") && !e.texto().contains(SENHA));

    api(p, "/api/sair", Some(&cookie), b"");
    assert_eq!(api(p, "/api/listar", Some(&cookie), &corpo).codigo, 401);
}

#[test]
fn sem_o_cabecalho_da_casa_e_recusado_mesmo_sem_login() {
    let p = subir(false, 1 << 20);
    let r = pedir(p, "POST", "/api/listar", &[], &envelope("{}", &um_7z(None)));
    assert_eq!(r.codigo, 403);
}

#[test]
fn corpo_acima_do_teto_e_413_antes_de_ler() {
    let p = subir(false, 4096);
    let r = api(p, "/api/listar", None, &envelope("{}", &vec![0u8; 20_000]));
    assert_eq!(r.codigo, 413, "{}", r.texto());
}

#[test]
fn nome_que_sai_da_pasta_nao_entra_no_arquivo() {
    let p = subir(false, 1 << 20);
    let meta = r#"{"arquivos":[{"nome":"../../.bashrc","tamanho":3}]}"#;
    let r = api(p, "/api/compactar", None, &envelope(meta, b"abc"));
    assert_eq!(r.codigo, 400);
    assert!(r.texto().contains("\"tipo\":\"caminho\""), "{}", r.texto());
}

/// O «extrair tudo»: um envio do `.7z`, uma resposta com a lista e os bytes
/// emendados na ordem dela. A soma da lista fecha com o corpo -- e e essa
/// conta que a tela usa para recusar uma resposta que chegou curta.
#[test]
fn extrair_tudo_devolve_a_lista_e_os_bytes_na_ordem() {
    let p = subir(false, 1 << 20);
    let arq = um_7z(Some("abc"));
    let r = api(
        p,
        "/api/extrair_tudo",
        None,
        &envelope(r#"{"senha":"abc"}"#, &arq),
    );
    assert_eq!(r.codigo, 200, "{}", r.texto());
    let quebra = r.corpo.iter().position(|b| *b == b'\n').unwrap();
    let cab = phxsql_core::json::Json::analisar(std::str::from_utf8(&r.corpo[..quebra]).unwrap())
        .unwrap();
    let lista = cab.campo("arquivos").and_then(|l| l.lista()).unwrap();
    let nomes: Vec<&str> = lista.iter().map(|e| e.texto_ou("nome", "")).collect();
    assert_eq!(nomes, ["docs/leia.txt", "b.bin"]);
    let corpo = &r.corpo[quebra + 1..];
    let soma: i64 = lista.iter().map(|e| e.inteiro_ou("tamanho", -1)).sum();
    assert_eq!(soma as usize, corpo.len());
    let leia = b"PhxZip na porta 4000\n".repeat(50);
    assert_eq!(&corpo[..leia.len()], &leia[..]);
    assert_eq!(&corpo[leia.len()..], &(0..=255u8).collect::<Vec<_>>()[..]);

    // Senha errada sai como ERRO, com codigo, antes do cabecalho de 200 --
    // e nao como uma resposta curta que a tela teria de adivinhar. Com os
    // nomes VISIVEIS, porque com nomes cifrados o proprio `abrir` ja recusa e
    // a prova nao alcancaria o `testar` que vem antes do cabecalho.
    let mut e = Escritor::novo(Opcoes {
        senha: Some("abc".into()),
        cifrar_nomes: false,
        acaso: [5; 32],
        ..Opcoes::default()
    });
    e.arquivo("a.txt", b"segredo\n".repeat(20), None, None)
        .unwrap();
    let visiveis = e.gravar().unwrap();
    let r = api(
        p,
        "/api/extrair_tudo",
        None,
        &envelope(r#"{"senha":"errada"}"#, &visiveis),
    );
    assert_eq!(r.codigo, 400);
    assert!(r.texto().contains("senha_errada"), "{}", r.texto());
}

/// A porta compacta em BLOCOS (dois fios, aqui) e o «menor arquivo» pede um
/// bloco so: os dois abrem inteiros, e o de um bloco e menor -- e o preco da
/// velocidade, que a tela deixa quem quer recusar.
#[test]
fn compactar_em_blocos_e_o_menor_arquivo_de_um_fio() {
    let p = subir(false, 8 << 20);
    let mut dados = Vec::new();
    let mut x = 3u32;
    while dados.len() < 2_600_000 {
        x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
        dados.extend_from_slice(format!("linha {} da porta 4000\n", x >> 17).as_bytes());
    }
    let pedir_7z = |um_fio: bool| {
        let meta = format!(
            r#"{{"nivel":1,"um_fio":{um_fio},"arquivos":[{{"nome":"a.txt","tamanho":{},"mtime":1790000000}}]}}"#,
            dados.len()
        );
        let r = api(p, "/api/compactar", None, &envelope(&meta, &dados));
        assert_eq!(r.codigo, 200, "{}", r.texto());
        let a = Arquivo7z::abrir(&r.corpo, None, Limites::default()).unwrap();
        assert_eq!(a.extrair(0).unwrap(), dados);
        r.corpo.len()
    };
    let em_blocos = pedir_7z(false);
    let um_bloco = pedir_7z(true);
    assert!(
        um_bloco < em_blocos,
        "um bloco {um_bloco} devia sair menor que em blocos {em_blocos}"
    );
}

//! O `Remoto` (multi-servidor da interface) ligando o tunel -- o buraco que a
//! §10 do `docs/CIFRA-DO-FIO.md` deixou escrito e esta rodada fechou.
//!
//! A prova e por SOQUETE, e nao por teste unitario, e de proposito: o aperto,
//! o sela/abre do registro e a queda da conexao so se provam contra o sistema
//! operacional. Cada teste sobe um servidor de verdade numa porta efemera e
//! fala com ele pela mesma `atender` da porta de dados.
use super::*;
use crate::config::ServidorWeb;

/// Um servidor de dados vivo, com a cifra do fio atendida. `exigir` liga a
/// recusa do que vem em claro -- e o que transforma "esqueci de cifrar" num
/// erro visivel em vez de um vazamento calado.
fn servidor_cifrado(dir: &std::path::Path, exigir: bool) -> Arc<Servidor> {
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    // A estatica mora DENTRO do dir do teste, e nao no cwd: sem isso dois
    // servidores de teste dividiriam a mesma `chave-do-fio.hex` e o pino de
    // um valeria para o outro.
    c.cifra_fio.arquivo = dir.join("chave-do-fio.hex");
    c.cifra_fio.exigir = exigir;
    Servidor::novo(c).unwrap()
}

/// Sobe a porta de dados numa porta efemera e devolve o numero. Uma thread
/// por conexao, exatamente como o `escutar` de producao.
fn porta_de_dados(s: &Arc<Servidor>) -> u16 {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let s = Arc::clone(s);
    std::thread::spawn(move || {
        for fluxo in ouvinte.incoming() {
            let Ok(fluxo) = fluxo else { return };
            let Ok(par) = fluxo.peer_addr() else { continue };
            let s = Arc::clone(&s);
            std::thread::spawn(move || s.atender(fluxo, par));
        }
    });
    porta
}

fn linha_desafio() -> String {
    // O token da rede vai junto: o destino confere a rede antes da
    // identidade, e todos os servidores destes testes usam "t".
    Json::objeto(vec![
        ("op", Json::texto_de("desafio")),
        ("usuario", Json::texto_de("qualquer")),
        ("token", Json::texto_de("t")),
    ])
    .escrever()
}

/// O tunel liga e um pedido REAL viaja por dentro dele -- o `desafio`, que
/// e pre-login, volta com um nonce. Se o `Canal` nao selasse e abrisse o
/// registro, nada disto voltaria legivel.
#[test]
fn o_remoto_liga_o_tunel_e_carrega_um_pedido_real() {
    let dir = DirTemp::novo("remoto-tunel");
    let s = servidor_cifrado(&dir, false);
    let porta = porta_de_dados(&s);
    let destino = format!("127.0.0.1:{porta}");

    let mut r = Remoto::abrir(&destino, 5).unwrap();
    assert!(!r.canal.cifrado(), "nasce em claro");
    let apresentada = r.cifrar(None).unwrap();
    assert!(r.canal.cifrado(), "depois do aperto, cifrado");
    assert_ne!(apresentada, [0u8; 32], "o destino apresentou uma chave");

    let resp = r.conversar(&linha_desafio()).unwrap();
    assert!(resp.booleano_ou("ok", false), "o desafio voltou: {resp:?}");
    let nonce = resp
        .campo("resultado")
        .map(|x| x.texto_ou("nonce", ""))
        .unwrap_or("");
    assert!(!nonce.is_empty(), "o nonce viajou pelo tunel");
}

/// O pino CERTO entra e o ERRADO derruba a conexao. E a prova de que a
/// conferencia do pino existe: sem ela, um destino que apresentasse outra
/// chave -- o homem-no-meio -- fecharia o aperto do mesmo jeito.
#[test]
fn o_pino_certo_entra_o_errado_derruba() {
    let dir = DirTemp::novo("remoto-pino");
    let s = servidor_cifrado(&dir, false);
    let porta = porta_de_dados(&s);
    let destino = format!("127.0.0.1:{porta}");

    // Captura a chave que ESTE servidor apresenta, sem pino.
    let mut scratch = Remoto::abrir(&destino, 5).unwrap();
    let pino = scratch.cifrar(None).unwrap();
    drop(scratch);

    // Pino certo: entra.
    let mut certo = Remoto::abrir(&destino, 5).unwrap();
    assert!(
        certo.cifrar(Some(pino)).is_ok(),
        "o pino certo tinha de entrar"
    );
    assert!(certo.canal.cifrado());

    // Pino errado (um bit trocado): cai, e a conexao NAO fica cifrada.
    let mut torto = pino;
    torto[0] ^= 0xff;
    let mut errado = Remoto::abrir(&destino, 5).unwrap();
    assert!(
        errado.cifrar(Some(torto)).is_err(),
        "o pino errado tinha de derrubar o aperto"
    );
    assert!(!errado.canal.cifrado());
}

/// A DECISAO, pelo caminho de producao: `abrir_remoto` liga o tunel quando
/// a config do destino pede `cifra`, e nao liga quando nao pede.
///
/// O servidor destino EXIGE a cifra -- entao "esqueci de cifrar" nao passa
/// calado, vira o erro nomeado. E o defeito reposto: sem a nova ligacao, o
/// login iria em claro e o destino que exige o recusaria.
#[test]
fn abrir_remoto_liga_o_tunel_quando_a_config_pede_cifra() {
    let dir_n = DirTemp::novo("remoto-wire-normal");
    let dir_e = DirTemp::novo("remoto-wire-exige");
    let dir_1 = DirTemp::novo("remoto-wire-interface");

    let normal = servidor_cifrado(&dir_n, false);
    let exigente = servidor_cifrado(&dir_e, true);
    let porta_n = porta_de_dados(&normal);
    let porta_e = porta_de_dados(&exigente);
    let destino_n = format!("127.0.0.1:{porta_n}");
    let destino_e = format!("127.0.0.1:{porta_e}");

    // O pino do servidor que exige, capturado por um aperto sem pino.
    let mut scratch = Remoto::abrir(&destino_e, 5).unwrap();
    let pino_e = phxsql_core::hash::para_hex(&scratch.cifrar(None).unwrap());
    drop(scratch);

    let interface = |servidores: Vec<ServidorWeb>| -> Arc<Servidor> {
        let dir = dir_1.join(format!("{}", servidores.len()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut c = Config {
            base: dir.clone(),
            log_acessos: dir.join("acessos.log"),
            blacklist: dir.join("blacklist.json"),
            dblink: dir.join("dblink.json"),
            token: "t".into(),
            ..Config::default()
        };
        c.web.servidores = servidores;
        Servidor::novo(c).unwrap()
    };
    let ip = "127.0.0.1";

    // (a) COMPORTAMENTO VELHO: texto solto = claro. Contra um destino que
    //     NAO exige, entra, e o Remoto NAO liga o tunel.
    let s_claro = interface(vec![ServidorWeb {
        endereco: destino_n.clone(),
        cifra: false,
        chave_do_fio: String::new(),
        pino_tls: String::new(),
    }]);
    let (op, _v, remoto) = s_claro
        .abrir_remoto(&destino_n, &linha_desafio(), ip)
        .unwrap();
    assert_eq!(op, "desafio");
    assert!(
        !remoto.lock().unwrap().canal.cifrado(),
        "texto solto nao devia ligar tunel"
    );

    // (b) FORMATO NOVO: objeto com cifra e pino. Contra o destino que
    //     EXIGE, entra JUSTAMENTE porque o tunel subiu antes do login.
    let s_cifra = interface(vec![ServidorWeb {
        endereco: destino_e.clone(),
        cifra: true,
        chave_do_fio: pino_e.clone(),
        pino_tls: String::new(),
    }]);
    let (_op, _v, remoto) = s_cifra
        .abrir_remoto(&destino_e, &linha_desafio(), ip)
        .unwrap();
    assert!(
        remoto.lock().unwrap().canal.cifrado(),
        "cifra:true tinha de ligar o tunel"
    );

    // (c) DEFEITO REPOSTO: o mesmo destino que EXIGE, mas pedido com
    //     cifra:false. O login vai em claro e o destino recusa nomeando a
    //     cifra do fio. Se a ligacao do tunel nao existisse, (b) cairia
    //     aqui tambem.
    let s_claro_para_exigente = interface(vec![ServidorWeb {
        endereco: destino_e.clone(),
        cifra: false,
        chave_do_fio: String::new(),
        pino_tls: String::new(),
    }]);
    // `unwrap_err` pediria `Debug` do `Remoto` do lado Ok; um `match`
    // evita isso e ainda diz o que falhou se por acaso ENTRAR.
    match s_claro_para_exigente.abrir_remoto(&destino_e, &linha_desafio(), ip) {
        Ok(_) => panic!("o destino exige cifra e deixou passar em claro"),
        Err((_op, e)) => assert!(
            e.to_string().contains("cifra do fio"),
            "a recusa tinha de nomear a cifra do fio: {e}"
        ),
    }
}

/// **`pino_tls` no destino da interface (pedido 572, T6b-2), pelo caminho de
/// producao.** O destino tem `"tls": true` e EXIGE cifra; a interface o tem
/// com `cifra: false` e o pino TLS -- entao so o TLS pode fazer o desafio
/// passar. Defeito reposto que isto derruba: `abrir_remoto` sem olhar o
/// `pino_tls` manda o login em claro, e o destino recusa. O pino errado cai
/// no aperto, antes do token.
#[test]
fn abrir_remoto_fala_tls_quando_o_destino_tem_pino_tls() {
    let dir_d = DirTemp::novo("remoto-tls-destino");
    let dir_i = DirTemp::novo("remoto-tls-interface");
    let mut c = Config {
        base: dir_d.to_path_buf(),
        log_acessos: dir_d.join("acessos.log"),
        blacklist: dir_d.join("blacklist.json"),
        dblink: dir_d.join("dblink.json"),
        token: "t".into(),
        caminho: Some(dir_d.join("config.json")),
        ..Config::default()
    };
    c.cifra_fio.arquivo = dir_d.join("chave-do-fio.hex");
    c.cifra_fio.exigir = true;
    c.tls.ligado = true;
    let destino_s = Servidor::novo(c).unwrap();
    destino_s.preparar_tls_dos_dados().unwrap();
    let porta = porta_de_dados(&destino_s);
    let destino = format!("127.0.0.1:{porta}");
    let pem = std::fs::read_to_string(dir_d.join("tls-dados-certificado.pem")).unwrap();
    let cert = phxsql_core::x509::blocos_pem(&pem, "CERTIFICATE").unwrap();
    let pino = phxsql_core::tls::pino_em_texto(&phxsql_core::hash::sha256(
        phxsql_core::x509::spki_do_certificado(&cert[0]).unwrap(),
    ));

    let interface = |pino_tls: String| -> Arc<Servidor> {
        let mut c = Config {
            base: dir_i.to_path_buf(),
            log_acessos: dir_i.join("acessos.log"),
            blacklist: dir_i.join("blacklist.json"),
            dblink: dir_i.join("dblink.json"),
            token: "t".into(),
            ..Config::default()
        };
        c.web.servidores = vec![ServidorWeb {
            endereco: destino.clone(),
            cifra: false,
            chave_do_fio: String::new(),
            pino_tls,
        }];
        Servidor::novo(c).unwrap()
    };
    let (op, _v, remoto) = interface(pino.clone())
        .abrir_remoto(&destino, &linha_desafio(), "127.0.0.1")
        .unwrap_or_else(|(_, e)| panic!("o desafio pelo TLS nao passou: {e}"));
    assert_eq!(op, "desafio");
    assert!(remoto.lock().unwrap().tls(), "o pino_tls nao ligou o TLS");

    let torto = phxsql_core::tls::pino_em_texto(&[9u8; 32]);
    match interface(torto).abrir_remoto(&destino, &linha_desafio(), "127.0.0.1") {
        Ok(_) => panic!("o pino TLS errado entrou"),
        Err((_, e)) => assert!(e.to_string().contains(&pino), "{e}"),
    }
}

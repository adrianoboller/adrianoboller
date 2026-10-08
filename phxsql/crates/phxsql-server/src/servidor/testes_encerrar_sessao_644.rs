//! Encerrar sessao: web x conexao pelo PEDIDO, nunca pela forma do id --
//! pedido 644. Pelo soquete, porque o que se prova e que a conexao de mesmo
//! numero continua de pe.
use super::*;
use crate::apoio_teste::Ligacao as Cliente;

fn subir(rotulo: &str) -> (DirTemp, Arc<Servidor>, u16) {
    let dir = DirTemp::novo(rotulo);
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        ..Config::default()
    };
    // A cifra exigida mediria o portao errado: o que se prova aqui e outra coisa.
    c.cifra_fio.exigir = false;
    let s = Servidor::novo(c).unwrap();
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    // Como o `servir` de producao: a porta REAL anotada antes de aceitar.
    s.anotar_porta_no_ar(&ouvinte);
    let s2 = Arc::clone(&s);
    std::thread::spawn(move || s2.aceitar_ate_mandarem_parar(&ouvinte));
    (dir, s, porta)
}

fn semear(s: &Servidor, id: &str) {
    s.sessoes
        .lock()
        .unwrap()
        .semear_para_teste(id, "ana", crate::agora_ms());
}

fn web_vivas(s: &Servidor) -> usize {
    s.sessoes.lock().unwrap().quantas()
}

/// A conexao 1 (a primeira) e uma sessao web cujo id comeca por «1»: com a
/// heuristica «so algarismos = numero de conexao» o pedido derrubava a
/// conexao 1 e deixava a sessao web de pe.
#[test]
fn id_web_so_de_algarismos_encerra_a_sessao_e_poupa_a_conexao_de_mesmo_numero() {
    let (_d, s, porta) = subir("encerrar-644-web");
    let mut vitima = Cliente::nova(porta);
    assert!(vitima.pedir(r#"{"op":"ping","token":"t"}"#).is_some());
    let mut admin = Cliente::nova(porta);
    semear(&s, "12345678901234567890123456789012345678901234567a");
    semear(&s, "1");
    assert_eq!(web_vivas(&s), 2);

    let r = admin
        .pedir(r#"{"op":"encerrar_sessao","token":"t","id":"1","tipo":"web"}"#)
        .expect("resposta");
    assert!(r.contains(r#""origem":"web""#), "{r}");
    assert_eq!(
        web_vivas(&s),
        1,
        "a sessao web de id «1» tinha de sair: {r}"
    );
    assert!(
        vitima.pedir(r#"{"op":"ping","token":"t"}"#).is_some(),
        "a conexao de mesmo numero (1) foi derrubada"
    );
}

/// Sem o campo, o id so de algarismos e recusado dizendo por que, e nada
/// cai: nem a sessao, nem a conexao.
#[test]
fn sem_tipo_o_id_ambiguo_e_recusado_e_nada_cai() {
    let (_d, s, porta) = subir("encerrar-644-ambiguo");
    let mut vitima = Cliente::nova(porta);
    assert!(vitima.pedir(r#"{"op":"ping","token":"t"}"#).is_some());
    let mut admin = Cliente::nova(porta);
    semear(&s, "1");
    let r = admin
        .pedir(r#"{"op":"encerrar_sessao","token":"t","id":"1"}"#)
        .expect("resposta");
    assert!(r.contains("ambiguo") && r.contains("tipo"), "{r}");
    assert_eq!(web_vivas(&s), 1);
    assert!(vitima.pedir(r#"{"op":"ping","token":"t"}"#).is_some());
}

/// O comportamento VELHO segue: numero JSON derruba a conexao, texto com
/// letra encerra a sessao web -- ninguem que mandava so `id` quebra.
#[test]
fn cliente_antigo_que_manda_so_id_continua_funcionando() {
    let (_d, s, porta) = subir("encerrar-644-antigo");
    let mut vitima = Cliente::nova(porta);
    assert!(vitima.pedir(r#"{"op":"ping","token":"t"}"#).is_some());
    let mut admin = Cliente::nova(porta);
    semear(&s, "abcdef0123456789abcdef0123456789abcdef0123456789");
    let r = admin
        .pedir(r#"{"op":"encerrar_sessao","token":"t","id":"abcdef01"}"#)
        .expect("resposta");
    assert!(r.contains(r#""origem":"web""#), "{r}");
    assert_eq!(web_vivas(&s), 0);
    let r = admin
        .pedir(r#"{"op":"encerrar_sessao","token":"t","id":1}"#)
        .expect("resposta");
    assert!(r.contains(r#""encerrada":1"#), "{r}");
    assert!(
        espera_cair(&mut vitima),
        "o numero JSON tinha de derrubar a conexao 1"
    );
    let r = admin
        .pedir(r#"{"op":"encerrar_sessao","token":"t","id":"x","tipo":"qualquer"}"#)
        .expect("resposta");
    assert!(r.contains("tipo"), "{r}");
}

/// Pedido 645: o `ping` diz a porta que o servidor escuta DE VERDADE (a
/// tela cravava 5000) e os tipos de arquivo da lista unica do motor.
#[test]
fn o_ping_diz_a_porta_real_e_os_tipos_de_arquivo_do_motor() {
    let (_d, s, porta) = subir("ping-645");
    assert_ne!(porta, 5000);
    let mut c = Cliente::nova(porta);
    let r = c.pedir(r#"{"op":"ping","token":"t"}"#).expect("resposta");
    let j = Json::analisar(&r).unwrap();
    let res = j.campo("resultado").unwrap();
    assert_eq!(
        res.inteiro_ou("porta_dados", 0),
        i64::from(porta),
        "o ping devia dizer a porta real: {r}"
    );
    let tipos: Vec<String> = res
        .campo("arquivos_por_tabela")
        .and_then(Json::lista)
        .expect("lista de tipos")
        .iter()
        .filter_map(|x| x.texto().map(String::from))
        .collect();
    let do_motor: Vec<String> = phxsql_store::catalogo::Database::extensoes_de_uma_tabela()
        .iter()
        .map(|e| e.to_string())
        .collect();
    assert_eq!(
        tipos, do_motor,
        "a lista vem do motor, nao de um segundo lugar"
    );
    assert!(
        tipos.len() > 5 && tipos.contains(&"fts".to_string()),
        "{tipos:?}"
    );
    drop(s);
}

fn espera_cair(c: &mut Cliente) -> bool {
    for _ in 0..200 {
        if c.pedir(r#"{"op":"ping","token":"t"}"#).is_none() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

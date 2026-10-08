//! A sonda `replicacao_testar` e a falha de REDE -- revisao SEC de 17/09/2026,
//! A5 (pedido 282). O caminho feliz pelo soquete esta em
//! `tests/sonda-da-replicacao.rs`; aqui e o que a sonda diz quando NAO chega.
use super::*;

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Uma porta que acabou de ser solta: conectar nela e recusado na hora.
fn porta_fechada() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn origem(nome: &str, porta: u16) -> crate::config::Origem {
    crate::config::Origem {
        nome: nome.into(),
        host: "127.0.0.1".into(),
        porta,
        token: "x".into(),
        databases: Vec::new(),
        reconectar_em: 10,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
        cifra: false,
        chave_do_fio: String::new(),
        espelho: false,
    }
}

fn servidor(nome: &str, porta_da_origem: u16) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("sonda-{nome}"));
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.replicacao.origens.push(origem("alfa", porta_da_origem));
    (Servidor::novo(c).unwrap(), dir)
}

/// A classificacao e por TIPO, e cobre todo tipo que o sistema invente.
#[test]
fn a_falha_de_rede_se_classifica_pelo_tipo() {
    use std::io::ErrorKind::*;
    assert_eq!(
        chave_da_falha_de_rede(ConnectionRefused),
        "erro.sonda_recusada"
    );
    assert_eq!(chave_da_falha_de_rede(TimedOut), "erro.sonda_prazo");
    assert_eq!(
        chave_da_falha_de_rede(HostUnreachable),
        "erro.sonda_sem_rota"
    );
    assert_eq!(
        chave_da_falha_de_rede(NetworkUnreachable),
        "erro.sonda_sem_rota"
    );
    assert_eq!(chave_da_falha_de_rede(NotFound), "erro.sonda_sem_rota");
    assert_eq!(chave_da_falha_de_rede(ConnectionReset), "erro.sonda_caiu");
    assert_eq!(chave_da_falha_de_rede(Other), "erro.sonda_caiu");
}

/// **Prova real do A5.** A porta fechada respondia «Connection refused»
/// com o texto do sistema dentro -- uma sonda de rede com o `errno` de
/// cada alvo. Agora a resposta e a frase da fabrica, e o host FORA da
/// configuracao que nao responde conta como tentativa leve.
///
/// **Defeito reposto**: `Err(e) => Err(e)` no lugar do braco `Io` do
/// `op_replicacao_testar`, e as duas asercoes de texto caem.
#[test]
fn a_falha_de_rede_da_sonda_nao_vaza_o_texto_do_sistema() {
    let porta = porta_fechada();
    let (s, _dir) = servidor("texto", porta);
    let ip = "198.51.100.7";
    let e = s
        .executar(
            "replicacao_testar",
            &pedido(&format!(
                r#"{{"host":"127.0.0.1","porta":{porta},"token_remoto":"x"}}"#
            )),
            &Sessao {
                ip: ip.into(),
                ..Sessao::default()
            },
        )
        .unwrap_err();
    let texto = e.to_string();
    assert_eq!(e.nome(), "ERRO_DE_ES", "a classe do erro nao muda: {texto}");
    assert!(texto.contains("recusou a conexao"), "{texto}");
    assert!(
        !texto.to_lowercase().contains("refused") && !texto.contains("os error"),
        "o texto do sistema vazou: {texto}"
    );
    assert!(texto.contains(&format!("127.0.0.1:{porta}")), "{texto}");
    assert_eq!(
        s.lista_negra.lock().unwrap().tentativas_de(ip),
        1,
        "o host solto sem resposta nao contou tentativa leve"
    );
}

/// O comportamento VELHO: a origem NOMEADA (a que esta no `config.json`)
/// recebe a mesma classificacao e NAO conta tentativa -- ela ja tem o
/// freio do `recusar_se_estacionada`, e testar a propria origem e rotina
/// de quem administra, nao sonda.
#[test]
fn origem_nomeada_sem_resposta_nao_conta_tentativa() {
    let porta = porta_fechada();
    let (s, _dir) = servidor("nomeada", porta);
    let ip = "198.51.100.8";
    let e = s
        .executar(
            "replicacao_testar",
            &pedido(r#"{"origem":"alfa"}"#),
            &Sessao {
                ip: ip.into(),
                ..Sessao::default()
            },
        )
        .unwrap_err();
    assert!(e.to_string().contains("recusou a conexao"), "{e}");
    assert_eq!(s.lista_negra.lock().unwrap().tentativas_de(ip), 0);
}

/* =========================================================== pedido 770
OS PADROES DE FABRICA ENDURECIDOS.

Decisao do dono, 09/10/2026: endurecer MESMO quebrando cliente antigo --
excecao explicita a «guarda nova entra pedida», so para este pedido. O
numero de cada padrao esta em `docs/propostas/padroes-770.md`.

Cada mudanca tem o PAR que a lei pede: o padrao novo para quem nao
declarou nada, e quem declarou o valor velho por escrito continua com ele
-- com o aviso `770:` no arranque. O par impede os dois estragos: o padrao
que nao mudou de verdade, e o que mudou ate para quem escreveu o contrario. */
use super::*;
use crate::config::{PoliticaDeSenha, SenhaRecusada};

const SENHA_DA_ANA: &str = "A-senha-da-Ana-2026";

fn config_de(texto: &str) -> Config {
    Config::de_json(&Json::analisar(texto).unwrap()).unwrap()
}

fn avisos_do_770(c: &Config) -> Vec<&String> {
    c.avisos.iter().filter(|a| a.starts_with("770:")).collect()
}

/// Um servidor de `config.json` de verdade (o cadastro se grava nele), com
/// `extra` entrando no topo do arquivo.
fn servidor(nome: &str, extra: &str) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("padroes-770-{nome}"));
    let caminho = dir.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{ "token": "t", "bind": "127.0.0.1:0", "base": "{}", {extra}
                 "usuarios": [ {{ "id": 9, "nome": "Ana", "login": "ana",
                                  "senha_hash": "{}", "supervisor": true }} ] }}"#,
            dir.join("dados").display(),
            phxsql_core::senha::cifrar_com(SENHA_DA_ANA, 64),
        ),
    )
    .unwrap();
    let mut c = Config::ler(&caminho).unwrap();
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    (Servidor::novo(c).unwrap(), dir)
}

fn criar(s: &Servidor, login: &str, senha: &str) -> Result<Json> {
    let mut ana = Sessao {
        usuario: s.cadastro().por_login("ana").cloned(),
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        &format!(r#"{{"op":"usuario_criar","token":"t","login":"{login}","senha":"{senha}"}}"#),
        &mut ana,
        "127.0.0.1",
    );
    r
}

// ------------------------------------------------------------- a senha

/// As tres regras, cada uma pegando a sua, e a senha que passa nas tres.
/// RED medido pela guarda `770-politica-de-senha-frouxa` (o `Default` de
/// volta a `0/false/false`).
#[test]
fn a_politica_de_fabrica_recusa_curta_sem_classes_e_igual_ao_login() {
    let p = PoliticaDeSenha::default();
    assert_eq!(p.conferir("ana", "Ab1!"), Err(SenhaRecusada::Curta(8)));
    assert_eq!(
        p.conferir("ana", "abcdefgh1"),
        Err(SenhaRecusada::SemClasses)
    );
    assert_eq!(
        p.conferir("Ana.Silva-2026", "Ana.Silva-2026"),
        Err(SenhaRecusada::IgualAoLogin)
    );
    // A classe e a Unicode: a cedilha e o til sao letras, e o portugues
    // passa sem trocar a senha por ASCII.
    assert_eq!(p.conferir("ana", "Ação-2026"), Ok(()));
    // O comprimento e em CARACTERES: seis caracteres em nove bytes nao
    // viram nove.
    assert_eq!(p.conferir("ana", "Ãção1!"), Err(SenhaRecusada::Curta(8)));
}

/// Pelo protocolo, com o texto da fabrica de mensagens -- que nomeia o
/// campo que afrouxa a regra, para quem cria usuario por script saber onde
/// mexer. E nada vai ao `config.json` quando recusa.
#[test]
fn usuario_criar_recusa_a_senha_fraca_pela_politica_de_fabrica() {
    let (s, dir) = servidor("fabrica", "");
    let e = criar(&s, "carlos", "curta").unwrap_err().to_string();
    assert!(e.contains("politica_de_senha.minimo"), "{e}");
    let e = criar(&s, "carlos", "so-minusculas-2026")
        .unwrap_err()
        .to_string();
    assert!(e.contains("politica_de_senha.classes"), "{e}");
    let e = criar(&s, "Carlos-2026!", "Carlos-2026!")
        .unwrap_err()
        .to_string();
    assert!(e.contains("politica_de_senha.diferente_do_login"), "{e}");
    let arquivo = std::fs::read_to_string(dir.join("config.json")).unwrap();
    assert!(!arquivo.contains("carlos") && !arquivo.contains("Carlos"));

    criar(&s, "carlos", "Carlos-da-Loja-2026").expect("a senha forte entra");
}

/// O comportamento VELHO por escrito: continua criando senha fraca, e o
/// arranque avisa cada regra afrouxada.
#[test]
fn quem_declara_a_politica_velha_continua_criando_senha_fraca_com_aviso() {
    let (s, _dir) = servidor(
        "velha",
        r#""politica_de_senha": { "minimo": 0, "classes": false, "diferente_do_login": false },"#,
    );
    let avisos = avisos_do_770(&s.config);
    assert_eq!(avisos.len(), 3, "{avisos:?}");
    assert!(avisos[0].contains("politica_de_senha.minimo = 0"));
    criar(&s, "carlos", "x").expect("a politica velha escrita vale");
    criar(&s, "zeca", "zeca").expect("igual ao login, com a regra desligada");
}

/// Alterar o telefone nao reconfere a senha que ja esta gravada -- a regra
/// dos tres maduros (conferem na definicao). Sem isto, quem tem senha
/// anterior ao 770 nao conseguiria nem mudar o proprio e-mail.
#[test]
fn alterar_sem_senha_nao_reconfere_a_que_esta_gravada() {
    // A camada de protecao desligada (so vale em binario de teste): o
    // `usuario_alterar` pede a senha de execucao, e a pergunta daqui e outra.
    let velha = r#""politica_de_senha": { "minimo": 0, "classes": false },
                   "protecao": { "ligada": false },"#;
    let (s, dir) = servidor("alterar", velha);
    criar(&s, "carlos", "fraca").unwrap();
    drop(s);
    // O MESMO arquivo, agora sem a politica velha: o carlos ficou com a
    // senha fraca gravada, e o servidor sobe com a politica de fabrica.
    let caminho = dir.join("config.json");
    let texto = std::fs::read_to_string(&caminho).unwrap();
    assert!(texto.contains("politica_de_senha"));
    let sem = Json::analisar(&texto).unwrap();
    let Json::Objeto(pares) = sem else {
        panic!("config sem objeto")
    };
    let pares: Vec<_> = pares
        .into_iter()
        .filter(|(k, _)| k != "politica_de_senha")
        .collect();
    std::fs::write(&caminho, Json::Objeto(pares).escrever()).unwrap();
    let mut c = Config::ler(&caminho).unwrap();
    assert_eq!(c.politica_de_senha, PoliticaDeSenha::default());
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    let s = Servidor::novo(c).unwrap();
    let mut ana = Sessao {
        usuario: s.cadastro().por_login("ana").cloned(),
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        r#"{"op":"usuario_alterar","token":"t","login":"carlos","telefone":"47 9999-0000"}"#,
        &mut ana,
        "127.0.0.1",
    );
    r.expect("alterar sem senha nao passa pela politica");
    // E quem ja tem a senha fraca continua ENTRANDO: a politica e da
    // definicao, nunca do login.
    assert!(s.cadastro().autenticar("carlos", "fraca").is_some());
}

// ------------------------------------------------------- o TLS das portas

/// As portas HTTP nascem com TLS; a de dados nao (o tunel do fio ja a
/// cifra). RED medido pela guarda `770-rest-sem-tls-de-fabrica` (o padrao
/// do `Rest::de_json` trocado por `false`); o da web e o
/// `770-web-sem-tls-de-fabrica`, pelo soquete.
#[test]
fn as_portas_http_nascem_com_tls_e_a_de_dados_nao() {
    let c = config_de(r#"{"token":"x","web":{"ligado":true},"rest":{"ligado":true}}"#);
    assert!(c.web.tls.ligado, "web sem TLS de fabrica");
    assert!(c.rest.tls.ligado, "rest sem TLS de fabrica");
    assert!(!c.tls.ligado, "a porta de dados nao muda");
    assert!(Config::default().web.tls.ligado && Config::default().rest.tls.ligado);
    assert!(avisos_do_770(&c).is_empty(), "{:?}", c.avisos);
}

/// O proxy declarado termina o TLS: a porta atras dele nasce em claro, senao
/// o 770 quebraria a ponte de toda instalacao que ja fazia a coisa certa.
#[test]
fn atras_de_proxy_a_porta_nasce_sem_tls() {
    let c = config_de(
        r#"{"token":"x","web":{"ligado":true,"atras_de_proxy":true},
            "rest":{"ligado":true,"atras_de_proxy":true}}"#,
    );
    assert!(!c.web.tls.ligado && !c.rest.tls.ligado);
}

/// `"tls": false` escrito continua valendo, e avisa -- so para a porta que
/// esta ligada (a desligada nao fala nada a ninguem).
#[test]
fn quem_escreve_tls_false_continua_em_claro_com_aviso() {
    let c = config_de(
        r#"{"token":"x","web":{"ligado":true,"tls":false},"rest":{"ligado":false,"tls":false}}"#,
    );
    assert!(!c.web.tls.ligado && !c.rest.tls.ligado);
    let avisos = avisos_do_770(&c);
    assert_eq!(avisos.len(), 1, "{avisos:?}");
    assert!(avisos[0].contains("web.tls = false"), "{avisos:?}");
}

// ----------------------------------------------------------- a sessao web

/// 15 minutos sem uso e 12 horas de teto. RED medido pela guarda
/// `770-sessao-de-60-min`.
#[test]
fn a_sessao_web_nasce_curta_e_com_teto() {
    let c = config_de(r#"{"token":"x","web":{"ligado":true}}"#);
    assert_eq!(c.web.sessao_minutos, 15);
    assert_eq!(c.web.sessao_teto_horas, 12);
    assert_eq!(c.web.sessao_ms(), 15 * 60_000);
    assert_eq!(c.web.sessao_teto_ms(), 12 * 3_600_000);
}

/// A sessao velha escrita (60 min, sem teto) continua valendo, com os dois
/// avisos.
#[test]
fn quem_escreve_a_sessao_velha_continua_com_ela_e_com_aviso() {
    let c = config_de(
        r#"{"token":"x","web":{"ligado":true,"sessao_minutos":60,"sessao_teto_horas":0}}"#,
    );
    assert_eq!(c.web.sessao_ms(), 3_600_000);
    assert_eq!(c.web.sessao_teto_ms(), 0);
    let avisos = avisos_do_770(&c);
    assert_eq!(avisos.len(), 2, "{avisos:?}");
    assert!(avisos.iter().any(|a| a.contains("SEM teto")), "{avisos:?}");
    // Escrever o proprio valor de fabrica nao e valor velho: sem aviso.
    let c = config_de(
        r#"{"token":"x","web":{"ligado":true,"sessao_minutos":15,"sessao_teto_horas":12}}"#,
    );
    assert!(avisos_do_770(&c).is_empty(), "{:?}", c.avisos);
}

/// O teto pelo caminho real: a sessao que o painel renova a cada 10 minutos
/// morre nas 12 h, e nao vive para sempre. RED medido pela guarda
/// `770-sessao-sem-teto` (o `usar` ignorando o `teto_ms`).
#[test]
fn o_teto_derruba_a_sessao_que_cada_clique_renovava() {
    let c = config_de(r#"{"token":"x","web":{"ligado":true}}"#);
    let mut vivas = crate::http::Sessoes::default();
    let t0 = 1_000_000;
    let id = vivas.nova("ana", c.web.sessao_ms(), t0);
    let dez_min = 10 * 60_000;
    let mut t = t0;
    let mut ultima_viva = t0;
    while t < t0 + 13 * 3_600_000 {
        t += dez_min;
        match vivas.usar(&id, c.web.sessao_ms(), c.web.sessao_teto_ms(), t) {
            Some(_) => ultima_viva = t,
            None => break,
        }
    }
    assert!(
        ultima_viva <= t0 + 12 * 3_600_000,
        "viveu ate {ultima_viva}"
    );
    assert!(
        ultima_viva > t0 + 11 * 3_600_000,
        "morreu cedo: {ultima_viva}"
    );
    // E o logins_vivos ja nao a conta depois do teto.
    assert!(vivas.logins_vivos(t0 + 12 * 3_600_000 + 1).is_empty());

    // Sem teto (o valor velho escrito), a mesma rotina vive as 13 h.
    let mut vivas = crate::http::Sessoes::default();
    let id = vivas.nova("ana", c.web.sessao_ms(), t0);
    let mut t = t0;
    while t < t0 + 13 * 3_600_000 {
        t += dez_min;
        assert!(vivas.usar(&id, c.web.sessao_ms(), 0, t).is_some());
    }
}

//! Jobs de execucao, pelo protocolo -- e sobretudo o portao de permissao.
//!
//! O que mais importa aqui nao e o relogio: e que um job rode com o poder do
//! usuario DELE, e nem um pingo a mais. Um agendador com poder proprio seria
//! uma porta dos fundos com hora marcada -- bastaria escrever no cadastro de
//! jobs a operacao que a rede recusaria.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "teste-dos-jobs";
const SENHA: &str = "segredo-de-teste";

fn porta_livre() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn pasta(nome: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "phxsql-jobs-{}-{}-{nome}",
        std::process::id(),
        porta_livre()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Sobe um servidor com o cadastro pedido.
///
/// `com_cadastro` falso e o servidor sem usuario nenhum -- o caso VELHO, que
/// nao pode mudar de comportamento por causa de uma regra nova.
fn subir(base: &std::path::Path, com_cadastro: bool) -> (Arc<Servidor>, u16) {
    let porta = porta_livre();
    // Uma iteracao so: a senha real nao interessa a este teste, e 210.000
    // iteracoes por login fariam a bateria levar segundos por nada.
    let h = phxsql_core::senha::cifrar_com(SENHA, 1);
    let usuarios = if com_cadastro {
        format!(
            r#""root": {{ "login": "root", "senha_hash": "{h}" }},
               "usuarios": [
                 {{ "id": 2, "login": "so_le", "nome": "So Le", "senha_hash": "{h}",
                    "ativo": true, "bases": {{ "*": {{ "ler": true, "administrar": true }} }} }},
                 {{ "id": 4, "login": "desligado", "nome": "Desligado",
                    "senha_hash": "{h}", "ativo": false, "supervisor": true }} ],"#
        )
    } else {
        String::new()
    };
    let texto = format!(
        r#"{{ "bind": "127.0.0.1:{porta}", "base": {base:?}, "token": "{TOKEN}",
              "log_acessos": {log:?}, "blacklist": {bl:?}, "dblink": {dbl:?},
              "jobs": {jobs:?}, {usuarios}
              "web": {{ "ligado": false }} }}"#,
        base = base.display().to_string(),
        log = base.join("acessos.log").display().to_string(),
        bl = base.join("blacklist.json").display().to_string(),
        dbl = base.join("dblink.json").display().to_string(),
        jobs = base.join("jobs.json").display().to_string(),
    );
    let c = Config::de_json(&Json::analisar(&texto).unwrap()).unwrap();
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(3);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return (s, porta);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("o servidor nao subiu na porta {porta}");
}

struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn nova(porta: u16) -> Ligacao {
        let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let f = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
        f.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        Ligacao {
            escrita: f.try_clone().unwrap(),
            leitor: BufReader::new(f),
        }
    }

    fn entrar(porta: u16, login: &str) -> Ligacao {
        let mut c = Ligacao::nova(porta);
        let r = c.pedir(&format!(
            r#""op":"login","usuario":"{login}","senha":"{SENHA}""#
        ));
        assert!(
            r.booleano_ou("ok", false),
            "login de {login}: {}",
            r.escrever()
        );
        c
    }

    /// O protocolo e JSON por LINHA: os pedidos escritos em varias linhas aqui
    /// viram uma so antes de sair, senao cada quebra terminaria um pedido pela
    /// metade.
    fn pedir(&mut self, corpo: &str) -> Json {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"))
    }
}

fn erro(j: &Json) -> String {
    j.texto_ou("erro", "").to_string()
}

/// O corpo da resposta. O protocolo embrulha tudo em `resultado`.
fn res(j: &Json) -> &Json {
    j.campo("resultado").unwrap_or(j)
}

/// Sem cadastro de usuarios o servidor inteiro entra sem login -- e o job
/// acompanha. Este e o teste do comportamento VELHO: regra nova que muda o
/// significado da configuracao que ja existe tira o direito de alguem sem
/// ninguem ter pedido.
#[test]
fn sem_cadastro_nada_muda() {
    let base = pasta("sem-cadastro");
    let (_s, porta) = subir(&base, false);
    let mut c = Ligacao::nova(porta);

    let r = c.pedir(
        r#""op":"job_salvar","job":{"nome":"pulso","ligado":true,
            "cada_minutos":60,"pedido":{"op":"ping"}}"#,
    );
    assert!(r.booleano_ou("ok", false), "{}", erro(&r));

    let r = c.pedir(r#""op":"job_rodar","nome":"pulso""#);
    assert!(r.booleano_ou("ok", false), "{}", erro(&r));
    let d = res(&r);
    assert!(d.booleano_ou("ok", false), "{}", d.escrever());
    assert!(
        !d.campo("resposta")
            .map(|x| x.texto_ou("phxsql", ""))
            .unwrap_or("")
            .is_empty(),
        "a resposta do ping tinha de vir junto: {}",
        r.escrever()
    );
}

/// **O ponto do item.** Um job carrega o poder do usuario dele, e nem um pingo
/// a mais. Quem so le nao ganha o direito de criar por escrever um job.
#[test]
fn o_job_roda_com_o_poder_do_usuario_dele() {
    let base = pasta("poder");
    let (_s, porta) = subir(&base, true);
    let mut root = Ligacao::entrar(porta, "root");

    // O root escreve um job que roda como `so_le` e tenta CRIAR um database.
    let r = root.pedir(
        r#""op":"job_salvar","job":{"nome":"abuso","usuario":"so_le",
            "cada_minutos":60,
            "pedido":{"op":"criar_database","database":"Roubada"}}"#,
    );
    assert!(r.booleano_ou("ok", false), "{}", erro(&r));

    let r = root.pedir(r#""op":"job_rodar","nome":"abuso""#);
    let d = res(&r);
    let detalhe = d.texto_ou("detalhe", "").to_string();
    assert!(
        !d.booleano_ou("ok", true),
        "o job nao podia criar: {}",
        r.escrever()
    );
    assert!(
        detalhe.contains("so_le") && detalhe.contains("criar"),
        "a recusa tem de nomear o usuario e a atividade: {detalhe}"
    );
    assert!(
        !base.join("Roubada").exists(),
        "o database foi criado apesar da recusa"
    );

    // E o mesmo job com um dono que PODE passa -- prova que a recusa foi do
    // portao de permissao e nao de outra coisa.
    let r = root.pedir(
        r#""op":"job_salvar","job":{"nome":"abuso","usuario":"root",
            "cada_minutos":60,
            "pedido":{"op":"criar_database","database":"Legitima"}}"#,
    );
    assert!(r.booleano_ou("ok", false), "{}", erro(&r));
    let r = root.pedir(r#""op":"job_rodar","nome":"abuso""#);
    assert!(res(&r).booleano_ou("ok", false), "{}", r.escrever());
    assert!(base.join("Legitima").exists(), "o database nao foi criado");
}

/// Usuario que sumiu do cadastro ou foi desativado PARA o job -- em vez de
/// cair para uma sessao sem dono, que rodaria com tudo liberado. E a recusa
/// vem no SALVAR: descobrir as tres da manha, no historico, e pior.
#[test]
fn job_sem_dono_valido_e_recusado_ao_salvar() {
    let base = pasta("dono");
    let (_s, porta) = subir(&base, true);
    let mut root = Ligacao::entrar(porta, "root");
    for (usuario, pedaco) in [
        ("fantasma", "nao esta no cadastro"),
        ("desligado", "desativado"),
        ("", "nao diz sob qual usuario"),
    ] {
        let r = root.pedir(&format!(
            r#""op":"job_salvar","job":{{"nome":"x","usuario":"{usuario}",
                "pedido":{{"op":"ping"}}}}"#
        ));
        assert!(
            !r.booleano_ou("ok", true),
            "{usuario:?} passou: {}",
            r.escrever()
        );
        assert!(erro(&r).contains(pedaco), "{usuario:?} -> {}", erro(&r));
    }
}

/// Ver a lista de jobs ja e poder de administrador: ela mostra que operacao
/// roda sobre que tabela, e sob que login. O apelido e o `job_ligar` entram
/// no MESMO teste porque o furo classico e a operacao que o portao esqueceu.
#[test]
fn a_lista_de_jobs_exige_administrar() {
    let base = pasta("lista");
    let (_s, porta) = subir(&base, true);
    let mut anonimo = Ligacao::nova(porta);
    for pedido in [
        r#""op":"jobs""#,
        r#""op":"job_listar""#,
        r#""op":"job_ligar","nome":"x","ligado":true"#,
    ] {
        let r = anonimo.pedir(pedido);
        assert!(!r.booleano_ou("ok", true), "{pedido}: {}", r.escrever());
        assert!(erro(&r).contains("login"), "{pedido}: {}", erro(&r));
    }
}

/// A ficha do job diz a agenda em texto, e a tela nao recalcula nada.
#[test]
fn a_ficha_do_job_traz_a_agenda_pronta() {
    let base = pasta("ficha");
    let (_s, porta) = subir(&base, false);
    let mut c = Ligacao::nova(porta);
    c.pedir(
        r#""op":"job_salvar","job":{"nome":"diario","hora":"03:30",
            "pedido":{"op":"backup"}}"#,
    );
    let r = c.pedir(r#""op":"jobs""#);
    let lista = res(&r).campo("jobs").and_then(Json::lista).unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].texto_ou("agenda", ""), "todo dia as 03:30");
    assert_eq!(lista[0].texto_ou("op", ""), "backup");
    assert!(!lista[0].booleano_ou("ligado", true), "nasce desligado");
}

/// O historico registra o que falhou, e nao so o que deu certo. Job que falha
/// calado nao existe.
#[test]
fn a_falha_entra_no_historico() {
    let base = pasta("historico");
    let (_s, porta) = subir(&base, false);
    let mut c = Ligacao::nova(porta);
    c.pedir(
        r#""op":"job_salvar","job":{"nome":"quebrado","cada_minutos":60,
            "pedido":{"op":"varrer","database":"NaoExiste","tabela":"Nem"}}"#,
    );
    let r = c.pedir(r#""op":"job_rodar","nome":"quebrado""#);
    assert!(
        !res(&r).booleano_ou("ok", true),
        "o job devia falhar: {}",
        r.escrever()
    );

    let r = c.pedir(r#""op":"jobs""#);
    let h = res(&r).campo("historico").and_then(Json::lista).unwrap();
    assert_eq!(h.len(), 1);
    assert_eq!(h[0].texto_ou("job", ""), "quebrado");
    assert!(!h[0].booleano_ou("ok", true));
    assert!(
        !h[0].texto_ou("detalhe", "").is_empty(),
        "o motivo tem de estar la"
    );
}

/// O resumo do historico ANALISA e reserializa -- nao recorta. Uma varredura
/// de vinte mil linhas vira "linhas: N itens", e nao meio JSON.
#[test]
fn o_historico_resume_analisando() {
    let base = pasta("resumo");
    let (_s, porta) = subir(&base, false);
    let mut c = Ligacao::nova(porta);
    for pedido in [
        r#""op":"criar_database","database":"C""#,
        r#""op":"criar_tabela","database":"C","tabela":"T",
            "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
            "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
        r#""op":"inserir","database":"C","tabela":"T","valores":{"id":1}"#,
    ] {
        let r = c.pedir(pedido);
        assert!(r.booleano_ou("ok", false), "{pedido}: {}", r.escrever());
    }
    c.pedir(
        r#""op":"job_salvar","job":{"nome":"le","cada_minutos":60,
            "pedido":{"op":"varrer","database":"C","tabela":"T"}}"#,
    );
    c.pedir(r#""op":"job_rodar","nome":"le""#);

    let r = c.pedir(r#""op":"jobs""#);
    let h = res(&r).campo("historico").and_then(Json::lista).unwrap();
    let d = h[0].texto_ou("detalhe", "");
    assert!(h[0].booleano_ou("ok", false), "{d}");
    assert!(d.contains("linhas: 1 itens"), "{d}");
    // E o corpo das linhas NAO esta la: o que nao se resume vira contagem.
    assert!(!d.contains("rowid"), "o corpo vazou para o historico: {d}");
}

/// O estado por job e a historia inteira numa palavra: nunca_rodou vira ok
/// quando roda bem, falhou quando quebra -- e a ultima corrida e a proxima
/// prevista vem juntas, para a tela nao recalcular nada.
#[test]
fn o_estado_conta_a_historia_do_job() {
    let base = pasta("estado");
    let (_s, porta) = subir(&base, false);
    let mut c = Ligacao::nova(porta);

    let ficha = |c: &mut Ligacao, nome: &str| {
        let r = c.pedir(r#""op":"jobs""#);
        res(&r)
            .campo("jobs")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .find(|j| j.texto_ou("nome", "") == nome)
            .unwrap()
            .clone()
    };

    // Nasce desligado: o estado diz isso antes de qualquer corrida.
    c.pedir(
        r#""op":"job_salvar","job":{"nome":"pulso","cada_minutos":5,
            "pedido":{"op":"ping"}}"#,
    );
    let f = ficha(&mut c, "pulso");
    assert_eq!(f.texto_ou("estado", ""), "desligado");
    assert!(f.campo("proximo_ms").unwrap().inteiro().is_none());
    assert!(f.campo("ultima").unwrap().campo("ok").is_none());

    // Ligado pelo `job_ligar` -- e neste processo nao ha relogio (nenhum job
    // estava ligado no arranque), entao ele esta ligado e abandonado.
    let r = c.pedir(r#""op":"job_ligar","nome":"pulso","ligado":true"#);
    assert!(r.booleano_ou("ok", false), "{}", erro(&r));
    assert!(!res(&r).booleano_ou("relogio_no_ar", true));
    let f = ficha(&mut c, "pulso");
    assert_eq!(f.texto_ou("estado", ""), "nunca_rodou");
    assert!(f.booleano_ou("parado", false), "vencido e sem relogio");
    assert!(
        f.campo("proximo_ms").unwrap().inteiro().is_some(),
        "ligado tem proxima prevista: {}",
        f.escrever()
    );

    // Rodou bem: ok, com a corrida inteira na ficha.
    c.pedir(r#""op":"job_rodar","nome":"pulso""#);
    let f = ficha(&mut c, "pulso");
    assert_eq!(f.texto_ou("estado", ""), "ok");
    assert!(
        !f.booleano_ou("parado", true),
        "rodou: parado nao esta mais"
    );
    let u = f.campo("ultima").unwrap();
    assert!(u.booleano_ou("ok", false), "{}", u.escrever());
    assert!(u.campo("duracao_ms").unwrap().inteiro().is_some());
    assert!(!u.texto_ou("quando", "").is_empty());

    // E um job quebrado fica "falhou", com o motivo na ultima corrida.
    c.pedir(
        r#""op":"job_salvar","job":{"nome":"quebrado","ligado":true,"cada_minutos":5,
            "pedido":{"op":"varrer","database":"NaoExiste","tabela":"Nem"}}"#,
    );
    c.pedir(r#""op":"job_rodar","nome":"quebrado""#);
    let f = ficha(&mut c, "quebrado");
    assert_eq!(f.texto_ou("estado", ""), "falhou");
    let u = f.campo("ultima").unwrap();
    assert!(!u.booleano_ou("ok", true));
    assert!(!u.texto_ou("detalhe", "").is_empty(), "o motivo tem de vir");
}

/// `job_listar` e apelido de `jobs`, e a resposta traz o que a tela precisa
/// alem da lista: se ha relogio e o estado do aviso por e-mail.
#[test]
fn job_listar_responde_com_relogio_e_aviso() {
    let base = pasta("listar");
    let (_s, porta) = subir(&base, false);
    let mut c = Ligacao::nova(porta);
    let r = c.pedir(r#""op":"job_listar""#);
    assert!(r.booleano_ou("ok", false), "{}", erro(&r));
    let d = res(&r);
    assert!(d.campo("relogio_no_ar").is_some(), "{}", d.escrever());
    let aviso = d.campo("aviso_email").unwrap();
    // Este servidor nao tem bloco de e-mail: o aviso diz isso, e nao ha
    // como um e-mail sair -- e o comportamento velho, garantido.
    assert!(!aviso.booleano_ou("ligado", true), "{}", aviso.escrever());
    assert!(!aviso.booleano_ou("avisar_jobs", true));
}

/// Liga e desliga sem reenviar a ficha -- e a mudanca persiste no arquivo.
#[test]
fn job_ligar_vira_so_a_chave() {
    let base = pasta("ligar");
    let (_s, porta) = subir(&base, false);
    let mut c = Ligacao::nova(porta);
    c.pedir(
        r#""op":"job_salvar","job":{"nome":"chave","descricao":"a ficha fica",
            "cada_minutos":7,"pedido":{"op":"ping"}}"#,
    );
    let r = c.pedir(r#""op":"job_ligar","nome":"chave","ligado":true"#);
    assert!(r.booleano_ou("ok", false), "{}", erro(&r));

    let texto = std::fs::read_to_string(base.join("jobs.json")).unwrap();
    let j = Json::analisar(&texto).unwrap();
    let job = &j.campo("jobs").and_then(Json::lista).unwrap()[0];
    assert!(job.booleano_ou("ligado", false), "persistiu: {texto}");
    assert_eq!(
        job.texto_ou("descricao", ""),
        "a ficha fica",
        "so a chave muda"
    );
    assert_eq!(job.inteiro_ou("cada_minutos", 0), 7);

    // Sem "ligado" o pedido e recusado com o campo nomeado -- e nao tratado
    // como false, que desligaria por engano.
    let r = c.pedir(r#""op":"job_ligar","nome":"chave""#);
    assert!(!r.booleano_ou("ok", true));
    assert!(erro(&r).contains("ligado"), "{}", erro(&r));
    // E job que nao existe avisa.
    let r = c.pedir(r#""op":"job_ligar","nome":"fantasma","ligado":true"#);
    assert!(erro(&r).contains("nao existe"), "{}", erro(&r));
}

#[test]
fn nome_de_job_hostil_nao_entra() {
    let base = pasta("nome");
    let (_s, porta) = subir(&base, false);
    let mut c = Ligacao::nova(porta);
    for nome in ["../fora", "com espaco", ""] {
        let r = c.pedir(&format!(
            r#""op":"job_salvar","job":{{"nome":"{nome}","pedido":{{"op":"ping"}}}}"#
        ));
        assert!(!r.booleano_ou("ok", true), "{nome:?}: {}", r.escrever());
    }
}

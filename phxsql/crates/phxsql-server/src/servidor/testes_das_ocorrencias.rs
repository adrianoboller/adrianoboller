//! Pedido 495, fatia F2: a camada de ocorrencias pelo servidor de verdade.
//!
//! As provas da camada sozinha moram em `crate::ocorrencias`; aqui mora a
//! que so o servidor faz -- o correio compartilhado com a saude do disco e
//! a thread `sonda-disco` como o carteiro unico.

use super::*;
use crate::apoio_teste::DirTemp;

fn config_base(dir: &std::path::Path) -> Config {
    Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    }
}

/// **O caminho real:** a camada mora no MESMO correio da saude do disco, e
/// o carteiro de verdade leva o alarme marcado na tarefa ao
/// `ocorrencias.log`, ao lado do `acessos.log` -- redigido, com quem e de
/// onde.
///
/// Vermelho: sem o laco do carteiro levar a carta `Ocorrencia` ao
/// `gravar_ocorrencia`, o arquivo fica vazio e o prazo de 10 s vence.
#[test]
fn o_alarme_da_tarefa_chega_redigido_ao_ocorrencias_log_pelo_carteiro() {
    let dir = DirTemp::novo("ocorrencias-f2");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    assert!(
        Arc::ptr_eq(s.saude.correio(), s.ocorrencias.correio()),
        "dois correios: dois carteiros"
    );
    s.ligar_sonda_de_disco();
    let sentinela = "SENTINELA-servidor-f2-k7";
    let agora = crate::agora_ms();
    let a = s
        .telemetria
        .entrar("dados:f2", "dados", "127.0.0.1", 1, agora)
        .expect("a telemetria nasce ligada");
    {
        let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
        a.comecou_pedido("sql", "root", "loja", "", agora);
        crate::telemetria::sinal(
            crate::aquario::Alarme::SenhaEmClaro,
            &format!(r#"{{"op":"sql","texto":"CREATE USER c PASSWORD '{sentinela}'"}}"#),
        );
        a.terminou_pedido("root");
    }
    let caminho = dir.join(crate::ocorrencias::NOME_DO_ARQUIVO);
    let ate = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let texto = loop {
        let t = std::fs::read_to_string(&caminho).unwrap_or_default();
        if t.contains("senha_em_claro") || std::time::Instant::now() > ate {
            break t;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert!(texto.contains(r#""alarme":"senha_em_claro""#), "{texto:?}");
    assert!(texto.contains(r#""ip":"127.0.0.1""#), "{texto}");
    assert!(texto.contains(r#""tarefa":"dados:f2""#), "{texto}");
    assert!(!texto.contains(sentinela), "a sentinela vazou: {texto}");
}

// ------------------------------------------------------------------ F9

/// Marca uma ocorrencia de verdade -- pelo `sinal`, com a atividade amarrada
/// como a conexao faz -- e a deixa para o carteiro. `dados` e o pedido CRU,
/// com o literal dentro: quem o redige e a F2, e e isso que se prova.
fn marcar(s: &Arc<Servidor>, ip: &str, alarme: crate::aquario::Alarme, dados: &str) {
    let agora = crate::agora_ms();
    let a = s
        .telemetria
        .entrar(&format!("dados:{ip}"), "dados", ip, 1, agora)
        .expect("a telemetria nasce ligada");
    {
        let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
        a.comecou_pedido("sql", "root", "loja", "clientes", agora);
        assert!(
            crate::telemetria::sinal(alarme, dados).is_some(),
            "o silencio calou a ocorrencia de {ip}"
        );
        a.terminou_pedido("root");
    }
}

/// Espera o `ocorrencias.log` ter `n` linhas (o carteiro e outra thread).
fn esperar_linhas(dir: &std::path::Path, n: usize) -> String {
    let caminho = dir.join(crate::ocorrencias::NOME_DO_ARQUIVO);
    let ate = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let t = std::fs::read_to_string(&caminho).unwrap_or_default();
        if t.lines().count() >= n || std::time::Instant::now() > ate {
            return t;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn pedir(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
    let mut sessao = Sessao::default();
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t","op":"ocorrencias"{corpo}}}"#),
        &mut sessao,
        "127.0.0.1",
    );
    r
}

/// **A op `ocorrencias` (F9).** Le o arquivo que a F2 escreveu, ja redigido:
/// o literal do pedido que disparou o alarme nao volta; o filtro `alarme`
/// separa; o `max` corta; o nome errado e erro, e nao lista vazia; e a lista
/// dos alarmes vem do codigo.
///
/// Vermelho medido: com o filtro ignorado (`|_| true` no lugar do `quer`),
/// o pedido de `forca_bruta` devolve as duas linhas e o teste cai.
#[test]
fn a_op_ocorrencias_le_redigido_e_filtra_por_alarme_periodo_e_max() {
    let dir = DirTemp::novo("ocorrencias-f9-op");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    s.ligar_sonda_de_disco();
    let literal = "LITERAL-f9-q3z";
    marcar(
        &s,
        "10.9.0.1",
        crate::aquario::Alarme::SenhaEmClaro,
        &format!(r#"{{"op":"sql","texto":"SELECT * FROM clientes WHERE nome = '{literal}'"}}"#),
    );
    marcar(&s, "10.9.0.2", crate::aquario::Alarme::ForcaBruta, "login");
    let texto = esperar_linhas(&dir, 2);
    assert_eq!(texto.lines().count(), 2, "{texto}");

    let tudo = pedir(&s, "").unwrap();
    let escrito = tudo.escrever();
    let linhas = tudo.campo("linhas").and_then(Json::lista).unwrap();
    assert_eq!(linhas.len(), 2, "{escrito}");
    assert!(!escrito.contains(literal), "o literal voltou: {escrito}");
    assert!(
        escrito.contains("nome = ?"),
        "a forma redigida tem de vir: {escrito}"
    );
    let alarmes = tudo.campo("alarmes").and_then(Json::lista).unwrap();
    assert_eq!(alarmes.len(), crate::aquario::Alarme::TODOS.len());

    let so = pedir(&s, r#","alarme":"forca_bruta""#).unwrap();
    let l = so.campo("linhas").and_then(Json::lista).unwrap();
    assert_eq!(l.len(), 1, "{}", so.escrever());
    assert_eq!(l[0].texto_ou("ip", ""), "10.9.0.2");

    let lista = pedir(&s, r#","alarme":["forca_bruta","senha_em_claro"],"max":1"#).unwrap();
    assert_eq!(
        lista.campo("linhas").and_then(Json::lista).unwrap().len(),
        1
    );
    assert!(lista.booleano_ou("truncado", false));

    let futuro = crate::agora_ms() + 3_600_000;
    let nada = pedir(&s, &format!(r#","desde":{futuro}"#)).unwrap();
    assert!(nada
        .campo("linhas")
        .and_then(Json::lista)
        .unwrap()
        .is_empty());

    let e = pedir(&s, r#","alarme":"forca_brutta""#)
        .unwrap_err()
        .to_string();
    assert!(e.contains("alarme desconhecido"), "{e}");
    assert!(e.contains("forca_bruta"), "o erro diz os que existem: {e}");
}

/// Liga o rele falso, com o aviso de seguranca pedido ou nao.
fn com_rele(c: &mut Config, porta: u16, avisar: bool) {
    c.alertas.email.ligado = true;
    c.alertas.email.avisar_seguranca = avisar;
    c.alertas.email.servidor = "127.0.0.1".into();
    c.alertas.email.porta = porta;
    c.alertas.email.de = "phxsql@exemplo.com".into();
    c.alertas.email.para = vec!["admin@exemplo.com".into()];
    c.alertas.email.timeout_s = 5;
}

/// **O e-mail da ocorrencia (F9), nos dois sentidos, pelo SMTP falso.**
/// Com `avisar_seguranca` ligado, a ocorrencia vermelha manda UM e-mail --
/// com o alarme, quem e de onde, e sem o literal nem o pedido --, e a
/// amarela nao manda. Sem o interruptor, nada sai, e o arquivo grava igual.
///
/// Vermelho medido: sem a chamada `avisar_ocorrencia_por_email` no
/// `gravar_ocorrencia`, o primeiro sentido estoura a espera do rele.
#[test]
fn a_ocorrencia_vermelha_avisa_por_email_so_com_o_interruptor() {
    use crate::apoio_teste::rele_falso;
    use std::time::Duration;
    let literal = "LITERAL-f9-email-w8";
    let pedido = format!(r#"{{"op":"sql","texto":"ALTER USER c PASSWORD '{literal}'"}}"#);

    // Ligado.
    let dir = DirTemp::novo("ocorrencias-f9-email");
    let (porta, caixa) = rele_falso();
    let mut c = config_base(&dir);
    com_rele(&mut c, porta, true);
    let s = Servidor::novo(c).unwrap();
    s.ligar_sonda_de_disco();
    // O amarelo primeiro: medir o controle depois do vermelho deixaria o
    // teste passar por engano, com o e-mail do vermelho na caixa.
    marcar(
        &s,
        "10.9.1.1",
        crate::aquario::Alarme::IntegridadeRecusada,
        "inserir",
    );
    esperar_linhas(&dir, 1);
    assert!(
        caixa.recv_timeout(Duration::from_millis(700)).is_err(),
        "a ocorrencia amarela mandou e-mail"
    );
    marcar(
        &s,
        "10.9.1.2",
        crate::aquario::Alarme::SenhaEmClaro,
        &pedido,
    );
    let bruto = caixa
        .recv_timeout(Duration::from_secs(15))
        .expect("nenhum e-mail chegou ao rele depois da ocorrencia vermelha");
    let (cabecalho, corpo) = bruto.split_once("\r\n\r\n").unwrap();
    assert!(cabecalho.contains("To: admin@exemplo.com"), "{cabecalho}");
    let texto = phxsql_core::base64::decodificar_texto(&corpo.replace("\r\n", "")).unwrap();
    assert!(texto.contains("senha_em_claro"), "{texto}");
    assert!(texto.contains("10.9.1.2"), "{texto}");
    assert!(!texto.contains(literal), "o literal foi no e-mail: {texto}");
    assert!(
        !texto.contains("PASSWORD"),
        "o pedido foi no e-mail: {texto}"
    );
    // O silencio e por alarme: o mesmo alarme de OUTRO IP nao manda segundo.
    marcar(
        &s,
        "10.9.1.3",
        crate::aquario::Alarme::SenhaEmClaro,
        &pedido,
    );
    esperar_linhas(&dir, 3);
    assert!(
        caixa.recv_timeout(Duration::from_millis(700)).is_err(),
        "variar o IP furou o silencio do e-mail"
    );

    // Desligado: o comportamento de quem configurou o rele so para o disco.
    let dir = DirTemp::novo("ocorrencias-f9-sem-email");
    let (porta, caixa) = rele_falso();
    let mut c = config_base(&dir);
    com_rele(&mut c, porta, false);
    let s = Servidor::novo(c).unwrap();
    s.ligar_sonda_de_disco();
    marcar(
        &s,
        "10.9.2.1",
        crate::aquario::Alarme::SenhaEmClaro,
        &pedido,
    );
    let texto = esperar_linhas(&dir, 1);
    assert!(
        texto.contains("senha_em_claro"),
        "o arquivo nao pode depender do e-mail: {texto}"
    );
    assert!(
        caixa.recv_timeout(Duration::from_millis(1500)).is_err(),
        "avisar_seguranca falso mandou e-mail assim mesmo"
    );
}

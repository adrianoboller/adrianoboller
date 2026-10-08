use super::*;

fn dir_temp(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("fw-{rotulo}"))
}

fn config_base(dir: &std::path::Path) -> Config {
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        ..Config::default()
    };
    // O ESCAPE ESCRITO, e ele esta aqui por assunto: desde 18/09/2026 a
    // cifra do fio nasce exigida (pedido 370), e estes testes conectam em
    // claro porque o que eles medem e OUTRA coisa. Sem esta linha, a
    // recusa que eles leriam seria a da cifra, e a prova mediria o portao
    // errado -- teste que passa (ou falha) por engano.
    c.cifra_fio.exigir = false;
    c
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// `replicacao_ligar` e o pedido que o laco consome -- e so ele: a
/// operacao nao religa nada por conta propria, porque quem sabe em que
/// passo o laco esta e o laco. Pedido 203.
#[test]
fn replicacao_ligar_deixa_o_pedido_para_o_laco_consumir() {
    let dir = dir_temp("religar");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    // Origem desconhecida: erro nomeado, e nada e marcado.
    let e = s
        .op_replicacao_ligar(&pedido(r#"{"origem":"ninguem"}"#))
        .unwrap_err();
    assert!(matches!(e, PhxError::NaoEncontrado(_)), "{e}");
    assert!(!s.tomar_religar("ninguem"));

    // Um laco estacionado por credencial recusada.
    s.anotar_estado("matriz", |e| e.parada = "credencial_recusada".into());
    let r = s
        .op_replicacao_ligar(&pedido(r#"{"origem":"matriz"}"#))
        .unwrap();
    assert_eq!(r.texto_ou("estava_parada", ""), "credencial_recusada");
    assert!(r.booleano_ou("pedido", false));
    // A operacao NAO limpa a parada: e o laco, ao acordar, que a limpa.
    let estado = s.op_replicacao_estado().unwrap();
    let matriz = estado.campo("origens").unwrap().campo("matriz").unwrap();
    assert_eq!(matriz.texto_ou("parada", ""), "credencial_recusada");
    // O laco consome o pedido UMA vez.
    assert!(s.tomar_religar("matriz"));
    assert!(!s.tomar_religar("matriz"));

    // Estacionada, a origem tambem NAO se testa: `replicacao_testar` e
    // ligar nela com a credencial recusada, e cada teste conta la.
    let e = s
        .op_replicacao_testar(&pedido(r#"{"origem":"matriz"}"#), &Sessao::default())
        .unwrap_err();
    assert!(
        matches!(&e, PhxError::Esquema(m) if m.contains("replicacao_ligar")),
        "a recusa nomeia o caminho de volta: {e}"
    );

    // O laco acordou e limpou a parada (e o que `apos_a_falha` faz ao
    // consumir o pedido). Dali em diante o laco so dorme, e o pedido vale
    // como «tente ja»: estava_parada volta nulo, e o pedido fica igual.
    s.anotar_estado("matriz", |e| e.parada.clear());
    // E sem parada o teste segue o caminho de sempre -- aqui, ate a
    // recusa de sempre, porque "matriz" nao esta em replicacao.origens.
    let e = s
        .op_replicacao_testar(&pedido(r#"{"origem":"matriz"}"#), &Sessao::default())
        .unwrap_err();
    assert!(matches!(e, PhxError::NaoEncontrado(_)), "{e}");
    let r = s
        .op_replicacao_ligar(&pedido(r#"{"origem":"matriz"}"#))
        .unwrap();
    assert!(
        matches!(r.campo("estava_parada"), Some(Json::Nulo)),
        "{r:?}"
    );
    assert!(s.tomar_religar("matriz"));
}

/// **O teste que mais importa**: sem o bloco `seguranca` no config, nada
/// do firewall novo muda o comportamento -- nenhum comando e proibido,
/// nenhuma tentativa conta, nenhum IP bloqueia.
#[test]
fn sem_bloco_seguranca_nada_muda() {
    let dir = dir_temp("sem-bloco");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let mut sessao = Sessao::default();
    for _ in 0..10 {
        let (_, _, r) = s.despachar(r#"{"token":"t","op":"bancos"}"#, &mut sessao, "203.0.113.5");
        r.unwrap();
    }
    // Ate operacao "perigosa" passa pelo portao de politica sem contar.
    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"excluir_tabela","database":"x","tabela":"y"}"#,
        &mut sessao,
        "203.0.113.5",
    );
    // O erro e do motor (a base nao existe), nunca da politica.
    let e = r.unwrap_err();
    assert!(
        !e.to_string().contains("proibida"),
        "sem bloco seguranca nao ha comando proibido: {e}"
    );
    assert!(s.barrado("203.0.113.5", crate::agora_ms()).is_none());
    // E sem tabela de mensagens nem idioma, phxsys nao nasce sozinho.
    assert!(
        !dir.join("phxsys").exists(),
        "phxsys nao pode nascer sem alguem pedir"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// **O teste que mais importa do pedido 215: o comportamento VELHO.**
///
/// Sem `seguranca.contar_injecao_sql` -- que e o padrao e o que todo
/// `config.json` de hoje tem --, comando empilhado continua sendo erro de
/// sintaxe e nada mais: ninguem bloqueia, e o IP continua entrando. E o
/// que a bancada mediu antes desta rodada (172.620 tentativas por minuto,
/// `blacklist.json` vazio antes e depois), e e o que este teste trava.
#[test]
fn sem_o_interruptor_a_injecao_nao_bloqueia_ninguem() {
    let dir = dir_temp("inj-desligado");
    let mut c = config_base(&dir);
    c.politica.tentativas_ate_bloquear = 3;
    let s = Servidor::novo(c).unwrap();
    let mut sessao = Sessao::default();
    for _ in 0..20 {
        let (_, _, r) = s.despachar(
            r#"{"token":"t","op":"sql","database":"loja",
                    "texto":"SELECT * FROM clientes; DROP TABLE clientes"}"#,
            &mut sessao,
            "203.0.113.40",
        );
        assert!(r.is_err(), "o comando empilhado tem de ser recusado");
    }
    assert!(
        s.barrado("203.0.113.40", crate::agora_ms()).is_none(),
        "com o interruptor desligado ninguem pode ser bloqueado"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// **Prova real, com o defeito reposto.** Ligado o interruptor, o comando
/// empilhado conta pela politica leve que ja existe e o IP cai na
/// blacklist na enesima -- pelo mesmo `violacao_leve` do token invalido,
/// sem portao novo. Tirando a chamada do `despachar`, este teste volta a
/// falhar na hora.
#[test]
fn com_o_interruptor_o_comando_empilhado_bloqueia_na_enesima() {
    let dir = dir_temp("inj-ligado");
    let mut c = config_base(&dir);
    c.politica.contar_injecao_sql = true;
    c.politica.tentativas_ate_bloquear = 3;
    let s = Servidor::novo(c).unwrap();
    let mut sessao = Sessao::default();
    for tentativa in 1..=3 {
        let (_, _, r) = s.despachar(
            r#"{"token":"t","op":"sql","database":"loja",
                    "texto":"SELECT * FROM clientes; DROP TABLE clientes; --"}"#,
            &mut sessao,
            "203.0.113.41",
        );
        assert!(r.is_err(), "tentativa {tentativa}");
    }
    let b = s
        .barrado("203.0.113.41", crate::agora_ms())
        .expect("tres comandos empilhados tinham de bloquear");
    assert!(
        b.motivo.contains("empilhado"),
        "o bloqueio tem de dizer o motivo: {}",
        b.motivo
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// O falso positivo e o que mataria a guarda: SQL LEGITIMO que da erro --
/// tabela que nao existe, `FROM` escrito errado, comentario no fim -- nao
/// e injecao, e vinte recusas dessas nao podem bloquear quem escreve
/// consulta a mao. Quem decide e o lexico do motor, e nao um casador de
/// texto.
#[test]
fn sql_legitimo_recusado_nao_conta_como_injecao() {
    let dir = dir_temp("inj-falso");
    let mut c = config_base(&dir);
    c.politica.contar_injecao_sql = true;
    c.politica.tentativas_ate_bloquear = 3;
    let s = Servidor::novo(c).unwrap();
    let mut sessao = Sessao::default();
    for texto in [
        "SELECT * FROM clientes",
        "SELECT * FROM clientes;",
        "SELECT * FRON clientes",
        "SELECT * FROM clientes WHERE nome = 'Alves' -- e o resto",
        "SELECT /* comentario */ * FROM clientes",
        // O veneno como DADO -- a bateria grava exatamente este valor.
        "SELECT * FROM clientes WHERE nome = '; DROP TABLE clientes; --'",
        "SELECT * FROM clientes WHERE nome = '' OR '1'='1",
    ] {
        for _ in 0..5 {
            let pedido = Json::objeto(vec![
                ("token", Json::texto_de("t")),
                ("op", Json::texto_de("sql")),
                ("database", Json::texto_de("loja")),
                ("texto", Json::texto_de(texto)),
            ]);
            let (_, _, r) = s.despachar(&pedido.escrever(), &mut sessao, "203.0.113.42");
            assert!(r.is_err(), "sem base, todos estes erram: {texto}");
        }
        assert!(
            s.barrado("203.0.113.42", crate::agora_ms()).is_none(),
            "SQL legitimo recusado nao pode bloquear: {texto}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// O ajudante do pedido 216: a drenagem para na quebra de linha, devolve
/// quantos bytes jogou fora, e o que vem DEPOIS da quebra continua no
/// leitor -- senao ela comeria o proximo pedido.
#[test]
fn a_drenagem_para_na_quebra_e_nao_come_o_proximo_pedido() {
    use std::io::BufReader;
    let bruto: Vec<u8> = b"o resto da linha gigante\n{\"op\":\"ping\"}\n".to_vec();
    let mut leitor = BufReader::new(&bruto[..]);
    assert_eq!(descartar_ate_a_quebra(&mut leitor, 1024), 25);
    let mut sobrou = String::new();
    leitor.read_line(&mut sobrou).unwrap();
    assert_eq!(sobrou, "{\"op\":\"ping\"}\n");

    // Sem quebra nenhuma, o teto e quem manda -- e ele nao pode ser
    // ultrapassado, senao a drenagem viraria a memoria que o teto existe
    // para nao gastar.
    let sem_quebra = vec![b'A'; 5000];
    let mut leitor = BufReader::new(&sem_quebra[..]);
    assert!(descartar_ate_a_quebra(&mut leitor, 100) >= 100);
}

/// O comportamento VELHO do comando proibido: bloqueio na primeira, com o
/// mesmo texto de sempre, byte a byte.
#[test]
fn comando_proibido_bloqueia_na_primeira_como_sempre() {
    let dir = dir_temp("grave-velho");
    let mut c = config_base(&dir);
    c.politica.comandos_proibidos = vec!["excluir_tabela".into()];
    let s = Servidor::novo(c).unwrap();
    let mut sessao = Sessao::default();
    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"excluir_tabela","database":"x","tabela":"y"}"#,
        &mut sessao,
        "203.0.113.9",
    );
    assert_eq!(
        r.unwrap_err().to_string(),
        "[SP000025] acesso negado: operacao excluir_tabela esta proibida neste servidor; o IP foi bloqueado"
    );
    assert!(s.barrado("203.0.113.9", crate::agora_ms()).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

use crate::apoio_teste::rele_falso;

/// Liga o rele falso nesta configuracao, com o aviso de seguranca pedido.
fn com_rele(c: &mut Config, porta: u16, avisar: bool) {
    c.alertas.email.ligado = true;
    c.alertas.email.avisar_seguranca = avisar;
    c.alertas.email.servidor = "127.0.0.1".into();
    c.alertas.email.porta = porta;
    c.alertas.email.de = "phxsql@exemplo.com".into();
    c.alertas.email.para = vec!["admin@exemplo.com".into()];
    c.alertas.email.timeout_s = 5;
}

/// O aviso ao administrador, nos DOIS sentidos: o comando proibido bloqueia
/// E manda e-mail; o permitido nao manda nada.
///
/// Ate 07/09/2026 a violacao grave so escrevia no erro padrao: o IP era
/// bloqueado e ninguem era avisado. Reponha o defeito tirando a chamada a
/// `avisar_violacao_por_email` e este teste falha na espera do rele.
#[test]
fn comando_proibido_avisa_o_administrador_por_email() {
    let dir = dir_temp("grave-email");
    let (porta, caixa) = rele_falso();
    let mut c = config_base(&dir);
    c.politica.comandos_proibidos = vec!["excluir_tabela".into()];
    com_rele(&mut c, porta, true);
    let s = Servidor::novo(c).unwrap();
    let mut sessao = Sessao::default();

    // Sentido 1 -- o PERMITIDO nao manda nada. Vem primeiro de proposito:
    // o silencio por IP e por chave, e medir o controle depois do proibido
    // deixaria o teste passar por engano.
    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"bancos"}"#,
        &mut sessao,
        "203.0.113.20",
    );
    r.unwrap();
    assert!(
        caixa.recv_timeout(Duration::from_millis(700)).is_err(),
        "comando permitido gerou e-mail"
    );

    // Sentido 2 -- o PROIBIDO recusa, bloqueia e manda.
    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"excluir_tabela","database":"x","tabela":"y"}"#,
        &mut sessao,
        "203.0.113.21",
    );
    assert!(r.unwrap_err().to_string().contains("o IP foi bloqueado"));
    assert!(s.barrado("203.0.113.21", crate::agora_ms()).is_some());

    let bruto = caixa
        .recv_timeout(Duration::from_secs(15))
        .expect("nenhum e-mail chegou ao rele depois do comando proibido");
    let (cabecalho, corpo) = bruto.split_once("\r\n\r\n").unwrap();
    assert!(cabecalho.contains("To: admin@exemplo.com"), "{cabecalho}");
    assert!(
        cabecalho.contains("203.0.113.21"),
        "o assunto tem de nomear o IP: {cabecalho}"
    );
    let texto = phxsql_core::base64::decodificar_texto(&corpo.replace("\r\n", "")).unwrap();
    assert!(texto.contains("203.0.113.21"), "{texto}");
    assert!(texto.contains("excluir_tabela"), "{texto}");
    assert!(texto.contains("comando proibido pela politica"), "{texto}");
    // Senha nunca em texto puro: o corpo leva o IP, o motivo e a operacao,
    // e NUNCA o pedido -- um `login` recusado carrega a senha nele.
    assert!(!texto.contains("token"), "o corpo vazou o pedido: {texto}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Guarda nova entra PEDIDA, nao imposta: sem `avisar_seguranca`, o
/// bloqueio continua acontecendo exatamente como antes e e-mail nenhum
/// sai. E o comportamento de quem configurou o rele so para o disco.
#[test]
fn sem_avisar_seguranca_o_bloqueio_acontece_calado() {
    let dir = dir_temp("grave-calado");
    let (porta, caixa) = rele_falso();
    let mut c = config_base(&dir);
    c.politica.comandos_proibidos = vec!["excluir_tabela".into()];
    com_rele(&mut c, porta, false);
    let s = Servidor::novo(c).unwrap();
    let mut sessao = Sessao::default();
    let (_, _, r) = s.despachar(
        r#"{"token":"t","op":"excluir_tabela","database":"x","tabela":"y"}"#,
        &mut sessao,
        "203.0.113.22",
    );
    assert!(r.unwrap_err().to_string().contains("o IP foi bloqueado"));
    assert!(
        s.barrado("203.0.113.22", crate::agora_ms()).is_some(),
        "o bloqueio nao pode depender do e-mail"
    );
    assert!(
        caixa.recv_timeout(Duration::from_millis(700)).is_err(),
        "avisar_seguranca falso mandou e-mail assim mesmo"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A guarda pedida: com `tentativas_para_bloqueio: 3`, as duas primeiras
/// recusam e CONTAM -- e a resposta diz isso --, e a terceira bloqueia.
#[test]
fn tentativas_para_bloqueio_recusa_sempre_e_bloqueia_na_enesima() {
    let dir = dir_temp("grave-contado");
    let mut c = config_base(&dir);
    c.politica.comandos_proibidos = vec!["excluir_tabela".into()];
    c.politica.tentativas_para_bloqueio = 3;
    let s = Servidor::novo(c).unwrap();
    let mut sessao = Sessao::default();
    let ip = "203.0.113.10";
    let proibido = r#"{"token":"t","op":"excluir_tabela","database":"x","tabela":"y"}"#;

    for n in 1..=2 {
        let (_, _, r) = s.despachar(proibido, &mut sessao, ip);
        let texto = r.unwrap_err().to_string();
        assert!(
            texto.contains(&format!("tentativa {n} de 3")),
            "a resposta tem de dizer a contagem: {texto}"
        );
        assert!(
            s.barrado(ip, crate::agora_ms()).is_none(),
            "bloqueou antes da hora"
        );
    }
    let (_, _, r) = s.despachar(proibido, &mut sessao, ip);
    assert!(r.unwrap_err().to_string().contains("o IP foi bloqueado"));
    assert!(s.barrado(ip, crate::agora_ms()).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Whitelist NUNCA bloqueia: a recusa continua, o IP fica livre -- e a
/// resposta nao mente dizendo que bloqueou. Vale ate para bloqueio ja
/// gravado antes de a regra entrar: a whitelist vence na conexao.
#[test]
fn whitelist_recusa_sem_bloquear_e_vence_bloqueio_gravado() {
    let dir = dir_temp("whitelist");
    let mut c = config_base(&dir);
    c.politica.comandos_proibidos = vec!["excluir_tabela".into()];
    c.politica.whitelist = vec!["192.168.50.0/24".into()];
    let s = Servidor::novo(c).unwrap();
    let mut sessao = Sessao::default();
    let proibido = r#"{"token":"t","op":"excluir_tabela","database":"x","tabela":"y"}"#;

    for _ in 0..5 {
        let (_, _, r) = s.despachar(proibido, &mut sessao, "192.168.50.20");
        assert_eq!(
            r.unwrap_err().to_string(),
            "[SP000025] acesso negado: operacao excluir_tabela esta proibida neste servidor"
        );
    }
    assert!(s.barrado("192.168.50.20", crate::agora_ms()).is_none());

    // Bloqueio gravado ANTES de a whitelist dinamica entrar: a regra
    // nova vence na proxima conexao, sem esperar o bloqueio vencer.
    let (_, _, r) = s.despachar(proibido, &mut sessao, "10.0.0.7");
    assert!(r.unwrap_err().to_string().contains("o IP foi bloqueado"));
    assert!(s.barrado("10.0.0.7", crate::agora_ms()).is_some());
    s.executar(
        "whitelist_salvar",
        &pedido(r#"{"whitelist":["10.0.0.7"]}"#),
        &Sessao::default(),
    )
    .unwrap();
    assert!(
        s.barrado("10.0.0.7", crate::agora_ms()).is_none(),
        "whitelist tem de vencer bloqueio ja gravado"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// As operacoes de gestao: listar traz whitelist e politica, salvar
/// valida, exportar entrega uma linha por IP.
#[test]
fn ops_de_bloqueio_listam_salvam_e_exportam() {
    let dir = dir_temp("ops");
    let mut c = config_base(&dir);
    c.politica.comandos_proibidos = vec!["reindexar".into()];
    c.politica.whitelist = vec!["127.0.0.1".into()];
    let s = Servidor::novo(c).unwrap();
    let sessao = Sessao::default();
    let mut anonima = Sessao::default();

    let proibido = r#"{"token":"t","op":"reindexar","database":"x","tabela":"y"}"#;
    let _ = s.despachar(proibido, &mut anonima, "203.0.113.77");

    let b = s.executar("bloqueios", &pedido("{}"), &sessao).unwrap();
    assert_eq!(b.campo("ativos").and_then(Json::lista).unwrap().len(), 1);
    assert_eq!(
        b.campo("whitelist_config")
            .and_then(Json::lista)
            .unwrap()
            .len(),
        1
    );
    let politica = b.campo("politica").unwrap();
    assert_eq!(politica.inteiro_ou("tentativas_para_bloqueio", 0), 1);
    assert_eq!(
        politica
            .campo("comandos_proibidos")
            .and_then(Json::lista)
            .unwrap()
            .len(),
        1
    );

    let e = s
        .executar(
            "bloqueios_exportar",
            &pedido(r#"{"formato":"iptables"}"#),
            &sessao,
        )
        .unwrap();
    assert_eq!(
        e.texto_ou("texto", ""),
        "iptables -I INPUT -s 203.0.113.77 -j DROP\n"
    );

    // Salvar recusa lixo inteiro, sem gravar metade.
    let ruim = s.executar(
        "whitelist_salvar",
        &pedido(r#"{"whitelist":["10.0.0.1","banana"]}"#),
        &sessao,
    );
    assert!(ruim.is_err());
    let b = s.executar("bloqueios", &pedido("{}"), &sessao).unwrap();
    assert!(b
        .campo("whitelist")
        .and_then(Json::lista)
        .unwrap()
        .is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A semeadura cria `phxsys.mensagens` com uma linha por mensagem de
/// fabrica -- e semear de novo nao toca em nada.
#[test]
fn mensagens_semear_cria_tudo_e_e_idempotente() {
    let dir = dir_temp("semear");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let sessao = Sessao::default();

    let r = s
        .executar("mensagens_semear", &pedido("{}"), &sessao)
        .unwrap();
    assert!(r.booleano_ou("criou_database", false));
    assert!(r.booleano_ou("criou_tabela", false));
    assert_eq!(
        r.inteiro_ou("semeadas", 0) as usize,
        crate::mensagens::FABRICA.len()
    );

    let de_novo = s
        .executar("mensagens_semear", &pedido("{}"), &sessao)
        .unwrap();
    assert!(!de_novo.booleano_ou("criou_tabela", true));
    assert_eq!(de_novo.inteiro_ou("semeadas", -1), 0);

    // O estado diz a verdade para a tela.
    let m = s.executar("mensagens", &pedido("{}"), &sessao).unwrap();
    assert!(m.booleano_ou("existe", false));
    assert_eq!(
        m.inteiro_ou("linhas", 0) as usize,
        crate::mensagens::FABRICA.len()
    );
    assert_eq!(m.texto_ou("idioma", ""), "Portugues");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Provas (g), (h) e (j) da rodada: com `idioma: Ingles` o texto humano
/// sai da coluna Ingles e o CODIGO estruturado nao muda; celula vazia cai
/// **Catraca: os campos do erro saem de UM lugar so.**
///
/// Eram tres copias, e a `sprint` teria de entrar nas tres. Quem
/// acrescentar um quarto construtor a mao reprova aqui, em vez de a
/// quarta resposta calar um campo que as outras dizem. E a mesma
/// armadilha do portao de permissao que so olhava o campo `"tabela"`.
#[test]
fn os_campos_do_erro_saem_de_um_lugar_so() {
    let fonte = FONTE_DO_SERVIDOR;
    let quantos = fonte
        .matches("(\"classe\", Json::texto_de(e.classe()))")
        .count();
    assert_eq!(
        quantos, 1,
        "ha {quantos} construtores de resposta de erro; tem de haver 1 \
             (`campos_do_erro`) -- veja o que o novo esqueceu de dizer"
    );
}

/// **A OpenAPI descreve a resposta que existe, e nao a de antes.**
///
/// O esquema do erro e uma lista escrita a mao dentro do `rest.rs`, longe
/// de quem monta a resposta -- foi por isso que a `sprint` quase entrou
/// so no servidor e deixou o contrato publicado mentindo. Em vez de
/// confiar, compara-se: os campos do erro de verdade contra as
/// `properties` do `Erro` na OpenAPI.
#[test]
fn a_openapi_descreve_os_campos_do_erro_que_existem() {
    let dir = dir_temp("openapi-erro");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let e = PhxError::Integridade("x".into());
    let mut reais: Vec<String> = s
        .campos_do_erro("inserir", &e, 1)
        .into_iter()
        .map(|(k, _)| k.to_string())
        .collect();

    let doc = crate::rest::openapi(&crate::config::Rest::default(), "0.0.0");
    let props = doc
        .campo("components")
        .and_then(|c| c.campo("schemas"))
        .and_then(|c| c.campo("Erro"))
        .and_then(|c| c.campo("properties"))
        .expect("a OpenAPI nao tem components.schemas.Erro.properties");
    let mut descritos: Vec<String> = props.chaves().into_iter().map(String::from).collect();

    // So o `ms` fica de fora, e MEDIDO: a OpenAPI descreve o `op` (eu
    // tinha suposto que nao, e o teste corrigiu a suposicao). O `ms` e
    // do envelope de tempo, nao da recusa.
    reais.retain(|k| k != "ms");
    reais.sort();
    descritos.sort();
    assert_eq!(
        reais, descritos,
        "a OpenAPI e a resposta divergiram nos campos do erro"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A sprint chega como CAMPO, e nao so dentro da frase.
///
/// E o que faz a moldura ser conveniencia de quem le o log, e nao um
/// formato que alguem precise analisar de volta.
#[test]
fn a_sprint_vem_no_campo_e_na_frase() {
    let dir = dir_temp("sprint-campo");
    let s = Servidor::novo(config_base(&dir)).unwrap();
    let e = PhxError::Integridade("mae 7 nao existe".into());
    let r = s.resposta_erro("inserir", &e, 1);
    assert_eq!(r.texto_ou("sprint", ""), "SP000008");
    assert!(
        r.texto_ou("erro", "").starts_with("[SP000008] "),
        "{}",
        r.texto_ou("erro", "")
    );
    // E os campos de sempre continuam intactos.
    assert_eq!(r.inteiro_ou("codigo", 0), 3006);
    assert_eq!(r.texto_ou("nome", ""), "INTEGRIDADE");
    let _ = std::fs::remove_dir_all(&dir);
}

/// para o portugues; sem idioma no config, portugues de sempre.
#[test]
fn idioma_troca_o_texto_e_nunca_o_codigo() {
    let dir = dir_temp("idioma");
    let mut c = config_base(&dir);
    c.idioma = "Ingles".into();
    // Com idioma configurado, a semeadura acontece sozinha no arranque.
    let s = Servidor::novo(c).unwrap();
    assert!(dir.join("phxsys/mensagens.reg").exists());

    let e = PhxError::EmCarga("clientes reservada".into());
    // (g) o texto vem da coluna Ingles...
    assert_eq!(
        s.texto_do_erro(&e),
        "[SP000012] table under bulk load: clientes reservada"
    );
    // ...e o campo estruturado continua identico ao de sempre.
    let resposta = s.resposta_erro("bulkinsert", &e, 3);
    assert_eq!(resposta.inteiro_ou("codigo", 0), 4002);
    assert_eq!(resposta.texto_ou("nome", ""), "EM_CARGA");
    assert!(resposta.booleano_ou("repetir", false));

    // (h) celula Ingles vazia (a fabrica nao traduz esta): portugues.
    let v = PhxError::VersaoNaoSuportada {
        arquivo: "x.reg".into(),
        encontrada: 9,
        suportada: 2,
    };
    assert_eq!(s.texto_do_erro(&v), v.to_string());

    // (j) outro servidor, mesma base semeada, SEM idioma: portugues de
    // sempre, byte a byte.
    let s2 = Servidor::novo(config_base(&dir)).unwrap();
    assert_eq!(s2.texto_do_erro(&e), e.to_string());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Prova (i): linha excluida da tabela = texto de fabrica byte a byte.
/// Prova (k): editar o texto pela operacao comum de tabela vale SEM
/// reiniciar -- o servidor rele pelo mtime, como a blacklist.
#[test]
fn editar_e_excluir_linhas_vale_sem_reiniciar() {
    let dir = dir_temp("editar");
    let mut c = config_base(&dir);
    c.idioma = "Ingles".into();
    let s = Servidor::novo(c).unwrap();
    let sessao = Sessao::default();

    // Aquece o cache com a tabela semeada.
    let e = PhxError::Duplicado("porNome".into());
    assert_eq!(s.texto_do_erro(&e), "[SP000020] duplicate key: porNome");

    // Acha a linha de erro.duplicado na tabela, como a grade acharia.
    let v = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"phxsys","tabela":"mensagens","max":1000}"#),
            &sessao,
        )
        .unwrap();
    let linhas = v.campo("linhas").and_then(Json::lista).unwrap();
    let alvo = linhas
        .iter()
        .find(|l| l.texto_ou("TextName", "") == "erro.duplicado")
        .expect("a linha semeada sumiu");
    let rowid = alvo.inteiro_ou("rowid", 0);

    // (k) edita a coluna Ingles pela operacao comum de atualizar -- a
    // linha inteira, como a ficha da tela manda.
    let mut valores: Vec<(String, Json)> = ["id", "TextName"]
        .iter()
        .chain(crate::mensagens::IDIOMAS.iter())
        .map(|c| (c.to_string(), alvo.campo(c).cloned().unwrap_or(Json::Nulo)))
        .collect();
    for (nome, valor) in &mut valores {
        if nome == "Ingles" {
            *valor = Json::texto_de("key already used: {detalhe}");
        }
    }
    s.executar(
        "atualizar",
        &pedido(&format!(
            r#"{{"database":"phxsys","tabela":"mensagens","rowid":{rowid},"valores":{}}}"#,
            Json::Objeto(valores).escrever()
        )),
        &sessao,
    )
    .unwrap();
    // O cache rele quando o intervalo de conferencia passa -- e o teste
    // espera esse intervalo de proposito: reiniciar nao pode ser preciso.
    std::thread::sleep(crate::mensagens::INTERVALO_DE_CONFERENCIA + Duration::from_millis(200));
    assert_eq!(s.texto_do_erro(&e), "[SP000020] key already used: porNome");

    // (i) excluir a linha devolve o texto de fabrica, byte a byte.
    s.executar(
        "excluir",
        &pedido(&format!(
            r#"{{"database":"phxsys","tabela":"mensagens","rowid":{rowid}}}"#
        )),
        &sessao,
    )
    .unwrap();
    std::thread::sleep(crate::mensagens::INTERVALO_DE_CONFERENCIA + Duration::from_millis(200));
    assert_eq!(s.texto_do_erro(&e), e.to_string());
    let _ = std::fs::remove_dir_all(&dir);
}

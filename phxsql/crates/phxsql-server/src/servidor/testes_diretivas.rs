/* ======================================================== as DIRETIVAS

O `SHOW … SETTINGS`, o `ALTER … SET`, o diario administrativo e a primeira
diretiva POR BANCO -- `comandos_proibidos`, o pedido 220.

O que estes testes protegem, em ordem de importancia:

1. **O comportamento VELHO.** Um `config.json` que so tem strings na
   `comandos_proibidos` continua significando exatamente o que significava.
   Guarda nova entra pedida.
2. **O portao e UM so.** `ALTER SERVER SET` desemboca no `config_gravar`, e
   quem nao tem `administrar` e recusado pelas duas portas.
3. **A politica por banco so APERTA.** O global continua valendo em todo
   banco, e o do banco nao afrouxa nada. */
use super::*;
use crate::usuarios::{Cadastro, Nivel, Permissoes, Usuario};

/// Um servidor de arquivo, com um bloco `seguranca` a escolha.
fn servidor_de_arquivo(nome: &str, seguranca: &str) -> (Arc<Servidor>, PathBuf, DirTemp) {
    let dir = DirTemp::novo(&format!("diretivas-{nome}"));
    let caminho = dir.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            "{{\n  \"_nota\": \"comentario que a gravacao nao pode comer\",\n  \
                 \"token\": \"t\",\n  \"bind\": \"127.0.0.1:5399\",\n  \
                 \"base\": \"{}\",\n  \"max_linhas\": 1000{seguranca}\n}}\n",
            dir.join("dados").display()
        ),
    )
    .unwrap();
    let mut c = Config::ler(&caminho).unwrap();
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    c.cadastro = Cadastro::default();
    (Servidor::novo(c).unwrap(), caminho, dir)
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Sessao de um IP qualquer -- o portao de politica bloqueia o IP, e sem
/// um endereco os testes de recusa nao exercitam o caminho de verdade.
fn sessao(ip: &str) -> Sessao {
    Sessao {
        ip: ip.to_string(),
        ..Sessao::default()
    }
}

/// Um usuario com tudo MENOS `administrar`.
fn operador() -> Usuario {
    Usuario {
        id: 7,
        nome: "Operador".into(),
        login: "op".into(),
        senha_hash: String::new(),
        email: String::new(),
        telefone: String::new(),
        supervisor: false,
        ativo: true,
        nivel: Nivel::Nenhum,
        chave_publica: None,
        bases: vec![(
            "*".into(),
            Permissoes {
                ler: true,
                inserir: true,
                alterar: true,
                excluir: true,
                reindexar: true,
                administrar: false,
                ..Permissoes::default()
            },
        )],
        tabelas: Vec::new(),
        colunas: Vec::new(),
    }
}

fn criar_bases(s: &Arc<Servidor>) {
    for b in ["erp", "loja"] {
        s.executar(
            "criar_database",
            &pedido(&format!(r#"{{"database":"{b}"}}"#)),
            &Sessao::default(),
        )
        .unwrap();
    }
}

// ------------------------------------------------- o comportamento velho

/// **O teste que mais importa.** Sem entrada por banco no `config.json`,
/// nada muda para ninguem: a bandeira nasce apagada e o portao novo nao
/// custa nem uma trava.
#[test]
fn sem_regra_por_banco_nada_muda() {
    let (s, _c, _g) = servidor_de_arquivo(
        "velho",
        ",\n  \"seguranca\":{\"comandos_proibidos\":[\"excluir_tabela\"]}",
    );
    assert!(
        !s.ha_proibidos_por_base.load(Ordering::Relaxed),
        "a bandeira nasceu ligada sem entrada por banco: todo pedido passa a pagar um lock"
    );
    criar_bases(&s);
    // O global continua proibindo em TODA base -- byte a byte o de antes.
    let (_op, _ok, r) = s.despachar(
        r#"{"op":"excluir_tabela","token":"t","database":"erp","tabela":"x"}"#,
        &mut sessao("10.0.0.1"),
        "10.0.0.1",
    );
    let e = r.expect_err("o global parou de proibir");
    assert!(e.to_string().contains("proibida neste servidor"), "{e}");
    // E o que nao esta na lista passa pelo portao (o erro que volta e de
    // DADO, e nao de politica).
    let (_op, _ok, r) = s.despachar(
        r#"{"op":"reindexar","token":"t","database":"erp","tabela":"x"}"#,
        &mut sessao("10.0.0.2"),
        "10.0.0.2",
    );
    let e = r.expect_err("reindexar de tabela inexistente devia falhar por dado");
    assert!(
        !e.to_string().contains("proibida"),
        "reindexar foi barrado por politica sem estar em lista nenhuma: {e}"
    );
}

// -------------------------------------------- o pedido 220, por banco

/// A proibicao por banco vale NAQUELE banco, e nao nos outros. Se valesse
/// nos dois, a forma por banco seria um global com nome enfeitado -- e o
/// pedido 220 continuaria aberto com cara de fechado.
#[test]
fn o_proibido_do_banco_so_vale_naquele_banco() {
    let (s, _c, _g) = servidor_de_arquivo(
        "por-banco",
        ",\n  \"seguranca\":{\"comandos_proibidos\":[{\"comando\":\"reindexar\",\"database\":\"erp\"}],\
             \"whitelist\":[\"10.0.0.0/8\"]}",
    );
    assert!(s.ha_proibidos_por_base.load(Ordering::Relaxed));
    criar_bases(&s);

    let (_op, _ok, r) = s.despachar(
        r#"{"op":"reindexar","token":"t","database":"erp","tabela":"x"}"#,
        &mut sessao("10.0.0.1"),
        "10.0.0.1",
    );
    let e = r.expect_err("reindexar em erp devia ser proibido");
    assert!(
        e.to_string().contains("proibida no banco erp"),
        "a recusa nao diz QUAL banco: {e}"
    );

    // O controle positivo, sem o qual o de cima passaria com um portao
    // que recusa tudo.
    let (_op, _ok, r) = s.despachar(
        r#"{"op":"reindexar","token":"t","database":"loja","tabela":"x"}"#,
        &mut sessao("10.0.0.1"),
        "10.0.0.1",
    );
    let e = r.expect_err("tabela inexistente");
    assert!(
        !e.to_string().contains("proibida"),
        "a regra de erp vazou para loja: {e}"
    );
}

/// E o global continua valendo em TODO banco -- o por banco acrescenta,
/// nunca substitui.
#[test]
fn o_global_continua_valendo_em_todo_banco() {
    let (s, _c, _g) = servidor_de_arquivo(
        "global-e-banco",
        ",\n  \"seguranca\":{\"comandos_proibidos\":[\"excluir_tabela\",\
             {\"comando\":\"reindexar\",\"database\":\"erp\"}],\"whitelist\":[\"10.0.0.0/8\"]}",
    );
    criar_bases(&s);
    for base in ["erp", "loja"] {
        let (_op, _ok, r) = s.despachar(
            &format!(r#"{{"op":"excluir_tabela","token":"t","database":"{base}","tabela":"x"}}"#),
            &mut sessao("10.0.0.1"),
            "10.0.0.1",
        );
        let e = r.expect_err("o global parou de valer em {base}");
        assert!(
            e.to_string().contains("proibida neste servidor"),
            "{base}: {e}"
        );
    }
}

/// `ALTER DATABASE … SET comandos_proibidos` grava, aplica A QUENTE e
/// nao mexe no global.
#[test]
fn alter_database_grava_aplica_a_quente_e_nao_toca_no_global() {
    let (s, caminho, _g) = servidor_de_arquivo(
        "alter-base",
        ",\n  \"seguranca\":{\"comandos_proibidos\":[\"excluir_tabela\"],\"whitelist\":[\"10.0.0.0/8\"]}",
    );
    criar_bases(&s);
    // Antes: reindexar passa em erp.
    let (_op, _ok, r) = s.despachar(
        r#"{"op":"reindexar","token":"t","database":"erp","tabela":"x"}"#,
        &mut sessao("10.0.0.1"),
        "10.0.0.1",
    );
    assert!(!r.unwrap_err().to_string().contains("proibida"));

    let r = s
        .executar(
            "sql",
            &pedido(
                r#"{"texto":"ALTER DATABASE erp SET comandos_proibidos = (reindexar) MOTIVO 'auditoria'"}"#,
            ),
            &Sessao::default(),
        )
        .unwrap();
    let dentro = r.campo("resultado").unwrap();
    assert!(dentro.booleano_ou("gravado", false), "{}", r.escrever());
    assert_eq!(dentro.textos("acrescentados"), vec!["reindexar"]);

    // (1) a quente, sem reiniciar nada.
    let (_op, _ok, r) = s.despachar(
        r#"{"op":"reindexar","token":"t","database":"erp","tabela":"x"}"#,
        &mut sessao("10.0.0.1"),
        "10.0.0.1",
    );
    assert!(
        r.unwrap_err().to_string().contains("proibida no banco erp"),
        "o aperto so valeria no proximo arranque"
    );
    // (2) e so em erp.
    let (_op, _ok, r) = s.despachar(
        r#"{"op":"reindexar","token":"t","database":"loja","tabela":"x"}"#,
        &mut sessao("10.0.0.1"),
        "10.0.0.1",
    );
    assert!(!r.unwrap_err().to_string().contains("proibida"));
    // (3) no arquivo, com o comentario preservado e o global intacto.
    let texto = std::fs::read_to_string(&caminho).unwrap();
    assert!(
        texto.contains("comentario que a gravacao nao pode comer"),
        "{texto}"
    );
    let arvore = Json::analisar(&texto).unwrap();
    let seg = arvore.campo("seguranca").unwrap();
    assert_eq!(
        crate::blacklist::proibidos_globais(seg),
        vec!["excluir_tabela"],
        "a diretiva por banco mexeu no global: {texto}"
    );
    assert_eq!(
        crate::blacklist::proibidos_por_base(seg),
        vec![("erp".to_string(), "reindexar".to_string())]
    );
}

/// **Ela so ACRESCENTA.** Lista vazia recusa dizendo por onde se retira --
/// e nao apaga nada em silencio, que seria a pior das duas.
#[test]
fn a_diretiva_por_banco_nao_retira_nada() {
    let (s, caminho, _g) = servidor_de_arquivo(
        "so-acrescenta",
        ",\n  \"seguranca\":{\"comandos_proibidos\":[{\"comando\":\"reindexar\",\"database\":\"erp\"}]}",
    );
    criar_bases(&s);
    let e = s
        .executar(
            "sql",
            &pedido(r#"{"texto":"ALTER DATABASE erp SET comandos_proibidos = ()"}"#),
            &Sessao::default(),
        )
        .expect_err("a lista vazia devia recusar");
    assert!(e.to_string().contains("so ACRESCENTA"), "{e}");
    // E o que estava la continua la.
    let texto = std::fs::read_to_string(&caminho).unwrap();
    let seg = Json::analisar(&texto)
        .unwrap()
        .campo("seguranca")
        .cloned()
        .unwrap();
    assert_eq!(
        crate::blacklist::proibidos_por_base(&seg).len(),
        1,
        "{texto}"
    );
}

/// Repetir o que ja esta la nao duplica a entrada -- e diz que ja estava.
#[test]
fn repetir_a_mesma_proibicao_nao_duplica() {
    let (s, _c, _g) = servidor_de_arquivo("repetido", "");
    criar_bases(&s);
    for _ in 0..2 {
        s.executar(
            "sql",
            &pedido(r#"{"texto":"ALTER DATABASE erp SET comandos_proibidos = (reindexar)"}"#),
            &Sessao::default(),
        )
        .unwrap();
    }
    let r = s
        .executar(
            "sql",
            &pedido(r#"{"texto":"ALTER DATABASE erp SET comandos_proibidos = (reindexar)"}"#),
            &Sessao::default(),
        )
        .unwrap();
    let dentro = r.campo("resultado").unwrap();
    assert!(dentro.textos("acrescentados").is_empty());
    assert_eq!(dentro.textos("ja_existiam"), vec!["reindexar"]);
    assert_eq!(
        dentro.textos("comandos_proibidos_da_base"),
        vec!["reindexar"]
    );
}

/// Proibir um nome que nao existe deixa a lista com uma guarda que nunca
/// fecha -- e quem escreveu acha que fechou a porta.
#[test]
fn proibir_operacao_inexistente_recusa() {
    let (s, _c, _g) = servidor_de_arquivo("inexistente", "");
    criar_bases(&s);
    let e = s
        .executar(
            "sql",
            &pedido(r#"{"texto":"ALTER DATABASE erp SET comandos_proibidos = (voar)"}"#),
            &Sessao::default(),
        )
        .expect_err("proibir uma op inexistente devia recusar");
    assert!(e.to_string().contains("nao e uma operacao"), "{e}");
}

// ------------------------------------------------------ o portao unico

/// **Quem nao administra nao ve e nao muda.** As tres portas novas
/// conferem pelo MESMO portao do `config_gravar` -- e a afirmacao e sobre
/// o ERRO, e nao sobre «terminou»: um teste que aceitasse sucesso *e*
/// recusa passaria com a porta dos fundos aberta.
#[test]
fn sem_administrar_a_recusa_e_de_permissao() {
    let (s, _c, _g) = servidor_de_arquivo("portao-2", "");
    let ses = Sessao {
        usuario: Some(operador()),
        ..Sessao::default()
    };
    for (op, ped) in [
        ("diretivas", r#"{"escopo":"servidor"}"#),
        (
            "diretiva_gravar",
            r#"{"escopo":"servidor","campo":"max_linhas","valor":7}"#,
        ),
        (
            "diretiva_gravar",
            r#"{"escopo":"database","database":"erp","campo":"comandos_proibidos","valor":["reindexar"]}"#,
        ),
    ] {
        let e = s.executar(op, &pedido(ped), &ses).expect_err(&format!(
            "{op} passou para quem NAO tem administrar: porta dos fundos"
        ));
        assert!(
            matches!(e, PhxError::Autorizacao(_)),
            "{op} recusou por outro motivo: {e}"
        );
        assert!(e.to_string().contains("administrar"), "{op}: {e}");
    }
    // E o controle positivo: sem usuario (token de servico) as tres passam.
    s.executar(
        "diretivas",
        &pedido(r#"{"escopo":"servidor"}"#),
        &Sessao::default(),
    )
    .unwrap();
}

/// `ALTER SERVER SET` desemboca no MESMO `config_gravar`: mesma validacao
/// de tipo, mesma gravacao, mesmo efeito a quente.
#[test]
fn alter_server_e_o_mesmo_caminho_do_config_gravar() {
    let (s, caminho, _g) = servidor_de_arquivo("alter-server", "");
    assert_eq!(s.max_linhas(), 1000);
    let r = s
        .executar(
            "sql",
            &pedido(r#"{"texto":"ALTER SERVER SET max_linhas = 7 MOTIVO 'teste'"}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(r.texto_ou("op", ""), "diretiva_gravar");
    assert_eq!(s.max_linhas(), 7, "o teto novo nao valeu a quente");
    let texto = std::fs::read_to_string(&caminho).unwrap();
    assert!(texto.contains("\"max_linhas\": 7"), "{texto}");
    assert!(texto.contains("comentario que a gravacao nao pode comer"));

    // A conferencia de TIPO e a mesma -- e ela e a que impede gravar um
    // valor que o leitor jogaria fora em silencio.
    let e = s
        .executar(
            "sql",
            &pedido(r#"{"texto":"ALTER SERVER SET max_linhas = abc"}"#),
            &Sessao::default(),
        )
        .expect_err("texto num campo inteiro devia recusar");
    assert!(e.to_string().contains("espera inteiro"), "{e}");
}

// ---------------------------------------------------------- o diario

/// **Os nove campos do dono, gravados pelas DUAS portas.** O diario entra
/// no `config_gravar`, que e onde as duas desembocam -- registrar so no
/// `ALTER` deixaria de fora tudo o que a tela de Configuracoes grava.
#[test]
fn o_diario_registra_as_duas_portas_com_os_nove_campos() {
    let (s, _c, _g) = servidor_de_arquivo("diario", "");
    let mut ses = sessao("192.168.50.20");
    ses.usuario = Some({
        let mut u = operador();
        u.login = "ana".into();
        u.nivel = Nivel::Admin;
        u.supervisor = true;
        u
    });
    // Pela porta do protocolo (a tela).
    s.executar(
        "config_gravar",
        &pedido(r#"{"campos":{"max_linhas":42},"motivo":"pela tela"}"#),
        &ses,
    )
    .unwrap();
    // Pela porta do SQL.
    s.executar(
        "sql",
        &pedido(r#"{"texto":"ALTER SERVER SET espelho = TRUE MOTIVO 'pelo SQL'"}"#),
        &ses,
    )
    .unwrap();

    let linhas = s.diario.ultimas(10);
    assert_eq!(linhas.len(), 2, "o diario nao pegou as duas portas");
    // A mais recente primeiro.
    assert_eq!(linhas[0].recurso, "espelho");
    assert_eq!(linhas[0].motivo, "pelo SQL");
    assert_eq!(linhas[0].valor_anterior, Json::Nulo);
    assert_eq!(linhas[0].valor_novo, Json::Bool(true));
    assert_eq!(linhas[1].recurso, "max_linhas");
    assert_eq!(linhas[1].motivo, "pela tela");
    assert_eq!(
        linhas[1].valor_anterior,
        Json::Numero(1000.0),
        "o diario nao diz de que valor se saiu"
    );
    assert_eq!(linhas[1].valor_novo, Json::Numero(42.0));
    for l in &linhas {
        assert_eq!(l.usuario, "ana", "o diario nao diz QUEM");
        assert_eq!(l.ip_origem, "192.168.50.20", "o diario nao diz DE ONDE");
        assert_eq!(l.servidor, "127.0.0.1:5399");
    }
}

/// O `SHOW … SETTINGS` traz o diario junto, e e o que faz alguem le-lo.
#[test]
fn o_show_traz_o_diario_junto() {
    let (s, _c, _g) = servidor_de_arquivo("diario-no-show", "");
    s.executar(
        "sql",
        &pedido(r#"{"texto":"ALTER SERVER SET max_linhas = 42 MOTIVO 'porque sim'"}"#),
        &Sessao::default(),
    )
    .unwrap();
    let r = s
        .executar(
            "sql",
            &pedido(r#"{"texto":"SHOW SERVER SETTINGS"}"#),
            &Sessao::default(),
        )
        .unwrap();
    let dentro = r.campo("resultado").unwrap();
    let diario = dentro.campo("diario").and_then(Json::lista).unwrap();
    assert_eq!(diario.len(), 1, "{}", dentro.escrever());
    assert_eq!(diario[0].texto_ou("motivo", ""), "porque sim");
    // E sem pedir, nao vem: `diario: 0` e a resposta so das diretivas.
    let so = s
        .executar(
            "diretivas",
            &pedido(r#"{"escopo":"servidor"}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert!(so.campo("diario").is_none());
}

// ------------------------------------------------------ o que se recusa

/// As duas recusas com MOTIVO: elas nomeiam o caminho que funciona, em vez
/// de dizer so «nao».
#[test]
fn alter_table_e_alter_connection_recusam_dizendo_o_caminho() {
    let (s, _c, _g) = servidor_de_arquivo("recusas", "");
    criar_bases(&s);
    for (sql, pedaco) in [
        (
            "ALTER TABLE clientes SET duplicate_check = TRUE",
            "criar_tabela",
        ),
        (
            "ALTER TABLE clientes SET referential_integrity = FALSE",
            "NASCE conferida",
        ),
        ("ALTER CONNECTION SET compression = TRUE", "nao existe"),
        (
            "ALTER DATABASE erp SET transactions = TRUE",
            "comandos_proibidos",
        ),
    ] {
        let e = s
            .executar(
                "sql",
                &pedido(&format!(r#"{{"texto":"{sql}","database":"erp"}}"#)),
                &Sessao::default(),
            )
            .expect_err(&format!("{sql:?} devia recusar"));
        assert!(
            e.to_string().contains(pedaco),
            "{sql:?} recusou sem dizer o caminho ({pedaco:?}): {e}"
        );
    }
}

/// **O `SHOW` nao vaza segredo.** A regra e por NOME de campo, e o teste e
/// nos dois sentidos: nenhum valor sigiloso sai, e os comuns saem.
#[test]
fn o_show_server_settings_nao_vaza_segredo() {
    let (s, _c, _g) = servidor_de_arquivo("segredo", "");
    let r = s
        .executar(
            "diretivas",
            &pedido(r#"{"escopo":"servidor"}"#),
            &Sessao::default(),
        )
        .unwrap();
    let texto = r.escrever();
    assert!(!texto.contains("\"t\""), "o token vazou: {texto}");
    let recursos = r.campo("recursos").and_then(Json::lista).unwrap();
    assert!(
        recursos.len() > 40,
        "a lista de diretivas encolheu: {}",
        recursos.len()
    );
    // O controle positivo: um valor comum SAI, senao a varredura acima so
    // provaria que a resposta esta vazia.
    let m = recursos
        .iter()
        .find(|x| x.texto_ou("recurso", "") == "max_linhas")
        .expect("max_linhas sumiu da lista");
    assert_eq!(m.campo("valor").unwrap().numero(), Some(1000.0));
    assert_eq!(m.texto_ou("aplica", ""), "a quente");
    // E o que exige reinicio diz isso, em vez de prometer efeito.
    let t = recursos
        .iter()
        .find(|x| x.texto_ou("recurso", "") == "timeout_s")
        .unwrap();
    assert_eq!(t.texto_ou("aplica", ""), "exige reinicio");
}

/// **O que esta GRAVADO e ainda nao vale aparece.**
///
/// Achado exercitando: o `SHOW` devolvia o valor VIVO, entao quem acabava
/// de gravar `timeout_s = 45` recebia 30 de volta, calado -- e nao tinha
/// como saber que o 45 estava no arquivo. E o mesmo defeito que a tela de
/// Configuracoes ja tinha pago, reaparecido pela porta nova.
#[test]
fn o_show_diz_o_que_esta_gravado_e_ainda_nao_vale() {
    let (s, _c, _g) = servidor_de_arquivo("no-arquivo", "");
    s.executar(
        "sql",
        &pedido(r#"{"texto":"ALTER SERVER SET timeout_s = 45"}"#),
        &Sessao::default(),
    )
    .unwrap();
    let r = s
        .executar(
            "diretivas",
            &pedido(r#"{"escopo":"servidor"}"#),
            &Sessao::default(),
        )
        .unwrap();
    let recursos = r.campo("recursos").and_then(Json::lista).unwrap();
    let t = recursos
        .iter()
        .find(|x| x.texto_ou("recurso", "") == "timeout_s")
        .unwrap();
    assert_eq!(t.campo("valor").unwrap().numero(), Some(30.0), "o vivo");
    assert_eq!(
        t.campo("no_arquivo").map(|v| v.numero()),
        Some(Some(45.0)),
        "o SHOW escondeu o valor ja gravado: {}",
        t.escrever()
    );
    assert!(t.booleano_ou("esperando_reinicio", false));
    // O controle positivo: o campo que aplica A QUENTE nao ganha o par --
    // se ganhasse, «esperando reinicio» perderia o sentido.
    s.executar(
        "sql",
        &pedido(r#"{"texto":"ALTER SERVER SET max_linhas = 42"}"#),
        &Sessao::default(),
    )
    .unwrap();
    let r = s
        .executar(
            "diretivas",
            &pedido(r#"{"escopo":"servidor"}"#),
            &Sessao::default(),
        )
        .unwrap();
    let recursos = r.campo("recursos").and_then(Json::lista).unwrap();
    let m = recursos
        .iter()
        .find(|x| x.texto_ou("recurso", "") == "max_linhas")
        .unwrap();
    assert_eq!(m.campo("valor").unwrap().numero(), Some(42.0));
    assert!(m.campo("no_arquivo").is_none(), "{}", m.escrever());
}

/// `SHOW TABLE … SETTINGS` le a DECLARACAO da tabela -- e nao um bloco de
/// configuracao paralelo, que seria uma segunda verdade ao lado do esquema.
#[test]
fn show_table_settings_le_o_indice_e_a_chave() {
    let (s, _c, _g) = servidor_de_arquivo("show-tabela", "");
    criar_bases(&s);
    let ses = Sessao::default();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"erp","tabela":"clientes",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &ses,
    )
    .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"erp","tabela":"pedidos",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"cliente_id","tipo":"Int4"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true},
                               {"nome":"porCliente","colunas":["cliente_id"],"unico":false}]}"#,
        ),
        &ses,
    )
    .unwrap();
    s.executar(
        "declarar_fk",
        &pedido(
            r#"{"database":"erp","tabela":"pedidos","nome":"fk_cliente",
                    "colunas":["cliente_id"],"tabela_ref":"clientes","colunas_ref":["id"]}"#,
        ),
        &ses,
    )
    .unwrap();

    let r = s
        .executar(
            "sql",
            &pedido(r#"{"texto":"SHOW TABLE pedidos SETTINGS","database":"erp"}"#),
            &ses,
        )
        .unwrap();
    let d = r.campo("resultado").unwrap();
    let dup = d.campo("duplicidade").and_then(Json::lista).unwrap();
    assert_eq!(dup.len(), 2);
    // `duplicate_check` do HFSQL e o `unico` daqui: um indice unico
    // confere duplicidade, o outro nao.
    assert!(dup
        .iter()
        .any(|x| x.texto_ou("indice", "") == "porId" && x.booleano_ou("duplicate_check", false)));
    assert!(dup.iter().any(
        |x| x.texto_ou("indice", "") == "porCliente" && !x.booleano_ou("duplicate_check", true)
    ));
    let fks = d
        .campo("integridade_referencial")
        .and_then(Json::lista)
        .unwrap();
    assert_eq!(fks.len(), 1);
    // A petrea, lida pela diretiva: a chave NASCE conferida, e o excluir
    // so aceita restringir.
    assert!(fks[0].booleano_ou("referential_integrity", false));
    assert_eq!(fks[0].texto_ou("ao_excluir", ""), "Restringir");
}

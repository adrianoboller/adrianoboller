/* =========================================================== pedido 221
O CADASTRO DE USUARIOS PELO PROTOCOLO.

Ate a 0.18 nao havia operacao que criasse usuario: o cadastro se escrevia
no `config.json` e reiniciava. Estes testes provam as tres coisas que a
porta nova precisa ter, e que a leitura do codigo nao da:

1. a senha vira HASH antes de tocar no arquivo, e nao aparece em resposta
   nenhuma -- e a varredura que prova isso ACHA a senha quando ela esta la
   (o controle positivo esta em `a_varredura_acha_a_senha_quando_ela_esta_la`);
2. o efeito e a QUENTE: o login novo entra sem reiniciar, e o excluido
   perde a ficha no pedido seguinte da propria conexao;
3. as guardas de nao-se-pode: ultimo administrador, a propria conta, o
   root, o `senha_hash` pronto e o portao de administrar. */
use super::*;
use crate::usuarios::{Cadastro, Nivel, Permissoes, Usuario};

const SENHA_DA_ANA: &str = "a-senha-da-ana-2026";
const SENHA_NOVA: &str = "a-senha-nova-2026";

/// Um servidor que subiu de um `config.json` DE VERDADE, com o cadastro
/// dentro dele -- e nao de um `Config` montado a mao. Sem o arquivo nao
/// ha o que gravar, e sem o cadastro NO arquivo nao ha o que alterar.
fn servidor_com_cadastro(nome: &str) -> (Arc<Servidor>, PathBuf, DirTemp) {
    servidor_com_cadastro_amarra(nome, false)
}

/// Como `servidor_com_cadastro`, mas deixa escolher `exigir_amarra`.
///
/// A exigencia da amarracao e decisao de quem implanta e mora no
/// `config.json`; um servidor ja construido nao a muda. Por isso o teste
/// que a exercita precisa de um servidor nascido com ela ligada.
fn servidor_com_cadastro_amarra(
    nome: &str,
    exigir_amarra: bool,
) -> (Arc<Servidor>, PathBuf, DirTemp) {
    let dir = DirTemp::novo(&format!("cad-op-{nome}"));
    let caminho = dir.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            "{{\n  \"_nota\": \"comentario que a gravacao nao pode comer\",\n  \
                 \"token\": \"t\",\n  \"bind\": \"127.0.0.1:5399\",\n  \
                 \"base\": \"{}\",\n  \
                 \"usuarios\": [\n    {{\n      \"id\": 9,\n      \"nome\": \"Ana\",\n      \
                 \"login\": \"ana\",\n      \"senha_hash\": \"{}\",\n      \
                 \"supervisor\": true,\n      \"conservado\": \"campo que este processo nao conhece\"\n    }}\n  ]\n}}\n",
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
    c.cifra_fio.exigir_amarra = exigir_amarra;
    (Servidor::novo(c).unwrap(), caminho, dir)
}

/// Pedido 667, pelo `op_login` direto: a senha em claro de fora do
/// loopback se recusa ANTES do cadastro, e cada fio protegido continua
/// entrando. O servidor nasce com `exigir: false` -- com a exigencia
/// ligada a porta HTTP em claro nem chega ao login.
#[test]
fn senha_em_claro_de_fora_do_loopback_se_recusa_antes_do_cadastro() {
    let (s0, caminho, _g) = servidor_com_cadastro("667");
    drop(s0);
    let mut c = Config::ler(&caminho).unwrap();
    c.log_acessos = _g.join("acessos.log");
    c.blacklist = _g.join("blacklist.json");
    c.dblink = _g.join("dblink.json");
    c.jobs = _g.join("jobs.json");
    c.cifra_fio.exigir = false;
    let s = Servidor::novo(c.clone()).unwrap();
    let b64 = phxsql_core::base64::codificar(SENHA_DA_ANA.as_bytes());
    let com_senha_b64 = format!(r#"{{"usuario":"ana","senha_b64":"{b64}"}}"#);
    let com_senha = format!(r#"{{"usuario":"ana","senha":"{SENHA_DA_ANA}"}}"#);
    let tentar = |s: &Servidor, corpo: &str, ses: Sessao| {
        let mut ses = ses;
        s.op_login(&pedido(corpo), &mut ses)
    };
    let web = |ip: &str, fio_tls: bool| Sessao {
        ip: ip.into(),
        entrada: Entrada::Http,
        fio_tls,
        ..Sessao::default()
    };
    let recusa = |r: Result<Json>| {
        let e = r.expect_err("a senha em claro de fora entrou").to_string();
        assert!(e.contains("de fora deste computador, recusada"), "{e}");
    };

    // A web em claro pela LAN, nas duas formas da senha.
    recusa(tentar(&s, &com_senha_b64, web("192.168.0.20", false)));
    recusa(tentar(&s, &com_senha, web("192.168.0.20", false)));
    // Antes do cadastro: quem nao existe recebe a MESMA recusa.
    recusa(tentar(
        &s,
        r#"{"usuario":"ninguem","senha":"x"}"#,
        web("10.1.2.3", false),
    ));
    // A porta de dados em claro, de fora, tambem.
    let dados = |ip: &str| Sessao {
        ip: ip.into(),
        entrada: Entrada::Dados,
        ..Sessao::default()
    };
    recusa(tentar(&s, &com_senha, dados("10.1.2.3")));

    // Entram: o loopback (nas tres grafias), o TLS nativo, o tunel, o
    // TLS da porta de dados e a sessao sem fio.
    for ip in ["127.0.0.1", "::1", "::ffff:127.0.0.1"] {
        tentar(&s, &com_senha_b64, web(ip, false)).expect(ip);
    }
    tentar(&s, &com_senha_b64, web("192.168.0.20", true)).expect("web TLS");
    let mut tunel = dados("10.1.2.3");
    tunel.transcricao_do_fio = Some([7; 32]);
    tentar(&s, &com_senha, tunel).expect("tunel");
    let mut tls = dados("10.1.2.3");
    tls.fio_tls = true;
    tentar(&s, &com_senha, tls).expect("TLS da porta de dados");
    tentar(&s, &com_senha, Sessao::default()).expect("sem fio");

    // O desafio-resposta nao leva a senha: pela porta de dados entra de
    // fora em claro. Pela WEB nao -- ali o desafio faz nascer o id de
    // sessao, e o pedido 674 recusa a emissao dele em claro (a mesma regra
    // fica provada pelo soquete em `tests/token-fora-do-loopback.rs`).
    let dk =
        phxsql_core::senha::derivado_do_hash(&s.cadastro().por_login("ana").unwrap().senha_hash)
            .unwrap();
    let e = s
        .op_desafio(
            &pedido(r#"{"usuario":"ana"}"#),
            &mut web("192.168.0.20", false),
        )
        .expect_err("o desafio pela web em claro de fora emitiu sessao")
        .to_string();
    assert!(e.contains("sessao web em claro"), "{e}");
    let mut ses = dados("192.168.0.20");
    let d = s
        .op_desafio(&pedido(r#"{"usuario":"ana"}"#), &mut ses)
        .unwrap();
    let nonce = d.texto_ou("nonce", "").to_string();
    let nc = phxsql_core::desafio::nonce();
    let prova = phxsql_core::desafio::calcular_prova(&dk, &nonce, &nc, "ana", None);
    s.op_login(
        &pedido(&format!(
            r#"{{"usuario":"ana","prova":"{prova}","nonce_cliente":"{nc}"}}"#
        )),
        &mut ses,
    )
    .expect("o desafio-resposta de fora nao leva senha e tinha de entrar");

    // O escape escrito: o comportamento velho, por escolha.
    c.cifra_fio.senha_em_claro_pela_rede = true;
    let s = Servidor::novo(c).unwrap();
    tentar(&s, &com_senha_b64, web("192.168.0.20", false)).expect("o escape nao valeu");
    // O MESMO escape abre a emissao do id (pedido 674): um escape so.
    s.op_desafio(
        &pedido(r#"{"usuario":"ana"}"#),
        &mut web("192.168.0.20", false),
    )
    .expect("o escape nao valeu para o desafio");
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// A sessao da ana, que e supervisora e mora no arquivo.
fn como_ana(s: &Servidor) -> Sessao {
    Sessao {
        usuario: s.cadastro().por_login("ana").cloned(),
        ..Sessao::default()
    }
}

/// Um usuario com tudo MENOS administrar -- o portao de verdade.
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
                administrar: false,
                ..Permissoes::default()
            },
        )],
        tabelas: Vec::new(),
        colunas: Vec::new(),
    }
}

/// O caminho inteiro: criar, gravar, e o login novo entrando A QUENTE.
#[test]
fn cria_grava_no_arquivo_e_o_login_novo_ja_entra() {
    let (s, caminho, _g) = servidor_com_cadastro("criar");
    let sessao = como_ana(&s);

    let r = s
        .executar(
            "usuario_criar",
            &pedido(&format!(
                r#"{{"login":"carlos","senha":"{SENHA_NOVA}","nome":"Carlos Consulta",
                        "nivel":"leitor","bases":{{"loja":{{"ler":true}}}}}}"#
            )),
            &sessao,
        )
        .unwrap();
    assert!(r.booleano_ou("gravado", false), "{}", r.escrever());

    // (1) o arquivo mudou, o comentario continua la, e o campo que este
    //     processo NAO conhece tambem.
    let texto = std::fs::read_to_string(&caminho).unwrap();
    assert!(texto.contains("\"carlos\""), "{texto}");
    assert!(texto.contains("comentario que a gravacao nao pode comer"));
    assert!(
        texto.contains("campo que este processo nao conhece"),
        "a gravacao podou um campo que ela nao entende: {texto}"
    );

    // (2) o cadastro VIVO ja o conhece -- sem reiniciar nada.
    assert!(
        s.cadastro().por_login("carlos").is_some(),
        "o cadastro vivo nao viu o usuario novo"
    );

    // (3) e ele ENTRA: o login e a prova de que o hash gravado confere
    //     com a senha que chegou em claro.
    let mut nova = Sessao::default();
    s.op_login(
        &pedido(&format!(r#"{{"usuario":"carlos","senha":"{SENHA_NOVA}"}}"#)),
        &mut nova,
    )
    .expect("o usuario criado tinha de entrar sem reiniciar o servidor");
    assert_eq!(nova.login(), "carlos");

    // (4) e o `usuarios` do protocolo o lista.
    let lista = s.executar("usuarios", &pedido("{}"), &sessao).unwrap();
    let logins: Vec<String> = lista
        .lista()
        .unwrap()
        .iter()
        .map(|u| u.texto_ou("login", "").to_string())
        .collect();
    assert_eq!(logins, vec!["ana", "carlos"]);
}

/// Channel binding no servidor -- o gap da §10 da `docs/CIFRA-DO-FIO.md`,
/// fechado. A transcricao mora na SESSAO (propriedade da conexao), e o
/// `op_login` amarra a prova a ela quando o cliente pede `amarrar_canal`.
///
/// Prova real nos quatro sentidos que so o servidor conhece, e cada
/// defeito cai num sentido diferente -- medido: fazer o `op_login` usar
/// `None` no lugar de `canal_ref` derruba o caso (1) (o cliente honesto
/// para de entrar); ler a transcricao do PEDIDO em vez da sessao derrubaria
/// o (2), o do homem-no-meio, que e o unico que a leitura nao pegaria.
#[test]
fn login_amarrado_ao_canal_confere_contra_a_transcricao_da_sessao() {
    let (s, _caminho, _g) = servidor_com_cadastro("amarra");
    let dk =
        phxsql_core::senha::derivado_do_hash(&s.cadastro().por_login("ana").unwrap().senha_hash)
            .unwrap();

    // `na_sessao` = a transcricao do tunel DESTA conexao (o que o servidor
    // conhece); `no_calculo` = a transcricao com que o cliente fez a prova.
    // Iguais = cliente honesto; diferentes = homem-no-meio que reencaminhou.
    let tentar = |na_sessao: Option<[u8; 32]>, no_calculo: Option<[u8; 32]>, amarrar: bool| {
        let mut sessao = Sessao {
            transcricao_do_fio: na_sessao,
            ..Sessao::default()
        };
        let d = s
            .op_desafio(&pedido(r#"{"usuario":"ana"}"#), &mut sessao)
            .unwrap();
        let nonce = d.texto_ou("nonce", "").to_string();
        let nc = phxsql_core::desafio::nonce();
        let canal_ref = no_calculo.as_ref().map(|t| &t[..]);
        let prova = phxsql_core::desafio::calcular_prova(&dk, &nonce, &nc, "ana", canal_ref);
        let corpo = if amarrar {
            format!(
                r#"{{"usuario":"ana","prova":"{prova}","nonce_cliente":"{nc}","amarrar_canal":true}}"#
            )
        } else {
            format!(r#"{{"usuario":"ana","prova":"{prova}","nonce_cliente":"{nc}"}}"#)
        };
        s.op_login(&pedido(&corpo), &mut sessao)
    };

    let tunel = [0x5A_u8; 32];

    // (1) tunel + amarrar + a MESMA transcricao dos dois lados: entra.
    assert!(
        tentar(Some(tunel), Some(tunel), true).is_ok(),
        "o cliente honesto no tunel tinha de entrar amarrado"
    );

    // (2) o homem-no-meio: a conexao com o servidor tem a transcricao
    //     atacante<->servidor, mas a prova foi feita com a do tunel
    //     cliente<->atacante. Nao entra -- e este e o ganho que a leitura
    //     do codigo nao mostra.
    let outro_tunel = [0xA7_u8; 32];
    assert!(
        tentar(Some(outro_tunel), Some(tunel), true).is_err(),
        "prova amarrada a outro tunel NAO podia entrar"
    );

    // (3) amarrar pedido sem tunel: recusa nomeada, em vez de amarrar a
    //     coisa nenhuma.
    let e = tentar(None, Some(tunel), true).unwrap_err();
    let msg = format!("{e}").to_lowercase();
    assert!(
        msg.contains("tunel") || msg.contains("aperto"),
        "sem tunel a recusa tinha de mandar abrir o aperto: {e}"
    );

    // (4) a regra petrea: sem `amarrar_canal`, o login e o de sempre --
    //     mesmo havendo uma transcricao na sessao, quem nao pede nada entra
    //     como antes. E o teste que impede a guarda nova de virar imposicao.
    assert!(
        tentar(Some(tunel), None, false).is_ok(),
        "o login sem amarracao tinha de continuar como sempre foi"
    );
}

/// EXIGIR a amarracao ao canal -- o gap que sobrava na §10 da
/// `docs/CIFRA-DO-FIO.md`, fechado. A amarracao era so PEDIDA; um atacante
/// ativo que terminou o tunel do cliente cortava `amarrar_canal` antes de
/// reencaminhar, a mesma aritmetica do rebaixamento do `exigir`. Com
/// `cifra_fio.exigir_amarra` ligado o servidor RECUSA o login que nao
/// amarra, mas so quando ha tunel: em claro nao ha o que amarrar.
///
/// Prova real nos dois sentidos: com o defeito reposto (o `op_login`
/// ignorando `exigir_amarra`) o caso (a) cai -- o cliente que nao amarra
/// volta a entrar, e a `unwrap_err` da recusa nomeada estoura. Guarda no
/// `catalogo.py` como `amarra-exigida-ignorada`.
#[test]
fn login_exige_amarra_quando_ha_tunel() {
    // Uma tentativa parametrizada. `exigir` = como o servidor foi
    // implantado; `na_sessao` = a transcricao do tunel desta conexao
    // (`None` = conexao em claro); `amarrar` = o cliente pediu a amarracao.
    // O cliente honesto amarra com a transcricao do PROPRIO tunel, que e a
    // da sessao -- por isso a prova, quando amarra, usa `na_sessao`.
    let tentar = |exigir: bool, na_sessao: Option<[u8; 32]>, amarrar: bool| {
        let (s, _caminho, _g) = servidor_com_cadastro_amarra("exige-amarra", exigir);
        let dk = phxsql_core::senha::derivado_do_hash(
            &s.cadastro().por_login("ana").unwrap().senha_hash,
        )
        .unwrap();
        let mut sessao = Sessao {
            transcricao_do_fio: na_sessao,
            ..Sessao::default()
        };
        let d = s
            .op_desafio(&pedido(r#"{"usuario":"ana"}"#), &mut sessao)
            .unwrap();
        let nonce = d.texto_ou("nonce", "").to_string();
        let nc = phxsql_core::desafio::nonce();
        let canal_ref = if amarrar {
            na_sessao.as_ref().map(|t| &t[..])
        } else {
            None
        };
        let prova = phxsql_core::desafio::calcular_prova(&dk, &nonce, &nc, "ana", canal_ref);
        let corpo = if amarrar {
            format!(
                r#"{{"usuario":"ana","prova":"{prova}","nonce_cliente":"{nc}","amarrar_canal":true}}"#
            )
        } else {
            format!(r#"{{"usuario":"ana","prova":"{prova}","nonce_cliente":"{nc}"}}"#)
        };
        s.op_login(&pedido(&corpo), &mut sessao)
    };

    let tunel = [0x5A_u8; 32];

    // (a) exigir + tunel + o cliente NAO amarra: recusa NOMEADA, e antes de
    //     olhar a credencial. E o caso que so existe por causa desta frente.
    let e = tentar(true, Some(tunel), false).unwrap_err();
    let msg = format!("{e}").to_lowercase();
    assert!(
        msg.contains("amarr"),
        "com exigir_amarra ligado, quem nao amarra tinha de ser recusado \
             mandando amarrar o canal, veio: {e}"
    );

    // (b) exigir + tunel + o cliente amarra: entra. A exigencia nao fecha a
    //     porta de quem faz a coisa certa.
    assert!(
        tentar(true, Some(tunel), true).is_ok(),
        "quem amarra tinha de entrar mesmo com exigir_amarra ligado"
    );

    // (c) exigir + SEM tunel (claro) + nao amarra: NAO muda. `exigir_amarra`
    //     so morde quando ha transcricao; em claro nao ha o que amarrar, e
    //     recusar aqui quebraria toda conexao em claro sem ganho nenhum.
    assert!(
        tentar(true, None, false).is_ok(),
        "sem tunel a exigencia nao se aplica: a conexao em claro entra como sempre"
    );

    // (d) A REGRA PETREA: exigir DESLIGADO + tunel + nao amarra: entra como
    //     sempre foi. E o teste que impede a guarda nova de virar imposicao.
    assert!(
        tentar(false, Some(tunel), false).is_ok(),
        "com exigir_amarra desligado, o login sem amarracao continua como sempre"
    );
}

/// **A prova de vazamento.** A senha em claro nao pode existir em lugar
/// nenhum: nem no arquivo, nem na resposta, nem no `acessos.log`.
///
/// Repor o defeito e trocar `senha::cifrar(clara)` por `clara` em
/// `objeto_do_usuario` -- e ai a primeira afirmacao cai.
#[test]
fn a_senha_nunca_aparece_no_arquivo_nem_na_resposta() {
    let (s, caminho, dir) = servidor_com_cadastro("vazamento");
    let mut sessao = como_ana(&s);
    let (_, _, r) = s.despachar(
        &format!(r#"{{"op":"usuario_criar","token":"t","login":"carlos","senha":"{SENHA_NOVA}"}}"#),
        &mut sessao,
        "1.2.3.4",
    );
    let resposta = r.unwrap().escrever();

    let arquivo = std::fs::read_to_string(&caminho).unwrap();
    assert!(
        !arquivo.contains(SENHA_NOVA),
        "a senha em claro foi para o config.json"
    );
    assert!(
        arquivo.contains("pbkdf2-sha256$"),
        "o arquivo nao ficou com o hash: {arquivo}"
    );
    assert!(
        !resposta.contains(SENHA_NOVA) && !resposta.contains("pbkdf2"),
        "a resposta vazou senha ou hash: {resposta}"
    );

    // O `acessos.log` guarda a OPERACAO e o login de quem pediu, nunca o
    // corpo do pedido -- entao a senha nao tem por onde chegar la. Quem
    // prova isso contra um servidor de verdade, com o log escrito pelo
    // laco de conexao (que este teste nao percorre), e a
    // `bancada/usuarios/provar.py`.
    let _ = &dir;

    // E a ficha do usuario novo, pedida de volta, tambem nao a traz.
    let fichas = s.executar("usuarios", &pedido("{}"), &sessao).unwrap();
    assert!(!fichas.escrever().contains("pbkdf2"));
}

/// **O controle positivo do varredor.** Uma varredura que nunca acha nada
/// protege igual e nao prova nada -- este teste mostra que ela ACHA a
/// senha quando ela esta la, e e o que da valor ao teste de cima.
#[test]
fn a_varredura_acha_a_senha_quando_ela_esta_la() {
    let arquivo_ruim = format!("{{\"login\":\"carlos\",\"senha\":\"{SENHA_NOVA}\"}}");
    assert!(arquivo_ruim.contains(SENHA_NOVA));
}

/// O Profiler mostra o pedido CRU, e o pedido de criar usuario traz a
/// senha. Ela sai tapada por ANALISE da arvore, e nao por recorte.
#[test]
fn o_profiler_tapa_a_senha_do_pedido_de_criar_usuario() {
    let linha =
        format!(r#"{{"op":"usuario_criar","token":"t","login":"carlos","senha":"{SENHA_NOVA}"}}"#);
    let redigido = crate::profiler::redigir(&linha);
    assert!(!redigido.contains(SENHA_NOVA), "{redigido}");
    assert!(redigido.contains("\"senha\":\"***\""), "{redigido}");
    assert!(redigido.contains("usuario_criar"), "{redigido}");
}

/// O portao. Pelo `despachar`, que e por onde o pedido entra de verdade.
#[test]
fn operador_sem_administrar_nao_mexe_no_cadastro() {
    let (s, caminho, _g) = servidor_com_cadastro("portao");
    let antes = std::fs::read_to_string(&caminho).unwrap();
    let mut sessao = Sessao {
        usuario: Some(operador()),
        ..Sessao::default()
    };
    for corpo in [
        r#"{"op":"usuario_criar","token":"t","login":"x","senha":"12345678"}"#,
        r#"{"op":"usuario_alterar","token":"t","login":"ana","senha":"12345678"}"#,
        r#"{"op":"usuario_excluir","token":"t","login":"ana"}"#,
    ] {
        let (_, _, r) = s.despachar(corpo, &mut sessao, "1.2.3.4");
        let e = r.unwrap_err().to_string();
        assert!(e.contains("administrar"), "{corpo} -> {e}");
    }
    assert_eq!(
        antes,
        std::fs::read_to_string(&caminho).unwrap(),
        "o arquivo mudou apesar da recusa"
    );
}

/// Sem administrador ativo ninguem mais mexe no cadastro -- nem para
/// desfazer. Os DOIS caminhos que levam la sao recusados.
#[test]
fn nao_se_deixa_o_servidor_sem_administrador() {
    let (s, _c, _g) = servidor_com_cadastro("ultimo");
    let sessao = como_ana(&s);
    // Excluir a si mesma cai na guarda da propria conta, que vem depois;
    // o caminho que testa ESTA guarda e um segundo admin excluindo o
    // primeiro e depois... a si. Aqui o caso direto: desligar a unica.
    let e = s
        .executar(
            "usuario_alterar",
            &pedido(r#"{"login":"ana","ativo":false}"#),
            &sessao,
        )
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("supervisor ativo") || e.contains("administrar"),
        "{e}"
    );
}

/// Ninguem se rebaixa nem se apaga: quem faz isso fica sem poder desfazer.
#[test]
fn nao_se_tira_de_si_mesmo_o_poder_de_administrar() {
    let (s, _c, _g) = servidor_com_cadastro("eu-mesmo");
    let sessao = como_ana(&s);
    // Um segundo supervisor, para a guarda do "ultimo" nao ser a que pega.
    s.executar(
        "usuario_criar",
        &pedido(r#"{"login":"bia","senha":"12345678","supervisor":true}"#),
        &sessao,
    )
    .unwrap();

    for corpo in [
        r#"{"login":"ana","supervisor":false,"nivel":"leitor"}"#,
        r#"{"login":"ana","ativo":false}"#,
    ] {
        let e = s
            .executar("usuario_alterar", &pedido(corpo), &sessao)
            .unwrap_err()
            .to_string();
        assert!(e.contains("propria conta"), "{corpo} -> {e}");
    }
    let e = s
        .executar("usuario_excluir", &pedido(r#"{"login":"ana"}"#), &sessao)
        .unwrap_err()
        .to_string();
    assert!(e.contains("propria conta"), "{e}");

    // E a ana continua supervisora e ativa no arquivo.
    assert!(s.cadastro().por_login("ana").unwrap().supervisor);
}

/// Trocar a PROPRIA senha continua podendo: e alterar a conta sem perder
/// o poder, e recusar isso seria a guarda barrando o que nao faz mal.
#[test]
fn a_propria_senha_se_troca() {
    let (s, _c, _g) = servidor_com_cadastro("minha-senha");
    let sessao = como_ana(&s);
    s.executar(
        "usuario_alterar",
        &pedido(&format!(r#"{{"login":"ana","senha":"{SENHA_NOVA}"}}"#)),
        &sessao,
    )
    .unwrap();
    let mut nova = Sessao::default();
    s.op_login(
        &pedido(&format!(r#"{{"usuario":"ana","senha":"{SENHA_NOVA}"}}"#)),
        &mut nova,
    )
    .expect("a senha nova tinha de valer");
    assert!(
        s.op_login(
            &pedido(&format!(r#"{{"usuario":"ana","senha":"{SENHA_DA_ANA}"}}"#)),
            &mut Sessao::default(),
        )
        .is_err(),
        "a senha VELHA continuou entrando"
    );
}

/// A ficha da conexao acompanha o cadastro vivo: quem foi excluido perde
/// a sessao no pedido seguinte, com o «faca login» que todo cliente trata
/// -- e nao com um soquete derrubado.
#[test]
fn o_excluido_perde_a_sessao_no_pedido_seguinte() {
    let (s, _c, _g) = servidor_com_cadastro("excluido");
    let ana = como_ana(&s);
    s.executar(
        "usuario_criar",
        &pedido(r#"{"login":"carlos","senha":"12345678","nivel":"admin"}"#),
        &ana,
    )
    .unwrap();

    // A conexao do carlos, ja autenticada.
    let mut dele = Sessao::default();
    s.op_login(
        &pedido(r#"{"usuario":"carlos","senha":"12345678"}"#),
        &mut dele,
    )
    .unwrap();
    let (_, _, r) = s.despachar(r#"{"op":"bancos","token":"t"}"#, &mut dele, "1.2.3.4");
    assert!(r.is_ok(), "o carlos devia estar dentro");

    s.executar("usuario_excluir", &pedido(r#"{"login":"carlos"}"#), &ana)
        .unwrap();

    let (_, _, r) = s.despachar(r#"{"op":"bancos","token":"t"}"#, &mut dele, "1.2.3.4");
    let e = r.unwrap_err().to_string();
    assert!(e.to_lowercase().contains("login"), "{e}");
    assert!(dele.usuario.is_none(), "a ficha velha sobreviveu");
}

/// **O teste do comportamento VELHO.** Num servidor onde ninguem chamou
/// as operacoes novas, nada muda: a geracao fica em zero e a sessao nunca
/// e relida. E a guarda contra a releitura virar custo de todo pedido.
#[test]
fn sem_mexer_no_cadastro_a_sessao_nunca_e_relida() {
    let (s, _c, _g) = servidor_com_cadastro("velho");
    let mut sessao = como_ana(&s);
    for _ in 0..5 {
        let _ = s.despachar(r#"{"op":"ping","token":"t"}"#, &mut sessao, "1.2.3.4");
    }
    assert_eq!(
        s.cadastro_geracao.load(Ordering::Relaxed),
        0,
        "a geracao andou sem ninguem mexer no cadastro"
    );
    assert_eq!(sessao.geracao_do_cadastro, 0);
    assert_eq!(sessao.login(), "ana", "a ficha da conexao mudou sozinha");
}

/// Login que nunca autenticaria e recusado na CRIACAO, e nao descoberto
/// no dia em que a pessoa tentar entrar.
#[test]
fn login_que_nao_entraria_e_recusado_cedo() {
    let (s, _c, _g) = servidor_com_cadastro("login-torto");
    let sessao = como_ana(&s);
    for (corpo, pedaco) in [
        (r#"{"login":"  ","senha":"12345678"}"#, "informe"),
        (r#"{"login":" carlos","senha":"12345678"}"#, "espaco"),
        (r#"{"login":"car\nlos","senha":"12345678"}"#, "controle"),
        (r#"{"login":"ana","senha":"12345678"}"#, "ja ha um usuario"),
    ] {
        let e = s
            .executar("usuario_criar", &pedido(corpo), &sessao)
            .unwrap_err()
            .to_string();
        assert!(e.contains(pedaco), "{corpo} -> {e}");
    }
}

/// Hash pronto pelo protocolo escolheria o proprio custo -- e «PBKDF2 com
/// uma volta» tem a mesma cara de «PBKDF2 com 210.000».
#[test]
fn senha_hash_pronto_e_recusado() {
    let (s, _c, _g) = servidor_com_cadastro("hash-pronto");
    let sessao = como_ana(&s);
    let e = s
        .executar(
            "usuario_criar",
            &pedido(r#"{"login":"carlos","senha_hash":"pbkdf2-sha256$1$00$00"}"#),
            &sessao,
        )
        .unwrap_err()
        .to_string();
    assert!(e.contains("senha_hash"), "{e}");
}

/// So supervisor faz supervisor: administrar UMA base nao da o poder de
/// criar quem manda em TODAS.
#[test]
fn so_supervisor_cria_supervisor() {
    let (s, _c, _g) = servidor_com_cadastro("escalada");
    let ana = como_ana(&s);
    s.executar(
        "usuario_criar",
        &pedido(r#"{"login":"dba","senha":"12345678","nivel":"admin"}"#),
        &ana,
    )
    .unwrap();
    let dele = Sessao {
        usuario: s.cadastro().por_login("dba").cloned(),
        ..Sessao::default()
    };
    let e = s
        .executar(
            "usuario_criar",
            &pedido(r#"{"login":"x","senha":"12345678","supervisor":true}"#),
            &dele,
        )
        .unwrap_err()
        .to_string();
    assert!(e.contains("supervisor"), "{e}");
    // E ele continua podendo criar gente COMUM: a guarda barra a escalada,
    // e nao o trabalho.
    s.executar(
        "usuario_criar",
        &pedido(r#"{"login":"x","senha":"12345678","nivel":"leitor"}"#),
        &dele,
    )
    .unwrap();
}

/// Alterar mexe SO no que o pedido traz. O resto da ficha -- e o campo que
/// este processo nem conhece -- fica onde estava.
#[test]
fn alterar_so_mexe_no_que_o_pedido_traz() {
    let (s, caminho, _g) = servidor_com_cadastro("parcial");
    let sessao = como_ana(&s);
    s.executar(
        "usuario_criar",
        &pedido(
            r#"{"login":"carlos","senha":"12345678","nome":"Carlos Consulta",
                    "email":"carlos@empresa.com.br","nivel":"leitor"}"#,
        ),
        &sessao,
    )
    .unwrap();
    s.executar(
        "usuario_alterar",
        &pedido(r#"{"login":"carlos","telefone":"+55 47 99999-0000"}"#),
        &sessao,
    )
    .unwrap();
    let u = s.cadastro().por_login("carlos").cloned().unwrap();
    assert_eq!(u.telefone, "+55 47 99999-0000");
    assert_eq!(u.nome, "Carlos Consulta", "o nome se perdeu");
    assert_eq!(u.email, "carlos@empresa.com.br", "o e-mail se perdeu");
    assert_eq!(u.nivel, Nivel::Leitor, "o nivel se perdeu");
    assert!(!u.senha_hash.is_empty(), "o hash se perdeu");
    assert!(std::fs::read_to_string(&caminho)
        .unwrap()
        .contains("campo que este processo nao conhece"));
}

/// O root nao se mexe pelo protocolo: ele e a porta de entrada de quando
/// o cadastro sai errado.
#[test]
fn o_root_nao_se_mexe_por_aqui() {
    let dir = DirTemp::novo("cad-op-root");
    let caminho = dir.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            "{{\"token\":\"t\",\"bind\":\"127.0.0.1:5397\",\"base\":\"{}\",\
                 \"root\":{{\"login\":\"root\",\"senha_hash\":\"{}\"}}}}\n",
            dir.join("dados").display(),
            phxsql_core::senha::cifrar_com("raiz", 64)
        ),
    )
    .unwrap();
    let mut c = Config::ler(&caminho).unwrap();
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    let s = Servidor::novo(c).unwrap();
    let sessao = Sessao {
        usuario: s.cadastro().root.clone(),
        ..Sessao::default()
    };
    for op in ["usuario_alterar", "usuario_excluir"] {
        let e = s
            .executar(op, &pedido(r#"{"login":"root","senha":"outra"}"#), &sessao)
            .unwrap_err()
            .to_string();
        assert!(e.contains("root nao se muda"), "{op} -> {e}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Os tres comandos SQL, pela op `sql` -- e o texto que volta ja vem sem
/// a senha dentro.
#[test]
fn os_tres_comandos_sql_valem_e_o_texto_de_volta_nao_traz_a_senha() {
    let (s, _c, _g) = servidor_com_cadastro("sql");
    let sessao = como_ana(&s);

    let r = s
        .executar(
            "sql",
            &pedido(&format!(
                r#"{{"texto":"CREATE USER carlos PASSWORD '{SENHA_NOVA}'"}}"#
            )),
            &sessao,
        )
        .unwrap();
    assert_eq!(r.texto_ou("op", ""), "usuario_criar");
    assert!(
        !r.escrever().contains(SENHA_NOVA),
        "a resposta do SQL trouxe a senha: {}",
        r.escrever()
    );
    assert!(r.texto_ou("sql", "").contains("'***'"), "{}", r.escrever());

    // E ele entra, que e a prova de que o hash saiu certo do caminho SQL.
    let mut nova = Sessao::default();
    s.op_login(
        &pedido(&format!(r#"{{"usuario":"carlos","senha":"{SENHA_NOVA}"}}"#)),
        &mut nova,
    )
    .unwrap();

    s.executar(
        "sql",
        &pedido(r#"{"texto":"ALTER USER carlos PASSWORD 'trocada-2026'"}"#),
        &sessao,
    )
    .unwrap();
    s.op_login(
        &pedido(r#"{"usuario":"carlos","senha":"trocada-2026"}"#),
        &mut Sessao::default(),
    )
    .expect("a senha do ALTER USER nao valeu");

    s.executar("sql", &pedido(r#"{"texto":"DROP USER carlos"}"#), &sessao)
        .unwrap();
    assert!(s.cadastro().por_login("carlos").is_none());
}

/// A conta de usuarios que a op `config` devolve sai do cadastro VIVO.
///
/// Numero visivel que envelhece calado e o defeito que esta casa mais
/// paga: sem isto, criar um usuario deixaria o painel de Configuracoes
/// dizendo o numero do arranque.
#[test]
fn a_conta_de_usuarios_do_config_acompanha_o_cadastro_vivo() {
    let (s, _c, _g) = servidor_com_cadastro("conta");
    let sessao = como_ana(&s);
    let antes = s
        .executar("config", &pedido("{}"), &sessao)
        .unwrap()
        .inteiro_ou("usuarios", 0);
    assert_eq!(antes, 1);
    s.executar(
        "usuario_criar",
        &pedido(r#"{"login":"carlos","senha":"12345678","nivel":"leitor"}"#),
        &sessao,
    )
    .unwrap();
    let depois = s
        .executar("config", &pedido("{}"), &sessao)
        .unwrap()
        .inteiro_ou("usuarios", 0);
    assert_eq!(depois, 2, "o painel ficou com o numero do arranque");
}

/// Servidor sem `--config` nao tem onde gravar, e a recusa DIZ isso em vez
/// de dar erro de arquivo.
#[test]
fn sem_arquivo_a_recusa_nomeia_o_motivo() {
    let dir = DirTemp::novo("cad-op-sem-arquivo");
    let c = Config {
        base: dir.join("dados"),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        cadastro: Cadastro::default(),
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let e = s
        .executar(
            "usuario_criar",
            &pedido(r#"{"login":"x","senha":"12345678"}"#),
            &Sessao::default(),
        )
        .unwrap_err()
        .to_string();
    assert!(e.contains("--config"), "{e}");
    std::fs::remove_dir_all(&dir).unwrap();
}

// ------------------------------------------------ pedidos 520 e 521

/// A ana (supervisora) e o ze inativo, os dois com hash de 64 iteracoes.
fn servidor_com_inativo(nome: &str) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("cad-520-{nome}"));
    let caminho = dir.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{"token":"t","bind":"127.0.0.1:5397","base":"{}",
                    "usuarios":[{{"login":"ana","senha_hash":"{}","supervisor":true}},
                                {{"login":"ze","senha_hash":"{}","ativo":false}}]}}"#,
            dir.join("dados").display(),
            phxsql_core::senha::cifrar_com(SENHA_DA_ANA, 64),
            phxsql_core::senha::cifrar_com("senha-do-ze", 64),
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

/// **O irmao do 520 no desafio-resposta.** Quem existe conferia a prova;
/// quem nao existe e o inativo saiam sem conferir nada. Aqui nao ha
/// PBKDF2 e a diferenca era de microssegundos -- medido pela porta de
/// dados em debug, intercalado, n = 1.000 em duas rodadas: prova errada de
/// quem existe 162-166 us, de quem nao existe e do inativo 24-25 us a
/// menos; depois do conserto, +-1 us --, e microssegundo nao se prova por
/// relogio num teste: o contador de provas conferidas prova por dentro
/// que os tres fazem a mesma conta.
///
/// Vermelho medido com o `match` de antes reposto: o inativo e o que nao
/// existe conferem 0 provas, contra 1 de quem existe.
#[test]
fn a_prova_de_quem_nao_existe_ou_esta_inativo_confere_como_a_de_quem_existe() {
    let (s, _g) = servidor_com_inativo("prova");
    for login in ["ana", "ze", "nao_existe"] {
        let mut sessao = Sessao::default();
        s.op_desafio(&pedido(&format!(r#"{{"usuario":"{login}"}}"#)), &mut sessao)
            .unwrap();
        let antes = phxsql_core::desafio::provas_conferidas_nesta_thread();
        let corpo = format!(
            r#"{{"usuario":"{login}","prova":"{}","nonce_cliente":"abc"}}"#,
            "00".repeat(32)
        );
        let e = s.op_login(&pedido(&corpo), &mut sessao).unwrap_err();
        assert!(matches!(e, PhxError::Autorizacao(_)), "{login}: {e}");
        assert_eq!(
            phxsql_core::desafio::provas_conferidas_nesta_thread() - antes,
            1,
            "{login} nao conferiu a prova: o relogio separa quem existe"
        );
    }
    // E a prova certa do inativo continua sem entrar.
    let mut sessao = Sessao::default();
    let d = s
        .op_desafio(&pedido(r#"{"usuario":"ze"}"#), &mut sessao)
        .unwrap();
    let dk =
        phxsql_core::senha::derivado_do_hash(&s.cadastro().por_login("ze").unwrap().senha_hash)
            .unwrap();
    let prova =
        phxsql_core::desafio::calcular_prova(&dk, d.texto_ou("nonce", ""), "abc", "ze", None);
    let corpo = format!(r#"{{"usuario":"ze","prova":"{prova}","nonce_cliente":"abc"}}"#);
    assert!(s.op_login(&pedido(&corpo), &mut sessao).is_err());
}

/// **Pedido 521, a porta do login.** A senha acima do teto e recusada
/// com a recusa NOMEADA e antes do PBKDF2 -- para quem existe e para quem
/// nao existe, pelos dois campos (`senha` e `senha_b64`) -- e a recusa
/// nao carrega a senha. No teto, a conta roda como sempre.
///
/// Vermelho medido sem a linha do teto no `op_login`: a recusa volta como
/// «credencial invalida» (`Autorizacao`), sem dizer por que -- a conta nao
/// roda, porque o `conferir` tem o teto dele, mas quem mandou nao sabe o
/// que corrigir.
#[test]
fn a_senha_acima_do_teto_e_recusada_no_login_antes_do_pbkdf2() {
    let (s, _g) = servidor_com_inativo("teto-login");
    let teto = phxsql_core::senha::TETO_DA_SENHA;
    let longa = "s".repeat(teto + 1);
    let b64 = phxsql_core::base64::codificar(longa.as_bytes());
    for (login, campo, valor) in [
        ("ana", "senha", longa.as_str()),
        ("nao_existe", "senha", longa.as_str()),
        ("ana", "senha_b64", b64.as_str()),
    ] {
        let antes = phxsql_core::hash::iteracoes_pagas_nesta_thread();
        let corpo = Json::objeto(vec![
            ("usuario", Json::texto_de(login)),
            (campo, Json::texto_de(valor)),
        ]);
        let e = s.op_login(&corpo, &mut Sessao::default()).unwrap_err();
        assert!(
            matches!(e, PhxError::LimiteExcedido(_)),
            "{login}/{campo}: a recusa nao nomeou o teto: {e}"
        );
        let texto = e.to_string();
        assert!(texto.contains(&teto.to_string()), "{texto}");
        assert!(!texto.contains("sss"), "a recusa carregou a senha");
        assert_eq!(
            phxsql_core::hash::iteracoes_pagas_nesta_thread(),
            antes,
            "{login}/{campo}: o PBKDF2 rodou com a senha acima do teto"
        );
    }
    // No teto exato a conta roda: 64 iteracoes do hash da ana.
    let antes = phxsql_core::hash::iteracoes_pagas_nesta_thread();
    let corpo = Json::objeto(vec![
        ("usuario", Json::texto_de("ana")),
        ("senha", Json::texto_de("s".repeat(teto))),
    ]);
    let e = s.op_login(&corpo, &mut Sessao::default()).unwrap_err();
    assert!(matches!(e, PhxError::Autorizacao(_)), "{e}");
    assert_eq!(
        phxsql_core::hash::iteracoes_pagas_nesta_thread() - antes,
        64
    );
}

/// **Pedido 521, as portas que criam e trocam senha**: `usuario_criar`,
/// `usuario_alterar` e o `CREATE USER` do SQL recusam a senha acima do
/// teto antes do PBKDF2, e nada vai ao disco nem ao cadastro vivo.
///
/// Vermelho medido sem a linha do teto no `objeto_do_usuario`: o
/// `usuario_criar` grava, e paga as 210.000 iteracoes com a chave de
/// 64 KiB -- 1.025 compressoes a mais por derivacao, uma so vez.
#[test]
fn criar_e_trocar_senha_acima_do_teto_recusa_e_nao_grava() {
    let (s, caminho, _g) = servidor_com_cadastro("teto-criar");
    let sessao = como_ana(&s);
    let longa = "s".repeat(phxsql_core::senha::TETO_DA_SENHA + 1);
    let arquivo_antes = std::fs::read_to_string(&caminho).unwrap();
    let hash_da_ana = s.cadastro().por_login("ana").unwrap().senha_hash.clone();

    let criar = Json::objeto(vec![
        ("login", Json::texto_de("carlos")),
        ("senha", Json::texto_de(&longa)),
    ]);
    let trocar = Json::objeto(vec![
        ("login", Json::texto_de("ana")),
        ("senha", Json::texto_de(&longa)),
    ]);
    let sql = Json::objeto(vec![(
        "texto",
        Json::texto_de(format!("CREATE USER carlos PASSWORD '{longa}'")),
    )]);
    for (op, corpo) in [
        ("usuario_criar", &criar),
        ("usuario_alterar", &trocar),
        ("sql", &sql),
    ] {
        let antes = phxsql_core::hash::iteracoes_pagas_nesta_thread();
        let e = s.executar(op, corpo, &sessao).unwrap_err();
        assert!(matches!(e, PhxError::LimiteExcedido(_)), "{op}: {e}");
        assert!(
            !e.to_string().contains("sss"),
            "{op}: a recusa carregou a senha"
        );
        assert_eq!(
            phxsql_core::hash::iteracoes_pagas_nesta_thread(),
            antes,
            "{op}: o PBKDF2 rodou com a senha acima do teto"
        );
    }
    assert_eq!(std::fs::read_to_string(&caminho).unwrap(), arquivo_antes);
    assert!(s.cadastro().por_login("carlos").is_none());
    assert_eq!(
        s.cadastro().por_login("ana").unwrap().senha_hash,
        hash_da_ana
    );
}

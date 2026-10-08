//! A gravacao da configuracao pela tela: o portao, o segredo e o efeito.
//!
//! O que estes testes travam nao e so o "grava" -- e principalmente o
//! **contrario**: quem nao tem `administrar` nao grava, o segredo nao volta na
//! resposta, e o que a lista nao permite nao entra no arquivo.
use super::*;
use crate::usuarios::{Cadastro, Nivel, Permissoes, Usuario};

/// Um servidor que subiu de um `config.json` de verdade -- e nao de um
/// `Config` montado a mao: sem o caminho no arquivo nao ha o que gravar.
fn servidor_de_arquivo(nome: &str, cadastro: Cadastro) -> (Arc<Servidor>, PathBuf, DirTemp) {
    let dir = DirTemp::novo(&format!("cfg-op-{nome}"));
    let caminho = dir.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            "{{\n  \"_nota\": \"comentario que a gravacao nao pode comer\",\n  \
                 \"token\": \"t\",\n  \"bind\": \"127.0.0.1:5398\",\n  \
                 \"base\": \"{}\",\n  \"max_linhas\": 1000,\n  \"espelho\": false\n}}\n",
            dir.join("dados").display()
        ),
    )
    .unwrap();
    let mut c = Config::ler(&caminho).unwrap();
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    c.cadastro = cadastro;
    (Servidor::novo(c).unwrap(), caminho, dir)
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
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
                alterar: true,
                excluir: true,
                administrar: false,
                ..Permissoes::default()
            },
        )],
        tabelas: Vec::new(),
        colunas: Vec::new(),
    }
}

/// Um servidor de arquivo COM bloco `cluster` -- o terreno dos testes do
/// escalonamento a quente (pedido 217).
fn servidor_em_cluster(nome: &str, cadastro: Cadastro) -> (Arc<Servidor>, PathBuf, DirTemp) {
    servidor_em_cluster_com(nome, cadastro, |_| {})
}

/// [`servidor_em_cluster`] com um ajuste na `Config` antes de subir -- para
/// ligar um interruptor de politica ou dar usuario ao cluster.
fn servidor_em_cluster_com(
    nome: &str,
    cadastro: Cadastro,
    ajuste: impl FnOnce(&mut Config),
) -> (Arc<Servidor>, PathBuf, DirTemp) {
    let dir = DirTemp::novo(&format!("cluster-op-{nome}"));
    let caminho = dir.join("config.json");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
  "token": "t",
  "bind": "127.0.0.1:5399",
  "base": "{}",
  "replicacao": {{"papel": "source", "id_servidor": "no1", "imagem_da_linha": true}},
  "cluster": {{
    "id": "no1",
    "janela_inatividade_s": 30,
    "nos": [
      {{"id": "no1", "endereco": "127.0.0.1", "porta": 5399}},
      {{"id": "no2", "endereco": "127.0.0.1", "porta": 5398}}
    ]
  }}
}}
"#,
            dir.join("dados").display()
        ),
    )
    .unwrap();
    let mut c = Config::ler(&caminho).unwrap();
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    c.cadastro = cadastro;
    ajuste(&mut c);
    (Servidor::novo(c).unwrap(), caminho, dir)
}

/// Um usuario que SO le, em toda base: o `ler` do A6.
fn leitor() -> Usuario {
    Usuario {
        id: 8,
        nome: "Leitora".into(),
        login: "le".into(),
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
                ..Permissoes::default()
            },
        )],
        tabelas: Vec::new(),
        colunas: Vec::new(),
    }
}

/// Um administrador de verdade, com IP de fora do cluster.
fn administrador() -> Usuario {
    let mut u = operador();
    u.id = 9;
    u.login = "adm".into();
    u.bases = vec![("*".into(), Permissoes::tudo())];
    u
}

fn sessao_com(usuario: Option<Usuario>, ip: &str) -> Sessao {
    Sessao {
        usuario,
        ip: ip.to_string(),
        ..Sessao::default()
    }
}

fn pulso_de(id: &str) -> Json {
    pedido(&format!(
        r#"{{"op":"cluster_pulso","id":"{id}","papel":"replica","epoca":0,"posicao":0}}"#
    ))
}

/// O no de `cluster_estado` com este `id`.
fn no_do_estado(estado: &Json, id: &str) -> Json {
    estado
        .campo("nos")
        .and_then(Json::lista)
        .and_then(|l| l.iter().find(|n| n.texto_ou("id", "") == id).cloned())
        .unwrap_or_else(|| panic!("{id} fora do cluster_estado: {}", estado.escrever()))
}

/// **Pedido 294: a posicao POR TABELA entra no pulso e no painel como
/// MEDIDA, ao lado da soma -- e o painel diz qual no esta atras em qual
/// tabela.**
///
/// O caso do parecer de C, por extenso: este no tem 5 eventos em `b/uma`
/// e 5 em `b/outra` (soma 10); o no2 publica 10 em `b/uma` e 0 em
/// `b/outra` (soma 10). As somas empatam -- e na eleicao continuam
/// empatando, porque a soma segue sendo o criterio --, mas o painel tem de
/// mostrar que o no2 esta cego em `b/outra` e este no atras em `b/uma`.
///
/// # O vermelho
///
/// Sem o vetor no pulso (`PulsoDeNo::de_json` ignorando `por_tabela`), o
/// `cluster_estado` nao tem o que mostrar do no2 e o teste cai no
/// `por_tabela` dele; sem o calculo do `atras_em`, cai na assimetria.
#[test]
fn a_posicao_por_tabela_vai_ao_painel_e_nao_ao_voto() {
    let (s, _caminho, _guarda) = servidor_em_cluster("por-tabela", Cadastro::default());
    let sessao = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &sessao)
        .unwrap();
    for t in ["uma", "outra"] {
        s.executar(
            "criar_tabela",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"{t}",
                        "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}}],
                        "indices":[{{"nome":"porId","colunas":["id"],"unico":true,"primario":true}}]}}"#
            )),
            &sessao,
        )
        .unwrap();
        for i in 1..=5 {
            s.executar(
                "inserir",
                &pedido(&format!(
                    r#"{{"database":"b","tabela":"{t}","linha":{{"id":{i}}}}}"#
                )),
                &sessao,
            )
            .unwrap();
        }
    }
    let estado = s.cluster.clone().expect("cluster");
    s.contar_posicao_do_cluster(&estado);
    assert_eq!(estado.posicao(), 10, "a soma continua sendo a soma");

    let pulso = pedido(
        r#"{"op":"cluster_pulso","id":"no2","papel":"replica","epoca":0,"posicao":10,
                "por_tabela":{"b/uma":10,"b/outra":0}}"#,
    );
    let resposta = s.executar("cluster_pulso", &pulso, &sessao).unwrap();
    // O outro sentido: a RESPOSTA do pulso leva o vetor deste no, senao
    // so um dos lados do par enxergaria a assimetria.
    assert_eq!(
        resposta
            .campo("por_tabela")
            .and_then(|v| v.campo("b/outra"))
            .and_then(Json::inteiro),
        Some(5),
        "a resposta do pulso sem o vetor deste no: {}",
        resposta.escrever()
    );

    let e = s
        .executar("cluster_estado", &pedido("{}"), &sessao)
        .unwrap();
    let no1 = no_do_estado(&e, "no1");
    let no2 = no_do_estado(&e, "no2");
    assert_eq!(no1.inteiro_ou("posicao", -1), no2.inteiro_ou("posicao", -2));
    assert_eq!(
        no2.campo("por_tabela")
            .and_then(|v| v.campo("b/uma"))
            .and_then(Json::inteiro),
        Some(10),
        "o painel nao mostra o vetor que o no2 publicou: {}",
        no2.escrever()
    );
    let atras = |n: &Json| -> Vec<String> {
        n.campo("atras_em")
            .and_then(Json::lista)
            .map(|l| {
                l.iter()
                    .filter_map(|x| x.texto().map(str::to_string))
                    .collect()
            })
            .unwrap_or_else(|| panic!("sem atras_em: {}", n.escrever()))
    };
    assert_eq!(atras(&no2), vec!["b/outra".to_string()], "{}", e.escrever());
    assert_eq!(atras(&no1), vec!["b/uma".to_string()], "{}", e.escrever());

    // E a medida NAO vota: a eleicao ve os dois com a mesma posicao, que e
    // a soma, exatamente como antes do vetor existir.
    let vivos = estado.vivos(crate::agora_ms());
    let pos: Vec<u64> = vivos.iter().map(|c| c.posicao).collect();
    assert_eq!(pos, vec![10, 10]);
}

/// Duas tabelas com cinco eventos cada em `b`, e um database `rascunho`
/// com tres -- o que so mora neste no.
fn com_tabela_local(nome: &str) -> (Arc<Servidor>, DirTemp) {
    com_tabela_local_com(nome, Cadastro::default(), |_| {})
}

/// [`com_tabela_local`] com cadastro e um ajuste na `Config` -- para dar
/// usuario ao cluster.
fn com_tabela_local_com(
    nome: &str,
    cadastro: Cadastro,
    ajuste: impl FnOnce(&mut Config),
) -> (Arc<Servidor>, DirTemp) {
    let (s, _caminho, guarda) = servidor_em_cluster_com(nome, cadastro, ajuste);
    let sessao = Sessao::default();
    for (db, t, n) in [("b", "uma", 5), ("b", "outra", 5), ("rascunho", "notas", 3)] {
        let _ = s.executar(
            "criar_database",
            &pedido(&format!(r#"{{"database":"{db}"}}"#)),
            &sessao,
        );
        s.executar(
            "criar_tabela",
            &pedido(&format!(
                r#"{{"database":"{db}","tabela":"{t}",
                        "colunas":[{{"nome":"id","tipo":"Int4","obrigatoria":true}}],
                        "indices":[{{"nome":"porId","colunas":["id"],"unico":true,"primario":true}}]}}"#
            )),
            &sessao,
        )
        .unwrap();
        for i in 1..=n {
            s.executar(
                "inserir",
                &pedido(&format!(
                    r#"{{"database":"{db}","tabela":"{t}","linha":{{"id":{i}}}}}"#
                )),
                &sessao,
            )
            .unwrap();
        }
    }
    (s, guarda)
}

/// **Pedido 300 (3): a replica soma SO o que o master anunciou.**
///
/// O master anunciou o database `b` com a tabela `uma`. Este no, replica,
/// tem ainda `b/outra` (que o master nao serve) e o database `rascunho`
/// inteiro (que so mora aqui): 13 eventos no disco, 5 replicados. A
/// posicao tem de ser 5, completa, e o vetor so com `b/uma`.
///
/// O vermelho: com a soma de antes (`db.todas_as_tabelas()` de todo
/// database, sem filtro), a posicao sai 13 e este no ganharia a eleicao de
/// quem tem os 5 que importam e mais nada. Medido em 01/10/2026 com o
/// filtro tirado: `left: 13, right: 5`.
#[test]
fn a_replica_soma_so_o_que_o_master_anunciou() {
    let (s, _guarda) = com_tabela_local("anunciadas");
    let estado = s.cluster.clone().expect("cluster");
    estado.rebaixar(1).unwrap();
    estado.anunciar_databases(&["b".to_string()]);
    estado.anunciar_tabelas("b", vec!["uma".to_string()]);
    s.contar_posicao_do_cluster(&estado);
    assert_eq!(estado.posicao(), 5, "a tabela local entrou na soma");
    assert!(!estado.posicao_incompleta());
    assert_eq!(estado.por_tabela(), vec![("b/uma".to_string(), 5)]);

    // E o anuncio e DURAVEL: o no que reinicia sabe o que o master
    // anunciava, e o arranque a frio nao volta a contar o rascunho.
    let base = s.config.base.clone();
    let de_volta = crate::cluster::EstadoCluster::novo(
        estado.config.clone(),
        &base,
        crate::config::Papel::Replica,
    );
    assert_eq!(de_volta.anunciadas(), estado.anunciadas());
}

/// Os dois irmaos que travam o comportamento: o MASTER continua somando
/// tudo o que tem (13 -- e o que ele serve), e a replica que NUNCA ouviu
/// um master soma tudo e sai INCOMPLETA, em vez de se declarar completa
/// com um numero que pode carregar tabela local.
/// O usuario do cluster: replica tudo, menos `b/outra` -- a folha que o
/// dono do master guardou para si.
fn replicador_sem_a_outra() -> Usuario {
    let mut u = leitor();
    u.id = 11;
    u.login = "clu".into();
    u.bases = vec![(
        "*".into(),
        Permissoes {
            ler: true,
            replicar: true,
            ..Permissoes::default()
        },
    )];
    u.tabelas = vec![("b".into(), vec![("outra".into(), Permissoes::default())])];
    u
}

/// **Pedido 300, o resto do (3): o master conta so o que a replica
/// ALCANCA.** O usuario do cluster nao pode `replicar` `b/outra`: nenhuma
/// replica a tera nunca, e somada ela punha o master 5 eventos a frente
/// de toda replica por um dado que nao viaja. Posicao 8 (`b/uma` 5 +
/// `rascunho/notas` 3), e `b/outra` fora do vetor.
///
/// O vermelho: com o master somando tudo (`DoQueSeServe(None)` no lugar da
/// ficha do usuario), a posicao sai 13.
#[test]
fn o_master_nao_conta_a_tabela_negada_ao_usuario_do_cluster() {
    let mut cadastro = Cadastro::default();
    cadastro.usuarios.push(replicador_sem_a_outra());
    let (s, _guarda) = com_tabela_local_com("negada", cadastro, |c| {
        c.cluster.as_mut().unwrap().usuario = "clu".into();
    });
    let estado = s.cluster.clone().expect("cluster");
    s.contar_posicao_do_cluster(&estado);
    assert_eq!(
        estado.posicao(),
        8,
        "a tabela negada ao cluster entrou na soma"
    );
    assert!(!estado.posicao_incompleta());
    assert!(
        !estado.por_tabela().iter().any(|(t, _)| t == "b/outra"),
        "{:?}",
        estado.por_tabela()
    );
    // O MESMO motor responde o `posicao` que a replica pergunta: a soma
    // do master e o que a replica ve batem, tabela a tabela.
    let r = s
        .op_posicao(
            &pedido(r#"{"database":"b"}"#),
            &Sessao {
                usuario: Some(replicador_sem_a_outra()),
                ..Sessao::default()
            },
        )
        .unwrap();
    let tabelas = r.campo("tabelas").expect("tabelas");
    assert!(tabelas.campo("uma").is_some(), "{}", r.escrever());
    assert!(tabelas.campo("outra").is_none(), "{}", r.escrever());
}

/// O comportamento velho, pelo outro lado: usuario do cluster que o
/// cadastro NAO tem (nenhuma replica entra com ele) nao inventa filtro --
/// o master soma tudo, como antes.
#[test]
fn usuario_do_cluster_fora_do_cadastro_soma_como_antes() {
    let (s, _guarda) = com_tabela_local_com("sem-ficha", Cadastro::default(), |c| {
        c.cluster.as_mut().unwrap().usuario = "ninguem".into();
    });
    let estado = s.cluster.clone().expect("cluster");
    s.contar_posicao_do_cluster(&estado);
    assert_eq!(estado.posicao(), 13);
}

#[test]
fn o_master_soma_tudo_e_a_replica_sem_anuncio_diz_que_nao_sabe() {
    let (s, _guarda) = com_tabela_local("sem-anuncio");
    let estado = s.cluster.clone().expect("cluster");
    s.contar_posicao_do_cluster(&estado);
    assert_eq!(estado.posicao(), 13, "o master soma o que ele serve");
    assert!(!estado.posicao_incompleta());

    estado.rebaixar(1).unwrap();
    s.contar_posicao_do_cluster(&estado);
    assert_eq!(estado.posicao(), 13);
    assert!(
        estado.posicao_incompleta(),
        "a replica sem anuncio nao sabe o que e replicado, e tem de dizer"
    );

    // Anunciado o database e ainda nao perguntadas as tabelas: conta o
    // database inteiro, deixa o rascunho de fora, e continua incompleta.
    estado.anunciar_databases(&["b".to_string()]);
    s.contar_posicao_do_cluster(&estado);
    assert_eq!(estado.posicao(), 10);
    assert!(estado.posicao_incompleta());
}

/// **O comportamento VELHO**: o pulso de um no anterior ao vetor nao traz
/// `por_tabela`, e o painel diz «nao medido» (nulo) em vez de acusar esse
/// no de estar atras em TODA tabela.
#[test]
fn pulso_sem_vetor_e_nao_medido_e_nao_atras_em_tudo() {
    let (s, _caminho, _guarda) = servidor_em_cluster("sem-vetor", Cadastro::default());
    let sessao = Sessao::default();
    s.executar("cluster_pulso", &pulso_de("no2"), &sessao)
        .expect("o pulso antigo continua aceito");
    let e = s
        .executar("cluster_estado", &pedido("{}"), &sessao)
        .unwrap();
    let no2 = no_do_estado(&e, "no2");
    assert!(
        matches!(no2.campo("por_tabela"), Some(Json::Nulo)),
        "{}",
        no2.escrever()
    );
    assert!(
        matches!(no2.campo("atras_em"), Some(Json::Nulo)),
        "{}",
        no2.escrever()
    );
}

/// Pedido 217, os dois sentidos numa prova so: o pulso de um no que nao
/// esta na lista e RECUSADO -- e passa a ser aceito assim que a operacao
/// de escalonamento o acrescenta, sem reiniciar nada.
///
/// A recusa de antes tem de continuar: sem ela, qualquer credencial de
/// replicacao inflaria o denominador da maioria com nos fantasmas.
#[test]
fn no_acrescentado_a_quente_passa_a_ser_aceito_no_pulso() {
    let (s, caminho, _guarda) = servidor_em_cluster("pulso", Cadastro::default());
    let sessao = Sessao::default();
    let estado = s.cluster.clone().expect("cluster");
    assert_eq!(estado.total(), 2);

    // (1) ANTES: o no3 nao existe para este servidor.
    let e = s
        .executar("cluster_pulso", &pulso_de("no3"), &sessao)
        .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO", "veio {e}");
    assert!(format!("{e}").contains("no3"), "{e}");

    // (2) o escalonamento a quente. `propagar:false` porque aqui nao ha
    // outro no de pe -- a propagacao pelo soquete quem prova e a bancada.
    let r = s
        .executar(
            "cluster_no_acrescentar",
            &pedido(r#"{"id":"no3","endereco":"10.0.0.3","porta":5400,"propagar":false}"#),
            &sessao,
        )
        .unwrap();
    assert!(r.booleano_ou("acrescentado", false), "{}", r.escrever());
    assert_eq!(r.inteiro_ou("nos", 0), 3);

    // (3) DEPOIS: o mesmo pulso passa, e sem reiniciar nada.
    s.executar("cluster_pulso", &pulso_de("no3"), &sessao)
        .expect("o pulso do no acrescentado tinha de passar");

    // (4) o denominador da maioria mudou junto: 2 de 3 e maioria, 1 nao.
    assert_eq!(estado.total(), 3);
    assert!(estado.e_maioria(2));
    assert!(!estado.e_maioria(1));

    // (5) e o arquivo tambem, senao o no sumiria no proximo arranque.
    let texto = std::fs::read_to_string(&caminho).unwrap();
    assert!(
        texto.contains("no3"),
        "o config.json nao guardou o no novo:\n{texto}"
    );
    let relido = Config::ler(&caminho).unwrap();
    assert_eq!(relido.cluster.unwrap().nos.len(), 3);

    // (6) a op `config` conta a lista VIVA -- o encontro do 217 com o 218.
    let c = s.executar("config", &pedido("{}"), &sessao).unwrap();
    let nos = c
        .campo("cluster")
        .and_then(|cl| cl.campo("nos"))
        .and_then(Json::lista)
        .expect("cluster.nos na resposta de config");
    assert_eq!(nos.len(), 3, "{}", c.escrever());
}

/// Repetir a ordem nao duplica o no: a propagacao repete de proposito, e
/// repeticao que vira erro faria a segunda tentativa de escalonar falhar
/// nos nos que ja tinham aceitado a primeira.
#[test]
fn acrescentar_o_mesmo_no_duas_vezes_e_idempotente() {
    let (s, _, _guarda) = servidor_em_cluster("idem", Cadastro::default());
    let ordem = pedido(r#"{"id":"no3","endereco":"10.0.0.3","porta":5400,"propagar":false}"#);
    let a = s
        .executar("cluster_no_acrescentar", &ordem, &Sessao::default())
        .unwrap();
    let b = s
        .executar("cluster_no_acrescentar", &ordem, &Sessao::default())
        .unwrap();
    assert!(a.booleano_ou("acrescentado", false));
    assert!(
        !b.booleano_ou("acrescentado", true),
        "duplicou: {}",
        b.escrever()
    );
    assert_eq!(b.inteiro_ou("nos", 0), 3);
}

/// As duas recusas do `cluster_no_remover`, que sao decisao e nao descuido.
#[test]
fn remover_nao_alcanca_este_no_nem_o_master() {
    let (s, _, _guarda) = servidor_em_cluster("remover", Cadastro::default());
    let sessao = Sessao::default();
    // Este servidor e o no1 E o master do config -- as duas recusas caem
    // sobre o mesmo id, e a primeira e a que responde.
    let e = s
        .executar(
            "cluster_no_remover",
            &pedido(r#"{"id":"no1","propagar":false}"#),
            &sessao,
        )
        .unwrap_err();
    assert!(format!("{e}").contains("ESTE servidor"), "{e}");

    // Com tres nos, o no3 sai e sobram dois.
    s.executar(
        "cluster_no_acrescentar",
        &pedido(r#"{"id":"no3","endereco":"10.0.0.3","propagar":false}"#),
        &sessao,
    )
    .unwrap();
    let r = s
        .executar(
            "cluster_no_remover",
            &pedido(r#"{"id":"no3","propagar":false}"#),
            &sessao,
        )
        .unwrap();
    assert!(r.booleano_ou("removido", false), "{}", r.escrever());
    assert_eq!(r.inteiro_ou("nos", 0), 2);

    // E a terceira recusa, que vem do `Cluster::validar` e nao das duas de
    // cima: sobrar UM no nao e cluster nenhum. O que importa aqui e o
    // DESFAZER -- a memoria volta a dois, porque uma lista viva menor que
    // a do arquivo seriam duas maiorias diferentes no mesmo servidor.
    let e = s
        .executar(
            "cluster_no_remover",
            &pedido(r#"{"id":"no2","propagar":false}"#),
            &sessao,
        )
        .unwrap_err();
    assert!(format!("{e}").contains("menos de dois nos"), "{e}");
    assert_eq!(
        s.cluster.clone().unwrap().total(),
        2,
        "a memoria ficou menor que o arquivo depois da recusa"
    );
}

/// O portao das duas operacoes de escalonamento, pelo `despachar` -- que e
/// por onde o pedido entra de verdade.
#[test]
fn operador_sem_administrar_nao_mexe_na_lista_de_nos() {
    let mut cadastro = Cadastro::default();
    cadastro.usuarios.push(operador());
    let usuario = cadastro.usuarios[0].clone();
    let (s, _, _guarda) = servidor_em_cluster("portao", cadastro);
    for op in ["cluster_no_acrescentar", "cluster_no_remover"] {
        let mut sessao = Sessao {
            usuario: Some(usuario.clone()),
            ..Sessao::default()
        };
        let (_, _, r) = s.despachar(
            &format!(r#"{{"op":"{op}","token":"t","id":"no9","endereco":"10.0.0.9"}}"#),
            &mut sessao,
            "1.2.3.4",
        );
        let e = r.unwrap_err();
        assert!(
            format!("{e}").contains("administrar"),
            "o operador mexeu na lista de nos por {op}: {e}"
        );
    }
    assert_eq!(s.cluster.clone().unwrap().total(), 2);
}

// ------------------------------------------------ A6: o mapa so para quem administra

/// **Prova real do A6 (revisao SEC de 17/09/2026, pedido 283).** Quem so
/// tem `ler` recebia a lista inteira dos nos -- endereco, epoca, posicao e
/// idade do pulso de cada um --, que e o mapa de que o pulso forjado do
/// A1 precisa. `ler` continua achando o master; `nos[]` exige administrar.
///
/// **Defeito reposto**: `administra = true` sem consultar a sessao, e a
/// primeira asercao cai com `nos` presente.
#[test]
fn cluster_estado_de_leitor_nao_lista_os_nos() {
    let (s, _, _guarda) = servidor_em_cluster("estado-leitor", Cadastro::default());
    let r = s
        .executar(
            "cluster_estado",
            &pedido("{}"),
            &sessao_com(Some(leitor()), "203.0.113.5"),
        )
        .unwrap();
    assert!(
        r.campo("nos").is_none(),
        "o leitor recebeu o mapa dos nos: {}",
        r.escrever()
    );
    // O que o cliente precisa para achar quem manda continua la.
    assert!(r.campo("master").is_some(), "{}", r.escrever());
    assert!(r.campo("escrita_liberada").is_some(), "{}", r.escrever());
    assert_eq!(r.texto_ou("id", ""), "no1");
    assert!(
        !r.escrever().contains("5398"),
        "a porta de outro no vazou para o leitor: {}",
        r.escrever()
    );
}

/// O comportamento VELHO: o administrador -- por usuario ou pelo token de
/// servico -- continua recebendo a lista inteira, com endereco e tudo.
#[test]
fn cluster_estado_de_administrador_continua_completo() {
    let (s, _, _guarda) = servidor_em_cluster("estado-adm", Cadastro::default());
    for sessao in [
        Sessao::default(),
        sessao_com(Some(administrador()), "203.0.113.5"),
    ] {
        let r = s
            .executar("cluster_estado", &pedido("{}"), &sessao)
            .unwrap();
        let nos = r
            .campo("nos")
            .and_then(Json::lista)
            .unwrap_or_else(|| panic!("sem `nos` para quem administra: {}", r.escrever()));
        assert_eq!(nos.len(), 2);
        assert!(nos
            .iter()
            .any(|n| n.texto_ou("endereco", "") == "127.0.0.1:5398"));
        assert!(nos.iter().all(|n| n.campo("ultimo_pulso_ms").is_some()));
    }
}

// ------------------------------------------------ A4: propagar:false so de dentro

/// **Prova real do A4 (revisao SEC de 17/09/2026, pedido 281).** Dois
/// `cluster_no_remover` com `propagar:false` vindos de um CLIENTE deixavam
/// o master sozinho na propria lista, contando maioria sobre um, enquanto
/// os outros dois promoviam outro master: dois masters gravaveis sem
/// particao nenhuma. O campo passa a valer so para a ordem que o proprio
/// cluster propaga -- credencial do cluster, de um endereco da lista.
///
/// **Defeito reposto**: tirar a chamada a `sem_propagar_so_de_dentro` das
/// duas ops, e as duas primeiras asercoes caem com `removido: true`.
#[test]
fn propagar_false_de_cliente_e_recusado() {
    let (s, _, _guarda) = servidor_em_cluster("sem-propagar-cliente", Cadastro::default());
    let estado = s.cluster.clone().unwrap();
    // Tres nos, como no cenario do SEC: de dentro (sem IP), o terceiro
    // entra -- e o caminho da propagacao, que continua valendo.
    s.executar(
        "cluster_no_acrescentar",
        &pedido(r#"{"id":"no3","endereco":"10.0.0.3","porta":5400,"propagar":false}"#),
        &Sessao::default(),
    )
    .unwrap();
    assert_eq!(estado.total(), 3);
    let cliente = sessao_com(Some(administrador()), "203.0.113.5");
    let e = s
        .executar(
            "cluster_no_remover",
            &pedido(r#"{"id":"no2","propagar":false}"#),
            &cliente,
        )
        .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert!(e.to_string().contains("propagar"), "{e}");
    assert_eq!(estado.total(), 3, "a lista viva mudou apesar da recusa");

    // O espelho: acrescentar fantasmas sem propagar recusaria toda escrita.
    let e = s
        .executar(
            "cluster_no_acrescentar",
            &pedido(r#"{"id":"no9","endereco":"10.0.0.9","porta":5000,"propagar":false}"#),
            &cliente,
        )
        .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    assert_eq!(estado.total(), 3);

    // E o mesmo cliente, pelo token de servico, tambem nao passa: o que
    // decide e de ONDE a ordem vem, nao quem e.
    let e = s
        .executar(
            "cluster_no_remover",
            &pedido(r#"{"id":"no2","propagar":false}"#),
            &sessao_com(None, "203.0.113.5"),
        )
        .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
}

/// A ordem INTERNA continua passando: e a propagacao de um no da lista,
/// com a credencial do cluster. E com usuario no cluster, a credencial
/// tem de bater -- vir do endereco certo nao basta.
#[test]
fn propagar_false_de_um_no_do_cluster_passa() {
    let (s, _, _guarda) = servidor_em_cluster("sem-propagar-no", Cadastro::default());
    let estado = s.cluster.clone().unwrap();
    // 127.0.0.1 e o endereco de no2 na lista; cluster sem usuario, so token.
    let r = s
        .executar(
            "cluster_no_acrescentar",
            &pedido(r#"{"id":"no3","endereco":"10.0.0.3","porta":5400,"propagar":false}"#),
            &sessao_com(None, "127.0.0.1"),
        )
        .unwrap();
    assert!(r.booleano_ou("acrescentado", false), "{}", r.escrever());
    assert_eq!(estado.total(), 3);
    // O IPv4 mapeado em IPv6 e o mesmo endereco.
    let r = s
        .executar(
            "cluster_no_remover",
            &pedido(r#"{"id":"no3","propagar":false}"#),
            &sessao_com(None, "::ffff:127.0.0.1"),
        )
        .unwrap();
    assert!(r.booleano_ou("removido", false), "{}", r.escrever());

    // Cluster COM usuario: o endereco certo com outra credencial nao passa.
    let mut cadastro = Cadastro::default();
    cadastro.usuarios.push(administrador());
    let (s, _, _guarda) = servidor_em_cluster_com("sem-propagar-cred", cadastro, |c| {
        c.cluster.as_mut().unwrap().usuario = "clu".into();
    });
    let ordem = pedido(r#"{"id":"no3","endereco":"10.0.0.3","porta":5400,"propagar":false}"#);
    let e = s
        .executar(
            "cluster_no_acrescentar",
            &ordem,
            &sessao_com(Some(administrador()), "127.0.0.1"),
        )
        .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
    // ...e a credencial do cluster, do endereco certo, passa.
    let mut clu = administrador();
    clu.login = "clu".into();
    let r = s
        .executar(
            "cluster_no_acrescentar",
            &ordem,
            &sessao_com(Some(clu), "127.0.0.1"),
        )
        .unwrap();
    assert!(r.booleano_ou("acrescentado", false), "{}", r.escrever());
}

/// Um usuario que so `replicar` nas bases listadas.
fn so_replica(login: &str, bases: &[&str]) -> Usuario {
    let mut u = operador();
    u.login = login.into();
    u.bases = bases
        .iter()
        .map(|b| {
            (
                b.to_string(),
                Permissoes {
                    replicar: true,
                    ..Permissoes::default()
                },
            )
        })
        .collect();
    u
}

fn aguardar_como(s: &Servidor, quem: &Usuario, confirmado: &str) -> Result<Json> {
    s.executar(
        "replicar_aguardar",
        &pedido(&format!(r#"{{"id":"no2","confirmado":[{confirmado}]}}"#)),
        &sessao_com(Some(quem.clone()), "127.0.0.1"),
    )
}

fn exigencia(db: &str, t: &str) -> Vec<crate::quorum::Exigencia> {
    vec![crate::quorum::Exigencia {
        database: db.into(),
        tabela: t.into(),
        posicao: 5,
    }]
}

fn servidor_de_quorum(nome: &str, usuario_do_cluster: &str) -> (Arc<Servidor>, DirTemp) {
    let usuario = usuario_do_cluster.to_string();
    let (s, _, g) = servidor_em_cluster_com(nome, Cadastro::default(), move |c| {
        let cl = c.cluster.as_mut().unwrap();
        cl.usuario = usuario;
        cl.quorum_minimo = 1;
        cl.quorum_prazo_ms = 100;
    });
    assert_eq!(
        s.cluster.as_ref().unwrap().papel(),
        crate::cluster::PapelVivo::Master
    );
    (s, g)
}

const A_5: &str = r#"{"database":"A","tabela":"t","posicao":5}"#;
const B_5: &str = r#"{"database":"B","tabela":"t","posicao":5}"#;

/// **Pedido 649: o ack do quorum so vale para a tabela que a SESSAO
/// alcanca, e a ficha do cluster so e gravada por quem e do cluster.**
///
/// Cenario A: um `Replicar` so da base A confirma B (que nenhuma replica
/// gravou) e o quorum de B nao pode fechar. Cenario B: a ficha do cluster
/// (alcance de todas as replicas) nao e sobrescrita por ele.
///
/// # O vermelho
///
/// Sem o filtro `replica_alcanca` nos confirmados, o `esperar` de B
/// alcanca; sem `sessao_e_do_cluster`, a ficha vira a do `fraco`.
#[test]
fn replicar_de_outra_credencial_nao_forja_o_ack_nem_a_ficha() {
    let (s, _g) = servidor_de_quorum("ack649", "clu");
    let cubo = s.quorum.clone().expect("cubo");
    let clu = so_replica("clu", &["*"]);
    let fraco = so_replica("fraco", &["A"]);
    aguardar_como(&s, &clu, "").unwrap();
    assert_eq!(cubo.ficha().unwrap().login, "clu");

    aguardar_como(&s, &fraco, &format!("{A_5},{B_5}")).unwrap();
    assert_eq!(
        cubo.ficha().unwrap().login,
        "clu",
        "a ficha do cluster foi sobrescrita por outra credencial"
    );
    let b = cubo.esperar(vec![], &exigencia("B", "t"));
    assert!(!b.alcancado, "o ack forjado de B fechou o quorum: {b:?}");
    // O que ele de fato alcanca continua contando: nao e um portao que
    // recusaria tudo.
    let a = cubo.esperar(vec![], &exigencia("A", "t"));
    assert!(a.alcancado, "o ack legitimo de A nao contou: {a:?}");
}

/// O comportamento VELHO (cluster de um usuario so, ou sem usuario): a
/// credencial do cluster confirma qualquer tabela que alcanca e grava a
/// ficha. O portao do 649 nao pode recusar o cluster que ja rodava.
#[test]
fn cluster_de_um_usuario_so_confirma_e_grava_a_ficha_como_antes() {
    let (s, _g) = servidor_de_quorum("ack649-velho", "clu");
    let cubo = s.quorum.clone().expect("cubo");
    aguardar_como(&s, &so_replica("clu", &["*"]), &format!("{A_5},{B_5}")).unwrap();
    assert_eq!(cubo.ficha().unwrap().login, "clu");
    assert!(cubo.esperar(vec![], &exigencia("B", "t")).alcancado);

    // Sem `cluster.usuario`: qualquer sessao com ficha a grava, como
    // sempre foi.
    let (s, _g) = servidor_de_quorum("ack649-sem-usuario", "");
    let cubo = s.quorum.clone().expect("cubo");
    aguardar_como(&s, &so_replica("qualquer", &["*"]), B_5).unwrap();
    assert_eq!(cubo.ficha().unwrap().login, "qualquer");
    assert!(cubo.esperar(vec![], &exigencia("B", "t")).alcancado);
}

/// A lista aceita NOME de host («host ou IP»), e a propagacao de um
/// cluster configurado por nome chega de um IP. Comparar so texto
/// recusaria a propria propagacao do cluster -- guarda que quebra o
/// cluster que ja rodava e estrago. `localhost` resolve para 127.0.0.1 em
/// qualquer maquina, entao a prova nao depende de DNS de fora.
#[test]
fn propagar_false_de_um_no_por_nome_de_host_passa() {
    let (s, _, _guarda) = servidor_em_cluster_com("sem-propagar-nome", Cadastro::default(), |c| {
        let cl = c.cluster.as_mut().unwrap();
        for n in cl.nos.iter_mut() {
            n.endereco = "localhost".into();
        }
    });
    let estado = s.cluster.clone().unwrap();
    let r = s
        .executar(
            "cluster_no_acrescentar",
            &pedido(r#"{"id":"no3","endereco":"10.0.0.3","porta":5400,"propagar":false}"#),
            &sessao_com(None, "127.0.0.1"),
        )
        .unwrap();
    assert!(r.booleano_ou("acrescentado", false), "{}", r.escrever());
    assert_eq!(estado.total(), 3);
    // Um IP que o nome NAO resolve continua de fora.
    let e = s
        .executar(
            "cluster_no_remover",
            &pedido(r#"{"id":"no3","propagar":false}"#),
            &sessao_com(None, "203.0.113.5"),
        )
        .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");
}

/// O comportamento VELHO, que e o que importa: sem o campo, a ordem de um
/// cliente e aceita e PROPAGADA como sempre -- o veredito por no continua
/// vindo na resposta (aqui no2 nao esta de pe, e o veredito diz isso).
#[test]
fn propagar_padrao_continua_igual() {
    let (s, _, _guarda) = servidor_em_cluster("propagar-padrao", Cadastro::default());
    let estado = s.cluster.clone().unwrap();
    let r = s
        .executar(
            "cluster_no_acrescentar",
            &pedido(r#"{"id":"no3","endereco":"10.0.0.3","porta":5400}"#),
            &sessao_com(Some(administrador()), "203.0.113.5"),
        )
        .unwrap();
    assert!(r.booleano_ou("acrescentado", false), "{}", r.escrever());
    assert_eq!(estado.total(), 3);
    let propagado = r
        .campo("propagado")
        .unwrap_or_else(|| panic!("sem veredito de propagacao: {}", r.escrever()));
    assert!(
        propagado.campo("no2").is_some(),
        "a ordem nao foi propagada a no2: {}",
        r.escrever()
    );
}

// ------------------------------------------------ A10: `config` mostra a lista viva

/// **Prova real do A10 (revisao SEC de 17/09/2026, pedido 287).** A op
/// `config` publicava o retrato do arranque enquanto `cluster_estado`
/// tinha a lista viva, e a diferenca entre as duas e o denominador da
/// maioria. A injecao da lista viva ja existia desde o 217 (`a446c7a`);
/// o que faltava era a prova, e a paridade de campos com o arquivo
/// (`tem_pino`).
///
/// **Defeito reposto**: tirar o bloco `if let (Some(estado), ...)` do
/// `configuracao_json`, e a contagem cai com 2 contra 3.
#[test]
fn config_mostra_a_lista_viva_do_cluster() {
    let (s, _, _guarda) = servidor_em_cluster("config-vivo", Cadastro::default());
    let pino = "ab".repeat(32);
    s.executar(
        "cluster_no_acrescentar",
        &pedido(&format!(
            r#"{{"id":"no3","endereco":"10.0.0.3","porta":5400,"chave_do_fio":"{pino}","propagar":false}}"#
        )),
        &Sessao::default(),
    )
    .unwrap();
    let cfg = s
        .executar("config", &pedido("{}"), &Sessao::default())
        .unwrap();
    let no_config = cfg
        .campo("cluster")
        .and_then(|c| c.campo("nos"))
        .and_then(Json::lista)
        .map(|l| l.to_vec())
        .unwrap_or_default();
    let est = s
        .executar("cluster_estado", &pedido("{}"), &Sessao::default())
        .unwrap();
    let no_estado = est
        .campo("nos")
        .and_then(Json::lista)
        .map(|l| l.len())
        .unwrap_or(0);
    assert_eq!(no_estado, 3);
    assert_eq!(
        no_config.len(),
        no_estado,
        "`config` mostra a lista de ontem: {}",
        cfg.escrever()
    );
    let no3 = no_config
        .iter()
        .find(|n| n.texto_ou("id", "") == "no3")
        .expect("no3 na lista do config");
    assert!(
        no3.booleano_ou("tem_pino", false),
        "a lista viva diz menos que a do arquivo"
    );
    assert!(!cfg.escrever().contains(&pino), "o pino vazou no config");
}

// ------------------------------------------------ A11: o pulso nao e oraculo

/// **Prova real do A11 (revisao SEC de 17/09/2026, pedido 288).** O pulso
/// respondia tres coisas -- «nao esta na lista», «e ESTE servidor», ou o
/// pulso --, e as duas recusas enumeravam os ids do cluster de graca.
/// Agora as duas recusas sao UMA frase, com o id, e o id duplicado vai
/// para o log do processo, nao para o fio.
///
/// **Defeito reposto**: voltar as duas recusas separadas, e a comparacao
/// dos dois textos cai.
#[test]
fn pulso_de_id_desconhecido_e_do_proprio_id_dao_a_mesma_resposta() {
    let (s, _, _guarda) = servidor_em_cluster("oraculo", Cadastro::default());
    let sessao = sessao_com(None, "203.0.113.5");
    let fora = s
        .executar("cluster_pulso", &pulso_de("no9"), &sessao)
        .unwrap_err();
    let proprio = s
        .executar("cluster_pulso", &pulso_de("no1"), &sessao)
        .unwrap_err();
    assert_eq!(fora.nome(), "ACESSO_NEGADO");
    assert_eq!(proprio.nome(), "ACESSO_NEGADO");
    assert_eq!(
        fora.to_string().replace("no9", "{id}"),
        proprio.to_string().replace("no1", "{id}"),
        "as duas recusas diferem, e a diferenca e o oraculo"
    );
    // O no da lista continua pulsando, como sempre.
    s.executar("cluster_pulso", &pulso_de("no2"), &sessao)
        .unwrap_or_else(|e| panic!("o pulso do no da lista: {e}"));
}

/// A recusa conta como tentativa leve SO com o interruptor ligado: o no
/// que entra a quente pulsa os antigos antes de ser acrescentado, e com
/// isto de fabrica cinco pulsos bloqueariam o IP dele por uma hora em
/// cada antigo -- na bancada, onde todos sao 127.0.0.1, o cluster inteiro.
/// Guarda nova entra pedida.
#[test]
fn pulso_desconhecido_conta_leve_so_com_o_interruptor() {
    let ip = "203.0.113.77";
    let (s, _, _guarda) = servidor_em_cluster("oraculo-leve-off", Cadastro::default());
    for _ in 0..3 {
        let _ = s.executar("cluster_pulso", &pulso_de("no9"), &sessao_com(None, ip));
    }
    assert_eq!(
        s.lista_negra.lock().unwrap().tentativas_de(ip),
        0,
        "sem o interruptor, o pulso desconhecido contou tentativa"
    );

    let (s, _, _guarda) = servidor_em_cluster_com("oraculo-leve-on", Cadastro::default(), |c| {
        c.politica.contar_pulso_desconhecido = true;
    });
    for _ in 0..3 {
        let _ = s.executar("cluster_pulso", &pulso_de("no9"), &sessao_com(None, ip));
    }
    assert_eq!(s.lista_negra.lock().unwrap().tentativas_de(ip), 3);
    // O no da lista nunca conta.
    s.executar("cluster_pulso", &pulso_de("no2"), &sessao_com(None, ip))
        .unwrap();
    assert_eq!(s.lista_negra.lock().unwrap().tentativas_de(ip), 3);
}

/// Guarda nova entra PEDIDA, nao imposta: um servidor SEM bloco `cluster`
/// continua sem nada disto, e as duas ops recusam dizendo por que.
#[test]
fn sem_bloco_cluster_as_ops_de_escalonar_recusam_explicando() {
    let (s, _, _guarda) = servidor_de_arquivo("sem-cluster", Cadastro::default());
    let e = s
        .executar(
            "cluster_no_acrescentar",
            &pedido(r#"{"id":"no2","endereco":"10.0.0.2"}"#),
            &Sessao::default(),
        )
        .unwrap_err();
    assert!(format!("{e}").contains("nao esta em cluster"), "{e}");
}

/// A OUTRA metade da cifra do cluster: a origem com que a replicacao puxa
/// do master leva a cifra e o pino do no de destino. A leitura nao pega que
/// a linha `cifra: c.cifra` some -- por isso esta pura, para o teste pegar.
#[test]
fn origem_do_cluster_carrega_a_cifra_e_o_pino() {
    let pino = "cd".repeat(32);
    let txt = format!(
        r#"{{"token":"t","replicacao":{{"papel":"replica"}},
                "cluster":{{"id":"no1","cifra":true,
                  "nos":[
                    {{"id":"no1","endereco":"127.0.0.1","porta":5310}},
                    {{"id":"no2","endereco":"10.0.0.2","porta":5311,"chave_do_fio":"{pino}"}}]}}}}"#
    );
    let c = Config::de_json(&Json::analisar(&txt).unwrap())
        .unwrap()
        .cluster
        .unwrap();
    let no2 = c.no("no2").unwrap().clone();
    let o = origem_do_master(&c, &no2);
    assert!(
        o.cifra,
        "a replicacao do cluster saiu em claro com a cifra ligada"
    );
    assert_eq!(
        o.chave_do_fio, pino,
        "o pino do master nao chegou na origem"
    );
    assert_eq!(o.pino_do_fio().unwrap(), Some([0xcdu8; 32]));
    assert_eq!(o.host, "10.0.0.2");
    assert_eq!(o.porta, 5311);

    // Cifra desligada pelo escape ESCRITO: a replicacao do cluster segue o
    // interruptor para os DOIS lados, e nao so para ligar. Ate 18/09/2026
    // este par tirava o campo do arquivo (o padrao era claro); com o padrao
    // ligado, tirar o campo deixaria de provar coisa alguma -- as duas
    // metades do teste dariam cifrado.
    let claro = txt.replace(r#""cifra":true,"#, r#""cifra":false,"#);
    let cc = Config::de_json(&Json::analisar(&claro).unwrap())
        .unwrap()
        .cluster
        .unwrap();
    let o2 = origem_do_master(&cc, &cc.no("no2").unwrap().clone());
    // O pino continua copiado (ele mora no no), mas `cifra` desligada e o
    // que decide: sem ela a origem nao aperta a mao, e o pino fica inerte.
    assert!(
        !o2.cifra,
        "a replicacao do cluster cifrou sem ninguem pedir"
    );
}

/// **O IRMAO do padrao de saida, e ele mora fora do `config.rs`.**
///
/// A sonda `replicacao_testar` monta a `Origem` com o que veio no pedido e
/// cai no MESMO `replica::ligar` do laco. Padrao que divergisse aqui faria
/// a tela provar a ligacao em CLARO e a replicacao configurada logo depois
/// pedir o aperto: "testei e funcionou" para uma configuracao que nao
/// sobe. O teste compara os DOIS caminhos, e nao so o valor, porque o
/// defeito e a divergencia entre eles.
#[test]
fn a_sonda_de_replicacao_tem_o_mesmo_padrao_de_cifra_do_arquivo() {
    let pedido = Json::analisar(r#"{"host":"10.0.0.9","porta":5000}"#).unwrap();
    let o = origem_da_sonda(&pedido, "10.0.0.9".into());
    assert!(
        o.cifra,
        "a sonda ia falar claro com um source que o laco vai pedir cifrado"
    );
    let do_arquivo = Config::de_json(
        &Json::analisar(
            r#"{"token":"x","replicacao":{"papel":"replica","origens":[
                     {"nome":"matriz","host":"10.0.0.9","porta":5000}]}}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        o.cifra, do_arquivo.replicacao.origens[0].cifra,
        "a sonda e o arquivo divergiram no padrao da cifra"
    );
    // O escape ESCRITO vale aqui tambem: quem sonda um source anterior ao
    // aperto manda `"cifra": false` no proprio pedido.
    let claro = Json::analisar(r#"{"host":"10.0.0.9","cifra":false}"#).unwrap();
    assert!(!origem_da_sonda(&claro, "10.0.0.9".into()).cifra);
}

/// O no acrescentado a quente com pino guarda o pino no `config.json` -- e o
/// que faz o cluster cifrado nao perder a ancora de um no escalonado no
/// proximo arranque. Reescrever `cluster.nos` sem esta linha derrubaria o
/// pino de TODO no de uma vez.
#[test]
fn no_acrescentado_a_quente_carrega_o_pino() {
    let (s, caminho, _guarda) = servidor_em_cluster("pino-a-quente", Cadastro::default());
    let pino = "ef".repeat(32);
    let r = s
        .executar(
            "cluster_no_acrescentar",
            &pedido(&format!(
                r#"{{"id":"no3","endereco":"10.0.0.3","porta":5400,"chave_do_fio":"{pino}","propagar":false}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    assert!(r.booleano_ou("acrescentado", false), "{}", r.escrever());

    // Releitura do arquivo: o pino sobreviveu a reescrita da lista inteira.
    let relido = Config::ler(&caminho).unwrap().cluster.unwrap();
    let no3 = relido.no("no3").expect("no3 no config relido");
    assert_eq!(no3.chave_do_fio, pino, "o pino do no novo nao foi gravado");
    assert_eq!(no3.pino_do_fio().unwrap(), Some([0xefu8; 32]));

    // E o pino NUNCA aparece na resposta da op -- so o fato de acrescentar.
    assert!(
        !r.escrever().contains(&pino),
        "o pino vazou na resposta: {}",
        r.escrever()
    );
}

#[test]
fn grava_no_arquivo_e_aplica_a_quente() {
    let (s, caminho, _guarda) = servidor_de_arquivo("quente", Cadastro::default());
    let sessao = Sessao::default();
    assert_eq!(s.max_linhas(), 1000);

    let r = s
        .executar(
            "config_gravar",
            &pedido(r#"{"campos":{"max_linhas":7,"espelho":true}}"#),
            &sessao,
        )
        .unwrap();
    assert!(r.booleano_ou("gravado", false));

    // (1) o arquivo no disco mudou, e o comentario continua la.
    let texto = std::fs::read_to_string(&caminho).unwrap();
    assert!(texto.contains("\"max_linhas\": 7"), "{texto}");
    assert!(texto.contains("comentario que a gravacao nao pode comer"));

    // (2) o efeito e imediato, sem reiniciar nada.
    assert_eq!(s.max_linhas(), 7, "o teto novo nao valeu a quente");
    assert!(s.espelho(), "o espelho novo nao valeu a quente");

    // (3) e a op `config` ja responde o valor vivo, e nao o de antes.
    let c = s.executar("config", &pedido("{}"), &sessao).unwrap();
    assert_eq!(c.inteiro_ou("max_linhas", 0), 7);

    // (4) nada aqui exige reinicio, entao a lista sai vazia.
    assert!(r
        .campo("exigem_reinicio")
        .and_then(Json::lista)
        .unwrap()
        .is_empty());
}

/// Campo que so vale no proximo arranque volta NOMEADO, para a tela poder
/// dizer isso ao lado dele em vez de prometer efeito que nao veio.
#[test]
fn campo_de_reinicio_volta_nomeado() {
    let (s, _, _guarda) = servidor_de_arquivo("reinicio", Cadastro::default());
    let r = s
        .executar(
            "config_gravar",
            &pedido(r#"{"campos":{"timeout_s":45}}"#),
            &Sessao::default(),
        )
        .unwrap();
    let espera: Vec<String> = r
        .campo("exigem_reinicio")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .filter_map(|j| j.texto().map(str::to_string))
        .collect();
    assert_eq!(espera, vec!["timeout_s"]);
}

/// O portao proprio da operacao. Pelo `despachar`, que e por onde o pedido
/// entra de verdade -- e o teste tem de falhar se ele sumir.
#[test]
fn operador_sem_administrar_nao_grava_a_configuracao() {
    let mut cadastro = Cadastro::default();
    cadastro.usuarios.push(operador());
    let usuario = cadastro.usuarios[0].clone();
    let (s, caminho, _guarda) = servidor_de_arquivo("portao", cadastro);
    let antes = std::fs::read_to_string(&caminho).unwrap();

    let mut sessao = Sessao {
        usuario: Some(usuario),
        ..Sessao::default()
    };
    let (_, _, r) = s.despachar(
        r#"{"op":"config_gravar","token":"t","campos":{"max_linhas":9}}"#,
        &mut sessao,
        "1.2.3.4",
    );
    let e = r.unwrap_err();
    assert!(
        format!("{e}").contains("administrar"),
        "o operador gravou a configuracao: {e}"
    );
    assert_eq!(
        std::fs::read_to_string(&caminho).unwrap(),
        antes,
        "o arquivo mudou apesar da recusa"
    );
    assert_eq!(s.max_linhas(), 1000, "aplicou a quente apesar da recusa");
}

/// O portao PROPRIO da operacao, provado onde ele e o unico que existe.
///
/// O teste de cima passa pelo `despachar`, e la o portao GERAL ja barra --
/// entao ele passa igual com a conferencia daqui de dentro removida, e nao
/// prova o que diz provar. Este chama `executar` direto, que e o caminho
/// de quem nao veio pelo soquete: sem a conferencia propria, um operador
/// sem `administrar` reescreveria o config.json por aqui.
#[test]
fn o_portao_proprio_barra_quem_chega_por_dentro() {
    let mut cadastro = Cadastro::default();
    cadastro.usuarios.push(operador());
    let usuario = cadastro.usuarios[0].clone();
    let (s, caminho, _guarda) = servidor_de_arquivo("cinto", cadastro);
    let antes = std::fs::read_to_string(&caminho).unwrap();

    let sessao = Sessao {
        usuario: Some(usuario),
        ..Sessao::default()
    };
    let e = s
        .executar(
            "config_gravar",
            &pedido(r#"{"campos":{"max_linhas":9}}"#),
            &sessao,
        )
        .unwrap_err();
    assert!(
        format!("{e}").contains("administrar"),
        "o portao proprio nao barrou: {e}"
    );
    assert_eq!(std::fs::read_to_string(&caminho).unwrap(), antes);
    assert_eq!(s.max_linhas(), 1000);
}

/// O gancho do operador EXECUTA um programa: se `alertas.gancho.*` fosse
/// editavel pela API, quem tem `administrar` executaria codigo no
/// servidor. Tentam-se todas as formas de chegar la -- campo a campo, a
/// secao inteira, e pelo `ALTER SERVER SET` (que desemboca na mesma
/// operacao) --, e o arquivo tem de sair igual.
///
/// Reponha o defeito (acrescente `("alertas.gancho.comando", ...)` aos
/// `CAMPOS_EDITAVEIS`) e a primeira tentativa grava.
#[test]
fn o_gancho_do_operador_nao_se_grava_pela_api() {
    let (s, caminho, _guarda) = servidor_de_arquivo("gancho", Cadastro::default());
    let antes = std::fs::read_to_string(&caminho).unwrap();
    let sessao = Sessao::default();
    for campos in [
        r#"{"alertas.gancho.comando":["/bin/sh","-c","id"]}"#,
        r#"{"alertas.gancho.ligado":true}"#,
        r#"{"alertas.gancho.timeout_s":5}"#,
        r#"{"alertas.gancho":{"ligado":true,"comando":["/bin/true"]}}"#,
        r#"{"alertas":{"gancho":{"ligado":true,"comando":["/bin/true"]}}}"#,
    ] {
        let e = s
            .executar(
                "config_gravar",
                &pedido(&format!(r#"{{"campos":{campos}}}"#)),
                &sessao,
            )
            .expect_err(campos);
        assert!(
            format!("{e}").contains("nao se grava pela tela"),
            "{campos}: {e}"
        );
    }
    let e = s
        .executar(
            "diretiva_gravar",
            &pedido(r#"{"escopo":"servidor","campo":"alertas.gancho.ligado","valor":true}"#),
            &sessao,
        )
        .expect_err("ALTER SERVER SET chegou ao gancho");
    assert!(format!("{e}").contains("nao se grava pela tela"), "{e}");
    assert_eq!(
        std::fs::read_to_string(&caminho).unwrap(),
        antes,
        "o arquivo mudou apesar da recusa"
    );
    assert!(!s.config.alertas.gancho.ligado);
}

/// **Pedido 249 (revisao SEC, B1a): injecao de TEXTO.** O teste de cima
/// prova que ninguem grava o campo `alertas.gancho`; este prova que
/// ninguem o grava ESCONDIDO dentro do VALOR de outro campo. Cada campo
/// Texto editavel recebe `x","alertas":{"gancho":{...}}` -- um valor que,
/// emendado cru no texto do arquivo, fecharia a string e abriria a secao
/// que executa programa. O arquivo relido tem de continuar sem `gancho`,
/// e o valor tem de ter ficado LITERAL (ou ter sido recusado): se nenhum
/// campo guardasse o texto, o teste passaria de olhos fechados.
///
/// O «cinto» de `gravar_a_arvore` (reparseia e compara) era lido, nao
/// provado; aqui ele e exercitado pelas duas rotas -- a troca cirurgica
/// (campo ja no arquivo) e a reserializacao (campo ausente).
///
/// Reponha o defeito (`escrever_texto` sem escapar a aspa) e o arquivo
/// relido ganha o `gancho`.
#[test]
fn texto_de_campo_editavel_nao_vira_gancho_no_arquivo() {
    const CARGA: &str = r#"x","alertas":{"gancho":{"ligado":true,"comando":["/bin/true"]}},"y":"z"#;
    let (s, caminho, _guarda) = servidor_de_arquivo("injecao-texto", Cadastro::default());
    let sessao = Sessao::default();
    let mut guardados = 0;
    let mut tentados = 0;
    // Duas passadas: a primeira com o campo ausente do arquivo
    // (reserializa), a segunda com ele ja la (troca cirurgica).
    for _ in 0..2 {
        for (campo, tipo, _) in crate::config::CAMPOS_EDITAVEIS {
            if !matches!(tipo, crate::config::TipoDoCampo::Texto) {
                continue;
            }
            tentados += 1;
            let corpo = Json::objeto(vec![(
                "campos",
                Json::objeto(vec![(campo, Json::texto_de(CARGA))]),
            )]);
            if s.executar("config_gravar", &corpo, &sessao).is_ok() {
                let relido = Json::analisar(&std::fs::read_to_string(&caminho).unwrap()).unwrap();
                let (secao, chave) = campo.split_once('.').unwrap_or(("", campo));
                let valor = if secao.is_empty() {
                    relido.campo(chave)
                } else {
                    relido.campo(secao).and_then(|o| o.campo(chave))
                };
                assert_eq!(
                    valor.and_then(Json::texto),
                    Some(CARGA),
                    "{campo}: o valor nao ficou literal"
                );
                guardados += 1;
            }
        }
    }
    // O cadastro tambem grava no config.json (`usuarios`), por outra
    // porta: login, nome e e-mail com a mesma carga.
    for (i, campo) in ["login", "nome", "email"].into_iter().enumerate() {
        let mut p = vec![
            ("login", Json::texto_de(format!("u{i}"))),
            ("senha", Json::texto_de("12345678")),
        ];
        p.retain(|(k, _)| *k != campo);
        p.push((campo, Json::texto_de(CARGA)));
        let _ = s.executar("usuario_criar", &Json::objeto(p), &sessao);
    }
    assert!(tentados > 0 && guardados > 0, "{guardados}/{tentados}");
    let texto = std::fs::read_to_string(&caminho).unwrap();
    let relido = Json::analisar(&texto).expect("o arquivo ficou ilegivel");
    assert!(
        relido
            .campo("alertas")
            .and_then(|a| a.campo("gancho"))
            .is_none(),
        "o texto do usuario virou a secao alertas.gancho:\n{texto}"
    );
    let c = Config::ler(&caminho).unwrap();
    assert!(!c.alertas.gancho.ligado && c.alertas.gancho.comando.is_empty());
}

/// O token e o resto dos segredos nao se gravam por aqui -- e a resposta
/// da operacao tambem nao os carrega de volta.
#[test]
fn o_segredo_nao_entra_nem_sai() {
    let (s, caminho, _guarda) = servidor_de_arquivo("segredo", Cadastro::default());
    let sessao = Sessao::default();

    let e = s
        .executar(
            "config_gravar",
            &pedido(r#"{"campos":{"token":"roubado"}}"#),
            &sessao,
        )
        .unwrap_err();
    assert!(format!("{e}").contains("nao se grava pela tela"), "{e}");
    assert!(std::fs::read_to_string(&caminho).unwrap().contains("\"t\""));

    // E a resposta de uma gravacao valida nao devolve o token em claro.
    let r = s
        .executar(
            "config_gravar",
            &pedido(r#"{"campos":{"max_linhas":11}}"#),
            &sessao,
        )
        .unwrap();
    let texto = r.escrever();
    assert!(!texto.contains("\"t\""), "o token vazou: {texto}");
    assert!(texto.contains("(oculto)"), "{texto}");
}

/// `recursos.memoria_max_mb` tem LEITOR: carregar tabela residente acima
/// do teto e recusado.
///
/// O campo estava no config.json, no MANUAL e na tela desde a 0.13.0 e
/// nenhuma linha o lia -- a mesma armadilha do `cache_paginas` sem cache.
#[test]
fn o_teto_de_memoria_residente_e_lido() {
    let dir = DirTemp::novo("mem-teto");
    let base = dir.join("dados");
    std::fs::create_dir_all(&base).unwrap();

    let com_teto = |mb: u64| {
        let c = Config {
            base: base.clone(),
            log_acessos: dir.join("acessos.log"),
            blacklist: dir.join("blacklist.json"),
            dblink: dir.join("dblink.json"),
            jobs: dir.join("jobs.json"),
            token: "t".into(),
            recursos: crate::config::Recursos {
                memoria_max_mb: mb,
                ..crate::config::Recursos::default()
            },
            ..Config::default()
        };
        Servidor::novo(c).unwrap()
    };

    // Um servidor sem teto monta a tabela e carrega -- o comportamento de
    // sempre, e o que mais importa provar.
    let s = com_teto(0);
    let sessao = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"m"}"#), &sessao)
        .unwrap();
    // A coluna larga e proposital: o menor teto que o campo aceita e 1 MB,
    // e uma tabela de inteiros nunca chega la. Com Str(1000), mil e
    // duzentas linhas passam do megabyte e a recusa pode ser provada de
    // verdade, e nao so pela conta.
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"m","tabela":"t",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"txt","tipo":"Str(1000)","obrigatoria":false}]}"#,
        ),
        &sessao,
    )
    .unwrap();
    let recheio = "x".repeat(900);
    for id in 1..=1_200 {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"m","tabela":"t","linha":{{"id":{id},"txt":"{recheio}"}}}}"#
            )),
            &sessao,
        )
        .unwrap();
    }
    s.executar(
        "memoria_carregar",
        &pedido(r#"{"database":"m","tabela":"t"}"#),
        &sessao,
    )
    .expect("sem teto tem de carregar, como sempre carregou");

    // E a consulta em memoria funciona depois de carregar -- a prova de
    // que a operacao VOLTA. Ela tomava a trava global de dados duas vezes
    // e travava a si mesma; nao havia teste que a chamasse, e pela tela a
    // chamada simplesmente nunca voltava.
    let r = s
        .executar(
            "SelectMemory",
            &pedido(r#"{"database":"m","tabela":"t"}"#),
            &sessao,
        )
        .expect("a consulta em memoria nao voltou");
    assert_eq!(r.inteiro_ou("achadas", -1), 1_200);

    // Com o teto de 1 MB a MESMA tabela nao entra, e a mensagem diz o
    // campo -- para quem levou a recusa saber onde mexer.
    let apertado = com_teto(1);
    let e = apertado
        .executar(
            "memoria_carregar",
            &pedido(r#"{"database":"m","tabela":"t"}"#),
            &sessao,
        )
        .unwrap_err();
    let texto = format!("{e}");
    assert!(texto.contains("memoria_max_mb"), "{texto}");
    assert!(texto.contains("teto de memoria"), "{texto}");

    // E a decisao em si, nos limites exatos.
    assert!(cabe_na_memoria(999_999_999, 0), "zero e sem teto");
    assert!(cabe_na_memoria(1024 * 1024, 1), "o limite exato cabe");
    assert!(
        !cabe_na_memoria(1024 * 1024 + 1, 1),
        "um byte acima nao cabe"
    );
}

/// `recursos.usuarios_max` tem LEITOR: o login recusa acima do teto de
/// logins DIFERENTES -- e nunca recusa quem ja esta dentro.
#[test]
fn o_teto_de_usuarios_simultaneos_e_lido() {
    let mut cadastro = Cadastro::default();
    cadastro.usuarios.push(operador());
    let dono = cadastro.usuarios[0].clone();
    let (s, _, _guarda) = servidor_de_arquivo("usuarios-teto", cadastro);

    // Sem teto (o padrao), nada muda: e o comportamento velho.
    assert_eq!(s.config().recursos.usuarios_max, 0);
    s.recusar_se_lotou("qualquer")
        .expect("sem teto ninguem e barrado");

    // Com teto 1 e uma conexao viva de OUTRA pessoa, o segundo login para.
    let com_teto = {
        let mut c = s.config().clone();
        c.recursos.usuarios_max = 1;
        c.caminho = None;
        Servidor::novo(c).unwrap()
    };
    if let Ok(mut l) = com_teto.ligacoes.lock() {
        let (id, _) = l.entrar("10.0.0.9", 4000, crate::agora_ms(), None);
        l.comecou(id, "ping", &dono.login, "", "", crate::agora_ms());
    }
    let e = com_teto.recusar_se_lotou("outro").unwrap_err();
    assert!(format!("{e}").contains("usuarios_max"), "{e}");
    // E quem JA esta dentro entra de novo sem gastar vaga.
    com_teto
        .recusar_se_lotou(&dono.login)
        .expect("quem ja esta dentro nao pode ser barrado");
}

/// O comportamento VELHO: um servidor onde ninguem grava pela tela se
/// comporta exatamente como antes de a operacao existir.
#[test]
fn sem_gravar_nada_o_servidor_e_o_de_antes() {
    let (s, caminho, _guarda) = servidor_de_arquivo("velho", Cadastro::default());
    let antes = std::fs::read_to_string(&caminho).unwrap();
    let c = s
        .executar("config", &pedido("{}"), &Sessao::default())
        .unwrap();
    assert_eq!(c.inteiro_ou("max_linhas", 0), 1000);
    assert!(!s.espelho());
    assert!(!s.somente_leitura());
    assert_eq!(std::fs::read_to_string(&caminho).unwrap(), antes);
}

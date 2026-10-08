use super::*;

fn servidor_com_papel(dir: &std::path::Path, papel: &str, somente_leitura: bool) -> Arc<Servidor> {
    let txt = format!(
        r#"{{"token":"t","somente_leitura":{somente_leitura},
                 "replicacao":{{"papel":"{papel}","id_servidor":"este-01",
                   "origens":[{{"nome":"primario","host":"10.0.0.7","porta":5000,"token":"t"}}]}}}}"#
    );
    let mut c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
    c.base = dir.to_path_buf();
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    Servidor::novo(c).unwrap()
}

fn dir(rotulo: &str) -> DirTemp {
    DirTemp::novo(&format!("papel-{rotulo}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// A read replica recusa escrita com o erro que APONTA o primario -- e
/// continua servindo leitura, que e a razao de ela existir.
#[test]
fn read_replica_recusa_escrita_apontando_o_primario_e_serve_leitura() {
    let d = dir("rr");
    let s = servidor_com_papel(&d, "read_replica", true);
    let sessao = Sessao::default();

    let erro = s
        .portoes_do_pedido(
            "inserir",
            &pedido(r#"{"database":"x","tabela":"t"}"#),
            &sessao,
        )
        .unwrap_err();
    // O nome era ESCRITA_NA_REPLICA quando esta frente nasceu sozinha. Na
    // integracao com o cluster os dois viraram UM erro -- para quem chama,
    // "escreveu no no errado, va para aquele" e o mesmo evento, e evento
    // so tem um codigo. O 4003 e o resto do teste continuam identicos: e a
    // GARANTIA que importa, e ela nao mudou.
    assert_eq!(erro.nome(), "REDIRECIONA");
    assert_eq!(erro.codigo(), 4003);
    let texto = erro.to_string();
    assert!(
        texto.contains("10.0.0.7:5000"),
        "sem o primario no erro: {texto}"
    );
    assert!(texto.contains("primario"), "{texto}");

    // Leitura passa pelo portao -- o papel nao barra quem so le.
    s.portoes_do_pedido(
        "varrer",
        &pedido(r#"{"database":"x","tabela":"t"}"#),
        &sessao,
    )
    .unwrap();
    std::fs::remove_dir_all(&d).unwrap();
}

/// Reserva e reserva: o spare recusa ate a leitura de cliente comum, e o
/// que passa e a administracao, o monitoramento e a propria replicacao.
#[test]
fn spare_nao_atende_cliente_nem_de_leitura() {
    let d = dir("spare");
    let s = servidor_com_papel(&d, "spare", true);
    let sessao = Sessao::default();

    for op in ["varrer", "ler", "buscar", "inserir", "sql", "juntar"] {
        let erro = s
            .portoes_do_pedido(op, &pedido(r#"{"database":"x","tabela":"t"}"#), &sessao)
            .unwrap_err();
        assert_eq!(erro.nome(), "SPARE_EM_ESPERA", "{op} passou no spare");
        assert!(
            erro.to_string().contains("spare_promover"),
            "{op}: o erro tem de ensinar a saida"
        );
    }
    for op in [
        "ping",
        "posicao",
        "replicar",
        "checksum",
        "replicacao_estado",
        "config",
    ] {
        s.portoes_do_pedido(op, &pedido("{}"), &sessao)
            .unwrap_or_else(|e| panic!("{op} devia passar no spare: {e}"));
    }
    std::fs::remove_dir_all(&d).unwrap();
}

/// `spare_promover`: o papel vira source, a escrita abre -- mesmo com o
/// `somente_leitura` do config ligado -- e o laco de replica tem como
/// perceber. E o degrau MANUAL em que a promocao automatica vai se apoiar.
#[test]
fn spare_promover_vira_primario_e_abre_a_escrita() {
    let d = dir("promo");
    let s = servidor_com_papel(&d, "spare", true);
    let sessao = Sessao::default();

    let r = s.promover_para_primario("teste: promocao manual").unwrap();
    assert_eq!(r.texto_ou("papel", ""), "source");
    assert_eq!(r.texto_ou("papel_anterior", ""), "spare");
    assert_eq!(s.papel_atual(), Papel::Source);
    assert!(
        !s.papel_atual().puxa_de_origem(),
        "o laco usa isto para parar"
    );

    // Escrita e leitura abertas, apesar do somente_leitura no config.
    s.portoes_do_pedido(
        "inserir",
        &pedido(r#"{"database":"x","tabela":"t"}"#),
        &sessao,
    )
    .unwrap();
    s.portoes_do_pedido(
        "varrer",
        &pedido(r#"{"database":"x","tabela":"t"}"#),
        &sessao,
    )
    .unwrap();

    // Promover um source ja promovido e erro, nao silencio.
    assert!(s.promover_para_primario("de novo").is_err());
    std::fs::remove_dir_all(&d).unwrap();
}

fn source_com_replicas(dir: &std::path::Path, autorizadas: &str) -> Arc<Servidor> {
    let txt = format!(
        r#"{{"token":"t",
                 "replicacao":{{"papel":"source","id_servidor":"curitiba-01",
                   "replicas_autorizadas":[{autorizadas}]}}}}"#
    );
    let mut c = Config::de_json(&Json::analisar(&txt).unwrap()).unwrap();
    c.base = dir.to_path_buf();
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    Servidor::novo(c).unwrap()
}

fn sessao_de(ip: &str) -> Sessao {
    Sessao {
        ip: ip.to_string(),
        ..Default::default()
    }
}

/// A LISTA nao barrou `op`: o pedido passou, ou a recusa veio de outro
/// portao. E o encontro de dois consertos de 17/09/2026 no mesmo
/// arquivo: `cluster_pulso` entrou em `OPS_DE_REPLICACAO` (A1) e o
/// portao 2b-bis passou a recusar `aplicar` num source ABERTO (A3), que
/// e o servidor destes testes. O que eles provam e a lista, entao para
/// `aplicar` a unica recusa admissivel e a do papel -- e ela nao pode
/// citar a lista.
fn a_lista_nao_barrou(r: Result<()>, op: &str, contexto: &str) {
    match r {
        Ok(()) => {}
        Err(e)
            if op == "aplicar"
                && e.nome() == "ACESSO_NEGADO"
                && e.to_string().contains("aplicar")
                && !e.to_string().contains("replicas_autorizadas") => {}
        Err(e) => panic!("{op} {contexto}: {e}"),
    }
}

/// **Prova real do pedido 214(c).** A bateria
/// `bancada/seguranca/porta.py` (caso 4d-ii) mediu um SOURCE em
/// `somente_leitura` recusando `inserir` e aceitando `aplicar` na mesma
/// sessao, gravando a linha inteira que o `replicar` acabara de entregar.
/// Tirando o portao 2b-bis, este teste volta a falhar na hora.
#[test]
fn aplicar_num_source_trancado_por_administracao_e_recusado() {
    for papel in ["source", "isolado"] {
        let d = dir(&format!("aplicar-{papel}"));
        let s = servidor_com_papel(&d, papel, true);
        let sessao = Sessao::default();
        let alvo = pedido(r#"{"database":"loja","tabela":"clientes"}"#);

        // CONTROLE POSITIVO: `inserir` ja recusava, e continua recusando
        // com o texto de sempre.
        let e = s.portoes_do_pedido("inserir", &alvo, &sessao).unwrap_err();
        assert_eq!(e.nome(), "ACESSO_NEGADO", "{papel}");

        let e = s.portoes_do_pedido("aplicar", &alvo, &sessao).unwrap_err();
        assert_eq!(
            e.nome(),
            "ACESSO_NEGADO",
            "{papel}: aplicar tinha de recusar"
        );
        assert!(
            e.to_string().contains("aplicar") && e.to_string().contains(papel),
            "{papel}: a recusa tem de dizer O QUE e QUAL papel: {e}"
        );

        // E o que NAO escreve continua passando: trancar o `aplicar` nao
        // pode trancar quem so pergunta ate onde o diario foi.
        for op in ["posicao", "replicar"] {
            s.portoes_do_pedido(op, &alvo, &sessao)
                .unwrap_or_else(|x| panic!("{papel}/{op} devia passar: {x}"));
        }
        std::fs::remove_dir_all(&d).unwrap();
    }
}

/// **O teste que mais importa do 214(c): o comportamento VELHO.**
///
/// Uma replica roda em `somente_leitura` POR DESENHO -- e o
/// `Config_exemplo_03.json` e o `montar.py` da bancada. Se o `aplicar`
/// passasse a recusar nela, toda replicacao por EMPURRAO pararia de um dia
/// para o outro. Os quatro papeis que existem para receber replicacao
/// continuam byte a byte como antes.
#[test]
fn replica_trancada_continua_aceitando_o_diario_do_source() {
    for papel in ["replica", "read_replica", "spare", "multi"] {
        let d = dir(&format!("aplicar-ok-{papel}"));
        let s = servidor_com_papel(&d, papel, true);
        s.portoes_do_pedido(
            "aplicar",
            &pedido(r#"{"database":"loja","tabela":"clientes"}"#),
            &Sessao::default(),
        )
        .unwrap_or_else(|e| panic!("{papel}: aplicar devia passar: {e}"));
        std::fs::remove_dir_all(&d).unwrap();
    }
}

/// O crivo passou a valer ABERTO tambem (revisao SEC de 17/09/2026, A3),
/// entao a pergunta obrigatoria e o comportamento VELHO do outro lado:
/// uma replica, read_replica, spare ou multi SEM `somente_leitura` continua
/// aceitando o diario do source -- o `multi` roda aberto por desenho, e
/// trancar o `aplicar` nele pararia o bidirecional por empurrao.
#[test]
fn replica_destrancada_continua_aceitando_o_diario_do_source() {
    for papel in ["replica", "read_replica", "spare", "multi"] {
        let d = dir(&format!("aplicar-aberto-{papel}"));
        let s = servidor_com_papel(&d, papel, false);
        s.portoes_do_pedido(
            "aplicar",
            &pedido(r#"{"database":"loja","tabela":"clientes"}"#),
            &Sessao::default(),
        )
        .unwrap_or_else(|e| panic!("{papel} aberto: aplicar devia passar: {e}"));
        std::fs::remove_dir_all(&d).unwrap();
    }
}

/// **Prova real do A3, no portao.** Source e isolado ABERTOS recusam o
/// `aplicar` -- e continuam aceitando `inserir`, que e o que um servidor
/// aberto faz, e `posicao`/`replicar`, que so leem. Repondo o
/// `&& self.somente_leitura()` no portao 2b-bis, este teste cai.
#[test]
fn aplicar_num_source_aberto_tambem_e_recusado() {
    for papel in ["source", "isolado"] {
        let d = dir(&format!("aplicar-aberto-recusa-{papel}"));
        let s = servidor_com_papel(&d, papel, false);
        let sessao = Sessao::default();
        let alvo = pedido(r#"{"database":"loja","tabela":"clientes"}"#);
        let e = s.portoes_do_pedido("aplicar", &alvo, &sessao).unwrap_err();
        assert_eq!(
            e.nome(),
            "ACESSO_NEGADO",
            "{papel}: aplicar tinha de recusar"
        );
        assert!(
            e.to_string().contains("aplicar") && e.to_string().contains(papel),
            "{papel}: a recusa tem de dizer O QUE e QUAL papel: {e}"
        );
        assert!(
            !e.to_string().contains("somente leitura"),
            "{papel}: o servidor esta ABERTO, e a recusa nao pode dizer que esta trancado: {e}"
        );
        for op in ["inserir", "posicao", "replicar"] {
            s.portoes_do_pedido(op, &alvo, &sessao)
                .unwrap_or_else(|x| panic!("{papel}/{op} devia passar: {x}"));
        }
        std::fs::remove_dir_all(&d).unwrap();
    }
}

/// **Prova real do A3, de ponta a ponta -- a petrea da integridade.**
/// Num source ABERTO com `clientes` mae e `pedidos` filha (chave conferida,
/// que e como toda chave nasce), o `excluir` normal recusa o pai com filhos
/// -- o comportamento velho -- e o `aplicar` pela rede com
/// `{"operacao":"exclusao"}` recusa tambem. Repondo o `somente_leitura`
/// no portao, o segundo GRAVA: `aplicar_evento` nao julga integridade, e a
/// linha 1 de `clientes` some com `pedidos` apontando para ela.
#[test]
fn aplicar_pela_rede_num_source_nao_mata_o_pai_com_filhos() {
    let d = dir("aplicar-pai-com-filhos");
    let s = servidor_com_papel(&d, "source", false);
    let pede = |corpo: &str| -> Result<Json> {
        let mut ses = Sessao::default();
        let (_, _, r) = s.despachar(
            &format!(r#"{{"token":"t",{corpo}}}"#),
            &mut ses,
            "192.168.50.20",
        );
        r
    };
    pede(r#""op":"criar_database","database":"b""#).unwrap();
    pede(
        r#""op":"criar_tabela","database":"b","tabela":"clientes",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    )
    .unwrap();
    pede(
        r#""op":"criar_tabela","database":"b","tabela":"pedidos",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                          {"nome":"cliente_id","tipo":"Int4"}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true},
                          {"nome":"porCliente","colunas":["cliente_id"]}],
               "chaves_estrangeiras":[{"nome":"fk_cliente","colunas":["cliente_id"],
                                       "tabela_ref":"clientes","colunas_ref":["id"]}]"#,
    )
    .unwrap();
    pede(r#""op":"inserir","database":"b","tabela":"clientes","linha":{"id":1}"#).unwrap();
    pede(r#""op":"inserir","database":"b","tabela":"pedidos","linha":{"id":1,"cliente_id":1}"#)
        .unwrap();

    // O comportamento VELHO: nunca se mata o pai que tem filhos.
    let e = pede(r#""op":"excluir","database":"b","tabela":"clientes","rowid":1"#).unwrap_err();
    assert!(
        e.to_string().contains("pedidos"),
        "o excluir normal tinha de recusar nomeando a filha: {e}"
    );

    // A porta que a declaracao da tabela nunca viu.
    let e = pede(
        r#""op":"aplicar","database":"b","tabela":"clientes",
               "eventos":[{"operacao":"exclusao","rowid":1}]"#,
    )
    .unwrap_err();
    assert_eq!(e.nome(), "ACESSO_NEGADO", "{e}");

    // E o pai continua la, com a filha apontando para ele.
    let l = pede(r#""op":"ler","database":"b","tabela":"clientes","rowid":1"#)
        .unwrap_or_else(|e| panic!("o pai foi apagado por baixo da chave estrangeira: {e}"));
    assert_eq!(l.inteiro_ou("id", 0), 1);
    std::fs::remove_dir_all(&d).unwrap();
}

/// **214(b)** -- a lista vazia continua liberando, e passa a AVISAR. O
/// aviso nao olha o papel de proposito: a bancada mediu um servidor de
/// fabrica (papel isolado) entregando o diario a quem so tinha o token.
#[test]
fn a_replicacao_aberta_se_anuncia_e_some_quando_a_lista_enche() {
    let d = dir("aviso-vazia");
    let s = source_com_replicas(&d, "");
    assert_eq!(
        s.replicacao_aberta(),
        Some(true),
        "source: a imagem liga sozinha, e o aviso tem de dizer isso"
    );
    let c = s.configuracao_json();
    let aberta = c
        .campo("replicacao_aberta")
        .expect("o campo tem de existir");
    assert!(aberta.booleano_ou("aberta", false));
    assert!(aberta.booleano_ou("com_imagem_da_linha", false));
    std::fs::remove_dir_all(&d).unwrap();

    let d = dir("aviso-cheia");
    let s = source_com_replicas(&d, r#""192.168.50.20""#);
    assert_eq!(
        s.replicacao_aberta(),
        None,
        "lista cheia: nao ha o que avisar"
    );
    assert!(
        s.configuracao_json().campo("replicacao_aberta").is_none(),
        "campo AUSENTE, e nao `false`: quem le nao pode confundir com servidor velho"
    );
    std::fs::remove_dir_all(&d).unwrap();
}

/// **O teste que mais importa aqui: o comportamento VELHO.**
///
/// Todo `config.json` que existe hoje tem `replicas_autorizadas` vazio (ou
/// nem tem o campo). Se a lista vazia passasse a significar «ninguem», a
/// replicacao de todo mundo pararia de um dia para o outro por causa de
/// uma guarda que ninguem pediu -- protecao que quebra todo cliente antigo
/// nao e protecao, e estrago. Lista vazia libera todos, e e este teste que
/// trava isso.
#[test]
fn sem_replicas_autorizadas_nada_muda() {
    let d = dir("rep-vazia");
    let s = source_com_replicas(&d, "");
    for op in OPS_DE_REPLICACAO {
        for ip in ["192.168.50.20", "10.9.9.9", ""] {
            a_lista_nao_barrou(
                s.portoes_do_pedido(op, &pedido("{}"), &sessao_de(ip)),
                op,
                &format!("de {ip:?} devia passar"),
            );
        }
    }
    std::fs::remove_dir_all(&d).unwrap();
}

/// **Prova real, com o defeito reposto.** Antes desta guarda o campo
/// existia e nao era lido: a bancada de conteiner mediu um vizinho com a
/// configuracao vazada levando os 200 de 200 eventos do diario COM a lista
/// preenchida. Tirando o portao, este teste volta a falhar na hora.
#[test]
fn replica_de_fora_da_lista_nao_le_o_diario() {
    let d = dir("rep-lista");
    let s = source_com_replicas(&d, r#""192.168.50.20""#);

    for op in OPS_DE_REPLICACAO {
        let erro = s
            .portoes_do_pedido(op, &pedido("{}"), &sessao_de("192.168.50.31"))
            .unwrap_err();
        assert_eq!(erro.nome(), "ACESSO_NEGADO", "{op}");
        assert!(
            erro.to_string().contains("replicas_autorizadas"),
            "{op}: a recusa tem de dizer QUAL lista barrou: {erro}"
        );
        // A replica da lista continua entrando -- a guarda tranca a porta,
        // nao muda a fechadura.
        a_lista_nao_barrou(
            s.portoes_do_pedido(op, &pedido("{}"), &sessao_de("192.168.50.20")),
            op,
            "da replica autorizada",
        );
    }

    // O que NAO e da replicacao nao passa a olhar a lista: quem administra
    // nao e uma replica remota, e trancar `replicacao_estado` pela lista
    // faria o campo significar duas coisas.
    for op in ["replicacao_estado", "replicacao_testar", "ping", "varrer"] {
        s.portoes_do_pedido(
            op,
            &pedido(r#"{"database":"x","tabela":"t"}"#),
            &sessao_de("192.168.50.31"),
        )
        .unwrap_or_else(|e| panic!("{op} nao e da lista de replicas: {e}"));
    }
    std::fs::remove_dir_all(&d).unwrap();
}

/// O portao passou a olhar um campo novo -- o IP da sessao --, entao a
/// pergunta obrigatoria e quem NAO tem esse campo. Job agendado, rotina
/// interna e a replicacao chamada de dentro chegam com `ip` vazio, e vazio
/// ali e a verdade e nao uma falta: elas nao vieram de fora.
#[test]
fn caminho_interno_sem_ip_nao_e_barrado_pela_lista() {
    let d = dir("rep-interno");
    let s = source_com_replicas(&d, r#""192.168.50.20""#);
    for op in OPS_DE_REPLICACAO {
        a_lista_nao_barrou(
            s.portoes_do_pedido(op, &pedido("{}"), &Sessao::default()),
            op,
            "de dentro devia passar",
        );
    }
    std::fs::remove_dir_all(&d).unwrap();
}

/// **Prova real do A1 (parte do portao), revisao SEC de 17/09/2026.** O
/// pulso do cluster entra com a credencial de replicacao e decide quem
/// manda, e a lista `replicas_autorizadas` nao o alcancava: bastava um
/// vizinho com o `config.json` de um no para pulsar epoca e posicao a
/// gosto. Tirando `cluster_pulso` de `OPS_DE_REPLICACAO`, este teste cai
/// na primeira asercao. O comportamento VELHO -- lista vazia libera todos,
/// pulso incluido -- e o `sem_replicas_autorizadas_nada_muda`, que percorre
/// a lista inteira.
#[test]
fn o_pulso_do_cluster_passa_pela_lista_de_replicas() {
    let d = dir("rep-pulso");
    let s = source_com_replicas(&d, r#""192.168.50.20""#);
    let pulso = pedido(r#"{"id":"no2","papel":"replica","epoca":1,"posicao":0}"#);
    let erro = s
        .portoes_do_pedido("cluster_pulso", &pulso, &sessao_de("192.168.50.31"))
        .unwrap_err();
    assert_eq!(erro.nome(), "ACESSO_NEGADO");
    assert!(
        erro.to_string().contains("replicas_autorizadas"),
        "a recusa tem de dizer QUAL lista barrou: {erro}"
    );
    // O no da lista continua pulsando; e de dentro (sem IP) tambem.
    s.portoes_do_pedido("cluster_pulso", &pulso, &sessao_de("192.168.50.20"))
        .unwrap_or_else(|e| panic!("o pulso do no autorizado: {e}"));
    s.portoes_do_pedido("cluster_pulso", &pulso, &Sessao::default())
        .unwrap_or_else(|e| panic!("o pulso de dentro: {e}"));
    std::fs::remove_dir_all(&d).unwrap();
}

/// O comportamento VELHO e o teste que mais importa: a replica classica
/// com `somente_leitura` continua recusando escrita com a MESMA recusa
/// generica de sempre -- nenhum cliente antigo passa a receber um erro
/// que nao conhece.
#[test]
fn replica_classica_continua_exatamente_como_era() {
    let d = dir("classica");
    let s = servidor_com_papel(&d, "replica", true);
    let sessao = Sessao::default();

    let erro = s
        .portoes_do_pedido(
            "inserir",
            &pedido(r#"{"database":"x","tabela":"t"}"#),
            &sessao,
        )
        .unwrap_err();
    assert_eq!(
        erro.nome(),
        "ACESSO_NEGADO",
        "a recusa antiga nao pode mudar"
    );
    assert!(erro.to_string().contains("somente leitura"));
    s.portoes_do_pedido(
        "varrer",
        &pedido(r#"{"database":"x","tabela":"t"}"#),
        &sessao,
    )
    .unwrap();
    std::fs::remove_dir_all(&d).unwrap();
}

/// Toda operacao da lista do spare existe de verdade no protocolo: uma
/// lista de permissao com nome fantasma e permissao que ninguem usa, e um
/// nome que sair do despachar sem sair daqui viraria promessa furada.
#[test]
fn a_lista_do_spare_so_tem_operacoes_reais() {
    for op in OPS_NO_SPARE {
        let conhecida =
            crate::catalogo::por_nome(op).is_some() || ["desafio", "login", "sair"].contains(op);
        assert!(
            conhecida,
            "{op:?} esta em OPS_NO_SPARE e nao existe no catalogo"
        );
    }
}

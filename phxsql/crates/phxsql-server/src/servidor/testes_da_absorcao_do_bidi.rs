//! Pedido 330: a primeira rodada do bidirecional depois do arranque absorvia
//! o diario local INTEIRO com a trava exclusiva na mao; e cada rodada seguinte
//! com um evento local novo caminhava o volume desde o comeco para le-lo.
use super::*;

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Um servidor MULTI com `b.c` e `n` linhas locais -- cada uma um evento
/// do diario com imagem, que e o que o mapa de toques absorve.
fn com_diario(nome: &str, n: i64) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("absorcao-{nome}"));
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.replicacao.papel = crate::config::Papel::Multi;
    c.replicacao.id_servidor = "no-da-absorcao".into();
    c.replicacao.imagem_da_linha = true;
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    for i in 1..=n {
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"c","linha":{{"id":{i}}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    (s, dir)
}

fn rodada(s: &Servidor) {
    let no = crate::replica::NoSource {
        nome: "c".into(),
        eventos: 0,
        esquema: None,
        proxima_sequencia: 0,
    };
    let origem = crate::config::Origem {
        nome: "outro".into(),
        host: "127.0.0.1".into(),
        porta: 1,
        token: String::new(),
        databases: vec!["b".into()],
        reconectar_em: 1,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
        cifra: false,
        chave_do_fio: String::new(),
        espelho: false,
    };
    let meu_hash = s.config.replicacao.numero();
    assert!(s
        .abrir_para_bidi("b", &no, &origem, meu_hash)
        .unwrap()
        .is_some());
}

/// (vistos, chaves, sob_a_exclusiva, marca)
fn mapa(s: &Servidor) -> (u64, usize, u64, Option<phxsql_store::log::MarcaDoDiario>) {
    let g = s.toques_bidi.lock().unwrap();
    let m = g.get("b/c").expect("mapa de b/c");
    (
        m.vistos,
        m.toques.len(),
        m.absorvidos_sob_a_exclusiva,
        m.marca,
    )
}

/// **Prova real, nos dois sentidos.** 3.000 eventos locais, processo
/// recem-nascido (mapa vazio): a primeira rodada absorve os 3.000, e menos
/// de UM lote passa com a trava exclusiva na mao -- o grosso foi sob a
/// compartilhada, em fatias.
///
/// O vermelho: sem a chamada a `pre_absorver_sob_leitura` em
/// `abrir_para_bidi`, os 3.000 passam pela exclusiva e a segunda asserção
/// cai. Medido em 01/10/2026.
#[test]
fn a_primeira_rodada_absorve_o_grosso_fora_da_exclusiva() {
    let (s, _d) = com_diario("primeira", 3_000);
    rodada(&s);
    let (vistos, chaves, sob, _) = mapa(&s);
    assert_eq!(
        (vistos, chaves),
        (3_000, 3_000),
        "o diario inteiro entrou no mapa"
    );
    assert!(
        sob < LOTE_PADRAO_DE_REPLICACAO,
        "{sob} evento(s) passaram pela trava EXCLUSIVA; so a cauda (< {}) devia",
        LOTE_PADRAO_DE_REPLICACAO
    );
    // E o numero sai pelo `replicacao_estado`, em vez de calado.
    let e = s
        .executar("replicacao_estado", &pedido("{}"), &Sessao::default())
        .unwrap();
    let t = e.campo("toques_no_mapa").and_then(|m| m.campo("b/c"));
    assert_eq!(
        t.map(|t| t.inteiro_ou("chaves", -1)),
        Some(3_000),
        "{}",
        e.escrever()
    );
}

/// **Pedido 623: a fatia que ja chega com o prazo vencido absorve UM
/// lote.** E o caso que a carga produz -- a fatia gasta o prazo abrindo a
/// tabela ou esperando o `toques_bidi` -- e aqui ele vem pronto, sem
/// relogio nenhum na conta: o prazo e o proprio instante da chamada.
///
/// O vermelho: com o prazo conferido no TOPO do laco (antes do lote), a
/// fatia sai com zero, a falta nao encurta e a pre-absorcao entrega o
/// resto a exclusiva -- medido sob carga, 211.060 eventos e 3,3 s de
/// escritor parado.
#[test]
fn a_fatia_com_o_prazo_ja_vencido_ainda_absorve_um_lote() {
    let (s, _d) = com_diario("prazo-vencido", 1_200);
    let meu_hash = s.config.replicacao.numero();
    let trava = s.travar_dados_para_ler().unwrap();
    let Ok(Aberta::Pronta(mut t)) = trava.abrir_diario_para_ler("b", "c") else {
        panic!("b.c nao abriu para ler o diario");
    };
    let (_, pos_chave) = bidirecional::chave_unica(t.esquema_do_diario()).unwrap();
    let falta = s
        .absorver_diario_local(
            &mut t,
            "b/c",
            &pos_chave,
            meu_hash,
            Some(Instant::now()),
            false,
        )
        .unwrap();
    drop(t);
    drop(trava);
    assert_eq!(
        (mapa(&s).0, falta),
        (LOTE_PADRAO_DE_REPLICACAO, 1_200 - LOTE_PADRAO_DE_REPLICACAO),
        "a fatia com o prazo vencido tinha de absorver um lote inteiro"
    );
}

/// **A rodada SEGUINTE, o irmao que a medicao achou.** O servidor reabre a
/// tabela a cada rodada, e a reaberta nao tem marca: ler o evento novo
/// caminhava o volume desde o comeco -- 507-517 ms num diario de 1 M
/// (`--example custo-da-absorcao-do-bidi`). A marca agora mora no mapa,
/// guardada a cada lote, e a rodada seguinte comeca dela.
///
/// O vermelho: sem `mapa.marca = tabela.marca_do_diario()`, a marca fica
/// `None` e a primeira asserção cai.
#[test]
fn a_rodada_seguinte_comeca_da_marca_guardada() {
    let (s, _d) = com_diario("seguinte", 1_200);
    rodada(&s);
    let (vistos, _, _, marca) = mapa(&s);
    let marca = marca.expect("a marca do diario nao ficou guardada no mapa");
    assert!(
        marca.evento <= vistos && vistos - marca.evento <= LOTE_PADRAO_DE_REPLICACAO,
        "a marca ({}) tem de estar a menos de um lote de vistos ({vistos})",
        marca.evento
    );
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"c","linha":{"id":99999}}"#),
        &Sessao::default(),
    )
    .unwrap();
    rodada(&s);
    let (vistos, chaves, _, _) = mapa(&s);
    assert_eq!((vistos, chaves), (1_201, 1_201), "o evento novo nao entrou");
}

/// **O comportamento VELHO**: sem nada novo no diario, a rodada nao le
/// nada e o mapa fica como estava -- por nenhuma das duas fichas.
#[test]
fn sem_evento_novo_a_rodada_nao_absorve_nada() {
    let (s, _d) = com_diario("parado", 10);
    rodada(&s);
    let antes = mapa(&s);
    rodada(&s);
    assert_eq!(mapa(&s), antes);
    assert_eq!(antes.0, 10);
}

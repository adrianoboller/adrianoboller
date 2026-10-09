//! O expurgo da trilha `.lgpd` pelo servidor (pedido 368): a op do
//! administrador e o relogio da retencao, que chamam o MESMO motor.
use super::*;

fn servidor_com(dir: &std::path::Path, retencao_anos: u32, somente_leitura: bool) -> Arc<Servidor> {
    servidor_com_protecao(dir, retencao_anos, somente_leitura, true)
}

/// `protecao` e o interruptor de teste da camada de protecao (765/767).
fn servidor_com_protecao(
    dir: &std::path::Path,
    retencao_anos: u32,
    somente_leitura: bool,
    protecao: bool,
) -> Arc<Servidor> {
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        somente_leitura,
        ..Config::default()
    };
    c.lgpd.retencao_anos = retencao_anos;
    c.protecao.ligada = protecao;
    Servidor::novo(c).unwrap()
}

fn servidor(dir: &std::path::Path, retencao_anos: u32) -> Arc<Servidor> {
    servidor_com(dir, retencao_anos, false)
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Banco `b` e a tabela `c` como o PROTOCOLO a cria -- sem
/// `registros_por_arquivo`, a tabela padrao --, com o `cpf` marcado e
/// `alteracoes` alteracoes gravadas na trilha.
fn tabela_padrao(s: &Arc<Servidor>, alteracoes: u32) {
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                               {"nome":"cpf","tipo":"Str(14)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "marcar_lgpd",
        &pedido(r#"{"database":"b","tabela":"c","colunas":{"cpf":"pessoal"}}"#),
        &dono,
    )
    .unwrap();
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"c","linha":{"id":1,"cpf":"01234567890"}}"#),
        &dono,
    )
    .unwrap();
    alterar(s, 1, alteracoes);
}

fn alterar(s: &Arc<Servidor>, de: u32, quantas: u32) {
    for i in de..de + quantas {
        s.executar(
            "atualizar",
            &pedido(&format!(
                r#"{{"database":"b","tabela":"c","rowid":1,"valores":{{"id":1,"cpf":"{i:011}"}}}}"#
            )),
            &Sessao::default(),
        )
        .unwrap();
    }
}

/// Os volumes do `.lgpd` pelo SISTEMA DE ARQUIVOS.
fn no_disco(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir.join("b"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".lgpd"))
        .collect();
    v.sort();
    v
}

fn expurgos_no_reason(s: &Arc<Servidor>) -> Vec<Json> {
    let m = s
        .executar(
            "motivos",
            &pedido(r#"{"database":"b","tabela":"c"}"#),
            &Sessao::default(),
        )
        .unwrap();
    m.campo("motivos")
        .and_then(Json::lista)
        .unwrap()
        .iter()
        .filter(|r| r.texto_ou("tipo", "") == "expurgo")
        .cloned()
        .collect()
}

fn expurgar(s: &Arc<Servidor>, extra: &str) -> Result<Json> {
    s.executar(
        "expurgar_trilha",
        &pedido(&format!(r#"{{"database":"b","tabela":"c"{extra}}}"#)),
        &Sessao::default(),
    )
}

/// **A op do administrador, de ponta a ponta, na tabela padrao.** As tres
/// recusas (sem motivo, sem limite, limite no futuro) nao tocam em nada.
/// Tres pedidos com `fechar_ativo` e um limite de 2000 fecham tres volumes
/// sem apagar nenhum (nada e de antes de 2000). O pedido com o limite de
/// agora derruba os tres e deixa o ativo -- conferido no DIRETORIO --,
/// grava o rastro no `.reason` com o bit do expurgo da trilha e sem a
/// chave de linha, e a op `trilha` continua lendo o que sobrou.
#[test]
fn o_administrador_expurga_so_volume_fechado_e_o_rastro_fica() {
    let dir = DirTemp::novo("expurgo-op");
    let s = servidor(&dir, 5);
    tabela_padrao(&s, 10);
    assert_eq!(no_disco(&dir), vec!["c.lgpd"]);

    for (extra, deve_citar) in [
        (r#","ate":"2099-01-01""#, "motivo"),
        (r#","motivo":"prazo""#, "\"ate\""),
        (r#","motivo":"prazo","ate":"2099-01-01""#, "futuro"),
    ] {
        let e = expurgar(&s, extra).unwrap_err().to_string();
        assert!(e.contains(deve_citar), "{extra}: {e}");
    }
    assert_eq!(no_disco(&dir), vec!["c.lgpd"], "uma recusa mexeu na trilha");
    assert!(expurgos_no_reason(&s).is_empty(), "recusa deixou rastro");

    for (n, esperado) in [(1u32, 1u64), (2, 2), (3, 3)] {
        let r = expurgar(
            &s,
            r#","motivo":"fechar","ate":"2000-01-01","fechar_ativo":true"#,
        )
        .unwrap();
        assert_eq!(
            r.inteiro_ou("fechou", -1),
            esperado as i64,
            "{}",
            r.escrever()
        );
        assert_eq!(
            r.campo("volumes").and_then(Json::lista).map(|l| l.len()),
            Some(0)
        );
        alterar(&s, 100 * n, 10);
    }
    assert_eq!(
        no_disco(&dir),
        vec!["c#001.lgpd", "c#002.lgpd", "c#003.lgpd", "c.lgpd"]
    );

    // +1: o ultimo registro pode ser do MESMO milissegundo, e o limite
    // derruba so o que e ANTERIOR a ele.
    let agora = crate::agora_ms() + 1;
    let r = expurgar(
        &s,
        &format!(r#","motivo":"prazo de guarda","ate_ms":{agora}"#),
    )
    .unwrap();
    let saidos = r.campo("volumes").and_then(Json::lista).unwrap();
    assert_eq!(saidos.len(), 3, "{}", r.escrever());
    assert_eq!(r.texto_ou("parada", ""), "volume_ativo");
    assert_eq!(r.inteiro_ou("parou_no_volume", -1), 4);
    assert!(
        matches!(r.campo("fechou"), Some(Json::Nulo)),
        "{}",
        r.escrever()
    );
    assert_eq!(
        no_disco(&dir),
        vec!["c.lgpd"],
        "o diretorio nao e o que a resposta diz"
    );

    let rastro = expurgos_no_reason(&s);
    assert_eq!(rastro.len(), 1, "{rastro:?}");
    assert!(
        rastro[0].booleano_ou("expurgo_da_trilha", false),
        "{:?}",
        rastro[0]
    );
    let identidade = rastro[0].texto_ou("identidade", "");
    assert!(identidade.starts_with(".lgpd volumes 1-3 "), "{identidade}");
    assert!(
        !identidade.contains("01234567890"),
        "o rastro guardou a chave que o expurgo apagou: {identidade}"
    );

    let t = s
        .executar(
            "trilha",
            &pedido(r#"{"database":"b","tabela":"c","limite":0}"#),
            &Sessao::default(),
        )
        .unwrap();
    assert_eq!(t.inteiro_ou("total", -1), r.inteiro_ou("restam", -2));
    assert_eq!(t.inteiro_ou("total", -1), 10);
}

/// Tabela sem trilha: nada acontece, e a resposta diz `sem_trilha` -- sem
/// criar arquivo nem deixar rastro.
#[test]
fn tabela_sem_trilha_responde_sem_trilha() {
    let dir = DirTemp::novo("expurgo-sem-trilha");
    let s = servidor(&dir, 5);
    let dono = Sessao::default();
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(r#"{"database":"b","tabela":"c","colunas":[{"nome":"n","tipo":"Int8"}]}"#),
        &dono,
    )
    .unwrap();
    let agora = crate::agora_ms();
    let r = expurgar(
        &s,
        &format!(r#","motivo":"x","ate_ms":{agora},"fechar_ativo":true"#),
    )
    .unwrap();
    assert_eq!(r.texto_ou("parada", ""), "sem_trilha");
    assert!(no_disco(&dir).is_empty());
    assert!(expurgos_no_reason(&s).is_empty());
}

/// **C2: a trilha e do NO.** So administrador, e no `OPS_DO_NO`: num
/// servidor somente-leitura -- o que uma replica e -- o administrador
/// expurga a trilha dele. O controle no mesmo servidor: o `restaurar`, que
/// grava dado replicado, e recusado. (Era o `esvaziar_lixeira`, ate ele
/// entrar no `OPS_DO_NO` pelo mesmo motivo -- pedido 499.)
///
/// **Defeito reposto** (`expurgar_trilha` fora do `OPS_DO_NO`): a replica
/// recusa, e o `unwrap` do expurgo cai.
#[test]
fn o_expurgo_pede_administrar_e_roda_no_servidor_somente_leitura() {
    assert_eq!(
        Atividade::da_operacao("expurgar_trilha"),
        Some(Atividade::Administrar)
    );

    let dir = DirTemp::novo("expurgo-replica");
    {
        let s = servidor(&dir, 5);
        tabela_padrao(&s, 5);
    }
    // Pelo `despachar`, que e a porta com os portoes -- o `executar` dos
    // outros testes pula o do somente-leitura.
    // `expurgar_trilha` e da lista de perigo, e a sessao de servico deste
    // teste nao tem login para liberar: a prova e do portao do
    // somente-leitura e do `administrar`, nao da camada.
    let s = servidor_com_protecao(&dir, 5, true, false);
    let mut sessao = Sessao::default();
    let (_, _, controle) = s.despachar(
        r#"{"token":"t","op":"restaurar","database":"b","tabela":"c","rowid":1}"#,
        &mut sessao,
        "127.0.0.1",
    );
    let controle = controle.unwrap_err();
    assert!(matches!(controle, PhxError::Autorizacao(_)), "{controle}");
    // +1: o ultimo registro pode ser do MESMO milissegundo, e o limite
    // derruba so o que e ANTERIOR a ele.
    let agora = crate::agora_ms() + 1;
    let (_, _, r) = s.despachar(
        &format!(
            r#"{{"token":"t","op":"expurgar_trilha","database":"b","tabela":"c",
                    "motivo":"na replica","ate_ms":{agora},"fechar_ativo":true}}"#
        ),
        &mut sessao,
        "127.0.0.1",
    );
    let r = r.unwrap();
    assert_eq!(r.inteiro_ou("fechou", -1), 1);
    assert_eq!(
        r.campo("volumes").and_then(Json::lista).map(|l| l.len()),
        Some(1)
    );
    assert_eq!(no_disco(&dir), vec!["c.lgpd"]);
    // Por ULTIMO, para o defeito reposto cair no comportamento (a replica
    // recusando) e nao nesta linha, que so repete a lista.
    assert!(!grava_dado_replicado("expurgar_trilha"));
}

/// **O relogio da retencao, na tabela PADRAO.** Com o prazo de 5 anos e a
/// trilha de agora: a passada de hoje nao fecha nem apaga nada; a de daqui
/// a dois anos FECHA o ativo por idade (o primeiro registro passou de 30
/// dias) e nao apaga -- o volume fechado ainda esta no prazo --; a de
/// daqui a seis anos derruba o volume fechado e nada fica retido. Rastro
/// assinado pelo servidor (usuario 0) com o prazo no motivo.
///
/// **Defeito reposto** (o limite sai de `agora` em vez de
/// `recuar_anos(agora, anos)`): a passada de daqui a dois anos ja apaga, e
/// a asserção dela cai.
#[test]
fn o_relogio_da_retencao_fecha_por_idade_e_so_derruba_o_que_passou_do_prazo() {
    let dir = DirTemp::novo("expurgo-relogio");
    let s = servidor(&dir, 5);
    tabela_padrao(&s, 20);
    let agora = crate::agora_ms();

    let hoje = s.expurgar_trilhas_vencidas(agora).unwrap();
    assert_eq!((hoje.fechados, hoje.volumes), (0, 0), "{}", hoje.resumo());
    assert_eq!(no_disco(&dir), vec!["c.lgpd"]);

    let dois_anos = s
        .expurgar_trilhas_vencidas(agora + 2 * 366 * 86_400_000)
        .unwrap();
    assert_eq!(
        (dois_anos.fechados, dois_anos.volumes),
        (1, 0),
        "{}",
        dois_anos.resumo()
    );
    assert_eq!(no_disco(&dir), vec!["c#001.lgpd", "c.lgpd"]);
    assert!(expurgos_no_reason(&s).is_empty());

    let seis_anos = s
        .expurgar_trilhas_vencidas(agora + 6 * 366 * 86_400_000)
        .unwrap();
    assert_eq!(seis_anos.volumes, 1, "{}", seis_anos.resumo());
    assert_eq!(seis_anos.registros, 20);
    assert_eq!(seis_anos.com_trilha, 1);
    assert!(seis_anos.retidas.is_empty(), "{:?}", seis_anos.retidas);
    assert!(seis_anos.falhas.is_empty(), "{:?}", seis_anos.falhas);
    assert_eq!(no_disco(&dir), vec!["c.lgpd"]);

    let rastro = expurgos_no_reason(&s);
    assert_eq!(rastro.len(), 1);
    assert_eq!(rastro[0].inteiro_ou("usuario", -1), 0);
    assert!(rastro[0].booleano_ou("expurgo_da_trilha", false));
    assert!(
        rastro[0]
            .texto_ou("motivo", "")
            .contains("lgpd.retencao_anos"),
        "{:?}",
        rastro[0]
    );
}

/// **Prazo zero desliga o relogio -- antes do trabalho.** Nem a passada
/// apaga, nem a thread sobe. E o controle do outro lado: com prazo, a
/// thread aparece com o nome que o teste procura, senao a primeira metade
/// passaria com qualquer nome.
///
/// **Defeito reposto** (tirar o `if anos == 0 { return; }` do
/// `subir_retencao_da_trilha`): a thread sobe com o prazo zero e a
/// asserção do meio cai.
#[test]
fn prazo_zero_desliga_o_relogio_antes_do_trabalho() {
    let dir = DirTemp::novo("expurgo-zero");
    let s = servidor(&dir, 0);
    tabela_padrao(&s, 5);
    let antes = no_disco(&dir);
    let r = s
        .expurgar_trilhas_vencidas(crate::agora_ms() + 100 * 366 * 86_400_000)
        .unwrap();
    assert_eq!((r.volumes, r.com_trilha, r.fechados), (0, 0, 0));
    assert_eq!(no_disco(&dir), antes);

    let tem_relogio = |s: &Arc<Servidor>| {
        s.telemetria
            .fios()
            .iter()
            .any(|f| f.nome == "retencao-trilha")
    };
    s.subir_retencao_da_trilha();
    assert!(!tem_relogio(&s), "o prazo zero subiu o relogio");

    let dir2 = DirTemp::novo("expurgo-cinco");
    let s2 = servidor(&dir2, 5);
    s2.subir_retencao_da_trilha();
    assert!(tem_relogio(&s2), "com prazo, o relogio devia ter subido");
}

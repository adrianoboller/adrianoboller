//! Pedidos 765/767, fatias P12 e P13, e a brecha do primeiro cadastro.
//!
//! * P12: a tabela `phxsys.protecao` -- semeada so se falta, lida pelo
//!   motor da grade -- e o job com a senha de execucao dada pelo
//!   `job_autorizar`.
//! * P13: a guarda guarda a si mesma -- baixar uma linha sem a sessao
//!   liberada recusa e a linha fica; subir nunca pede senha.
//! * 767: o primeiro cadastro da segunda senha e do administrador.
//!
//! Cada teste diz qual defeito reposto o derruba (o RED, registrado no
//! desenho §6.2 e no catalogo de guardas).

use super::*;
use crate::Cadastro;

const IP: &str = "127.0.0.1";
const LOGIN_DA_BIA: &str = "login-da-bia-77";
const LOGIN_DO_CAIO: &str = "login-do-caio-88";
const LOGIN_DA_ANA: &str = "login-da-ana-66";

fn p(t: &str) -> Json {
    Json::analisar(t).unwrap()
}

fn cadastro() -> Cadastro {
    let h = |s: &str| phxsql_core::senha::cifrar_com(s, 1);
    Cadastro::de_json(&p(&format!(
        r#"{{"usuarios":[
             {{"login":"ana","id":9,"supervisor":true,"senha_hash":"{}"}},
             {{"login":"caio","id":11,"supervisor":true,"senha_hash":"{}"}},
             {{"login":"bia","id":10,"nivel":"leitor","senha_hash":"{}"}}]}}"#,
        h(LOGIN_DA_ANA),
        h(LOGIN_DO_CAIO),
        h(LOGIN_DA_BIA)
    )))
    .unwrap()
}

fn servidor_com(nome: &str, ajuste: impl FnOnce(&mut Config)) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("guarda-{nome}"));
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        cadastro: cadastro(),
        ..Config::default()
    };
    ajuste(&mut c);
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    s.executar("criar_database", &p(r#"{"database":"b"}"#), &dono)
        .unwrap();
    criar_c(&s);
    pede(&s, &mut sessao("ana"), r#""op":"protecao_semear""#).unwrap();
    (s, dir)
}

fn servidor(nome: &str) -> (Arc<Servidor>, DirTemp) {
    servidor_com(nome, |_| {})
}

fn criar_c(s: &Servidor) {
    s.executar(
        "criar_tabela",
        &p(r#"{"database":"b","tabela":"c",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#),
        &Sessao::default(),
    )
    .unwrap();
}

fn sessao(login: &str) -> Sessao {
    Sessao {
        usuario: cadastro().por_login(login).cloned(),
        ip: IP.into(),
        ..Sessao::default()
    }
}

fn liberada(login: &str) -> Sessao {
    Sessao {
        execucao_liberada: Some(LiberacaoDeExecucao {
            login: login.into(),
            ip: IP.into(),
        }),
        ..sessao(login)
    }
}

/// Com a atividade amarrada, como a conexao faz: a ocorrencia do bloqueio
/// vai a fila DESTE servidor, e nao a de outro teste.
fn pede(s: &Arc<Servidor>, sessao: &mut Sessao, corpo: &str) -> Result<Json> {
    let agora = crate::agora_ms();
    let a = s
        .telemetria
        .entrar("dados:guarda", "dados", IP, 1, agora)
        .expect("a telemetria nasce ligada");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    a.comecou_pedido("guarda", "ana", "phxsys", "protecao", agora);
    let linha = format!(r#"{{"token":"t",{corpo}}}"#);
    let r = s.despachar(&linha, sessao, IP).2;
    a.terminou_pedido("ana");
    r
}

fn recusou(r: &Result<Json>, onde: &str) {
    match r {
        Err(e) => assert_eq!(e.codigo(), 4009, "{onde}: recusou por outro motivo: {e}"),
        Ok(j) => panic!("{onde}: executou -- {}", j.escrever()),
    }
}

const DROP: &str = r#""op":"excluir_tabela","database":"b","tabela":"c","confirmar":"c""#;

fn tabela_existe(d: &DirTemp) -> bool {
    d.join("b").join("c.reg").exists()
}

/// As linhas de `phxsys.protecao`, por op: (rowid, linha).
fn linhas(s: &Servidor) -> Vec<(u64, Json)> {
    let r = s
        .executar(
            "varrer",
            &p(r#"{"database":"phxsys","tabela":"protecao","max":1000}"#),
            &Sessao::default(),
        )
        .unwrap();
    r.campo("linhas")
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .map(|l| (l.inteiro_ou("rowid", 0) as u64, l.clone()))
        .collect()
}

fn linha_da(s: &Servidor, op: &str) -> (u64, Json) {
    linhas(s)
        .into_iter()
        .find(|(_, l)| l.texto_ou("op", "") == op)
        .unwrap_or_else(|| panic!("sem a linha {op}"))
}

fn modo_de(s: &Servidor, op: &str) -> String {
    linha_da(s, op).1.texto_ou("modo", "").to_string()
}

/// O `atualizar` da linha inteira -- o `atualizar` grava a linha toda.
fn mudar(op: &str, rowid: u64, linha: &Json, modo: &str) -> String {
    format!(
        r#""op":"atualizar","database":"phxsys","tabela":"protecao","rowid":{rowid},
           "valores":{{"id":"{}","op":"{op}","categoria":"{}","modo":"{modo}"}}"#,
        linha.texto_ou("id", ""),
        linha.texto_ou("categoria", "")
    )
}

/// Baixa a linha COM a sessao liberada -- o caminho legitimo.
fn baixar(s: &Arc<Servidor>, op: &str, modo: &str) {
    let (rowid, linha) = linha_da(s, op);
    pede(s, &mut liberada("ana"), &mudar(op, rowid, &linha, modo)).unwrap();
    assert_eq!(modo_de(s, op), modo);
}

fn trilha(s: &Servidor, recurso: &str) -> usize {
    s.diario
        .ultimas(50)
        .iter()
        .filter(|a| a.recurso == recurso)
        .count()
}

// ------------------------------------------------------------- P12: a tabela

/// O `protecao_semear` cria a tabela com a lista de perigo inteira em
/// `proteger`, e semear de novo -- pela op ou pelo arranque -- nao desfaz a
/// escolha do dono do banco.
///
/// RED: com a semeadura sobrescrevendo, a linha volta a `proteger`.
#[test]
fn semear_cria_em_proteger_e_semear_de_novo_nao_desfaz() {
    let (s, _d) = servidor("semeada");
    let todas = linhas(&s);
    assert_eq!(todas.len(), crate::protecao::FABRICA.len());
    for (op, _) in crate::protecao::FABRICA {
        assert_eq!(modo_de(&s, op), "proteger", "{op}");
    }
    baixar(&s, "excluir_visao", "observar");
    let r = pede(&s, &mut sessao("ana"), r#""op":"protecao_semear""#).unwrap();
    assert_eq!(r.inteiro_ou("semeadas", -1), 0);
    let (_, semeadas) = s.semear_o_sistema(false, true, false);
    assert_eq!(semeadas.unwrap(), 0);
    assert_eq!(modo_de(&s, "excluir_visao"), "observar");
    assert_eq!(linhas(&s).len(), crate::protecao::FABRICA.len());
}

/// O arranque nao faz nascer `phxsys` em quem nunca o pediu (o
/// comportamento velho), e COMPLETA a tabela de quem ja o tem: a linha que
/// sumiu volta, em `proteger`. O no somente-leitura nao semeia.
///
/// RED: o arranque semeando sempre cria `phxsys` no servidor limpo; sem a
/// semeadura no arranque, a linha apagada nao volta.
#[test]
fn o_arranque_completa_quem_tem_o_sistema_e_nao_cria_em_quem_nao_tem() {
    let dir = DirTemp::novo("guarda-arranque");
    let config = |so_leitura: bool| Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        cadastro: cadastro(),
        somente_leitura: so_leitura,
        ..Config::default()
    };
    let s = Servidor::novo(config(false)).unwrap();
    assert!(!dir.join("phxsys").exists(), "phxsys nasceu sem pedido");
    pede(&s, &mut sessao("ana"), r#""op":"protecao_semear""#).unwrap();
    let (rowid, _) = linha_da(&s, "excluir_fk");
    pede(
        &s,
        &mut sessao("ana"),
        &format!(r#""op":"excluir","database":"phxsys","tabela":"protecao","rowid":{rowid},"fisico":true,"motivo":"teste""#),
    )
    .unwrap();
    drop(s);
    let s = Servidor::novo(config(true)).unwrap();
    assert!(
        linhas(&s)
            .iter()
            .all(|(_, l)| l.texto_ou("op", "") != "excluir_fk"),
        "o somente-leitura semeou"
    );
    drop(s);
    let s = Servidor::novo(config(false)).unwrap();
    assert_eq!(modo_de(&s, "excluir_fk"), "proteger");
}

/// Fabrica (`modo_dispensa_a_senha` desligado): a linha so liga e desliga o
/// MONITORAMENTO. `desligado` ainda exige a senha, e o executado nao vai a
/// trilha; `proteger` vai.
///
/// RED: o `julgar` ignorando o modo deixa a linha na trilha com
/// `desligado`; a dispensa valendo sem o interruptor executa sem a senha.
#[test]
fn a_linha_desligada_tira_da_trilha_e_a_senha_continua() {
    let (s, d) = servidor("desligada");
    baixar(&s, "excluir_tabela", "desligado");
    recusou(&pede(&s, &mut sessao("ana"), DROP), "desligado sem a senha");
    assert!(tabela_existe(&d));
    let antes = trilha(&s, "protecao.executou");
    pede(&s, &mut liberada("ana"), DROP).unwrap();
    assert!(!tabela_existe(&d));
    assert_eq!(
        trilha(&s, "protecao.executou"),
        antes,
        "o desligado foi a trilha"
    );
    // O comportamento velho: proteger vai a trilha.
    criar_c(&s);
    baixar(&s, "excluir_tabela", "proteger");
    let antes = trilha(&s, "protecao.executou");
    pede(&s, &mut liberada("ana"), DROP).unwrap();
    assert_eq!(trilha(&s, "protecao.executou"), antes + 1);
}

/// Com `protecao.modo_dispensa_a_senha` (a pergunta de produto que sobe ao
/// dono): `observar` executa sem a senha e vai a trilha; `proteger`
/// continua recusando. Pelo plano da faixa (sem a trava) e pela cascata
/// (DENTRO da trava): a leitura da tabela tem de funcionar nos dois.
///
/// RED: sem o ramo da dispensa no `julgar`, o primeiro DROP recusa; com a
/// leitura dentro da trava falhando calada, a cascata recusa.
#[test]
fn com_a_dispensa_observar_executa_sem_a_senha_e_vai_a_trilha() {
    let (s, d) = servidor_com("dispensa", |c| c.protecao.modo_dispensa_a_senha = true);
    recusou(&pede(&s, &mut sessao("ana"), DROP), "proteger com dispensa");
    baixar(&s, "excluir_tabela", "observar");
    let antes = trilha(&s, "protecao.executou");
    pede(&s, &mut sessao("ana"), DROP).unwrap();
    assert!(!tabela_existe(&d));
    assert_eq!(trilha(&s, "protecao.executou"), antes + 1);

    // A cascata larga decide com a trava na mao.
    let dono = Sessao::default();
    s.executar(
        "criar_tabela",
        &p(r#"{"database":"b","tabela":"mae",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#),
        &dono,
    )
    .unwrap();
    s.executar(
        "inserir",
        &p(r#"{"database":"b","tabela":"mae","linha":{"id":1}}"#),
        &dono,
    )
    .unwrap();
    s.executar(
        "criar_tabela",
        &p(r#"{"database":"b","tabela":"filha",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                          {"nome":"mae_id","tipo":"Int4"}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true},
                          {"nome":"porMae","colunas":["mae_id"]}],
               "chaves_estrangeiras":[{"nome":"fk_mae","colunas":["mae_id"],
                   "tabela_ref":"mae","colunas_ref":["id"],
                   "ao_excluir":"restringir","ao_alterar":"cascata"}]}"#),
        &dono,
    )
    .unwrap();
    let lote: Vec<String> = (1..=1_000)
        .map(|i| format!(r#"{{"id":{i},"mae_id":1}}"#))
        .collect();
    s.executar(
        "inserir_lote",
        &p(&format!(
            r#"{{"database":"b","tabela":"filha","linhas":[{}]}}"#,
            lote.join(",")
        )),
        &dono,
    )
    .unwrap();
    let troca = r#""op":"atualizar","database":"b","tabela":"mae","rowid":1,"valores":{"id":2}"#;
    recusou(&pede(&s, &mut sessao("ana"), troca), "cascata em proteger");
    baixar(&s, "cascata", "observar");
    pede(&s, &mut sessao("ana"), troca).unwrap();
}

// ---------------------------------------------------------- P13: a guarda

/// Baixar sem a sessao liberada recusa, e a linha fica -- pelo JSON e pelo
/// SQL, que chega como `atualizar` derivado.
///
/// RED: `toque_na_guarda` devolvendo `None` (a guarda sem guarda) deixa as
/// duas passarem e a linha vira `desligado`.
#[test]
fn baixar_a_guarda_sem_a_sessao_liberada_e_recusado_e_a_linha_fica() {
    let (s, _d) = servidor("baixar");
    let (rowid, linha) = linha_da(&s, "excluir_tabela");
    for modo in ["observar", "desligado", "Desligado ", "torto"] {
        let r = pede(
            &s,
            &mut sessao("ana"),
            &mudar("excluir_tabela", rowid, &linha, modo),
        );
        if modo == "torto" {
            // Torto vale proteger: nao baixa, e passa.
            r.unwrap();
            continue;
        }
        recusou(&r, modo);
        assert_eq!(modo_de(&s, "excluir_tabela"), "proteger", "{modo}");
    }
    let sql = r#""op":"sql","database":"phxsys","texto":"UPDATE protecao SET modo = 'observar' WHERE op = 'excluir_visao'""#;
    recusou(&pede(&s, &mut sessao("ana"), sql), "pelo SQL");
    assert_eq!(modo_de(&s, "excluir_visao"), "proteger");
    // A coluna sem caixa nao escapa.
    let (rowid, linha) = linha_da(&s, "excluir_fk");
    let caixa = format!(
        r#""op":"atualizar","database":"PHXSYS","tabela":"Protecao","rowid":{rowid},
           "valores":{{"id":"{}","op":"excluir_fk","categoria":"x","MODO":"observar"}}"#,
        linha.texto_ou("id", "")
    );
    recusou(&pede(&s, &mut sessao("ana"), &caixa), "caixa");
    assert_eq!(modo_de(&s, "excluir_fk"), "proteger");
    // A recusa vira a ocorrencia do bloqueado, como todo comando da lista.
    // E com a sessao liberada, baixa.
    pede(&s, &mut liberada("ana"), sql).unwrap();
    assert_eq!(modo_de(&s, "excluir_visao"), "observar");
}

/// Subir nunca pede senha -- pelo JSON, pelo SQL e apagando a linha (que
/// volta a fabrica). O comportamento velho: escrita em OUTRA tabela de
/// sistema nao pede senha nenhuma.
///
/// RED: `Toque::Sobe` tratado como `PodeBaixar` recusa as tres.
#[test]
fn subir_a_guarda_nunca_pede_senha() {
    let (s, _d) = servidor("subir");
    baixar(&s, "excluir_tabela", "desligado");
    baixar(&s, "excluir_visao", "observar");
    baixar(&s, "excluir_fk", "observar");
    let antes = trilha(&s, "protecao.executou");
    let (rowid, linha) = linha_da(&s, "excluir_tabela");
    pede(
        &s,
        &mut sessao("ana"),
        &mudar("excluir_tabela", rowid, &linha, "proteger"),
    )
    .unwrap();
    assert_eq!(modo_de(&s, "excluir_tabela"), "proteger");
    let sql = r#""op":"sql","database":"phxsys","texto":"UPDATE protecao SET modo = 'proteger' WHERE op = 'excluir_visao'""#;
    pede(&s, &mut sessao("ana"), sql).unwrap();
    assert_eq!(modo_de(&s, "excluir_visao"), "proteger");
    let (rowid, _) = linha_da(&s, "excluir_fk");
    pede(
        &s,
        &mut sessao("ana"),
        &format!(r#""op":"excluir","database":"phxsys","tabela":"protecao","rowid":{rowid}"#),
    )
    .unwrap();
    assert!(linhas(&s)
        .iter()
        .all(|(_, l)| l.texto_ou("op", "") != "excluir_fk"));
    // Subir vai a trilha: a guarda e monitorada sempre.
    assert_eq!(trilha(&s, "protecao.executou"), antes + 3);
    // Outra tabela do sistema: sem senha, como antes.
    s.executar(
        "criar_tabela",
        &p(r#"{"database":"phxsys","tabela":"outra",
               "colunas":[{"nome":"modo","tipo":"Str(12)"}]}"#),
        &Sessao::default(),
    )
    .unwrap();
    pede(
        &s,
        &mut sessao("ana"),
        r#""op":"inserir","database":"phxsys","tabela":"outra","valores":{"modo":"desligado"}"#,
    )
    .unwrap();
}

/// Os caminhos que a camada nao le pedem a senha mesmo para subir: o lote,
/// e copiar uma tabela PARA o lugar da guarda.
///
/// RED: tirar o `restaurar_backup`/`destino` da conferencia deixa a copia
/// passar.
#[test]
fn os_caminhos_que_nao_se_leem_pedem_a_senha() {
    let (s, _d) = servidor("opacos");
    recusou(
        &pede(
            &s,
            &mut sessao("ana"),
            r#""op":"inserir_lote","database":"phxsys","tabela":"protecao","linhas":[{"op":"x","modo":"proteger"}]"#,
        ),
        "lote",
    );
    recusou(
        &pede(
            &s,
            &mut sessao("ana"),
            r#""op":"copiar_tabela","database":"b","tabela":"protecao","destino_database":"phxsys""#,
        ),
        "copia para o sistema",
    );
    // Upsert com o SET baixando: o `valores` em proteger nao basta.
    let (_, linha) = linha_da(&s, "excluir_tabela");
    let upsert = format!(
        r#""op":"inserir","database":"phxsys","tabela":"protecao",
           "valores":{{"id":"{}","op":"excluir_tabela","modo":"proteger"}},
           "se_existir":"atualizar","atualizar":{{"modo":"desligado"}}"#,
        linha.texto_ou("id", "")
    );
    recusou(&pede(&s, &mut sessao("ana"), &upsert), "upsert");
    assert_eq!(modo_de(&s, "excluir_tabela"), "proteger");
}

/// Hipotese que morreu medida: o `aplicar` NAO e porta dos fundos da
/// guarda. Ele fica fora da conferencia (P5, a replica isenta) porque na
/// origem e no isolado o papel ja o recusa (4001) -- e a linha fica.
#[test]
fn o_aplicar_na_origem_ja_recusa_pelo_papel() {
    let (s, _d) = servidor("aplicar");
    let (rowid, _) = linha_da(&s, "excluir_tabela");
    let evento = format!(
        r#""op":"aplicar","database":"phxsys","tabela":"protecao",
           "eventos":[{{"operacao":"exclusao","rowid":{rowid}}}]"#
    );
    let e = pede(&s, &mut sessao("ana"), &evento).unwrap_err();
    assert_eq!(e.codigo(), 4001, "{e}");
    assert_eq!(linha_da(&s, "excluir_tabela").0, rowid);
}

// ------------------------------------------------------------- P12: o job

fn salvar_job(s: &Arc<Servidor>, nome: &str, pedido: &str, extra: &str) -> Result<Json> {
    pede(
        s,
        &mut sessao("ana"),
        &format!(
            r#""op":"job_salvar","job":{{"nome":"{nome}","usuario":"ana","cada_minutos":60,{extra}"pedido":{{{pedido}}}}}"#
        ),
    )
}

fn rodar(s: &Arc<Servidor>, nome: &str) -> Json {
    pede(
        s,
        &mut sessao("ana"),
        &format!(r#""op":"job_rodar","nome":"{nome}""#),
    )
    .unwrap()
}

/// O job com DROP recusa sem autorizacao; `job_autorizar` sem a sessao
/// liberada recusa; com ela, o job roda o DROP e gasta o uso -- a segunda
/// corrida volta a recusar. A autorizacao vai a trilha.
///
/// RED: sem a liberacao no `executar_job`, a corrida autorizada recusa; sem
/// o gasto do uso, a segunda corrida apaga de novo.
#[test]
fn o_job_autorizado_roda_o_drop_e_gasta_o_uso() {
    let (s, d) = servidor("job");
    salvar_job(&s, "limpa", DROP, "").unwrap();
    assert!(!rodar(&s, "limpa").booleano_ou("ok", true));
    assert!(tabela_existe(&d));
    let autorizar = r#""op":"job_autorizar","nome":"limpa","usos":1,"dias":1"#;
    recusou(
        &pede(&s, &mut sessao("ana"), autorizar),
        "autorizar sem liberar",
    );
    assert!(!rodar(&s, "limpa").booleano_ou("ok", true));
    let r = pede(&s, &mut liberada("ana"), autorizar).unwrap();
    assert_eq!(r.inteiro_ou("usos", 0), 1);
    assert_eq!(trilha(&s, "protecao.autorizou_job"), 1);
    let r = rodar(&s, "limpa");
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());
    assert!(!tabela_existe(&d));
    criar_c(&s);
    assert!(
        !rodar(&s, "limpa").booleano_ou("ok", true),
        "o uso nao gastou"
    );
    assert!(tabela_existe(&d));
    // O teto.
    let r = pede(
        &s,
        &mut liberada("ana"),
        r#""op":"job_autorizar","nome":"limpa","usos":367"#,
    );
    assert_eq!(r.unwrap_err().codigo(), 2001);
    // Quem nao administra nao autoriza, nem liberado.
    let r = pede(&s, &mut liberada("bia"), autorizar);
    assert!(r.is_err());
}

/// O escopo e o pedido: regravar o job com OUTRO pedido apaga a
/// autorizacao; regravar o MESMO (outra descricao) a mantem. E ela nunca
/// chega pela rede, nem com a impressao certa.
///
/// RED: o `salvar` herdando sempre deixa o pedido novo rodar; herdando
/// nunca, a regravacao da descricao a perde; o `de_json` aceitando o campo
/// deixa a forjada rodar.
#[test]
fn o_escopo_da_autorizacao_e_o_pedido_e_ela_nao_chega_pela_rede() {
    let (s, d) = servidor("job-escopo");
    salvar_job(&s, "limpa", DROP, "").unwrap();
    pede(
        &s,
        &mut liberada("ana"),
        r#""op":"job_autorizar","nome":"limpa","usos":5"#,
    )
    .unwrap();
    // A mesma ficha, outra descricao: fica.
    salvar_job(&s, "limpa", DROP, r#""descricao":"outra","#).unwrap();
    // Outro pedido: morre.
    s.executar(
        "criar_tabela",
        &p(r#"{"database":"b","tabela":"z",
               "colunas":[{"nome":"id","tipo":"Int4"}]}"#),
        &Sessao::default(),
    )
    .unwrap();
    let outro = r#""op":"excluir_tabela","database":"b","tabela":"z","confirmar":"z""#;
    salvar_job(&s, "limpa", outro, "").unwrap();
    assert!(
        !rodar(&s, "limpa").booleano_ou("ok", true),
        "herdou o escopo"
    );
    assert!(d.join("b").join("z.reg").exists());
    // Volta ao pedido autorizado: a autorizacao ja morreu.
    salvar_job(&s, "limpa", DROP, "").unwrap();
    assert!(!rodar(&s, "limpa").booleano_ou("ok", true));
    // De novo, e a descricao mantem.
    pede(
        &s,
        &mut liberada("ana"),
        r#""op":"job_autorizar","nome":"limpa","usos":5"#,
    )
    .unwrap();
    salvar_job(&s, "limpa", DROP, r#""descricao":"de novo","#).unwrap();
    assert!(
        rodar(&s, "limpa").booleano_ou("ok", false),
        "perdeu ao regravar"
    );
    assert!(!tabela_existe(&d));

    // Forjada pela rede, com a impressao CERTA.
    criar_c(&s);
    let impressao = s
        .jobs
        .tomar("jobs")
        .unwrap()
        .achar("limpa")
        .unwrap()
        .impressao();
    let forjada = format!(
        r#""autorizacao":{{"por":"ana","quando_ms":0,"ate_ms":9999999999999,"usos":9,"impressao":"{impressao}"}},"#
    );
    salvar_job(&s, "forja", DROP, &forjada).unwrap();
    assert!(
        !rodar(&s, "forja").booleano_ou("ok", true),
        "a forjada rodou"
    );
    assert!(tabela_existe(&d));
    // Revogar nao pede senha.
    let r = pede(
        &s,
        &mut sessao("ana"),
        r#""op":"job_autorizar","nome":"limpa","revogar":true"#,
    )
    .unwrap();
    assert!(r.booleano_ou("revogada", false));
    assert!(!rodar(&s, "limpa").booleano_ou("ok", true));
}

// ------------------------------------------- 767: o primeiro cadastro

fn cadastrar_a_propria(s: &Arc<Servidor>, login: &str, senha_login: &str) -> Result<Json> {
    pede(
        s,
        &mut sessao(login),
        &format!(
            r#""op":"senha_execucao_definir","senha":"{senha_login}","nova_senha_execucao":"execucao-de-{login}-1""#
        ),
    )
}

/// Quem so tem a senha de login de alguem nao cadastra a segunda: o
/// primeiro cadastro de quem nao administra e do administrador liberado.
/// E o segundo administrador tambem ja nao se cadastra sozinho.
///
/// RED: sem a conferencia, a bia (e quem roubou a senha dela) cadastra.
#[test]
fn o_primeiro_cadastro_e_do_administrador() {
    let (s, _d) = servidor("primeiro");
    let r = cadastrar_a_propria(&s, "bia", LOGIN_DA_BIA);
    assert_eq!(r.unwrap_err().codigo(), 4001);
    assert!(!s.senhas_de_execucao.tem("bia").unwrap());
    // A recusa vem antes da senha de login: a errada recebe a MESMA
    // resposta, e nao vira oraculo.
    let errada = cadastrar_a_propria(&s, "bia", "nao-e-a-senha").unwrap_err();
    assert!(
        errada
            .to_string()
            .contains("o primeiro cadastro da senha de execucao e do administrador"),
        "{errada}"
    );
    // O primeiro administrador do servidor se cadastra sozinho...
    cadastrar_a_propria(&s, "ana", LOGIN_DA_ANA).unwrap();
    // ...e o segundo, depois dele, ja nao.
    let r = cadastrar_a_propria(&s, "caio", LOGIN_DO_CAIO);
    assert_eq!(r.unwrap_err().codigo(), 4001);
    assert!(!s.senhas_de_execucao.tem("caio").unwrap());
    // O administrador liberado cadastra a da bia.
    pede(
        &s,
        &mut liberada("ana"),
        r#""op":"senha_execucao_definir","login":"bia","nova_senha_execucao":"provisoria-da-bia""#,
    )
    .unwrap();
    assert!(s.senhas_de_execucao.tem("bia").unwrap());
}

/// O comportamento da P14, atras do interruptor: com
/// `primeiro_cadastro_pelo_administrador: false`, cada um cadastra a sua
/// com a senha de login.
#[test]
fn sem_o_interruptor_cada_um_cadastra_a_sua() {
    let (s, _d) = servidor_com("primeiro-velho", |c| {
        c.protecao.primeiro_cadastro_pelo_administrador = false
    });
    cadastrar_a_propria(&s, "bia", LOGIN_DA_BIA).unwrap();
    assert!(s.senhas_de_execucao.tem("bia").unwrap());
    let r = cadastrar_a_propria(&s, "caio", "nao-e-a-senha");
    assert_eq!(r.unwrap_err().codigo(), 4001);
}

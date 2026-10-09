//! Pedidos 765/767, fatia P1: a camada unica de protecao, pelo `despachar`.
//!
//! O RED nos dois sentidos: cada caminho recusa com a camada ligada e
//! EXECUTA com ela desligada -- a mesma corrida, o mesmo pedido, so o
//! interruptor de teste mudando. Se a recusa viesse de outro portao, a
//! corrida desligada tambem recusaria. E o comportamento velho: o comando
//! comum responde byte a byte igual com a camada ligada e desligada.
//! O REST, que precisa da porta HTTP de verdade, esta em
//! `tests/protecao-pelo-rest.rs`.

use super::*;
use crate::Cadastro;

const IP: &str = "127.0.0.1";

fn p(t: &str) -> Json {
    Json::analisar(t).unwrap()
}

fn cadastro() -> Cadastro {
    Cadastro::de_json(&p(r#"{"usuarios":[{"login":"ana","id":9,"supervisor":true,
             "senha_hash":"pbkdf2-sha256$1000$00$00"},
             {"login":"bia","id":10,"nivel":"leitor",
             "senha_hash":"pbkdf2-sha256$1000$00$00"}]}"#))
    .unwrap()
}

/// `ligada` e o interruptor `protecao.ligada`; desligado so aqui, na corrida
/// que prova que a recusa vem da camada.
fn servidor(nome: &str, ligada: bool) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("protecao-{nome}"));
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
    c.protecao.ligada = ligada;
    let s = Servidor::novo(c).unwrap();
    popular(&s, 3);
    (s, dir)
}

fn popular(s: &Servidor, linhas: u32) {
    let dono = Sessao::default();
    s.executar("criar_database", &p(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &p(r#"{"database":"b","tabela":"c",
               "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                          {"nome":"nome","tipo":"Str(20)"}],
               "indices":[{"nome":"porId","colunas":["id"],"unico":true,
                           "primario":true}]}"#),
        &dono,
    )
    .unwrap();
    let lote: Vec<String> = (1..=linhas)
        .map(|i| format!(r#"{{"id":{i},"nome":"n{i}"}}"#))
        .collect();
    s.executar(
        "inserir_lote",
        &p(&format!(
            r#"{{"database":"b","tabela":"c","linhas":[{}]}}"#,
            lote.join(",")
        )),
        &dono,
    )
    .unwrap();
    s.executar(
        "criar_visao",
        &p(r#"{"database":"b","nome":"v","sql":"SELECT id FROM c"}"#),
        &dono,
    )
    .unwrap();
}

fn sessao_da_ana() -> Sessao {
    Sessao {
        usuario: cadastro().por_login("ana").cloned(),
        ip: IP.into(),
        ..Sessao::default()
    }
}

/// A sessao liberada a mao -- o ponto que a P14 vai preencher.
fn liberada() -> Sessao {
    Sessao {
        execucao_liberada: Some(LiberacaoDeExecucao {
            login: "ana".into(),
            ip: IP.into(),
        }),
        ..sessao_da_ana()
    }
}

/// Sempre com a atividade amarrada, como a conexao faz: sem ela, a
/// ocorrencia do bloqueio (e a do plano largo) iria a camada do PROCESSO, e
/// cairia na fila de outro teste que roda em paralelo.
fn pede(s: &Arc<Servidor>, sessao: &mut Sessao, corpo: &str) -> Result<Json> {
    pede_amarrado(s, sessao, corpo).0
}

fn sql(texto: &str) -> String {
    format!(r#""op":"sql","database":"b","texto":"{texto}""#)
}

const DROP_TABELA: &str = r#""op":"excluir_tabela","database":"b","tabela":"c","confirmar":"c""#;

fn recusou(r: &Result<Json>, onde: &str) {
    match r {
        Err(e) => assert_eq!(e.codigo(), 4009, "{onde}: recusou por outro motivo: {e}"),
        Ok(j) => panic!("{onde}: executou -- {}", j.escrever()),
    }
}

fn tabela_existe(d: &DirTemp) -> bool {
    d.join("b").join("c.reg").exists()
}

/// Os caminhos do servidor com a camada LIGADA, um teste por caminho para
/// que tirar a camada derrube cada um por si: a rede, a op `sql` (com o
/// `DROP USER`, que chamava o `executar` direto) e o job.
#[test]
fn o_drop_e_recusado_pela_rede() {
    let (s, d) = servidor("ligada", true);
    let mut ana = sessao_da_ana();
    let r = pede(&s, &mut ana, DROP_TABELA);
    recusou(&r, "rede");
    assert!(tabela_existe(&d), "a tabela sumiu apesar da recusa");
    assert!(r
        .unwrap_err()
        .to_string()
        .contains("excluir_tabela em b.c (destruir_estrutura)"));
}

/// A op `sql`: o DROP VIEW vira `excluir_visao` e cai igual. E o `DROP
/// USER`, o caminho que chamava o `executar` sem os irmaos.
#[test]
fn o_drop_e_recusado_pela_op_sql() {
    let (s, _d) = servidor("ligada-sql", true);
    let mut ana = sessao_da_ana();
    recusou(&pede(&s, &mut ana, &sql("DROP VIEW v")), "sql DROP VIEW");
    assert!(
        pede(&s, &mut ana, &sql("SELECT id FROM v")).is_ok(),
        "a visao sumiu"
    );

    // O `DROP USER` pelo SQL -- o caminho que pulava os irmaos.
    recusou(&pede(&s, &mut ana, &sql("DROP USER ana")), "sql DROP USER");
}

/// O job, que roda sob o usuario dele pelo terceiro irmao.
#[test]
fn o_drop_e_recusado_pelo_job() {
    let (s, d) = servidor("ligada-job", true);
    let mut ana = sessao_da_ana();
    let r = pede(
        &s,
        &mut ana,
        &format!(
            r#""op":"job_salvar","job":{{"nome":"limpa","usuario":"ana","cada_minutos":60,"pedido":{{{DROP_TABELA}}}}}"#
        ),
    );
    assert!(r.is_ok(), "{r:?}");
    let r = pede(&s, &mut ana, r#""op":"job_rodar","nome":"limpa""#).unwrap();
    assert!(
        !r.booleano_ou("ok", true),
        "o job rodou o DROP: {}",
        r.escrever()
    );
    assert!(
        r.escrever().contains("senha de execucao"),
        "{}",
        r.escrever()
    );
    assert!(tabela_existe(&d), "a tabela sumiu pelo job");
}

/// O RED: o MESMO roteiro com a camada desligada executa -- a recusa de
/// cima vem dela, e de nenhum outro portao.
#[test]
fn sem_a_camada_os_mesmos_caminhos_executam() {
    let (s, d) = servidor("desligada", false);
    let mut ana = sessao_da_ana();
    assert!(pede(&s, &mut ana, &sql("DROP VIEW v")).is_ok());
    // O DROP USER chega ao cadastro, que recusa por OUTRO motivo (este
    // servidor nao subiu de arquivo): o que importa e que nao e mais 4009.
    let r = pede(&s, &mut ana, &sql("DROP USER ana"));
    assert_ne!(r.as_ref().err().map(PhxError::codigo), Some(4009), "{r:?}");
    let r = pede(
        &s,
        &mut ana,
        &format!(
            r#""op":"job_salvar","job":{{"nome":"limpa","usuario":"ana","cada_minutos":60,"pedido":{{{DROP_TABELA}}}}}"#
        ),
    );
    assert!(r.is_ok(), "{r:?}");
    let r = pede(&s, &mut ana, r#""op":"job_rodar","nome":"limpa""#).unwrap();
    assert!(r.booleano_ou("ok", false), "{}", r.escrever());
    assert!(!tabela_existe(&d), "o job nao apagou");

    let (s, d) = servidor("desligada-rede", false);
    assert!(pede(&s, &mut ana, DROP_TABELA).is_ok());
    assert!(!tabela_existe(&d));
}

/// O DELETE em massa pelo SQL: o plano largo da F8 recusa com NADA
/// apagado; abaixo do piso, o mesmo DELETE passa.
#[test]
fn o_delete_largo_e_recusado_antes_da_primeira_linha() {
    let (s, _d) = servidor("delete-largo", true);
    let dono = Sessao::default();
    let lote: Vec<String> = (100..1_200)
        .map(|i| format!(r#"{{"id":{i},"nome":"m"}}"#))
        .collect();
    s.executar(
        "inserir_lote",
        &p(&format!(
            r#"{{"database":"b","tabela":"c","linhas":[{}]}}"#,
            lote.join(",")
        )),
        &dono,
    )
    .unwrap();
    let mut ana = sessao_da_ana();
    let r = pede(&s, &mut ana, &sql("DELETE FROM c WHERE id > 0"));
    recusou(&r, "DELETE largo");
    assert!(r.unwrap_err().to_string().contains("excluir_por_faixa"));
    // A faixa apaga em ordem de rowid: a primeira e a ultima continuam vivas.
    for rowid in [1, 1_103] {
        let lida = s
            .executar(
                "ler",
                &p(&format!(
                    r#"{{"database":"b","tabela":"c","rowid":{rowid}}}"#
                )),
                &dono,
            )
            .unwrap();
        assert!(
            !matches!(lida, Json::Nulo),
            "apagou o rowid {rowid} antes de recusar"
        );
    }
    // Pequeno: tres linhas de 1.103 nao sao plano largo.
    let r = pede(&s, &mut ana, &sql("DELETE FROM c WHERE id < 4"));
    assert!(r.is_ok(), "{r:?}");
    // E com a sessao liberada, o largo passa.
    let mut lib = liberada();
    assert!(pede(&s, &mut lib, &sql("UPDATE c SET nome = 'z' WHERE id > 0")).is_ok());
}

/// ALTER em tabela pequena passa; em tabela grande pede a senha.
#[test]
fn acrescentar_coluna_so_e_perigoso_em_tabela_grande() {
    let (s, _d) = servidor("alter", true);
    let mut ana = sessao_da_ana();
    let alter = r#""op":"acrescentar_coluna","database":"b","tabela":"c","coluna":{"nome":"x","tipo":"Int4"}"#;
    assert!(
        pede(&s, &mut ana, alter).is_ok(),
        "3 linhas nao e tabela grande"
    );
    let lote: Vec<String> = (100..1_200)
        .map(|i| format!(r#"{{"id":{i},"nome":"m"}}"#))
        .collect();
    s.executar(
        "inserir_lote",
        &p(&format!(
            r#"{{"database":"b","tabela":"c","linhas":[{}]}}"#,
            lote.join(",")
        )),
        &Sessao::default(),
    )
    .unwrap();
    let alter2 = r#""op":"acrescentar_coluna","database":"b","tabela":"c","coluna":{"nome":"y","tipo":"Int4"}"#;
    recusou(&pede(&s, &mut ana, alter2), "ALTER em tabela grande");
}

/// A precisao do dono: a senha de execucao libera a SESSAO -- e so ela, so
/// do mesmo IP.
#[test]
fn a_sessao_liberada_executa_e_outra_nao() {
    let (s, d) = servidor("liberada", true);
    // Mesmo login, outro IP: nao vale.
    let mut outro_ip = Sessao {
        ip: "10.0.0.9".into(),
        ..liberada()
    };
    recusou(&pede(&s, &mut outro_ip, DROP_TABELA), "outro IP");
    // Uma sessao nova da mesma pessoa: nao herda.
    recusou(&pede(&s, &mut sessao_da_ana(), DROP_TABELA), "outra sessao");
    assert!(tabela_existe(&d));
    let mut lib = liberada();
    assert!(pede(&s, &mut lib, DROP_TABELA).is_ok());
    assert!(!tabela_existe(&d));
}

/// O `sudo -k`: liberada, tranca, e o DROP seguinte recusa. Pelo protocolo
/// e pelo SQL (`LOCK EXECUTION`) -- uma op so.
#[test]
fn trancar_volta_a_sessao_para_nao_liberada() {
    for (rotulo, trancar) in [
        ("op", r#""op":"trancar_execucao""#.to_string()),
        ("sql", sql("LOCK EXECUTION")),
    ] {
        let (s, d) = servidor(&format!("trancar-{rotulo}"), true);
        let mut lib = liberada();
        assert!(lib.execucao_liberada());
        let r = pede(&s, &mut lib, &trancar).unwrap();
        assert!(r.booleano_ou("estava_liberada", false), "{}", r.escrever());
        assert!(!lib.execucao_liberada(), "{rotulo}: nao trancou");
        recusou(&pede(&s, &mut lib, DROP_TABELA), rotulo);
        assert!(tabela_existe(&d));
        // Idempotente, e sem senha nenhuma.
        let r = pede(&s, &mut lib, &trancar).unwrap();
        assert!(!r.booleano_ou("estava_liberada", true));
    }
}

/// O RED do trancar: sem ele, a sessao liberada executa o DROP.
#[test]
fn sem_trancar_a_sessao_liberada_executa() {
    let (s, d) = servidor("sem-trancar", true);
    let mut lib = liberada();
    assert!(pede(&s, &mut lib, r#""op":"ping""#).is_ok());
    assert!(pede(&s, &mut lib, DROP_TABELA).is_ok());
    assert!(!tabela_existe(&d));
}

/// `sair` e um `login` novo tiram a liberacao.
#[test]
fn sair_tira_a_liberacao() {
    let (s, _d) = servidor("sair", true);
    let mut lib = liberada();
    assert!(pede(&s, &mut lib, r#""op":"sair""#).is_ok());
    assert!(lib.execucao_liberada.is_none());
}

/// O comportamento VELHO: o comando comum responde byte a byte igual com a
/// camada ligada e desligada.
#[test]
fn o_comando_comum_responde_byte_a_byte_igual() {
    let roteiro = [
        r#""op":"ping""#.to_string(),
        r#""op":"inserir","database":"b","tabela":"c","linha":{"id":50,"nome":"x"}"#.into(),
        r#""op":"ler","database":"b","tabela":"c","rowid":1"#.into(),
        r#""op":"varrer","database":"b","tabela":"c""#.into(),
        r#""op":"buscar","database":"b","tabela":"c","indice":"porId","chave":[2]"#.into(),
        r#""op":"atualizar","database":"b","tabela":"c","rowid":2,"linha":{"id":2,"nome":"y"}"#
            .into(),
        r#""op":"excluir","database":"b","tabela":"c","rowid":3"#.into(),
        sql("SELECT id, nome FROM c WHERE id = 1"),
        sql("UPDATE c SET nome = 'k' WHERE id = 1"),
        sql("DELETE FROM c WHERE id = 50"),
        r#""op":"esquema","database":"b","tabela":"c""#.into(),
        r#""op":"usuario_criar","login":"bia","senha":"segredo-123","nivel":"leitor""#.into(),
    ];
    let correr = |ligada: bool| -> Vec<String> {
        let (s, _d) = servidor(&format!("byte-{ligada}"), ligada);
        let mut ana = sessao_da_ana();
        roteiro
            .iter()
            .map(|c| match pede(&s, &mut ana, c) {
                Ok(j) => sem_relogio(&j).escrever(),
                Err(e) => format!("ERRO {}", e),
            })
            .collect()
    };
    let ligada = correr(true);
    let desligada = correr(false);
    for (i, (a, b)) in ligada.iter().zip(&desligada).enumerate() {
        assert_eq!(
            a, b,
            "o passo {i} ({}) mudou com a camada ligada",
            roteiro[i]
        );
    }
}

/// O que muda de uma corrida para a outra sem ter nada a ver com a camada:
/// o relogio, o contador de linha do processo (`rowstamp`, que anda com
/// TODA tabela que este processo de teste ja gravou) e o caminho do
/// diretorio temporario.
fn sem_relogio(j: &Json) -> Json {
    match j {
        Json::Objeto(pares) => Json::Objeto(
            pares
                .iter()
                .filter(|(k, _)| {
                    !matches!(
                        k.as_str(),
                        "rowtime"
                            | "rowstamp"
                            | "ms"
                            | "quando_ms"
                            | "arquivo"
                            | "desde"
                            | "no_ar_s"
                    )
                })
                // O `id` de coluna e um UUID sorteado na criacao da tabela.
                .filter(|(k, v)| !(k == "id" && v.texto().is_some_and(|t| t.len() == 36)))
                .map(|(k, v)| (k.clone(), sem_relogio(v)))
                .collect(),
        ),
        Json::Lista(l) => Json::Lista(l.iter().map(sem_relogio).collect()),
        outro => outro.clone(),
    }
}

/// O pedido como a conexao o faz: a atividade amarrada, para a ocorrencia
/// chegar a camada DESTE servidor e o bit a bolha da tarefa.
fn pede_amarrado(s: &Arc<Servidor>, sessao: &mut Sessao, corpo: &str) -> (Result<Json>, u32) {
    let agora = crate::agora_ms();
    let a = s
        .telemetria
        .entrar("dados:protecao", "dados", IP, 1, agora)
        .expect("a telemetria nasce ligada");
    let _amarrada = crate::telemetria::amarrar(Some(Arc::clone(&a)));
    a.comecou_pedido("excluir_tabela", "ana", "b", "c", agora);
    let linha = format!(r#"{{"token":"t",{corpo}}}"#);
    let r = s.despachar(&linha, sessao, IP).2;
    let mascara = a.alarmes();
    a.terminou_pedido("ana");
    (r, mascara)
}

fn bloqueios(s: &Servidor) -> Vec<crate::ocorrencias::Ocorrencia> {
    use crate::ocorrencias::Carta;
    s.ocorrencias
        .correio()
        .retirar(std::time::Duration::from_millis(1), |c| {
            matches!(c, Carta::Ocorrencia(o) if o.alarme == crate::aquario::Alarme::ComandoBloqueado)
        })
        .into_iter()
        .filter_map(|c| match c {
            Carta::Ocorrencia(o) => Some(o),
            _ => None,
        })
        .collect()
}

/// BLOQUEADO deixa a ocorrencia `ComandoBloqueado` -- o motivo e o alarme,
/// com a op, a base e a tabela -- e o bit vermelho na bolha da tarefa.
#[test]
fn o_drop_bloqueado_vira_ocorrencia_e_bolha_vermelha() {
    let (s, _d) = servidor("ocorrencia", true);
    let mut ana = sessao_da_ana();
    let (r, mascara) = pede_amarrado(&s, &mut ana, DROP_TABELA);
    recusou(&r, "rede");
    assert_ne!(
        mascara & crate::aquario::Alarme::ComandoBloqueado.bit(),
        0,
        "sem o bit na tarefa"
    );
    let o = bloqueios(&s);
    assert_eq!(o.len(), 1, "{o:?}");
    let j = o[0].para_json().escrever();
    assert!(j.contains("comando_bloqueado"), "{j}");
    assert!(j.contains("excluir_tabela"), "{j}");
    assert_eq!(
        crate::aquario::Alarme::ComandoBloqueado.gravidade(),
        crate::aquario::Gravidade::Vermelho
    );
}

/// EXECUTOU e TRANCOU deixam linha na trilha administrativa, com op, base,
/// tabela e quem.
#[test]
fn o_drop_liberado_e_o_trancar_vao_a_trilha() {
    let (s, _d) = servidor("trilha", true);
    let mut lib = liberada();
    assert!(pede(&s, &mut lib, DROP_TABELA).is_ok());
    assert!(pede(&s, &mut lib, r#""op":"trancar_execucao""#).is_ok());
    let linhas = s.diario.ultimas(10);
    let executou = linhas
        .iter()
        .find(|a| a.recurso == "protecao.executou")
        .unwrap_or_else(|| panic!("sem a linha executou: {linhas:?}"));
    assert_eq!(executou.banco, "b");
    assert_eq!(executou.usuario, "ana");
    let novo = executou.valor_novo.escrever();
    assert!(
        novo.contains("excluir_tabela") && novo.contains("\"tabela\":\"c\""),
        "{novo}"
    );
    assert!(
        linhas
            .iter()
            .any(|a| a.recurso == "protecao.trancou" && a.usuario == "ana"),
        "sem a linha trancou: {linhas:?}"
    );
    // O bloqueado nao finge ter executado.
    let (s, _d) = servidor("trilha-bloqueado", true);
    recusou(&pede(&s, &mut sessao_da_ana(), DROP_TABELA), "bloqueado");
    assert!(s
        .diario
        .ultimas(10)
        .iter()
        .all(|a| a.recurso != "protecao.executou"));
}

/// A cascata larga do `ao_alterar`: trocar a chave da mae leva as 1.000
/// filhas -- e recusa com NADA gravado, solta e dentro da transacao (os
/// dois irmaos que planejam a cascata).
#[test]
fn a_cascata_larga_e_recusada_solta_e_empilhada() {
    for na_transacao in [false, true] {
        let (s, _d) = servidor(&format!("cascata-{na_transacao}"), true);
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
        let mut ana = Sessao {
            ligacao: 31,
            ..sessao_da_ana()
        };
        if na_transacao {
            pede(&s, &mut ana, r#""op":"begin","database":"b""#).unwrap();
        }
        let r = pede(&s, &mut ana, &sql("UPDATE mae SET id = 2 WHERE id = 1"));
        recusou(&r, &format!("cascata, transacao={na_transacao}"));
        assert!(r.unwrap_err().to_string().contains("cascata em b.filha"));
        if na_transacao {
            let _ = pede(&s, &mut ana, r#""op":"rollback""#);
        }
        let mae = s
            .executar(
                "ler",
                &p(r#"{"database":"b","tabela":"mae","rowid":1}"#),
                &dono,
            )
            .unwrap();
        assert_eq!(
            mae.inteiro_ou("id", 0),
            1,
            "a mae mudou: {}",
            mae.escrever()
        );
    }
}

/// P14: o administrador redefine a senha de execucao de OUTRO usuario so com
/// a propria sessao liberada; quem nao administra nao redefine a de ninguem.
#[test]
fn o_administrador_redefine_a_de_outro_so_com_a_sessao_liberada() {
    let (s, _d) = servidor("redefine", true);
    let corpo =
        r#""op":"senha_execucao_definir","login":"bia","nova_senha_execucao":"nova-da-bia-123""#;
    let r = pede(&s, &mut sessao_da_ana(), corpo);
    assert_eq!(r.unwrap_err().codigo(), 4009, "sem a sessao liberada");
    assert!(!s.senhas_de_execucao.tem("bia").unwrap());
    pede(&s, &mut liberada(), corpo).unwrap();
    assert!(s.senhas_de_execucao.tem("bia").unwrap());
    let mut bia = Sessao {
        usuario: cadastro().por_login("bia").cloned(),
        ip: IP.into(),
        ..Sessao::default()
    };
    let r = pede(
        &s,
        &mut bia,
        r#""op":"senha_execucao_definir","login":"ana","nova_senha_execucao":"tomada-da-ana-1""#,
    );
    assert_eq!(r.unwrap_err().codigo(), 4001, "quem nao administra");
    assert!(!s.senhas_de_execucao.tem("ana").unwrap());
}

/// P14: a quinta senha errada seguida bloqueia, ate a certa -- e o bloqueio
/// e da identidade, nao da sessao.
#[test]
fn a_quinta_errada_bloqueia_ate_a_certa() {
    let (s, _d) = servidor("bloqueio", true);
    s.senhas_de_execucao
        .definir(
            "ana",
            phxsql_core::senha::cifrar_com("execucao-da-ana", 8),
            "ana",
            0,
        )
        .unwrap();
    let mut ana = sessao_da_ana();
    for _ in 0..crate::senha_de_execucao::TENTATIVAS_ATE_BLOQUEAR {
        let r = pede(
            &s,
            &mut ana,
            r#""op":"liberar_execucao","senha":"errada-000""#,
        );
        assert_eq!(r.unwrap_err().codigo(), 4001);
    }
    let r = pede(
        &s,
        &mut ana,
        r#""op":"liberar_execucao","senha":"execucao-da-ana""#,
    );
    assert!(r.unwrap_err().to_string().contains("bloqueada"));
    assert!(!ana.execucao_liberada());
    let mut outra = sessao_da_ana();
    let r = pede(
        &s,
        &mut outra,
        r#""op":"liberar_execucao","senha":"execucao-da-ana""#,
    );
    assert!(r.is_err(), "o bloqueio e da identidade, nao da sessao");
}

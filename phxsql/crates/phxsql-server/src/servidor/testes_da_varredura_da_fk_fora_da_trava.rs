//! **Pedido 422: a varredura da chave que nasce conferida roda com a trava
//! global SOLTA, e com a filha E a mae congeladas.**
//!
//! O gancho `na_janela_sem_trava` roda na mesma thread do
//! `declarar_fk`, entre soltar a trava e varrer -- por isso a prova nao
//! depende de corrida: ou a trava esta solta e o gancho consegue pedir, ou
//! ela esta presa e o teste nem termina.
//!
//! As tres perguntas feitas dentro da janela, e o defeito que cada uma pega:
//!
//! * a VIZINHA grava -- sem isso a trava nao saiu, e o pedido nao andou;
//! * a FILHA recusa -- sem o congelamento dela, uma linha orfa entraria
//!   depois de a varredura passar pelo slot dela;
//! * o PAI referenciado recusa sair -- sem congelar a MAE, o `excluir`
//!   (que ainda nao ve a chave, porque ela nao foi gravada) apagaria um pai
//!   com filha, e a chave nasceria «conferida» sobre uma orfa. E o «nunca» da
//!   regra primordial, e e a metade que um conserto so da filha esqueceria.
use super::*;
use std::sync::Mutex as Mx;

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

fn preparar(nome: &str) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("fk-fora-da-trava-{nome}"));
    let config = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    let s = Servidor::novo(config).unwrap();
    let dono = Sessao::default();
    for (op, txt) in [
        ("criar_database", r#"{"database":"b"}"#),
        (
            "criar_tabela",
            r#"{"database":"b","tabela":"clientes",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true}]}"#,
        ),
        (
            "criar_tabela",
            r#"{"database":"b","tabela":"pedidos",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"cliente_id","tipo":"Int4"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true},
                               {"nome":"porCliente","colunas":["cliente_id"]}]}"#,
        ),
        (
            "criar_tabela",
            r#"{"database":"b","tabela":"vizinha",
                    "colunas":[{"nome":"id","tipo":"Int4"}]}"#,
        ),
        (
            "inserir",
            r#"{"database":"b","tabela":"clientes","linha":{"id":1}}"#,
        ),
        (
            "inserir",
            r#"{"database":"b","tabela":"pedidos","linha":{"id":10,"cliente_id":1}}"#,
        ),
    ] {
        s.executar(op, &pedido(txt), &dono).unwrap();
    }
    (s, dir)
}

#[test]
fn a_janela_da_varredura_solta_a_vizinha_e_segura_filha_e_mae() {
    let (s, _dir) = preparar("janela");
    type Vistos = Vec<(&'static str, std::result::Result<(), String>)>;
    let vistos: Arc<Mx<Vistos>> = Arc::new(Mx::new(Vec::new()));
    {
        let s2 = Arc::clone(&s);
        let vistos = Arc::clone(&vistos);
        *s.na_janela_sem_trava.lock().unwrap() = Some(Box::new(move || {
            let dono = Sessao::default();
            let mut v = vistos.lock().unwrap();
            for (rotulo, op, txt) in [
                (
                    "vizinha",
                    "inserir",
                    r#"{"database":"b","tabela":"vizinha","linha":{"id":1}}"#,
                ),
                (
                    "filha",
                    "inserir",
                    r#"{"database":"b","tabela":"pedidos","linha":{"id":11,"cliente_id":99}}"#,
                ),
                (
                    "mae",
                    "excluir",
                    r#"{"database":"b","tabela":"clientes","rowid":1,"fisico":true}"#,
                ),
            ] {
                let r = s2.executar(op, &pedido(txt), &dono);
                v.push((rotulo, r.map(|_| ()).map_err(|e| format!("{e:?}"))));
            }
        }));
    }

    s.executar(
        "declarar_fk",
        &pedido(
            r#"{"database":"b","tabela":"pedidos","nome":"fk_cliente",
                    "colunas":["cliente_id"],"tabela_ref":"clientes","colunas_ref":["id"]}"#,
        ),
        &Sessao::default(),
    )
    .expect("a chave sobre dado limpo tinha de nascer conferida");

    let vistos = vistos.lock().unwrap().clone();
    assert_eq!(
        vistos.len(),
        3,
        "o gancho nao rodou: a varredura nao passou pela janela sem a trava"
    );
    let de = |r: &str| vistos.iter().find(|(x, _)| *x == r).unwrap().1.clone();
    assert!(
        de("vizinha").is_ok(),
        "a vizinha nao gravou na janela: {:?}",
        de("vizinha")
    );
    for r in ["filha", "mae"] {
        let e = de(r).expect_err(&format!(
            "a {r} GRAVOU no meio da varredura: a chave nasceu conferida sobre \
                 dado que ela nao viu"
        ));
        assert!(
            e.contains("EmMigracao"),
            "{r} recusou por outro motivo: {e}"
        );
    }
    // O pai continua la, e a filha que o citava tambem.
    s.executar(
        "ler",
        &pedido(r#"{"database":"b","tabela":"clientes","rowid":1}"#),
        &Sessao::default(),
    )
    .expect("o pai referenciado saiu durante a varredura");
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"clientes","linha":{"id":2}}"#),
        &Sessao::default(),
    )
    .expect("a mae ficou congelada depois da declaracao");
}

/// Autorreferencia: filha e mae sao a MESMA tabela, e congelar duas vezes
/// a mesma chave e o que o registro recusa. Uma posse basta.
#[test]
fn a_chave_que_aponta_para_a_propria_tabela_declara() {
    let (s, _dir) = preparar("auto");
    let dono = Sessao::default();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"arvore",
                    "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                               {"nome":"pai","tipo":"Int4"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true},
                               {"nome":"porPai","colunas":["pai"]}]}"#,
        ),
        &dono,
    )
    .unwrap();
    for txt in [
        r#"{"database":"b","tabela":"arvore","linha":{"id":1,"pai":null}}"#,
        r#"{"database":"b","tabela":"arvore","linha":{"id":2,"pai":1}}"#,
    ] {
        s.executar("inserir", &pedido(txt), &dono).unwrap();
    }
    s.executar(
        "declarar_fk",
        &pedido(
            r#"{"database":"b","tabela":"arvore","nome":"fk_pai",
                    "colunas":["pai"],"tabela_ref":"arvore","colunas_ref":["id"]}"#,
        ),
        &dono,
    )
    .expect("a autorreferencia nao declarou");
    let e = s
        .executar(
            "inserir",
            &pedido(r#"{"database":"b","tabela":"arvore","linha":{"id":3,"pai":77}}"#),
            &dono,
        )
        .expect_err("a chave declarada nao ficou conferida");
    assert!(!format!("{e:?}").contains("EmMigracao"), "{e:?}");
}

/// Transacao viva na FILHA: quem cede e a declaracao, na hora, com
/// `EM_TRANSACAO` e nada gravado (D5 do 426). Sem a pergunta, o
/// congelamento pegaria o `COMMIT` dela pela metade.
#[test]
fn transacao_viva_na_filha_segura_a_declaracao_e_ela_cede() {
    let (s, _dir) = preparar("transacao");
    let caixa = Sessao {
        ligacao: 7,
        ip: "127.0.0.1".into(),
        ..Sessao::default()
    };
    let pede = |corpo: &str| {
        let mut ses = Sessao {
            ligacao: caixa.ligacao,
            ip: caixa.ip.clone(),
            ..Sessao::default()
        };
        s.despachar(
            &format!(r#"{{"token":"t",{corpo}}}"#),
            &mut ses,
            "127.0.0.1",
        )
        .2
    };
    pede(r#""op":"begin""#).unwrap();
    pede(
        r#""op":"inserir","database":"b","tabela":"pedidos",
               "linha":{"id":20,"cliente_id":1}"#,
    )
    .unwrap();
    let declarar = || {
        s.executar(
            "declarar_fk",
            &pedido(
                r#"{"database":"b","tabela":"pedidos","nome":"fk_cliente",
                        "colunas":["cliente_id"],"tabela_ref":"clientes","colunas_ref":["id"]}"#,
            ),
            &Sessao::default(),
        )
    };
    let e = declarar().expect_err("a declaracao congelou a filha de uma transacao viva");
    assert_eq!(e.nome(), "EM_TRANSACAO", "{e}");
    assert!(e.adianta_repetir(), "{e}");
    pede(r#""op":"commit""#).expect("o COMMIT da transacao que segurou a declaracao");
    declarar().expect("depois do COMMIT a declaracao tinha de passar");
}

/// Os `*.novo` de `b` agora, pelo nome do volume.
fn novos(dir: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(dir.join("b"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".novo"))
        .collect()
}

/// **A reescrita do `.reg` tambem sai da trava**: um nome comprido faz o
/// bloco de esquema nao caber antes do slot 1, e a FASE A -- o `*.novo` de
/// cada volume -- acontece na janela, com a vizinha gravando. So os
/// `rename` ficam para a trava retomada.
#[test]
fn a_reescrita_do_esquema_acontece_fora_da_trava() {
    let (s, dir) = preparar("reescrita");
    type NaJanela = Option<(Vec<String>, bool)>;
    let na_janela: Arc<Mx<NaJanela>> = Arc::new(Mx::new(None));
    {
        let s2 = Arc::clone(&s);
        let d = dir.to_path_buf();
        let na_janela = Arc::clone(&na_janela);
        *s.na_janela_sem_trava.lock().unwrap() = Some(Box::new(move || {
            let vizinha = s2
                .executar(
                    "inserir",
                    &pedido(r#"{"database":"b","tabela":"vizinha","linha":{"id":9}}"#),
                    &Sessao::default(),
                )
                .is_ok();
            *na_janela.lock().unwrap() = Some((novos(&d), vizinha));
        }));
    }
    let r = s
        .executar(
            "declarar_fk",
            &pedido(
                r#"{"database":"b","tabela":"pedidos",
                        "nome":"fk_cliente_com_nome_comprido_de_proposito_para_estourar_a_folga_do_alinhamento",
                        "colunas":["cliente_id"],"tabela_ref":"clientes","colunas_ref":["id"]}"#,
            ),
            &Sessao::default(),
        )
        .expect("a declaracao com reescrita falhou");
    assert!(
        r.booleano_ou("arquivos_reescritos", false),
        "o preparo falhou: o nome nao forcou a reescrita, e o teste nao exercitou a FASE A"
    );
    let (vistos, vizinha) = na_janela
        .lock()
        .unwrap()
        .clone()
        .expect("o gancho nao rodou");
    assert!(
        vistos.iter().any(|n| n.starts_with("pedidos")),
        "nenhum `*.novo` de pedidos na janela: a reescrita ficou para dentro da trava ({vistos:?})"
    );
    assert!(vizinha, "a vizinha nao gravou durante a reescrita");
    assert!(
        novos(&dir).is_empty(),
        "sobrou `*.novo` depois da troca: {:?}",
        novos(&dir)
    );
    let l = s
        .executar(
            "ler",
            &pedido(r#"{"database":"b","tabela":"pedidos","rowid":1}"#),
            &Sessao::default(),
        )
        .expect("a linha de antes nao se le depois da reescrita");
    assert_eq!(l.inteiro_ou("cliente_id", -1), 1, "{}", l.escrever());
}

/// **Pedido 661: o `*.novo` trocado na janela sem trava e recusado na
/// FASE B.** O gancho tira o `pedidos.reg.novo` que a FASE A escreveu e
/// poe no nome um arquivo de MESMO conteudo e outro inode -- quem tem
/// escrita na pasta de dados faz isso entre as fases. A declaracao recusa
/// com `Conflito`, a tabela fica como estava e o nome plantado sai.
///
/// # Prova real
///
/// Sem o `conferir_novos` no `alargar_fase_b`, o `rename` publica o
/// arquivo plantado, a declaracao passa e o `expect_err` reprova.
#[test]
fn o_novo_trocado_na_janela_sem_trava_e_recusado() {
    let (s, dir) = preparar("novo-trocado");
    let trocou: Arc<Mx<bool>> = Arc::new(Mx::new(false));
    {
        let d = dir.to_path_buf();
        let trocou = Arc::clone(&trocou);
        *s.na_janela_sem_trava.lock().unwrap() = Some(Box::new(move || {
            let novo = d.join("b").join("pedidos.reg.novo");
            let Ok(bytes) = std::fs::read(&novo) else {
                return;
            };
            std::fs::remove_file(&novo).unwrap();
            std::fs::write(&novo, &bytes).unwrap();
            *trocou.lock().unwrap() = true;
        }));
    }
    let e = s
        .executar(
            "declarar_fk",
            &pedido(
                r#"{"database":"b","tabela":"pedidos",
                        "nome":"fk_cliente_com_nome_comprido_de_proposito_para_estourar_a_folga_do_alinhamento",
                        "colunas":["cliente_id"],"tabela_ref":"clientes","colunas_ref":["id"]}"#,
            ),
            &Sessao::default(),
        )
        .expect_err("o `.novo` trocado na janela foi publicado pela FASE B");
    assert!(
        *trocou.lock().unwrap(),
        "o preparo falhou: nao havia `.novo` na janela, e o teste nao exercitou a troca"
    );
    assert!(matches!(e, PhxError::Conflito(_)), "recusou como {e:?}");
    assert!(
        novos(&dir).is_empty(),
        "o nome plantado ficou: {:?}",
        novos(&dir)
    );
    // A tabela continua a velha: le, grava, e a chave nao nasceu.
    let l = s
        .executar(
            "ler",
            &pedido(r#"{"database":"b","tabela":"pedidos","rowid":1}"#),
            &Sessao::default(),
        )
        .expect("a tabela nao se le depois da recusa");
    assert_eq!(l.inteiro_ou("cliente_id", -1), 1, "{}", l.escrever());
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"pedidos","linha":{"id":12,"cliente_id":99}}"#),
        &Sessao::default(),
    )
    .expect("a chave nasceu apesar da recusa, ou a tabela ficou congelada");
}

/// O `marcar_lgpd` tem a mesma forma: na janela a tabela esta congelada
/// e a vizinha grava; com transacao viva na tabela, ele cede.
#[test]
fn marcar_lgpd_solta_a_trava_e_cede_a_transacao() {
    let (s, _dir) = preparar("lgpd");
    let vistos: Arc<Mx<Vec<(&'static str, bool)>>> = Arc::new(Mx::new(Vec::new()));
    {
        let s2 = Arc::clone(&s);
        let vistos = Arc::clone(&vistos);
        *s.na_janela_sem_trava.lock().unwrap() = Some(Box::new(move || {
            let mut v = vistos.lock().unwrap();
            for (rotulo, txt) in [
                (
                    "vizinha",
                    r#"{"database":"b","tabela":"vizinha","linha":{"id":7}}"#,
                ),
                (
                    "clientes",
                    r#"{"database":"b","tabela":"clientes","linha":{"id":8}}"#,
                ),
            ] {
                let r = s2.executar("inserir", &pedido(txt), &Sessao::default());
                v.push((rotulo, r.is_ok()));
            }
        }));
    }
    let marcar = || {
        s.executar(
            "marcar_lgpd",
            &pedido(r#"{"database":"b","tabela":"clientes","colunas":{"id":"pessoal"}}"#),
            &Sessao::default(),
        )
    };
    marcar().expect("marcar_lgpd falhou");
    let v = vistos.lock().unwrap().clone();
    assert_eq!(
        v,
        vec![("vizinha", true), ("clientes", false)],
        "a janela: {v:?}"
    );

    let pede = |corpo: &str| {
        let mut ses = Sessao {
            ligacao: 9,
            ip: "127.0.0.1".into(),
            ..Sessao::default()
        };
        s.despachar(
            &format!(r#"{{"token":"t",{corpo}}}"#),
            &mut ses,
            "127.0.0.1",
        )
        .2
    };
    pede(r#""op":"begin""#).unwrap();
    pede(r#""op":"inserir","database":"b","tabela":"clientes","linha":{"id":30}"#).unwrap();
    let e = marcar().expect_err("marcar_lgpd congelou a tabela de uma transacao viva");
    assert_eq!(e.nome(), "EM_TRANSACAO", "{e}");
    pede(r#""op":"commit""#).expect("o COMMIT que segurou a marcacao");
}

/// O comportamento VELHO: dado sujo continua recusando a chave conferida,
/// agora dito pela varredura de fora da trava.
#[test]
fn dado_que_viola_continua_recusando() {
    let (s, _dir) = preparar("sujo");
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"pedidos","linha":{"id":12,"cliente_id":5}}"#),
        &Sessao::default(),
    )
    .unwrap();
    let e = s
        .executar(
            "declarar_fk",
            &pedido(
                r#"{"database":"b","tabela":"pedidos","nome":"fk_cliente",
                        "colunas":["cliente_id"],"tabela_ref":"clientes","colunas_ref":["id"]}"#,
            ),
            &Sessao::default(),
        )
        .expect_err("a chave nasceu conferida sobre uma orfa");
    assert!(e.to_string().contains("nao pode nascer conferida"), "{e}");
    // O erro saiu no meio da janela: as duas tabelas tem de voltar a
    // gravar. Contar `congelamento::quantas()` aqui flocaria -- ele e do
    // processo, e os testes vizinhos congelam em paralelo.
    for txt in [
        r#"{"database":"b","tabela":"pedidos","linha":{"id":13,"cliente_id":1}}"#,
        r#"{"database":"b","tabela":"clientes","linha":{"id":2}}"#,
    ] {
        s.executar("inserir", &pedido(txt), &Sessao::default())
            .expect("o erro da varredura deixou a tabela congelada");
    }
}

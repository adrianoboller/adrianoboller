//! O conflito por chave duplicada num indice unico SECUNDARIO -- pedido 292.
//!
//! O casamento entre servidores usa UMA chave (`bidirecional::chave_unica`), e
//! a unicidade dos outros indices continua sendo conferida na gravacao. Com
//! primaria `porId` e um secundario `porEmail`, o evento do outro lado com um
//! e-mail que ja existe aqui e recusado -- e a recusa subia pelo `?`, `desde`
//! nunca andava, e o mesmo lote voltava para sempre, CALADO.
//!
//! Hoje ela vira `Aplicacao::Conflito`, com o indice, o valor da chave e as
//! duas linhas redigidas -- o conteudo que o PostgreSQL carrega --, e quem
//! chama para o par naquela tabela.
//!
//! Aqui esta o caminho que aplica o evento; a prova do laco inteiro, pelo
//! soquete e com dois servidores no ar, e
//! `tests/laco-do-unico-secundario.rs`.
use super::*;

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

const COLUNAS: &str = r#""colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                                       {"nome":"email","tipo":"Str(20)"}],
                            "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true},
                                       {"nome":"porEmail","colunas":["email"],"unico":true}]"#;

/// Dois databases com a MESMA tabela: `b` e este servidor, `r` faz o papel
/// do outro lado -- e da o que so o outro lado poderia dar, uma linha com
/// id proprio e o e-mail que ja existe aqui.
fn terreno(nome: &str, email_de_la: &str) -> (Arc<Servidor>, Table, Table, DirTemp) {
    let dir = DirTemp::novo(&format!("unicidade-{nome}"));
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    let dono = Sessao::default();
    for (db, id, email) in [("b", 1, "a@x"), ("r", 2, email_de_la)] {
        s.executar(
            "criar_database",
            &pedido(&format!(r#"{{"database":"{db}"}}"#)),
            &dono,
        )
        .unwrap();
        s.executar(
            "criar_tabela",
            &pedido(&format!(r#"{{"database":"{db}","tabela":"c",{COLUNAS}}}"#)),
            &dono,
        )
        .unwrap();
        s.executar(
            "inserir",
            &pedido(&format!(
                r#"{{"database":"{db}","tabela":"c","linha":{{"id":{id},"email":"{email}"}}}}"#
            )),
            &dono,
        )
        .unwrap();
    }
    let aqui = Table::abrir(dir.join("b"), "c").unwrap();
    let la = Table::abrir(dir.join("r"), "c").unwrap();
    (s, aqui, la, dir)
}

fn evento(la: &mut Table, operacao: Operacao) -> crate::replica::EventoRecebido {
    crate::replica::EventoRecebido {
        operacao,
        rowid: 1,
        versao: 1,
        imagem: la.imagem_da_linha_do_rowid(1).unwrap(),
        carimbo_ms: crate::agora_ms(),
        origem: 0,
        tx: 0,
        posicao: 7,
    }
}

fn contador(s: &Servidor) -> u64 {
    s.toques_bidi
        .lock()
        .unwrap()
        .get("b/c")
        .map(|m| m.recusas_por_unicidade)
        .unwrap_or(0)
}

fn no_estado(s: &Servidor) -> Option<i64> {
    s.executar("replicacao_estado", &pedido("{}"), &Sessao::default())
        .unwrap()
        .campo("recusas_por_unicidade")
        .and_then(|c| c.campo("b/c"))
        .and_then(Json::inteiro)
}

fn ids(t: &mut Table) -> Vec<i64> {
    t.varrer()
        .unwrap()
        .into_iter()
        .map(|(_, v)| match v[0] {
            Value::Int(i) => i,
            _ => -1,
        })
        .collect()
}

/// **Prova real.** O evento de la traz id 2 com o e-mail que a linha 1
/// daqui ja ocupa: o `porEmail` recusa, a recusa NAO pode virar `Err`, e o
/// conflito volta ANALISADO -- indice, valor da chave e as duas linhas.
///
/// **Defeito reposto**: trocar o bloco da escrita de volta por
/// `... tabela.inserir_replicado(&valores)?;` subindo pelo `?` --
/// `aplicar_por_chave` devolve `Err(Duplicado)`, o `expect` abaixo estoura
/// e o contador fica em zero.
#[test]
fn chave_duplicada_no_unico_secundario_volta_como_conflito_analisado() {
    let (s, mut aqui, mut la, _dir) = terreno("recusa", "a@x");
    let e = evento(&mut la, Operacao::Inclusao);
    let r = s
        .aplicar_por_chave(
            &mut aqui,
            ALVO_DE_TESTE,
            &e,
            bidirecional::hash_id("beta"),
            &mut None,
        )
        .expect("a recusa por unicidade nao pode subir: ela para o par de servidores");
    let bidirecional::Aplicacao::Conflito(c) = &r else {
        panic!("a recusa tinha de voltar como conflito, e voltou {r:?}");
    };
    // O grito diz o que o PostgreSQL diz: o INDICE que recusou -- que e o
    // secundario, e nao o do casamento --, o valor da chave e as duas
    // linhas. Sem isto o operador sabe que parou e nao sabe em que.
    assert_eq!(c.indice, "porEmail", "o indice culpado nao foi analisado");
    assert_eq!(c.valor, "a@x");
    assert!(
        c.linha_daqui.contains("id=1") && c.linha_daqui.contains("a@x"),
        "a linha daqui nao saiu: {}",
        c.linha_daqui
    );
    assert!(
        c.linha_de_la.contains("id=2") && c.linha_de_la.contains("a@x"),
        "a linha de la nao saiu: {}",
        c.linha_de_la
    );
    assert_eq!(contador(&s), 1, "a recusa nao foi contada");
    assert_eq!(no_estado(&s), Some(1), "o contador nao chegou ao estado");
    // O dado daqui ficou como estava, e a chave recusada NAO ganhou toque:
    // um toque de linha que nunca entrou faria o proximo evento dela
    // disputar contra um passado inventado.
    assert_eq!(ids(&mut aqui), vec![1]);
    assert!(
        s.toques_bidi.lock().unwrap()["b/c"].toques.is_empty(),
        "a chave recusada ganhou toque"
    );
}

/// O comportamento VELHO: sem colisao de unicidade o evento entra, nada e
/// contado, e a tabela some do campo do `replicacao_estado`.
#[test]
fn sem_colisao_o_evento_entra_e_nada_e_contado() {
    let (s, mut aqui, mut la, _dir) = terreno("passa", "b@x");
    let e = evento(&mut la, Operacao::Inclusao);
    let aplicou = s
        .aplicar_por_chave(
            &mut aqui,
            ALVO_DE_TESTE,
            &e,
            bidirecional::hash_id("beta"),
            &mut None,
        )
        .unwrap();
    assert!(aplicou.entrou(), "o evento sem conflito tem de entrar");
    assert_eq!(ids(&mut aqui), vec![1, 2]);
    assert_eq!(contador(&s), 0);
    assert_eq!(no_estado(&s), None, "tabela sem recusa apareceu no campo");
}

/// O comportamento VELHO que a recusa nao pode alargar: erro que NAO e
/// duplicidade continua subindo e parando a rodada. O evento sem imagem e
/// o caso escrito -- o outro lado sem `replicacao.imagem_da_linha` nao tem
/// o que aplicar, e seguir calado gravaria menos do que o source tem.
#[test]
fn erro_que_nao_e_duplicidade_continua_parando_a_rodada() {
    let (s, mut aqui, _la, _dir) = terreno("outro-erro", "b@x");
    let e = crate::replica::EventoRecebido {
        operacao: Operacao::Inclusao,
        rowid: 1,
        versao: 1,
        imagem: Vec::new(),
        carimbo_ms: crate::agora_ms(),
        origem: 0,
        tx: 0,
        posicao: 0,
    };
    let erro = s
        .aplicar_por_chave(
            &mut aqui,
            ALVO_DE_TESTE,
            &e,
            bidirecional::hash_id("beta"),
            &mut None,
        )
        .unwrap_err();
    assert!(erro.to_string().contains("sem imagem"), "{erro}");
    assert_eq!(contador(&s), 0, "erro de outro naipe virou recusa contada");
}

//! O lote do `replicar` -- revisao SEC de 17/09/2026 (A2), e o buraco que o
//! papel G apontou na mesma rodada: os dois tetos so existiam na declaracao.
use super::*;

/// Um source com a imagem no diario -- e so isso: a tabela nasce pelo
/// store, antes de o servidor subir, porque o que se mede aqui e o que o
/// `replicar` LE, e nao como as linhas entraram.
fn source(dir: &std::path::Path) -> Arc<Servidor> {
    let txt = r#"{"token":"t","replicacao":{"papel":"source","id_servidor":"src-01",
                        "imagem_da_linha":true}}"#;
    let mut c = Config::de_json(&Json::analisar(txt).unwrap()).unwrap();
    c.base = dir.to_path_buf();
    c.log_acessos = dir.join("acessos.log");
    c.blacklist = dir.join("blacklist.json");
    c.dblink = dir.join("dblink.json");
    c.jobs = dir.join("jobs.json");
    Servidor::novo(c).unwrap()
}

fn tabela(dir: &std::path::Path, com_memo: bool) -> Table {
    std::fs::create_dir_all(dir.join("loja")).unwrap();
    let mut colunas = vec![Column::new("id", ColumnType::Int8).obrigatoria()];
    if com_memo {
        colunas.push(Column::new("memo", ColumnType::Memo));
    }
    let esquema = Schema::new(
        "clientes",
        colunas,
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap();
    Table::criar(dir.join("loja"), esquema)
        .unwrap()
        .com_imagem_no_diario(true)
}

fn replicar(s: &Arc<Servidor>, desde: u64, extra: &str) -> Json {
    let p = Json::analisar(&format!(
        r#"{{"database":"loja","tabela":"clientes","desde":{desde}{extra}}}"#
    ))
    .unwrap();
    s.op_replicar(&p, &Sessao::default()).unwrap()
}

fn quantos(r: &Json) -> usize {
    r.campo("eventos").and_then(Json::lista).unwrap().len()
}

/// Ausente, zero e negativo valem o padrao; um numero vale o numero; o
/// absurdo vale o teto -- inclusive o que satura ao virar `i64`.
#[test]
fn max_zero_ou_negativo_vale_o_padrao_e_o_absurdo_vale_o_teto() {
    let j = |t: &str| Json::analisar(t).unwrap();
    assert_eq!(lote_de_replicacao(&j("{}")), LOTE_PADRAO_DE_REPLICACAO);
    assert_eq!(
        lote_de_replicacao(&j(r#"{"max":0}"#)),
        LOTE_PADRAO_DE_REPLICACAO
    );
    assert_eq!(
        lote_de_replicacao(&j(r#"{"max":-1}"#)),
        LOTE_PADRAO_DE_REPLICACAO
    );
    assert_eq!(lote_de_replicacao(&j(r#"{"max":7}"#)), 7);
    assert_eq!(
        lote_de_replicacao(&j(r#"{"max":5000}"#)),
        TETO_DE_EVENTOS_POR_LOTE
    );
    assert_eq!(
        lote_de_replicacao(&j(r#"{"max":5001}"#)),
        TETO_DE_EVENTOS_POR_LOTE
    );
    assert_eq!(
        lote_de_replicacao(&j(r#"{"max":1e19}"#)),
        TETO_DE_EVENTOS_POR_LOTE
    );
    assert_eq!(
        lote_de_replicacao(&j(r#"{"max":"x"}"#)),
        LOTE_PADRAO_DE_REPLICACAO
    );
}

/// **Prova real do A2.** 600 eventos e `"max":0`: voltam 500 -- o padrao
/// --, e nao os 600. Com o `.max(0)` de antes reposto no `op_replicar`,
/// voltam os 600 e este teste cai na primeira asercao.
#[test]
fn replicar_com_max_zero_serve_o_lote_padrao_e_nao_o_diario_inteiro() {
    let dir = DirTemp::novo("lote-max-zero");
    {
        let mut t = tabela(&dir, false);
        for i in 1..=600 {
            t.inserir(&[Value::Int(i)]).unwrap();
        }
        t.sincronizar().unwrap();
    }
    let s = source(&dir);
    for extra in [r#","max":0"#, r#","max":-1"#, ""] {
        let r = replicar(&s, 0, extra);
        assert_eq!(
            quantos(&r),
            500,
            "{extra:?}: max zero nao e o diario inteiro"
        );
        assert_eq!(r.inteiro_ou("ate", 0), 500, "{extra:?}");
        assert!(
            !r.booleano_ou("fim", true),
            "{extra:?}: ainda ha 100 para servir"
        );
    }
    // O comportamento VELHO: quem pede um numero leva o numero.
    assert_eq!(quantos(&replicar(&s, 0, r#","max":7"#)), 7);
    // E a continuacao pelo `ate` fecha o diario sem pular nada.
    let r = replicar(&s, 500, r#","max":0"#);
    assert_eq!(quantos(&r), 100);
    assert!(r.booleano_ou("fim", false));
}

/// O teto de BYTES do lado do source, que nao tinha prova nenhuma (papel
/// G, 17/09/2026): tres linhas de 5,6 MiB somam 16,8 MiB, e o lote de
/// 16 MiB para na segunda. O primeiro evento entra sempre -- com
/// `desde: 2` a terceira, sozinha, sai inteira. Sem o teto na leitura
/// (`usize::MAX` no lugar de `TETO_DO_LOTE_SERVIDO`), voltam tres.
#[test]
fn o_lote_servido_corta_por_bytes_e_o_primeiro_evento_entra_sempre() {
    const GORDA: usize = 5_600 * 1024;
    let dir = DirTemp::novo("lote-bytes");
    {
        let mut t = tabela(&dir, true);
        for i in 1..=3 {
            t.inserir(&[Value::Int(i), Value::Memo("x".repeat(GORDA))])
                .unwrap();
        }
        t.sincronizar().unwrap();
    }
    let s = source(&dir);
    let r = replicar(&s, 0, "");
    assert_eq!(
        quantos(&r),
        2,
        "duas de 5,6 MiB cabem em 16 MiB; a terceira nao"
    );
    assert_eq!(r.inteiro_ou("ate", 0), 2);
    assert!(!r.booleano_ou("fim", true));
    let r = replicar(&s, 2, "");
    let eventos = r.campo("eventos").and_then(Json::lista).unwrap();
    assert_eq!(
        eventos.len(),
        1,
        "o primeiro evento entra custe o que custar"
    );
    assert!(
        eventos[0].texto_ou("imagem", "").len() >= 2 * GORDA,
        "a imagem gorda tem de vir inteira, em hexadecimal"
    );
    assert!(r.booleano_ou("fim", false));
}

/// Irmao do A2: `diario` sem `rowid` devolve a CAUDA com o total certo --
/// e continua devolvendo a mesma resposta de sempre, byte a byte, porque
/// o que mudou foi o que se aloca e nao o que se responde.
#[test]
fn diario_sem_rowid_devolve_a_cauda_com_o_total_do_diario_inteiro() {
    let dir = DirTemp::novo("diario-cauda");
    {
        let mut t = tabela(&dir, false);
        for i in 1..=30 {
            t.inserir(&[Value::Int(i)]).unwrap();
        }
        t.sincronizar().unwrap();
    }
    let s = source(&dir);
    let p = Json::analisar(r#"{"database":"loja","tabela":"clientes","max":5}"#).unwrap();
    let r = s.op_diario(&p, &Sessao::default()).unwrap();
    assert_eq!(
        r.inteiro_ou("total", 0),
        30,
        "o total e do diario, nao da cauda"
    );
    let eventos = r.campo("eventos").and_then(Json::lista).unwrap();
    let rowids: Vec<i64> = eventos.iter().map(|e| e.inteiro_ou("rowid", 0)).collect();
    assert_eq!(
        rowids,
        vec![26, 27, 28, 29, 30],
        "os cinco mais recentes, em ordem"
    );
    // Pedir mais do que ha devolve tudo, e o total nao muda.
    let p = Json::analisar(r#"{"database":"loja","tabela":"clientes","max":100}"#).unwrap();
    let r = s.op_diario(&p, &Sessao::default()).unwrap();
    assert_eq!(r.campo("eventos").and_then(Json::lista).unwrap().len(), 30);
    assert_eq!(r.inteiro_ou("total", 0), 30);
}

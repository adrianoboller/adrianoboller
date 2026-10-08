//! **Pedido 605: o TERCEIRO so ouve «ok» na tabela que acabou de nascer
//! depois do `fsync` da pasta de quem a criou.**
//!
//! O gancho `na_janela_da_criacao` roda na thread do `criar_tabela`, com a
//! tabela ja no disco, a trava solta e o `fsync` ainda por fazer -- a janela
//! que o M2 do desenho alarga com `strace` (`bancada/durabilidade/
//! terceiro-605.py`). Dentro dela entram duas conexoes:
//!
//! * B insere na tabela que nasce: tem de ESPERAR a publicacao;
//! * C insere numa vizinha: tem de ANDAR -- e ela que prova que B espera fora
//!   da trava global, e nao segurando o servidor inteiro.
use super::*;
use std::sync::atomic::AtomicBool;

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

fn preparar(nome: &str) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("terceiro-605-{nome}"));
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
            r#"{"database":"b","tabela":"vizinha",
                    "colunas":[{"nome":"id","tipo":"Int4"}]}"#,
        ),
    ] {
        s.executar(op, &pedido(txt), &dono).unwrap();
    }
    (s, dir)
}

const NOVA: &str = r#"{"database":"b","tabela":"nova",
        "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
        "indices":[{"nome":"porId","colunas":["id"],"unico":true}]}"#;

type Resultado = Arc<Mutex<Option<std::result::Result<(), String>>>>;

/// Roda `op` noutra thread; devolve a marca de «voltou» e o resultado.
fn noutra_thread(
    s: &Arc<Servidor>,
    op: &'static str,
    txt: &'static str,
) -> (Arc<AtomicBool>, Resultado) {
    let voltou = Arc::new(AtomicBool::new(false));
    let r: Resultado = Arc::new(Mutex::new(None));
    let (s, v, r2) = (Arc::clone(s), Arc::clone(&voltou), Arc::clone(&r));
    std::thread::spawn(move || {
        let x = s.executar(op, &pedido(txt), &Sessao::default());
        *r2.lock().unwrap() = Some(x.map(|_| ()).map_err(|e| format!("{e:?}")));
        v.store(true, Ordering::SeqCst);
    });
    (voltou, r)
}

fn ate(voltou: &AtomicBool, ms: u64) -> bool {
    let fim = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < fim {
        if voltou.load(Ordering::SeqCst) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    voltou.load(Ordering::SeqCst)
}

/// (B voltou na janela?, C voltou na janela?, a marca e o resultado de B)
type Visto = (bool, bool, Arc<AtomicBool>, Resultado);

#[test]
fn o_terceiro_so_ouve_ok_depois_do_fsync_da_pasta_e_espera_fora_da_trava() {
    let (s, _dir) = preparar("janela");
    let visto: Arc<Mutex<Option<Visto>>> = Arc::new(Mutex::new(None));
    {
        let (s2, visto) = (Arc::clone(&s), Arc::clone(&visto));
        *s.na_janela_da_criacao.lock().unwrap() = Some(Box::new(move || {
            let (b, rb) = noutra_thread(
                &s2,
                "inserir",
                r#"{"database":"b","tabela":"nova","linha":{"id":1}}"#,
            );
            // Folga para B chegar a esperar antes de C entrar.
            std::thread::sleep(Duration::from_millis(100));
            let (c, _rc) = noutra_thread(
                &s2,
                "inserir",
                r#"{"database":"b","tabela":"vizinha","linha":{"id":7}}"#,
            );
            let c_andou = ate(&c, 2_000);
            let b_passou = b.load(Ordering::SeqCst);
            *visto.lock().unwrap() = Some((b_passou, c_andou, b, rb));
        }));
    }
    s.executar("criar_tabela", &pedido(NOVA), &Sessao::default())
        .expect("a criacao tinha de responder");
    let (b_passou, c_andou, b, rb) = visto
        .lock()
        .unwrap()
        .take()
        .expect("o gancho nao rodou: a criacao nao passou pela janela");
    assert!(
        !b_passou,
        "o terceiro ouviu «ok» na tabela que ainda nao estava no disco: \
             o `fsync` da pasta de quem criou nao tinha terminado"
    );
    assert!(
        c_andou,
        "a vizinha nao andou enquanto B esperava: a espera segurou a trava global"
    );
    assert!(ate(&b, 5_000), "B nao acordou depois da publicacao");
    let r = rb.lock().unwrap().clone().unwrap();
    assert!(r.is_ok(), "B esperou e depois falhou: {r:?}");
}

/// **O comportamento VELHO**: quem cria e usa na mesma conexao nao sente
/// nada -- o «ok» do criar so sai depois da publicacao, entao nao ha o que
/// esperar, e a resposta continua a mesma.
#[test]
fn criar_e_usar_na_mesma_conexao_continua_igual() {
    let (s, dir) = preparar("mesma");
    let dono = Sessao::default();
    let r = s.executar("criar_tabela", &pedido(NOVA), &dono).unwrap();
    assert_eq!(r.texto_ou("tabela", ""), "nova");
    assert!(
        !phxsql_store::nascendo::reservada(&dir.join("b"), "nova"),
        "o criar respondeu com a tabela ainda reservada"
    );
    let antes = Instant::now();
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"nova","linha":{"id":1}}"#),
        &dono,
    )
    .unwrap();
    let r = s
        .executar(
            "varrer",
            &pedido(r#"{"database":"b","tabela":"nova"}"#),
            &dono,
        )
        .unwrap();
    assert_eq!(r.inteiro_ou("registros", -1), 1, "{}", r.escrever());
    assert!(
        antes.elapsed() < Duration::from_secs(5),
        "usar a tabela recem-criada na mesma conexao passou a esperar"
    );
}

/// O `fsync` lento de quem cria: o gancho dorme este tanto dentro da
/// janela, e tudo o que se mede acontece enquanto ele dorme.
const FSYNC_LENTO: Duration = Duration::from_secs(2);

/// Roda `dentro` na janela da criacao e dorme o resto do `FSYNC_LENTO`;
/// devolve o que `dentro` viu, depois de a criacao responder.
fn na_janela_lenta<T: Send + 'static>(
    s: &Arc<Servidor>,
    dentro: impl FnOnce(&Arc<Servidor>) -> T + Send + 'static,
) -> T {
    let visto: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));
    {
        let (s2, visto) = (Arc::clone(s), Arc::clone(&visto));
        *s.na_janela_da_criacao.lock().unwrap() = Some(Box::new(move || {
            let inicio = Instant::now();
            let v = dentro(&s2);
            *visto.lock().unwrap() = Some(v);
            std::thread::sleep(FSYNC_LENTO.saturating_sub(inicio.elapsed()));
        }));
    }
    s.executar("criar_tabela", &pedido(NOVA), &Sessao::default())
        .expect("a criacao tinha de responder");
    let v = visto.lock().unwrap().take();
    v.expect("o gancho nao rodou: a criacao nao passou pela janela")
}

/// C insere na vizinha; devolve quanto levou, ou `None` se nao voltou
/// antes do fim do `fsync` lento.
fn a_vizinha_anda(s: &Arc<Servidor>) -> Option<Duration> {
    let inicio = Instant::now();
    let (c, rc) = noutra_thread(
        s,
        "inserir",
        r#"{"database":"b","tabela":"vizinha","linha":{"id":7}}"#,
    );
    if !ate(&c, 1_600) {
        return None;
    }
    let r = rc.lock().unwrap().clone().unwrap();
    assert!(r.is_ok(), "a vizinha recusou: {r:?}");
    Some(inicio.elapsed())
}

/// **Pedido 629 (SEC M1): o `fsync` lento de quem cria nao para o servidor
/// pelos pedidos que ESCONDEM a tabela.**
///
/// A espera de fora lia so `"tabela"`. O `juntar` com a tabela nova em
/// `b.tabela` ia para a espera de DENTRO da trava global, e C, inserindo
/// numa vizinha, ficava parado o `fsync` inteiro de outro. Com a espera
/// de fora lendo `tabelas_do_pedido`, o `juntar` e o `sql` esperam sem
/// segurar ninguem -- e passam, sem recusa, quando a tabela publica.
#[test]
fn o_fsync_lento_de_quem_cria_nao_para_quem_esconde_a_tabela() {
    let (s, _dir) = preparar("esconde");
    type Bs = Vec<(&'static str, Arc<AtomicBool>, Resultado)>;
    let (bs, c_levou): (Bs, Option<Duration>) = na_janela_lenta(&s, |s2| {
        let mut bs = Vec::new();
        for (op, txt) in [
            ("sql", r#"{"database":"b","texto":"SELECT * FROM nova"}"#),
            (
                "juntar",
                r#"{"database":"b","a":{"tabela":"vizinha","chave":"id"},
                        "b":{"tabela":"nova","chave":"id"}}"#,
            ),
        ] {
            let (v, r) = noutra_thread(s2, op, txt);
            bs.push((op, v, r));
        }
        // Folga para os dois chegarem a esperar antes de C entrar.
        std::thread::sleep(Duration::from_millis(150));
        let c = a_vizinha_anda(s2);
        (bs, c)
    });
    assert!(
        c_levou.is_some(),
        "a vizinha ficou parada o `fsync` inteiro de quem criava: alguem \
             esperou a tabela que nasce com a trava global na mao"
    );
    for (op, v, r) in bs {
        assert!(ate(&v, 5_000), "{op} nao acordou depois da publicacao");
        let r = r.lock().unwrap().clone().unwrap();
        assert!(
            r.is_ok(),
            "{op} foi recusado em vez de esperar fora da trava: {r:?}"
        );
    }
}

/// **Pedido 629: a espera de DENTRO da trava tem prazo.** A mae de uma
/// chave conferida nao vem em campo nenhum do pedido -- a espera de fora
/// nao tem como ve-la, e quem a abre e o `conferir_fks`, com a trava na
/// mao. Sem prazo, C ficava parado o `fsync` inteiro de quem criava a
/// mae; com ele, B ouve `NASCENDO` (repetir) e C anda.
#[test]
fn a_mae_que_nasce_recusa_no_prazo_e_a_vizinha_anda() {
    let (s, _dir) = preparar("mae");
    let dono = Sessao::default();
    for (op, txt) in [
        (
            "criar_tabela",
            r#"{"database":"b","tabela":"filha",
                    "colunas":[{"nome":"id","tipo":"Int4"},{"nome":"mae","tipo":"Int4"}],
                    "indices":[{"nome":"porMae","colunas":["mae"]}]}"#,
        ),
        (
            "declarar_fk",
            r#"{"database":"b","tabela":"filha","nome":"fk_mae",
                    "colunas":["mae"],"tabela_ref":"nova","colunas_ref":["id"],
                    "ao_alterar":"restringir"}"#,
        ),
    ] {
        s.executar(op, &pedido(txt), &dono).unwrap();
    }
    let (b, rb, c_levou) = na_janela_lenta(&s, |s2| {
        let (b, rb) = noutra_thread(
            s2,
            "inserir",
            r#"{"database":"b","tabela":"filha","linha":{"id":1,"mae":1}}"#,
        );
        std::thread::sleep(Duration::from_millis(150));
        let c = a_vizinha_anda(s2);
        (b, rb, c)
    });
    assert!(
        c_levou.is_some(),
        "a vizinha ficou parada o `fsync` inteiro de quem criava a mae: a \
             espera de dentro da trava nao tem prazo"
    );
    assert!(ate(&b, 5_000), "B nao voltou");
    let r = rb.lock().unwrap().clone().unwrap();
    let e = r.expect_err("B gravou a filha apontando para a mae que ainda nascia");
    assert!(e.contains("Nascendo"), "B recusou com outra familia: {e}");
}

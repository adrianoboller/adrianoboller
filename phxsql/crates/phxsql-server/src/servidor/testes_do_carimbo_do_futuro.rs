//! O carimbo do modo B vem do outro lado -- revisao SEC de 17/09/2026, A9
//! (pedido 286). A regra pura esta em `bidirecional.rs`; aqui e o caminho que
//! aplica o evento e guarda o toque.
use super::*;

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// Um servidor com `b.c` (id primaria, nome), uma linha, e a tabela
/// aberta a parte para o `aplicar_por_chave`.
fn terreno(nome: &str) -> (Arc<Servidor>, Table, DirTemp) {
    let dir = DirTemp::novo(&format!("carimbo-{nome}"));
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
    s.executar("criar_database", &pedido(r#"{"database":"b"}"#), &dono)
        .unwrap();
    s.executar(
        "criar_tabela",
        &pedido(
            r#"{"database":"b","tabela":"c",
                    "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                               {"nome":"nome","tipo":"Str(20)"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]}"#,
        ),
        &dono,
    )
    .unwrap();
    s.executar(
        "inserir",
        &pedido(r#"{"database":"b","tabela":"c","linha":{"id":1,"nome":"um"}}"#),
        &dono,
    )
    .unwrap();
    let t = Table::abrir(dir.join("b"), "c").unwrap();
    (s, t, dir)
}

fn evento(t: &mut Table, carimbo_ms: i64) -> crate::replica::EventoRecebido {
    crate::replica::EventoRecebido {
        operacao: Operacao::Alteracao,
        rowid: 1,
        versao: 2,
        imagem: t.imagem_da_linha_do_rowid(1).unwrap(),
        carimbo_ms,
        origem: 0,
        tx: 0,
        posicao: 0,
    }
}

fn toque_de(s: &Servidor) -> (Toque, u64) {
    let g = s.toques_bidi.lock().unwrap();
    let m = g.get("b/c").expect("mapa da tabela");
    let toque = *m.toques.values().next().expect("um toque");
    (toque, m.carimbos_do_futuro)
}

/// **Prova real do A9.** O evento com `i64::MAX` entrava como veio, e o
/// toque guardado ficava num instante que nenhuma escrita local alcanca.
///
/// **Defeito reposto**: `let carimbo = e.carimbo_ms;` no
/// `aplicar_por_chave`, e a asercao do toque cai com `i64::MAX`.
#[test]
fn carimbo_do_futuro_entra_com_o_relogio_local_e_e_contado() {
    let (s, mut t, _dir) = terreno("futuro");
    let antes = crate::agora_ms();
    let e = evento(&mut t, i64::MAX);
    let aplicou = s
        .aplicar_por_chave(
            &mut t,
            ALVO_DE_TESTE,
            &e,
            bidirecional::hash_id("beta"),
            &mut None,
        )
        .unwrap();
    assert!(
        aplicou.entrou(),
        "o evento nao e recusado: entra com o relogio daqui"
    );
    let (toque, contados) = toque_de(&s);
    let depois = crate::agora_ms();
    assert!(
        (antes..=depois).contains(&toque.carimbo),
        "o toque guardou o carimbo mentido: {}",
        toque.carimbo
    );
    assert_eq!(contados, 1, "a troca nao foi contada");
    // E a escrita local do instante seguinte vence de novo.
    assert!(bidirecional::remoto_vence(depois + 1, 1, &toque));

    // O contador chega ao `replicacao_estado`, so para a tabela atingida.
    let r = s
        .executar("replicacao_estado", &pedido("{}"), &Sessao::default())
        .unwrap();
    assert_eq!(
        r.campo("carimbos_do_futuro")
            .and_then(|c| c.campo("b/c"))
            .and_then(Json::inteiro),
        Some(1),
        "{}",
        r.escrever()
    );
}

/// O comportamento VELHO: dentro da folga o carimbo vale como veio -- a
/// deriva de relogio que a regra sempre tolerou --, e nada e contado.
#[test]
fn carimbo_dentro_da_folga_entra_como_veio() {
    let (s, mut t, _dir) = terreno("folga");
    let adiantado = crate::agora_ms() + bidirecional::FOLGA_DO_CARIMBO_MS - 1_000;
    let e = evento(&mut t, adiantado);
    s.aplicar_por_chave(
        &mut t,
        ALVO_DE_TESTE,
        &e,
        bidirecional::hash_id("beta"),
        &mut None,
    )
    .unwrap();
    let (toque, contados) = toque_de(&s);
    assert_eq!(toque.carimbo, adiantado);
    assert_eq!(contados, 0);
    let r = s
        .executar("replicacao_estado", &pedido("{}"), &Sessao::default())
        .unwrap();
    assert!(
        r.campo("carimbos_do_futuro")
            .and_then(|c| c.campo("b/c"))
            .is_none(),
        "tabela sem troca apareceu no contador: {}",
        r.escrever()
    );
}

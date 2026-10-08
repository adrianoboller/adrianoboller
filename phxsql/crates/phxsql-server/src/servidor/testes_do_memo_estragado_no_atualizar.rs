//! **Pedido 381: o `.memo` estragado nao recusa o `atualizar` que vem
//! substitui-lo.**
//!
//! O `atualizar` sem `"softdeleted"` relia a linha INTEIRA so para copiar a
//! marca de excluida, e inteira inclui o `.memo`: o `Corrompido` do bloco
//! subia e a gravacao toda era recusada -- inclusive a que trazia valor novo
//! para a coluna estragada. PostgreSQL (TOAST) e InnoDB/MariaDB (paginas
//! externas) liberam o valor velho sem decodifica-lo: os tres convergem, e e
//! aceite automatico. O irmao e o upsert sem SET, que le a linha pelo mesmo
//! motivo.
//!
//! A linha nasce EXCLUIDA SUAVE de proposito: o conserto troca a leitura, e
//! a leitura nova tem de continuar entregando a marca -- senao o `atualizar`
//! passaria a ressuscitar linha, que e o defeito que aquela leitura existe
//! para impedir. As duas metades se conferem em cada teste.
use super::*;

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

/// `b.p` com um `Memo`, a linha 1 gravada com laudo e excluida suave, e o
/// ultimo byte do `.memo` virado -- o CRC do unico bloco deixa de bater.
fn preparar(nome: &str) -> (Arc<Servidor>, DirTemp) {
    let dir = DirTemp::novo(&format!("memo-estragado-{nome}"));
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
            r#"{"database":"b","tabela":"p",
                    "colunas":[{"nome":"id","tipo":"Int8"},
                               {"nome":"laudo","tipo":"Memo"}],
                    "indices":[{"nome":"porId","colunas":["id"],"unico":true}]}"#,
        ),
        (
            "inserir",
            r#"{"database":"b","tabela":"p",
                    "linha":{"id":1,"laudo":"LAUDO_VELHO"}}"#,
        ),
        ("excluir", r#"{"database":"b","tabela":"p","rowid":1}"#),
    ] {
        s.executar(op, &pedido(txt), &dono).unwrap();
    }
    let memo = dir.join("b/p.memo");
    let mut cru = std::fs::read(&memo).expect("o .memo da linha 1 tinha de existir");
    let ultimo = cru.len() - 1;
    cru[ultimo] ^= 0xFF;
    std::fs::write(&memo, &cru).unwrap();
    // Preparo conferido antes do veredito: se o bloco ainda se lesse, o
    // teste passaria sem ter exercitado o defeito.
    assert!(
        s.executar(
            "ler",
            &pedido(r#"{"database":"b","tabela":"p","rowid":1}"#),
            &dono
        )
        .is_err(),
        "o preparo falhou: o .memo estragado ainda se le"
    );
    (s, dir)
}

fn linha_1(s: &Arc<Servidor>) -> Json {
    s.executar(
        "ler",
        &pedido(r#"{"database":"b","tabela":"p","rowid":1}"#),
        &Sessao::default(),
    )
    .expect("depois da substituicao o laudo novo tem de se ler")
}

#[test]
fn atualizar_sem_a_coluna_de_sistema_substitui_o_memo_estragado() {
    let (s, _dir) = preparar("atualizar");
    s.executar(
        "atualizar",
        &pedido(
            r#"{"database":"b","tabela":"p","rowid":1,
                    "valores":{"id":1,"laudo":"LAUDO_NOVO"}}"#,
        ),
        &Sessao::default(),
    )
    .expect("o `.memo` velho estragado recusou a gravacao que o substituia");
    let l = linha_1(&s);
    assert_eq!(l.texto_ou("laudo", ""), "LAUDO_NOVO", "{}", l.escrever());
    assert!(
        l.booleano_ou("softdeleted", false),
        "a leitura sem as externas perdeu a marca e o atualizar \
             RESSUSCITOU a linha: {}",
        l.escrever()
    );
}

#[test]
fn upsert_sem_set_substitui_o_memo_estragado() {
    let (s, _dir) = preparar("upsert");
    s.executar(
        "inserir",
        &pedido(
            r#"{"database":"b","tabela":"p","se_existir":"atualizar",
                    "linha":{"id":1,"laudo":"LAUDO_NOVO"}}"#,
        ),
        &Sessao::default(),
    )
    .expect("o `.memo` velho estragado recusou o upsert que o substituia");
    let l = linha_1(&s);
    assert_eq!(l.texto_ou("laudo", ""), "LAUDO_NOVO", "{}", l.escrever());
    assert!(
        l.booleano_ou("softdeleted", false),
        "o upsert RESSUSCITOU a linha: {}",
        l.escrever()
    );
}

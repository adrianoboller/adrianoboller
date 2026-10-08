//! Pedidos 583 e 584: a sincronia do DbLink no dialeto de cada motor, e os
//! tipos que o puxar trazia errados, provados pelo soquete contra pares falsos
//! do PostgreSQL(R) e do MySQL(R) que respondem como o de verdade responde --
//! inclusive o erro de sintaxe do PostgreSQL(R) para a crase e para o
//! `ON DUPLICATE KEY`, e o BLOB cru do `SELECT *` do MySQL(R).
//!
//! O terceiro motor, `phxsql`, nao entra aqui: a sincronia recusa esse motor
//! antes de ir ao fio (`exigir_catalogo_em_sql`), porque entre dois PhxSql a
//! convergencia e a replicacao nativa.
use super::testes_dblink_fora_da_trava::{
    coluna, coluna_em, lenenc, ler_pacote, pede, quadro, saudar, servidor, EOF,
};
use super::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;

type Celulas = Vec<Vec<Option<Vec<u8>>>>;

fn hex(b: &[u8], maiuscula: bool) -> String {
    b.iter()
        .map(|x| {
            if maiuscula {
                format!("{x:02X}")
            } else {
                format!("{x:02x}")
            }
        })
        .collect()
}

// ------------------------------------------------ o PostgreSQL(R) falso

/// Uma coluna la: nome, OID do tipo, o texto do `format_type` e se e chave.
#[derive(Clone)]
struct ColPg {
    nome: &'static str,
    oid: u32,
    tipo: &'static str,
    chave: bool,
}

fn col_pg(nome: &'static str, oid: u32, tipo: &'static str, chave: bool) -> ColPg {
    ColPg {
        nome,
        oid,
        tipo,
        chave,
    }
}

fn msg_pg(tipo: u8, corpo: &[u8]) -> Vec<u8> {
    let mut m = vec![tipo];
    m.extend_from_slice(&((corpo.len() + 4) as i32).to_be_bytes());
    m.extend_from_slice(corpo);
    m
}

fn descricao_pg(cols: &[(&str, u32)]) -> Vec<u8> {
    let mut c = (cols.len() as i16).to_be_bytes().to_vec();
    for (nome, oid) in cols {
        c.extend_from_slice(nome.as_bytes());
        c.push(0);
        c.extend_from_slice(&0i32.to_be_bytes());
        c.extend_from_slice(&0i16.to_be_bytes());
        c.extend_from_slice(&oid.to_be_bytes());
        c.extend_from_slice(&(-1i16).to_be_bytes());
        c.extend_from_slice(&(-1i32).to_be_bytes());
        c.extend_from_slice(&0i16.to_be_bytes());
    }
    c
}

fn linha_pg(celulas: &[Option<Vec<u8>>]) -> Vec<u8> {
    let mut c = (celulas.len() as i16).to_be_bytes().to_vec();
    for cel in celulas {
        match cel {
            None => c.extend_from_slice(&(-1i32).to_be_bytes()),
            Some(b) => {
                c.extend_from_slice(&(b.len() as i32).to_be_bytes());
                c.extend_from_slice(b);
            }
        }
    }
    c
}

/// A resposta a uma instrucao, como o PostgreSQL(R) a daria. O que ele
/// recusaria por sintaxe -- crase, `ON DUPLICATE KEY` -- volta como `E`.
fn resposta_pg(sql: &str, cols: &[ColPg], linhas: &Celulas) -> Vec<u8> {
    let mut r = Vec::new();
    if sql.contains('`') || sql.contains("DUPLICATE KEY") || sql.contains("VALUES(") {
        r.extend(msg_pg(b'E', b"SERROR\0C42601\0Msyntax error\0\0"));
        r.extend(msg_pg(b'Z', b"I"));
        return r;
    }
    if sql.contains("pg_attribute") {
        let nomes = ["Field", "Type", "Null", "Key", "Default", "Comment"];
        let d: Vec<(&str, u32)> = nomes.iter().map(|n| (*n, 25)).collect();
        r.extend(msg_pg(b'T', &descricao_pg(&d)));
        let cel = |t: &str| Some(t.as_bytes().to_vec());
        for c in cols {
            r.extend(msg_pg(
                b'D',
                &linha_pg(&[
                    cel(c.nome),
                    cel(c.tipo),
                    cel(if c.chave { "NO" } else { "YES" }),
                    cel(if c.chave { "PRI" } else { "" }),
                    cel(""),
                    cel(""),
                ]),
            ));
        }
    } else if sql.starts_with("SELECT") {
        // O `encode(..,'hex')` devolve `text` (OID 25), e nao `bytea`: e o
        // que o PostgreSQL(R) de verdade anuncia no `RowDescription`, e
        // o leitor do fio decide o binario por ele -- pedido 590.
        let d: Vec<(&str, u32)> = cols
            .iter()
            .map(|c| {
                let codificada = sql.contains(&format!("encode(\"{}\",'hex')", c.nome));
                (c.nome, if codificada { 25 } else { c.oid })
            })
            .collect();
        r.extend(msg_pg(b'T', &descricao_pg(&d)));
        if !sql.contains("LIMIT 0") {
            for l in linhas {
                let render: Vec<Option<Vec<u8>>> = l
                    .iter()
                    .zip(cols)
                    .map(|(cel, c)| {
                        cel.as_ref().map(|b| {
                            // O bytea sai no `bytea_output = hex`, o padrao;
                            // pela `encode(..,'hex')`, sem o `\x`.
                            if c.oid != 17 {
                                b.clone()
                            } else if sql.contains(&format!("encode(\"{}\",'hex')", c.nome)) {
                                hex(b, false).into_bytes()
                            } else {
                                format!("\\x{}", hex(b, false)).into_bytes()
                            }
                        })
                    })
                    .collect();
                r.extend(msg_pg(b'D', &linha_pg(&render)));
            }
        }
    } else {
        r.extend(msg_pg(b'C', b"INSERT 0 1\0"));
        r.extend(msg_pg(b'Z', b"I"));
        return r;
    }
    r.extend(msg_pg(b'C', b"SELECT 1\0"));
    r.extend(msg_pg(b'Z', b"I"));
    r
}

fn atender_pg(mut s: TcpStream, cols: Vec<ColPg>, linhas: Celulas, avisar: mpsc::Sender<String>) {
    let mut tam = [0u8; 4];
    if s.read_exact(&mut tam).is_err() {
        return;
    }
    let mut abertura = vec![0u8; (i32::from_be_bytes(tam).max(4) - 4) as usize];
    if s.read_exact(&mut abertura).is_err() {
        return;
    }
    let mut ola = msg_pg(b'R', &0i32.to_be_bytes());
    ola.extend(msg_pg(b'S', b"server_version\x0016.0-falso\0"));
    ola.extend(msg_pg(b'K', &[0, 0, 0, 7, 0, 0, 0, 9]));
    ola.extend(msg_pg(b'Z', b"I"));
    if s.write_all(&ola).is_err() {
        return;
    }
    loop {
        let mut cabeca = [0u8; 5];
        if s.read_exact(&mut cabeca).is_err() {
            return;
        }
        let n = i32::from_be_bytes([cabeca[1], cabeca[2], cabeca[3], cabeca[4]]);
        let mut corpo = vec![0u8; (n.max(4) - 4) as usize];
        if s.read_exact(&mut corpo).is_err() || cabeca[0] != b'Q' {
            return;
        }
        let sql = String::from_utf8_lossy(corpo.strip_suffix(&[0]).unwrap_or(&corpo)).into_owned();
        let _ = avisar.send(sql.clone());
        if s.write_all(&resposta_pg(&sql, &cols, &linhas)).is_err() {
            return;
        }
    }
}

fn par_pg(cols: Vec<ColPg>, linhas: Celulas) -> (u16, mpsc::Receiver<String>) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let (avisar, avisos) = mpsc::channel();
    std::thread::spawn(move || {
        for s in ouvinte.incoming() {
            let Ok(s) = s else { return };
            let (cols, linhas, avisar) = (cols.clone(), linhas.clone(), avisar.clone());
            std::thread::spawn(move || atender_pg(s, cols, linhas, avisar));
        }
    });
    (porta, avisos)
}

// ------------------------------------------------------ o MySQL(R) falso

/// Uma coluna la: nome, codigo do tipo, bandeiras, tamanho e conjunto de
/// caracteres no fio (63 e o `binary` de um BLOB de verdade).
type ColMy = (&'static str, u8, u16, u32, u16);

/// Como o MySQL(R): o `SELECT *` manda o BLOB CRU; o `HEX(..)` manda o
/// texto hexadecimal, em maiuscula, numa coluna de texto.
fn resposta_my(sql: &str, cols: &[ColMy], linhas: &Celulas) -> Vec<Vec<u8>> {
    if !sql.starts_with("SELECT") {
        return vec![vec![0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00]];
    }
    let em_hex = |nome: &str| sql.contains(&format!("HEX(`{nome}`)"));
    // O `CAST(bit AS UNSIGNED)` volta como o MySQL(R) o anuncia: BIGINT
    // UNSIGNED, conjunto 63, e a celula em digitos decimais -- pedido 592.
    let em_numero = |nome: &str| sql.contains(&format!("CAST(`{nome}` AS UNSIGNED)"));
    let mut r = vec![vec![cols.len() as u8]];
    for (nome, codigo, bandeiras, tamanho, charset) in cols {
        r.push(if em_hex(nome) {
            coluna(nome, 0xfd, 0, tamanho * 2)
        } else if em_numero(nome) {
            coluna_em(nome, 0x08, 0x00a0, 21, 63)
        } else {
            coluna_em(nome, *codigo, *bandeiras, *tamanho, *charset)
        });
    }
    r.push(EOF.to_vec());
    if !sql.contains("LIMIT 0") {
        for l in linhas {
            let mut p = Vec::new();
            for (cel, (nome, ..)) in l.iter().zip(cols) {
                match cel {
                    None => p.push(0xFB),
                    Some(b) if em_hex(nome) => lenenc(&mut p, hex(b, true).as_bytes()),
                    Some(b) if em_numero(nome) => {
                        let n = b.iter().fold(0u64, |a, x| (a << 8) | u64::from(*x));
                        lenenc(&mut p, n.to_string().as_bytes())
                    }
                    Some(b) => lenenc(&mut p, b),
                }
            }
            r.push(p);
        }
    }
    r.push(EOF.to_vec());
    r
}

fn atender_my(mut s: TcpStream, cols: Vec<ColMy>, linhas: Celulas, avisar: mpsc::Sender<String>) {
    if !saudar(&mut s) {
        return;
    }
    while let Some(pacote) = ler_pacote(&mut s) {
        if pacote.first() != Some(&0x03) {
            return;
        }
        let sql = String::from_utf8_lossy(&pacote[1..]).into_owned();
        let _ = avisar.send(sql.clone());
        let mut bytes = Vec::new();
        for (i, carga) in resposta_my(&sql, &cols, &linhas).iter().enumerate() {
            bytes.extend(quadro(i as u8 + 1, carga));
        }
        if s.write_all(&bytes).is_err() {
            return;
        }
    }
}

fn par_my(cols: Vec<ColMy>, linhas: Celulas) -> (u16, mpsc::Receiver<String>) {
    let ouvinte = TcpListener::bind("127.0.0.1:0").unwrap();
    let porta = ouvinte.local_addr().unwrap().port();
    let (avisar, avisos) = mpsc::channel();
    std::thread::spawn(move || {
        for s in ouvinte.incoming() {
            let Ok(s) = s else { return };
            let (cols, linhas, avisar) = (cols.clone(), linhas.clone(), avisar.clone());
            std::thread::spawn(move || atender_my(s, cols, linhas, avisar));
        }
    });
    (porta, avisos)
}

// ------------------------------------------------------------ o preparo

fn salvar(s: &Arc<Servidor>, motor: &str, porta: u16) {
    pede(s, r#""op":"criar_database","database":"loja""#).unwrap();
    pede(
        s,
        &format!(
            r#""op":"dblink_salvar","nome":"erp","motor":"{motor}","host":"127.0.0.1",
                   "porta":{porta},"usuario":"u","database":"erp","timeout_s":5,
                   "somente_leitura":false,"cifra":false"#
        ),
    )
    .unwrap();
}

fn ligar(s: &Arc<Servidor>, sentido: &str) -> Result<Json> {
    pede(
        s,
        &format!(
            r#""op":"dblink_ligar","dblink":"erp","tabelas":[{{"remota":"clientes",
                   "local_database":"loja","sentido":"{sentido}","dono":"aqui"}}]"#
        ),
    )
}

fn varrer(s: &Arc<Servidor>) -> String {
    pede(s, r#""op":"varrer","database":"loja","tabela":"clientes""#)
        .unwrap()
        .escrever()
}

fn cel(t: &str) -> Option<Vec<u8>> {
    Some(t.as_bytes().to_vec())
}

const TABELA_COM_UUID: &str = r#""op":"criar_tabela","database":"loja","tabela":"clientes",
        "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},{"nome":"cod","tipo":"Uuid"}],
        "indices":[{"nome":"porChave","colunas":["id"],"unico":true}]"#;

/// **583: uma rodada inteira ida e volta contra o PostgreSQL(R).** Liga,
/// puxa a linha de la (com o booleano `t`) e empurra a daqui (com aspa e
/// booleano falso), conferindo o SQL que chegou ao par.
///
/// # Prova real
///
/// Com a instrucao de MySQL(R) de volta -- crase, `ON DUPLICATE KEY
/// UPDATE`, `1`/`0` --, o par recusa por sintaxe e a rodada nem liga; com
/// o booleano do fio lido pela regua da carga colada, o `t` recusa.
#[test]
fn a_sincronia_contra_postgres_roda_ida_e_volta_no_dialeto_dele() {
    let dir = DirTemp::novo("583-dblink-pg");
    let s = servidor(&dir);
    let cols = vec![
        col_pg("id", 23, "integer", true),
        col_pg("nome", 1043, "character varying(40)", false),
        col_pg("ativo", 16, "boolean", false),
    ];
    let (porta, avisos) = par_pg(cols, vec![vec![cel("1"), cel("remoto"), cel("t")]]);
    salvar(&s, "postgres", porta);
    let r = match ligar(&s, "dois") {
        Ok(j) => j.escrever(),
        Err(e) => panic!("o dblink_ligar contra o PostgreSQL falhou: {e}"),
    };
    assert!(
        r.contains("\"tabela_criada\":true") && r.contains("\"chave\":\"id\""),
        "{r}"
    );
    pede(
        &s,
        r#""op":"inserir","database":"loja","tabela":"clientes",
               "linha":{"id":2,"nome":"D'Avila","ativo":false}"#,
    )
    .unwrap();
    let r = match pede(&s, r#""op":"dblink_sincronizar","dblink":"erp""#) {
        Ok(j) => j.escrever(),
        Err(e) => panic!("o dblink_sincronizar contra o PostgreSQL falhou: {e}"),
    };
    assert!(r.contains("\"puxadas_novas\":1"), "{r}");
    assert!(r.contains("\"empurradas\":1"), "{r}");

    let recebidas: Vec<String> = avisos.try_iter().collect();
    for sql in &recebidas {
        assert!(
            !sql.contains('`'),
            "crase de MySQL no fio do PostgreSQL: {sql}"
        );
    }
    let insert = recebidas
        .iter()
        .find(|q| q.starts_with("INSERT"))
        .expect("o par nao recebeu o INSERT");
    assert_eq!(
        insert,
        "INSERT INTO \"clientes\" (\"id\",\"nome\",\"ativo\") VALUES \
             (2,E'D''Avila',FALSE) ON CONFLICT (\"id\") DO UPDATE SET \
             \"nome\"=EXCLUDED.\"nome\",\"ativo\"=EXCLUDED.\"ativo\""
    );
    // O `t` do fio chegou como verdadeiro, e nao como recusa nem falso.
    let local = varrer(&s);
    assert!(local.contains("\"remoto\""), "{local}");
    assert!(local.contains("\"ativo\":true"), "{local}");
}

/// **584, BLOB pelo MySQL(R):** o `SELECT *` manda o BLOB cru, e o puxar
/// o passava pelo `hex_para_bytes`. Os bytes `cafe` viravam os DOIS bytes
/// `CA FE`, calados; os bytes `00 FF 80` recusavam a rodada.
///
/// # Prova real
///
/// Com a leitura por `SELECT *` de volta, a primeira rodada grava `cafe`
/// como os dois bytes `CA FE` (o dado errado CALADO) e a segunda recusa na
/// linha 2. O teste falha nas duas, cada uma com a sua mensagem.
#[test]
fn o_blob_do_mysql_chega_byte_a_byte() {
    let cols: Vec<ColMy> = vec![
        ("id", 0x03, 0x0003, 11, 63),
        ("foto", 0xfc, 0x0090, 65_535, 63),
    ];
    let so_cafe = vec![vec![cel("1"), Some(b"cafe".to_vec())]];
    let todas = vec![
        vec![cel("1"), Some(b"cafe".to_vec())],
        vec![cel("2"), Some(vec![0x00, 0xFF, 0x80])],
        vec![cel("3"), Some(Vec::new())],
        vec![cel("4"), None],
    ];
    for linhas in [so_cafe, todas] {
        let n = linhas.len();
        let dir = DirTemp::novo("584-dblink-blob-my");
        let s = servidor(&dir);
        let (porta, _avisos) = par_my(cols.clone(), linhas);
        salvar(&s, "mysql", porta);
        ligar(&s, "puxar").unwrap();
        let r = match pede(&s, r#""op":"dblink_sincronizar","dblink":"erp""#) {
            Ok(j) => j.escrever(),
            Err(e) => panic!("o puxar recusou um BLOB: {e}"),
        };
        assert!(r.contains(&format!("\"puxadas_novas\":{n}")), "{r}");
        let local = varrer(&s);
        assert!(
            local.contains("\"63616665\""),
            "cafe nao chegou como 4 bytes: {local}"
        );
        if n > 1 {
            assert!(local.contains("\"00ff80\""), "{local}");
        }
    }
}

/// **584, bytea pelo PostgreSQL(R):** o texto do `bytea` vem com `\x` na
/// frente, e o `hex_para_bytes` o recusava -- todo bytea parava a rodada.
#[test]
fn o_bytea_do_postgres_chega_byte_a_byte() {
    let dir = DirTemp::novo("584-dblink-bytea-pg");
    let s = servidor(&dir);
    let cols = vec![
        col_pg("id", 23, "integer", true),
        col_pg("foto", 17, "bytea", false),
    ];
    let linhas = vec![
        vec![cel("1"), Some(vec![0x00, 0xFF, 0x80])],
        vec![cel("2"), Some(b"cafe".to_vec())],
    ];
    let (porta, avisos) = par_pg(cols, linhas);
    salvar(&s, "postgres", porta);
    ligar(&s, "dois").unwrap();
    let r = match pede(&s, r#""op":"dblink_sincronizar","dblink":"erp""#) {
        Ok(j) => j.escrever(),
        Err(e) => panic!("o puxar recusou um bytea: {e}"),
    };
    assert!(r.contains("\"puxadas_novas\":2"), "{r}");
    let local = varrer(&s);
    assert!(
        local.contains("\"00ff80\"") && local.contains("\"63616665\""),
        "{local}"
    );
    // E o empurrao do bytea sobe sem depender do `bytea_output` nem do
    // `standard_conforming_strings`: `decode('..','hex')`.
    pede(
        &s,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":3,"foto":"0a0b"}"#,
    )
    .unwrap();
    pede(&s, r#""op":"dblink_sincronizar","dblink":"erp""#).unwrap();
    let insert = avisos
        .try_iter()
        .find(|q| q.starts_with("INSERT"))
        .expect("o par nao recebeu o INSERT");
    assert!(insert.contains("(3,decode('0a0b','hex'))"), "{insert}");
}

/// **584, uuid:** a celula remota `novo` (ou `v4`, `v7`) numa coluna local
/// Uuid virava um uuid ALEATORIO -- o gerador da carga colada respondendo
/// por um dado que veio de outro banco, um diferente a cada rodada. Agora
/// recusa sem citar a celula; e o uuid de verdade chega igual.
#[test]
fn o_uuid_remoto_nao_vira_uuid_inventado() {
    let cols: Vec<ColMy> = vec![("id", 0x03, 0x0003, 11, 63), ("cod", 0xfe, 0, 144, 45)];
    let u = "0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b";
    for (celula, vale) in [(u, true), ("novo", false), ("v4", false), ("v7", false)] {
        let dir = DirTemp::novo("584-dblink-uuid");
        let s = servidor(&dir);
        let (porta, _avisos) = par_my(cols.clone(), vec![vec![cel("1"), cel(celula)]]);
        salvar(&s, "mysql", porta);
        // A tabela local ja existe, com a coluna Uuid: e o caminho em que
        // um uuid local recebe a celula de la.
        pede(&s, TABELA_COM_UUID).unwrap();
        ligar(&s, "puxar").unwrap();
        let r = pede(&s, r#""op":"dblink_sincronizar","dblink":"erp""#);
        match (r, vale) {
            (Ok(_), true) => assert!(varrer(&s).contains(u), "{}", varrer(&s)),
            (Err(e), true) => panic!("o uuid de verdade recusou: {e}"),
            (Ok(_), false) => panic!(
                "a celula {celula:?} de la virou um uuid inventado aqui: {}",
                varrer(&s)
            ),
            (Err(e), false) => {
                let e = e.to_string();
                assert!(e.contains("\"cod\"") && !e.contains(celula), "{e}");
            }
        }
    }
}
// ------------------------------------------ pedido 590: a celula exibida

/// As celulas de uma coluna, pelo NOME, na resposta da grade -- e a marca
/// `binario` dela. Ler por nome, e nao por posicao, porque a tela le assim.
fn celulas_de(r: &Json, nome: &str) -> (Vec<Json>, bool) {
    let cols = r.campo("colunas").and_then(Json::lista).unwrap();
    let i = cols
        .iter()
        .position(|c| c.texto_ou("nome", "") == nome)
        .unwrap_or_else(|| panic!("sem a coluna {nome:?}: {}", r.escrever()));
    let binario = cols[i].booleano_ou("binario", false);
    let linhas = r.campo("linhas").and_then(Json::lista).unwrap();
    let v = linhas
        .iter()
        .map(|l| l.lista().unwrap()[i].clone())
        .collect();
    (v, binario)
}

/// Confere a resposta das duas ops da tela contra o que esta la: o
/// binario em hexadecimal minusculo (a forma do BLOB daqui), NULL como
/// NULL, e o texto UTF-8 -- acento e travessao -- intacto.
fn confere_a_grade(op: &str, r: &Json) {
    let texto = r.escrever();
    assert!(!texto.contains('\u{FFFD}'), "{op}: U+FFFD na tela: {texto}");
    let (foto, bin) = celulas_de(r, "foto");
    assert!(bin, "{op}: a coluna binaria nao veio marcada: {texto}");
    assert_eq!(
        foto,
        vec![
            Json::texto_de("00ff80"),
            Json::texto_de("63616665"),
            Json::texto_de(""),
            Json::Nulo
        ],
        "{op}: a celula exibida nao e o dado"
    );
    let (nome, bin) = celulas_de(r, "nome");
    assert!(!bin, "{op}: texto marcado como binario");
    assert_eq!(nome[0], Json::texto_de("São Paulo — ação"), "{op}");
    assert_eq!(nome[1], Json::texto_de("cafe"), "{op}: o texto virou hex");
}

fn linhas_590() -> Celulas {
    vec![
        vec![
            cel("1"),
            Some(vec![0x00, 0xFF, 0x80]),
            cel("São Paulo — ação"),
        ],
        vec![cel("2"), Some(b"cafe".to_vec()), cel("cafe")],
        vec![cel("3"), Some(Vec::new()), cel("")],
        vec![cel("4"), None, None],
    ]
}

/// **590, MySQL(R):** o `dblink_ler` e o `dblink_consultar` mostravam o
/// BLOB pelo `from_utf8_lossy` -- `00 FF 80` virava `\0` e dois `U+FFFD`,
/// e `cafe` aparecia como a palavra. O conjunto `binary` (63) da definicao
/// da coluna decide; o texto -- inclusive o `_bin`, que acende a mesma
/// bandeira 0x80 -- continua texto.
///
/// # Prova real
///
/// Com o leitor do fio de volta ao `texto_lenenc` para toda coluna, as
/// duas ops falham no `U+FFFD`; com a decisao pela bandeira 0x80 em vez do
/// conjunto, o `apelido` em `utf8mb4_bin` vira hexadecimal e falha.
#[test]
fn o_blob_do_mysql_aparece_na_tela_como_o_dado() {
    let dir = DirTemp::novo("590-dblink-tela-my");
    let s = servidor(&dir);
    let cols: Vec<ColMy> = vec![
        ("id", 0x03, 0x0083, 11, 63),
        ("foto", 0xfc, 0x0090, 65_535, 63),
        ("nome", 0xfd, 0, 160, 45),
    ];
    let mut linhas = linhas_590();
    // Uma quarta coluna em `utf8mb4_bin`: bandeira BINARY acesa, texto.
    let mut cols = cols;
    cols.push(("apelido", 0xfd, 0x0080, 160, 45));
    for l in &mut linhas {
        l.push(cel("Blumenau ç"));
    }
    let (porta, _avisos) = par_my(cols, linhas);
    salvar(&s, "mysql", porta);
    for (op, pedido) in [
        (
            "dblink_ler",
            r#""op":"dblink_ler","dblink":"erp","tabela":"clientes""#,
        ),
        (
            "dblink_consultar",
            r#""op":"dblink_consultar","dblink":"erp","sql":"SELECT * FROM clientes""#,
        ),
    ] {
        let r = pede(&s, pedido).unwrap_or_else(|e| panic!("{op}: {e}"));
        confere_a_grade(op, &r);
        let (apelido, bin) = celulas_de(&r, "apelido");
        assert!(!bin, "{op}: a colacao _bin nao e binario");
        assert_eq!(apelido[0], Json::texto_de("Blumenau ç"), "{op}");
        let (id, bin) = celulas_de(&r, "id");
        assert!(!bin && id[0] == Json::texto_de("1"), "{op}: {id:?}");
    }
}

/// **590, PostgreSQL(R):** o `bytea` aparecia no texto do fio -- `\x00ff80`
/// no `bytea_output = hex`, e `\000\377\200` no `escape` --, uma forma
/// que muda com a configuracao do outro servidor e nao e a do BLOB daqui.
///
/// # Prova real
///
/// Com o leitor de volta ao `from_utf8_lossy` para toda coluna, a celula
/// chega `\x00ff80` e o teste falha nas duas ops.
#[test]
fn o_bytea_do_postgres_aparece_na_tela_como_o_dado() {
    let dir = DirTemp::novo("590-dblink-tela-pg");
    let s = servidor(&dir);
    let cols = vec![
        col_pg("id", 23, "integer", true),
        col_pg("foto", 17, "bytea", false),
        col_pg("nome", 1043, "character varying(40)", false),
    ];
    let (porta, _avisos) = par_pg(cols, linhas_590());
    salvar(&s, "postgres", porta);
    for (op, pedido) in [
        (
            "dblink_ler",
            r#""op":"dblink_ler","dblink":"erp","tabela":"clientes""#,
        ),
        (
            "dblink_consultar",
            r#""op":"dblink_consultar","dblink":"erp","sql":"SELECT * FROM clientes""#,
        ),
    ] {
        let r = pede(&s, pedido).unwrap_or_else(|e| panic!("{op}: {e}"));
        confere_a_grade(op, &r);
    }
}

// ------------------------------- pedido 592: o espelho decide como a tela

/// A coluna local, pelo nome, no esquema que o `dblink_ligar` criou.
fn tipo_local_de(s: &Arc<Servidor>, coluna: &str) -> String {
    let r = pede(s, r#""op":"esquema","database":"loja","tabela":"clientes""#)
        .unwrap()
        .escrever();
    let marca = format!("\"nome\":\"{coluna}\"");
    let i = r
        .find(&marca)
        .unwrap_or_else(|| panic!("sem a coluna {coluna:?}: {r}"));
    let resto = &r[i..];
    let fim = resto.find('}').unwrap_or(resto.len());
    resto[..fim].to_string()
}

/// **592:** o `dblink_ligar` decidia binario pela bandeira 0x80, que acende
/// tambem num `VARCHAR` em `utf8mb4_bin` (conjunto 46): a tabela local
/// nascia com a coluna `Bin` e a leitura pedia `HEX()` -- texto gravado
/// como bytes. E o `BIT(16)`, que o leitor do 590 ja entrega em hex, virava
/// `Int8` pelo `valor_de_texto`, que le os digitos hex como DECIMAIS: o
/// `0x0110` (272) gravava 110, calado.
///
/// # Prova real
///
/// Com a decisao do `nome_do_tipo` pela bandeira de volta, o `apelido`
/// nasce `Bin` e falha; com a leitura do BIT sem o `CAST`, o `bits` grava
/// 110 (ou recusa em `00ff`) e falha.
#[test]
fn o_espelho_do_mysql_decide_binario_pelo_conjunto_e_le_o_bit_inteiro() {
    let dir = DirTemp::novo("592-dblink-bin");
    let s = servidor(&dir);
    let cols: Vec<ColMy> = vec![
        ("id", 0x03, 0x0003, 11, 63),
        ("apelido", 0xfd, 0x0080, 160, 46),
        ("bits", 0x10, 0x00a0, 16, 63),
    ];
    let linhas = vec![
        vec![cel("1"), cel("Blumenau ç"), Some(vec![0x01, 0x10])],
        vec![cel("2"), cel("cafe"), Some(vec![0x00, 0xff])],
        vec![cel("3"), cel(""), Some(vec![0x00, 0x00])],
        vec![cel("4"), None, None],
    ];
    let (porta, avisos) = par_my(cols, linhas);
    salvar(&s, "mysql", porta);
    ligar(&s, "puxar").unwrap();
    let apelido = tipo_local_de(&s, "apelido");
    assert!(
        apelido.contains("\"tipo\":\"Str(40)\""),
        "a colacao _bin nasceu binaria aqui: {apelido}"
    );
    let r = match pede(&s, r#""op":"dblink_sincronizar","dblink":"erp""#) {
        Ok(j) => j.escrever(),
        Err(e) => panic!("o puxar recusou: {e}"),
    };
    assert!(r.contains("\"puxadas_novas\":4"), "{r}");
    let recebidas: Vec<String> = avisos.try_iter().collect();
    let leitura = recebidas
        .iter()
        .find(|q| !q.contains("LIMIT 0") && q.starts_with("SELECT"))
        .expect("o par nao recebeu a leitura");
    assert!(!leitura.contains("HEX(`apelido`)"), "{leitura}");
    let local = varrer(&s);
    for esperado in [
        "\"Blumenau ç\"",
        "\"cafe\"",
        "\"bits\":272",
        "\"bits\":255",
        "\"bits\":0",
    ] {
        assert!(local.contains(esperado), "falta {esperado}: {local}");
    }
}

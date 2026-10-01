//! A imagem da linha no diario e decisao do SERVIDOR, tomada num lugar so --
//! pedidos 564, 416 e 517, e o defeito da exclusao fisica do multi. Provado
//! PELO SOQUETE, com `phxsqld` de verdade dos dois lados.
//!
//! # Por que processos, e nao `Servidor` dentro do teste
//!
//! Porque o 564 precisa de um ARRANQUE: a marca de commit orfa so se completa
//! quando o servidor sobe, e um `Servidor` em processo nao tem como parar --
//! o laco de replicacao dele seguiria vivo depois do teste. O `Filho` mata o
//! `phxsqld` no `Drop`, inclusive quando a asercao falha no meio.
//!
//! # O que cada teste trava
//!
//! 1. **564** -- a recuperacao do arranque grava o evento COM imagem, e a
//!    replica o aplica. Antes, a recuperacao abria a tabela por fora do
//!    `abrir_travada`, o evento ia sem imagem e a replica parava em «veio sem
//!    imagem».
//! 2. **416** -- a exclusao que chega de OUTRA origem num rowid que guarda
//!    outra linha PARA, nomeando os dois carimbos, em vez de apagar a linha
//!    errada com `Ok`.
//! 3. **416, comportamento velho** -- a replica fiel de uma origem so aplica
//!    a exclusao como sempre, agora com a imagem no evento.
//! 4. **Exclusao fisica no multi** -- a exclusao pela porta chega ao parceiro.
//!    Antes ela ia sem imagem e o parceiro parava o par.
//! 5. **517** -- indice unico sobre coluna que aceita nulo nao vale como
//!    identidade no bidirecional: a tabela e recusada dizendo por que, e a
//!    linha de chave nula de um no nao apaga a do outro.
//! 6. **601** -- duas caixas recem-nascidas empatam o carimbo; a LINHAGEM da
//!    tabela (`PSCH` v11) recusa a exclusao de outra historia mesmo assim.

#![cfg(unix)]

mod comum;
use comum::{porta_do_phxsqld, DirTemp, Filho};

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_server::transacao::{gravar_marca, Acao, Escrita};
use phxsql_store::catalogo::Instancia;

const TOKEN: &str = "imagem-pela-politica";
/// Teto de espera de qualquer condicao: o laco de replicacao roda a cada
/// segundo, e vinte voltas dele bastam com folga.
const ESPERA: Duration = Duration::from_secs(20);

/// Um `phxsqld` no ar. Morre no `Drop` do `Filho`.
struct No {
    _filho: Filho,
    porta: u16,
}

/// Sobe um `phxsqld` com a base `base` e o bloco `replicacao` dado.
///
/// A cifra do fio vai desligada de proposito (o escape escrito dos outros
/// testes de replicacao): o que se mede aqui e a imagem no diario, e a recusa
/// lida seria a da cifra.
fn subir(dir: &DirTemp, nome: &str, replicacao: &str) -> No {
    let raiz = dir.join(nome);
    std::fs::create_dir_all(&raiz).unwrap();
    let config = raiz.join("config.json");
    std::fs::write(
        &config,
        format!(
            r#"{{
                "bind": "127.0.0.1:0",
                "token": "{TOKEN}",
                "base": {base:?},
                "log_acessos": {log:?},
                "cifra_fio": {{ "exigir": false }},
                "web": {{ "ligado": false }},
                "replicacao": {replicacao}
            }}"#,
            base = raiz.join("dados").display().to_string(),
            log = raiz.join("acessos.log").display().to_string(),
        ),
    )
    .unwrap();
    let erro_padrao = raiz.join("stderr.txt");
    let mut filho = Filho(
        Command::new(env!("CARGO_BIN_EXE_phxsqld"))
            .arg("--config")
            .arg(&config)
            .current_dir(&raiz)
            .stdout(Stdio::null())
            .stderr(Stdio::from(std::fs::File::create(&erro_padrao).unwrap()))
            .spawn()
            .unwrap(),
    );
    let porta = porta_do_phxsqld(&mut filho, &erro_padrao).unwrap_or_else(|e| panic!("{e}"));
    No {
        _filho: filho,
        porta,
    }
}

/// Uma origem que nunca responde, para o papel que exige ao menos uma e nao
/// deve puxar nada no cenario: porta fechada, database que ninguem tem.
fn origem_muda() -> String {
    format!(
        r#"[{{"nome":"muda","host":"127.0.0.1","porta":{},"token":"{TOKEN}",
             "databases":["ninguem"],"reconectar_em":1}}]"#,
        comum::porta_fechada()
    )
}

/// O bloco `origens` que puxa `loja` de `porta`.
fn origem(nome: &str, porta: u16) -> String {
    format!(
        r#"[{{"nome":"{nome}","host":"127.0.0.1","porta":{porta},"token":"{TOKEN}",
             "databases":["loja"],"reconectar_em":1}}]"#
    )
}

fn pedir(porta: u16, corpo: &str) -> Json {
    let linha = format!("{{\"token\":\"{TOKEN}\",{}}}", corpo.replace('\n', " "));
    let resposta = comum::pedir(porta, &linha);
    Json::analisar(&resposta)
        .unwrap_or_else(|e| panic!("{corpo}: resposta ilegivel ({e}): {resposta}"))
}

fn exigir(porta: u16, corpo: &str) -> Json {
    let r = pedir(porta, corpo);
    assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    r.campo("resultado").cloned().unwrap_or(Json::Nulo)
}

/// Espera `f` virar verdade, com teto de voltas: nenhum laco de teste aqui
/// gira sem fim.
fn esperar<F: Fn() -> bool>(o_que: &str, f: F) {
    let ate = Instant::now() + ESPERA;
    while Instant::now() < ate {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("{o_que} nao aconteceu em {} s", ESPERA.as_secs());
}

/// As linhas de `loja.clientes` como (id, nome), na ordem do `.reg`.
fn linhas(porta: u16) -> Vec<(i64, String)> {
    let r = pedir(
        porta,
        r#""op":"varrer","database":"loja","tabela":"clientes","max":100"#,
    );
    r.campo("resultado")
        .and_then(|x| x.campo("linhas"))
        .and_then(Json::lista)
        .unwrap_or(&[])
        .iter()
        .map(|l| (l.inteiro_ou("id", -1), l.texto_ou("nome", "").to_string()))
        .collect()
}

/// O estado da origem `nome` em `replicacao_estado`.
fn estado_da_origem(porta: u16, nome: &str) -> Json {
    exigir(porta, r#""op":"replicacao_estado""#)
        .campo("origens")
        .and_then(|o| o.campo(nome))
        .cloned()
        .unwrap_or(Json::Nulo)
}

/// Os eventos do diario de `loja.clientes`, pelo `replicar` de quem os gravou.
fn diario(porta: u16) -> Vec<Json> {
    exigir(
        porta,
        r#""op":"replicar","database":"loja","tabela":"clientes","desde":0"#,
    )
    .campo("eventos")
    .and_then(Json::lista)
    .map(<[Json]>::to_vec)
    .unwrap_or_default()
}

const CLIENTES: &str = r#""op":"criar_tabela","database":"loja","tabela":"clientes",
   "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
              {"nome":"nome","tipo":"Str(20)"}],
   "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#;

fn criar_clientes(porta: u16) {
    exigir(porta, r#""op":"criar_database","database":"loja""#);
    exigir(porta, CLIENTES);
}

fn inserir(porta: u16, id: i64, nome: &str) {
    exigir(
        porta,
        &format!(
            r#""op":"inserir","database":"loja","tabela":"clientes",
               "linha":{{"id":{id},"nome":"{nome}"}}"#
        ),
    );
}

fn excluir_fisico(porta: u16, rowid: u64) -> Json {
    pedir(
        porta,
        &format!(
            r#""op":"excluir","database":"loja","tabela":"clientes","rowid":{rowid},
               "fisico":true,"motivo":"teste""#
        ),
    )
}

const SOURCE: &str = r#"{"papel":"source","id_servidor":"S","imagem_da_linha":true}"#;

// ---------------------------------------------------------------------------
// 1. Pedido 564 -- a recuperacao grava com imagem
// ---------------------------------------------------------------------------

/// **Prova real do 564.** Um COMMIT morreu depois da marca e antes da
/// passada: em disco ficam a tabela vazia e a marca `.tx` com a insercao. O
/// source sobe, a recuperacao completa a marca, e a replica puxa o evento.
///
/// O estado pos-queda se monta pelo caminho que continua fiel: a marca e
/// gravada pelo mesmo `gravar_marca` do `COMMIT`, com o servidor ainda fora
/// do ar -- byte a byte o que a queda deixaria.
///
/// **Defeito reposto**: tirar a politica do diario do `Database` (a
/// recuperacao volta a abrir a tabela com o padrao, sem imagem). O evento
/// completado sai sem imagem, a replica para em «veio sem imagem» e a linha
/// nunca chega -- o `esperar` estoura.
#[test]
fn a_recuperacao_grava_com_imagem_e_a_replica_aplica() {
    let dir = DirTemp::novo("564");
    let base = dir.join("source").join("dados");
    {
        let inst = Instancia::nova(&base).unwrap();
        let db = inst.criar_database("loja").unwrap();
        let e = Schema::new(
            "clientes",
            vec![
                Column::new("id", ColumnType::Int4).obrigatoria(),
                Column::new("nome", ColumnType::Str(20)),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap();
        drop(db.criar_tabela(None, e).unwrap());
        gravar_marca(
            db.caminho(),
            1,
            0,
            &[Escrita {
                database: "loja".into(),
                tabela: "clientes".into(),
                acao: Acao::Inserir,
                rowid: 1,
                linha: vec![Value::Int(1), Value::Str("Ana".into())],
                linha_antiga: Vec::new(),
                motivo: String::new(),
                cascata_na_lista: false,
                elo_do_empilhar: false,
                elo_da_cascata: false,
            }],
        )
        .unwrap();
    }
    let s = subir(&dir, "source", SOURCE);
    // A premissa: a recuperacao completou a marca no arranque.
    assert_eq!(linhas(s.porta), vec![(1, "Ana".to_string())]);
    let ev = diario(s.porta);
    assert_eq!(ev.len(), 1, "a recuperacao gerou {} evento(s)", ev.len());
    assert!(
        !ev[0].texto_ou("imagem", "").is_empty(),
        "o evento completado pela recuperacao saiu SEM imagem: {}",
        ev[0].escrever()
    );

    let r = subir(
        &dir,
        "replica",
        &format!(
            r#"{{"papel":"replica","id_servidor":"R","imagem_da_linha":true,
                "origens":{}}}"#,
            origem("fonte", s.porta)
        ),
    );
    esperar("a linha recuperada chegar a replica", || {
        linhas(r.porta) == vec![(1, "Ana".to_string())]
    });
    assert_eq!(
        estado_da_origem(r.porta, "fonte").texto_ou("ultimo_erro", ""),
        "",
        "a replica aplicou e mesmo assim guardou erro"
    );
}

// ---------------------------------------------------------------------------
// 2 e 3. Pedido 416 -- a exclusao replicada confere o carimbo
// ---------------------------------------------------------------------------

/// Empurra para `destino` o evento `i` do diario de `origem`, pelo `aplicar`
/// -- o mesmo caminho de quem replica por fora, e o que deixa DUAS origens
/// escreverem no mesmo database de um no so.
fn aplicar(destino: u16, evento: &Json) -> Json {
    exigir(
        destino,
        &format!(
            r#""op":"aplicar","database":"loja","tabela":"clientes","eventos":[{}]"#,
            evento.escrever()
        ),
    )
}

/// **Prova real do 416.** Duas origens, cada uma com o proprio `.reg`: os
/// rowids das duas comecam em 1, e por isso colidem sem ninguem errar nada.
/// A da origem A chega ao destino; a origem B grava a linha DELA no rowid 1 e
/// a apaga. Essa exclusao chega ao destino num rowid que guarda a linha de A.
///
/// **Defeito reposto (a)**: tirar o `imagem_na_exclusao` da politica do
/// diario -- o evento de B sai sem imagem, nao ha o que conferir, e a linha
/// de A some com `Ok`. **(b)**: tirar a conferencia do braco `Exclusao` do
/// `aplicar_evento_interno` -- mesma queda, com a imagem presente.
#[test]
fn a_exclusao_de_outra_origem_para_em_vez_de_apagar_a_linha_errada() {
    let dir = DirTemp::novo("416-duas");
    let a = subir(&dir, "a", SOURCE);
    let b = subir(
        &dir,
        "b",
        r#"{"papel":"source","id_servidor":"B","imagem_da_linha":true}"#,
    );
    // O `aplicar` so e aceito por quem existe para receber replicacao.
    let destino = subir(
        &dir,
        "destino",
        &format!(
            r#"{{"papel":"replica","id_servidor":"D","origens":{}}}"#,
            origem_muda()
        ),
    );
    for p in [a.porta, b.porta, destino.porta] {
        criar_clientes(p);
    }

    inserir(a.porta, 1, "da caixa A");
    let r = aplicar(destino.porta, &diario(a.porta)[0]);
    assert_eq!(r.inteiro_ou("aplicados", -1), 1, "{}", r.escrever());

    // O carimbo de criacao e um contador do PROCESSO, e dois `phxsqld` recem
    // nascidos emitem os mesmos numeros: a conferencia PEGA divergencia, nao
    // prova acordo (ver `Table::conferir_identidade`). Duas caixas de verdade
    // tem historias diferentes; aqui a historia de B anda cinco linhas numa
    // tabela vizinha antes, para o carimbo da linha dela nao coincidir com o
    // de A por acaso de arranque.
    exigir(
        b.porta,
        r#""op":"criar_tabela","database":"loja","tabela":"vizinha",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}]"#,
    );
    for id in 1..=5 {
        exigir(
            b.porta,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"vizinha","linha":{{"id":{id}}}"#
            ),
        );
    }
    inserir(b.porta, 500, "da caixa B");
    let x = excluir_fisico(b.porta, 1);
    assert!(x.booleano_ou("ok", false), "{}", x.escrever());
    let ev = diario(b.porta);
    assert_eq!(ev[1].texto_ou("operacao", ""), "exclusao");

    let r = aplicar(destino.porta, &ev[1]);
    let erro = r.texto_ou("erro", "").to_string();
    assert_eq!(r.inteiro_ou("aplicados", -1), 0, "{}", r.escrever());
    for pedaco in ["divergiu", "clientes", "rowid 1", "carimbo"] {
        assert!(erro.contains(pedaco), "a recusa nao diz {pedaco:?}: {erro}");
    }
    // O que mais importa: a linha de A continua la.
    assert_eq!(linhas(destino.porta), vec![(1, "da caixa A".to_string())]);
}

/// **O comportamento VELHO, que nao pode mudar.** A replica fiel de uma
/// origem so aplica a exclusao como sempre aplicou -- e o evento dela, agora,
/// carrega a imagem, o que e a metade «ligar a imagem fora do multi» do 416.
///
/// **Defeito reposto**: com a conferencia comparando carimbos que nao vieram
/// da mesma linha (por exemplo, recusando quando a imagem existe), a exclusao
/// para aqui e a linha 2 nunca sai da replica.
#[test]
fn a_replica_fiel_aplica_a_exclusao_com_imagem() {
    let dir = DirTemp::novo("416-fiel");
    let s = subir(&dir, "source", SOURCE);
    criar_clientes(s.porta);
    for id in 1..=3 {
        inserir(s.porta, id, &format!("cliente {id}"));
    }
    let r = subir(
        &dir,
        "replica",
        &format!(
            r#"{{"papel":"replica","id_servidor":"R","imagem_da_linha":true,
                "origens":{}}}"#,
            origem("fonte", s.porta)
        ),
    );
    esperar("as tres linhas na replica", || linhas(r.porta).len() == 3);

    let x = excluir_fisico(s.porta, 2);
    assert!(x.booleano_ou("ok", false), "{}", x.escrever());
    let ev = diario(s.porta);
    assert_eq!(ev[3].texto_ou("operacao", ""), "exclusao");
    assert!(
        !ev[3].texto_ou("imagem", "").is_empty(),
        "a exclusao do source saiu sem imagem: {}",
        ev[3].escrever()
    );
    esperar("a exclusao chegar a replica", || {
        linhas(r.porta).iter().map(|l| l.0).collect::<Vec<_>>() == vec![1, 3]
    });
    assert_eq!(
        estado_da_origem(r.porta, "fonte").texto_ou("ultimo_erro", ""),
        ""
    );
}

// ---------------------------------------------------------------------------
// 4. A exclusao fisica pela porta, no papel multi
// ---------------------------------------------------------------------------

fn multi(id: &str, origens: &str) -> String {
    format!(
        r#"{{"papel":"multi","id_servidor":"{id}","imagem_da_linha":true,
            "origens":{origens}}}"#
    )
}

/// **Prova real do defeito novo (lido pelo papel J, medido aqui).** No multi,
/// a exclusao fisica pela porta gravava o evento SEM imagem: o
/// `abrir_travada` ligava so a imagem da linha, e a da exclusao so o
/// aplicador ligava. O outro lado recebia exclusao sem chave, o
/// `aplicar_por_chave` devolvia erro, e o par parava naquela tabela.
///
/// So o alfa puxa, pelo mesmo motivo do `laco-do-unico-secundario.rs`: os
/// dois puxando, o cenario vira corrida.
///
/// **Defeito reposto**: tirar o `imagem_na_exclusao` da politica do diario.
/// A linha 2 nunca sai do alfa, e o `esperar` estoura.
#[test]
fn no_multi_a_exclusao_fisica_pela_porta_chega_ao_parceiro() {
    let dir = DirTemp::novo("multi-excl");
    let beta = subir(&dir, "beta", &multi("beta", &origem_muda()));
    criar_clientes(beta.porta);
    inserir(beta.porta, 1, "um");
    inserir(beta.porta, 2, "dois");
    let alfa = subir(
        &dir,
        "alfa",
        &multi("alfa", &origem("parceiro", beta.porta)),
    );
    esperar("as duas linhas no alfa", || linhas(alfa.porta).len() == 2);

    let x = excluir_fisico(beta.porta, 2);
    assert!(x.booleano_ou("ok", false), "{}", x.escrever());
    esperar("a exclusao chegar ao alfa", || {
        linhas(alfa.porta) == vec![(1, "um".to_string())]
    });
    assert_eq!(
        estado_da_origem(alfa.porta, "parceiro").texto_ou("ultimo_erro", ""),
        "",
        "o par guardou erro mesmo replicando"
    );
}

// ---------------------------------------------------------------------------
// 5. Pedido 517 -- chave unica anulavel nao e identidade
// ---------------------------------------------------------------------------

const ANULAVEL: &str = r#""op":"criar_tabela","database":"loja","tabela":"clientes",
   "colunas":[{"nome":"codigo","tipo":"Str(10)"},
              {"nome":"nome","tipo":"Str(20)"}],
   "indices":[{"nome":"porCodigo","colunas":["codigo"],"unico":true}]"#;

fn nomes(porta: u16) -> Vec<String> {
    linhas(porta).into_iter().map(|l| l.1).collect()
}

/// **Prova real do 517.** O unico indice unico da tabela e sobre uma coluna
/// que aceita nulo. NULL nao colide com NULL na gravacao, entao cada no pode
/// ter a sua linha de codigo nulo -- e casar por essa chave faria a exclusao
/// de um apagar a do outro. A tabela passa a ser RECUSADA no bidirecional,
/// com o motivo dizendo qual coluna tornar obrigatoria.
///
/// **Defeito reposto**: tirar o `!nullable` do `serve` de
/// `bidirecional::chave_unica`. A recusa nao aparece e o `esperar` estoura.
#[test]
fn a_chave_unica_anulavel_nao_vale_como_identidade() {
    let dir = DirTemp::novo("517");
    let beta = subir(&dir, "beta", &multi("beta", &origem_muda()));
    exigir(beta.porta, r#""op":"criar_database","database":"loja""#);
    exigir(beta.porta, ANULAVEL);
    exigir(
        beta.porta,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"nome":"do beta"}"#,
    );
    let alfa = subir(
        &dir,
        "alfa",
        &multi("alfa", &origem("parceiro", beta.porta)),
    );
    let recusa = || {
        estado_da_origem(alfa.porta, "parceiro")
            .campo("recusas")
            .and_then(|r| r.campo("loja/clientes"))
            .and_then(Json::texto)
            .map(str::to_string)
    };
    esperar("a recusa da tabela de chave anulavel", || {
        recusa().is_some()
    });
    let motivo = recusa().unwrap();
    for pedaco in ["codigo", "nulo", "obrigatoria"] {
        assert!(
            motivo.contains(pedaco),
            "o motivo nao diz {pedaco:?}: {motivo}"
        );
    }

    // A linha de chave nula do alfa sobrevive a exclusao da do beta.
    exigir(
        alfa.porta,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"nome":"do alfa"}"#,
    );
    let x = excluir_fisico(beta.porta, 1);
    assert!(x.booleano_ou("ok", false), "{}", x.escrever());
    // Tres voltas do laco: tempo de sobra para a exclusao chegar, se fosse.
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(nomes(alfa.porta), vec!["do alfa".to_string()]);
}

// ---------------------------------------------------------------------------
// 6. Pedido 601 -- a linhagem da tabela fecha o empate do carimbo
// ---------------------------------------------------------------------------

/// O `replicar` inteiro de `loja.clientes`: os eventos E a linhagem.
fn replicar_tudo(porta: u16) -> Json {
    exigir(
        porta,
        r#""op":"replicar","database":"loja","tabela":"clientes","desde":0"#,
    )
}

/// **Prova real do 601 (segunda parte).** O buraco que o 416 deixou nomeado:
/// duas caixas RECEM-NASCIDAS, sem historia nenhuma antes, emitem o mesmo
/// carimbo `1` -- e a conferencia do `rowstamp` ve `1 == 1` e deixa a
/// exclusao de B apagar a linha de A. Aqui nao ha tabela vizinha andando o
/// contador de B: e exatamente o empate que a cognicao de 01/10 mediu.
///
/// O destino e a replica FIEL de A -- a tabela dele nasceu do bloco de
/// esquema de A, com a linhagem de A. O evento de B chega pelo `aplicar` com
/// a linhagem de B, que o `replicar` de B entrega.
///
/// **Defeito reposto**: o `aplicar` ignorando o campo `linhagem` (ou o
/// `Schema::new` sem cunhar) -- a linha de A some com `aplicados: 1`.
#[test]
fn a_linhagem_recusa_a_exclusao_de_outra_historia_com_o_carimbo_empatado() {
    let dir = DirTemp::novo("601-linhagem");
    let a = subir(&dir, "a", SOURCE);
    let b = subir(
        &dir,
        "b",
        r#"{"papel":"source","id_servidor":"B","imagem_da_linha":true}"#,
    );
    criar_clientes(a.porta);
    criar_clientes(b.porta);
    inserir(a.porta, 1, "da caixa A");
    let destino = subir(
        &dir,
        "destino",
        &format!(
            r#"{{"papel":"replica","id_servidor":"D","imagem_da_linha":true,
                "origens":{}}}"#,
            origem("fonte", a.porta)
        ),
    );
    esperar("a linha de A na replica", || {
        linhas(destino.porta) == vec![(1, "da caixa A".to_string())]
    });

    let de_a = replicar_tudo(a.porta).texto_ou("linhagem", "").to_string();
    let no_destino = replicar_tudo(destino.porta)
        .texto_ou("linhagem", "")
        .to_string();
    assert!(!de_a.is_empty(), "o replicar de A nao entrega a linhagem");
    assert_eq!(
        no_destino, de_a,
        "a replica fiel nasceu com outra linhagem que a do source"
    );

    inserir(b.porta, 500, "da caixa B");
    let x = excluir_fisico(b.porta, 1);
    assert!(x.booleano_ou("ok", false), "{}", x.escrever());
    let de_b = replicar_tudo(b.porta);
    let linhagem_b = de_b.texto_ou("linhagem", "").to_string();
    assert_ne!(
        linhagem_b, de_a,
        "duas caixas por conta com a mesma linhagem"
    );
    let ev = de_b
        .campo("eventos")
        .and_then(Json::lista)
        .map(<[Json]>::to_vec)
        .unwrap_or_default();
    assert_eq!(ev[1].texto_ou("operacao", ""), "exclusao");
    // A premissa do buraco -- os carimbos EMPATAM -- e o que a prova com o
    // defeito reposto mede: sem a linhagem, a exclusao de B passa pela
    // conferencia do carimbo e apaga a linha de A com `aplicados: 1`.
    let r = exigir(
        destino.porta,
        &format!(
            r#""op":"aplicar","database":"loja","tabela":"clientes",
               "linhagem":"{linhagem_b}","eventos":[{}]"#,
            ev[1].escrever()
        ),
    );
    assert_eq!(r.inteiro_ou("aplicados", -1), 0, "{}", r.escrever());
    let erro = r.texto_ou("erro", "").to_string();
    for pedaco in ["clientes", "historia", "linhagem"] {
        assert!(erro.contains(pedaco), "a recusa nao diz {pedaco:?}: {erro}");
    }
    assert_eq!(
        linhas(destino.porta),
        vec![(1, "da caixa A".to_string())],
        "a linha de A sumiu"
    );
}

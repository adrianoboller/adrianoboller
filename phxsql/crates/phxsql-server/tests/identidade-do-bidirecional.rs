//! A identidade de uma linha e de um servidor no bidirecional, provada PELO
//! SOQUETE, com servidores de verdade no ar -- pedidos 329 e 331, e a troca de
//! chave achada na onda de 01/10/2026.
//!
//! Teste unitario nao prova nenhum dos tres, e o motivo e o de sempre nesta
//! casa: o que se quer saber nao e o que uma funcao devolve, e quantas linhas
//! ficam do OUTRO lado depois de os lacos rodarem.
//!
//! - **Troca de chave.** A imagem do evento e o «depois»: quem casa por chave
//!   do outro lado procurava a linha pela chave nova, nao achava, inseria -- e
//!   a antiga ficava. Uma alteracao virava duas linhas.
//! - **Chave composta (331).** A tabela de itens `(venda, item)` era recusada
//!   inteira; agora a identidade e a tupla, e coluna diferente e outra linha.
//! - **Numero de origem (329).** O `hash_id` de 16 bits colide, e a
//!   conferencia era do PAR: dois caixas com o mesmo numero entre si nao eram
//!   vistos por ninguem, e o central suprimia os eventos de um ao servir o
//!   outro. Agora o numero se confere contra todos os pares vistos, e pode
//!   ser ATRIBUIDO.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Origem, Papel, Servidor};

const TOKEN: &str = "identidade-bidi";
/// Quanto esperar por um laco que roda a cada segundo. O mesmo teto dos
/// outros testes de replicacao desta pasta.
const ESPERA: Duration = Duration::from_secs(20);

/// Um servidor no ar, PARADO no fim -- inclusive quando o teste cai no meio
/// de uma asercao. O `servico_parar` fecha a porta de dados; os lacos de
/// replicacao morrem com o processo de teste.
struct NoAr {
    _s: Arc<Servidor>,
    porta: u16,
}

impl Drop for NoAr {
    fn drop(&mut self) {
        let _ = tentar(self.porta, r#""op":"servico_parar""#);
    }
}

fn config(base: &std::path::Path, bind: String, id: &str, numero: u16) -> Config {
    let mut c = Config {
        bind,
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    // O ESCAPE ESCRITO, como no `laco-do-unico-secundario.rs`: o que esta
    // bateria mede e a identidade, nao o portao da cifra do fio.
    c.cifra_fio.exigir = false;
    c.cifra_fio.arquivo = base.join("chave-do-fio.hex");
    c.web.ligado = false;
    c.replicacao.papel = Papel::Multi;
    c.replicacao.id_servidor = id.into();
    c.replicacao.numero_servidor = numero;
    c.replicacao.imagem_da_linha = true;
    c
}

fn origem(nome: &str, porta: u16) -> Origem {
    Origem {
        nome: nome.into(),
        host: "127.0.0.1".into(),
        porta,
        token: TOKEN.into(),
        databases: vec!["loja".into()],
        reconectar_em: 1,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
        cifra: false,
        chave_do_fio: String::new(),
        espelho: false,
    }
}

/// Sobe num ouvinte que o teste reserva na porta 0 e so entrega ao servidor
/// (pedidos 352 e 401): o arranque que falha volta como erro, e a porta
/// conferida e a que o PROPRIO servidor anotou.
fn subir(mut c: Config) -> NoAr {
    let (ouvinte, porta) = comum::ouvinte_reservado();
    c.bind = format!("127.0.0.1:{porta}");
    subir_no_ouvinte(c, ouvinte)
}

/// Sobe no ouvinte que o TESTE ja abriu e segura (pedidos 352 e 401): no par
/// que puxa nos dois sentidos, cada um precisa da porta do outro no config
/// antes de subir, e o ouvinte reservado nunca devolve o numero ao sistema.
fn subir_no_ouvinte(c: Config, ouvinte: std::net::TcpListener) -> NoAr {
    let s = Servidor::novo(c).unwrap();
    let porta = comum::no_ar_no_ouvinte(&s, ouvinte);
    NoAr { _s: s, porta }
}

fn tentar(porta: u16, corpo: &str) -> Option<Json> {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().ok()?;
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).ok()?;
    fluxo.set_read_timeout(Some(Duration::from_secs(10))).ok()?;
    let mut escrita = fluxo.try_clone().ok()?;
    let mut leitor = BufReader::new(fluxo);
    writeln!(
        escrita,
        "{{\"token\":\"{TOKEN}\",{}}}",
        corpo.replace('\n', " ")
    )
    .ok()?;
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).ok()?;
    Json::analisar(&resposta).ok()
}

fn exigir(porta: u16, corpo: &str) -> Json {
    let r = tentar(porta, corpo).unwrap_or_else(|| panic!("{corpo}: sem resposta de {porta}"));
    assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    r.campo("resultado").cloned().unwrap_or(Json::Nulo)
}

fn esperar<F: FnMut() -> bool>(o_que: &str, mut f: F) {
    let ate = Instant::now() + ESPERA;
    while Instant::now() < ate {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("{o_que} nao aconteceu em {} s", ESPERA.as_secs());
}

/// As linhas vivas de uma tabela, com o rowid DAQUI. Vazia enquanto a tabela
/// nao existe deste lado: a replica cria a tabela na primeira rodada, e quem
/// espera por ela pergunta antes disso.
fn linhas(porta: u16, tabela: &str) -> Vec<Json> {
    tentar(
        porta,
        &format!(r#""op":"varrer","database":"loja","tabela":"{tabela}","max":100"#),
    )
    .and_then(|r| r.campo("resultado").cloned())
    .and_then(|r| {
        r.campo("linhas")
            .and_then(Json::lista)
            .map(<[Json]>::to_vec)
    })
    .unwrap_or_default()
}

fn criar_clientes(porta: u16) {
    exigir(porta, r#""op":"criar_database","database":"loja""#);
    exigir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                      {"nome":"nome","tipo":"Str(20)"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
}

fn ids(porta: u16) -> Vec<i64> {
    let mut v: Vec<i64> = linhas(porta, "clientes")
        .iter()
        .map(|l| l.inteiro_ou("id", -1))
        .collect();
    v.sort();
    v
}

// ----------------------------------------------------------- troca de chave

/// **Prova real da troca de chave.** A troca o `id` da linha 1 para 10; do
/// outro lado tem de ficar UMA linha, a 10 -- e na MESMA posicao da ordem de
/// digitacao de la (o rowid de B nao muda: a troca chega como alteracao).
///
/// **Medido** (01/10/2026) com o defeito reposto -- `Table::muda_chave_unica`
/// devolvendo sempre `false`, a imagem sem o «antes», que e o caminho de antes
/// do conserto: B terminava com `[1, 2, 10]` contra `[2, 10]` -- a alteracao
/// virava insercao nova, e a linha antiga ficava.
#[test]
fn a_troca_de_chave_chega_como_alteracao_e_nao_como_linha_nova() {
    let dir_a = DirTemp::novo("troca-a");
    let dir_b = DirTemp::novo("troca-b");
    let a = subir(config(&dir_a, "127.0.0.1:0".into(), "troca-a", 0));
    criar_clientes(a.porta);
    exigir(
        a.porta,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"ana"}"#,
    );
    exigir(
        a.porta,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":2,"nome":"bia"}"#,
    );
    let mut cb = config(&dir_b, "127.0.0.1:0".into(), "troca-b", 0);
    cb.replicacao.origens = vec![origem("a", a.porta)];
    let b = subir(cb);
    esperar("as duas linhas em B", || ids(b.porta) == vec![1, 2]);
    let rowid_em_b = linhas(b.porta, "clientes")
        .iter()
        .find(|l| l.inteiro_ou("id", -1) == 1)
        .map(|l| l.inteiro_ou("rowid", -1))
        .unwrap();

    exigir(
        a.porta,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
           "valores":{"id":10,"nome":"ana"}"#,
    );
    esperar("a chave 10 em B", || ids(b.porta).contains(&10));
    // Um lote a mais de folga: a linha antiga, se fosse ficar, ja estaria la.
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(
        ids(b.porta),
        vec![2, 10],
        "a troca de chave virou linha nova e a antiga ficou"
    );
    let dez = linhas(b.porta, "clientes")
        .into_iter()
        .find(|l| l.inteiro_ou("id", -1) == 10)
        .unwrap();
    assert_eq!(
        dez.inteiro_ou("rowid", -1),
        rowid_em_b,
        "a troca chegou como exclusao+insercao: a linha foi para o fim da fila"
    );
    assert_eq!(dez.texto_ou("nome", ""), "ana");
}

/// O comportamento VELHO: a alteracao que NAO troca a chave continua
/// casando pela chave, sem rabo e sem exclusao nenhuma.
#[test]
fn a_alteracao_sem_troca_de_chave_continua_como_antes() {
    let dir_a = DirTemp::novo("semtroca-a");
    let dir_b = DirTemp::novo("semtroca-b");
    let a = subir(config(&dir_a, "127.0.0.1:0".into(), "semtroca-a", 0));
    criar_clientes(a.porta);
    exigir(
        a.porta,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"ana"}"#,
    );
    let mut cb = config(&dir_b, "127.0.0.1:0".into(), "semtroca-b", 0);
    cb.replicacao.origens = vec![origem("a", a.porta)];
    let b = subir(cb);
    esperar("a linha em B", || ids(b.porta) == vec![1]);
    exigir(
        a.porta,
        r#""op":"atualizar","database":"loja","tabela":"clientes","rowid":1,
           "valores":{"id":1,"nome":"ana maria"}"#,
    );
    esperar("o nome novo em B", || {
        linhas(b.porta, "clientes")
            .iter()
            .any(|l| l.texto_ou("nome", "") == "ana maria")
    });
    assert_eq!(ids(b.porta), vec![1]);
}

// ---------------------------------------------------- chave composta (331)

fn criar_itens(porta: u16) {
    exigir(porta, r#""op":"criar_database","database":"loja""#);
    exigir(
        porta,
        r#""op":"criar_tabela","database":"loja","tabela":"itens",
           "colunas":[{"nome":"venda","tipo":"Int8","obrigatoria":true},
                      {"nome":"item","tipo":"Int8","obrigatoria":true},
                      {"nome":"qtd","tipo":"Int4"}],
           "indices":[{"nome":"porVendaItem","colunas":["venda","item"],
                       "unico":true,"primario":true}]"#,
    );
}

fn itens(porta: u16) -> Vec<(i64, i64, i64)> {
    let mut v: Vec<(i64, i64, i64)> = linhas(porta, "itens")
        .iter()
        .map(|l| {
            (
                l.inteiro_ou("venda", -1),
                l.inteiro_ou("item", -1),
                l.inteiro_ou("qtd", -1),
            )
        })
        .collect();
    v.sort();
    v
}

fn rowid_do_item(porta: u16, venda: i64, item: i64) -> i64 {
    linhas(porta, "itens")
        .iter()
        .find(|l| l.inteiro_ou("venda", -1) == venda && l.inteiro_ou("item", -1) == item)
        .map(|l| l.inteiro_ou("rowid", -1))
        .unwrap_or_else(|| panic!("({venda},{item}) nao esta em {porta}"))
}

/// **Prova real do 331, nos dois sentidos.** A cria (1,1) e (1,2), B cria
/// (1,3): os dois terminam com tres linhas. Alterar (1,2) em A nao toca (1,1)
/// em B, e excluir (1,3) em B -- exclusao fisica pela porta, que desde o 416
/// leva imagem no papel multi -- nao toca as outras em A.
///
/// **Medido antes do conserto**: a tabela era recusada inteira
/// (`recusas["loja/itens"]`), e nenhum lado via a linha do outro. **Defeito
/// reposto** (casar so pela PRIMEIRA coluna da composta --
/// `bidirecional::tupla` olhando `&colunas[..1]`): a tupla pela metade nao
/// casa o indice composto, a rodada de B para, e as linhas de A nunca chegam
/// -- o `esperar` estoura em 20 s (medido em 01/10/2026).
#[test]
fn a_chave_composta_replica_e_a_identidade_e_a_tupla() {
    let dir_a = DirTemp::novo("composta-a");
    let dir_b = DirTemp::novo("composta-b");
    let (ouvinte_a, porta_a) = comum::ouvinte_reservado();
    let (ouvinte_b, porta_b) = comum::ouvinte_reservado();
    let mut ca = config(&dir_a, format!("127.0.0.1:{porta_a}"), "composta-a", 0);
    ca.replicacao.origens = vec![origem("b", porta_b)];
    let a = subir_no_ouvinte(ca, ouvinte_a);
    criar_itens(a.porta);
    for i in [1, 2] {
        exigir(
            a.porta,
            &format!(
                r#""op":"inserir","database":"loja","tabela":"itens",
                   "linha":{{"venda":1,"item":{i},"qtd":5}}"#
            ),
        );
    }
    let mut cb = config(&dir_b, format!("127.0.0.1:{porta_b}"), "composta-b", 0);
    cb.replicacao.origens = vec![origem("a", porta_a)];
    let b = subir_no_ouvinte(cb, ouvinte_b);
    // B NAO cria a tabela: a rodada dele a cria do esquema de A, e criar aqui
    // tambem seria uma corrida com ela. Espera as duas de A chegarem.
    esperar("as duas linhas de A em B", || itens(b.porta).len() == 2);
    exigir(
        b.porta,
        r#""op":"inserir","database":"loja","tabela":"itens",
           "linha":{"venda":1,"item":3,"qtd":5}"#,
    );
    let tres = vec![(1, 1, 5), (1, 2, 5), (1, 3, 5)];
    esperar("as tres linhas nos dois lados", || {
        itens(a.porta) == tres && itens(b.porta) == tres
    });

    // Alterar (1,2) em A: em B so (1,2) muda.
    let r12 = rowid_do_item(a.porta, 1, 2);
    exigir(
        a.porta,
        &format!(
            r#""op":"atualizar","database":"loja","tabela":"itens","rowid":{r12},
               "valores":{{"venda":1,"item":2,"qtd":9}}"#
        ),
    );
    let alterada = vec![(1, 1, 5), (1, 2, 9), (1, 3, 5)];
    esperar("a alteracao de (1,2) em B", || itens(b.porta) == alterada);

    // Excluir (1,3) em B: em A so (1,3) sai.
    let r13 = rowid_do_item(b.porta, 1, 3);
    exigir(
        b.porta,
        &format!(
            r#""op":"excluir","database":"loja","tabela":"itens","rowid":{r13},
               "fisico":true"#
        ),
    );
    let sem13 = vec![(1, 1, 5), (1, 2, 9)];
    esperar("a exclusao de (1,3) em A", || itens(a.porta) == sem13);
    // Um lote a mais de folga: nada mais muda, nos dois lados.
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(itens(a.porta), sem13);
    assert_eq!(itens(b.porta), sem13);
}

/// O que o `posicao` anuncia da composta: `chave` com os nomes juntos e
/// `chave_colunas` como lista -- e a de uma coluna continua com o NOME so.
#[test]
fn o_posicao_anuncia_a_composta_e_a_simples_como_antes() {
    let dir = DirTemp::novo("composta-posicao");
    let a = subir(config(&dir, "127.0.0.1:0".into(), "composta-posicao", 0));
    criar_itens(a.porta);
    exigir(
        a.porta,
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    );
    let r = exigir(a.porta, r#""op":"posicao","database":"loja""#);
    let tab = |n: &str| {
        r.campo("tabelas")
            .and_then(|t| t.campo(n))
            .cloned()
            .unwrap()
    };
    assert_eq!(tab("itens").texto_ou("chave", ""), "venda,item");
    assert_eq!(
        tab("itens").campo("chave_colunas").map(Json::escrever),
        Some(r#"["venda","item"]"#.to_string())
    );
    assert_eq!(tab("clientes").texto_ou("chave", ""), "id");
}

// ------------------------------------------------- numero de origem (329)

/// Dois ids cujo `hash_id` de 16 bits colide, achados por forca bruta -- o
/// aniversario sobre 65.536 baldes acha um par em poucas centenas.
fn ids_que_colidem() -> (String, String) {
    use std::collections::HashMap;
    let mut vistos: HashMap<u16, String> = HashMap::new();
    for n in 0..200_000u32 {
        let id = format!("caixa-{n}");
        let h = phxsql_server::bidirecional::hash_id(&id);
        if let Some(outro) = vistos.get(&h) {
            return (outro.clone(), id);
        }
        vistos.insert(h, id);
    }
    panic!("nenhuma colisao em 200.000 ids");
}

/// O ultimo erro de UMA origem deste servidor, em `replicacao_estado`.
fn ultimo_erro(porta: u16, origem: &str) -> String {
    exigir(porta, r#""op":"replicacao_estado""#)
        .campo("origens")
        .and_then(|o| o.campo(origem))
        .map(|o| o.texto_ou("ultimo_erro", "").to_string())
        .unwrap_or_default()
}

/// Caixa X -> central C -> caixa Y, cada um puxando do anterior. X e Y tem
/// ids que colidem no hash; `numeros` da o numero atribuido de (X, C, Y).
#[allow(clippy::type_complexity)]
fn tres_nos(
    rotulo: &str,
    numeros: (u16, u16, u16),
) -> (NoAr, NoAr, NoAr, String, String, Vec<DirTemp>) {
    let (x_id, y_id) = ids_que_colidem();
    let dirs = vec![
        DirTemp::novo(&format!("{rotulo}-x")),
        DirTemp::novo(&format!("{rotulo}-c")),
        DirTemp::novo(&format!("{rotulo}-y")),
    ];
    let x = subir(config(&dirs[0], "127.0.0.1:0".into(), &x_id, numeros.0));
    criar_clientes(x.porta);
    let mut cc = config(&dirs[1], "127.0.0.1:0".into(), "central", numeros.1);
    cc.replicacao.origens = vec![origem("caixa_x", x.porta)];
    let c = subir(cc);
    // O central conhece X antes de Y pedir: o caso do supermercado, em que o
    // caixa novo entra numa rede que ja roda.
    exigir(
        x.porta,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":1,"nome":"de x"}"#,
    );
    esperar("a linha de X no central", || ids(c.porta) == vec![1]);
    let mut cy = config(&dirs[2], "127.0.0.1:0".into(), &y_id, numeros.2);
    cy.replicacao.origens = vec![origem("central", c.porta)];
    let y = subir(cy);
    (x, c, y, x_id, y_id, dirs)
}

/// **Prova real do 329: a recusa.** X e Y caem no mesmo numero, e nenhum dos
/// dois e o central -- a conferencia do PAR nunca os comparava. O central,
/// que ja viu X, recusa Y nomeando os dois.
///
/// **Defeito reposto** (so o par: `conferir_numero_do_par` comparando o
/// numero do outro apenas com o proprio): nao ha recusa nenhuma, o central
/// suprime os eventos de X ao servir Y, e Y fica sem a linha CALADO -- o
/// `esperar` do grito estoura em 20 s.
#[test]
fn dois_caixas_com_o_mesmo_numero_sao_recusados_nomeando_os_dois() {
    let (_x, _c, y, x_id, y_id, _dirs) = tres_nos("colide", (0, 0, 0));
    esperar("o grito da colisao em Y", || {
        let e = ultimo_erro(y.porta, "central");
        e.contains(&x_id) && e.contains(&y_id)
    });
    assert!(
        ids(y.porta).is_empty(),
        "Y recebeu linha de um par recusado"
    );
}

/// **Prova real do 329: o numero atribuido.** Os MESMOS ids que colidem no
/// hash, com `numero_servidor` dado a cada um: Y recebe a linha de X.
///
/// **Defeito reposto** (`Replicacao::numero` ignorando o atribuido e
/// devolvendo o hash): X e Y voltam ao mesmo numero, o central recusa Y, e a
/// linha nunca chega -- o `esperar` estoura.
#[test]
fn com_numero_atribuido_o_caixa_inocente_recebe() {
    let (_x, _c, y, _x_id, _y_id, _dirs) = tres_nos("atribuido", (11, 1, 12));
    esperar("a linha de X em Y", || ids(y.porta) == vec![1]);
    assert!(ultimo_erro(y.porta, "central").is_empty());
}

/// O comportamento VELHO: sem `numero_servidor`, um par sem colisao replica
/// exatamente como antes -- o numero e o hash de sempre.
#[test]
fn sem_numero_atribuido_o_par_sem_colisao_replica_como_antes() {
    let dir_a = DirTemp::novo("velho-a");
    let dir_b = DirTemp::novo("velho-b");
    let a = subir(config(&dir_a, "127.0.0.1:0".into(), "loja-a", 0));
    criar_clientes(a.porta);
    exigir(
        a.porta,
        r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":7,"nome":"x"}"#,
    );
    let mut cb = config(&dir_b, "127.0.0.1:0".into(), "loja-b", 0);
    cb.replicacao.origens = vec![origem("a", a.porta)];
    let b = subir(cb);
    esperar("a linha em B", || ids(b.porta) == vec![7]);
    let r = exigir(a.porta, r#""op":"posicao","database":"loja""#);
    assert_eq!(
        r.inteiro_ou("numero_servidor", -1),
        phxsql_server::bidirecional::hash_id("loja-a") as i64,
        "sem numero atribuido, o numero anunciado e o hash de sempre"
    );
}

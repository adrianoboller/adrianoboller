//! Pedidos 175 e 364, pelo soquete: o indice que a chave pede nasce com ela,
//! e o indice de texto de uma tabela que ja existe se redeclara.
//!
//! # Pedido 175 -- a mae perdia o `excluir` INTEIRO
//!
//! A chave conferida pede indice dos dois lados. Sem o da FILHA o motor nao
//! consegue nem perguntar «alguem aponta para esta linha?», e por isso recusa
//! TODO `excluir` da mae -- inclusive da linha que ninguem referencia. O
//! exemplo do `MANUAL.txt` (um indice so, pela primaria) caia nisso. A saida
//! do parecer (`docs/PARECER-175-INDICE-NA-DECLARACAO.md` §4) e a (b): o
//! indice nasce na filha, se diz na resposta, e so para chave que confere.
//!
//! # Pedido 364 -- nao havia por onde redeclarar o indice de texto
//!
//! Quem perdeu a declaracao no defeito do pedido 353 ficava com um `.fts`
//! orfao e so recuperava recriando a tabela. A prova mais dura e a do orfao
//! VELHO: o arquivo que sobreviveu nao acompanhou as gravacoes, e
//! reaproveita-lo seria uma busca que acha menos que a varredura.
//!
//! # Por que a prova mede o dano
//!
//! Cada teste do 175 tenta o `excluir` da mae que NAO tem filha -- e e ele que
//! falha com o defeito reposto, nao o veredito da declaracao.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_server::{Config, Servidor};

const TOKEN: &str = "indice-da-chave-e-de-texto";

fn subir(base: &Path) -> (Arc<Servidor>, u16) {
    let texto = format!(
        r#"{{ "bind": "127.0.0.1:0", "base": {base:?}, "token": "{TOKEN}",
              "log_acessos": {log:?}, "blacklist": {bl:?}, "dblink": {dbl:?},
              "cifra_fio": {{ "exigir": false }},
              "recursos": {{ "durabilidade": "sistema" }},
              "web": {{ "ligado": false }} }}"#,
        base = base.display().to_string(),
        log = base.join("acessos.log").display().to_string(),
        bl = base.join("blacklist.json").display().to_string(),
        dbl = base.join("dblink.json").display().to_string(),
    );
    let c = Config::de_json(&Json::analisar(&texto).unwrap()).unwrap();
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let ate = Instant::now() + Duration::from_secs(5);
    while Instant::now() < ate {
        if TcpStream::connect_timeout(&alvo, Duration::from_millis(200)).is_ok() {
            return (s, porta);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("o servidor nao subiu na porta {porta}");
}

struct Ligacao {
    escrita: TcpStream,
    leitor: BufReader<TcpStream>,
}

impl Ligacao {
    fn nova(porta: u16) -> Ligacao {
        let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
        let f = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).unwrap();
        f.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
        Ligacao {
            escrita: f.try_clone().unwrap(),
            leitor: BufReader::new(f),
        }
    }

    fn pedir(&mut self, corpo: &str) -> Json {
        let corpo: String = corpo.split_whitespace().collect::<Vec<_>>().join(" ");
        writeln!(self.escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
        let mut r = String::new();
        self.leitor.read_line(&mut r).unwrap();
        Json::analisar(&r).unwrap_or_else(|e| panic!("resposta ilegivel {r:?}: {e}"))
    }
}

fn ok(j: Json) -> Json {
    assert!(
        j.booleano_ou("ok", false),
        "o pedido falhou: {}",
        j.escrever()
    );
    j.campo("resultado").cloned().unwrap_or(j)
}

fn recusou(j: &Json) -> String {
    assert!(
        !j.booleano_ou("ok", false),
        "devia ter recusado e aceitou: {}",
        j.escrever()
    );
    j.escrever()
}

fn criados(r: &Json) -> Vec<String> {
    r.campo("indices_criados")
        .and_then(Json::lista)
        .map(|l| {
            l.iter()
                .filter_map(|x| x.texto().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// `clientes` com 3 e 7, pela primaria -- a mae dos tres testes do 175.
fn mae(c: &mut Ligacao) {
    ok(c.pedir(r#""op":"criar_database","database":"loja""#));
    ok(c.pedir(
        r#""op":"criar_tabela","database":"loja","tabela":"clientes",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    ));
    ok(c.pedir(r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":3}"#));
    ok(c.pedir(r#""op":"inserir","database":"loja","tabela":"clientes","linha":{"id":7}"#));
}

/// O dano que o 175 conserta, medido nas DUAS linhas da mae: a 7 (sem filha)
/// tem de sair, e a 3 (com filha) tem de ficar pela regra primordial.
fn a_mae_exclui_quem_nao_tem_filha(c: &mut Ligacao) {
    let r =
        c.pedir(r#""op":"excluir","database":"loja","tabela":"clientes","rowid":2,"fisico":true"#);
    assert!(
        r.booleano_ou("ok", false),
        "a linha 7 nao tem filha e a mae nao conseguiu exclui-la -- o defeito \
         do 175: sem indice na filha o motor recusa TODO excluir da mae. {}",
        r.escrever()
    );
    let r =
        c.pedir(r#""op":"excluir","database":"loja","tabela":"clientes","rowid":1,"fisico":true"#);
    let texto = recusou(&r);
    assert!(
        texto.contains("filhas"),
        "a linha 3 tem filha e tinha de recusar pela regra primordial: {texto}"
    );
}

fn indices_de(c: &mut Ligacao, tabela: &str) -> Vec<String> {
    let r = ok(c.pedir(&format!(
        r#""op":"esquema","database":"loja","tabela":"{tabela}""#
    )));
    r.campo("indices")
        .and_then(Json::lista)
        .map(|l| {
            l.iter()
                .map(|i| i.texto_ou("nome", "").to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// O exemplo do `MANUAL.txt`: a chave no `criar_tabela`, um indice so.
#[test]
fn o_exemplo_do_manual_nasce_com_o_indice_da_chave() {
    let d = DirTemp::novo("175-manual");
    let (_s, porta) = subir(&d);
    let mut c = Ligacao::nova(porta);
    mae(&mut c);
    let r = ok(c.pedir(
        r#""op":"criar_tabela","database":"loja","tabela":"pedidos",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                      {"nome":"cliente_id","tipo":"Int4"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}],
           "chaves_estrangeiras":[{"nome":"fk_cliente","colunas":["cliente_id"],
                                   "tabela_ref":"clientes","colunas_ref":["id"]}]"#,
    ));
    ok(c.pedir(
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"cliente_id":3}"#,
    ));
    // O dano primeiro, e so depois o que a resposta diz.
    a_mae_exclui_quem_nao_tem_filha(&mut c);
    assert_eq!(criados(&r), vec!["idx_fk_cliente"], "{}", r.escrever());
    assert_eq!(
        indices_de(&mut c, "pedidos"),
        vec!["porId", "idx_fk_cliente"]
    );
}

/// A chave declarada DEPOIS, sobre uma filha que ja tem dado.
#[test]
fn declarar_a_chave_numa_filha_com_dado_cria_o_indice_e_o_monta() {
    let d = DirTemp::novo("175-declarar");
    let (_s, porta) = subir(&d);
    let mut c = Ligacao::nova(porta);
    mae(&mut c);
    ok(c.pedir(
        r#""op":"criar_tabela","database":"loja","tabela":"pedidos",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                      {"nome":"cliente_id","tipo":"Int4"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    ));
    ok(c.pedir(
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":1,"cliente_id":3}"#,
    ));
    let r = ok(c.pedir(
        r#""op":"declarar_fk","database":"loja","tabela":"pedidos","nome":"fk_cliente",
           "colunas":["cliente_id"],"tabela_ref":"clientes","colunas_ref":["id"]"#,
    ));
    // A arvore nova tem a linha que ja estava la: e ela que faz a 3 recusar.
    a_mae_exclui_quem_nao_tem_filha(&mut c);
    assert_eq!(criados(&r), vec!["idx_fk_cliente"], "{}", r.escrever());
    // E a filha continua gravando, agora com a arvore nova acompanhando.
    ok(c.pedir(
        r#""op":"inserir","database":"loja","tabela":"pedidos","linha":{"id":2,"cliente_id":3}"#,
    ));
}

/// O comportamento VELHO: quem ja tinha o indice, e quem pediu para nao
/// conferir, saem exatamente como antes -- sem indice novo e sem campo novo.
#[test]
fn com_o_indice_ja_la_ou_sem_conferir_nada_nasce() {
    let d = DirTemp::novo("175-velho");
    let (_s, porta) = subir(&d);
    let mut c = Ligacao::nova(porta);
    mae(&mut c);
    ok(c.pedir(
        r#""op":"criar_tabela","database":"loja","tabela":"pedidos",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                      {"nome":"cliente_id","tipo":"Int4"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true},
                      {"nome":"porCliente","colunas":["cliente_id"]}]"#,
    ));
    let r = ok(c.pedir(
        r#""op":"declarar_fk","database":"loja","tabela":"pedidos","nome":"fk_cliente",
           "colunas":["cliente_id"],"tabela_ref":"clientes","colunas_ref":["id"]"#,
    ));
    assert!(
        r.campo("indices_criados").is_none(),
        "com o indice ja la a resposta nao muda: {}",
        r.escrever()
    );
    assert_eq!(indices_de(&mut c, "pedidos"), vec!["porId", "porCliente"]);

    ok(c.pedir(
        r#""op":"criar_tabela","database":"loja","tabela":"notas",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                      {"nome":"cliente_id","tipo":"Int4"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    ));
    let r = ok(c.pedir(
        r#""op":"declarar_fk","database":"loja","tabela":"notas","nome":"fk_solta",
           "colunas":["cliente_id"],"tabela_ref":"clientes","colunas_ref":["id"],
           "verificar":false"#,
    ));
    assert!(
        r.campo("indices_criados").is_none(),
        "quem dispensou a conferencia nao paga indice: {}",
        r.escrever()
    );
    assert_eq!(indices_de(&mut c, "notas"), vec!["porId"]);
}

fn produtos(c: &mut Ligacao) {
    ok(c.pedir(r#""op":"criar_database","database":"loja""#));
    ok(c.pedir(
        r#""op":"criar_tabela","database":"loja","tabela":"produtos",
           "colunas":[{"nome":"id","tipo":"Int4","obrigatoria":true},
                      {"nome":"descricao","tipo":"Str(60)"}],
           "indices":[{"nome":"porId","colunas":["id"],"unico":true,"primario":true}]"#,
    ));
    for (id, desc) in [(1, "ave Fênix dourada"), (2, "caneca azul"), (3, "lapis")] {
        ok(c.pedir(&format!(
            r#""op":"inserir","database":"loja","tabela":"produtos",
               "linha":{{"id":{id},"descricao":"{desc}"}}"#
        )));
    }
}

fn achados(c: &mut Ligacao, palavra: &str) -> Result<u64, String> {
    let r = c.pedir(&format!(
        r#""op":"procurar_texto","database":"loja","tabela":"produtos",
           "indice":"porDescricao","palavra":"{palavra}""#
    ));
    if r.booleano_ou("ok", false) {
        Ok(ok(r).inteiro_ou("encontrados", -1) as u64)
    } else {
        Err(r.escrever())
    }
}

const DECLARA: &str = r#""op":"redeclarar_indices_texto","database":"loja","tabela":"produtos",
    "indices_texto":[{"nome":"porDescricao","coluna":"descricao"}]"#;

#[test]
fn o_indice_de_texto_se_redeclara_numa_tabela_que_ja_existe() {
    let d = DirTemp::novo("364-redeclarar");
    let (_s, porta) = subir(&d);
    let mut c = Ligacao::nova(porta);
    produtos(&mut c);
    assert!(
        achados(&mut c, "fenix").is_err(),
        "sem declaracao nao ha indice"
    );

    let r = ok(c.pedir(DECLARA));
    assert_eq!(r.inteiro_ou("linhas_indexadas", -1), 3, "{}", r.escrever());
    assert_eq!(
        achados(&mut c, "fenix"),
        Ok(1),
        "a dobra de acento nasce ligada"
    );
    // A gravacao seguinte entra no `.fts` sem ninguem pedir.
    ok(c.pedir(
        r#""op":"inserir","database":"loja","tabela":"produtos",
           "linha":{"id":4,"descricao":"fenix de pelucia"}"#,
    ));
    assert_eq!(achados(&mut c, "fenix"), Ok(2));

    // Campo ausente e recusado e NAO apaga nada.
    recusou(&c.pedir(
        r#""op":"redeclarar_indices_texto","database":"loja","tabela":"produtos",
           "indices":[]"#,
    ));
    assert_eq!(
        achados(&mut c, "fenix"),
        Ok(2),
        "o pedido errado apagou o indice"
    );

    // A lista vazia tira a declaracao E o arquivo: declaracao vazia com
    // `.fts` ao lado e o orfao que o 364 existe para nao deixar.
    let fts = d.join("loja").join("produtos.fts");
    assert!(fts.exists());
    let r = ok(c.pedir(
        r#""op":"redeclarar_indices_texto","database":"loja","tabela":"produtos",
           "indices_texto":[]"#,
    ));
    assert_eq!(r.inteiro_ou("linhas_indexadas", -1), 0);
    assert!(!fts.exists(), "a lista vazia deixou o .fts orfao no disco");
    assert!(achados(&mut c, "fenix").is_err());
}

/// O orfao VELHO do 353: o `.fts` sobreviveu a um esquema que nao o
/// declarava, e as linhas gravadas nesse meio-tempo nao estao nele.
/// Redeclarar tem de RECONSTRUIR do `.reg`, nunca reaproveitar o arquivo.
#[test]
fn o_fts_orfao_e_reconstruido_e_nao_reaproveitado() {
    let d = DirTemp::novo("364-orfao");
    let (_s, porta) = subir(&d);
    let mut c = Ligacao::nova(porta);
    produtos(&mut c);
    ok(c.pedir(DECLARA));
    let fts = d.join("loja").join("produtos.fts");
    let velho = std::fs::read(&fts).unwrap();

    // A perda: a declaracao sai, o arquivo velho volta para o lugar, e a
    // tabela grava sem indice nenhum acompanhando.
    ok(c.pedir(
        r#""op":"redeclarar_indices_texto","database":"loja","tabela":"produtos",
           "indices_texto":[]"#,
    ));
    std::fs::write(&fts, &velho).unwrap();
    ok(c.pedir(
        r#""op":"inserir","database":"loja","tabela":"produtos",
           "linha":{"id":5,"descricao":"fenix de vidro"}"#,
    ));

    let r = ok(c.pedir(DECLARA));
    // O dano primeiro: a busca. A contagem da resposta vem depois.
    assert_eq!(
        achados(&mut c, "fenix"),
        Ok(2),
        "a linha gravada enquanto o .fts estava orfao ficou fora da busca"
    );
    assert_eq!(r.inteiro_ou("linhas_indexadas", -1), 4, "{}", r.escrever());
}

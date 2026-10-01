//! Pedido 300 (3): a posicao somada do cluster contava tabela que nao e
//! replicada -- provado PELO SOQUETE, com dois nos de verdade.
//!
//! O comentario dizia «somada sobre as tabelas replicadas»; o codigo somava
//! todo database do disco. Um no que entra no cluster com um database que so
//! mora nele (dado de antes, um rascunho de trabalho) publicava no pulso uma
//! posicao maior que a do master -- e a eleicao, que compara essa soma, o
//! escolheria estando atras no dado que importa.
//!
//! O que se prova aqui e o caminho inteiro: a replica aprende o que o master
//! anuncia na RODADA da replicacao (o `bancos` e o `posicao` pelo fio), e a
//! posicao que o pulso publica so soma isso. Teste de unidade prova o filtro;
//! so dois processos conversando provam que o anuncio chega.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_server::{Config, Servidor};
use phxsql_store::catalogo::Instancia;

const TOKEN: &str = "posicao-do-cluster";

/// Sobe UM no do cluster de dois. Fio em claro, pelo escape escrito: o que se
/// mede e a soma da posicao, nao o aperto de mao.
fn subir_no(
    base: &std::path::Path,
    este: &str,
    papel: &str,
    ouvinte_este: TcpListener,
    outro: &str,
    porta_outro: u16,
) -> Arc<Servidor> {
    let porta_este = ouvinte_este.local_addr().unwrap().port();
    let caminho = base.join("config.json");
    let bar = |p: std::path::PathBuf| p.display().to_string().replace('\\', "/");
    std::fs::write(
        &caminho,
        format!(
            r#"{{
              "bind": "127.0.0.1:{porta_este}",
              "base": "{base_dir}",
              "token": "{TOKEN}",
              "log_acessos": "{log}",
              "seguranca": {{ "blacklist": "{bl}" }},
              "dblink": "{dblink}",
              "jobs": "{jobs}",
              "web": {{ "ligado": false }},
              "cifra_fio": {{ "exigir": false }},
              "replicacao": {{ "papel": "{papel}", "id_servidor": "{este}", "imagem_da_linha": true }},
              "cluster": {{
                "id": "{este}",
                "token": "{TOKEN}",
                "janela_inatividade_s": 30,
                "pulso_s": 1,
                "cifra": false,
                "nos": [
                  {{ "id": "{este}", "endereco": "127.0.0.1", "porta": {porta_este} }},
                  {{ "id": "{outro}", "endereco": "127.0.0.1", "porta": {porta_outro} }}
                ]
              }}
            }}"#,
            base_dir = bar(base.join("base")),
            log = bar(base.join("acessos.log")),
            bl = bar(base.join("blacklist.json")),
            dblink = bar(base.join("dblink.json")),
            jobs = bar(base.join("jobs.json")),
        ),
    )
    .unwrap();
    let s = Servidor::novo(Config::ler(&caminho).unwrap()).unwrap();
    comum::no_ar_no_ouvinte(&s, ouvinte_este);
    s
}

fn pedir(porta: u16, corpo: &str) -> Option<Json> {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2)).ok()?;
    fluxo.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut escrita = fluxo.try_clone().ok()?;
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").ok()?;
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).ok()?;
    Json::analisar(&resposta).ok()
}

/// (posicao, incompleta) que o no da `porta` publica para `id`.
fn posicao_de(porta: u16, id: &str) -> Option<(i64, bool)> {
    let r = pedir(porta, r#""op":"cluster_estado""#)?;
    let nos = r.campo("resultado")?.campo("nos")?.lista()?;
    let n = nos.iter().find(|n| n.texto_ou("id", "") == id)?;
    Some((
        n.inteiro_ou("posicao", -1),
        n.booleano_ou("posicao_incompleta", n.booleano_ou("incompleta", false)),
    ))
}

fn tabela(nome: &str) -> Schema {
    Schema::new(
        nome,
        vec![Column::new("id", ColumnType::Int8).obrigatoria()],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
    )
    .unwrap()
}

/// **A prova real.** O master tem `loja/clientes` com 4 eventos. A replica
/// entra no cluster com `rascunho/notas` -- 7 eventos que so moram nela. Ela
/// alcanca os 4 de `loja`, e a posicao que ela publica tem de ser 4, a MESMA
/// do master -- nao 11.
///
/// O vermelho: com a soma de antes (todo database do disco), a replica
/// publica 11 e o laco abaixo estoura o prazo. Medido em 01/10/2026 com o
/// filtro do anuncio tirado de `contar_posicao_do_cluster`.
#[test]
fn a_replica_publica_so_a_posicao_do_que_o_master_replica() {
    let base_a = DirTemp::novo("pos300-master");
    let base_b = DirTemp::novo("pos300-replica");
    {
        let inst = Instancia::nova(base_a.0.join("base")).unwrap();
        let db = inst.criar_database("loja").unwrap();
        let mut t = db.criar_tabela(None, tabela("clientes")).unwrap();
        // O master grava com imagem, como o `imagem_da_linha` do config dele
        // manda -- sem ela o diario existe e nao replica.
        t.ligar_imagem_no_diario(true);
        for i in 1..=4 {
            t.inserir(&[Value::Int(i)]).unwrap();
        }
        t.sincronizar().unwrap();
    }
    {
        let inst = Instancia::nova(base_b.0.join("base")).unwrap();
        let db = inst.criar_database("rascunho").unwrap();
        let mut t = db.criar_tabela(None, tabela("notas")).unwrap();
        for i in 1..=7 {
            t.inserir(&[Value::Int(i)]).unwrap();
        }
        t.sincronizar().unwrap();
    }
    let (ouvinte_a, porta_a) = comum::ouvinte_reservado();
    let (ouvinte_b, porta_b) = comum::ouvinte_reservado();
    let _a = subir_no(&base_a, "noA", "source", ouvinte_a, "noB", porta_b);
    let _b = subir_no(&base_b, "noB", "replica", ouvinte_b, "noA", porta_a);

    // A replica ja alcancou `loja` quando a tabela existe nela com 4 linhas
    // no diario; a posicao publicada anda no ritmo do pulso (1 s).
    let ate = Instant::now() + Duration::from_secs(30);
    let mut visto = None;
    while Instant::now() < ate {
        visto = posicao_de(porta_b, "noB");
        if let Some((4, false)) = visto {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    assert_eq!(
        visto,
        Some((4, false)),
        "a replica tinha de publicar 4 (so loja/clientes), completa; o rascunho \
         local nao e posicao de nada que o cluster replica"
    );
    // E a soma da replica e a do master: os dois estao em dia no que importa.
    assert_eq!(posicao_de(porta_a, "noA").map(|p| p.0), Some(4));
    // O rascunho continua la, intocado: o filtro mexe na CONTA, nao no dado.
    assert!(base_b
        .0
        .join("base")
        .join("rascunho")
        .join("notas.reg")
        .exists());
}

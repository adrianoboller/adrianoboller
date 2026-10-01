//! Pedido 424: a recusa do ledger mora na DECLARACAO, e a replica nao declara.
//!
//! Provado PELO SOQUETE, entre um source e uma replica de verdade. O source
//! tem uma cadeia com coluna marcada gravada antes da guarda do pedido 355 --
//! ela nasce pelo caminho do DISCO (`Schema::do_disco`), que e o unico por onde
//! ainda nasce. A replica remonta o esquema por `Schema::desserializar`, que nao
//! julga, e cria a mesma cadeia aqui.
//!
//! Decisao do J (01/10/2026, `decisoes-onda-01-10-2026.md` §424): a replica
//! NAO recusa -- a petrea «guarda nova entra pedida» ganha da regua 7x2 dos
//! motores --, e sim cria, grita e CONTA. O que se mede e o contador
//! `ledger_marcado_recebido` no `replicacao_estado` e o censo da base da
//! replica, nunca o veredito da replicacao.

mod comum;
use comum::DirTemp;

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phxsql_core::json::Json;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_server::{Config, Origem, Papel, Servidor};
use phxsql_store::catalogo::Instancia;

const TOKEN: &str = "ledger-marcado-na-replica";

fn config_base(base: &std::path::Path) -> Config {
    let mut c = Config {
        bind: "127.0.0.1:0".into(),
        base: base.to_path_buf(),
        log_acessos: base.join("acessos.log"),
        blacklist: base.join("blacklist.json"),
        dblink: base.join("dblink.json"),
        jobs: base.join("jobs.json"),
        token: TOKEN.into(),
        ..Default::default()
    };
    // O escape escrito da cifra exigida (pedido 370): o que se mede aqui e o
    // esquema que viaja, nao o aperto de mao.
    c.cifra_fio.exigir = false;
    c.web.ligado = false;
    c
}

fn subir(c: Config) -> (Arc<Servidor>, u16) {
    let s = Servidor::novo(c).unwrap();
    let copia = Arc::clone(&s);
    std::thread::spawn(move || {
        let _ = copia.escutar();
    });
    let porta = comum::porta_real(|| s.porta_dos_dados());
    (s, porta)
}

fn subir_replica(base: &std::path::Path, porta_do_source: u16) -> (Arc<Servidor>, u16) {
    let mut c = config_base(base);
    c.replicacao.papel = Papel::Replica;
    c.replicacao.id_servidor = "replica-424".into();
    c.replicacao.origens = vec![Origem {
        nome: "fonte".into(),
        host: "127.0.0.1".into(),
        porta: porta_do_source,
        token: TOKEN.into(),
        databases: vec!["livro".into()],
        reconectar_em: 1,
        usuario: String::new(),
        senha_hash: String::new(),
        senha: String::new(),
        cada_minutos: 0,
        hora: String::new(),
        cifra: false,
        chave_do_fio: String::new(),
    }];
    subir(c)
}

fn pedir(porta: u16, corpo: &str) -> Json {
    let alvo: SocketAddr = format!("127.0.0.1:{porta}").parse().unwrap();
    let fluxo = TcpStream::connect_timeout(&alvo, Duration::from_secs(2))
        .unwrap_or_else(|e| panic!("nao conectei em {porta}: {e}"));
    fluxo
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut escrita = fluxo.try_clone().unwrap();
    let mut leitor = BufReader::new(fluxo);
    writeln!(escrita, "{{\"token\":\"{TOKEN}\",{corpo}}}").unwrap();
    let mut resposta = String::new();
    leitor.read_line(&mut resposta).unwrap();
    Json::analisar(&resposta).unwrap_or_else(|e| panic!("{corpo}: ilegivel ({e}): {resposta}"))
}

fn exigir(porta: u16, corpo: &str) -> Json {
    let r = pedir(porta, corpo);
    assert!(r.booleano_ou("ok", false), "{corpo} -> {}", r.escrever());
    r.campo("resultado").cloned().unwrap_or(Json::Nulo)
}

/// A cadeia legada: `cpf` marcado, montada pelo caminho do disco -- o
/// `Schema::new` a recusaria desde o pedido 355.
fn cadeia_com_cpf_marcado(nome: &str) -> Schema {
    let base = Schema::new(
        nome,
        vec![
            Column::new("hash", ColumnType::Uuid256).obrigatoria(),
            Column::new("anterior", ColumnType::Uuid256),
            Column::new("altura", ColumnType::Sequence),
            Column::new("cpf", ColumnType::Str(11)),
        ],
        vec![IndexDef::new("porAltura", vec![IndexColumn::asc(2)]).unico()],
    )
    .unwrap();
    let mut colunas = base.colunas().to_vec();
    let i = base.coluna_por_nome("cpf").unwrap();
    colunas[i].dado_pessoal = DadoPessoal::Pessoal;
    Schema::do_disco(nome, colunas, base.indices().to_vec()).unwrap()
}

/// A mesma cadeia, sem marca nenhuma -- o irmao que NAO pode contar.
fn cadeia_limpa(nome: &str) -> Schema {
    Schema::new(
        nome,
        vec![
            Column::new("hash", ColumnType::Uuid256).obrigatoria(),
            Column::new("anterior", ColumnType::Uuid256),
            Column::new("altura", ColumnType::Sequence),
            Column::new("autor", ColumnType::Str(20)),
        ],
        vec![IndexDef::new("porAltura", vec![IndexColumn::asc(2)]).unico()],
    )
    .unwrap()
}

fn contador(porta_replica: u16) -> i64 {
    exigir(porta_replica, r#""op":"replicacao_estado""#).inteiro_ou("ledger_marcado_recebido", -1)
}

fn existe_na_replica(base: &std::path::Path, tabela: &str) -> bool {
    base.join("livro").join(format!("{tabela}.reg")).exists()
}

/// **Prova real, nos dois sentidos.** O source tem uma cadeia marcada e uma
/// limpa. A replica cria as duas (nao recusa: a petrea ganhou), o contador vai
/// a 1 -- so a marcada conta -- e o censo da base da replica lista a marcada
/// como PROIBIDA.
///
/// O vermelho: tirada a chamada ao `recensear` na criacao, a replica cria igual
/// e o contador fica em 0 -- a asserção do contador cai. Medido em 01/10/2026.
#[test]
fn a_replica_cria_a_cadeia_marcada_grita_e_conta() {
    let base_s = DirTemp::novo("ledger424-source");
    let base_r = DirTemp::novo("ledger424-replica");
    {
        let inst = Instancia::nova(&base_s.0).unwrap();
        let db = inst.criar_database("livro").unwrap();
        db.criar_tabela(None, cadeia_com_cpf_marcado("blocos"))
            .unwrap();
        db.criar_tabela(None, cadeia_limpa("limpa")).unwrap();
    }
    let mut c = config_base(&base_s.0);
    c.replicacao.papel = Papel::Source;
    c.replicacao.id_servidor = "source-424".into();
    c.replicacao.imagem_da_linha = true;
    let (_source, porta_s) = subir(c);

    let (_replica, porta_r) = subir_replica(&base_r.0, porta_s);
    let ate = Instant::now() + Duration::from_secs(20);
    while !(existe_na_replica(&base_r.0, "blocos") && existe_na_replica(&base_r.0, "limpa")) {
        assert!(
            Instant::now() < ate,
            "a replica nao criou as duas cadeias em 20 s"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    // A replica CRIOU a marcada: a petrea ganhou, nada foi recusado.
    assert_eq!(
        contador(porta_r),
        1,
        "a cadeia marcada chegou e o contador nao andou -- ou andou pela limpa"
    );
    // Rodadas seguintes ABREM a tabela, nao a criam: o numero nao infla.
    std::thread::sleep(Duration::from_millis(2500));
    assert_eq!(
        contador(porta_r),
        1,
        "a abertura de cada rodada inflou a conta"
    );

    let censo = phxsql_store::ledger::censo(&base_r.0).unwrap();
    let proibidas: Vec<_> = censo.iter().filter(|l| l.proibida()).collect();
    assert_eq!(proibidas.len(), 1, "{censo:?}");
    assert_eq!(proibidas[0].tabela, "blocos");
    assert_eq!(proibidas[0].colunas_marcadas, vec!["cpf".to_string()]);
}

/// **O comportamento VELHO.** Sem cadeia marcada no source, o contador nasce e
/// fica em zero, e a replica segue criando a cadeia limpa como sempre criou.
#[test]
fn sem_cadeia_marcada_o_contador_fica_em_zero() {
    let base_s = DirTemp::novo("ledger424-source-limpo");
    let base_r = DirTemp::novo("ledger424-replica-limpa");
    {
        let inst = Instancia::nova(&base_s.0).unwrap();
        let db = inst.criar_database("livro").unwrap();
        db.criar_tabela(None, cadeia_limpa("limpa")).unwrap();
    }
    let mut c = config_base(&base_s.0);
    c.replicacao.papel = Papel::Source;
    c.replicacao.id_servidor = "source-424-limpo".into();
    c.replicacao.imagem_da_linha = true;
    let (_source, porta_s) = subir(c);
    let (_replica, porta_r) = subir_replica(&base_r.0, porta_s);
    let ate = Instant::now() + Duration::from_secs(20);
    while !existe_na_replica(&base_r.0, "limpa") {
        assert!(Instant::now() < ate, "a replica nao criou a cadeia limpa");
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(contador(porta_r), 0);
}

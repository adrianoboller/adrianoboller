/* =========================================================== pedido 222
O CAMPO "volumes" DO `esquema` NA PARTICAO POR QUANTIDADE.

Ate aqui so a particao por PERIODO enchia `esquema.volumes`: ela le a
fronteira gravada no cabecalho de cada volume, porque so ali o corte depende
do calendario. Na particao por QUANTIDADE o endereco sempre foi uma DIVISAO
-- nao ha fronteira nenhuma para o ENDERECAMENTO ler --, e por isso ninguem
calculava uma para MOSTRAR: a tela e um ETL de fora ficavam sem saber quantos
volumes a tabela tinha e quantas linhas cada um guardava.

`RegFile::fronteiras` (em `phxsql-store/src/reg.rs`) agora CALCULA a lista
tambem neste modo, a partir dos volumes que existem em disco -- sem tocar em
`self.fronteiras`, o cache que `localizar` usa para enderecar. Este teste
prova que o `esquema` do protocolo devolve a lista certa, com a MESMA conta
que o enderecamento ja fazia, e que a particao por periodo continua devolvendo
a dela sem mudar em nada (controle). */
use super::*;

fn dir_temp(nome: &str) -> DirTemp {
    DirTemp::novo(&format!("volumes-qtd-{nome}"))
}

fn pedido(txt: &str) -> Json {
    Json::analisar(txt).unwrap()
}

fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
    let c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        token: "t".into(),
        ..Config::default()
    };
    let s = Servidor::novo(c).unwrap();
    s.executar(
        "criar_database",
        &pedido(r#"{"database":"b"}"#),
        &Sessao::default(),
    )
    .unwrap();
    s
}

/// Pelo `despachar`: e por onde o pedido entra de verdade.
fn pede(s: &Arc<Servidor>, corpo: &str) -> Result<Json> {
    let mut ses = Sessao::default();
    let (_, _, r) = s.despachar(
        &format!(r#"{{"token":"t",{corpo}}}"#),
        &mut ses,
        "127.0.0.1",
    );
    r
}

/// Quantas linhas cada volume tem, derivado de `primeiro_rowid` -- a
/// MESMA conta que a tela (`volumesDe` em `ui/index.html`) ja faz para a
/// particao por periodo, aplicada aqui contra a por quantidade.
fn contagens(volumes: &[Json], total: u64) -> Vec<u64> {
    volumes
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let de = v.campo("primeiro_rowid").and_then(Json::inteiro).unwrap() as u64;
            let ate = volumes
                .get(i + 1)
                .and_then(|prox| prox.campo("primeiro_rowid"))
                .and_then(Json::inteiro)
                .map(|n| n as u64 - 1)
                .unwrap_or(total);
            ate - de + 1
        })
        .collect()
}

/// **A prova.** 25 linhas, 10 por arquivo: tem de nascer 3 volumes
/// (10, 10, 5), e `esquema.volumes` tem de contar isso sem que ninguem
/// precise adivinhar dividindo `slots` na mao.
///
/// # O defeito reposto
///
/// Voltando `RegFile::fronteiras` a devolver so' `self.fronteiras.clone()`
/// (o comportamento de antes do pedido 222), este teste cai na PRIMEIRA
/// asserção depois do `esquema`: `volumes.len()` vem `0`, nao `3` -- o
/// campo volta vazio porque a particao por quantidade nunca alimentou
/// aquele cache.
#[test]
fn esquema_devolve_volumes_com_a_contagem_certa_na_particao_por_quantidade() {
    let guarda = dir_temp("basico");
    let s = servidor(&guarda);
    pede(
        &s,
        r#""op":"criar_tabela","database":"b","tabela":"grande",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                          {"nome":"nome","tipo":"Str","tamanho":20}],
               "registros_por_arquivo":10,"max_arquivos":50,
               "particao":"quantidade""#,
    )
    .unwrap();

    for id in 1..=25i64 {
        pede(
            &s,
            &format!(
                r#""op":"inserir","database":"b","tabela":"grande",
                       "linha":{{"id":{id},"nome":"r{id}"}}"#
            ),
        )
        .unwrap();
    }

    let e = pede(&s, r#""op":"esquema","database":"b","tabela":"grande""#).unwrap();
    assert_eq!(e.texto_ou("tabela", ""), "grande");
    let pag = e.campo("paginacao").unwrap();
    assert_eq!(pag.texto_ou("modo", ""), "quantidade");

    let volumes = e.campo("volumes").and_then(Json::lista).unwrap();
    assert_eq!(
        volumes.len(),
        3,
        "25 linhas / 10 por arquivo = 3 volumes; veio {volumes:?}"
    );
    let primeiros: Vec<i64> = volumes
        .iter()
        .map(|v| v.campo("primeiro_rowid").and_then(Json::inteiro).unwrap())
        .collect();
    assert_eq!(
        primeiros,
        vec![1, 11, 21],
        "a mesma conta que o enderecamento ja fazia"
    );
    // Sem periodo neste modo: a tela nao pode confundir volume com data.
    assert!(
        volumes
            .iter()
            .all(|v| v.campo("periodo") == Some(&Json::Nulo)),
        "por quantidade nao tem periodo: {volumes:?}"
    );
    assert_eq!(contagens(volumes, 25), vec![10, 10, 5]);
}

/// CONTROLE: a particao por periodo continua devolvendo a fronteira dela
/// sem mudar em nada. `RegFile::fronteiras` so' calcula quando o cache
/// esta vazio, e na por periodo ele nunca esta -- o volume 1 ja nasce com
/// uma fronteira gravada.
#[test]
fn esquema_continua_devolvendo_volumes_na_particao_por_periodo() {
    let guarda = dir_temp("controle-periodo");
    let s = servidor(&guarda);
    pede(
        &s,
        r#""op":"criar_tabela","database":"b","tabela":"lancamentos",
               "colunas":[{"nome":"id","tipo":"Int8","obrigatoria":true},
                          {"nome":"emissao","tipo":"Date","obrigatoria":true}],
               "registros_por_arquivo":1000,
               "particao":"mensal","particao_coluna":"emissao""#,
    )
    .unwrap();

    for mes in 1..=3u32 {
        pede(
            &s,
            &format!(
                r#""op":"inserir","database":"b","tabela":"lancamentos",
                       "linha":{{"id":{mes},"emissao":"2026-{mes:02}-05"}}"#
            ),
        )
        .unwrap();
    }

    let e = pede(
        &s,
        r#""op":"esquema","database":"b","tabela":"lancamentos""#,
    )
    .unwrap();
    let volumes = e.campo("volumes").and_then(Json::lista).unwrap();
    assert_eq!(volumes.len(), 3, "um volume por mes; veio {volumes:?}");
    assert_eq!(
        volumes[0].texto_ou("periodo", ""),
        "2026-01",
        "a particao por periodo continua rotulando o mes"
    );
    assert_eq!(
        volumes[0].campo("primeiro_rowid").and_then(Json::inteiro),
        Some(1)
    );
    assert_eq!(
        volumes[1].campo("primeiro_rowid").and_then(Json::inteiro),
        Some(2)
    );
    assert_eq!(
        volumes[2].campo("primeiro_rowid").and_then(Json::inteiro),
        Some(3)
    );
}

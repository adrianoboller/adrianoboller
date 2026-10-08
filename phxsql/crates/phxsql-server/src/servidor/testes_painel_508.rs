//! O painel soma os bytes do `.reg` pelo caminho que o MOTOR compoe (pedido
//! 508), e nao por uma copia do analisador de nome.
use super::*;
use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;

fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
    let mut c = Config {
        base: dir.to_path_buf(),
        log_acessos: dir.join("acessos.log"),
        blacklist: dir.join("blacklist.json"),
        dblink: dir.join("dblink.json"),
        jobs: dir.join("jobs.json"),
        token: "t".into(),
        ..Config::default()
    };
    c.cifra_fio.exigir = false;
    Servidor::novo(c).unwrap()
}

/// Quantos bytes os `.reg` da tabela ocupam, pelo SISTEMA DE ARQUIVOS --
/// o `du` do teste, que nao passa pelo motor que esta sendo provado.
fn no_disco(dir: &std::path::Path, tabela: &str) -> u64 {
    std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.ends_with(".reg")
                && (n == format!("{tabela}.reg") || n.starts_with(&format!("{tabela}#")))
        })
        .map(|e| e.metadata().unwrap().len())
        .sum()
}

/// **O painel mede o que o disco tem, em toda regra de nome de volume.**
///
/// A copia do analisador que morava no `op_painel` compunha `x.reg` para o
/// volume 1 e `x_NNN.reg` de tres digitos para os outros: numa tabela
/// paginada o volume 1 nunca entrava na soma, e numa de 4 digitos ou por
/// letra nenhum volume entrava -- medido antes do conserto, 0 contra
/// 37.884 bytes (4 digitos), 0 contra 39.676 (por letra) e 27.188 contra
/// 37.884 (3 digitos, 350 linhas em cada).
///
/// **Defeito reposto** (`Table::caminhos_do_reg` com a composicao velha):
/// as tres somas saem menores que o disco e o `assert_eq!` cai.
#[test]
fn painel_soma_os_bytes_do_disco() {
    let d = crate::apoio_teste::DirTemp::novo("painel-508");
    let s = servidor(&d.0);
    let inst = phxsql_store::catalogo::Instancia::nova(&d.0).unwrap();
    let db = inst.criar_database("loja").unwrap();
    let colunas = || {
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("nome", ColumnType::Str(40)).obrigatoria(),
        ]
    };
    let tabelas = [
        ("d3", Paginacao::nova(100, 99).unwrap()),
        (
            "d4",
            Paginacao::nova(100, 99).unwrap().com_digitos(4).unwrap(),
        ),
        ("letra", Paginacao::por_letra(1_000, 1).unwrap()),
    ];
    let nomes = ["Ana", "Bruno", "Carla", "Zeca", "0800", "Ovo"];
    for (nome, p) in tabelas {
        let e = Schema::new(nome, colunas(), vec![])
            .unwrap()
            .com_paginacao(p)
            .unwrap();
        let mut t = db.criar_tabela(None, e).unwrap();
        for i in 0..350i64 {
            t.inserir(&[
                Value::Int(i),
                Value::Str(nomes[i as usize % nomes.len()].into()),
            ])
            .unwrap();
        }
        t.sincronizar().unwrap();
    }
    let painel = s.op_painel(&Sessao::default()).unwrap();
    let maiores = painel
        .campo("maiores_tabelas")
        .and_then(Json::lista)
        .unwrap();
    let bytes = |t: &str| {
        maiores
            .iter()
            .find(|m| m.texto_ou("tabela", "") == format!("loja/{t}"))
            .map(|m| m.inteiro_ou("bytes", -1))
            .unwrap()
    };
    let loja = d.0.join("loja");
    for t in ["d3", "d4", "letra"] {
        let disco = no_disco(&loja, t);
        eprintln!("508 painel: {t}: painel={} disco={disco}", bytes(t));
        assert!(disco > 0);
        assert_eq!(bytes(t), disco as i64, "{t}: o painel nao mede o disco");
    }
}

# Trilhas `.lgpd` gravadas pelo código de ANTES do formato B (pedido 368)

Três tabelas `clientes` (coluna `email` marcada como dado pessoal), gravadas em
24/09/2026 pelo código do commit **`82a17ef`** — a árvore exportada por
`git archive 82a17ef phxsql`, nunca a árvore nova. É o que a prova da migração
abre: a trilha nasceu do código antigo, e não de uma imitação dele.

| pasta | tabela | a trilha no disco |
|---|---|---|
| `unico/` | sem paginação | `clientes.lgpd` (volume 1), 5 registros |
| `paginada/` | `registros_por_arquivo` 10, `bytes_por_arquivo` 2 KiB | `clientes_001.lgpd` a `_003`, 60 registros, sem `clientes.lgpd` |
| `paginada-4-digitos/` | a mesma, com sufixo de **quatro** dígitos | `clientes_0001.lgpd` a `_0003`, 60 registros |

Os testes copiam a pasta para um diretório temporário antes de abrir: abrir
com escrita migraria os arquivos daqui.

## Como foram gerados

```bash
git archive 82a17ef phxsql | tar -x -C /tmp/antes-do-b
# o gerador do fim desta pagina em /tmp/antes-do-b/phxsql/crates/phxsql-store/tests/gerar-trilha-antes-do-b.rs
PHX_FIXTURE_DESTINO=<pasta> cargo test -p phxsql-store --test gerar-trilha-antes-do-b -- --nocapture
```

O gerador cria cada tabela com `Table::criar`, insere a linha 1 com
`a0@x.com` e a altera N vezes para `a1@x.com` … `aN@x.com` (5 na sem paginação,
60 nas paginadas), com o usuário 7. Cada alteração grava UM registro na trilha
(só o `email` muda). A saída da corrida que gerou estes arquivos:

```text
unico: 5 registros, arquivos ["clientes.lgpd"]
paginada: 60 registros, arquivos ["clientes_001.lgpd", "clientes_002.lgpd", "clientes_003.lgpd"]
paginada-4-digitos: 60 registros, arquivos ["clientes_0001.lgpd", "clientes_0002.lgpd", "clientes_0003.lgpd"]
```

A paginada de quatro dígitos: `Paginacao::nova(10, 1)`, `com_digitos(4)`,
`com_max_arquivos(9_999)`, `com_bytes_por_arquivo(2_048)`. O gerador não fica
no repositório como teste porque só grava o formato antigo com o código
antigo: rodado na árvore nova, ele gravaria o formato novo e a pasta mentiria
sobre a origem dos arquivos. O texto dele fica aqui, para a pasta poder ser
refeita:

```rust
//! Gera as trilhas `.lgpd` no formato de ANTES do formato B (pedido 368),
//! com o codigo de antes -- para a prova da migracao abrir o que ele gravou.
//!
//! Roda contra `git archive 82a17ef`, nunca contra a arvore nova:
//!
//! ```bash
//! PHX_FIXTURE_DESTINO=<dir> cargo test -p phxsql-store --test gerar-trilha-antes-do-b -- --nocapture
//! ```

use phxsql_core::paginacao::Paginacao;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::{ColumnType, DadoPessoal};
use phxsql_core::value::Value;
use phxsql_store::table::Table;

fn esquema(pag: Option<Paginacao>) -> Schema {
    let e = Schema::new(
        "clientes",
        vec![
            Column::new("id", ColumnType::Int8).obrigatoria(),
            Column::new("email", ColumnType::Str(40)).com_dado_pessoal(DadoPessoal::Pessoal),
        ],
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])
            .unico()
            .primaria()],
    )
    .unwrap();
    match pag {
        None => e,
        Some(p) => e.com_paginacao(p).unwrap(),
    }
}

fn gerar(destino: &std::path::Path, caso: &str, pag: Option<Paginacao>, alteracoes: u32) {
    let d = destino.join(caso);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let mut t = Table::criar(&d, esquema(pag)).unwrap();
    t.definir_usuario(7);
    t.inserir(&[Value::Int(1), Value::Str("a0@x.com".into())])
        .unwrap();
    for i in 1..=alteracoes {
        t.atualizar(1, &[Value::Int(1), Value::Str(format!("a{i}@x.com"))])
            .unwrap();
    }
    t.sincronizar().unwrap();
    let total = t.total_da_trilha().unwrap();
    let volumes = "(nao exposto no codigo de antes)";
    drop(t);
    let mut nomes: Vec<String> = std::fs::read_dir(&d)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".lgpd"))
        .collect();
    nomes.sort();
    println!("{caso}: {total} registros, volumes {volumes}, arquivos {nomes:?}");
}

#[test]
fn gerar_as_tres_trilhas_de_antes() {
    let destino = std::path::PathBuf::from(
        std::env::var("PHX_FIXTURE_DESTINO").expect("PHX_FIXTURE_DESTINO"),
    );
    // Tabela sem paginacao: um arquivo so, `clientes.lgpd`, volume 1.
    gerar(&destino, "unico", None, 5);
    // Paginada, volume do diario de 2 KiB: `clientes_001.lgpd` ate `_00K`.
    let pag = Paginacao::nova(10, 999)
        .unwrap()
        .com_bytes_por_arquivo(2_048)
        .unwrap();
    gerar(&destino, "paginada", Some(pag), 60);
    // Paginada com sufixo de QUATRO digitos: `clientes_0001.lgpd`...
    let pag4 = Paginacao::nova(10, 1)
        .unwrap()
        .com_digitos(4)
        .unwrap()
        .com_max_arquivos(9_999)
        .unwrap()
        .com_bytes_por_arquivo(2_048)
        .unwrap();
    gerar(&destino, "paginada-4-digitos", Some(pag4), 60);
}
```

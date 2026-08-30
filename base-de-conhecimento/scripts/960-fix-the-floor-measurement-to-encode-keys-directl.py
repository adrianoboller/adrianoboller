# Fix the floor measurement to encode keys directly
# 29/08 00:53

import pathlib, re
p = pathlib.Path("crates/phxsql-store/examples/indice-adiado.rs")
s = p.read_text()
inicio = s.index("/// O PISO de uma reconstrucao em lote de verdade.")
fim = s.index("fn main() {")
novo = '''/// O PISO de uma reconstrucao em lote de verdade.
///
/// `reindexar` hoje insere chave a chave -- uma descida na arvore por chave, o
/// mesmo trabalho do caminho de dentro. E por isso que adiar quase nao compra.
/// Uma reconstrucao EM LOTE seria outra coisa: varrer o `.reg`, codificar as
/// chaves, ORDENAR, e encher as folhas em sequencia, montando os niveis de
/// cima por cima. Nenhuma descida.
///
/// Este medidor faz as tres primeiras partes -- varrer, codificar, ordenar --
/// e cronometra. E o piso: encher folha em sequencia custa o CRC de cada
/// pagina, e sao poucas paginas.
fn piso_do_lote(ls: &[Vec<Value>], custo_de_hoje: f64) {
    let dir = std::env::temp_dir().join(format!("phx-piso-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let esquema = Schema::new("precos", colunas(), vec![unico(), nao_unico()]).unwrap();
    let mut t = Table::criar(&dir, esquema).unwrap();
    for l in ls {
        t.inserir(l).unwrap();
    }
    t.sincronizar().unwrap();
    let paginas = t.paginas_indice();

    // A chave do indice unico e a coluna 0 (Int8), codificada pelo mesmo
    // `keyenc` que a arvore usa -- a codificacao preserva ordem, entao ordenar
    // os bytes e ordenar os valores.
    let tipo = ColumnType::Int8;
    let largura = largura_componente(&tipo).unwrap();

    let inicio = Instant::now();
    let linhas_lidas = t.varrer().unwrap();
    let mut chaves: Vec<(Vec<u8>, u64)> = Vec::with_capacity(linhas_lidas.len());
    for (rowid, linha) in &linhas_lidas {
        let mut buf = vec![0u8; largura];
        escrever_componente(&linha[0], &tipo, false, false, &mut buf).unwrap();
        chaves.push((buf, *rowid));
    }
    let varrer = inicio.elapsed().as_secs_f64();

    let inicio = Instant::now();
    chaves.sort_unstable();
    let ordenar = inicio.elapsed().as_secs_f64();

    let _ = std::fs::remove_dir_all(&dir);

    println!("\\n=== o piso de uma reconstrucao EM LOTE ===\\n");
    println!(
        "  varrer o `.reg` e codificar {} chaves .... {varrer:.2}s",
        chaves.len()
    );
    println!("  ordenar as chaves ....................... {ordenar:.2}s");
    println!("  paginas de indice a encher .............. {paginas}");
    println!(
        "\\n  Piso de ~{:.2}s por indice, contra os {custo_de_hoje:.2}s que o\\n  \
         `reindexar` de hoje custa para os dois. E ai que mora o ganho de\\n  \
         adiar -- e nao no adiar em si.",
        varrer + ordenar
    );
}

'''
s = s[:inicio] + novo + s[fim:]

s = s.replace('''use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};''',
'''use phxsql_core::keyenc::{escrever_componente, largura_componente};
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};''', 1)

# rodada devolve o custo do reindexar dos dois, para o piso comparar
s = s.replace("fn rodada(nome: &str, n: i64, embaralhar: bool) {",
              "fn rodada(nome: &str, n: i64, embaralhar: bool) -> f64 {", 1)
s = s.replace('''        dois.reindexar,
        so_unico.reindexar,
        dois.reindexar - so_unico.reindexar
    );
}''','''        dois.reindexar,
        so_unico.reindexar,
        dois.reindexar - so_unico.reindexar
    );
    dois.reindexar
}''', 1)
s = s.replace('''    rodada("chaves crescentes (arquivo ja ordenado)", n, false);
    rodada("chaves embaralhadas (o caso comum)", n, true);''',
'''    rodada("chaves crescentes (arquivo ja ordenado)", n, false);
    let custo_de_hoje = rodada("chaves embaralhadas (o caso comum)", n, true);
    piso_do_lote(&linhas(n, true), custo_de_hoje);''', 1)
p.write_text(s)
print("ok")

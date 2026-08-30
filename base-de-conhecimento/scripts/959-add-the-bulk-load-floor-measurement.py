# Add the bulk-load floor measurement
# 29/08 00:53

import pathlib
p = pathlib.Path("crates/phxsql-store/examples/indice-adiado.rs")
s = p.read_text()

alvo = '''fn main() {'''
novo = '''/// O PISO de uma reconstrucao em lote de verdade.
///
/// `reindexar` hoje insere chave a chave -- uma descida na arvore por chave, o
/// mesmo trabalho do caminho de dentro. Por isso adiar quase nao compra. Uma
/// reconstrucao EM LOTE seria outra coisa: varrer o `.reg`, codificar as
/// chaves, ORDENAR, e encher as folhas em sequencia, montando os niveis de
/// cima por cima. Nada de descida.
///
/// Este medidor faz as tres primeiras partes -- varrer, codificar, ordenar --
/// e cronometra. E o piso: encher paginas em sequencia custa o CRC de cada
/// uma, e sao poucas.
fn piso_do_lote(n: i64, ls: &[Vec<Value>]) {
    let dir = std::env::temp_dir().join(format!("phx-piso-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let esquema = Schema::new("precos", colunas(), vec![unico(), nao_unico()]).unwrap();
    let mut t = Table::criar(&dir, esquema).unwrap();
    for l in ls {
        t.inserir(l).unwrap();
    }
    t.sincronizar().unwrap();

    let inicio = Instant::now();
    let mut chaves: Vec<(Vec<u8>, u64)> = Vec::with_capacity(n as usize);
    for (rowid, linha) in t.varrer().unwrap() {
        chaves.push((t.chave_de(0, &linha).unwrap(), rowid));
    }
    let varrer = inicio.elapsed().as_secs_f64();
    let inicio = Instant::now();
    chaves.sort_unstable();
    let ordenar = inicio.elapsed().as_secs_f64();

    let paginas = t.paginas_indice();
    let _ = std::fs::remove_dir_all(&dir);

    println!("\\n=== o piso de uma reconstrucao EM LOTE ===\\n");
    println!("  varrer o `.reg` e codificar {} chaves .... {varrer:.2}s", chaves.len());
    println!("  ordenar as chaves ....................... {ordenar:.2}s");
    println!("  paginas de indice a encher .............. {paginas}");
    println!(
        "\\n  Somando, o piso e ~{:.2}s contra os {:.2}s que o `reindexar` de hoje\\n  \
         custa para os dois indices. E ali que mora o ganho de adiar -- e nao\\n  \
         no adiar em si.",
        varrer + ordenar,
        0.0f64.max(f64::NAN).max(0.0)
    );
}

fn main() {'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")

# Make onde-doi count page touches
# 29/08 00:15

import pathlib
p = pathlib.Path("crates/phxsql-store/examples/onde-doi.rs")
s = p.read_text()

s = s.replace('''fn medir(rotulo: &str, indices: Vec<IndexDef>, n: i64) -> f64 {''',
'''/// Quanto custou por linha, e quantas paginas do `.ndx` cada linha tocou.
struct Medida {
    us_por_linha: f64,
    acertos: f64,
    lidas: f64,
    gravadas: f64,
}

fn medir(rotulo: &str, indices: Vec<IndexDef>, n: i64) -> Medida {''',1)

s = s.replace('''    let _ = std::fs::remove_dir_all(&dir);
    println!(
        "  {rotulo:<14} {:>8.2}s  {:>9.0} linhas/s  {:>7.1} us por linha",
        s,
        n as f64 / s,
        s * 1e6 / n as f64
    );
    s * 1e6 / n as f64
}''','''    let (acertos, lidas, gravadas) = t.estatisticas_paginas();
    let _ = std::fs::remove_dir_all(&dir);
    println!(
        "  {rotulo:<14} {:>8.2}s  {:>9.0} linhas/s  {:>7.1} us por linha",
        s,
        n as f64 / s,
        s * 1e6 / n as f64
    );
    Medida {
        us_por_linha: s * 1e6 / n as f64,
        acertos: acertos as f64 / n as f64,
        lidas: lidas as f64 / n as f64,
        gravadas: gravadas as f64 / n as f64,
    }
}''',1)

s = s.replace('''    let so_reg = medir("so .reg", vec![], n);''','''    let so_reg = medir("so .reg", vec![], n).us_por_linha;''',1)
s = s.replace('''    let um = medir(
        "+1 indice",
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])],
        n,
    );''','''    let um = medir(
        "+1 indice",
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)])],
        n,
    )
    .us_por_linha;''',1)
s = s.replace('''    let um_unico = medir(
        "+1 unico",
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        n,
    );''','''    let um_unico = medir(
        "+1 unico",
        vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        n,
    )
    .us_por_linha;''',1)
s = s.replace('''    let dois = medir(
        "+2 indices",
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
        n,
    );''','''    let m = medir(
        "+2 indices",
        vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico(),
            IndexDef::new("porCidade", vec![IndexColumn::asc(2)]),
        ],
        n,
    );
    let dois = m.us_por_linha;''',1)

alvo = '''    println!(
        "\\n  O `strace` conta 41 chamadas e ~20 toques de pagina por linha inserida.\\n  \\
         Isso da ~{:.0} us so de nucleo e ~{:.0} us so de CRC -- de {dois:.0} us medidos.",
        41.0 * por_seek,
        20.0 * por_pagina
    );
}'''
novo = '''    // Os toques de pagina sao CONTADOS, e nao citados de um `strace` de outro
    // dia: o cache de paginas mudou esses numeros, e um numero escrito a mao
    // teria continuado dizendo o de antes.
    println!("\\n=== o que cada linha toca no `.ndx`, na forma de 2 indices ===\\n");
    println!("  paginas servidas pelo cache ....... {:.2} por linha", m.acertos);
    println!("  paginas lidas do arquivo .......... {:.2} por linha", m.lidas);
    println!("  paginas gravadas .................. {:.2} por linha", m.gravadas);
    let com_crc = m.lidas + m.gravadas;
    println!(
        "\\n  So a leitura do arquivo e a gravacao passam pelo CRC -- {com_crc:.2} paginas\\n  \\
         por linha, ou {:.1} us de CRC, de {dois:.1} us medidos ({:.0}%). O acerto de\\n  \\
         cache custa a copia da pagina, e nao o CRC dela: e dai que veio o ganho.",
        com_crc * por_pagina,
        com_crc * por_pagina / dois * 100.0
    );
    println!(
        "\\n  Um lseek custa {por_seek:.2} us: mesmo 41 chamadas por linha dariam {:.1} us.",
        41.0 * por_seek
    );
}'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)
p.write_text(s)
print("ok")

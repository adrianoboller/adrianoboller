# Medir tambem a copia da pagina
# 29/08 04:12

import io
p='crates/phxsql-store/examples/onde-doi.rs'
s=io.open(p,encoding='utf-8').read()
velho = '''    println!("\\n=== as duas suspeitas do caminho de cada pagina ===\\n");
    println!("  CRC-32 de uma pagina de 4 KiB .... {por_pagina:.2} us   (acumulador {acc:x})");'''
novo = '''    // A terceira suspeita, que o proprio texto abaixo insinuava sem medir:
    // `ler_pagina` devolve `Vec<u8>`, entao um ACERTO de cache copia os 4 KiB
    // inteiros. Nao paga CRC, mas paga a copia -- e o numero de acertos cresce
    // com a altura da arvore, que cresce com a tabela.
    let inicio = Instant::now();
    let mut soma = 0usize;
    for _ in 0..voltas {
        let copia = pagina.clone();
        soma += copia[0] as usize;
    }
    let copia_pagina = inicio.elapsed().as_secs_f64() * 1e6 / voltas as f64;

    println!("\\n=== as tres suspeitas do caminho de cada pagina ===\\n");
    println!("  CRC-32 de uma pagina de 4 KiB .... {por_pagina:.2} us   (acumulador {acc:x})");
    println!("  COPIAR uma pagina de 4 KiB ....... {copia_pagina:.2} us   (soma {soma})");'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')

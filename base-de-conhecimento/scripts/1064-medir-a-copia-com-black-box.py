# Medir a copia com black_box
# 29/08 04:14

import io
p='crates/phxsql-store/examples/onde-doi.rs'
s=io.open(p,encoding='utf-8').read()
velho = '''    let inicio = Instant::now();
    let mut soma = 0usize;
    for _ in 0..voltas {
        let copia = pagina.clone();
        soma += copia[0] as usize;
    }
    let copia_pagina = inicio.elapsed().as_secs_f64() * 1e6 / voltas as f64;'''
novo = '''    //
    // `black_box` nao e decoracao: sem ele o LLVM ve que a copia nao e usada e
    // apaga o laco inteiro -- a primeira versao deste medidor mediu 0,00 us
    // para copiar 4 KiB, que e impossivel, e o numero passaria como bom.
    let inicio = Instant::now();
    let mut soma = 0usize;
    for _ in 0..voltas {
        let copia = std::hint::black_box(&pagina).clone();
        soma += std::hint::black_box(&copia)[0] as usize;
    }
    let copia_pagina = inicio.elapsed().as_secs_f64() * 1e6 / voltas as f64;'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')

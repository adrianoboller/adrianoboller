# Medir de novo com as chaves caindo no meio
# 29/08 03:28

import io
p='crates/phxsql-store/examples/indice-em-lote.rs'
s=io.open(p,encoding='utf-8').read()
velho = """    // As chaves ficam embaralhadas: o caso comum, e o unico em que a ordem da
    // insercao importa. As que entram DEPOIS sao 10% do total, tambem
    // espalhadas, para cair em folhas diferentes.
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut valores: Vec<i64> = (1..=n as i64).collect();
    for i in (1..valores.len()).rev() {
        let j = (rng.proximo() % (i as u64 + 1)) as usize;
        valores.swap(i, j);
    }
    let cresceu = n / 10;
    let depois: Vec<i64> = (0..cresceu).map(|i| n as i64 + 1 + i as i64 * 7).collect();"""
novo = """    // As chaves ficam embaralhadas: o caso comum, e o unico em que a ordem da
    // insercao importa.
    //
    // As que entram DEPOIS sao 10% do total e caem NO MEIO da faixa, e nao
    // acima dela: chave maior que todas vai sempre para a ultima folha, e ai a
    // divisao que o enchimento deveria provocar nunca acontece -- foi assim que
    // a primeira versao deste medidor deu 100% de graca. As de tras sao pares,
    // as de crescer sao impares.
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut valores: Vec<i64> = (1..=n as i64).map(|i| i * 2).collect();
    for i in (1..valores.len()).rev() {
        let j = (rng.proximo() % (i as u64 + 1)) as usize;
        valores.swap(i, j);
    }
    let cresceu = n / 10;
    let mut depois: Vec<i64> = (0..cresceu).map(|i| (i as i64 * 10 + 1) % (n as i64 * 2)).collect();
    depois.sort_unstable();
    depois.dedup();
    for i in (1..depois.len()).rev() {
        let j = (rng.proximo() % (i as u64 + 1)) as usize;
        depois.swap(i, j);
    }
    let cresceu = depois.len();"""
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')

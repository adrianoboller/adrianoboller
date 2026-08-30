# Corrigir o hex e o sono do laco
# 29/08 03:40

import io
p='crates/phxsql-server/src/valores.rs'
s=io.open(p,encoding='utf-8').read()
velho = '''pub fn bytes_para_hex(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for byte in b {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}'''
novo = '''pub fn bytes_para_hex(b: &[u8]) -> String {
    // Tabela em vez de `format!("{byte:02x}")`: o `format!` alocava uma String
    // POR BYTE, e a imagem de uma linha tem dezenas deles. Medido em
    // `--example onde-doi-na-replica`: era 3,48 us por evento da replicacao,
    // 14,6% de todo o caminho de CPU dos dois lados.
    const DIG: &[u8; 16] = b"0123456789abcdef";
    let mut s = Vec::with_capacity(b.len() * 2);
    for byte in b {
        s.push(DIG[(byte >> 4) as usize]);
        s.push(DIG[(byte & 0x0f) as usize]);
    }
    // Só saiu de `DIG`, que é ASCII: nao ha como nao ser UTF-8.
    String::from_utf8(s).unwrap_or_default()
}'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('hex ok')

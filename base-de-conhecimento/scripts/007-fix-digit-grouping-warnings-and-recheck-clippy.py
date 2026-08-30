# Fix digit grouping warnings and recheck clippy
# 27/08 17:58

import re
p='crates/phxsql-cli/src/main.rs'
s=open(p).read()
s=s.replace('''    let amostra = [
        (1i64, "Adriano Boller", "Blumenau", 1_500_00i128),
        (2, "Marcia Alves", "Joinville", 320_00),
        (3, "Zuleica Prado", "Blumenau", 890_00),
        (4, "Beatriz Nunes", "Itajai", 45_00),
        (5, "Carlos Menezes", "Blumenau", 2_700_00),
    ];''','''    // O limite e Decimal(15,2): os valores ja vao escalados por 100,
    // entao 150_000 significa R$ 1.500,00.
    let amostra = [
        (1i64, "Adriano Boller", "Blumenau", 150_000i128),
        (2, "Marcia Alves", "Joinville", 32_000),
        (3, "Zuleica Prado", "Blumenau", 89_000),
        (4, "Beatriz Nunes", "Itajai", 4_500),
        (5, "Carlos Menezes", "Blumenau", 270_000),
    ];''')
open(p,'w').write(s)

p='crates/phxsql-store/tests/tabela.rs'
s=open(p).read()
s=s.replace('let mut linha = cliente(1, "Adriano Boller", "Blumenau", 1_500_00);','// Decimal(15,2): 150_000 = R$ 1.500,00\n    let mut linha = cliente(1, "Adriano Boller", "Blumenau", 150_000);')
open(p,'w').write(s)

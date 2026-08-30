# Fix byte loading and retest
# 27/08 20:43

p='crates/phxsql-core/src/ed25519.rs'
s=open(p).read()
velho = '''    [
        carregar(0, 7) & MASCARA,
        (carregar(6, 7) >> 3) & MASCARA,
        (carregar(12, 7) >> 6) & MASCARA,
        (carregar(19, 7) >> 1) & MASCARA,
        // O bit 255 e o sinal, e nao faz parte do numero.
        (carregar(24, 8) >> 12) & MASCARA,
    ]'''
novo = '''    // Oito bytes em cada leitura, sempre. Com sete, o pedaco do meio perde o
    // bit 152 -- e o defeito passa despercebido, porque o ponto base tem esse
    // bit em zero.
    [
        carregar(0, 8) & MASCARA,
        (carregar(6, 8) >> 3) & MASCARA,
        (carregar(12, 8) >> 6) & MASCARA,
        (carregar(19, 8) >> 1) & MASCARA,
        // O bit 255 e o sinal, e nao faz parte do numero.
        (carregar(24, 8) >> 12) & MASCARA,
    ]'''
assert s.count(velho)==1
open(p,'w').write(s.replace(velho,novo))

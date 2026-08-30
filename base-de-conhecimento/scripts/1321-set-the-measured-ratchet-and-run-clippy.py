# Set the measured ratchet and run clippy
# 30/08 17:41

p='crates/phxsql-server/src/conferidor.rs'
s=open(p,encoding='utf-8').read()
velho='pub const TETO: usize = 1_806;'
novo='''/// 1.806 -> 1.771 na integracao da frente das transacoes. Nenhum dos dois
/// lados do merge tinha este numero, e nao tinham como ter: a frente media
/// 1.961 sobre a base 1.996 dela, a integracao anterior tinha deixado 1.806, e
/// a tela de transacoes -- escrita inteira pela fabrica -- derrubou mais 35.
/// Escolher um dos dois lados seria regressao silenciosa; o valor saiu de rodar
/// o conferidor depois do merge.
pub const TETO: usize = 1_771;'''
assert s.count(velho)==1
open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print("TETO -> 1.771, medido depois do merge")

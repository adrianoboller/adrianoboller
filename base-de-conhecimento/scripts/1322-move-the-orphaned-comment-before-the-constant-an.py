# Move the orphaned comment before the constant and recheck clippy
# 30/08 17:42

p='crates/phxsql-server/src/conferidor.rs'
s=open(p,encoding='utf-8').read()
orfao_ini = s.index('/// 1.996 -> 1.961 na frente das transacoes:')
orfao_fim = s.index('\n\n#[cfg(test)]', orfao_ini)
orfao = s[orfao_ini:orfao_fim].rstrip()
s = s[:orfao_ini] + s[orfao_fim:].lstrip('\n')

# A historia dela e de 1.996 -> 1.961 e vem ANTES da minha, que fecha em 1.771
# medido. Reescrevo a ponte para as duas se lerem em sequencia.
minha = '''/// 1.806 -> 1.771 na integracao da frente das transacoes. Nenhum dos dois
/// lados do merge tinha este numero, e nao tinham como ter: a frente media
/// 1.961 sobre a base 1.996 dela, a integracao anterior tinha deixado 1.806, e
/// a tela de transacoes -- escrita inteira pela fabrica -- derrubou mais 35.
/// Escolher um dos dois lados seria regressao silenciosa; o valor saiu de rodar
/// o conferidor depois do merge.
pub const TETO: usize = 1_771;'''
ponte = orfao.replace('/// 1.996 -> 1.961 na frente das transacoes:',
                      '/// A frente das transacoes, medindo sobre a base 1.996 dela, chegou a 1.961:') \
        + '\n' + minha
assert s.count(minha)==1
open(p,'w',encoding='utf-8').write(s.replace(minha, ponte))
print("comentario orfao movido para antes da constante, em ordem")

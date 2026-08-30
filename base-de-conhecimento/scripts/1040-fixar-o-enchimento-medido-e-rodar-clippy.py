# Fixar o enchimento medido e rodar clippy
# 29/08 03:29

import io
p='crates/phxsql-store/src/ndx.rs'
s=io.open(p,encoding='utf-8').read()
velho = '''/// Quanto de cada folha a construcao em lote enche, em porcento.
///
/// 100 daria a arvore mais compacta e a varredura mais rapida -- e faria a
/// PRIMEIRA insercao em cada folha dividir, porque a tabela recem-carregada
/// continua crescendo. 70 e a folga classica; o numero esta medido em
/// `--example indice-em-lote`, e nao chutado.
const ENCHIMENTO_PADRAO: usize = 70;'''
novo = '''/// Quanto de cada folha a construcao em lote enche, em porcento.
///
/// **80, e o numero e medido** (`--example indice-em-lote`, um milhao de chaves
/// e mais 10% inseridas depois no MEIO da faixa). Nao e nem a folga classica de
/// 70 nem o instinto de encher tudo:
///
/// ```text
///   enchimento   paginas   varrer   crescer   paginas novas
///          70%      6.028    0,035s    0,804s              0
///          80%      5.271    0,028s    0,770s              0   <- o joelho
///          90%      4.683    0,026s    0,901s          2.342
///         100%      4.213    0,023s    0,984s          2.110
/// ```
///
/// 70 nao compra nada: insercao aleatoria ja assenta perto de 69% de ocupacao
/// sozinha, e um resultado classico de B-tree. De 90 para cima a folha nao tem
/// mais folga, e crescer passa a alocar milhares de paginas e a ficar mais
/// LENTO do que a arvore mais frouxa -- a varredura mais rapida nao paga isso.
///
/// 80 e a ocupacao mais densa que ainda absorve 10% de crescimento sem alocar
/// uma pagina, e por isso e a mais rapida das duas pontas que importam.
const ENCHIMENTO_PADRAO: usize = 80;'''
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print('ok')

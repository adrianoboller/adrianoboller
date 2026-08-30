# Correct the ratchet to the measured truth
# 30/08 05:23

p='crates/phxsql-server/src/conferidor.rs'
s=open(p,encoding='utf-8').read()
velho='''/// 2.000 -> 1.999 na revisao do dossie 0.18: o item «Jobs» do Gerir banco
/// ganhou o par `rot:`/`txt:` ao passar a apontar para a tela que ja existia.
/// Um so, e ele desce a catraca junto -- catraca frouxa nao segura nada.
pub const TETO: usize = 1_999;'''
novo='''/// 2.000 -> 1.999 na revisao do dossie 0.18: o item «Jobs» do Gerir banco
/// ganhou o par `rot:`/`txt:` ao passar a apontar para a tela que ja existia.
/// Um so, e ele desce a catraca junto -- catraca frouxa nao segura nada.
///
/// 1.999 -> 2.068, e este e o unico tipo de subida que nao afrouxa nada: **o
/// numero de baixo era falso.** O `multitela.js` era servido pelo `http.rs` e
/// nao estava no `FONTES`, entao seus 69 textos cravados nunca foram contados
/// -- nao foram acrescentados agora, sempre estiveram la. 2.068 e a primeira
/// medida sobre a interface inteira; 1.999 era medida sobre cinco sextos dela.
/// A guarda `a_lista_cobre_tudo_que_o_http_serve` impede a proxima leitura
/// falsa, e a frente que traduz os 69 desce a catraca de volta.
pub const TETO: usize = 2_068;'''
assert s.count(velho)==1
open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print("TETO -> 2.068, com o motivo escrito")

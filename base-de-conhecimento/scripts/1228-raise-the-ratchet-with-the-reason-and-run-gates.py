# Raise the ratchet with the reason and run gates
# 29/08 23:44

import io
p="phxsql/crates/phxsql-server/src/conferidor.rs"
s=io.open(p,encoding="utf-8").read()
velho = "pub const TETO: usize = 1_994;"
novo = """/// **A unica subida registrada, e o motivo dela.** 1.994 -> 2.000, na
/// integracao em que esta catraca NASCEU. Tres frentes paralelas fecharam na
/// mesma rodada -- multitela, cores das bolhas e este conferidor -- e as duas
/// primeiras comecaram antes de a regra existir: nao havia como elas nascerem
/// na fabrica. Do que sobrou, as etiquetas curtas foram traduzidas na hora
/// (`tela.abas_da_regiao`, `tela.cores_de_fabrica` e as nove do menu Ver); os
/// seis que restam sao paragrafos de explicacao da tela de cores, e traduzi-los
/// as pressas numa integracao daria texto pior que deixa-los na fila. Estao
/// nomeados no `PENDENCIAS.md`.
///
/// **A partir daqui o numero so desce.** Subir de novo pede o mesmo que este
/// comentario: dizer quais textos, de qual frente, e por que nao couberam.
pub const TETO: usize = 2_000;"""
assert s.count(velho)==1
io.open(p,"w",encoding="utf-8").write(s.replace(velho,novo))
print("catraca a 2.000, com o motivo escrito")

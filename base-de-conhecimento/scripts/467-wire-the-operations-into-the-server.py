# Wire the operations into the server
# 28/08 15:28

p='/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/juncao_extra.rs'
s=open(p).read()
s=s.replace('("tipo", Json::texto_de(c.ty.nome())),','("tipo", Json::texto_de(format!("{:?}", c.ty))),')
open(p,'w').write(s)

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
ops=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/juncao_ops.rs').read()
extra=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/juncao_extra.rs').read()

marca='    // ------------------------------------------------------------- o DbLink\n'
assert marca in s
s=s.replace(marca, ops+"\n"+marca, 1)

marca2='''/// Percorre a tabela de fatos linha a linha, sem materializa-la.'''
assert marca2 in s
s=s.replace(marca2, extra.strip()+"\n\n"+marca2, 1)

# despacho
a='''            "pivotar" | "pivot" => self.op_pivotar(p, sessao),'''
b='''            "pivotar" | "pivot" => self.op_pivotar(p, sessao),
            "juntar" | "join" => self.op_juntar(p, sessao),
            "unir" | "union" => self.op_unir(p, sessao),'''
assert a in s; s=s.replace(a,b,1)

# imports
a='''use crate::dblink::{mysql, Definicao, Motor};'''
b='''use crate::dblink::{mysql, Definicao, Motor};
use crate::juncao::{Lado, Tipo as Juncao, Uniao};'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')

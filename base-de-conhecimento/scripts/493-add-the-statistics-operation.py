# Add the statistics operation
# 28/08 16:27

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
ops=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/estat.rs').read()
extra=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/contagem.rs').read()
marca='    // -------------------------------------------------- junção e união\n'
assert marca in s
s=s.replace(marca, ops+"\n"+marca, 1)
marca2='''/// Percorre a tabela de fatos linha a linha, sem materializa-la.'''
s=s.replace(marca2, extra.strip()+"\n\n"+marca2, 1)
a='''            "painel" => self.op_painel(sessao),'''
b='''            "painel" => self.op_painel(sessao),
            "estatisticas" | "estatisticas_uso" => self.op_estatisticas(p),'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)

# Ligar as operacoes no despachar
# 29/08 11:35

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()

# 1. despachar: as duas operacoes novas
velho='''            "dblink_consultar" => self.op_dblink_consultar(p),'''
novo='''            "dblink_consultar" => self.op_dblink_consultar(p),
            "dblink_ligar" => self.op_dblink_ligar(p, sessao),
            "dblink_sincronizar" => self.op_dblink_sincronizar(p, sessao),'''
assert s.count(velho)==1
s=s.replace(velho,novo)

# 2. escrevem: entram na lista de escrita
velho2='''pub(crate) const OPS_ESCRITA: &[&str] = &[
    "inserir",'''
novo2='''pub(crate) const OPS_ESCRITA: &[&str] = &[
    "inserir",
    // As duas da sincronia: ligar cria tabela local, sincronizar grava nela.
    "dblink_ligar",
    "dblink_sincronizar",'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)

# 3. dblink_salvar herda as sincronias quando o pedido nao as traz
velho3='''        if p.campo("senha").is_none() && d.senha_env.is_empty() {
            if let Ok(antiga) = r.achar(&d.nome) {
                d = d.com_a_senha_de(antiga);
            }
        }'''
novo3='''        if let Ok(antiga) = r.achar(&d.nome) {
            if p.campo("senha").is_none() && d.senha_env.is_empty() {
                d = d.com_a_senha_de(antiga);
            }
            // A tela salva sem mandar as sincronias; um salvar comum nao pode
            // apagar o que o assistente montou.
            if p.campo("sincronias").is_none() {
                d = d.com_as_sincronias_de(antiga);
            }
        }'''
assert s.count(velho3)==1
s=s.replace(velho3,novo3)
io.open(p,'w',encoding='utf-8').write(s)
print('despachar ok')

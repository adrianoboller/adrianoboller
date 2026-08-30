# Wire DbLink ops into servidor
# 28/08 14:48

p='crates/phxsql-server/src/dblink/mod.rs'
s=open(p).read()
a='''    pub fn senha(&self) -> &str {
        &self.senha
    }'''
b='''    pub fn senha(&self) -> &str {
        &self.senha
    }

    /// Esta definicao, com a senha de outra.
    ///
    /// Existe para a tela de edicao: ela nunca RECEBE a senha (o `para_json`
    /// nao a manda), entao nao teria como devolve-la, e sem isto mudar a porta
    /// apagaria a credencial.
    pub fn com_a_senha_de(mut self, outra: &Definicao) -> Definicao {
        self.senha = outra.senha.clone();
        self.senha_env = outra.senha_env.clone();
        self
    }'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
marca='''    // ------------------------------------------------------------- o DbLink
'''
novo=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/dblink_ops.rs').read()
if marca not in s:
    alvo='''    // ----------------------------------------------------- a maquina embaixo
'''
    assert alvo in s
    s=s.replace(alvo, novo+"\n"+alvo,1)

# campo no Servidor
a='''    /// Ultimo aviso mandado por caminho, para nao repetir enquanto o disco
    /// continua cheio.
    avisados: Mutex<HashMap<String, i64>>,'''
b='''    /// Ultimo aviso mandado por caminho, para nao repetir enquanto o disco
    /// continua cheio.
    avisados: Mutex<HashMap<String, i64>>,
    /// Ligacoes para bancos de fora.
    dblink: Mutex<crate::dblink::Registro>,'''
assert a in s; s=s.replace(a,b,1)
a='''            monitor: Mutex::new(crate::sistema::Monitor::novo()),'''
b='''            monitor: Mutex::new(crate::sistema::Monitor::novo()),
            dblink: Mutex::new(dblink),'''
assert a in s; s=s.replace(a,b,1)
a='''        let lista_negra = Blacklist::abrir(&config.blacklist)?;'''
b='''        let lista_negra = Blacklist::abrir(&config.blacklist)?;
        let dblink = crate::dblink::Registro::abrir(&config.dblink)?;'''
assert a in s; s=s.replace(a,b,1)

# despacho
a='''            "sistema" => Ok(self.op_sistema()),'''
b='''            "sistema" => Ok(self.op_sistema()),
            "dblink" => self.op_dblink(),
            "dblink_salvar" => self.op_dblink_salvar(p),
            "dblink_excluir" => self.op_dblink_excluir(p),
            "dblink_testar" => self.op_dblink_testar(p),
            "dblink_bancos" => self.op_dblink_bancos(p),
            "dblink_tabelas" => self.op_dblink_tabelas(p),
            "dblink_estrutura" => self.op_dblink_estrutura(p),
            "dblink_ler" => self.op_dblink_ler(p),
            "dblink_consultar" => self.op_dblink_consultar(p),'''
assert a in s; s=s.replace(a,b,1)

# escrita
a='''    "ajustar_sequencia",
];'''
b='''    "ajustar_sequencia",
    // Gravam o cadastro de ligacoes, que e arquivo deste servidor.
    "dblink_salvar",
    "dblink_excluir",
];'''
assert a in s; s=s.replace(a,b,1)

# imports
a='''use crate::config::{Config, Durabilidade};'''
b='''use crate::config::{Config, Durabilidade};
use crate::dblink::{mysql, Definicao, Motor};'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')

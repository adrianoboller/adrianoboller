# Add helper functions and build
# 28/08 17:40

import io
p='crates/phxsql-server/src/servidor.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    fn op_esquema(&self, p: &Json, sessao: &Sessao) -> Result<Json> {'''
novo='''    /// Nome de quem tem este id no cadastro. Vazio quando ninguem tem.
    ///
    /// O `.log`, o `.trash` e o `.reason` guardam o id numerico, e nao o nome:
    /// o id nao muda quando alguem e renomeado, e uma exclusao de 2019 tem de
    /// continuar apontando para a mesma pessoa. Traduzir na hora de MOSTRAR e
    /// o que faz o registro ser legivel sem prender o arquivo ao cadastro.
    fn nome_do_usuario(&self, id: u32) -> String {
        if id == 0 {
            return String::new();
        }
        self.config
            .cadastro
            .root
            .iter()
            .chain(self.config.cadastro.usuarios.iter())
            .find(|u| u.id == id)
            .map(|u| u.nome.clone())
            .unwrap_or_default()
    }

    fn op_esquema(&self, p: &Json, sessao: &Sessao) -> Result<Json> {'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)

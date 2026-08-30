# Persistir e expor as sincronias
# 29/08 11:35

import io
p='crates/phxsql-server/src/dblink/mod.rs'
s=io.open(p,encoding='utf-8').read()

# de_json le as sincronias (vindas do disco ou de um salvar completo)
velho='''            max_linhas: j
                .inteiro_ou("max_linhas", padrao.max_linhas as i64)
                .clamp(1, 100_000) as u64,
        })'''
novo='''            max_linhas: j
                .inteiro_ou("max_linhas", padrao.max_linhas as i64)
                .clamp(1, 100_000) as u64,
            sincronias: match j.campo("sincronias").and_then(Json::lista) {
                None => Vec::new(),
                Some(l) => l
                    .iter()
                    .map(sincronia::Sincronia::de_json)
                    .collect::<Result<Vec<_>>>()?,
            },
        })'''
assert s.count(velho)==1
s=s.replace(velho,novo)

# para_disco e para_json carregam as sincronias
velho2='''        if self.senha_env.is_empty() {
            campos.push(("senha", Json::texto_de(&self.senha)));
        } else {
            campos.push(("senha_env", Json::texto_de(&self.senha_env)));
        }
        Json::objeto(campos)
    }'''
novo2='''        if self.senha_env.is_empty() {
            campos.push(("senha", Json::texto_de(&self.senha)));
        } else {
            campos.push(("senha_env", Json::texto_de(&self.senha_env)));
        }
        if !self.sincronias.is_empty() {
            campos.push((
                "sincronias",
                Json::Lista(self.sincronias.iter().map(|s| s.para_json()).collect()),
            ));
        }
        Json::objeto(campos)
    }'''
assert s.count(velho2)==1
s=s.replace(velho2,novo2)

# para_json (a vista publica) tambem mostra
velho3='''    /// Como a definicao aparece na tela e no protocolo: sem a senha, nunca.
    pub fn para_json(&self) -> Json {
        Json::objeto(vec!['''
novo3='''    /// Como a definicao aparece na tela e no protocolo: sem a senha, nunca.
    pub fn para_json(&self) -> Json {
        let sincronias = Json::Lista(self.sincronias.iter().map(|s| s.para_json()).collect());
        Json::objeto(vec![
            ("sincronias", sincronias),'''
assert s.count(velho3)==1
s=s.replace(velho3,novo3)

# `com_a_senha_de` precisa existir do jeito que esta; sincronias de um salvar
# sem o campo nao podem APAGAR as existentes -- confere onde ela e usada
io.open(p,'w',encoding='utf-8').write(s)
print('ok')

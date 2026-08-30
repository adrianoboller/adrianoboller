# Complete the Usuario struct and add alguem_exige_chave
# 27/08 20:43

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
s=s.replace('''            ativo: j.booleano_ou("ativo", true),
            bases,
        })''','''            ativo: j.booleano_ou("ativo", true),
            chave_publica,
            bases,
        })''')

# ficha diz que ha chave, sem nunca mostrar a chave
s=s.replace('''    pub fn vazio(&self) -> bool {''','''    /// Algum usuario exige chave? A pagina de entrada usa isto para decidir
    /// se mostra o campo -- e nao ha segredo nenhum na resposta.
    pub fn alguem_exige_chave(&self) -> bool {
        self.root.iter().chain(self.usuarios.iter()).any(|u| u.chave_publica.is_some())
    }

    pub fn vazio(&self) -> bool {''')
open(p,'w').write(s)

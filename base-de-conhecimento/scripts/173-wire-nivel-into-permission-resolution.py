# Wire Nivel into permission resolution
# 27/08 21:09

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()

s=s.replace('''    pub fn permissoes(&self, database: &str) -> Permissoes {
        if self.supervisor {
            return Permissoes::tudo();
        }
        if let Some((_, p)) = self.bases.iter().find(|(b, _)| b == database) {
            return *p;
        }
        if let Some((_, p)) = self.bases.iter().find(|(b, _)| b == "*") {
            return *p;
        }
        Permissoes::default()
    }''','''    /// O poder deste usuario nesta base.
    ///
    /// Ordem de precedencia, do mais especifico para o mais geral:
    /// supervisor, a regra da base, a regra `"*"`, e por fim o nivel. O
    /// especifico ganha do geral -- e o que permite dar `admin` a alguem e
    /// ainda assim tirar uma base especifica dele.
    pub fn permissoes(&self, database: &str) -> Permissoes {
        if self.supervisor {
            return Permissoes::tudo();
        }
        if let Some((_, p)) = self.bases.iter().find(|(b, _)| b == database) {
            return *p;
        }
        if let Some((_, p)) = self.bases.iter().find(|(b, _)| b == "*") {
            return *p;
        }
        self.nivel.permissoes()
    }

    /// E administrador? Vale para operacao de servidor, que nao tem base.
    pub fn e_admin(&self) -> bool {
        self.supervisor || self.nivel >= Nivel::Admin
    }''')

s=s.replace('''        let chave_publica = match j.campo("chave_publica").and_then(Json::texto) {''',
'''        let nivel = Nivel::de_texto(j.texto_ou("nivel", ""))?;

        let chave_publica = match j.campo("chave_publica").and_then(Json::texto) {''')
s=s.replace('''            ativo: j.booleano_ou("ativo", true),
            chave_publica,''','''            ativo: j.booleano_ou("ativo", true),
            nivel,
            chave_publica,''')
s=s.replace('''            ("supervisor", Json::Bool(self.supervisor)),
            ("ativo", Json::Bool(self.ativo)),''','''            ("nivel", Json::texto_de(self.nivel.nome())),
            ("supervisor", Json::Bool(self.supervisor)),
            ("ativo", Json::Bool(self.ativo)),''')
open(p,'w').write(s)

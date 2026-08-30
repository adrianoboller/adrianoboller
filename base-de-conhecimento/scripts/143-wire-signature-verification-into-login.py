# Wire signature verification into login
# 27/08 20:44

p='crates/phxsql-server/src/usuarios.rs'
s=open(p).read()
s=s.replace('''            ("supervisor", Json::Bool(self.supervisor)),
            ("ativo", Json::Bool(self.ativo)),
            (
                "bases",''','''            ("supervisor", Json::Bool(self.supervisor)),
            ("ativo", Json::Bool(self.ativo)),
            // Diz que HA chave, nunca qual e. A publica nao e segredo, mas
            // tambem nao ha motivo para espalhar quem usa o que.
            ("exige_chave", Json::Bool(self.chave_publica.is_some())),
            (
                "bases",''')
open(p,'w').write(s)

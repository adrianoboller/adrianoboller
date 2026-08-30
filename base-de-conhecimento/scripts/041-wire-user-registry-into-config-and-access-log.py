# Wire user registry into config and access log
# 27/08 19:04

# --- acesso.rs: registrar QUEM fez ---
p='crates/phxsql-server/src/acesso.rs'
s=open(p).read()
s=s.replace('''    pub op: String,
    /// O token conferiu?
    pub autenticado: bool,''','''    pub op: String,
    /// Login de quem fez, quando houve login. Vazio para anonimo.
    pub usuario: String,
    /// O token conferiu?
    pub autenticado: bool,''')
s=s.replace('''            ("op", Json::texto_de(&self.op)),
            ("autenticado", Json::Bool(self.autenticado)),''','''            ("op", Json::texto_de(&self.op)),
            ("usuario", Json::texto_de(&self.usuario)),
            ("autenticado", Json::Bool(self.autenticado)),''')
s=s.replace('''            op: j.texto_ou("op", "").to_string(),
            autenticado: j.booleano_ou("autenticado", false),''','''            op: j.texto_ou("op", "").to_string(),
            usuario: j.texto_ou("usuario", "").to_string(),
            autenticado: j.booleano_ou("autenticado", false),''')
s=s.replace('''            op: "ping".into(),
            autenticado: ok,''','''            op: "ping".into(),
            usuario: "adriano".into(),
            autenticado: ok,''')
s=s.replace('''        assert_eq!(lidos[0].porta_origem, 54_321);
        assert!(lidos[0].ok);''','''        assert_eq!(lidos[0].porta_origem, 54_321);
        assert_eq!(lidos[0].usuario, "adriano", "o log diz quem fez");
        assert!(lidos[0].ok);''')
open(p,'w').write(s)

# --- config.rs: o cadastro entra na configuracao ---
p='crates/phxsql-server/src/config.rs'
s=open(p).read()
s=s.replace('''use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;''','''use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;

use crate::usuarios::Cadastro;''')
s=s.replace('''    pub somente_leitura: bool,
    pub replicacao: Replicacao,
}''','''    pub somente_leitura: bool,
    pub replicacao: Replicacao,
    /// Usuarios e o poder de cada um sobre cada base.
    pub cadastro: Cadastro,
}''')
s=s.replace('''            somente_leitura: false,
            replicacao: Replicacao::default(),
        }
    }
}''','''            somente_leitura: false,
            replicacao: Replicacao::default(),
            cadastro: Cadastro::default(),
        }
    }
}''')
s=s.replace('''            somente_leitura: j.booleano_ou("somente_leitura", false),
            replicacao: rep,
        })''','''            somente_leitura: j.booleano_ou("somente_leitura", false),
            replicacao: rep,
            cadastro: Cadastro::de_json(j)?,
        })''')
s=s.replace('''            ("papel", Json::texto_de(self.replicacao.papel.nome())),
        ])''','''            ("papel", Json::texto_de(self.replicacao.papel.nome())),
            (
                "usuarios",
                Json::de_u64(
                    (self.cadastro.usuarios.len() + usize::from(self.cadastro.root.is_some()))
                        as u64,
                ),
            ),
        ])''')
open(p,'w').write(s)

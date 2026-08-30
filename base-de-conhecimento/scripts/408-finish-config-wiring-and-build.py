# Finish config wiring and build
# 28/08 14:21

p='crates/phxsql-server/src/config.rs'
s=open(p).read()
a='''            backup: Backup::default(),'''
b='''            backup: Backup::default(),
            alertas: Alertas::default(),'''
assert a in s; s=s.replace(a,b,1)

# para_json do Config: acrescenta base_absoluta e alertas
a='''            (
                "usuarios",
                Json::de_u64(
                    (self.cadastro.usuarios.len() + usize::from(self.cadastro.root.is_some()))
                        as u64,
                ),
            ),
        ])
    }
}'''
b='''            (
                "usuarios",
                Json::de_u64(
                    (self.cadastro.usuarios.len() + usize::from(self.cadastro.root.is_some()))
                        as u64,
                ),
            ),
            // Onde os dados moram DE VERDADE. O campo "base" pode ser
            // relativo, e relativo a que depende de onde o servidor foi
            // iniciado -- que e a duvida que a tela precisa tirar.
            (
                "base_absoluta",
                Json::texto_de(
                    std::fs::canonicalize(&self.base)
                        .unwrap_or_else(|_| self.base.clone())
                        .display()
                        .to_string(),
                ),
            ),
            ("alertas", self.alertas.para_json()),
        ])
    }
}'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')

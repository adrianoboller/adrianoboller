# Add dblink path to config
# 28/08 14:46

p='crates/phxsql-server/src/config.rs'
s=open(p).read()
a='''    /// Aviso de disco apertado, e o e-mail por onde ele sai.
    pub alertas: Alertas,'''
b='''    /// Aviso de disco apertado, e o e-mail por onde ele sai.
    pub alertas: Alertas,
    /// Arquivo com as ligacoes de DbLink.
    ///
    /// Separado do `config.json` de proposito: o cadastro de ligacoes muda
    /// pela tela, e reescrever o `config.json` inteiro a cada ligacao nova
    /// arriscaria os comentarios e o resto da configuracao a cada gravacao.
    pub dblink: PathBuf,'''
assert a in s; s=s.replace(a,b,1)
a='''const CAMPOS_CONHECIDOS: [&str; 18] = ['''
b='''const CAMPOS_CONHECIDOS: [&str; 19] = ['''
assert a in s; s=s.replace(a,b,1)
a='''    "backup",
    "alertas",
];'''
b='''    "backup",
    "alertas",
    "dblink",
];'''
assert a in s; s=s.replace(a,b,1)
a='''            alertas: Alertas::de_json(j)?,'''
b='''            alertas: Alertas::de_json(j)?,
            dblink: PathBuf::from(j.texto_ou("dblink", "dblink.json")),'''
assert a in s; s=s.replace(a,b,1)
a='''            alertas: Alertas::default(),'''
b='''            alertas: Alertas::default(),
            dblink: PathBuf::from("dblink.json"),'''
assert a in s; s=s.replace(a,b,1)
a='''            ("alertas", self.alertas.para_json()),'''
b='''            ("alertas", self.alertas.para_json()),
            (
                "dblink",
                Json::texto_de(self.dblink.display().to_string()),
            ),'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')

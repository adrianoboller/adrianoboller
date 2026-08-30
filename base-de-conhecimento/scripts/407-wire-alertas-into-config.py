# Wire alertas into Config
# 28/08 14:21

p='crates/phxsql-server/src/config.rs'
s=open(p).read()

# 1. campo na struct Config
a='''    /// Backup agendado.
    pub backup: Backup,
'''
b='''    /// Backup agendado.
    pub backup: Backup,
    /// Aviso de disco apertado, e o e-mail por onde ele sai.
    pub alertas: Alertas,
'''
assert a in s; s=s.replace(a,b,1)

# 2. campo conhecido
a='''const CAMPOS_CONHECIDOS: [&str; 17] = ['''
b='''const CAMPOS_CONHECIDOS: [&str; 18] = ['''
assert a in s; s=s.replace(a,b,1)
a='''    "web",
    "backup",
];'''
b='''    "web",
    "backup",
    "alertas",
];'''
assert a in s; s=s.replace(a,b,1)

# 3. de_json
a='''            backup: Backup::de_json(j)?,'''
b='''            backup: Backup::de_json(j)?,
            alertas: Alertas::de_json(j)?,'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')

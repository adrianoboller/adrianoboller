# Fix the clippy warning
# 28/08 15:45

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
a='''    fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
        let mut c = Config::default();
        c.base = dir.to_path_buf();
        c.log_acessos = dir.join("acessos.log");
        c.blacklist = dir.join("blacklist.json");
        c.dblink = dir.join("dblink.json");
        c.token = "t".into();
        Servidor::novo(c).unwrap()
    }'''
b='''    fn servidor(dir: &std::path::Path) -> Arc<Servidor> {
        let c = Config {
            base: dir.to_path_buf(),
            log_acessos: dir.join("acessos.log"),
            blacklist: dir.join("blacklist.json"),
            dblink: dir.join("dblink.json"),
            token: "t".into(),
            ..Config::default()
        };
        Servidor::novo(c).unwrap()
    }'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)

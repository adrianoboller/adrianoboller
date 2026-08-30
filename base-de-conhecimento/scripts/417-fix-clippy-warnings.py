# Fix clippy warnings
# 28/08 14:24

p='crates/phxsql-server/src/sistema.rs'
s=open(p).read()
a='''        assert_eq!(cpu.booleano_ou("primeira_leitura", false), true);
        let segunda = m.ler(&[]);
        assert_eq!(
            segunda
                .campo("cpu")
                .unwrap()
                .booleano_ou("primeira_leitura", true),
            false
        );'''
b='''        assert!(cpu.booleano_ou("primeira_leitura", false));
        let segunda = m.ler(&[]);
        assert!(!segunda
            .campo("cpu")
            .unwrap()
            .booleano_ou("primeira_leitura", true));'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)

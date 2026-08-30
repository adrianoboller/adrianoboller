# Bump the config array sizes and rebuild
# 30/08 06:36

p='crates/phxsql-server/src/config.rs'
s=open(p,encoding='utf-8').read()
for velho,novo in [('const CAMPOS_CONHECIDOS: [&str; 26] =','const CAMPOS_CONHECIDOS: [&str; 27] ='),
                   ('const SECOES_CONHECIDAS: [(&str, &[&str]); 9] =','const SECOES_CONHECIDAS: [(&str, &[&str]); 10] =')]:
    assert s.count(velho)==1, velho
    s=s.replace(velho,novo)
open(p,'w',encoding='utf-8').write(s)
print("26->27 campos e 9->10 secoes: cada frente acrescentou a sua")

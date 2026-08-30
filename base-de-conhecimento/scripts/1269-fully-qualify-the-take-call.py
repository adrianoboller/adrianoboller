# Fully qualify the take call
# 30/08 06:36

p='crates/phxsql-core/src/fio.rs'
s=open(p,encoding='utf-8').read()
velho='            let mut limitado = std::io::Read::by_ref(leitor).take(teto + 1);'
novo='            let mut limitado = <&mut L as std::io::Read>::take(leitor, teto + 1);'
assert s.count(velho)==1
open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print("chamada totalmente qualificada: Self = &mut L, sem auto-deref")

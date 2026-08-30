# Fix the borrow and rebuild
# 30/08 06:35

p='crates/phxsql-core/src/fio.rs'
s=open(p,encoding='utf-8').read()
velho='            let mut limitado = leitor.take(teto + 1);'
novo='            let mut limitado = (&mut *leitor).take(teto + 1);'
assert s.count(velho)==1
open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print("take passa a emprestar em vez de consumir")

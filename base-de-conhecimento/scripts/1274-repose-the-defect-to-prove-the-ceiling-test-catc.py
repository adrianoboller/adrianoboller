# Repose the defect to prove the ceiling test catches it
# 30/08 06:39

p='crates/phxsql-core/src/fio.rs'
s=open(p,encoding='utf-8').read()
velho='''        let lidos = {
            let mut limitado = <&mut L as std::io::Read>::take(leitor, teto + 1);
            limitado.read_line(&mut linha)?
        };'''
novo='''        // DEFEITO REPOSTO: a leitura volta a ser ilimitada.
        let lidos = leitor.read_line(&mut linha)?;'''
assert s.count(velho)==1
open(p,'w',encoding='utf-8').write(s.replace(velho,novo))

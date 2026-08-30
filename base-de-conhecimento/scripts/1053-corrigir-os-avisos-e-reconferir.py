# Corrigir os avisos e reconferir
# 29/08 03:55

import io
p='crates/phxsql-store/src/log.rs'
s=io.open(p,encoding='utf-8').read()
velho = 'let no_comeco_do_volume = comeco.is_none_or(|m| m.volume != volume);'
novo  = 'let no_comeco_do_volume = !matches!(comeco, Some(m) if m.volume == volume);'
assert s.count(velho)==1
io.open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
p='crates/phxsql-store/examples/custo-do-desde.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('t.inserir(&vec![Value::Int(i), Value::Str(format!("Cliente {i:08}"))])',
            't.inserir(&[Value::Int(i), Value::Str(format!("Cliente {i:08}"))])')
io.open(p,'w',encoding='utf-8').write(s)
print('ok')

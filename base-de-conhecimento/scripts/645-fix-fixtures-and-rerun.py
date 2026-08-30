# Fix fixtures and rerun
# 28/08 18:29

import io
p='crates/phxsql-server/src/juncao.rs'
s=io.open(p,encoding='utf-8').read()
# o cenario da juncao precisa das DUAS colunas de sistema
s=s.replace('''            vec![Value::Int(1), txt("Adriano"), Value::Bool(false)],
            vec![Value::Int(2), txt("Maria"), Value::Bool(false)],
            vec![Value::Int(3), txt("João"), Value::Bool(false)],''',
'''            vec![Value::Int(1), txt("Adriano"), Value::Bool(false), Value::UInt(1)],
            vec![Value::Int(2), txt("Maria"), Value::Bool(false), Value::UInt(2)],
            vec![Value::Int(3), txt("João"), Value::Bool(false), Value::UInt(3)],''',1)
s=s.replace('''            vec![Value::Int(1), Value::Int(100), Value::Bool(false)],
            vec![Value::Int(1), Value::Int(200), Value::Bool(false)],
            vec![Value::Int(2), Value::Int(300), Value::Bool(false)],
            vec![Value::Int(9), Value::Int(400), Value::Bool(false)],''',
'''            vec![Value::Int(1), Value::Int(100), Value::Bool(false), Value::UInt(1)],
            vec![Value::Int(1), Value::Int(200), Value::Bool(false), Value::UInt(2)],
            vec![Value::Int(2), Value::Int(300), Value::Bool(false), Value::UInt(3)],
            vec![Value::Int(9), Value::Int(400), Value::Bool(false), Value::UInt(4)],''',1)
s=s.replace('''        // O `false` no fim é a coluna de sistema: `Schema::new` a acrescenta,
        // e a linha tem de bater com o esquema.''',
'''        // O `false` e o número no fim são as colunas de sistema, que
        // `Schema::new` acrescenta: a linha tem de bater com o esquema.''',1)
io.open(p,'w',encoding='utf-8').write(s)

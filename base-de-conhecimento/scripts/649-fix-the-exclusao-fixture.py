# Fix the exclusao fixture
# 28/08 18:30

import io
p='crates/phxsql-store/tests/exclusao.rs'
s=io.open(p,encoding='utf-8').read()
s=s.replace('''    let rowid = t.inserir(&curta).unwrap();
    let linha = t.ler(rowid).unwrap().unwrap();
    assert_eq!(linha.len(), 5);
    assert_eq!(linha[4], Value::Bool(false));
    assert!(!t.esta_excluida(&linha));''','''    let rowid = t.inserir(&curta).unwrap();
    let linha = t.ler(rowid).unwrap().unwrap();
    // As DUAS colunas de sistema entraram sozinhas: a marca falsa e o numero
    // de ordem, que o motor preencheu.
    assert_eq!(linha.len(), 6);
    assert_eq!(linha[4], Value::Bool(false));
    assert_eq!(linha[5], Value::UInt(1));
    assert!(!t.esta_excluida(&linha));''',1)
s=s.replace('''        Value::Bin(vec![id as u8; 300]),
        Value::Memo(format!("ficha de {nome}")),
        Value::Bool(false),
    ]''','''        Value::Bin(vec![id as u8; 300]),
        Value::Memo(format!("ficha de {nome}")),
        Value::Bool(false),
        // Zero = "numere para mim". O motor troca pelo proximo da tabela.
        Value::UInt(0),
    ]''',1)
io.open(p,'w',encoding='utf-8').write(s)

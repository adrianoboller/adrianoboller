# Add Conflito error variant
# 28/08 23:51

import re, pathlib
p = pathlib.Path("crates/phxsql-core/src/error.rs")
s = p.read_text()

s = s.replace(
'''    /// Violacao de indice unico.
    Duplicado(String),''',
'''    /// Violacao de indice unico.
    Duplicado(String),
    /// Outra sessao mexeu no registro entre a leitura e a gravacao.
    Conflito(String),''')

s = s.replace(
'''            PhxError::Duplicado(_) => 3002,
            PhxError::LimiteExcedido(_) => 3003,''',
'''            PhxError::Duplicado(_) => 3002,
            PhxError::LimiteExcedido(_) => 3003,
            PhxError::Conflito(_) => 3004,''')

s = s.replace(
'''            PhxError::Duplicado(_) => "DUPLICADO",
            PhxError::LimiteExcedido(_) => "LIMITE_EXCEDIDO",''',
'''            PhxError::Duplicado(_) => "DUPLICADO",
            PhxError::LimiteExcedido(_) => "LIMITE_EXCEDIDO",
            PhxError::Conflito(_) => "CONFLITO",''')

s = s.replace(
'''            PhxError::Duplicado(m) => write!(f, "chave duplicada: {m}"),''',
'''            PhxError::Duplicado(m) => write!(f, "chave duplicada: {m}"),
            PhxError::Conflito(m) => write!(f, "conflito de escrita: {m}"),''')

s = s.replace(
'''            PhxError::LimiteExcedido(String::new()),
            PhxError::Autorizacao(String::new()),''',
'''            PhxError::LimiteExcedido(String::new()),
            PhxError::Conflito(String::new()),
            PhxError::Autorizacao(String::new()),''')

s = s.replace(
'''        assert_eq!(PhxError::LimiteExcedido(String::new()).codigo(), 3003);''',
'''        assert_eq!(PhxError::LimiteExcedido(String::new()).codigo(), 3003);
        assert_eq!(PhxError::Conflito(String::new()).codigo(), 3004);''')

p.write_text(s)
print("ok")

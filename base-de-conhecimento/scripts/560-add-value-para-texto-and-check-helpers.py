# Add Value::para_texto and check helpers
# 28/08 17:32

import io
p='crates/phxsql-core/src/value.rs'
s=io.open(p,encoding='utf-8').read()
velho='''    pub fn como_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Memo(s) => Some(s.as_str()),
            _ => None,
        }
    }
}'''
novo='''    pub fn como_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Memo(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// O valor em texto, para quando ele vai virar rotulo e nao dado.
    ///
    /// Serve para identificar uma linha num registro que sobrevive a ela --
    /// o `.reason` guarda "id=42" porque seis meses depois o esquema daquela
    /// linha nao esta mais na cabeca de ninguem.
    ///
    /// Nao e serializacao: `Bin` sai como o tamanho em bytes, e `Decimal` sai
    /// sem escala, porque a escala mora no esquema e nao no valor. Quem
    /// precisa do dado de volta le o `.trash`, que guarda os bytes.
    pub fn para_texto(&self) -> String {
        match self {
            Value::Null => String::new(),
            Value::Bool(b) => (if *b { "sim" } else { "nao" }).to_string(),
            Value::Int(v) => v.to_string(),
            Value::UInt(v) => v.to_string(),
            Value::Real(v) => v.to_string(),
            Value::Decimal(v) => v.to_string(),
            Value::Date(d) => crate::datahora::data_iso(*d),
            Value::Time(t) => crate::datahora::hora_iso(*t),
            Value::DateTime(ms) => crate::datahora::instante_iso(*ms),
            Value::Str(s) | Value::Memo(s) => s.clone(),
            Value::Bin(b) => format!("{} bytes", b.len()),
            Value::Uuid(u) => u.to_string(),
            Value::Uuid256(u) => u.to_string(),
        }
    }
}'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)

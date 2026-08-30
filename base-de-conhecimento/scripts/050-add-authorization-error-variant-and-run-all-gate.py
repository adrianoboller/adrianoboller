# Add authorization error variant and run all gates
# 27/08 19:07

p='crates/phxsql-core/src/error.rs'
s=open(p).read()
s=s.replace('''    /// Violacao de indice unico.
    Duplicado(String),''','''    /// Violacao de indice unico.
    Duplicado(String),
    /// Credencial invalida ou poder insuficiente.
    Autorizacao(String),''')
s=s.replace('''            PhxError::Duplicado(m) => write!(f, "chave duplicada: {m}"),''','''            PhxError::Duplicado(m) => write!(f, "chave duplicada: {m}"),
            PhxError::Autorizacao(m) => write!(f, "acesso negado: {m}"),''')
open(p,'w').write(s)

p='crates/phxsql-server/src/servidor.rs'
s=open(p).read()
for antes, depois in [
  ('Err(PhxError::Esquema("token invalido".into()))',
   'Err(PhxError::Autorizacao("token invalido".into()))'),
  ('''                Err(PhxError::Esquema(
                    "faca login antes: {\\"op\\":\\"login\\",\\"usuario\\":...,\\"senha\\":...}".into(),
                )),''',
   '''                Err(PhxError::Autorizacao(
                    "faca login antes: {\\"op\\":\\"login\\",\\"usuario\\":...,\\"senha\\":...}".into(),
                )),'''),
  ('''                    Err(PhxError::Esquema(format!(
                        "{} nao tem permissao de {} em {}",''',
   '''                    Err(PhxError::Autorizacao(format!(
                        "{} nao tem permissao de {} em {}",'''),
  ('Err(PhxError::Esquema("usuario ou senha invalidos".into()))',
   'Err(PhxError::Autorizacao("usuario ou senha invalidos".into()))'),
  ('''                Err(PhxError::Esquema("servidor em modo somente leitura".into())),''',
   '''                Err(PhxError::Autorizacao(
                    "servidor em modo somente leitura".into(),
                )),'''),
]:
    if antes not in s:
        print("NAO CASOU:", antes[:60])
    s=s.replace(antes, depois)
open(p,'w').write(s)

# Rewrite loop
# 28/08 14:23

p='crates/phxsql-server/src/email.rs'
s=open(p).read()
a='''        let ultima;
        loop {
            let mut linha = String::new();
            let lidos = self
                .leitor
                .read_line(&mut linha)
                .map_err(|e| PhxError::Esquema(format!("smtp: leitura falhou: {e}")))?;
            if lidos == 0 {
                return Err(PhxError::Esquema(
                    "smtp: o servidor fechou a conexao no meio da resposta".into(),
                ));
            }
            let limpa = linha.trim_end().to_string();
            let continua = limpa.as_bytes().get(3) == Some(&b'-');
            ultima = limpa;
            if !continua {
                break;
            }
        }
'''
b='''        let ultima = loop {
            let mut linha = String::new();
            let lidos = self
                .leitor
                .read_line(&mut linha)
                .map_err(|e| PhxError::Esquema(format!("smtp: leitura falhou: {e}")))?;
            if lidos == 0 {
                return Err(PhxError::Esquema(
                    "smtp: o servidor fechou a conexao no meio da resposta".into(),
                ));
            }
            let limpa = linha.trim_end().to_string();
            if limpa.as_bytes().get(3) != Some(&b'-') {
                break limpa;
            }
        };
'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)

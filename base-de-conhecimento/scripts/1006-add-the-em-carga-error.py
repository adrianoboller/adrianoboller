# Add the EM_CARGA error
# 29/08 02:52

import pathlib
p = pathlib.Path("crates/phxsql-core/src/error.rs")
s = p.read_text()

s = s.replace('''    /// Outra sessao mexeu no registro entre a leitura e a gravacao.
    Conflito(String),''','''    /// Outra sessao mexeu no registro entre a leitura e a gravacao.
    Conflito(String),
    /// A tabela esta reservada para uma carga, por outra sessao.
    EmCarga(String),''',1)

s = s.replace('''            PhxError::Autorizacao(_) => 4001,''','''            PhxError::Autorizacao(_) => 4001,
            PhxError::EmCarga(_) => 4002,''',1)

s = s.replace('''            PhxError::Autorizacao(_) => "ACESSO_NEGADO",''','''            PhxError::Autorizacao(_) => "ACESSO_NEGADO",
            PhxError::EmCarga(_) => "EM_CARGA",''',1)

s = s.replace('''            PhxError::Autorizacao(m) => write!(f, "acesso negado: {m}"),''','''            PhxError::Autorizacao(m) => write!(f, "acesso negado: {m}"),
            PhxError::EmCarga(m) => write!(f, "tabela em carga: {m}"),''',1)

# adianta_repetir deixa de ser so E/S
alvo = '''    /// Vale a pena tentar de novo?
    ///
    /// So o erro de E/S -- disco cheio que liberou, arquivo que estava
    /// travado. Os outros vao dar o mesmo resultado quantas vezes forem
    /// tentados, e repetir e so gastar o servidor.
    pub fn adianta_repetir(&self) -> bool {
        matches!(self, PhxError::Io(_))
    }'''
novo = '''    /// Vale a pena tentar de novo?
    ///
    /// Dois erros, e os dois pelo mesmo motivo: sao os unicos que descrevem
    /// uma situacao PASSAGEIRA.
    ///
    /// * o de E/S -- disco cheio que liberou, arquivo que estava travado;
    /// * o de tabela em carga -- alguem reservou a tabela e vai soltar.
    ///
    /// Os outros vao dar o mesmo resultado quantas vezes forem tentados, e
    /// repetir e so gastar o servidor. E a distincao que importa para quem
    /// integra: «em carga» e «acesso negado» sao os dois uma recusa, mas a
    /// primeira funciona daqui a pouco e a segunda nao funciona nunca.
    pub fn adianta_repetir(&self) -> bool {
        matches!(self, PhxError::Io(_) | PhxError::EmCarga(_))
    }'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

s = s.replace('''            PhxError::Conflito(String::new()),
            PhxError::Autorizacao(String::new()),''','''            PhxError::Conflito(String::new()),
            PhxError::EmCarga(String::new()),
            PhxError::Autorizacao(String::new()),''',1)

s = s.replace('''        assert_eq!(PhxError::Autorizacao(String::new()).codigo(), 4001);''',
'''        assert_eq!(PhxError::Autorizacao(String::new()).codigo(), 4001);
        assert_eq!(PhxError::EmCarga(String::new()).codigo(), 4002);''',1)

s = s.replace('''        assert!(!PhxError::Conflito(String::new()).adianta_repetir());''',
'''        assert!(!PhxError::Conflito(String::new()).adianta_repetir());
        // «Em carga» e passageiro: quem reservou vai soltar. E a diferenca
        // entre ele e «acesso negado», que nao muda por esperar.
        assert!(PhxError::EmCarga(String::new()).adianta_repetir());
        assert_eq!(PhxError::EmCarga(String::new()).classe(), "acesso");''',1)
p.write_text(s)
print("ok")

# Add stable error codes
# 28/08 16:23

p='crates/phxsql-core/src/error.rs'
s=open(p).read()
a='''impl fmt::Display for PhxError {'''
b='''impl PhxError {
    /// O codigo numerico do erro, estavel para sempre.
    ///
    /// # Por que numero, e nao so texto
    ///
    /// Sem codigo, quem integra com o PhxSql precisa comparar TEXTO para saber
    /// o que aconteceu -- e a hora que alguem melhorar a redacao de uma
    /// mensagem, o cliente quebra sem ninguem perceber. O MySQL(R) mantem mais
    /// de cinco mil codigos justamente por isso: `1062` e chave duplicada
    /// desde sempre, seja qual for a lingua ou a redacao da mensagem.
    ///
    /// A regra que faz o numero valer alguma coisa: **numero nunca muda e
    /// numero aposentado nunca volta**. Trocar o significado de um codigo e
    /// pior do que nao ter codigo, porque o cliente antigo continua tratando
    /// pelo sentido velho.
    ///
    /// As faixas agrupam por familia, para quem quiser tratar em bloco:
    ///
    /// | faixa | familia   | o que e |
    /// |-------|-----------|---------|
    /// | 1000  | `formato` | o arquivo nao e o que dizia ser |
    /// | 2000  | `esquema` | o pedido nao casa com a estrutura |
    /// | 3000  | `dado`    | o dado em si recusa |
    /// | 4000  | `acesso`  | quem pediu nao podia |
    /// | 5000  | `sistema` | o sistema de arquivos falhou |
    pub fn codigo(&self) -> u16 {
        match self {
            PhxError::Corrompido(_) => 1001,
            PhxError::BadMagic { .. } => 1002,
            PhxError::VersaoNaoSuportada { .. } => 1003,
            PhxError::Esquema(_) => 2001,
            PhxError::Tipo(_) => 2002,
            PhxError::NaoEncontrado(_) => 3001,
            PhxError::Duplicado(_) => 3002,
            PhxError::LimiteExcedido(_) => 3003,
            PhxError::Autorizacao(_) => 4001,
            PhxError::Io(_) => 5001,
        }
    }

    /// O nome simbolico, para quem prefere ler a decorar numero.
    ///
    /// Anda junto com o codigo e obedece a mesma regra: nao muda.
    pub fn nome(&self) -> &'static str {
        match self {
            PhxError::Corrompido(_) => "CORROMPIDO",
            PhxError::BadMagic { .. } => "ASSINATURA_INVALIDA",
            PhxError::VersaoNaoSuportada { .. } => "VERSAO_NAO_SUPORTADA",
            PhxError::Esquema(_) => "ESQUEMA_INVALIDO",
            PhxError::Tipo(_) => "TIPO_INVALIDO",
            PhxError::NaoEncontrado(_) => "NAO_ENCONTRADO",
            PhxError::Duplicado(_) => "DUPLICADO",
            PhxError::LimiteExcedido(_) => "LIMITE_EXCEDIDO",
            PhxError::Autorizacao(_) => "ACESSO_NEGADO",
            PhxError::Io(_) => "ERRO_DE_ES",
        }
    }

    /// A familia, derivada da faixa do codigo.
    ///
    /// Derivada e nao escrita a mao de proposito: um codigo novo cai na
    /// familia certa sozinho, e as duas nao tem como divergir.
    pub fn classe(&self) -> &'static str {
        match self.codigo() / 1000 {
            1 => "formato",
            2 => "esquema",
            3 => "dado",
            4 => "acesso",
            _ => "sistema",
        }
    }

    /// Vale a pena tentar de novo?
    ///
    /// So o erro de E/S -- disco cheio que liberou, arquivo que estava
    /// travado. Os outros vao dar o mesmo resultado quantas vezes forem
    /// tentados, e repetir e so gastar o servidor.
    pub fn adianta_repetir(&self) -> bool {
        matches!(self, PhxError::Io(_))
    }
}

impl fmt::Display for PhxError {'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')

//! Tipos de coluna do PhxSql.
//!
//! Todo tipo tem largura FIXA dentro do `.reg`. Os tipos de tamanho ilimitado
//! (`Bin` e `Memo`) guardam no `.reg` apenas um ponteiro de 16 bytes para o
//! `.bin` / `.memo`. E isso que permite calcular a posicao de um registro
//! direto do seu rowid, sem varrer o arquivo.

use crate::error::{PhxError, Result};

/// Largura, em bytes, do ponteiro para `.bin` / `.memo` gravado no `.reg`.
///
/// Layout: offset u64 | tamanho u32 | crc32 do conteudo u32.
pub const PONTEIRO_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnType {
    Bool,
    Int1,
    Int2,
    Int4,
    Int8,
    UInt1,
    UInt2,
    UInt4,
    UInt8,
    Real4,
    Real8,
    /// Decimal exato de ate 38 digitos, guardado como i128 escalado.
    /// Equivale ao DECIMAL do Clarion(R) e ao numerico do HFSQL(R).
    Decimal {
        precisao: u8,
        escala: u8,
    },
    /// Dias desde 1970-01-01 (i32).
    Date,
    /// Centesimos de segundo desde a meia-noite (i32).
    Time,
    /// Milissegundos desde 1970-01-01T00:00:00Z (i64).
    DateTime,
    /// Texto UTF-8 de largura fixa, preenchido com NUL.
    Str(u16),
    /// Binario de tamanho livre, armazenado no `.bin`.
    Bin,
    /// Texto longo UTF-8, armazenado no `.memo`.
    Memo,
    /// UUID de 128 bits (RFC 9562). Guardado em 16 bytes crus, big-endian.
    ///
    /// Dezesseis bytes contra os 36 de um `Str(36)` -- e, mais importante, o
    /// indice compara numeros em vez de texto. Com v7 a chave e crescente, e
    /// insercao com chave crescente cai sempre na folha mais a direita da
    /// B+tree em vez de espalhar a arvore inteira.
    Uuid,
    /// Identificador de 256 bits, em 32 bytes crus.
    ///
    /// Nao e um UUID do RFC 9562, que so define 128 bits -- o nome carrega o
    /// 256 para nao se passar por um. Existe porque um SHA-256 cabe nele
    /// exatamente: hash de bloco, hash de transacao, impressao digital.
    Uuid256,
    /// Contador crescente da tabela, atribuido na insercao quando o valor
    /// chega nulo.
    ///
    /// Diferente do rowid em uma coisa que importa: o rowid e a POSICAO fisica
    /// do registro e nao se escolhe, enquanto a sequencia e um valor de dado --
    /// pode comecar onde se quiser, pode ser gravada a mao e continua de onde
    /// parou. Nunca reaproveita numero, nem depois de exclusao.
    ///
    /// Uma por tabela. Duas sequencias na mesma tabela compartilhariam o mesmo
    /// contador, o que so pareceria um defeito.
    Sequence,
}

impl ColumnType {
    /// Numero de bytes que o tipo ocupa dentro do slot do `.reg`.
    pub fn largura(&self) -> usize {
        match self {
            ColumnType::Bool | ColumnType::Int1 | ColumnType::UInt1 => 1,
            ColumnType::Int2 | ColumnType::UInt2 => 2,
            ColumnType::Int4
            | ColumnType::UInt4
            | ColumnType::Real4
            | ColumnType::Date
            | ColumnType::Time => 4,
            ColumnType::Int8 | ColumnType::UInt8 | ColumnType::Real8 | ColumnType::DateTime => 8,
            ColumnType::Decimal { .. } => 16,
            ColumnType::Str(n) => *n as usize,
            ColumnType::Bin | ColumnType::Memo => PONTEIRO_LEN,
            ColumnType::Uuid => 16,
            ColumnType::Uuid256 => 32,
            ColumnType::Sequence => 8,
        }
    }

    /// Um tipo e indexavel quando pode ser transformado em chave de ordenacao
    /// de largura fixa. `Bin` e `Memo` moram fora do `.reg`, entao nao sao.
    pub fn indexavel(&self) -> bool {
        !matches!(self, ColumnType::Bin | ColumnType::Memo)
    }

    /// Largura do componente deste tipo dentro de uma chave do `.ndx`,
    /// sem contar o byte de presenca (NULL).
    pub fn largura_chave(&self) -> Result<usize> {
        if !self.indexavel() {
            return Err(PhxError::Esquema(format!(
                "tipo {self:?} nao pode compor um indice"
            )));
        }
        Ok(self.largura())
    }

    /// Tipos que guardam o conteudo fora do `.reg`.
    pub fn externo(&self) -> bool {
        matches!(self, ColumnType::Bin | ColumnType::Memo)
    }

    pub(crate) fn tag(&self) -> u8 {
        match self {
            ColumnType::Bool => 1,
            ColumnType::Int1 => 2,
            ColumnType::Int2 => 3,
            ColumnType::Int4 => 4,
            ColumnType::Int8 => 5,
            ColumnType::UInt1 => 6,
            ColumnType::UInt2 => 7,
            ColumnType::UInt4 => 8,
            ColumnType::UInt8 => 9,
            ColumnType::Real4 => 10,
            ColumnType::Real8 => 11,
            ColumnType::Decimal { .. } => 12,
            ColumnType::Date => 13,
            ColumnType::Time => 14,
            ColumnType::DateTime => 15,
            ColumnType::Str(_) => 16,
            ColumnType::Bin => 17,
            ColumnType::Memo => 18,
            ColumnType::Uuid => 19,
            ColumnType::Uuid256 => 20,
            ColumnType::Sequence => 21,
        }
    }

    pub(crate) fn de_tag(tag: u8, param_a: u16, param_b: u8) -> Result<Self> {
        Ok(match tag {
            1 => ColumnType::Bool,
            2 => ColumnType::Int1,
            3 => ColumnType::Int2,
            4 => ColumnType::Int4,
            5 => ColumnType::Int8,
            6 => ColumnType::UInt1,
            7 => ColumnType::UInt2,
            8 => ColumnType::UInt4,
            9 => ColumnType::UInt8,
            10 => ColumnType::Real4,
            11 => ColumnType::Real8,
            12 => ColumnType::Decimal {
                precisao: param_a as u8,
                escala: param_b,
            },
            13 => ColumnType::Date,
            14 => ColumnType::Time,
            15 => ColumnType::DateTime,
            16 => ColumnType::Str(param_a),
            17 => ColumnType::Bin,
            18 => ColumnType::Memo,
            19 => ColumnType::Uuid,
            20 => ColumnType::Uuid256,
            21 => ColumnType::Sequence,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "tag de tipo desconhecida: {outro}"
                )))
            }
        })
    }

    pub(crate) fn params(&self) -> (u16, u8) {
        match self {
            ColumnType::Decimal { precisao, escala } => (*precisao as u16, *escala),
            ColumnType::Str(n) => (*n, 0),
            _ => (0, 0),
        }
    }
}

/// A coluna guarda dado pessoal? E de que grau?
///
/// # Por que tres graus, e nao um "sim/nao"
///
/// A LGPD separa o dado pessoal (art. 5º, I) do dado pessoal **sensivel**
/// (art. 5º, II -- origem racial, conviccao religiosa, opiniao politica,
/// saude, vida sexual, genetico, biometrico), e o GDPR faz a mesma separacao
/// entre o art. 4(1) e a "special category" do art. 9. Os dois regimes NAO
/// sao o mesmo: o sensivel exige base legal propria e costuma exigir
/// consentimento especifico.
///
/// Um `bool` obrigaria quem audita a reabrir a lei coluna a coluna para
/// separar de novo. Como custa o mesmo byte, o grau vai gravado.
///
/// Os tres sao ORDENADOS -- `Nao` < `Pessoal` < `Sensivel` --, o que deixa a
/// pergunta "esta coluna e pelo menos pessoal?" virar uma comparacao em vez de
/// uma lista de casos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum DadoPessoal {
    /// Nao e dado pessoal. E o padrao, e e o que toda tabela ja gravada tem.
    #[default]
    Nao,
    /// Dado pessoal comum: nome, endereco, telefone, CPF, e-mail.
    Pessoal,
    /// Dado pessoal sensivel: saude, biometria, conviccao, origem racial.
    Sensivel,
}

impl DadoPessoal {
    /// O byte gravado no esquema. **Nunca muda**, pela mesma razao dos codigos
    /// de erro: numero que troca de significado quebra o leitor antigo em
    /// silencio.
    pub fn tag(self) -> u8 {
        match self {
            DadoPessoal::Nao => 0,
            DadoPessoal::Pessoal => 1,
            DadoPessoal::Sensivel => 2,
        }
    }

    /// Byte desconhecido vira `Nao`, e nao erro.
    ///
    /// # Por que nao erro
    ///
    /// Se um dia entrar um grau 3, uma tabela gravada com ele abriria num
    /// motor antigo. Recusar o arquivo inteiro por causa de uma classificacao
    /// que o motor nao conhece tiraria do ar uma tabela que esta perfeita --
    /// e o dado que ela guarda continua legivel de qualquer jeito. O grau
    /// desconhecido some do relatorio; a tabela abre.
    pub fn de_tag(tag: u8) -> DadoPessoal {
        match tag {
            1 => DadoPessoal::Pessoal,
            2 => DadoPessoal::Sensivel,
            _ => DadoPessoal::Nao,
        }
    }

    /// O nome curto, estavel, que sai no protocolo.
    pub fn nome(self) -> &'static str {
        match self {
            DadoPessoal::Nao => "nao",
            DadoPessoal::Pessoal => "pessoal",
            DadoPessoal::Sensivel => "sensivel",
        }
    }

    /// Do texto que o cliente manda. Vazio e `"nao"` sao a mesma coisa.
    pub fn de_texto(s: &str) -> Result<DadoPessoal> {
        Ok(match s.trim().to_lowercase().as_str() {
            "" | "nao" | "não" | "no" | "false" | "0" => DadoPessoal::Nao,
            "pessoal" | "sim" | "yes" | "true" | "1" | "personal" => DadoPessoal::Pessoal,
            "sensivel" | "sensível" | "sensitive" | "2" => DadoPessoal::Sensivel,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "dado_pessoal {outro:?} desconhecido \
                     (use \"nao\", \"pessoal\" ou \"sensivel\")"
                )))
            }
        })
    }

    /// E dado pessoal de algum grau?
    pub fn e_pessoal(self) -> bool {
        self > DadoPessoal::Nao
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O byte gravado e o contrato com toda tabela ja em disco.
    #[test]
    fn as_tags_do_dado_pessoal_nao_mudam() {
        assert_eq!(DadoPessoal::Nao.tag(), 0);
        assert_eq!(DadoPessoal::Pessoal.tag(), 1);
        assert_eq!(DadoPessoal::Sensivel.tag(), 2);
        for d in [
            DadoPessoal::Nao,
            DadoPessoal::Pessoal,
            DadoPessoal::Sensivel,
        ] {
            assert_eq!(DadoPessoal::de_tag(d.tag()), d);
        }
    }

    /// Grau que este motor nao conhece nao derruba a tabela.
    #[test]
    fn grau_desconhecido_vira_nao() {
        assert_eq!(DadoPessoal::de_tag(3), DadoPessoal::Nao);
        assert_eq!(DadoPessoal::de_tag(255), DadoPessoal::Nao);
    }

    #[test]
    fn o_padrao_e_nao_marcado() {
        assert_eq!(DadoPessoal::default(), DadoPessoal::Nao);
        assert!(!DadoPessoal::Nao.e_pessoal());
        assert!(DadoPessoal::Pessoal.e_pessoal());
        assert!(DadoPessoal::Sensivel.e_pessoal());
        assert!(DadoPessoal::Sensivel > DadoPessoal::Pessoal);
    }

    #[test]
    fn texto_do_cliente_vira_grau() {
        assert_eq!(DadoPessoal::de_texto("").unwrap(), DadoPessoal::Nao);
        assert_eq!(
            DadoPessoal::de_texto(" Sensivel ").unwrap(),
            DadoPessoal::Sensivel
        );
        assert_eq!(
            DadoPessoal::de_texto("PESSOAL").unwrap(),
            DadoPessoal::Pessoal
        );
        assert!(DadoPessoal::de_texto("talvez").is_err());
    }

    #[test]
    fn larguras() {
        assert_eq!(ColumnType::Bool.largura(), 1);
        assert_eq!(ColumnType::Int8.largura(), 8);
        assert_eq!(ColumnType::Str(30).largura(), 30);
        assert_eq!(ColumnType::Memo.largura(), PONTEIRO_LEN);
        assert_eq!(
            ColumnType::Decimal {
                precisao: 15,
                escala: 2
            }
            .largura(),
            16
        );
    }

    #[test]
    fn memo_e_bin_nao_indexam() {
        assert!(!ColumnType::Memo.indexavel());
        assert!(!ColumnType::Bin.indexavel());
        assert!(ColumnType::Str(10).indexavel());
        assert!(ColumnType::Memo.largura_chave().is_err());
    }

    #[test]
    fn roundtrip_de_tag() {
        for t in [
            ColumnType::Bool,
            ColumnType::Int4,
            ColumnType::Real8,
            ColumnType::Date,
            ColumnType::Str(42),
            ColumnType::Decimal {
                precisao: 18,
                escala: 4,
            },
            ColumnType::Memo,
        ] {
            let (a, b) = t.params();
            assert_eq!(ColumnType::de_tag(t.tag(), a, b).unwrap(), t);
        }
    }
}

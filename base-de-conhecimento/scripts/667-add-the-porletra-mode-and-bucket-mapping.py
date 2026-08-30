# Add the PorLetra mode and bucket mapping
# 28/08 18:44

import io
p='crates/phxsql-core/src/paginacao.rs'
s=io.open(p,encoding='utf-8').read()

velho='''#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModoParticao {
    /// Volume novo a cada `registros_por_arquivo`. O endereco sai de divisao.
    #[default]
    PorQuantidade,
    /// Volume novo quando o periodo da coluna de data vira -- ou quando o
    /// volume enche, o que vier primeiro. O teto continua valendo: um mes de
    /// movimento intenso nao pode estourar o arquivo.
    PorPeriodo { coluna: u16, periodo: Periodo },
}'''

novo='''#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModoParticao {
    /// Volume novo a cada `registros_por_arquivo`. O endereco sai de divisao.
    #[default]
    PorQuantidade,
    /// Volume novo quando o periodo da coluna de data vira -- ou quando o
    /// volume enche, o que vier primeiro. O teto continua valendo: um mes de
    /// movimento intenso nao pode estourar o arquivo.
    PorPeriodo { coluna: u16, periodo: Periodo },
    /// Um volume FIXO por letra inicial de uma coluna de referencia:
    /// `Clientes_A.reg`, `Clientes_B.reg`, ... `Clientes_Outros.reg`.
    ///
    /// # O que muda, e o que nao muda
    ///
    /// Aqui a linha nao vai para o volume corrente: vai para o volume DELA. E
    /// duas linhas digitadas em seguida caem em arquivos diferentes.
    ///
    /// O endereco continua saindo de uma conta, e essa e a razao de o desenho
    /// funcionar. O rowid e atribuido como
    ///
    /// ```text
    /// rowid = (balde - 1) x registros_por_arquivo + slot_no_balde
    /// ```
    ///
    /// que e exatamente a inversa da conta que [`Paginacao::localizar`] ja
    /// fazia. Ler nao muda em nada, o `.ndx` nao muda em nada, e o espelho
    /// tambem nao.
    ///
    /// # A ordem de digitacao nao se perde -- ela muda de campo
    ///
    /// O que se perde e o rowid ser crescente na ordem de chegada: com os
    /// baldes, o rowid de uma linha diz em que ARQUIVO ela esta, e nao quando
    /// ela chegou. Dentro de cada volume a ordem continua sendo a de digitacao,
    /// e slot excluido continua sem ser reaproveitado.
    ///
    /// A ordem global fica na coluna de sistema `rownum`, que existe em toda
    /// tabela e cresce por tabela e nao por volume. Sem ela este modo seria
    /// uma quebra da regra da casa; com ela, e uma troca de campo.
    PorLetra { coluna: u16 },
}

/// Os baldes da particao alfanumerica, na ordem em que viram volume.
///
/// A ordem e o formato: o balde 1 e o `_A`, o 27 e o `_0`, o 37 e o
/// `_Outros`. Mudar esta lista mudaria o endereco de toda linha ja gravada.
pub const BALDES: [&str; 37] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "Outros",
];

/// O balde de "nao e letra nem algarismo". O ultimo, e sempre presente.
pub const BALDE_OUTROS: u32 = 37;

/// Em que balde este texto cai. Devolve 1..=37.
///
/// # As tres decisoes
///
/// **Acento cai na letra sem acento.** «Ávila» vai para o `_A`. Um balde
/// `_Á` separado dividiria o cadastro em dois lugares que ninguem procura
/// juntos, e faria «Avila» e «Ávila» -- que sao a mesma pessoa digitada por
/// duas pessoas -- pararem em arquivos diferentes.
///
/// **Vazio vai para `Outros`,** e nao para `A`. Nome em branco nao comeca com
/// A; junta-lo com os Andrades esconderia o problema no meio do maior balde.
///
/// **Minuscula e maiuscula sao o mesmo balde.** O contrario faria a mesma
/// consulta achar ou nao achar conforme como foi digitada.
pub fn balde_de(texto: &str) -> u32 {
    let Some(c) = texto.trim().chars().next() else {
        return BALDE_OUTROS;
    };
    let c = sem_acento(c).to_ascii_uppercase();
    if c.is_ascii_uppercase() {
        return (c as u32) - ('A' as u32) + 1;
    }
    if c.is_ascii_digit() {
        return (c as u32) - ('0' as u32) + 27;
    }
    BALDE_OUTROS
}

/// A letra sem acento, para o Latin-1 que o portugues usa.
///
/// Tabela a mao, e nao normalizacao Unicode: normalizar de verdade exigiria a
/// tabela do Unicode inteira, que e uma dependencia. Isto cobre o portugues,
/// o espanhol e o alemao -- e o que nao cobrir cai em `Outros`, que e um lugar
/// visivel e nao um erro escondido.
fn sem_acento(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ç' | 'Ç' => 'C',
        'ñ' | 'Ñ' => 'N',
        'ý' | 'ÿ' | 'Ý' => 'Y',
        outro => outro,
    }
}'''
assert velho in s
s=s.replace(velho,novo,1)
io.open(p,'w',encoding='utf-8').write(s)

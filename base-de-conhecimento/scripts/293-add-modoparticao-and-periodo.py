# Add ModoParticao and Periodo
# 28/08 11:16

import pathlib
p = pathlib.Path('crates/phxsql-core/src/paginacao.rs')
s = p.read_text()

TIPOS = '''
/// Em que ritmo um volume novo comeca, quando a particao e por periodo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Periodo {
    Mensal,
    Bimestral,
    Semestral,
    Anual,
}

impl Periodo {
    pub fn meses(self) -> u32 {
        match self {
            Periodo::Mensal => 1,
            Periodo::Bimestral => 2,
            Periodo::Semestral => 6,
            Periodo::Anual => 12,
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Periodo::Mensal => "mensal",
            Periodo::Bimestral => "bimestral",
            Periodo::Semestral => "semestral",
            Periodo::Anual => "anual",
        }
    }

    pub fn de_nome(n: &str) -> Result<Periodo> {
        Ok(match n.trim().to_ascii_lowercase().as_str() {
            "mensal" => Periodo::Mensal,
            "bimestral" => Periodo::Bimestral,
            "semestral" => Periodo::Semestral,
            "anual" => Periodo::Anual,
            outro => {
                return Err(PhxError::Esquema(format!(
                    "periodo desconhecido: {outro:?} \\
                     (use mensal, bimestral, semestral ou anual)"
                )))
            }
        })
    }

    fn tag(self) -> u8 {
        match self {
            Periodo::Mensal => 1,
            Periodo::Bimestral => 2,
            Periodo::Semestral => 3,
            Periodo::Anual => 4,
        }
    }

    fn de_tag(t: u8) -> Result<Periodo> {
        Ok(match t {
            1 => Periodo::Mensal,
            2 => Periodo::Bimestral,
            3 => Periodo::Semestral,
            4 => Periodo::Anual,
            outro => return Err(PhxError::Esquema(format!("periodo invalido: {outro}"))),
        })
    }

    /// A chave do periodo a que um mes pertence.
    ///
    /// E o numero do periodo desde o ano zero: `ano * (12/meses) + bloco`. Dois
    /// registros caem no mesmo volume exatamente quando esta chave e igual, e
    /// a chave e crescente no tempo -- entao comparar chaves compara periodos.
    ///
    /// Os blocos comecam sempre em janeiro: bimestre e jan-fev, mar-abr, …;
    /// semestre e jan-jun e jul-dez. Nao ha bimestre a comecar em fevereiro.
    pub fn chave(self, ano: i32, mes: u32) -> i64 {
        let mes = mes.clamp(1, 12) - 1;
        let blocos_por_ano = (12 / self.meses()) as i64;
        ano as i64 * blocos_por_ano + (mes / self.meses()) as i64
    }

    /// O primeiro mes do periodo de uma chave: `(ano, mes)`.
    pub fn primeiro_mes(self, chave: i64) -> (i32, u32) {
        let blocos_por_ano = (12 / self.meses()) as i64;
        let ano = chave.div_euclid(blocos_por_ano);
        let bloco = chave.rem_euclid(blocos_por_ano);
        (ano as i32, bloco as u32 * self.meses() + 1)
    }

    /// Como o periodo de uma chave se escreve numa tela: `2026-03`, `2026-S1`.
    pub fn rotulo(self, chave: i64) -> String {
        let (ano, mes) = self.primeiro_mes(chave);
        match self {
            Periodo::Mensal => format!("{ano:04}-{mes:02}"),
            Periodo::Bimestral => format!("{ano:04}-B{}", (mes - 1) / 2 + 1),
            Periodo::Semestral => format!("{ano:04}-S{}", (mes - 1) / 6 + 1),
            Periodo::Anual => format!("{ano:04}"),
        }
    }
}

/// O que decide abrir um volume novo.
///
/// # Por que o periodo NAO reordena as linhas
///
/// A ordem de digitacao e sagrada: o volume N+1 vem sempre depois do N, e o
/// rowid e crescente no arquivo inteiro. Um lancamento de janeiro digitado em
/// marco vai para o volume corrente -- o de marco --, e nao volta para o de
/// janeiro. Fazer o contrario significaria inserir no meio de um arquivo ja
/// fechado, quebrando ao mesmo tempo a ordem de digitacao e o endereco por
/// conta.
///
/// Entao `PorPeriodo` quer dizer: **o volume corta quando o periodo vira**.
/// Cada volume guarda no cabecalho o periodo em que foi aberto e o primeiro
/// rowid que recebeu, e por isso as faixas continuam contiguas e crescentes --
/// achar o volume de um rowid e uma busca binaria numa tabela do tamanho do
/// numero de volumes, em vez de uma divisao.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModoParticao {
    /// Volume novo a cada `registros_por_arquivo`. O endereco sai de divisao.
    #[default]
    PorQuantidade,
    /// Volume novo quando o periodo da coluna de data vira -- ou quando o
    /// volume enche, o que vier primeiro. O teto continua valendo: um mes de
    /// movimento intenso nao pode estourar o arquivo.
    PorPeriodo { coluna: u16, periodo: Periodo },
}

impl ModoParticao {
    pub fn periodo(&self) -> Option<Periodo> {
        match self {
            ModoParticao::PorQuantidade => None,
            ModoParticao::PorPeriodo { periodo, .. } => Some(*periodo),
        }
    }

    pub fn coluna(&self) -> Option<usize> {
        match self {
            ModoParticao::PorQuantidade => None,
            ModoParticao::PorPeriodo { coluna, .. } => Some(*coluna as usize),
        }
    }

    /// Dois bytes na serializacao: tag do periodo (0 = por quantidade) e, se
    /// houver, a coluna.
    pub fn tag(&self) -> (u8, u16) {
        match self {
            ModoParticao::PorQuantidade => (0, 0),
            ModoParticao::PorPeriodo { coluna, periodo } => (periodo.tag(), *coluna),
        }
    }

    pub fn de_tag(tag: u8, coluna: u16) -> Result<ModoParticao> {
        Ok(match tag {
            0 => ModoParticao::PorQuantidade,
            outro => ModoParticao::PorPeriodo {
                coluna,
                periodo: Periodo::de_tag(outro)?,
            },
        })
    }
}
'''

marca = '#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct Paginacao {'
assert s.count(marca) == 1
s = s.replace(marca, TIPOS.strip() + '\n\n' + marca, 1)

# o campo no struct
v = '''    /// Bytes por volume nos arquivos externos (`.bin`, `.memo`, `.log`).
    pub bytes_por_arquivo: u64,
}'''
n = '''    /// Bytes por volume nos arquivos externos (`.bin`, `.memo`, `.log`).
    pub bytes_por_arquivo: u64,
    /// O que faz o volume cortar: a contagem ou o calendario.
    pub modo: ModoParticao,
}'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''    pub const DESLIGADA: Paginacao = Paginacao {
        registros_por_arquivo: 0,
        max_arquivos: 0,
        digitos: 0,
        bytes_por_arquivo: 0,
    };'''
n = '''    pub const DESLIGADA: Paginacao = Paginacao {
        registros_por_arquivo: 0,
        max_arquivos: 0,
        digitos: 0,
        bytes_por_arquivo: 0,
        modo: ModoParticao::PorQuantidade,
    };'''
assert s.count(v) == 1
s = s.replace(v, n)

v = '''        Paginacao {
            registros_por_arquivo,
            max_arquivos,
            digitos: DIGITOS_PADRAO,
            bytes_por_arquivo: BYTES_POR_ARQUIVO_PADRAO,
        }
        .validada()'''
n = '''        Paginacao {
            registros_por_arquivo,
            max_arquivos,
            digitos: DIGITOS_PADRAO,
            bytes_por_arquivo: BYTES_POR_ARQUIVO_PADRAO,
            modo: ModoParticao::PorQuantidade,
        }
        .validada()'''
assert s.count(v) == 1
s = s.replace(v, n)
p.write_text(s)
print('ok')

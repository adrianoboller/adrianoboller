//! Paginacao de tabelas grandes.
//!
//! Uma tabela grande se parte em volumes numerados:
//!
//! ```text
//! cadastroClientes_001.reg
//! cadastroClientes_002.reg
//! cadastroClientes_003.reg
//! ```
//!
//! A quantidade de registros por arquivo e a quantidade de arquivos sao
//! definidas na criacao da tabela e ficam gravadas no esquema.
//!
//! # A propriedade que faz isso sair barato
//!
//! O endereco de um registro continua saindo de uma CONTA, nao de uma busca:
//!
//! ```text
//! volume = (rowid - 1) / registros_por_arquivo + 1
//! slot   = (rowid - 1) % registros_por_arquivo + 1
//! ```
//!
//! Com isso:
//!
//! * o `rowid` continua GLOBAL e imutavel -- ele nao e "posicao no volume",
//!   e posicao na tabela, e o volume sai dele por divisao;
//! * a ordem de digitacao continua garantida, porque o volume N+1 vem sempre
//!   depois do N;
//! * o `.ndx` NAO MUDA EM NADA. Ele ja guarda rowid, e nenhuma linha do codigo
//!   de indice precisa saber que volume existe.

use crate::error::{PhxError, Result};
use crate::RowId;

/// Bytes por volume adotados por padrao nos arquivos externos (1 GiB).
pub const BYTES_POR_ARQUIVO_PADRAO: u64 = 1024 * 1024 * 1024;

/// Largura padrao do sufixo numerico (`_001`).
pub const DIGITOS_PADRAO: u8 = 3;

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
                    "periodo desconhecido: {outro:?} \
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
}

/// Tag da particao alfanumerica na serializacao do esquema.
///
/// Escolhida bem longe das do periodo (1..=4) de proposito: um byte trocado
/// entre elas trocaria o modo da tabela, e o modo decide o ENDERECO de cada
/// linha. Longe, o byte torto cai em "tag desconhecida" e o esquema e recusado.
const TAG_POR_LETRA: u8 = 200;

impl ModoParticao {
    pub fn periodo(&self) -> Option<Periodo> {
        match self {
            ModoParticao::PorPeriodo { periodo, .. } => Some(*periodo),
            _ => None,
        }
    }

    pub fn coluna(&self) -> Option<usize> {
        match self {
            ModoParticao::PorQuantidade => None,
            ModoParticao::PorPeriodo { coluna, .. } | ModoParticao::PorLetra { coluna } => {
                Some(*coluna as usize)
            }
        }
    }

    /// A particao e alfanumerica?
    pub fn por_letra(&self) -> bool {
        matches!(self, ModoParticao::PorLetra { .. })
    }

    /// O nome deste modo, como a tela e o `.pag` escrevem.
    pub fn nome(&self) -> &'static str {
        match self {
            ModoParticao::PorQuantidade => "quantidade",
            ModoParticao::PorPeriodo { .. } => "periodo",
            ModoParticao::PorLetra { .. } => "letra",
        }
    }

    /// Dois bytes na serializacao: tag do modo (0 = por quantidade) e, se
    /// houver, a coluna.
    pub fn tag(&self) -> (u8, u16) {
        match self {
            ModoParticao::PorQuantidade => (0, 0),
            ModoParticao::PorPeriodo { coluna, periodo } => (periodo.tag(), *coluna),
            ModoParticao::PorLetra { coluna } => (TAG_POR_LETRA, *coluna),
        }
    }

    pub fn de_tag(tag: u8, coluna: u16) -> Result<ModoParticao> {
        Ok(match tag {
            0 => ModoParticao::PorQuantidade,
            TAG_POR_LETRA => ModoParticao::PorLetra { coluna },
            outro => ModoParticao::PorPeriodo {
                coluna,
                periodo: Periodo::de_tag(outro)?,
            },
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paginacao {
    /// Registros em cada volume do `.reg`. Zero = sem paginacao.
    pub registros_por_arquivo: u64,
    /// Teto de volumes da tabela.
    pub max_arquivos: u32,
    /// Largura do sufixo numerico. Zero = arquivo unico, sem sufixo.
    pub digitos: u8,
    /// Bytes por volume nos arquivos externos (`.bin`, `.memo`, `.log`).
    pub bytes_por_arquivo: u64,
    /// O que faz o volume cortar: a contagem ou o calendario.
    pub modo: ModoParticao,
}

impl Paginacao {
    /// Tabela em arquivo unico: `cadastroClientes.reg`, sem sufixo.
    pub const DESLIGADA: Paginacao = Paginacao {
        registros_por_arquivo: 0,
        max_arquivos: 0,
        digitos: 0,
        bytes_por_arquivo: 0,
        modo: ModoParticao::PorQuantidade,
    };

    /// Paginacao com os dois numeros que o `CREATE TABLE` define.
    pub fn nova(registros_por_arquivo: u64, max_arquivos: u32) -> Result<Paginacao> {
        Paginacao {
            registros_por_arquivo,
            max_arquivos,
            digitos: DIGITOS_PADRAO,
            bytes_por_arquivo: BYTES_POR_ARQUIVO_PADRAO,
            modo: ModoParticao::PorQuantidade,
        }
        .validada()
    }

    /// Particao ALFANUMERICA: 37 volumes fixos, um por letra inicial.
    ///
    /// `registros_por_arquivo` passa a ser o teto POR LETRA, e nao da tabela.
    /// Dimensionar isto e a decisao que a tabela pede: num cadastro brasileiro
    /// o `_S` costuma ter dez vezes o `_K`, e quem enche primeiro derruba a
    /// insercao daquela letra -- com as outras 36 ainda vazias.
    pub fn por_letra(registros_por_arquivo: u64, coluna: u16) -> Result<Paginacao> {
        Paginacao {
            registros_por_arquivo,
            max_arquivos: BALDES.len() as u32,
            // Dois digitos porque 37 cabe neles. O sufixo desta particao e a
            // LETRA, e nao o numero -- mas `digitos` continua sendo o que
            // `ligada()` olha, e zero desligaria a paginacao inteira.
            digitos: 2,
            bytes_por_arquivo: BYTES_POR_ARQUIVO_PADRAO,
            modo: ModoParticao::PorLetra { coluna },
        }
        .validada()
    }

    /// A mesma paginacao, mas para os arquivos que NAO sao particionados.
    ///
    /// So o `.reg` -- e o espelho `.bkp`, que e um clone dele -- se parte por
    /// letra. O `.bin`, o `.memo`, o `.log`, o `.trash` e o `.reason` rolam por
    /// TAMANHO, e continuam rolando: um `.log` que passa do volume 1 viraria
    /// `Clientes_B.log`, que se le como «o diario do balde B» e nao e -- o
    /// diario e da tabela inteira.
    ///
    /// Entao eles voltam ao sufixo numerico, com tres digitos.
    pub fn para_externos(mut self) -> Paginacao {
        if self.modo.por_letra() {
            self.modo = ModoParticao::PorQuantidade;
            self.digitos = DIGITOS_PADRAO;
            self.max_arquivos = 10u32.pow(DIGITOS_PADRAO as u32) - 1;
        }
        self
    }

    /// Muda a largura do sufixo (por exemplo 4, para passar de 999 volumes).
    pub fn com_digitos(mut self, digitos: u8) -> Result<Paginacao> {
        self.digitos = digitos;
        self.validada()
    }

    /// Muda o teto de volumes, ja com a largura do sufixo que vigora agora.
    ///
    /// Existe por causa da ordem: `nova` confere o teto contra os tres digitos
    /// do padrao, entao pedir 9999 volumes ali e recusado antes de o quarto
    /// digito existir. Com este metodo a largura entra primeiro e o teto
    /// depois, que e a ordem em que os dois fazem sentido.
    pub fn com_max_arquivos(mut self, max_arquivos: u32) -> Result<Paginacao> {
        self.max_arquivos = max_arquivos;
        self.validada()
    }

    /// Troca a regra de corte do volume para o calendario.
    ///
    /// O `registros_por_arquivo` continua valendo como **teto**: um periodo de
    /// movimento intenso corta o volume ao encher, mesmo antes de o periodo
    /// virar. Sem isso um unico mes poderia estourar o arquivo, e a paginacao
    /// existe justamente para que isso nao aconteca.
    pub fn com_modo(mut self, modo: ModoParticao) -> Result<Paginacao> {
        self.modo = modo;
        self.validada()
    }

    /// Muda o tamanho de cada volume dos arquivos externos.
    pub fn com_bytes_por_arquivo(mut self, bytes: u64) -> Result<Paginacao> {
        self.bytes_por_arquivo = bytes;
        self.validada()
    }

    fn validada(self) -> Result<Paginacao> {
        if self.registros_por_arquivo == 0 {
            return Err(PhxError::Esquema(
                "registros_por_arquivo precisa ser maior que zero".into(),
            ));
        }
        if self.max_arquivos == 0 {
            return Err(PhxError::Esquema(
                "max_arquivos precisa ser maior que zero".into(),
            ));
        }
        if self.digitos == 0 || self.digitos > 9 {
            return Err(PhxError::Esquema(format!(
                "digitos precisa estar entre 1 e 9, recebido {}",
                self.digitos
            )));
        }
        let teto = 10u64.pow(self.digitos as u32) - 1;
        if self.max_arquivos as u64 > teto {
            return Err(PhxError::Esquema(format!(
                "max_arquivos {} nao cabe em {} digitos (maximo {teto})",
                self.max_arquivos, self.digitos
            )));
        }
        if self.bytes_por_arquivo == 0 {
            return Err(PhxError::Esquema(
                "bytes_por_arquivo precisa ser maior que zero".into(),
            ));
        }
        Ok(self)
    }

    pub fn ligada(&self) -> bool {
        self.digitos > 0 && self.registros_por_arquivo > 0
    }

    /// Volume e slot de um rowid. Sem paginacao, o volume e sempre 1 e o slot
    /// e o proprio rowid.
    pub fn localizar(&self, rowid: RowId) -> (u32, u64) {
        if !self.ligada() {
            return (1, rowid);
        }
        let base = rowid - 1;
        (
            (base / self.registros_por_arquivo) as u32 + 1,
            base % self.registros_por_arquivo + 1,
        )
    }

    /// Quantos registros a tabela comporta.
    pub fn capacidade(&self) -> u64 {
        if !self.ligada() {
            return u64::MAX;
        }
        self.registros_por_arquivo
            .saturating_mul(self.max_arquivos as u64)
    }

    /// Um rowid ainda cabe na tabela?
    pub fn cabe(&self, rowid: RowId) -> bool {
        rowid >= 1 && rowid <= self.capacidade()
    }

    /// Sufixo do nome do arquivo: `_001` com paginacao, `_A` na alfanumerica,
    /// vazio sem paginacao.
    pub fn sufixo(&self, volume: u32) -> String {
        if !self.ligada() {
            String::new()
        } else if self.modo.por_letra() {
            // Fora da faixa nao acontece por construcao, mas um nome de
            // arquivo e o ultimo lugar onde se quer um `unwrap`: um volume
            // desconhecido vira `_Outros`, que existe e e legivel.
            let i = (volume as usize).clamp(1, BALDES.len());
            format!("_{}", BALDES[i - 1])
        } else {
            format!("_{:0largura$}", volume, largura = self.digitos as usize)
        }
    }

    /// Volume que deve receber o proximo bloco externo, dado quanto ja foi
    /// escrito no volume atual e o tamanho do bloco novo.
    ///
    /// Um bloco nunca e partido entre volumes: se nao couber no volume atual,
    /// vai inteiro para o proximo.
    ///
    /// `volume_vazio` diz se o volume atual ainda nao tem nenhum bloco. Um
    /// bloco maior que `bytes_por_arquivo` fica sozinho no seu volume, em vez
    /// de ser recusado -- caso contrario uma foto de 2 MB seria impossivel de
    /// gravar num arquivo de 1 MB, e trocar de volume nao resolveria nada.
    pub fn volume_externo(
        &self,
        volume_atual: u32,
        usado: u64,
        novo: u64,
        volume_vazio: bool,
    ) -> (u32, bool) {
        if !self.ligada() || volume_vazio || usado + novo <= self.bytes_por_arquivo {
            (volume_atual, false)
        } else {
            (volume_atual + 1, true)
        }
    }
}

impl Default for Paginacao {
    fn default() -> Self {
        Paginacao::DESLIGADA
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn largura_do_sufixo_entra_antes_do_teto() {
        // O teto de 9999 so e valido depois que o quarto digito existe. Na
        // ordem contraria a validacao recusa -- e foi assim que a tela de
        // nova tabela quebrou na primeira tentativa.
        let p = Paginacao::nova(1000, 1)
            .unwrap()
            .com_digitos(4)
            .unwrap()
            .com_max_arquivos(9_999)
            .unwrap();
        assert_eq!(p.max_arquivos, 9_999);
        assert_eq!(p.digitos, 4);
        assert_eq!(p.capacidade(), 9_999_000);

        // E continua recusando o que nao cabe.
        assert!(Paginacao::nova(1000, 1)
            .unwrap()
            .com_max_arquivos(1_000)
            .is_err());
        assert!(Paginacao::nova(1000, 1)
            .unwrap()
            .com_max_arquivos(0)
            .is_err());
    }

    #[test]
    fn desligada_mantem_arquivo_unico() {
        let p = Paginacao::DESLIGADA;
        assert!(!p.ligada());
        assert_eq!(p.localizar(1), (1, 1));
        assert_eq!(p.localizar(999_999), (1, 999_999));
        assert_eq!(p.sufixo(1), "");
        assert_eq!(p.capacidade(), u64::MAX);
    }

    #[test]
    fn fronteiras_de_volume() {
        let p = Paginacao::nova(1_000, 999).unwrap();
        assert_eq!(p.localizar(1), (1, 1));
        assert_eq!(p.localizar(1_000), (1, 1_000));
        assert_eq!(p.localizar(1_001), (2, 1));
        assert_eq!(p.localizar(2_000), (2, 1_000));
        assert_eq!(p.localizar(2_001), (3, 1));
    }

    #[test]
    fn o_rowid_e_global_e_a_ordem_de_digitacao_se_mantem() {
        let p = Paginacao::nova(10, 99).unwrap();
        // Percorrer rowids em ordem crescente percorre volumes em ordem
        // crescente, e dentro de cada volume os slots em ordem crescente.
        let mut ultimo = (0u32, 0u64);
        for rowid in 1..=100u64 {
            let atual = p.localizar(rowid);
            assert!(atual > ultimo, "rowid {rowid} quebrou a ordem");
            ultimo = atual;
        }
    }

    #[test]
    fn sufixo_e_zero_a_esquerda() {
        let p = Paginacao::nova(100, 999).unwrap();
        assert_eq!(p.sufixo(1), "_001");
        assert_eq!(p.sufixo(42), "_042");
        assert_eq!(p.sufixo(999), "_999");
        let q = p.com_digitos(4).unwrap();
        assert_eq!(q.sufixo(7), "_0007");
    }

    #[test]
    fn capacidade_e_tabela_cheia() {
        let p = Paginacao::nova(1_000, 3).unwrap();
        assert_eq!(p.capacidade(), 3_000);
        assert!(p.cabe(3_000));
        assert!(!p.cabe(3_001));
        assert!(!p.cabe(0));
    }

    #[test]
    fn max_arquivos_precisa_caber_nos_digitos() {
        assert!(
            Paginacao::nova(100, 1_000).is_err(),
            "1000 nao cabe em 3 digitos"
        );
        assert!(Paginacao::nova(100, 999).is_ok());
        assert!(Paginacao::nova(100, 1_000).is_err());
        assert!(Paginacao::nova(100, 999).unwrap().com_digitos(4).is_ok());
        assert!(Paginacao::nova(0, 10).is_err());
        assert!(Paginacao::nova(10, 0).is_err());
    }

    #[test]
    fn bloco_externo_nao_e_partido_entre_volumes() {
        let p = Paginacao::nova(1_000, 999)
            .unwrap()
            .com_bytes_por_arquivo(1_000)
            .unwrap();
        // Cabe: fica no volume atual.
        assert_eq!(p.volume_externo(1, 900, 100, false), (1, false));
        // Nao cabe por um byte: vai inteiro para o proximo.
        assert_eq!(p.volume_externo(1, 900, 101, false), (2, true));
        // Sem paginacao nunca troca de volume.
        assert_eq!(
            Paginacao::DESLIGADA.volume_externo(1, u64::MAX / 2, 1_000, false),
            (1, false)
        );
    }

    #[test]
    fn bloco_maior_que_o_volume_fica_sozinho_em_vez_de_ser_recusado() {
        let p = Paginacao::nova(1_000, 999)
            .unwrap()
            .com_bytes_por_arquivo(1_000)
            .unwrap();
        // Volume ainda vazio e o bloco e maior que o volume inteiro:
        // grava assim mesmo, senao nunca caberia em lugar nenhum.
        assert_eq!(p.volume_externo(5, 64, 50_000, true), (5, false));
        // Com o volume ja ocupado, ele rola para o proximo.
        assert_eq!(p.volume_externo(5, 500, 50_000, false), (6, true));
    }
}

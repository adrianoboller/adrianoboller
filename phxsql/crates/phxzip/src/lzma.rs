//! LZMA e LZMA2: o DECODIFICADOR. O codificador mora em `lzma_compressor.rs` e
//! usa o MESMO modelo de probabilidades daqui (`Modelo`) -- a disposicao das
//! tabelas e decisao escrita uma vez so, e um lado nao consegue divergir do
//! outro sem o teste de ida e volta acusar.
//!
//! # Por que os dois, e nao so o que o escritor grava
//!
//! O escritor grava LZMA2. Mas o administrador que abre o `.phz` no 7-Zip, mexe
//! e grava de novo tambem devolve LZMA2, e o cabecalho de um arquivo com muitas
//! entradas vem em **LZMA** puro. Ler so o proprio dialeto recusaria justamente
//! o arquivo que o operador acabou de conferir (a mesma assimetria que o
//! `zip.rs` do `phxsql-core` ja escolheu para o DEFLATE).
//!
//! # O que protege quem le arquivo de fora
//!
//! * **Teto de saida.** Quem chama diz quanto o fluxo PODE produzir (o tamanho
//!   declarado no cabecalho do 7z, ja conferido contra o teto do chamador), e o
//!   decodificador recusa ANTES de passar dele -- inclusive o pedaco de LZMA2
//!   que anuncia mais do que cabe, que e recusado antes de decodificar um byte.
//!   Bomba de descompressao para no teto, nao na memoria da maquina.
//! * **Janela = a propria saida.** O dicionario declarado pode ser de 4 GiB e
//!   nao se aloca nada por ele: como a saida inteira fica na memoria, ela e a
//!   janela (`DOC/lzma.txt`, «decoding of full stream to one RAM buffer»).
//! * **Toda distancia conferida** contra o que ja saiu desde o ultimo reinicio
//!   do dicionario e contra o tamanho do dicionario, como o `C/LzmaDec.c:539`.
//!   Sem isso um fluxo hostil leria antes do comeco do vetor.
//! * **Estrito no fim**, como o `C/7zDec.c:196` e `:201`: o fluxo tem de
//!   terminar exatamente no tamanho declarado, com o codificador de faixa em
//!   zero e sem sobra de entrada. Byte trocado costuma cair aqui antes do CRC.
//!
//! # De onde veio
//!
//! A especificacao `DOC/lzma-specification.txt` do LZMA SDK (Igor Pavlov,
//! 2015; o 7-Zip 26.03 traz o resumo em `DOC/lzma.txt`) e o `C/LzmaDec.c` /
//! `C/Lzma2Dec.c` do 7-Zip 26.03 (dominio publico), lidos e reescritos. Onde
//! diverge: la o decodificador e retomavel (aceita a entrada aos pedacos e
//! guarda o meio de um casamento entre chamadas); aqui a entrada inteira ja
//! esta na memoria -- o `.phz` e lido de uma vez --, entao cada bit devolve
//! `Result` e o laco e direto, sem a maquina de estados de retomada. Menos
//! velocidade, que um arquivo de configuracao nao sente, e muito menos lugar
//! para errar.

use alloc::vec;
use alloc::vec::Vec;

/// Por que o fluxo foi recusado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalhaLzma {
    /// A entrada acabou no meio do fluxo.
    Truncado,
    /// O fluxo quer produzir mais do que o tamanho declarado.
    PassouDoDeclarado,
    /// O fluxo e invalido; o texto diz onde.
    Invalido(&'static str),
}

impl core::fmt::Display for FalhaLzma {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FalhaLzma::Truncado => write!(f, "o fluxo LZMA acaba antes de fechar"),
            FalhaLzma::PassouDoDeclarado => {
                write!(
                    f,
                    "o fluxo LZMA quer produzir mais do que o tamanho declarado"
                )
            }
            FalhaLzma::Invalido(onde) => write!(f, "fluxo LZMA invalido: {onde}"),
        }
    }
}

type Resultado<T> = core::result::Result<T, FalhaLzma>;

pub(crate) const BITS_DO_MODELO: u32 = 11;
pub(crate) const PROB_INICIAL: u16 = 1 << (BITS_DO_MODELO - 1);
pub(crate) const BITS_DE_MOVIMENTO: u32 = 5;
pub(crate) const TOPO: u32 = 1 << 24;
const ESTADOS: usize = 12;
pub(crate) const POS_BITS_MAX: usize = 4;
/// `kEndPosModelIndex` da especificacao: da fenda 14 em diante os bits do meio
/// da distancia vem diretos, sem modelo.
pub(crate) const FIM_DO_MODELO_DE_POSICAO: u32 = 14;
/// `1 + kNumFullDistances - kEndPosModelIndex` = 1 + 128 - 14.
const ESPECIAIS: usize = 115;
/// O menor dicionario que o decodificador aceita (`LZMA_DIC_MIN`).
pub(crate) const DIC_MINIMO: u32 = 1 << 12;

// ------------------------------------------------------ codificador de faixa

/// O decodificador de faixa (`CRangeDecoder` da especificacao).
struct Faixa<'a> {
    dados: &'a [u8],
    pos: usize,
    alcance: u32,
    codigo: u32,
}

impl<'a> Faixa<'a> {
    fn nova(dados: &'a [u8]) -> Resultado<Faixa<'a>> {
        if dados.len() < 5 {
            return Err(FalhaLzma::Truncado);
        }
        // O codificador sempre escreve zero no primeiro byte; a especificacao
        // manda parar se nao for. E o primeiro lugar onde a senha errada aparece
        // num fluxo decifrado: 255 em 256 chaves erradas caem aqui.
        if dados[0] != 0 {
            return Err(FalhaLzma::Invalido(
                "o primeiro byte do codificador de faixa nao e zero",
            ));
        }
        Ok(Faixa {
            dados,
            pos: 5,
            alcance: u32::MAX,
            codigo: u32::from_be_bytes([dados[1], dados[2], dados[3], dados[4]]),
        })
    }

    fn normalizar(&mut self) -> Resultado<()> {
        if self.alcance < TOPO {
            let b = *self.dados.get(self.pos).ok_or(FalhaLzma::Truncado)?;
            self.pos += 1;
            self.alcance <<= 8;
            self.codigo = (self.codigo << 8) | b as u32;
        }
        Ok(())
    }

    /// Um bit com probabilidade modelada. As contas nao transbordam: `limite`
    /// e no maximo `(2^32 >> 11) * 2047`, e so se subtrai o que foi comparado.
    fn bit(&mut self, p: &mut u16) -> Resultado<u32> {
        let v = *p as u32;
        let limite = (self.alcance >> BITS_DO_MODELO) * v;
        let bit = if self.codigo < limite {
            *p = (v + (((1 << BITS_DO_MODELO) - v) >> BITS_DE_MOVIMENTO)) as u16;
            self.alcance = limite;
            0
        } else {
            *p = (v - (v >> BITS_DE_MOVIMENTO)) as u16;
            self.codigo -= limite;
            self.alcance -= limite;
            1
        };
        self.normalizar()?;
        Ok(bit)
    }

    /// Bits de probabilidade fixa (`DecodeDirectBits`), escritos como
    /// comparacao em vez do truque do sinal: da o mesmo resultado para fluxo
    /// valido, e para fluxo hostil nunca subtrai mais do que ha.
    fn diretos(&mut self, n: u32) -> Resultado<u32> {
        let mut r = 0u32;
        for _ in 0..n {
            self.alcance >>= 1;
            let bit = if self.codigo >= self.alcance {
                self.codigo -= self.alcance;
                1
            } else {
                0
            };
            r = (r << 1) | bit;
            self.normalizar()?;
        }
        Ok(r)
    }
}

/// Arvore de bits, do mais significativo para o menos.
fn arvore(f: &mut Faixa, probs: &mut [u16], bits: u32) -> Resultado<u32> {
    let mut m = 1usize;
    for _ in 0..bits {
        m = (m << 1) + f.bit(&mut probs[m])? as usize;
    }
    Ok(m as u32 - (1 << bits))
}

/// Arvore de bits reversa, do menos significativo para o mais.
fn arvore_reversa(f: &mut Faixa, probs: &mut [u16], bits: u32) -> Resultado<u32> {
    let mut m = 1usize;
    let mut simbolo = 0u32;
    for i in 0..bits {
        let b = f.bit(&mut probs[m])?;
        m = (m << 1) + b as usize;
        simbolo |= b << i;
    }
    Ok(simbolo)
}

// ------------------------------------------------------------------ o modelo

pub(crate) struct Comprimento {
    pub escolha: u16,
    pub escolha2: u16,
    pub baixo: [[u16; 8]; 1 << POS_BITS_MAX],
    pub medio: [[u16; 8]; 1 << POS_BITS_MAX],
    pub alto: [u16; 256],
}

impl Comprimento {
    fn novo() -> Comprimento {
        Comprimento {
            escolha: PROB_INICIAL,
            escolha2: PROB_INICIAL,
            baixo: [[PROB_INICIAL; 8]; 1 << POS_BITS_MAX],
            medio: [[PROB_INICIAL; 8]; 1 << POS_BITS_MAX],
            alto: [PROB_INICIAL; 256],
        }
    }

    /// O comprimento a partir de zero (0 a 271); o real e este mais 2.
    fn decodificar(&mut self, f: &mut Faixa, pos_estado: usize) -> Resultado<u32> {
        if f.bit(&mut self.escolha)? == 0 {
            return arvore(f, &mut self.baixo[pos_estado], 3);
        }
        if f.bit(&mut self.escolha2)? == 0 {
            return Ok(8 + arvore(f, &mut self.medio[pos_estado], 3)?);
        }
        Ok(16 + arvore(f, &mut self.alto, 8)?)
    }
}

/// Os parametros do modelo: bits de contexto do literal, de posicao do
/// literal e de posicao. Vem num byte so, `(pb * 5 + lp) * 9 + lc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Parametros {
    pub lc: u32,
    pub lp: u32,
    pub pb: u32,
}

impl Parametros {
    pub fn do_byte(b: u8) -> Resultado<Parametros> {
        let mut d = b as u32;
        if d >= 9 * 5 * 5 {
            return Err(FalhaLzma::Invalido(
                "byte de parametros lc/lp/pb fora da faixa",
            ));
        }
        let lc = d % 9;
        d /= 9;
        Ok(Parametros {
            lc,
            lp: d % 5,
            pb: d / 5,
        })
    }
}

/// Quantos bytes o `Modelo` ocupa com os parametros deste byte lc/lp/pb --
/// para quem chama conferir contra um teto ANTES de o decodificador alocar.
/// `None` quando o byte nem e valido (o decodificador recusa sozinho).
pub fn bytes_do_modelo(byte_dos_parametros: u8) -> Option<u64> {
    let par = Parametros::do_byte(byte_dos_parametros).ok()?;
    Some(((0x300u64 << (par.lc + par.lp)) * 2) + core::mem::size_of::<Modelo>() as u64)
}

/// Todas as probabilidades e o estado que atravessa os simbolos.
pub(crate) struct Modelo {
    pub par: Parametros,
    /// `0x300 << (lc + lp)` probabilidades. Com lc = 8 e lp = 4 -- o maximo do
    /// LZMA, que o LZMA2 nao permite -- sao 6 MiB, e e o unico lugar do
    /// decodificador onde o fluxo escolhe quanto se aloca. Fica, porque e o
    /// formato; o teto do chamador limita a saida, nao isto.
    pub literais: Vec<u16>,
    pub e_casamento: [u16; ESTADOS << POS_BITS_MAX],
    pub e_rep: [u16; ESTADOS],
    pub e_rep_g0: [u16; ESTADOS],
    pub e_rep_g1: [u16; ESTADOS],
    pub e_rep_g2: [u16; ESTADOS],
    pub e_rep0_longo: [u16; ESTADOS << POS_BITS_MAX],
    pub fenda: [[u16; 64]; 4],
    pub especiais: [u16; ESPECIAIS],
    pub alinhamento: [u16; 16],
    pub comp: Comprimento,
    pub comp_rep: Comprimento,
    pub estado: usize,
    pub reps: [u32; 4],
}

impl Modelo {
    pub fn novo(par: Parametros) -> Modelo {
        Modelo {
            par,
            literais: vec![PROB_INICIAL; 0x300usize << (par.lc + par.lp)],
            e_casamento: [PROB_INICIAL; ESTADOS << POS_BITS_MAX],
            e_rep: [PROB_INICIAL; ESTADOS],
            e_rep_g0: [PROB_INICIAL; ESTADOS],
            e_rep_g1: [PROB_INICIAL; ESTADOS],
            e_rep_g2: [PROB_INICIAL; ESTADOS],
            e_rep0_longo: [PROB_INICIAL; ESTADOS << POS_BITS_MAX],
            fenda: [[PROB_INICIAL; 64]; 4],
            especiais: [PROB_INICIAL; ESPECIAIS],
            alinhamento: [PROB_INICIAL; 16],
            comp: Comprimento::novo(),
            comp_rep: Comprimento::novo(),
            estado: 0,
            reps: [0; 4],
        }
    }

    /// Reinicio de estado do LZMA2: probabilidades, estado e distancias
    /// voltam ao comeco, e os parametros ficam.
    fn reiniciar(&mut self) {
        *self = Modelo::novo(self.par);
    }

    fn distancia(&mut self, f: &mut Faixa, comprimento: u32) -> Resultado<u32> {
        let estado_do_comp = (comprimento as usize).min(3);
        let fenda = arvore(f, &mut self.fenda[estado_do_comp], 6)?;
        if fenda < 4 {
            return Ok(fenda);
        }
        let diretos = (fenda >> 1) - 1;
        let mut dist = (2 | (fenda & 1)) << diretos;
        if fenda < FIM_DO_MODELO_DE_POSICAO {
            // `dist - fenda` vai de 0 (fenda 4) a 83 (fenda 13), e a arvore
            // de `diretos` bits toca ate o indice `2^diretos - 1`: o maior
            // acesso e 83 + 31 = 114, dentro dos 115.
            let base = (dist - fenda) as usize;
            dist += arvore_reversa(f, &mut self.especiais[base..], diretos)?;
        } else {
            // Na fenda 63 isto soma ate 0xC000_0000 + 0x3FFF_FFF0 + 15 =
            // 0xFFFF_FFFF, que e a marca de fim: nao ha transbordo possivel.
            dist += f.diretos(diretos - 4)? << 4;
            dist += arvore_reversa(f, &mut self.alinhamento, 4)?;
        }
        Ok(dist)
    }
}

// --------------------------------------------------------- o laco principal

/// Onde a janela comeca e ate onde a saida pode ir.
struct Janela {
    /// Posicao na saida do ultimo reinicio de dicionario: distancia nenhuma
    /// alcanca antes daqui.
    inicio: usize,
    dic: usize,
}

/// Decodifica simbolos ate a saida chegar a `alvo`. Devolve `true` se achou a
/// marca de fim antes -- o que so e aceito quando `aceita_marca`.
fn decodificar_ate(
    m: &mut Modelo,
    f: &mut Faixa,
    saida: &mut Vec<u8>,
    jan: &Janela,
    alvo: usize,
    aceita_marca: bool,
) -> Resultado<bool> {
    let mascara_pb = (1usize << m.par.pb) - 1;
    let mascara_lp = (1usize << m.par.lp) - 1;
    while saida.len() < alvo {
        let pos = saida.len() - jan.inicio;
        let pos_estado = pos & mascara_pb;
        let s2 = (m.estado << POS_BITS_MAX) + pos_estado;

        if f.bit(&mut m.e_casamento[s2])? == 0 {
            // Literal.
            let anterior = if pos > 0 { saida[saida.len() - 1] } else { 0 };
            let ctx = ((pos & mascara_lp) << m.par.lc) + ((anterior as usize) >> (8 - m.par.lc));
            let probs = &mut m.literais[0x300 * ctx..0x300 * (ctx + 1)];
            let mut simbolo = 1usize;
            if m.estado >= 7 {
                // Depois de um casamento, o byte que estaria na distancia rep0
                // ajuda a prever o literal. A rep0 ja foi conferida quando o
                // casamento aconteceu; confere-se de novo porque custa uma
                // comparacao e um indice fora do vetor custaria um panico.
                let d = m.reps[0] as usize;
                if d >= pos {
                    return Err(FalhaLzma::Invalido(
                        "literal casado antes do comeco da janela",
                    ));
                }
                let mut casado = saida[saida.len() - 1 - d] as usize;
                loop {
                    let bit_casado = (casado >> 7) & 1;
                    casado <<= 1;
                    let bit = f.bit(&mut probs[((1 + bit_casado) << 8) + simbolo])? as usize;
                    simbolo = (simbolo << 1) | bit;
                    if bit_casado != bit || simbolo >= 0x100 {
                        break;
                    }
                }
            }
            while simbolo < 0x100 {
                simbolo = (simbolo << 1) | f.bit(&mut probs[simbolo])? as usize;
            }
            saida.push((simbolo - 0x100) as u8);
            m.estado = match m.estado {
                0..=3 => 0,
                4..=9 => m.estado - 3,
                _ => m.estado - 6,
            };
            continue;
        }

        let comprimento;
        if f.bit(&mut m.e_rep[m.estado])? == 0 {
            // Casamento simples: distancia nova.
            comprimento = m.comp.decodificar(f, pos_estado)?;
            m.estado = if m.estado < 7 { 7 } else { 10 };
            let dist = m.distancia(f, comprimento)?;
            if dist == u32::MAX {
                if aceita_marca {
                    return Ok(true);
                }
                return Err(FalhaLzma::Invalido(
                    "marca de fim onde o formato nao a admite",
                ));
            }
            m.reps = [dist, m.reps[0], m.reps[1], m.reps[2]];
        } else {
            if pos == 0 {
                return Err(FalhaLzma::Invalido("repeticao com a janela vazia"));
            }
            if f.bit(&mut m.e_rep_g0[m.estado])? == 0 {
                if f.bit(&mut m.e_rep0_longo[s2])? == 0 {
                    // Repeticao curta: um byte so, na distancia rep0.
                    m.estado = if m.estado < 7 { 9 } else { 11 };
                    let d = m.reps[0] as usize;
                    if d >= pos || d >= jan.dic {
                        return Err(FalhaLzma::Invalido("distancia antes do comeco da janela"));
                    }
                    let b = saida[saida.len() - 1 - d];
                    saida.push(b);
                    continue;
                }
            } else {
                let dist;
                if f.bit(&mut m.e_rep_g1[m.estado])? == 0 {
                    dist = m.reps[1];
                } else {
                    if f.bit(&mut m.e_rep_g2[m.estado])? == 0 {
                        dist = m.reps[2];
                    } else {
                        dist = m.reps[3];
                        m.reps[3] = m.reps[2];
                    }
                    m.reps[2] = m.reps[1];
                }
                m.reps[1] = m.reps[0];
                m.reps[0] = dist;
            }
            comprimento = m.comp_rep.decodificar(f, pos_estado)?;
            m.estado = if m.estado < 7 { 8 } else { 11 };
        }

        let d = m.reps[0] as usize;
        if d >= pos || d >= jan.dic {
            return Err(FalhaLzma::Invalido("distancia antes do comeco da janela"));
        }
        let tam = comprimento as usize + 2;
        if tam > alvo - saida.len() {
            return Err(FalhaLzma::PassouDoDeclarado);
        }
        // Byte a byte de proposito: a copia pode se sobrepor a si mesma
        // (distancia 1, comprimento 200 = repetir um byte), e copiar a fatia de
        // uma vez daria outro resultado.
        let origem = saida.len() - 1 - d;
        for k in 0..tam {
            let b = saida[origem + k];
            saida.push(b);
        }
    }
    Ok(false)
}

/// Depois do tamanho declarado, o fluxo LZMA pode ainda trazer a marca de fim.
/// Aqui ela tem de ser o PROXIMO simbolo -- qualquer outra coisa seria o fluxo
/// querendo passar do declarado.
fn ler_marca_de_fim(m: &mut Modelo, f: &mut Faixa, pos: usize) -> Resultado<()> {
    let pos_estado = pos & ((1usize << m.par.pb) - 1);
    let s2 = (m.estado << POS_BITS_MAX) + pos_estado;
    if f.bit(&mut m.e_casamento[s2])? == 0 || f.bit(&mut m.e_rep[m.estado])? != 0 {
        return Err(FalhaLzma::PassouDoDeclarado);
    }
    let comprimento = m.comp.decodificar(f, pos_estado)?;
    m.estado = if m.estado < 7 { 7 } else { 10 };
    if m.distancia(f, comprimento)? != u32::MAX {
        return Err(FalhaLzma::PassouDoDeclarado);
    }
    Ok(())
}

pub(crate) fn tamanho_do_dic(bruto: u32) -> usize {
    // `u32` cabe em `usize` em todo alvo com 32 bits ou mais; num alvo de 16
    // bits satura, e a conferencia `d >= dic` continua certa porque a saida
    // nunca passaria disso.
    usize::try_from(bruto.max(DIC_MINIMO)).unwrap_or(usize::MAX)
}

/// LZMA do 7z (metodo `030101`): as cinco propriedades do coder -- o byte
/// lc/lp/pb e o dicionario em 4 bytes little-endian -- e um fluxo que produz
/// EXATAMENTE `tamanho` bytes, com ou sem a marca de fim depois deles.
pub fn lzma(propriedades: &[u8], entrada: &[u8], tamanho: usize) -> Result<Vec<u8>, FalhaLzma> {
    let mut saida = Vec::new();
    lzma_em(propriedades, entrada, tamanho, &mut saida)?;
    Ok(saida)
}

fn lzma_em(
    propriedades: &[u8],
    entrada: &[u8],
    tamanho: usize,
    saida: &mut Vec<u8>,
) -> Resultado<()> {
    if propriedades.len() != 5 {
        return Err(FalhaLzma::Invalido(
            "o LZMA do 7z tem cinco bytes de propriedades",
        ));
    }
    let par = Parametros::do_byte(propriedades[0])?;
    let dic = u32::from_le_bytes([
        propriedades[1],
        propriedades[2],
        propriedades[3],
        propriedades[4],
    ]);
    let jan = Janela {
        inicio: 0,
        dic: tamanho_do_dic(dic),
    };
    let mut m = Modelo::novo(par);
    let mut f = Faixa::nova(entrada)?;
    // A saida cresce com o que sai de fato, e nao com o que o cabecalho diz: um
    // tamanho declarado mentiroso nao reserva memoria nenhuma sozinho.
    saida.reserve(tamanho.min(1 << 16));
    let marca = decodificar_ate(&mut m, &mut f, saida, &jan, tamanho, true)?;
    if marca {
        return Err(FalhaLzma::Invalido(
            "marca de fim antes do tamanho declarado",
        ));
    }
    if f.codigo != 0 {
        ler_marca_de_fim(&mut m, &mut f, saida.len())?;
    }
    if f.codigo != 0 {
        return Err(FalhaLzma::Invalido(
            "o codificador de faixa nao termina em zero",
        ));
    }
    if f.pos != entrada.len() {
        return Err(FalhaLzma::Invalido("sobra entrada depois do fim do fluxo"));
    }
    Ok(())
}

/// O dicionario do LZMA2, do byte de propriedade do coder (`21`).
pub(crate) fn dic_do_lzma2(prop: u8) -> Resultado<u32> {
    match prop {
        0..=39 => Ok((2 | (prop as u32 & 1)) << (prop / 2 + 11)),
        40 => Ok(u32::MAX),
        _ => Err(FalhaLzma::Invalido("dicionario do LZMA2 acima de 4 GiB")),
    }
}

/// LZMA2 do 7z (metodo `21`): uma sequencia de pedacos, cada um com o proprio
/// tamanho, ate o byte de controle zero. Produz EXATAMENTE `tamanho` bytes.
pub fn lzma2(prop: u8, entrada: &[u8], tamanho: usize) -> Result<Vec<u8>, FalhaLzma> {
    let mut saida = Vec::new();
    lzma2_em(prop, entrada, tamanho, &mut saida)?;
    Ok(saida)
}

fn be16(entrada: &[u8], pos: usize) -> Resultado<usize> {
    let b = entrada.get(pos..pos + 2).ok_or(FalhaLzma::Truncado)?;
    Ok(((b[0] as usize) << 8) | b[1] as usize)
}

fn lzma2_em(prop: u8, entrada: &[u8], tamanho: usize, saida: &mut Vec<u8>) -> Resultado<()> {
    let dic = tamanho_do_dic(dic_do_lzma2(prop)?);
    saida.reserve(tamanho.min(1 << 16));
    let mut pos = 0usize;
    let mut jan = Janela { inicio: 0, dic };
    let mut modelo: Option<Modelo> = None;
    // O que o proximo pedaco LZMA e obrigado a reiniciar (`needInitLevel` do
    // `C/Lzma2Dec.c:88`): no comeco, o dicionario (0xE0); depois de um pedaco
    // cru que reiniciou o dicionario, os parametros (0xC0); fora isso, nada.
    let mut exige = 0xE0u8;
    loop {
        let controle = *entrada.get(pos).ok_or(FalhaLzma::Truncado)?;
        pos += 1;
        if controle == 0 {
            break;
        }
        if controle < 0x80 {
            // Pedaco sem compressao: 1 reinicia o dicionario, 2 nao.
            match controle {
                1 => {
                    exige = 0xC0;
                    jan.inicio = saida.len();
                }
                2 if exige == 0xE0 => {
                    return Err(FalhaLzma::Invalido(
                        "o primeiro pedaco do LZMA2 nao reinicia o dicionario",
                    ));
                }
                2 => {}
                _ => {
                    return Err(FalhaLzma::Invalido(
                        "byte de controle do LZMA2 desconhecido",
                    ))
                }
            }
            let n = be16(entrada, pos)? + 1;
            pos += 2;
            if n > tamanho - saida.len() {
                return Err(FalhaLzma::PassouDoDeclarado);
            }
            let cru = entrada.get(pos..pos + n).ok_or(FalhaLzma::Truncado)?;
            saida.extend_from_slice(cru);
            pos += n;
            continue;
        }

        if controle < exige {
            return Err(FalhaLzma::Invalido(
                "pedaco LZMA sem o reinicio que o pedaco anterior exige",
            ));
        }
        exige = 0;
        let desempacotado = (((controle & 0x1F) as usize) << 16) + be16(entrada, pos)? + 1;
        let empacotado = be16(entrada, pos + 2)? + 1;
        pos += 4;
        if controle >= 0xE0 {
            jan.inicio = saida.len();
        }
        if controle >= 0xC0 {
            let par = Parametros::do_byte(*entrada.get(pos).ok_or(FalhaLzma::Truncado)?)?;
            pos += 1;
            if par.lc + par.lp > 4 {
                return Err(FalhaLzma::Invalido("LZMA2 com lc + lp acima de 4"));
            }
            modelo = Some(Modelo::novo(par));
        } else if controle >= 0xA0 {
            modelo
                .as_mut()
                .ok_or(FalhaLzma::Invalido("reinicio de estado sem parametros"))?
                .reiniciar();
        }
        let m = modelo
            .as_mut()
            .ok_or(FalhaLzma::Invalido("pedaco LZMA sem parametros"))?;
        // A conferencia que faz a bomba parar: o pedaco ANUNCIA quanto vai
        // produzir, e o anuncio e comparado com o que ainda cabe antes de
        // decodificar um byte dele.
        if desempacotado > tamanho - saida.len() {
            return Err(FalhaLzma::PassouDoDeclarado);
        }
        let trecho = entrada
            .get(pos..pos + empacotado)
            .ok_or(FalhaLzma::Truncado)?;
        pos += empacotado;
        let mut f = Faixa::nova(trecho)?;
        let alvo = saida.len() + desempacotado;
        decodificar_ate(m, &mut f, saida, &jan, alvo, false)?;
        if f.codigo != 0 || f.pos != trecho.len() {
            return Err(FalhaLzma::Invalido(
                "o pedaco LZMA nao fecha onde o cabecalho dele disse",
            ));
        }
    }
    if pos != entrada.len() {
        return Err(FalhaLzma::Invalido("sobra entrada depois do fim do LZMA2"));
    }
    if saida.len() != tamanho {
        return Err(FalhaLzma::Invalido(
            "o LZMA2 produziu menos do que o declarado",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use alloc::format;
    use alloc::string::String;

    fn hex(s: &str) -> Vec<u8> {
        phxsql_core::hash::de_hex(s).expect("hexadecimal do vetor")
    }

    // Os vetores abaixo NAO sao do nosso codificador (nao ha um): sao bytes do
    // liblzma (xz-utils), uma implementacao independente do 7-Zip, gerados pelo
    // `lzma` do Python com o comando escrito ao lado de cada um. Ida e volta com
    // codigo proprio nao provaria nada -- os dois lados poderiam errar juntos.

    /// O texto de onde sai quase todo vetor: repeticao curta, longa e literal.
    fn texto_do_rato() -> Vec<u8> {
        let mut t = String::new();
        for i in 0..40 {
            t.push_str(&format!("o rato roeu a roupa do rei de roma, linha {i}\n"));
        }
        t.into_bytes()
    }

    /// `lzma.compress(texto, format=FORMAT_RAW, filters=[{"id": FILTER_LZMA1,
    /// "dict_size": 1<<16, "lc": 3, "lp": 0, "pb": 2}])` -- com a marca de fim,
    /// que o liblzma sempre escreve no LZMA cru.
    const LZMA1_RATO: &str =
        "0037880a46547785aa9abea5f235fc5938124568a3962d9dba83d47d6c96c29570610bea\
     4a41d0624a4ef5e143ba24781c75e9e85657b618e81da9638aca91387305c52c69d12383\
     2a643125e9e9749c25e37289ed0cf6de80f1ef173d27e39e4dc25b955c69f3607a2aec98\
     6505b9f8a39b81057f994623dcffff7e9f0000";
    /// As propriedades do mesmo vetor: `(pb*5 + lp)*9 + lc` = 0x5D, dicionario
    /// de 64 KiB little-endian.
    const PROPS_RATO: [u8; 5] = [0x5d, 0x00, 0x00, 0x01, 0x00];

    /// O mesmo texto em `FILTER_LZMA2`, dicionario de 64 KiB (propriedade 8).
    const LZMA2_RATO: &str =
        "e006fd00785d0037880a46547785aa9abea5f235fc5938124568a3962d9dba83d47d6c96\
     c29570610bea4a41d0624a4ef5e143ba24781c75e9e85657b618e81da9638aca91387305\
     c52c69d123832a643125e9e9749c25e37289ed0cf6de80f1ef173d27e39e4dc25b955c69\
     f3607a2aec986505b9f8a39b81057f97247f8400";

    /// 300 bytes do `xorshift` abaixo em `FILTER_LZMA2`: nao comprime, e o
    /// liblzma grava um pedaco CRU (controle 0x01).
    const LZMA2_RUIDO: &str =
        "01012b193e3ab51f37d0bf39b8eeb4d33cb85f8ade7d3fbfded8a21c49ea8ee174a69a6b\
     41c77a7e7eaedf9d7329b476653da6db74cefd7d440f6877e529b894982ec053cfe2ecb0\
     ab2cbdcbb7c7c0872b7601059ffbe084a486d81b6cf1691c090ff81b0cedddcaa1bd429d\
     0cdebfa935e0554fb3d7788365bb8f16bb301b4de11dc1e57899d872acae2dc68d2f9190\
     785d747419c2f4772e31ec2b031606fa1085a29924d3b1c800c7fca6834cbee60851f3ef\
     b510af0d868a932e65e9862bd649b5fe36accde7f6ee18ac06c075fd182e8aba1a5a30f9\
     ff0241f1b6446ad82313eac4b0a0c2ae24b7f6b8a065a2d8b5910caf6ebc3d81d6933219\
     8908ee72cc6a93f6c5b087189270ad3709ce7d52fd0b7934fca8d303e0d8357e27e71da4\
     06ba7099abeff804672e5b527105b300";

    /// `bytes(3_000_000)` em `FILTER_LZMA2`: 510 bytes que viram 3 MB, em dois
    /// pedacos -- 0xFF (tudo reiniciado, 2.096.914 bytes) e 0x8D (continua o
    /// estado, 903.086 bytes). E a bomba de descompressao pequena de verdade.
    const LZMA2_ZEROS: &str =
        "ffff11016c5d00006ffdffffa3b7ff473e481572396151b89228e6a38607f9eee41e82d3\
     2fc53a3c014bb17ec98a8a4d2fa30dd97fa6e38c231153e05918c5758ae277f8b6947f0c\
     6ac0de744964e2e95c53b204d8f7440cab5f0d6d46e9e5c37688b79657acb64de1691d6f\
     fb4b88106c42cb883f5c008fd04eaf262894711f3d8f24e1709ea7235fec28cb85d19598\
     8a7e2a91f22775f719c006984d98fdd8afd5900fc42553f8f591363105a5b0ee6fc1704d\
     470cd19111aaad601dbaceb127185c5986e9665258bee976ac59e4e55b0508f9c7daadfc\
     fb522b74cd1e5b2042f9dd533df82964093b80cb2a6cdfb53bf0c4bd2e5faa0f3e4b6642\
     90130eff1093f8717859f80bcdff9528460fa9fc7cdefb9a302e56c08f85f38381c065c4\
     2553f8f591363105a5b0ee6fc1704d470cd19111aaad601dbaceb127185c5986e9665258\
     bee976ac59e4e55b0508f9c7daadfcfb522b74cd1e5b2042f9dd533df82964093b80cb2a\
     6cdfb53bf0c4bc4827e6588dc7ad008400ec7353a7fdbeae7c311a9fb78d316e709ea723\
     5fec28cb85d195988a7e2a91f22775f719c006984d98fdd8afd5900fc42553f8f5913631\
     05a5b0ee6fc1704d470cd19111aaad601dbaceb127185c5986e9665258bee976ac59e4e5\
     5b0508f9c7daadfcfb522b74cd1e5b2042f9dd533df82964093b80cb2a6cdfb53bf0c4bd\
     14792d5d0000";

    /// O mesmo gerador do vetor em Python: xorshift32 a partir de 0x9E3779B9.
    fn xorshift(n: usize) -> Vec<u8> {
        let mut x: u32 = 0x9E37_79B9;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            v.push(x as u8);
        }
        v
    }

    #[test]
    fn lzma1_do_liblzma_com_a_marca_de_fim() {
        let t = texto_do_rato();
        assert_eq!(lzma(&PROPS_RATO, &hex(LZMA1_RATO), t.len()).unwrap(), t);
    }

    #[test]
    fn lzma2_do_liblzma() {
        let t = texto_do_rato();
        assert_eq!(lzma2(8, &hex(LZMA2_RATO), t.len()).unwrap(), t);
    }

    #[test]
    fn lzma2_pedaco_cru() {
        assert_eq!(lzma2(8, &hex(LZMA2_RUIDO), 300).unwrap(), xorshift(300));
    }

    /// Dois pedacos, o segundo SEM reinicio: o modelo atravessa a fronteira e o
    /// codificador de faixa recomeca. Se o estado nao atravessasse, o segundo
    /// pedaco sairia lixo.
    #[test]
    fn lzma2_segundo_pedaco_continua_o_estado() {
        let v = lzma2(8, &hex(LZMA2_ZEROS), 3_000_000).unwrap();
        assert_eq!(v.len(), 3_000_000);
        assert!(v.iter().all(|&b| b == 0));
    }

    /// A bomba: 510 bytes anunciando 3 MB, lidos com teto de 1.000. O pedaco
    /// ANUNCIA 2.096.914 bytes e tem de ser recusado antes de decodificar um
    /// so -- e o vermelho, com a conferencia tirada, mede quanto saiu.
    #[test]
    fn a_bomba_para_no_teto_sem_produzir_o_que_anuncia() {
        let mut saida = Vec::new();
        let r = lzma2_em(8, &hex(LZMA2_ZEROS), 1_000, &mut saida);
        assert!(
            saida.len() <= 1_000,
            "a bomba produziu {} bytes com teto de 1.000",
            saida.len()
        );
        assert_eq!(r, Err(FalhaLzma::PassouDoDeclarado));
    }

    /// O pedaco cru tambem anuncia o tamanho, e tambem e conferido antes.
    #[test]
    fn pedaco_cru_que_passa_do_declarado_e_recusado_antes_de_copiar() {
        let mut saida = Vec::new();
        let r = lzma2_em(8, &hex(LZMA2_RUIDO), 100, &mut saida);
        assert!(
            saida.len() <= 100,
            "copiou {} bytes com teto de 100",
            saida.len()
        );
        assert_eq!(r, Err(FalhaLzma::PassouDoDeclarado));
    }

    /// Declarado menor que o conteudo: o LZMA cru acha um simbolo que nao e a
    /// marca de fim onde ela deveria estar.
    #[test]
    fn lzma1_que_passa_do_declarado_e_recusado() {
        let t = texto_do_rato();
        let r = lzma(&PROPS_RATO, &hex(LZMA1_RATO), t.len() - 10);
        assert_eq!(r, Err(FalhaLzma::PassouDoDeclarado));
    }

    /// Pedaco cru que reinicia o dicionario, seguido de pedaco LZMA 0xC0
    /// (estado e parametros novos, dicionario mantido) -- a sequencia que o
    /// 7-Zip grava quando o comeco do conteudo nao comprime. O pedaco LZMA e o
    /// do `LZMA2_RATO` com o controle trocado; ele decodifica igual porque o
    /// cru tem 16 bytes (multiplo de 2^pb) e termina em `\n` (contexto de
    /// literal zero, como no comeco).
    #[test]
    fn pedaco_cru_seguido_de_lzma_com_parametros_novos() {
        let cru = b"cabecalho cru..\n";
        let rato = hex(LZMA2_RATO);
        let montar = |controle: u8| {
            let mut v = vec![0x01, 0x00, (cru.len() - 1) as u8];
            v.extend_from_slice(cru);
            v.push(controle | (rato[0] & 0x1f));
            v.extend_from_slice(&rato[1..]);
            v
        };
        let mut esperado = cru.to_vec();
        esperado.extend_from_slice(&texto_do_rato());
        assert_eq!(lzma2(8, &montar(0xc0), esperado.len()).unwrap(), esperado);
        // Depois de um cru que reiniciou o dicionario, o LZMA TEM de trazer
        // parametros: 0xA0 (so estado) e 0x80 (nada) sao recusados.
        for controle in [0xa0u8, 0x80] {
            assert!(
                matches!(
                    lzma2(8, &montar(controle), esperado.len()),
                    Err(FalhaLzma::Invalido(_))
                ),
                "controle {controle:#x} passou sem os parametros"
            );
        }
    }

    #[test]
    fn primeiro_pedaco_sem_reinicio_de_dicionario_e_recusado() {
        // Um cru 0x02 e um LZMA 0xC0 logo no comeco: nenhum reinicia o dicionario.
        assert!(matches!(
            lzma2(8, &[0x02, 0x00, 0x00, 0x41, 0x00], 1),
            Err(FalhaLzma::Invalido(_))
        ));
        let mut rato = hex(LZMA2_RATO);
        rato[0] = 0xc0;
        assert!(matches!(lzma2(8, &rato, 2000), Err(FalhaLzma::Invalido(_))));
    }

    #[test]
    fn o_dicionario_do_lzma2_segue_a_tabela() {
        assert_eq!(dic_do_lzma2(0), Ok(4096));
        assert_eq!(dic_do_lzma2(1), Ok(6144));
        assert_eq!(dic_do_lzma2(8), Ok(65536));
        assert_eq!(dic_do_lzma2(40), Ok(u32::MAX));
        assert!(dic_do_lzma2(41).is_err());
    }

    /// Todo prefixo de um fluxo valido e recusado com erro, e nenhum derruba a
    /// thread. E a prova de que todo tamanho lido do fluxo e conferido.
    #[test]
    fn fluxo_cortado_em_qualquer_ponto_e_recusado_sem_panico() {
        let t = texto_do_rato();
        for v in [hex(LZMA1_RATO), hex(LZMA2_RATO)] {
            for corte in 0..v.len() {
                let a = lzma(&PROPS_RATO, &v[..corte], t.len());
                let b = lzma2(8, &v[..corte], t.len());
                assert!(
                    a.is_err() && b.is_err(),
                    "o prefixo de {corte} bytes passou"
                );
            }
        }
    }

    /// Cada bit de cada byte trocado, um de cada vez. Nao se exige erro -- um
    /// bit trocado pode decodificar para outro texto do mesmo tamanho, e quem
    /// pega isso e o CRC do 7z --, exige-se NAO derrubar a thread: e aqui que
    /// uma distancia sem conferencia leria antes do comeco do vetor.
    #[test]
    fn fluxo_adulterado_nunca_derruba_a_thread() {
        let t = texto_do_rato();
        let mut recusados = 0usize;
        let mut total = 0usize;
        for (v, e_lzma2) in [(hex(LZMA1_RATO), false), (hex(LZMA2_RATO), true)] {
            for i in 0..v.len() {
                for bit in 0..8 {
                    let mut w = v.clone();
                    w[i] ^= 1 << bit;
                    let r = if e_lzma2 {
                        lzma2(8, &w, t.len())
                    } else {
                        lzma(&PROPS_RATO, &w, t.len())
                    };
                    total += 1;
                    if r.is_err() {
                        recusados += 1;
                    }
                }
            }
        }
        // O numero nao e o ponto; e registro de que o fim estrito pega quase
        // tudo antes do CRC.
        assert!(
            recusados * 10 > total * 9,
            "so {recusados} de {total} recusados"
        );
    }
}

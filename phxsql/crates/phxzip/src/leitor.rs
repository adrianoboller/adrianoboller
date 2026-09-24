//! O leitor do 7z: abre o arquivo, lista as entradas e extrai.
//!
//! # O que ele aceita
//!
//! O formato inteiro do `DOC/7zFormat.txt` no que diz respeito a ESTRUTURA --
//! cabecalho em claro ou codificado (`kEncodedHeader`), blocos solidos ou nao,
//! entradas vazias, pastas, nomes, datas e atributos. Dos metodos, so o
//! conjunto atual do 7z (decisao do dono, 24/09/2026): `Copy`, `LZMA`, `LZMA2` e
//! `7zAES`. O resto e recusado pelo nome -- [`Erro::MetodoLegado`] --, antes de
//! decodificar um byte.
//!
//! # O que ele recusa, e por que no motor
//!
//! * **Nome perigoso** ([`conferir_nome`]): a web e o terminal vao extrair, e a
//!   conferencia que cada chamador teria de lembrar e a que um deles esquece.
//! * **Tamanho mentiroso**: todo tamanho e deslocamento lido do arquivo e
//!   conferido contra o que existe ANTES de qualquer alocacao; os descompactados
//!   contra os [`Limites`] de quem chamou.
//! * **Cifrado sem CRC**: sem um CRC do conteudo decifrado, senha errada viraria
//!   lixo aceito. O 7-Zip sempre grava; arquivo que nao tem e recusado.
//!
//! # De onde veio
//!
//! `DOC/7zFormat.txt` e o `C/7zArcIn.c` do 7-Zip 26.03 (dominio publico). O
//! leitor de la e o modelo das conferencias: a assinatura e a versao maior
//! (`7zArcIn.c:1489`), o CRC do cabecalho de inicio, o limite dos fluxos
//! empacotados antes do cabecalho (`RangeLimit`), os limites de coder por bloco
//! (`C/7z.h:38`), a leitura dos subfluxos (`7zArcIn.c:832`) e das entradas
//! (`7zArcIn.c:1077`). Onde diverge, e por restricao nossa: la o cabecalho e
//! lido de um fluxo que se procura por posicao; aqui o arquivo inteiro e uma
//! fatia -- a crate nao abre arquivo --, e isso deixa toda conferencia de
//! limite ser uma comparacao contra `dados.len()`.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use phxsql_core::crc::crc32;

use crate::aes::{self, Aes256, BLOCO};
use crate::chave::{self, Chaves, ID_7ZAES};
use crate::erro::Erro;
use crate::lzma::{self, FalhaLzma};

/// A assinatura do 7z: `'7' 'z' BC AF 27 1C`.
pub const ASSINATURA: [u8; 6] = [b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C];
/// O cabecalho de inicio: assinatura, versao, CRC e o `StartHeader`.
pub(crate) const INICIO: usize = 32;

pub(crate) const ID_COPIA: u64 = 0x00;
pub(crate) const ID_LZMA2: u64 = 0x21;
pub(crate) const ID_LZMA: u64 = 0x03_0101;

/// Os identificadores de propriedade (`7zFormat.txt`, «Property IDs»).
pub(crate) mod id {
    pub const FIM: u64 = 0x00;
    pub const CABECALHO: u64 = 0x01;
    pub const PROPRIEDADES: u64 = 0x02;
    pub const FLUXOS_ADICIONAIS: u64 = 0x03;
    pub const FLUXOS: u64 = 0x04;
    pub const ARQUIVOS: u64 = 0x05;
    pub const EMPACOTADOS: u64 = 0x06;
    pub const DESEMPACOTADOS: u64 = 0x07;
    pub const SUBFLUXOS: u64 = 0x08;
    pub const TAMANHO: u64 = 0x09;
    pub const CRC: u64 = 0x0A;
    pub const BLOCO: u64 = 0x0B;
    pub const TAMANHOS_DOS_CODERS: u64 = 0x0C;
    pub const QUANTOS_SUBFLUXOS: u64 = 0x0D;
    pub const FLUXO_VAZIO: u64 = 0x0E;
    pub const ARQUIVO_VAZIO: u64 = 0x0F;
    pub const ANTI: u64 = 0x10;
    pub const NOME: u64 = 0x11;
    pub const MODIFICADO: u64 = 0x14;
    pub const ATRIBUTOS: u64 = 0x15;
    pub const CABECALHO_CODIFICADO: u64 = 0x17;
}

/// Atributo de pasta do Windows, que o 7z guarda.
pub(crate) const ATRIBUTO_PASTA: u32 = 0x10;
/// Coders por bloco, o mesmo limite do leitor de referencia (`C/7z.h:38`).
const CODERS_MAX: u64 = 4;

/// O nome do metodo, para a recusa dizer QUAL -- os identificadores sao os do
/// `DOC/Methods.txt`. Tudo aqui e legado por decisao do dono.
fn metodo_legado(id: u64) -> Option<&'static str> {
    Some(match id {
        0x03 => "Delta",
        0x04 | 0x0303_0103 => "BCJ (x86)",
        0x0303_011B => "BCJ2",
        0x05 | 0x0303_0205 => "PPC",
        0x06 | 0x0303_0401 => "IA64",
        0x07 | 0x0303_0501 => "ARM",
        0x08 | 0x0303_0701 => "ARMT",
        0x09 | 0x0303_0805 => "SPARC",
        0x0A => "ARM64",
        0x0B => "RISCV",
        0x0303_0301 => "Alpha",
        0x0303_0605 => "M68",
        0x02_0302 | 0x02_0304 => "Swap",
        0x03_0401 => "PPMd",
        0x04_0108 => "Deflate",
        0x04_0109 => "Deflate64",
        0x04_0202 => "BZip2",
        0x04_010C => "BZip2 (zip)",
        0x04_010E => "LZMA (zip)",
        0x04_015D | 0x04F7_1101 => "ZSTD",
        0x04_0162 => "PPMd (zip)",
        0x04_0163 => "wzAES (zip)",
        0x06F1_0101 => "ZipCrypto",
        0x06F1_0303 => "Rar29AES",
        _ => return None,
    })
}

fn conferir_metodo(id: u64) -> Result<(), Erro> {
    match id {
        ID_COPIA | ID_LZMA | ID_LZMA2 | ID_7ZAES => Ok(()),
        _ => Err(match metodo_legado(id) {
            Some(nome) => Erro::MetodoLegado(format!("{nome} ({id:X})")),
            None => Erro::MetodoDesconhecido(format!("{id:X}")),
        }),
    }
}

/// Converte um tamanho do arquivo para o endereco desta plataforma, com nome.
pub(crate) fn para_usize(valor: u64, oque: &'static str) -> Result<usize, Erro> {
    usize::try_from(valor).map_err(|_| Erro::NaoCabe { oque, valor })
}

fn estrutura(onde: &str) -> Erro {
    Erro::Estrutura(String::from(onde))
}

// ------------------------------------------------------------------ nomes

/// Confere um nome de entrada e devolve-o com `/` como separador.
///
/// Recusa o que, extraido, escreveria fora do destino -- caminho absoluto,
/// `..` em qualquer componente, letra de unidade, NUL -- e trata a barra
/// invertida como separador, porque e isso que ela e no Windows: `..\x` e
/// tao perigoso quanto `../x`, e um conferidor que so olhasse `/` deixaria
/// passar exatamente o caminho que o Windows segue. A barra final de pasta
/// sai; `.` e componente vazio ficam, porque nao sobem de nivel.
pub fn conferir_nome(bruto: &str) -> Result<String, Erro> {
    let perigoso = || Erro::NomePerigoso(String::from(bruto));
    if bruto.contains('\0') {
        return Err(perigoso());
    }
    let mut nome: String = bruto
        .chars()
        .map(|c| if c == '\\' { '/' } else { c })
        .collect();
    while nome.len() > 1 && nome.ends_with('/') {
        nome.pop();
    }
    if nome.is_empty() || nome.starts_with('/') {
        return Err(perigoso());
    }
    let b = nome.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return Err(perigoso());
    }
    if nome.split('/').any(|componente| componente == "..") {
        return Err(perigoso());
    }
    Ok(nome)
}

// ------------------------------------------------------ leitura de bytes

/// Cursor sobre o cabecalho, com toda leitura conferida.
pub(crate) struct Cursor<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Cursor<'a> {
    pub fn novo(d: &'a [u8]) -> Cursor<'a> {
        Cursor { d, p: 0 }
    }

    fn resta(&self) -> usize {
        self.d.len() - self.p
    }

    fn byte(&mut self) -> Result<u8, Erro> {
        let b = *self
            .d
            .get(self.p)
            .ok_or_else(|| estrutura("o cabecalho acaba no meio de um campo"))?;
        self.p += 1;
        Ok(b)
    }

    fn fatia(&mut self, n: u64) -> Result<&'a [u8], Erro> {
        let n = para_usize(n, "campo do cabecalho")?;
        if n > self.resta() {
            return Err(estrutura(
                "campo do cabecalho maior do que o que resta dele",
            ));
        }
        let f = &self.d[self.p..self.p + n];
        self.p += n;
        Ok(f)
    }

    fn u32le(&mut self) -> Result<u32, Erro> {
        let f = self.fatia(4)?;
        Ok(u32::from_le_bytes([f[0], f[1], f[2], f[3]]))
    }

    fn u64le(&mut self) -> Result<u64, Erro> {
        let f = self.fatia(8)?;
        let mut b = [0u8; 8];
        b.copy_from_slice(f);
        Ok(u64::from_le_bytes(b))
    }

    /// O `UINT64` do 7z: o primeiro byte diz, pelos bits altos ligados, quantos
    /// bytes vem depois (`7zFormat.txt`, «Notes about Notation»).
    pub fn numero(&mut self) -> Result<u64, Erro> {
        let primeiro = self.byte()?;
        let mut mascara = 0x80u8;
        let mut valor = 0u64;
        for i in 0..8 {
            if primeiro & mascara == 0 {
                let alto = (primeiro & mascara.wrapping_sub(1)) as u64;
                return Ok(valor | (alto << (8 * i)));
            }
            valor |= (self.byte()? as u64) << (8 * i);
            mascara >>= 1;
        }
        Ok(valor)
    }

    /// Uma contagem: cada item ocupa pelo menos um byte do cabecalho, entao
    /// contagem maior do que o que resta e mentira -- e recusar aqui impede de
    /// reservar memoria por ela.
    fn contagem(&mut self, oque: &'static str) -> Result<usize, Erro> {
        let n = self.numero()?;
        let n = para_usize(n, oque)?;
        if n > self.resta() {
            return Err(Erro::Estrutura(format!(
                "{oque} maior do que o cabecalho comporta"
            )));
        }
        Ok(n)
    }

    fn pular_dado(&mut self) -> Result<(), Erro> {
        let n = self.numero()?;
        self.fatia(n)?;
        Ok(())
    }

    /// `WaitId` do `7zArcIn.c`: pula propriedades desconhecidas ate a esperada.
    fn esperar(&mut self, esperado: u64) -> Result<(), Erro> {
        loop {
            let t = self.numero()?;
            if t == esperado {
                return Ok(());
            }
            if t == id::FIM {
                return Err(Erro::Estrutura(format!(
                    "faltou a propriedade {esperado:#x} do cabecalho"
                )));
            }
            self.pular_dado()?;
        }
    }

    /// Vetor de bits, do bit mais alto de cada byte para o mais baixo.
    fn bits(&mut self, n: usize) -> Result<Vec<bool>, Erro> {
        let bytes = self.fatia(n.div_ceil(8) as u64)?;
        Ok((0..n)
            .map(|i| bytes[i / 8] & (0x80 >> (i % 8)) != 0)
            .collect())
    }

    /// `AllAreDefined` + vetor de bits opcional.
    fn definidos(&mut self, n: usize) -> Result<Vec<bool>, Erro> {
        if self.byte()? != 0 {
            Ok(alloc::vec![true; n])
        } else {
            self.bits(n)
        }
    }

    fn crcs(&mut self, n: usize) -> Result<Vec<Option<u32>>, Erro> {
        let def = self.definidos(n)?;
        def.into_iter()
            .map(|d| if d { self.u32le().map(Some) } else { Ok(None) })
            .collect()
    }
}

// ------------------------------------------------------- a estrutura lida

#[derive(Debug, Clone)]
pub(crate) struct Coder {
    pub id: u64,
    pub props: Vec<u8>,
}

/// Um bloco (o `Folder` do 7z): uma corrente de coders simples, um fluxo
/// empacotado na entrada e o desempacotado na saida.
#[derive(Debug, Clone)]
pub(crate) struct Bloco {
    pub coders: Vec<Coder>,
    /// A ordem em que os coders rodam, do empacotado para o desempacotado.
    pub ordem: Vec<usize>,
    /// O tamanho da saida de cada coder, na ordem em que foram declarados.
    pub tamanhos: Vec<u64>,
    /// O coder cuja saida e a do bloco.
    pub principal: usize,
    pub crc: Option<u32>,
    pub pack_inicio: usize,
    pub pack_tamanho: usize,
    pub pack_crc: Option<u32>,
    /// Os subfluxos: tamanho e CRC de cada entrada dentro do bloco.
    pub sub: Vec<(u64, Option<u32>)>,
}

impl Bloco {
    fn tamanho(&self) -> u64 {
        self.tamanhos[self.principal]
    }

    fn cifrado(&self) -> bool {
        self.coders.iter().any(|c| c.id == ID_7ZAES)
    }
}

/// Le um bloco. So coders simples (uma entrada, uma saida): os complexos do 7z
/// sao o BCJ2, que e legado -- e a recusa sai com o nome dele.
fn ler_bloco(c: &mut Cursor) -> Result<(Vec<Coder>, Vec<usize>, usize), Erro> {
    let n = c.numero()?;
    if n == 0 || n > CODERS_MAX {
        return Err(Erro::Estrutura(format!("bloco com {n} coders")));
    }
    let n = n as usize;
    let mut coders = Vec::with_capacity(n);
    for _ in 0..n {
        let marca = c.byte()?;
        if marca & 0xC0 != 0 {
            return Err(estrutura("coder com metodo alternativo ou bit reservado"));
        }
        let tam_id = (marca & 0x0F) as usize;
        if tam_id > 8 {
            return Err(estrutura("identificador de metodo com mais de 8 bytes"));
        }
        let mut ident = 0u64;
        for &b in c.fatia(tam_id as u64)? {
            ident = (ident << 8) | b as u64;
        }
        if marca & 0x10 != 0 {
            let entradas = c.numero()?;
            let saidas = c.numero()?;
            if entradas != 1 || saidas != 1 {
                conferir_metodo(ident)?;
                return Err(estrutura("coder com mais de um fluxo"));
            }
        }
        let props = if marca & 0x20 != 0 {
            let t = c.numero()?;
            c.fatia(t)?.to_vec()
        } else {
            Vec::new()
        };
        coders.push(Coder { id: ident, props });
    }
    // Ligacoes: a entrada do coder `dentro` recebe a saida do coder `fora`.
    let mut recebe_de: Vec<Option<usize>> = alloc::vec![None; n];
    let mut saida_usada = alloc::vec![false; n];
    for _ in 0..n - 1 {
        let dentro = c.numero()?;
        let fora = c.numero()?;
        if dentro >= n as u64 || fora >= n as u64 {
            return Err(estrutura("ligacao de coder fora do bloco"));
        }
        let (dentro, fora) = (dentro as usize, fora as usize);
        if recebe_de[dentro].is_some() || saida_usada[fora] {
            return Err(estrutura("ligacao de coder repetida"));
        }
        recebe_de[dentro] = Some(fora);
        saida_usada[fora] = true;
    }
    // Com coders simples ha exatamente um fluxo empacotado, e o formato so
    // grava o indice dele quando ha mais de um.
    let principal = saida_usada
        .iter()
        .position(|u| !u)
        .ok_or_else(|| estrutura("bloco sem saida principal"))?;
    // Da saida principal para tras ate o coder que le o empacotado; a ordem de
    // execucao e o inverso. A corrente tem de passar por TODOS -- senao ha
    // ciclo ou coder solto.
    let mut ordem = Vec::with_capacity(n);
    let mut atual = principal;
    loop {
        if ordem.contains(&atual) || ordem.len() == n {
            return Err(estrutura("ciclo nas ligacoes dos coders"));
        }
        ordem.push(atual);
        match recebe_de[atual] {
            Some(anterior) => atual = anterior,
            None => break,
        }
    }
    if ordem.len() != n {
        return Err(estrutura("coder solto no bloco"));
    }
    ordem.reverse();
    Ok((coders, ordem, principal))
}

/// O `StreamsInfo`: empacotados, blocos e subfluxos.
struct Fluxos {
    pack_pos: u64,
    packs: Vec<(u64, Option<u32>)>,
    blocos: Vec<Bloco>,
}

fn ler_fluxos(c: &mut Cursor) -> Result<Fluxos, Erro> {
    let mut f = Fluxos {
        pack_pos: 0,
        packs: Vec::new(),
        blocos: Vec::new(),
    };
    let mut t = c.numero()?;
    if t == id::EMPACOTADOS {
        f.pack_pos = c.numero()?;
        let n = c.contagem("quantidade de fluxos empacotados")?;
        c.esperar(id::TAMANHO)?;
        let mut tamanhos = Vec::with_capacity(n);
        for _ in 0..n {
            tamanhos.push(c.numero()?);
        }
        let mut crcs = alloc::vec![None; n];
        loop {
            let t = c.numero()?;
            if t == id::FIM {
                break;
            }
            if t == id::CRC {
                crcs = c.crcs(n)?;
            } else {
                c.pular_dado()?;
            }
        }
        f.packs = tamanhos.into_iter().zip(crcs).collect();
        t = c.numero()?;
    }
    if t == id::DESEMPACOTADOS {
        c.esperar(id::BLOCO)?;
        let n = c.contagem("quantidade de blocos")?;
        if c.byte()? != 0 {
            return Err(estrutura("blocos guardados fora do cabecalho (External)"));
        }
        for _ in 0..n {
            let (coders, ordem, principal) = ler_bloco(c)?;
            f.blocos.push(Bloco {
                tamanhos: Vec::with_capacity(coders.len()),
                coders,
                ordem,
                principal,
                crc: None,
                pack_inicio: 0,
                pack_tamanho: 0,
                pack_crc: None,
                sub: Vec::new(),
            });
        }
        c.esperar(id::TAMANHOS_DOS_CODERS)?;
        for b in f.blocos.iter_mut() {
            for _ in 0..b.coders.len() {
                b.tamanhos.push(c.numero()?);
            }
        }
        loop {
            let t = c.numero()?;
            if t == id::FIM {
                break;
            }
            if t == id::CRC {
                for (b, crc) in f.blocos.iter_mut().zip(c.crcs(n)?) {
                    b.crc = crc;
                }
            } else {
                c.pular_dado()?;
            }
        }
        t = c.numero()?;
    }
    // Sem `SubStreamsInfo`, cada bloco e uma entrada inteira.
    let mut quantos: Vec<usize> = alloc::vec![1; f.blocos.len()];
    if t == id::SUBFLUXOS {
        loop {
            t = c.numero()?;
            if t == id::QUANTOS_SUBFLUXOS {
                for q in quantos.iter_mut() {
                    *q = c.contagem("quantidade de subfluxos")?;
                }
                continue;
            }
            if t == id::CRC || t == id::TAMANHO || t == id::FIM {
                break;
            }
            c.pular_dado()?;
        }
        let mut tamanhos: Vec<Vec<u64>> = Vec::with_capacity(f.blocos.len());
        if t == id::TAMANHO {
            for (b, &q) in f.blocos.iter().zip(quantos.iter()) {
                let mut v = Vec::with_capacity(q);
                let mut soma = 0u64;
                for _ in 1..q {
                    let s = c.numero()?;
                    soma = soma
                        .checked_add(s)
                        .ok_or_else(|| estrutura("soma dos subfluxos transborda"))?;
                    v.push(s);
                }
                if q > 0 {
                    let ultimo = b
                        .tamanho()
                        .checked_sub(soma)
                        .ok_or_else(|| estrutura("subfluxos somam mais que o bloco"))?;
                    v.push(ultimo);
                }
                tamanhos.push(v);
            }
            t = c.numero()?;
        } else {
            for (b, &q) in f.blocos.iter().zip(quantos.iter()) {
                match q {
                    0 => tamanhos.push(Vec::new()),
                    1 => tamanhos.push(alloc::vec![b.tamanho()]),
                    _ => return Err(estrutura("varios subfluxos sem os tamanhos")),
                }
            }
        }
        // Precisam de CRC aqui os subfluxos cujo bloco nao traz um que sirva.
        let precisam: usize = f
            .blocos
            .iter()
            .zip(quantos.iter())
            .map(|(b, &q)| if q == 1 && b.crc.is_some() { 0 } else { q })
            .sum();
        let mut crcs: Vec<Option<u32>> = alloc::vec![None; precisam];
        loop {
            if t == id::FIM {
                break;
            }
            if t == id::CRC {
                crcs = c.crcs(precisam)?;
            } else {
                c.pular_dado()?;
            }
            t = c.numero()?;
        }
        let mut proximo = crcs.into_iter();
        for ((b, &q), tam) in f.blocos.iter_mut().zip(quantos.iter()).zip(tamanhos) {
            b.sub = if q == 1 && b.crc.is_some() {
                alloc::vec![(tam[0], b.crc)]
            } else {
                tam.into_iter()
                    .map(|s| (s, proximo.next().flatten()))
                    .collect()
            };
        }
        t = c.numero()?;
    } else {
        for b in f.blocos.iter_mut() {
            b.sub = alloc::vec![(b.tamanho(), b.crc)];
        }
    }
    if t != id::FIM {
        return Err(estrutura("propriedade inesperada no fim dos fluxos"));
    }
    Ok(f)
}

/// Amarra cada bloco ao seu fluxo empacotado e confere que todos cabem antes
/// do cabecalho -- o `RangeLimit` do `7zArcIn.c`.
fn posicionar(f: &mut Fluxos, limite: usize) -> Result<(), Erro> {
    if f.packs.len() != f.blocos.len() {
        return Err(estrutura(
            "numero de fluxos empacotados diferente do de blocos",
        ));
    }
    let mut pos = INICIO
        .checked_add(para_usize(f.pack_pos, "posicao dos empacotados")?)
        .ok_or_else(|| estrutura("posicao dos empacotados transborda"))?;
    for (b, &(tam, crc)) in f.blocos.iter_mut().zip(f.packs.iter()) {
        let tam = para_usize(tam, "fluxo empacotado")?;
        let fim = pos
            .checked_add(tam)
            .filter(|&fim| fim <= limite)
            .ok_or_else(|| estrutura("fluxo empacotado aponta para fora do arquivo"))?;
        b.pack_inicio = pos;
        b.pack_tamanho = tam;
        b.pack_crc = crc;
        pos = fim;
    }
    for b in &f.blocos {
        for c in &b.coders {
            conferir_metodo(c.id)?;
        }
        // Cifrado sem CRC: senha errada viraria lixo aceito, calado.
        if b.cifrado() && b.sub.iter().any(|(_, crc)| crc.is_none()) {
            return Err(estrutura(
                "bloco cifrado sem CRC do conteudo: senha errada viraria lixo aceito",
            ));
        }
    }
    Ok(())
}

// ------------------------------------------------------------ decodificar

/// O que veio de um bloco decodificado.
struct Decodificado {
    dados: Vec<u8>,
    cifrado: bool,
    pack_conferido: bool,
}

/// A falha que aparece DEPOIS de decifrar: com o CRC do cifrado conferido, so
/// pode ser a senha; sem ele, o formato nao separa.
fn depois_da_cifra(d_cifrado: bool, pack_conferido: bool, sem_cifra: Erro) -> Erro {
    match (d_cifrado, pack_conferido) {
        (true, true) => Erro::SenhaErrada,
        (true, false) => Erro::SenhaErradaOuCorrompido,
        (false, _) => sem_cifra,
    }
}

pub(crate) struct Contexto<'s> {
    pub senha16: Option<&'s [u8]>,
    pub chaves: &'s mut Chaves,
    pub teto_ciclos: u8,
    pub teto_modelo: u64,
}

fn decodificar(
    dados: &[u8],
    b: &Bloco,
    teto: u64,
    oque: &'static str,
    ctx: &mut Contexto,
) -> Result<Decodificado, Erro> {
    for &t in &b.tamanhos {
        if t > teto {
            return Err(Erro::GrandeDemais {
                oque,
                declarado: t,
                teto,
            });
        }
    }
    let empacotado = &dados[b.pack_inicio..b.pack_inicio + b.pack_tamanho];
    let pack_conferido = match b.pack_crc {
        Some(esperado) if crc32(empacotado) != esperado => {
            return Err(Erro::Corrompido(format!(
                "o CRC do {oque} empacotado nao bate"
            )))
        }
        Some(_) => true,
        None => false,
    };
    let mut buf = empacotado.to_vec();
    let mut cifrado = false;
    for &i in &b.ordem {
        let coder = &b.coders[i];
        let alvo = para_usize(b.tamanhos[i], "saida de coder")?;
        let falha = |f: FalhaLzma, cifrado: bool| {
            depois_da_cifra(
                cifrado,
                pack_conferido,
                Erro::Corrompido(format!("{oque}: {f}")),
            )
        };
        buf = match coder.id {
            ID_COPIA => {
                if buf.len() != alvo {
                    return Err(depois_da_cifra(
                        cifrado,
                        pack_conferido,
                        estrutura("Copy com saida de tamanho diferente da entrada"),
                    ));
                }
                buf
            }
            ID_7ZAES => {
                let senha16 = ctx.senha16.ok_or(Erro::SenhaAusente)?;
                let p = chave::ler_props(&coder.props, ctx.teto_ciclos)?;
                if buf.len() % BLOCO != 0 || alvo > buf.len() {
                    return Err(estrutura("fluxo do 7zAES com tamanho que nao fecha bloco"));
                }
                let k = ctx.chaves.obter(senha16, &p.sal, p.ciclos);
                aes::cbc_decifrar(&Aes256::nova(&k), &p.iv, &mut buf)
                    .ok_or_else(|| estrutura("fluxo do 7zAES com tamanho que nao fecha bloco"))?;
                buf.truncate(alvo);
                cifrado = true;
                buf
            }
            ID_LZMA => {
                // A tabela que o fluxo pede e conferida ANTES de alocar.
                if let Some(bytes) = coder.props.first().and_then(|b| lzma::bytes_do_modelo(*b)) {
                    if bytes > ctx.teto_modelo {
                        return Err(Erro::GrandeDemais {
                            oque: "modelo do LZMA",
                            declarado: bytes,
                            teto: ctx.teto_modelo,
                        });
                    }
                }
                lzma::lzma(&coder.props, &buf, alvo).map_err(|f| falha(f, cifrado))?
            }
            ID_LZMA2 => {
                let [prop] = coder.props[..] else {
                    return Err(estrutura("LZMA2 sem o byte de propriedade"));
                };
                lzma::lzma2(prop, &buf, alvo).map_err(|f| falha(f, cifrado))?
            }
            outro => {
                conferir_metodo(outro)?;
                return Err(estrutura("metodo sem decodificador"));
            }
        };
    }
    Ok(Decodificado {
        dados: buf,
        cifrado,
        pack_conferido,
    })
}

fn conferir_crc(
    dec: &Decodificado,
    dado: &[u8],
    esperado: Option<u32>,
    oque: &str,
) -> Result<(), Erro> {
    match esperado {
        Some(c) if crc32(dado) != c => Err(depois_da_cifra(
            dec.cifrado,
            dec.pack_conferido,
            Erro::Corrompido(format!("o CRC de {oque} nao bate")),
        )),
        _ => Ok(()),
    }
}

// -------------------------------------------------------------- o arquivo

/// Os tetos de quem abre. Tudo o que o arquivo declara e conferido contra eles
/// antes de alocar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limites {
    /// O maior conteudo de uma entrada, descompactado.
    pub entrada: u64,
    /// O maior bloco descompactado -- num arquivo solido, a soma das entradas
    /// dele, porque extrair uma exige decodificar o bloco inteiro.
    pub bloco: u64,
    /// O maior cabecalho descompactado.
    pub cabecalho: u64,
    /// O maior `NumCyclesPower` aceito no 7zAES: e a bomba de CPU.
    pub ciclos: u8,
    /// A maior tabela de probabilidades do LZMA, em bytes. E o unico lugar
    /// onde o FLUXO escolhe quanto o decodificador aloca: o byte lc/lp/pb do
    /// LZMA puro vai de 16 KiB (o 0x5D que o 7-Zip grava) a 6 MiB (lc=8,
    /// lp=4). O LZMA2 nao passa de 28 KiB (lc + lp <= 4). O padrao nao
    /// restringe; num microcontrolador se baixa.
    pub modelo: u64,
}

impl Default for Limites {
    fn default() -> Limites {
        Limites {
            entrada: 256 << 20,
            bloco: 256 << 20,
            cabecalho: 16 << 20,
            ciclos: chave::CICLOS_MAXIMO,
            modelo: 8 << 20,
        }
    }
}

/// Onde o conteudo de uma entrada mora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lugar {
    Vazia,
    NoBloco { bloco: usize, inicio: u64 },
}

/// Uma entrada do arquivo, como a lista mostra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entrada {
    /// O caminho relativo, com `/`, ja conferido por [`conferir_nome`].
    pub nome: String,
    pub pasta: bool,
    pub tamanho: u64,
    /// `FILETIME` do Windows: intervalos de 100 ns desde 1601-01-01 UTC. Ver
    /// [`crate::filetime_para_unix`].
    pub modificado: Option<u64>,
    pub atributos: Option<u32>,
    /// O conteudo passa pelo 7zAES.
    pub cifrada: bool,
    pub crc: Option<u32>,
    pub(crate) lugar: Lugar,
}

/// Um arquivo 7z aberto sobre uma fatia de bytes.
pub struct Arquivo<'a> {
    dados: &'a [u8],
    senha16: Option<Vec<u8>>,
    limites: Limites,
    blocos: Vec<Bloco>,
    entradas: Vec<Entrada>,
    cabecalho_cifrado: bool,
    chaves: Chaves,
}

impl<'a> Arquivo<'a> {
    /// Abre: confere a assinatura e os CRCs do cabecalho, decodifica o
    /// cabecalho (decifrando-o se veio cifrado) e confere cada nome.
    pub fn abrir(
        dados: &'a [u8],
        senha: Option<&str>,
        limites: Limites,
    ) -> Result<Arquivo<'a>, Erro> {
        if dados.len() < ASSINATURA.len() || dados[..6] != ASSINATURA {
            return Err(Erro::NaoE7z);
        }
        if dados.len() < INICIO {
            return Err(estrutura("menor que o cabecalho de inicio"));
        }
        if dados[6] != 0 {
            return Err(Erro::VersaoNaoSuportada(dados[6]));
        }
        let crc_inicio = u32::from_le_bytes([dados[8], dados[9], dados[10], dados[11]]);
        if crc32(&dados[12..INICIO]) != crc_inicio {
            return Err(Erro::Corrompido(String::from(
                "o CRC do cabecalho de inicio nao bate",
            )));
        }
        let mut c = Cursor::novo(&dados[12..INICIO]);
        let desloc = c.u64le()?;
        let tam = c.u64le()?;
        let crc_prox = c.u32le()?;
        let mut arq = Arquivo {
            dados,
            senha16: senha.map(chave::senha_utf16),
            limites,
            blocos: Vec::new(),
            entradas: Vec::new(),
            cabecalho_cifrado: false,
            chaves: Chaves::default(),
        };
        if tam == 0 {
            // Arquivo vazio: e o que o 7-Zip grava sem entrada nenhuma.
            return Ok(arq);
        }
        let inicio_cab = INICIO
            .checked_add(para_usize(desloc, "posicao do cabecalho")?)
            .ok_or_else(|| estrutura("posicao do cabecalho transborda"))?;
        let fim_cab = inicio_cab
            .checked_add(para_usize(tam, "tamanho do cabecalho")?)
            .filter(|&f| f <= dados.len())
            .ok_or_else(|| estrutura("o cabecalho aponta para fora do arquivo: cortado?"))?;
        let cab = &dados[inicio_cab..fim_cab];
        if crc32(cab) != crc_prox {
            return Err(Erro::Corrompido(String::from(
                "o CRC do cabecalho nao bate",
            )));
        }
        let mut c = Cursor::novo(cab);
        let decodificado;
        match c.numero()? {
            id::CABECALHO => {}
            id::CABECALHO_CODIFICADO => {
                let mut f = ler_fluxos(&mut c)?;
                posicionar(&mut f, inicio_cab)?;
                let [b] = &f.blocos[..] else {
                    return Err(estrutura("cabecalho codificado sem exatamente um bloco"));
                };
                let mut ctx = Contexto {
                    senha16: arq.senha16.as_deref(),
                    chaves: &mut arq.chaves,
                    teto_ciclos: limites.ciclos,
                    teto_modelo: limites.modelo,
                };
                let dec = decodificar(dados, b, limites.cabecalho, "cabecalho", &mut ctx)?;
                let crc = b.sub.first().and_then(|s| s.1).or(b.crc);
                conferir_crc(&dec, &dec.dados, crc, "o cabecalho")?;
                arq.cabecalho_cifrado = dec.cifrado;
                decodificado = dec.dados;
                c = Cursor::novo(&decodificado);
                if c.numero()? != id::CABECALHO {
                    return Err(depois_da_cifra(
                        arq.cabecalho_cifrado,
                        true,
                        estrutura("o cabecalho decodificado nao comeca com kHeader"),
                    ));
                }
            }
            _ => {
                return Err(estrutura(
                    "o cabecalho nao comeca com kHeader nem kEncodedHeader",
                ))
            }
        }
        arq.ler_cabecalho(&mut c, inicio_cab)?;
        Ok(arq)
    }

    fn ler_cabecalho(&mut self, c: &mut Cursor, limite: usize) -> Result<(), Erro> {
        let mut t = c.numero()?;
        if t == id::PROPRIEDADES {
            loop {
                if c.numero()? == id::FIM {
                    break;
                }
                c.pular_dado()?;
            }
            t = c.numero()?;
        }
        if t == id::FLUXOS_ADICIONAIS {
            return Err(estrutura("AdditionalStreamsInfo nao suportado"));
        }
        if t == id::FLUXOS {
            let mut f = ler_fluxos(c)?;
            posicionar(&mut f, limite)?;
            self.blocos = f.blocos;
            t = c.numero()?;
        }
        if t == id::FIM {
            return if self.blocos.is_empty() {
                Ok(())
            } else {
                Err(estrutura("fluxos sem a lista de entradas"))
            };
        }
        if t != id::ARQUIVOS {
            return Err(estrutura("propriedade inesperada no cabecalho"));
        }
        self.ler_entradas(c)?;
        loop {
            match c.numero()? {
                id::FIM => return Ok(()),
                _ => c.pular_dado()?,
            }
        }
    }

    fn ler_entradas(&mut self, c: &mut Cursor) -> Result<(), Erro> {
        let n = c.contagem("quantidade de entradas")?;
        let mut vazio: Vec<bool> = alloc::vec![false; n];
        let mut arquivo_vazio: Vec<bool> = Vec::new();
        let mut nomes: Vec<String> = Vec::new();
        let mut modificado: Vec<Option<u64>> = alloc::vec![None; n];
        let mut atributos: Vec<Option<u32>> = alloc::vec![None; n];
        loop {
            let t = c.numero()?;
            if t == id::FIM {
                break;
            }
            let tam = c.numero()?;
            let mut p = Cursor::novo(c.fatia(tam)?);
            match t {
                id::FLUXO_VAZIO => vazio = p.bits(n)?,
                id::ARQUIVO_VAZIO => {
                    arquivo_vazio = p.bits(vazio.iter().filter(|v| **v).count())?
                }
                id::ANTI => {
                    let quantos = vazio.iter().filter(|v| **v).count();
                    if p.bits(quantos)?.iter().any(|a| *a) {
                        return Err(estrutura(
                            "entrada «anti» (apagar na atualizacao) nao suportada",
                        ));
                    }
                }
                id::NOME => {
                    if p.byte()? != 0 {
                        return Err(estrutura("nomes guardados fora do cabecalho (External)"));
                    }
                    let resto = p.fatia(p.resta() as u64)?;
                    if resto.len() % 2 != 0 {
                        return Err(estrutura("nomes com numero impar de bytes"));
                    }
                    let unidades: Vec<u16> = resto
                        .chunks_exact(2)
                        .map(|u| u16::from_le_bytes([u[0], u[1]]))
                        .collect();
                    for pedaco in unidades.split(|u| *u == 0).take(n) {
                        nomes.push(
                            String::from_utf16(pedaco).map_err(|_| {
                                estrutura("nome de entrada que nao e UTF-16 valido")
                            })?,
                        );
                    }
                    if nomes.len() != n || unidades.last() != Some(&0) {
                        return Err(estrutura("lista de nomes nao fecha com as entradas"));
                    }
                }
                id::MODIFICADO => {
                    let def = p.definidos(n)?;
                    if p.byte()? != 0 {
                        return Err(estrutura("datas guardadas fora do cabecalho (External)"));
                    }
                    for (m, d) in modificado.iter_mut().zip(def) {
                        if d {
                            *m = Some(p.u64le()?);
                        }
                    }
                }
                id::ATRIBUTOS => {
                    let def = p.definidos(n)?;
                    if p.byte()? != 0 {
                        return Err(estrutura(
                            "atributos guardados fora do cabecalho (External)",
                        ));
                    }
                    for (a, d) in atributos.iter_mut().zip(def) {
                        if d {
                            *a = Some(p.u32le()?);
                        }
                    }
                }
                // Datas de criacao e acesso, `kDummy`, `kStartPos`, comentario:
                // nada que decida o que se extrai.
                _ => {}
            }
        }
        if nomes.is_empty() && n > 0 {
            nomes = alloc::vec![String::new(); n];
        }
        // Cada entrada com fluxo toma o proximo subfluxo, bloco apos bloco.
        let mut subfluxos = self.blocos.iter().enumerate().flat_map(|(i, b)| {
            let mut inicio = 0u64;
            b.sub.iter().map(move |&(tam, crc)| {
                let r = (i, inicio, tam, crc);
                inicio += tam;
                r
            })
        });
        let mut i_vazio = 0usize;
        let mut entradas = Vec::with_capacity(n);
        for (i, nome) in nomes.into_iter().enumerate() {
            let e = if vazio[i] {
                let e_arquivo = arquivo_vazio.get(i_vazio).copied().unwrap_or(false);
                i_vazio += 1;
                Entrada {
                    nome,
                    pasta: !e_arquivo,
                    tamanho: 0,
                    modificado: modificado[i],
                    atributos: atributos[i],
                    cifrada: false,
                    crc: None,
                    lugar: Lugar::Vazia,
                }
            } else {
                let (bloco, inicio, tamanho, crc) = subfluxos
                    .next()
                    .ok_or_else(|| estrutura("mais entradas com conteudo do que subfluxos"))?;
                Entrada {
                    nome,
                    pasta: false,
                    tamanho,
                    modificado: modificado[i],
                    atributos: atributos[i],
                    cifrada: self.blocos[bloco].cifrado(),
                    crc,
                    lugar: Lugar::NoBloco { bloco, inicio },
                }
            };
            entradas.push(e);
        }
        if subfluxos.next().is_some() {
            return Err(estrutura("subfluxos sobrando depois da ultima entrada"));
        }
        // O nome e conferido por ultimo, e para TODAS: um arquivo com uma
        // entrada perigosa nao abre, em vez de abrir e confiar que cada chamador
        // lembre de pular a entrada ruim.
        for e in entradas.iter_mut() {
            e.nome = conferir_nome(&e.nome)?;
        }
        self.entradas = entradas;
        Ok(())
    }

    /// As entradas, na ordem do arquivo.
    pub fn entradas(&self) -> &[Entrada] {
        &self.entradas
    }

    /// Se o cabecalho veio cifrado (os nomes estavam escondidos).
    pub fn cabecalho_cifrado(&self) -> bool {
        self.cabecalho_cifrado
    }

    fn decodificar_bloco(&mut self, i: usize) -> Result<Decodificado, Erro> {
        let b = &self.blocos[i];
        let mut ctx = Contexto {
            senha16: self.senha16.as_deref(),
            chaves: &mut self.chaves,
            teto_ciclos: self.limites.ciclos,
            teto_modelo: self.limites.modelo,
        };
        decodificar(self.dados, b, self.limites.bloco, "bloco", &mut ctx)
    }

    /// O pedaco do bloco que e a entrada, com o CRC dela ja conferido.
    fn fatia<'d>(dec: &'d Decodificado, e: &Entrada, inicio: u64) -> Result<&'d [u8], Erro> {
        let ini = para_usize(inicio, "posicao da entrada no bloco")?;
        let tam = para_usize(e.tamanho, "tamanho da entrada")?;
        let fim = ini
            .checked_add(tam)
            .filter(|&f| f <= dec.dados.len())
            .ok_or_else(|| estrutura("entrada passa do fim do bloco"))?;
        let dado = &dec.dados[ini..fim];
        conferir_crc(dec, dado, e.crc, "entrada")?;
        Ok(dado)
    }

    /// Extrai o conteudo de UMA entrada. Pasta devolve vazio.
    pub fn extrair(&mut self, indice: usize) -> Result<Vec<u8>, Erro> {
        let e = self
            .entradas
            .get(indice)
            .cloned()
            .ok_or(Erro::EntradaInexistente(indice))?;
        let Lugar::NoBloco { bloco, inicio } = e.lugar else {
            return Ok(Vec::new());
        };
        if e.tamanho > self.limites.entrada {
            return Err(Erro::GrandeDemais {
                oque: "entrada",
                declarado: e.tamanho,
                teto: self.limites.entrada,
            });
        }
        let dec = self.decodificar_bloco(bloco)?;
        // Entrada que e o bloco inteiro -- o `.phz`, e todo arquivo que o
        // PhxZip grava, que nao e solido -- sai sem copia: o pico de memoria
        // fica em um conteudo, e nao em dois. E o que decide no microcontrolador.
        if inicio == 0 && e.tamanho == dec.dados.len() as u64 {
            Self::fatia(&dec, &e, 0)?;
            return Ok(dec.dados);
        }
        Ok(Self::fatia(&dec, &e, inicio)?.to_vec())
    }

    /// Percorre TODAS as entradas na ordem, decodificando cada bloco uma vez
    /// so, e entrega cada uma a `visitar` -- sem juntar tudo na memoria.
    pub fn percorrer<F: FnMut(&Entrada, &[u8])>(&mut self, mut visitar: F) -> Result<(), Erro> {
        for e in &self.entradas {
            if e.tamanho > self.limites.entrada {
                return Err(Erro::GrandeDemais {
                    oque: "entrada",
                    declarado: e.tamanho,
                    teto: self.limites.entrada,
                });
            }
        }
        let mut atual: Option<(usize, Decodificado)> = None;
        for i in 0..self.entradas.len() {
            let e = self.entradas[i].clone();
            match e.lugar {
                Lugar::Vazia => visitar(&e, &[]),
                Lugar::NoBloco { bloco, inicio } => {
                    if atual.as_ref().map(|(b, _)| *b) != Some(bloco) {
                        // Solta o bloco anterior ANTES de decodificar o novo: o
                        // pico de memoria e um bloco, nao dois.
                        drop(atual.take());
                        atual = Some((bloco, self.decodificar_bloco(bloco)?));
                    }
                    let (_, dec) = atual.as_ref().ok_or_else(|| estrutura("bloco perdido"))?;
                    visitar(&e, Self::fatia(dec, &e, inicio)?);
                }
            }
        }
        Ok(())
    }

    /// Todas as entradas com o conteudo, na ordem. Junta tudo na memoria: para
    /// arquivo grande, [`Arquivo::percorrer`].
    pub fn extrair_todas(&mut self) -> Result<Vec<(Entrada, Vec<u8>)>, Erro> {
        let mut v = Vec::with_capacity(self.entradas.len());
        self.percorrer(|e, d| v.push((e.clone(), d.to_vec())))?;
        Ok(v)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn numero_do_7z_nos_limites_de_cada_largura() {
        let casos: [(&[u8], u64); 6] = [
            (&[0x7f], 0x7f),
            (&[0x80, 0x80], 0x80),
            (&[0xbf, 0xff], 0x3fff),
            (&[0xc0, 0x00, 0x40], 0x4000),
            (&[0xfe, 1, 2, 3, 4, 5, 6, 7], 0x07_0605_0403_0201),
            (&[0xff, 1, 2, 3, 4, 5, 6, 7, 8], 0x0807_0605_0403_0201),
        ];
        for (bytes, esperado) in casos {
            assert_eq!(Cursor::novo(bytes).numero().unwrap(), esperado, "{bytes:?}");
        }
        assert!(Cursor::novo(&[0xff, 1, 2]).numero().is_err());
    }

    #[test]
    fn nome_perigoso_e_recusado_em_toda_forma() {
        for ruim in [
            "/etc/passwd",
            "\\Windows\\win.ini",
            "../fora",
            "a/../../fora",
            "a\\..\\..\\fora",
            "..",
            "C:\\Windows\\x",
            "c:x",
            "a\0b",
            "",
            "/",
        ] {
            assert!(
                matches!(conferir_nome(ruim), Err(Erro::NomePerigoso(_))),
                "{ruim:?} passou"
            );
        }
    }

    #[test]
    fn nome_bom_passa_e_sai_com_barra_normal() {
        assert_eq!(conferir_nome("config.json").unwrap(), "config.json");
        assert_eq!(conferir_nome("a\\b\\c.txt").unwrap(), "a/b/c.txt");
        assert_eq!(conferir_nome("pasta/").unwrap(), "pasta");
        assert_eq!(conferir_nome("./a/..b/c..").unwrap(), "./a/..b/c..");
        assert_eq!(
            conferir_nome("nome com : no meio").unwrap(),
            "nome com : no meio"
        );
    }

    #[test]
    fn metodo_legado_sai_com_o_nome() {
        assert_eq!(
            conferir_metodo(0x03_0401),
            Err(Erro::MetodoLegado("PPMd (30401)".into()))
        );
        assert!(
            matches!(conferir_metodo(0x04_0108), Err(Erro::MetodoLegado(n)) if n.contains("Deflate"))
        );
        assert!(
            matches!(conferir_metodo(0x06F1_0101), Err(Erro::MetodoLegado(n)) if n.contains("ZipCrypto"))
        );
        assert!(matches!(
            conferir_metodo(0x7777),
            Err(Erro::MetodoDesconhecido(_))
        ));
        for bom in [ID_COPIA, ID_LZMA, ID_LZMA2, ID_7ZAES] {
            assert!(conferir_metodo(bom).is_ok());
        }
    }
}

//! Leitura de um 7z inteiro em memoria.
//!
//! O arquivo vem como fatia de bytes, sem API de sistema operacional: e o que
//! deixa a mesma leitura rodar no servidor, no terminal e no microcontrolador.
//! Quem tem arquivo em disco le para a memoria antes -- e o teto de memoria e
//! o de `Limites`, conferido antes de cada alocacao que o arquivo pede.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefCell;

use phxhash::crc::crc32;

use crate::aes::Aes256;
use crate::caminho::caminho_seguro;
use crate::chave::ParamAes;
use crate::erro::{para_usize, Erro, Resultado};
use crate::formato::*;
use crate::lzma::dec::decodificar_lzma;
use crate::lzma::decodificar_lzma2_em_fios;

/// Tetos da leitura: o que um arquivo hostil pode pedir de memoria e de laco.
#[derive(Clone, Copy, Debug)]
pub struct Limites {
    /// Soma do que uma pasta (bloco solido) descompacta.
    pub max_pasta: u64,
    /// Quantidade de entradas.
    pub max_entradas: usize,
    /// Tamanho do cabecalho, cru ou descompactado.
    pub max_cabecalho: u64,
    /// Fios da descompactacao: um fluxo LZMA2 gravado em blocos se decodifica
    /// com ate este tanto de fios (so com o recurso `std`). 1, o padrao, e
    /// um fio; o resultado e o mesmo em qualquer numero.
    pub fios: usize,
}

impl Default for Limites {
    /// 1 GiB por pasta, um milhao de entradas, 64 MiB de cabecalho.
    fn default() -> Limites {
        Limites {
            max_pasta: 1 << 30,
            max_entradas: 1_000_000,
            max_cabecalho: 64 << 20,
            fios: 1,
        }
    }
}

const MAX_CODERS: usize = 8;
const MAX_PASTAS: usize = 1 << 20;

#[derive(Clone, Debug)]
struct Coder {
    id: u64,
    props: Vec<u8>,
    entradas: usize,
    saidas: usize,
}

#[derive(Clone, Debug)]
struct Pasta {
    coders: Vec<Coder>,
    ligacoes: Vec<(usize, usize)>,
    empacotados: Vec<usize>,
    tamanhos: Vec<u64>,
    crc: Option<u32>,
}

impl Pasta {
    fn saida_principal(&self) -> Resultado<usize> {
        let total: usize = self.coders.iter().map(|c| c.saidas).sum();
        (0..total)
            .find(|o| !self.ligacoes.iter().any(|(_, s)| s == o))
            .ok_or(Erro::Corrompido("pasta sem saida principal"))
    }

    fn tamanho(&self) -> Resultado<u64> {
        let i = self.saida_principal()?;
        self.tamanhos
            .get(i)
            .copied()
            .ok_or(Erro::Corrompido("pasta sem tamanho"))
    }

    fn cifrada(&self) -> bool {
        self.coders.iter().any(|c| c.id == AES)
    }
}

#[derive(Clone, Debug, Default)]
struct Fluxos {
    pos_empacotado: u64,
    empacotados: Vec<u64>,
    pastas: Vec<Pasta>,
    subfluxos: Vec<usize>,
    tam_sub: Vec<u64>,
    crc_sub: Vec<Option<u32>>,
    /// Indice do primeiro fluxo empacotado de cada pasta.
    primeiro_emp: Vec<usize>,
    /// Soma dos empacotados antes de cada um (`None` se transborda). Com os
    /// dois prefixos, achar o inicio de uma pasta e O(1) -- somar a cada
    /// pasta tornava `testar` quadratico no numero de pastas.
    desl_emp: Vec<Option<u64>>,
}

/// Uma entrada do arquivo: arquivo ou pasta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entrada {
    /// Nome como gravado (use [`Entrada::caminho`] para ir ao disco).
    pub nome: String,
    /// Tamanho descompactado.
    pub tamanho: u64,
    /// E pasta (diretorio).
    pub e_pasta: bool,
    /// Modificacao, em FILETIME, se gravada.
    pub mtime: Option<u64>,
    /// Atributos do Windows; com o bit 0x8000, o modo Unix nos 16 bits altos.
    pub atributos: Option<u32>,
    /// CRC-32 do conteudo, se gravado.
    pub crc: Option<u32>,
    /// O conteudo passa por 7zAES.
    pub cifrada: bool,
    pasta: Option<usize>,
    deslocamento: u64,
}

impl Entrada {
    /// Caminho relativo seguro para extrair (recusa zip-slip).
    pub fn caminho(&self) -> Resultado<String> {
        caminho_seguro(&self.nome)
    }

    /// Modo Unix, se o atributo o trouxer.
    pub fn modo_unix(&self) -> Option<u32> {
        self.atributos.filter(|a| a & 0x8000 != 0).map(|a| a >> 16)
    }

    /// E ligacao simbolica (modo Unix `S_IFLNK`). Quem extrai recusa: a
    /// ligacao apontando para fora da pasta e a outra metade do zip-slip.
    pub fn e_ligacao(&self) -> bool {
        self.modo_unix().is_some_and(|m| m & 0o170000 == 0o120000)
    }
}

/// Um 7z aberto.
pub struct Arquivo7z<'a> {
    bytes: &'a [u8],
    senha: Option<String>,
    limites: Limites,
    fluxos: Fluxos,
    entradas: Vec<Entrada>,
    /// Por pasta, o intervalo de `entradas` onde moram as dela. As entradas
    /// se ligam as pastas em ordem, entao o intervalo e contiguo; sem ele,
    /// conferir uma pasta varria todas as entradas, e `testar`/`extrair_tudo`
    /// saiam quadraticos (pastas x entradas) num 7z nao solido ou hostil.
    faixas: Vec<(usize, usize)>,
    /// Chave derivada da ultima senha usada, com o sal e os ciclos dela: o
    /// cabecalho cifrado e os dados usam os mesmos parametros, e cada
    /// derivacao custa 2^19 voltas de SHA-256.
    chave: RefCell<Option<(Vec<u8>, u8, Aes256)>>,
    cabecalho_cifrado: bool,
}

impl core::fmt::Debug for Arquivo7z<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // A senha nunca sai num Debug.
        f.debug_struct("Arquivo7z")
            .field("entradas", &self.entradas.len())
            .finish()
    }
}

impl Drop for Arquivo7z<'_> {
    fn drop(&mut self) {
        if let Some(s) = self.senha.take() {
            let mut b = s.into_bytes();
            for x in b.iter_mut() {
                // SAFETY: escrita volatil num byte valido do proprio buffer.
                unsafe { core::ptr::write_volatile(x, 0) };
            }
        }
    }
}

impl<'a> Arquivo7z<'a> {
    /// Abre o arquivo: confere assinatura e CRCs do cabecalho, decifra e
    /// descompacta o cabecalho se preciso, e lista as entradas. Nao
    /// descompacta dado nenhum ainda.
    pub fn abrir(
        bytes: &'a [u8],
        senha: Option<&str>,
        limites: Limites,
    ) -> Resultado<Arquivo7z<'a>> {
        let mut a = Arquivo7z {
            bytes,
            senha: senha.map(String::from),
            limites,
            fluxos: Fluxos::default(),
            entradas: Vec::new(),
            faixas: Vec::new(),
            chave: RefCell::new(None),
            cabecalho_cifrado: false,
        };
        if bytes.len() < CABECALHO_INICIAL || bytes[..6] != ASSINATURA {
            return Err(Erro::NaoE7z);
        }
        if bytes[6] != 0 {
            return Err(Erro::VersaoDesconhecida(bytes[6], bytes[7]));
        }
        let mut b = Bytes::novo(&bytes[8..32]);
        let crc_inicial = b.u32()?;
        if crc32(&bytes[12..32]) != crc_inicial {
            return Err(Erro::CrcNaoBate("cabecalho de assinatura"));
        }
        let desl = b.u64()?;
        let tam = b.u64()?;
        let crc = b.u32()?;
        if tam == 0 {
            return Ok(a);
        }
        if tam > limites.max_cabecalho {
            return Err(Erro::Teto("cabecalho maior que o teto"));
        }
        let ini = (CABECALHO_INICIAL as u64)
            .checked_add(desl)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or(Erro::Corrompido("deslocamento do cabecalho"))?;
        let fim = ini
            .checked_add(para_usize(tam)?)
            .ok_or(Erro::Corrompido("tamanho do cabecalho"))?;
        let cab = bytes
            .get(ini..fim)
            .ok_or(Erro::Corrompido("cabecalho fora do arquivo"))?;
        if crc32(cab) != crc {
            return Err(Erro::CrcNaoBate("cabecalho"));
        }

        let mut cab: Vec<u8> = cab.to_vec();
        // Cabecalho codificado: o cabecalho verdadeiro e uma pasta. Mais de
        // quatro camadas e arquivo montado para prender a leitura.
        for _ in 0..4 {
            let mut r = Bytes::novo(&cab);
            match r.numero()? {
                K_CABECALHO => {
                    a.ler_cabecalho(&mut r)?;
                    return Ok(a);
                }
                K_CABECALHO_CODIFICADO => {
                    let f = ler_fluxos(&mut r, &limites)?;
                    let pasta = f
                        .pastas
                        .first()
                        .ok_or(Erro::Corrompido("cabecalho codificado sem pasta"))?;
                    if pasta.cifrada() {
                        a.cabecalho_cifrado = true;
                    }
                    if pasta.tamanho()? > limites.max_cabecalho {
                        return Err(Erro::Teto("cabecalho maior que o teto"));
                    }
                    let d = a.decodificar(&f, 0)?;
                    if let Some(c) = pasta.crc {
                        if crc32(&d) != c {
                            return Err(if pasta.cifrada() {
                                Erro::SenhaErradaOuCorrompido
                            } else {
                                Erro::CrcNaoBate("cabecalho codificado")
                            });
                        }
                    }
                    cab = d;
                }
                _ => return Err(Erro::Corrompido("cabecalho nao comeca por Header")),
            }
        }
        Err(Erro::Corrompido("camadas demais de cabecalho codificado"))
    }

    /// As entradas, na ordem do arquivo.
    pub fn entradas(&self) -> &[Entrada] {
        &self.entradas
    }

    /// Os nomes estavam cifrados (o `-mhe` do 7-Zip).
    pub fn cabecalho_cifrado(&self) -> bool {
        self.cabecalho_cifrado
    }

    fn ler_cabecalho(&mut self, r: &mut Bytes) -> Resultado<()> {
        let mut t = r.numero()?;
        if t == K_PROPRIEDADES_DO_ARQUIVO {
            loop {
                let p = r.numero()?;
                if p == K_FIM {
                    break;
                }
                let n = r.quantos(r_resto(r))?;
                r.fatia(n)?;
            }
            t = r.numero()?;
        }
        if t == K_FLUXOS_ADICIONAIS {
            return Err(Erro::NaoSuportado("fluxos adicionais no cabecalho"));
        }
        if t == K_FLUXOS_PRINCIPAIS {
            self.fluxos = ler_fluxos(r, &self.limites)?;
            t = r.numero()?;
        }
        let mut entradas = Vec::new();
        if t == K_ARQUIVOS {
            entradas = self.ler_arquivos(r)?;
            t = r.numero()?;
        }
        if t != K_FIM {
            return Err(Erro::Corrompido("cabecalho sem fim"));
        }
        let mut faixas = vec![(0usize, 0usize); self.fluxos.pastas.len()];
        for (k, e) in entradas.iter().enumerate() {
            if let Some(p) = e.pasta {
                if faixas[p].1 == 0 {
                    faixas[p].0 = k;
                }
                faixas[p].1 = k + 1;
            }
        }
        self.faixas = faixas;
        self.entradas = entradas;
        Ok(())
    }

    fn ler_arquivos(&self, r: &mut Bytes) -> Resultado<Vec<Entrada>> {
        let n = r.quantos(self.limites.max_entradas)?;
        let mut vazio = vec![false; n];
        let mut arquivo_vazio: Vec<bool> = Vec::new();
        let mut nomes: Vec<String> = Vec::new();
        let mut mtimes: Vec<Option<u64>> = vec![None; n];
        let mut atributos: Vec<Option<u32>> = vec![None; n];
        loop {
            let tipo = r.numero()?;
            if tipo == K_FIM {
                break;
            }
            let tam = r.quantos(r_resto(r))?;
            let dados = r.fatia(tam)?;
            let mut p = Bytes::novo(dados);
            match tipo {
                K_FLUXO_VAZIO => vazio = p.bits(n)?,
                K_ARQUIVO_VAZIO => arquivo_vazio = p.bits(vazio.iter().filter(|x| **x).count())?,
                K_NOME => {
                    if p.byte()? != 0 {
                        return Err(Erro::NaoSuportado("nomes fora do cabecalho"));
                    }
                    let resto = &dados[1..];
                    let unidades = resto
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]));
                    let mut atual: Vec<u16> = Vec::new();
                    for u in unidades {
                        if u == 0 {
                            nomes.push(
                                char::decode_utf16(atual.drain(..))
                                    .map(|c| c.unwrap_or('\u{FFFD}'))
                                    .collect(),
                            );
                        } else {
                            atual.push(u);
                        }
                    }
                    if nomes.len() != n {
                        return Err(Erro::Corrompido("quantidade de nomes"));
                    }
                }
                K_MTIME => {
                    let def = p.bits_ou_todos(n)?;
                    if p.byte()? != 0 {
                        return Err(Erro::NaoSuportado("datas fora do cabecalho"));
                    }
                    for (i, d) in def.iter().enumerate() {
                        if *d {
                            mtimes[i] = Some(p.u64()?);
                        }
                    }
                }
                K_ATRIBUTOS => {
                    let def = p.bits_ou_todos(n)?;
                    if p.byte()? != 0 {
                        return Err(Erro::NaoSuportado("atributos fora do cabecalho"));
                    }
                    for (i, d) in def.iter().enumerate() {
                        if *d {
                            atributos[i] = Some(p.u32()?);
                        }
                    }
                }
                // CTime, ATime, Anti, StartPos, Dummy e o que vier depois:
                // o tamanho veio, entao pular e seguro.
                _ => {}
            }
        }
        if nomes.len() != n {
            if n == 0 {
                return Ok(Vec::new());
            }
            return Err(Erro::Corrompido("arquivo sem nomes"));
        }

        // Liga cada entrada nao vazia ao seu subfluxo, pasta a pasta.
        let f = &self.fluxos;
        let mut pasta = 0usize;
        let mut no_sub = 0usize;
        let mut sub = 0usize;
        let mut desl = 0u64;
        let mut i_vazio = 0usize;
        let mut saida = Vec::with_capacity(n);
        for (i, nome) in nomes.into_iter().enumerate() {
            let mut e = Entrada {
                nome,
                tamanho: 0,
                e_pasta: false,
                mtime: mtimes[i],
                atributos: atributos[i],
                crc: None,
                cifrada: false,
                pasta: None,
                deslocamento: 0,
            };
            if vazio[i] {
                let e_arquivo = arquivo_vazio.get(i_vazio).copied().unwrap_or(false);
                i_vazio += 1;
                e.e_pasta = !e_arquivo;
            } else {
                while pasta < f.pastas.len() && no_sub >= f.subfluxos[pasta] {
                    pasta += 1;
                    no_sub = 0;
                    desl = 0;
                }
                if pasta >= f.pastas.len() {
                    return Err(Erro::Corrompido("mais arquivos que subfluxos"));
                }
                e.tamanho = *f
                    .tam_sub
                    .get(sub)
                    .ok_or(Erro::Corrompido("subfluxo sem tamanho"))?;
                e.crc = f.crc_sub.get(sub).copied().flatten();
                e.cifrada = f.pastas[pasta].cifrada();
                e.pasta = Some(pasta);
                e.deslocamento = desl;
                desl += e.tamanho;
                no_sub += 1;
                sub += 1;
            }
            if let Some(a) = e.atributos {
                if a & 0x10 != 0 {
                    e.e_pasta = true;
                }
            }
            saida.push(e);
        }
        Ok(saida)
    }

    /// Decodifica a pasta `i` de `f` inteira.
    fn decodificar(&self, f: &Fluxos, i: usize) -> Resultado<Vec<u8>> {
        let p = f
            .pastas
            .get(i)
            .ok_or(Erro::Corrompido("pasta inexistente"))?;
        let cifrada = p.cifrada();
        let cifra = |pa: &ParamAes| -> Resultado<Aes256> {
            let senha = self.senha.as_deref().ok_or(Erro::SenhaNecessaria)?;
            let mut c = self.chave.borrow_mut();
            if let Some((sal, ciclos, aes)) = c.as_ref() {
                if *sal == pa.sal && *ciclos == pa.ciclos {
                    return Ok(aes.clone());
                }
            }
            let aes = pa.cifra(senha)?;
            *c = Some((pa.sal.clone(), pa.ciclos, aes.clone()));
            Ok(aes)
        };
        match decodificar_pasta(self.bytes, f, i, &cifra, &self.limites) {
            Err(Erro::Corrompido(_)) | Err(Erro::CrcNaoBate(_)) if cifrada => {
                Err(Erro::SenhaErradaOuCorrompido)
            }
            r => r,
        }
    }

    /// Descompacta a pasta inteira e confere o CRC de cada subfluxo dela.
    fn pasta_conferida(&self, i: usize) -> Resultado<Vec<u8>> {
        let d = self.decodificar(&self.fluxos, i)?;
        let cifrada = self.fluxos.pastas[i].cifrada();
        let (de, ate) = self.faixas.get(i).copied().unwrap_or((0, 0));
        for e in self.entradas[de..ate].iter().filter(|e| e.pasta == Some(i)) {
            if let Some(c) = e.crc {
                let a = para_usize(e.deslocamento)?;
                let b = a + para_usize(e.tamanho)?;
                let fatia = d
                    .get(a..b)
                    .ok_or(Erro::Corrompido("subfluxo alem da pasta"))?;
                if crc32(fatia) != c {
                    return Err(if cifrada {
                        Erro::SenhaErradaOuCorrompido
                    } else {
                        Erro::CrcNaoBate("conteudo")
                    });
                }
            }
        }
        Ok(d)
    }

    /// Conteudo de uma entrada, conferido pelo CRC. Descompacta a pasta
    /// solida inteira: para extrair muitas, [`Arquivo7z::extrair_tudo`].
    pub fn extrair(&self, indice: usize) -> Resultado<Vec<u8>> {
        let e = self
            .entradas
            .get(indice)
            .ok_or(Erro::Uso("indice de entrada inexistente"))?;
        let Some(p) = e.pasta else {
            return Ok(Vec::new());
        };
        let d = self.pasta_conferida(p)?;
        let a = para_usize(e.deslocamento)?;
        Ok(d[a..a + para_usize(e.tamanho)?].to_vec())
    }

    /// Todas as entradas, cada pasta descompactada uma vez so. O `visitar`
    /// recebe a entrada e o conteudo (vazio para pasta e arquivo vazio) --
    /// por visita e nao por `Vec` de tudo, para nao segurar duas copias.
    pub fn extrair_tudo<F>(&self, mut visitar: F) -> Resultado<()>
    where
        F: FnMut(&Entrada, &[u8]) -> Resultado<()>,
    {
        let mut atual: Option<(usize, Vec<u8>)> = None;
        for e in &self.entradas {
            match e.pasta {
                None => visitar(e, &[])?,
                Some(p) => {
                    if atual.as_ref().map(|(q, _)| *q) != Some(p) {
                        atual = Some((p, self.pasta_conferida(p)?));
                    }
                    let d = &atual.as_ref().map(|(_, d)| d).ok_or(Erro::Uso("pasta"))?;
                    let a = para_usize(e.deslocamento)?;
                    visitar(e, &d[a..a + para_usize(e.tamanho)?])?;
                }
            }
        }
        Ok(())
    }

    /// Descompacta tudo e confere todos os CRCs, sem entregar nada.
    pub fn testar(&self) -> Resultado<()> {
        for i in 0..self.fluxos.pastas.len() {
            self.pasta_conferida(i)?;
        }
        Ok(())
    }

    /// Descricao dos metodos da pasta da entrada, como o `7z l -slt` mostra.
    pub fn metodos(&self, indice: usize) -> Vec<&'static str> {
        let Some(p) = self.entradas.get(indice).and_then(|e| e.pasta) else {
            return Vec::new();
        };
        self.fluxos.pastas[p]
            .coders
            .iter()
            .map(|c| match c.id {
                COPY => "Copy",
                LZMA => "LZMA",
                LZMA2 => "LZMA2",
                AES => "7zAES",
                x => nome_recusado(x),
            })
            .collect()
    }
}

fn r_resto(r: &Bytes) -> usize {
    // Nenhum tamanho dentro do cabecalho passa do que resta dele.
    usize::MAX - r.pos
}

fn ler_digestos(r: &mut Bytes, n: usize) -> Resultado<Vec<Option<u32>>> {
    let def = r.bits_ou_todos(n)?;
    let mut v = Vec::with_capacity(n);
    for d in def {
        v.push(if d { Some(r.u32()?) } else { None });
    }
    Ok(v)
}

fn ler_pasta(r: &mut Bytes) -> Resultado<Pasta> {
    let n = r.quantos(MAX_CODERS)?;
    if n == 0 {
        return Err(Erro::Corrompido("pasta sem coder"));
    }
    let mut coders = Vec::with_capacity(n);
    for _ in 0..n {
        let flag = r.byte()?;
        if flag & 0x80 != 0 {
            return Err(Erro::NaoSuportado("metodos alternativos no coder"));
        }
        let tam_id = (flag & 0x0F) as usize;
        if tam_id > 8 {
            return Err(Erro::Corrompido("identificador de metodo longo demais"));
        }
        let mut id = 0u64;
        for b in r.fatia(tam_id)? {
            id = (id << 8) | *b as u64;
        }
        let (entradas, saidas) = if flag & 0x10 != 0 {
            (r.quantos(MAX_CODERS)?, r.quantos(MAX_CODERS)?)
        } else {
            (1, 1)
        };
        let props = if flag & 0x20 != 0 {
            let t = r.quantos(1 << 16)?;
            r.fatia(t)?.to_vec()
        } else {
            Vec::new()
        };
        coders.push(Coder {
            id,
            props,
            entradas,
            saidas,
        });
    }
    let total_saidas: usize = coders.iter().map(|c| c.saidas).sum();
    let total_entradas: usize = coders.iter().map(|c| c.entradas).sum();
    if total_saidas == 0 {
        return Err(Erro::Corrompido("pasta sem saida"));
    }
    // `quantos(teto)` aceita o proprio teto; indice de fluxo nao pode: um
    // indice igual ao total apontaria um coder que nao existe, e a corrente
    // de `decodificar_pasta` indexaria fora do vetor (panico, e nao erro).
    let indice = |r: &mut Bytes, total: usize| -> Resultado<usize> {
        let i = r.quantos(total)?;
        if i >= total {
            return Err(Erro::Corrompido("indice de fluxo fora da pasta"));
        }
        Ok(i)
    };
    let mut ligacoes = Vec::new();
    for _ in 0..total_saidas - 1 {
        ligacoes.push((indice(r, total_entradas)?, indice(r, total_saidas)?));
    }
    let n_emp = total_entradas
        .checked_sub(ligacoes.len())
        .filter(|n| *n >= 1)
        .ok_or(Erro::Corrompido("fluxos empacotados da pasta"))?;
    let empacotados = if n_emp == 1 {
        let i = (0..total_entradas)
            .find(|i| !ligacoes.iter().any(|(e, _)| e == i))
            .ok_or(Erro::Corrompido("pasta sem fluxo empacotado"))?;
        vec![i]
    } else {
        let mut v = Vec::new();
        for _ in 0..n_emp {
            v.push(indice(r, total_entradas)?);
        }
        v
    };
    Ok(Pasta {
        coders,
        ligacoes,
        empacotados,
        tamanhos: Vec::new(),
        crc: None,
    })
}

fn ler_fluxos(r: &mut Bytes, lim: &Limites) -> Resultado<Fluxos> {
    let mut f = Fluxos::default();
    let mut t = r.numero()?;
    if t == K_EMPACOTADOS {
        f.pos_empacotado = r.numero()?;
        let n = r.quantos(MAX_PASTAS)?;
        loop {
            let s = r.numero()?;
            match s {
                K_FIM => break,
                K_TAMANHO => {
                    for _ in 0..n {
                        f.empacotados.push(r.numero()?);
                    }
                }
                K_CRC => {
                    ler_digestos(r, n)?;
                }
                _ => return Err(Erro::Corrompido("propriedade desconhecida em PackInfo")),
            }
        }
        t = r.numero()?;
    }
    if t == K_DESEMPACOTADOS {
        if r.numero()? != K_PASTA {
            return Err(Erro::Corrompido("UnPackInfo sem Folder"));
        }
        let n = r.quantos(MAX_PASTAS)?;
        if r.byte()? != 0 {
            return Err(Erro::NaoSuportado("pastas fora do cabecalho"));
        }
        for _ in 0..n {
            f.pastas.push(ler_pasta(r)?);
        }
        if r.numero()? != K_TAMANHOS_DOS_CODERS {
            return Err(Erro::Corrompido("UnPackInfo sem CodersUnPackSize"));
        }
        for p in f.pastas.iter_mut() {
            let saidas: usize = p.coders.iter().map(|c| c.saidas).sum();
            for _ in 0..saidas {
                p.tamanhos.push(r.numero()?);
            }
        }
        loop {
            let s = r.numero()?;
            match s {
                K_FIM => break,
                K_CRC => {
                    let d = ler_digestos(r, n)?;
                    for (p, c) in f.pastas.iter_mut().zip(d) {
                        p.crc = c;
                    }
                }
                _ => return Err(Erro::Corrompido("propriedade desconhecida em UnPackInfo")),
            }
        }
        t = r.numero()?;
    }
    // Sem SubStreamsInfo: um subfluxo por pasta, com o CRC da pasta.
    f.subfluxos = vec![1; f.pastas.len()];
    let mut tam_definidos = false;
    if t == K_SUBFLUXOS {
        let mut s = r.numero()?;
        if s == K_QUANTOS_SUBFLUXOS {
            for q in f.subfluxos.iter_mut() {
                *q = r.quantos(lim.max_entradas)?;
            }
            s = r.numero()?;
        }
        if s == K_TAMANHO {
            for (i, p) in f.pastas.iter().enumerate() {
                let q = f.subfluxos[i];
                if q == 0 {
                    continue;
                }
                let total = p.tamanho()?;
                let mut soma = 0u64;
                for _ in 0..q - 1 {
                    let v = r.numero()?;
                    soma = soma
                        .checked_add(v)
                        .ok_or(Erro::Corrompido("tamanhos de subfluxo"))?;
                    f.tam_sub.push(v);
                }
                f.tam_sub.push(
                    total
                        .checked_sub(soma)
                        .ok_or(Erro::Corrompido("subfluxos maiores que a pasta"))?,
                );
            }
            tam_definidos = true;
            s = r.numero()?;
        }
        if !tam_definidos {
            preencher_tamanhos(&mut f)?;
            tam_definidos = true;
        }
        loop {
            match s {
                K_FIM => break,
                K_CRC => {
                    let mut faltam = 0usize;
                    for (i, p) in f.pastas.iter().enumerate() {
                        let q = f.subfluxos[i];
                        if !(q == 1 && p.crc.is_some()) {
                            faltam += q;
                        }
                    }
                    let d = ler_digestos(r, faltam)?;
                    let mut it = d.into_iter();
                    f.crc_sub.clear();
                    for (i, p) in f.pastas.iter().enumerate() {
                        let q = f.subfluxos[i];
                        if q == 1 && p.crc.is_some() {
                            f.crc_sub.push(p.crc);
                        } else {
                            for _ in 0..q {
                                f.crc_sub.push(it.next().flatten());
                            }
                        }
                    }
                }
                _ => {
                    return Err(Erro::Corrompido(
                        "propriedade desconhecida em SubStreamsInfo",
                    ))
                }
            }
            s = r.numero()?;
        }
        t = r.numero()?;
    }
    if !tam_definidos {
        preencher_tamanhos(&mut f)?;
    }
    if f.crc_sub.is_empty() {
        for (i, p) in f.pastas.iter().enumerate() {
            for _ in 0..f.subfluxos[i] {
                f.crc_sub
                    .push(if f.subfluxos[i] == 1 { p.crc } else { None });
            }
        }
    }
    if t != K_FIM {
        return Err(Erro::Corrompido("StreamsInfo sem fim"));
    }
    let mut n = 0usize;
    for p in &f.pastas {
        f.primeiro_emp.push(n);
        n = n.saturating_add(p.empacotados.len());
    }
    let mut soma = Some(0u64);
    for t in &f.empacotados {
        f.desl_emp.push(soma);
        soma = soma.and_then(|s| s.checked_add(*t));
    }
    f.desl_emp.push(soma);
    Ok(f)
}

fn preencher_tamanhos(f: &mut Fluxos) -> Resultado<()> {
    f.tam_sub.clear();
    for (i, p) in f.pastas.iter().enumerate() {
        match f.subfluxos[i] {
            0 => {}
            1 => f.tam_sub.push(p.tamanho()?),
            _ => return Err(Erro::Corrompido("varios subfluxos sem tamanhos")),
        }
    }
    Ok(())
}

/// Descompacta a pasta `i`: segue a corrente de coders da entrada empacotada
/// ate a saida principal.
fn decodificar_pasta(
    bytes: &[u8],
    f: &Fluxos,
    i: usize,
    cifra: &dyn Fn(&ParamAes) -> Resultado<Aes256>,
    lim: &Limites,
) -> Resultado<Vec<u8>> {
    let p = &f.pastas[i];
    for c in &p.coders {
        if !matches!(c.id, COPY | LZMA | LZMA2 | AES) {
            return Err(Erro::MetodoRecusado {
                id: c.id,
                nome: nome_recusado(c.id),
            });
        }
        if c.entradas != 1 || c.saidas != 1 {
            return Err(Erro::NaoSuportado("coder com varios fluxos"));
        }
    }
    if p.empacotados.len() != 1 {
        return Err(Erro::NaoSuportado("pasta com varios fluxos empacotados"));
    }
    for t in &p.tamanhos {
        if *t > lim.max_pasta {
            return Err(Erro::Teto("pasta descompacta mais que o teto"));
        }
    }
    // Onde comeca o fluxo empacotado desta pasta.
    let antes = *f
        .primeiro_emp
        .get(i)
        .ok_or(Erro::Corrompido("fluxos empacotados"))?;
    let ini = f
        .desl_emp
        .get(antes)
        .copied()
        .ok_or(Erro::Corrompido("fluxos empacotados"))?
        .and_then(|d| {
            (CABECALHO_INICIAL as u64)
                .checked_add(f.pos_empacotado)?
                .checked_add(d)
        })
        .ok_or(Erro::Corrompido("deslocamento empacotado"))?;
    let tam = *f
        .empacotados
        .get(antes)
        .ok_or(Erro::Corrompido("tamanho empacotado ausente"))?;
    let a = para_usize(ini)?;
    let b = a
        .checked_add(para_usize(tam)?)
        .ok_or(Erro::Corrompido("tamanho empacotado"))?;
    let emp = bytes
        .get(a..b)
        .ok_or(Erro::Corrompido("fluxo empacotado fora do arquivo"))?;

    let principal = p.saida_principal()?;
    let mut k = p.empacotados[0];
    let mut dado: Vec<u8> = emp.to_vec();
    for _ in 0..p.coders.len() {
        let c = &p.coders[k];
        let n = para_usize(p.tamanhos[k])?;
        dado = match c.id {
            COPY => {
                if dado.len() < n {
                    return Err(Erro::Corrompido("Copy menor que o declarado"));
                }
                dado.truncate(n);
                dado
            }
            LZMA => {
                let mut s = Vec::new();
                s.try_reserve_exact(n)
                    .map_err(|_| Erro::Teto("memoria para descompactar"))?;
                decodificar_lzma(&c.props, &dado, n, &mut s)?;
                s
            }
            LZMA2 => {
                let pr = *c
                    .props
                    .first()
                    .ok_or(Erro::Corrompido("LZMA2 sem propriedade"))?;
                let mut s = Vec::new();
                s.try_reserve_exact(n)
                    .map_err(|_| Erro::Teto("memoria para descompactar"))?;
                decodificar_lzma2_em_fios(pr, &dado, n, &mut s, lim.fios)?;
                s
            }
            _ => {
                let pa = ParamAes::ler(&c.props)?;
                let aes = cifra(&pa)?;
                if dado.len() < n {
                    return Err(Erro::Corrompido("7zAES menor que o declarado"));
                }
                aes.decifrar_cbc(&pa.iv, &mut dado);
                dado.truncate(n);
                dado
            }
        };
        if k == principal {
            return Ok(dado);
        }
        k = p
            .ligacoes
            .iter()
            .find(|(_, s)| *s == k)
            .map(|(e, _)| *e)
            .ok_or(Erro::Corrompido("corrente de coders quebrada"))?;
    }
    Err(Erro::Corrompido("corrente de coders em ciclo"))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ligacao_que_aponta_coder_inexistente_e_recusada_sem_panico() {
        // Dois coders Copy simples; a ligacao diz entrada 2 (so ha 0 e 1).
        // Aceita, a corrente de `decodificar_pasta` indexava `coders[2]`.
        let mut r = Bytes::novo(&[0x02, 0x00, 0x00, 0x02, 0x00]);
        assert_eq!(
            ler_pasta(&mut r).unwrap_err(),
            Erro::Corrompido("indice de fluxo fora da pasta")
        );
        // A mesma pasta com a ligacao certa continua valendo.
        let mut r = Bytes::novo(&[0x02, 0x00, 0x00, 0x01, 0x00]);
        assert_eq!(ler_pasta(&mut r).unwrap().empacotados, vec![0]);
    }
}

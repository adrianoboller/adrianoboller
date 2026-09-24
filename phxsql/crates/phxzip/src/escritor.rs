//! Escrita de um 7z: um bloco solido com todos os arquivos, LZMA2, e 7zAES
//! quando ha senha -- nos dados e, por padrao, tambem nos nomes.
//!
//! # Por que os nomes vao cifrados por padrao
//!
//! Sem cifrar o cabecalho, qualquer um lista o arquivo sem a senha e le o
//! nome, o tamanho e a data de cada entrada -- e `senhas-da-producao.txt`
//! vaza so pelo nome. O 7-Zip deixa isso desligado por compatibilidade com o
//! `.zip`; aqui nao ha legado a proteger (pedido 454), entao nasce ligado, e
//! quem quiser nomes visiveis pede.

use alloc::string::String;
use alloc::vec::Vec;

use phxhash::crc::crc32;

use crate::aes::{Aes256, BLOCO};
use crate::caminho::caminho_seguro;
use crate::chave::{derivar, ParamAes, CICLOS_PADRAO};
use crate::erro::{Erro, Resultado};
use crate::formato::*;
use crate::lzma::{
    blocos_lzma2, codificar_bloco_lzma2, codificar_lzma2, juntar_blocos_lzma2, Nivel,
};

/// Opcoes de gravacao.
#[derive(Clone)]
pub struct Opcoes {
    /// 0 grava sem comprimir (Copy); 1 a 9 comprime em LZMA2.
    pub nivel: u8,
    /// Senha do 7zAES. `None` grava em claro.
    pub senha: Option<String>,
    /// Cifra tambem o cabecalho (nomes, tamanhos, datas). So vale com senha.
    pub cifrar_nomes: bool,
    /// Semente dos vetores iniciais do AES: 32 bytes imprevisiveis que quem
    /// chama fornece. A biblioteca nao sorteia porque em microcontrolador nao
    /// ha fonte de acaso comum -- e um IV previsivel nao pode nascer calado.
    pub acaso: [u8; 32],
    /// Quantos fios compactam. Com 1 (o padrao), o dado vira UM bloco LZMA2,
    /// como sempre foi. Com mais de 1, o dado se corta em blocos de `bloco`
    /// bytes que recomecam o dicionario e se compactam em paralelo (so com o
    /// recurso `std`; sem ele, os mesmos blocos saem num fio so, com os mesmos
    /// bytes). O arquivo continua 7z padrao.
    pub fios: usize,
    /// Tamanho do bloco quando `fios > 1`; 0 escolhe pelo nivel
    /// ([`bloco_padrao`]). Os bytes do arquivo dependem daqui, e NAO de
    /// quantos fios trabalharam.
    pub bloco: usize,
}

/// Quantos fios a compactacao usa quando ninguem diz: os nucleos que o
/// sistema oferece. UM lugar so -- o terminal e a porta web perguntam aqui,
/// para os dois nunca divergirem sobre o que «automatico» quer dizer.
#[cfg(feature = "std")]
pub fn fios_padrao() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// O bloco que `bloco: 0` escolhe: um quarto do dicionario do nivel, com
/// piso de 1 MiB e teto de 8 MiB -- nivel 5 corta em 4 MiB, do 7 ao 9 em 8.
/// Medido em 21 MB e 4 nucleos (`docs/PHXZIP.md` §3d): 4 MiB compacta 3,9x
/// mais rapido que um bloco so, por +3,7% de tamanho; o corte do 7-Zip (4x o
/// dicionario) nao corta nada abaixo de 64 MiB, e o dobro do dicionario
/// (8 MiB no nivel 5) ficava mais lento que o proprio 7-Zip com 4 fios.
/// Dado menor que um bloco sai com os MESMOS bytes de um fio so.
pub fn bloco_padrao(nivel: u8) -> usize {
    let dic = Nivel::de(nivel.max(1)).dicionario as usize;
    (dic / 4).clamp(1 << 20, 8 << 20)
}

impl core::fmt::Debug for Opcoes {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Opcoes")
            .field("nivel", &self.nivel)
            .field("senha", &self.senha.as_ref().map(|_| "<oculta>"))
            .field("cifrar_nomes", &self.cifrar_nomes)
            .field("fios", &self.fios)
            .field("bloco", &self.bloco)
            .finish()
    }
}

impl Default for Opcoes {
    fn default() -> Opcoes {
        Opcoes {
            nivel: 5,
            senha: None,
            cifrar_nomes: true,
            acaso: [0; 32],
            fios: 1,
            bloco: 0,
        }
    }
}

struct Item {
    nome: String,
    dados: Option<Vec<u8>>,
    mtime: Option<u64>,
    atributos: Option<u32>,
}

/// Monta um 7z em memoria.
pub struct Escritor {
    opcoes: Opcoes,
    itens: Vec<Item>,
}

impl Escritor {
    /// Escritor vazio.
    pub fn novo(opcoes: Opcoes) -> Escritor {
        Escritor {
            opcoes,
            itens: Vec::new(),
        }
    }

    fn conferir_nome(&self, nome: &str) -> Resultado<String> {
        let n = caminho_seguro(nome)?;
        if self.itens.iter().any(|i| i.nome == n) {
            return Err(Erro::Uso("nome repetido no arquivo"));
        }
        Ok(n)
    }

    /// Acrescenta um arquivo. `mtime` em FILETIME ([`filetime_de_unix`]).
    /// `atributos`: `None` grava o atributo de arquivo comum.
    pub fn arquivo(
        &mut self,
        nome: &str,
        dados: Vec<u8>,
        mtime: Option<u64>,
        atributos: Option<u32>,
    ) -> Resultado<()> {
        let nome = self.conferir_nome(nome)?;
        self.itens.push(Item {
            nome,
            dados: Some(dados),
            mtime,
            atributos,
        });
        Ok(())
    }

    /// Acrescenta uma pasta.
    pub fn pasta(&mut self, nome: &str, mtime: Option<u64>) -> Resultado<()> {
        let nome = self.conferir_nome(nome)?;
        self.itens.push(Item {
            nome,
            dados: None,
            mtime,
            atributos: None,
        });
        Ok(())
    }

    /// Grava o arquivo inteiro.
    pub fn gravar(self) -> Resultado<Vec<u8>> {
        let op = &self.opcoes;
        if op.senha.as_deref() == Some("") {
            return Err(Erro::Uso("senha vazia"));
        }
        // Com senha e o `acaso` do `Default` (zeros), o IV sairia o mesmo em
        // todo arquivo da mesma senha -- e calado. O campo promete que isso
        // nao nasce calado; a promessa so vale se a gravacao recusar.
        if op.senha.is_some() && op.acaso == [0; 32] {
            return Err(Erro::Uso(
                "senha sem acaso: forneca 32 bytes imprevisiveis em Opcoes::acaso",
            ));
        }
        if self.itens.is_empty() {
            return Ok(assinatura(0, 0, 0));
        }
        let mut solido = Vec::new();
        let mut tamanhos = Vec::new();
        let mut crcs = Vec::new();
        for i in &self.itens {
            if let Some(d) = i.dados.as_ref().filter(|d| !d.is_empty()) {
                solido.extend_from_slice(d);
                tamanhos.push(d.len() as u64);
                crcs.push(crc32(d));
            }
        }

        // A chave sai UMA vez: dados e cabecalho usam a mesma senha, o mesmo
        // sal (vazio) e os mesmos ciclos, e cada derivacao custa 2^19 voltas
        // de SHA-256 -- derivar duas vezes dobrava o preco de todo `.phz`.
        let aes = match op.senha.as_deref() {
            Some(s) => Some(Aes256::novo(&derivar(s, &[], CICLOS_PADRAO)?)),
            None => None,
        };
        let mut empacotado = Vec::new();
        let mut cab = Vec::new();
        gravar_numero(&mut cab, K_CABECALHO);
        if !solido.is_empty() {
            let (dado, pasta) = codificar(&solido, op.nivel, aes.as_ref(), iv(&op.acaso, 0), op);
            gravar_numero(&mut cab, K_FLUXOS_PRINCIPAIS);
            gravar_fluxos(&mut cab, 0, dado.len() as u64, &pasta, None);
            // SubStreamsInfo: quantos, os tamanhos menos o ultimo, e os CRCs.
            gravar_numero(&mut cab, K_SUBFLUXOS);
            if tamanhos.len() != 1 {
                gravar_numero(&mut cab, K_QUANTOS_SUBFLUXOS);
                gravar_numero(&mut cab, tamanhos.len() as u64);
                gravar_numero(&mut cab, K_TAMANHO);
                for t in &tamanhos[..tamanhos.len() - 1] {
                    gravar_numero(&mut cab, *t);
                }
            }
            gravar_numero(&mut cab, K_CRC);
            cab.push(1);
            for c in &crcs {
                cab.extend_from_slice(&c.to_le_bytes());
            }
            gravar_numero(&mut cab, K_FIM);
            gravar_numero(&mut cab, K_FIM);
            empacotado = dado;
        }
        self.gravar_arquivos(&mut cab);
        gravar_numero(&mut cab, K_FIM);

        // O cabecalho vai sempre comprimido, e cifrado quando os nomes vao.
        let aes_cab = if op.cifrar_nomes { aes.as_ref() } else { None };
        let (dado_cab, pasta_cab) = codificar(
            &cab,
            5.max(op.nivel.min(9)),
            aes_cab,
            iv(&op.acaso, 1),
            &Opcoes::default(),
        );
        let mut cod = Vec::new();
        gravar_numero(&mut cod, K_CABECALHO_CODIFICADO);
        gravar_fluxos(
            &mut cod,
            empacotado.len() as u64,
            dado_cab.len() as u64,
            &pasta_cab,
            Some(crc32(&cab)),
        );

        let desl = (empacotado.len() + dado_cab.len()) as u64;
        let mut saida = assinatura(desl, cod.len() as u64, crc32(&cod));
        saida.extend_from_slice(&empacotado);
        saida.extend_from_slice(&dado_cab);
        saida.extend_from_slice(&cod);
        Ok(saida)
    }

    fn gravar_arquivos(&self, cab: &mut Vec<u8>) {
        let n = self.itens.len();
        gravar_numero(cab, K_ARQUIVOS);
        gravar_numero(cab, n as u64);

        let vazio: Vec<bool> = self
            .itens
            .iter()
            .map(|i| i.dados.as_ref().map_or(true, |d| d.is_empty()))
            .collect();
        if vazio.iter().any(|v| *v) {
            let mut p = Vec::new();
            gravar_bits(&mut p, &vazio);
            propriedade(cab, K_FLUXO_VAZIO, &p);
            let arquivo_vazio: Vec<bool> = self
                .itens
                .iter()
                .zip(&vazio)
                .filter(|(_, v)| **v)
                .map(|(i, _)| i.dados.is_some())
                .collect();
            if arquivo_vazio.iter().any(|v| *v) {
                let mut p = Vec::new();
                gravar_bits(&mut p, &arquivo_vazio);
                propriedade(cab, K_ARQUIVO_VAZIO, &p);
            }
        }

        let mut p = alloc::vec![0u8];
        for i in &self.itens {
            for u in i.nome.encode_utf16() {
                p.extend_from_slice(&u.to_le_bytes());
            }
            p.extend_from_slice(&[0, 0]);
        }
        propriedade(cab, K_NOME, &p);

        if self.itens.iter().any(|i| i.mtime.is_some()) {
            let def: Vec<bool> = self.itens.iter().map(|i| i.mtime.is_some()).collect();
            let mut p = Vec::new();
            gravar_definidos(&mut p, &def);
            p.push(0);
            for t in self.itens.iter().filter_map(|i| i.mtime) {
                p.extend_from_slice(&t.to_le_bytes());
            }
            propriedade(cab, K_MTIME, &p);
        }

        let mut p = alloc::vec![1u8, 0];
        for i in &self.itens {
            let a = match (i.dados.is_some(), i.atributos) {
                (_, Some(a)) => a,
                (true, None) => 0x20,
                (false, None) => 0x10,
            };
            p.extend_from_slice(&a.to_le_bytes());
        }
        propriedade(cab, K_ATRIBUTOS, &p);
        gravar_numero(cab, K_FIM);
    }
}

fn gravar_definidos(p: &mut Vec<u8>, def: &[bool]) {
    if def.iter().all(|d| *d) {
        p.push(1);
    } else {
        p.push(0);
        gravar_bits(p, def);
    }
}

fn propriedade(cab: &mut Vec<u8>, tipo: u64, dados: &[u8]) {
    gravar_numero(cab, tipo);
    gravar_numero(cab, dados.len() as u64);
    cab.extend_from_slice(dados);
}

fn assinatura(desl: u64, tam: u64, crc: u32) -> Vec<u8> {
    let mut s = Vec::with_capacity(CABECALHO_INICIAL);
    s.extend_from_slice(&ASSINATURA);
    s.extend_from_slice(&[0, 4]);
    let mut resto = Vec::with_capacity(20);
    resto.extend_from_slice(&desl.to_le_bytes());
    resto.extend_from_slice(&tam.to_le_bytes());
    resto.extend_from_slice(&crc.to_le_bytes());
    s.extend_from_slice(&crc32(&resto).to_le_bytes());
    s.extend_from_slice(&resto);
    s
}

/// IV do fluxo `n`, tirado da semente: dois fluxos do mesmo arquivo (dados e
/// cabecalho) com a mesma chave nunca repetem IV.
fn iv(acaso: &[u8; 32], n: u8) -> [u8; BLOCO] {
    let mut e = [0u8; 33];
    e[..32].copy_from_slice(acaso);
    e[32] = n;
    let h = phxhash::hash::sha256(&e);
    let mut v = [0u8; BLOCO];
    v.copy_from_slice(&h[..BLOCO]);
    v
}

/// Uma pasta a gravar: coders na ordem do arquivo, e o tamanho de cada saida.
struct PastaEscrita {
    coders: Vec<(u64, Vec<u8>)>,
    tamanhos: Vec<u64>,
}

/// Comprime (e cifra) `dados`. Coder 0 e o compressor, cuja saida e o dado
/// final; coder 1, se ha, e o 7zAES, cuja entrada e o fluxo empacotado.
fn codificar(
    dados: &[u8],
    nivel: u8,
    aes: Option<&Aes256>,
    iv: [u8; BLOCO],
    op: &Opcoes,
) -> (Vec<u8>, PastaEscrita) {
    let (id, props, mut fluxo) = if nivel == 0 {
        (COPY, Vec::new(), dados.to_vec())
    } else if op.fios > 1 {
        let (p, c) = em_blocos(dados, Nivel::de(nivel), op.fios, op.bloco_efetivo(nivel));
        (LZMA2, alloc::vec![p], c)
    } else {
        let (p, c) = codificar_lzma2(dados, Nivel::de(nivel));
        (LZMA2, alloc::vec![p], c)
    };
    let mut pasta = PastaEscrita {
        coders: alloc::vec![(id, props)],
        tamanhos: alloc::vec![dados.len() as u64],
    };
    if let Some(aes) = aes {
        let pa = ParamAes {
            ciclos: CICLOS_PADRAO,
            sal: Vec::new(),
            iv,
            iv_len: BLOCO,
        };
        let real = fluxo.len();
        fluxo.resize(real.div_ceil(BLOCO) * BLOCO, 0);
        aes.cifrar_cbc(&pa.iv, &mut fluxo);
        pasta.coders.push((AES, pa.gravar()));
        pasta.tamanhos.push(real as u64);
    }
    (fluxo, pasta)
}

impl Opcoes {
    fn bloco_efetivo(&self, nivel: u8) -> usize {
        if self.bloco == 0 {
            bloco_padrao(nivel)
        } else {
            self.bloco
        }
    }
}

/// Os blocos, compactados por ate `fios` fios. Cada fio pega o proximo bloco
/// livre (um contador atomico), entao um bloco lento nao prende os outros; o
/// resultado volta na ORDEM do corte, que e o que torna os bytes iguais em
/// qualquer numero de fios. O numero de fios e limitado pelo de blocos: fio
/// sem bloco nao nasce.
#[cfg(feature = "std")]
fn em_blocos(dados: &[u8], nivel: Nivel, fios: usize, bloco: usize) -> (u8, Vec<u8>) {
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    let partes: Vec<&[u8]> = blocos_lzma2(dados, bloco).collect();
    let prontos: Vec<Mutex<Vec<u8>>> = partes.iter().map(|_| Mutex::new(Vec::new())).collect();
    let proximo = AtomicUsize::new(0);
    // Fios sobrando viram BUSCA em paralelo: na arvore (niveis 5 a 9) cada
    // bloco pode levar dois fios, a busca numa thread e a codificacao noutra,
    // sem custo de compressao. Mas so quando ha fio para os dois: com blocos
    // de sobra, um fio por bloco rende mais (medido, 21 MB e 4 nucleos: 4
    // blocos de 4 MiB em 4 fios, 3,23 s; os mesmos em 2x2 fios, 4,04 s).
    let busca_em_fio = nivel.arvore && partes.len() * 2 <= fios;
    let n = fios.min(partes.len()).max(1);
    std::thread::scope(|s| {
        for _ in 0..n {
            s.spawn(|| loop {
                let i = proximo.fetch_add(1, Ordering::Relaxed);
                let Some(p) = partes.get(i) else { break };
                let c = if busca_em_fio {
                    crate::lzma::codificar_bloco_lzma2_em_fio(p, nivel)
                } else {
                    codificar_bloco_lzma2(p, nivel)
                };
                if let Ok(mut v) = prontos[i].lock() {
                    *v = c;
                }
            });
        }
    });
    let blocos: Vec<Vec<u8>> = prontos
        .into_iter()
        .map(|m| m.into_inner().unwrap_or_default())
        .collect();
    juntar_blocos_lzma2(dados.len(), bloco, nivel, &blocos)
}

/// Sem `std` nao ha fio: os mesmos blocos, um depois do outro -- e os mesmos
/// bytes que o paralelo produziria.
#[cfg(not(feature = "std"))]
fn em_blocos(dados: &[u8], nivel: Nivel, _fios: usize, bloco: usize) -> (u8, Vec<u8>) {
    let blocos: Vec<Vec<u8>> = blocos_lzma2(dados, bloco)
        .map(|p| codificar_bloco_lzma2(p, nivel))
        .collect();
    juntar_blocos_lzma2(dados.len(), bloco, nivel, &blocos)
}

fn gravar_fluxos(cab: &mut Vec<u8>, pos: u64, tam: u64, pasta: &PastaEscrita, crc: Option<u32>) {
    gravar_numero(cab, K_EMPACOTADOS);
    gravar_numero(cab, pos);
    gravar_numero(cab, 1);
    gravar_numero(cab, K_TAMANHO);
    gravar_numero(cab, tam);
    gravar_numero(cab, K_FIM);

    gravar_numero(cab, K_DESEMPACOTADOS);
    gravar_numero(cab, K_PASTA);
    gravar_numero(cab, 1);
    cab.push(0);
    gravar_numero(cab, pasta.coders.len() as u64);
    for (id, props) in &pasta.coders {
        let mut id_bytes = Vec::new();
        let mut v = *id;
        loop {
            id_bytes.insert(0, v as u8);
            v >>= 8;
            if v == 0 {
                break;
            }
        }
        let tem_props = !props.is_empty();
        cab.push(id_bytes.len() as u8 | if tem_props { 0x20 } else { 0 });
        cab.extend_from_slice(&id_bytes);
        if tem_props {
            gravar_numero(cab, props.len() as u64);
            cab.extend_from_slice(props);
        }
    }
    // Ligacao: a entrada do coder 0 vem da saida do coder 1 (o AES).
    if pasta.coders.len() == 2 {
        gravar_numero(cab, 0);
        gravar_numero(cab, 1);
    }
    gravar_numero(cab, K_TAMANHOS_DOS_CODERS);
    for t in &pasta.tamanhos {
        gravar_numero(cab, *t);
    }
    if let Some(c) = crc {
        gravar_numero(cab, K_CRC);
        cab.push(1);
        cab.extend_from_slice(&c.to_le_bytes());
    }
    gravar_numero(cab, K_FIM);
    if crc.is_some() {
        gravar_numero(cab, K_FIM);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Senha com o `acaso` do `Default` e recusa: o IV zero sairia igual em
    /// todo arquivo da mesma senha. Sem senha, o acaso nao importa e grava.
    fn gravar_com(dados: &[u8], fios: usize, bloco: usize) -> Vec<u8> {
        let mut e = Escritor::novo(Opcoes {
            nivel: 5,
            fios,
            bloco,
            ..Opcoes::default()
        });
        e.arquivo("a", dados.to_vec(), None, None).unwrap();
        e.gravar().unwrap()
    }

    /// Texto que repete com variacao: comprime, mas nao trivialmente.
    fn amostra(n: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(n);
        let mut x = 12345u32;
        while v.len() < n {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            v.extend_from_slice(b"linha do PhxZip numero ");
            v.extend_from_slice(alloc::format!("{}\n", x >> 20).as_bytes());
        }
        v.truncate(n);
        v
    }

    /// Os bytes dependem do BLOCO e nao dos fios: 2 e 7 fios dao o mesmo
    /// arquivo, e ele volta inteiro pelo leitor.
    #[test]
    fn em_blocos_os_bytes_nao_dependem_dos_fios_e_voltam_inteiros() {
        let d = amostra(600_000);
        let a = gravar_com(&d, 2, 1 << 16);
        let b = gravar_com(&d, 7, 1 << 16);
        assert_eq!(a, b, "o numero de fios mudou os bytes");
        assert_ne!(a, gravar_com(&d, 1, 0), "o corte em blocos nao aconteceu");
        let arq = crate::Arquivo7z::abrir(&a, None, crate::Limites::default()).unwrap();
        assert_eq!(arq.extrair(0).unwrap(), d);
    }

    /// Dado menor que um bloco: nada a cortar, e o arquivo e o de um fio so.
    #[test]
    fn dado_menor_que_o_bloco_sai_igual_ao_de_um_fio() {
        let d = amostra(50_000);
        assert_eq!(gravar_com(&d, 4, 0), gravar_com(&d, 1, 0));
    }

    #[test]
    fn senha_sem_acaso_e_recusada_e_sem_senha_nao() {
        let mut e = Escritor::novo(Opcoes {
            senha: Some("s".into()),
            ..Opcoes::default()
        });
        e.arquivo("a", b"x".to_vec(), None, None).unwrap();
        assert!(matches!(e.gravar(), Err(Erro::Uso(m)) if m.contains("acaso")));
        let mut e = Escritor::novo(Opcoes::default());
        e.arquivo("a", b"x".to_vec(), None, None).unwrap();
        assert!(e.gravar().is_ok());
    }
}

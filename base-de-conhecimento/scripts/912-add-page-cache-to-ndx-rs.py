# Add page cache to ndx.rs
# 29/08 00:10

import pathlib
p = pathlib.Path("crates/phxsql-store/src/ndx.rs")
s = p.read_text()

# 1) a estrutura do cache, antes de `pub struct NdxFile`
alvo = '''pub struct NdxFile {
    arquivo: File,
    caminho: PathBuf,
    page_size: usize,
    qtd_paginas: u64,
    pagina_livre: u64,
    indices: Vec<DescritorIndice>,
}'''
novo = '''/// Quantas paginas ficam em RAM por arquivo `.ndx` aberto.
///
/// 512 paginas de 4 KiB dao 2 MiB por tabela aberta. O servidor abre e fecha a
/// tabela a cada operacao, entao esse teto vale enquanto a operacao dura -- e a
/// operacao que importa aqui e a carga em lote, que insere milhares de linhas
/// dentro de uma unica abertura.
const PAGINAS_EM_CACHE: usize = 512;

/// As paginas do `.ndx` que ficam em RAM.
///
/// # De onde vem o ganho
///
/// Toda insercao DESCE a arvore: raiz, no interno, folha. Sao tres `pread` de
/// uma pagina inteira mais tres CRC-32 de pagina inteira -- e a raiz e a mesma
/// pagina em todas as insercoes da carga. Guardar a pagina lida tira o nucleo e
/// o CRC do caminho de quem ja passou por ali.
///
/// # Por que a gravacao continua atravessando
///
/// Segurar pagina suja em RAM daria mais, e trocaria uma garantia por
/// desempenho **sem avisar**: hoje uma queda do PROCESSO nao atrasa o `.ndx`
/// em relacao ao `.reg`, porque o `write` ja entregou a pagina ao nucleo. So
/// uma queda da MAQUINA faz isso. A diferenca entre os dois casos e grande
/// demais para ser trocada de lado num commit de desempenho.
///
/// # A politica de despejo
///
/// Segunda chance (CLOCK): a pagina despejada e a mais antiga que nao foi
/// usada desde que entrou. Fila simples nao serviria -- a raiz, que e a mais
/// visitada de todas, sairia junto com as outras assim que o teto enchesse.
struct CachePaginas {
    paginas: HashMap<u64, Entrada>,
    fila: VecDeque<u64>,
    teto: usize,
    acertos: u64,
    faltas: u64,
}

struct Entrada {
    bytes: Vec<u8>,
    usada: bool,
}

impl CachePaginas {
    fn nova(teto: usize) -> CachePaginas {
        CachePaginas {
            paginas: HashMap::with_capacity(teto.min(1024)),
            fila: VecDeque::with_capacity(teto.min(1024)),
            teto,
            acertos: 0,
            faltas: 0,
        }
    }

    fn pegar(&mut self, n: u64) -> Option<Vec<u8>> {
        match self.paginas.get_mut(&n) {
            Some(e) => {
                e.usada = true;
                self.acertos += 1;
                Some(e.bytes.clone())
            }
            None => {
                self.faltas += 1;
                None
            }
        }
    }

    fn por(&mut self, n: u64, bytes: &[u8]) {
        if let Some(e) = self.paginas.get_mut(&n) {
            e.bytes.clear();
            e.bytes.extend_from_slice(bytes);
            e.usada = true;
            return;
        }
        while self.paginas.len() >= self.teto {
            match self.fila.pop_front() {
                None => break,
                Some(velha) => match self.paginas.get_mut(&velha) {
                    None => {}
                    Some(e) if e.usada => {
                        e.usada = false;
                        self.fila.push_back(velha);
                    }
                    Some(_) => {
                        self.paginas.remove(&velha);
                    }
                },
            }
        }
        self.paginas.insert(
            n,
            Entrada {
                bytes: bytes.to_vec(),
                usada: false,
            },
        );
        self.fila.push_back(n);
    }

    /// Tira a pagina do cache. A pagina que volta da lista de livres vai ser
    /// reescrita do zero, e o conteudo velho dela nao vale mais nada.
    fn esquecer(&mut self, n: u64) {
        self.paginas.remove(&n);
    }
}

pub struct NdxFile {
    arquivo: File,
    caminho: PathBuf,
    page_size: usize,
    qtd_paginas: u64,
    pagina_livre: u64,
    indices: Vec<DescritorIndice>,
    cache: CachePaginas,
}'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# 2) os dois construtores
s = s.replace('''            qtd_paginas: 1, // pagina 0 = cabecalho + diretorio
            pagina_livre: 0,
            indices: Vec::new(),
        };''','''            qtd_paginas: 1, // pagina 0 = cabecalho + diretorio
            pagina_livre: 0,
            indices: Vec::new(),
            cache: CachePaginas::nova(PAGINAS_EM_CACHE),
        };''', 1)
s = s.replace('''        Ok(NdxFile {
            arquivo,
            caminho,
            page_size,
            qtd_paginas,
            pagina_livre,
            indices,
        })''','''        Ok(NdxFile {
            arquivo,
            caminho,
            page_size,
            qtd_paginas,
            pagina_livre,
            indices,
            cache: CachePaginas::nova(PAGINAS_EM_CACHE),
        })''', 1)

# 3) ler_pagina / gravar_pagina / alocar_pagina
s = s.replace('''        let mut p = vec![0u8; self.page_size];
        ler_exato(&mut self.arquivo, n * self.page_size as u64, &mut p)?;
        if pag_crc(&p) != Campos(&p).u32(28) {
            return Err(PhxError::Corrompido(format!(
                "CRC invalido na pagina {n} de {}",
                self.caminho.display()
            )));
        }
        Ok(p)
    }''','''        if let Some(p) = self.cache.pegar(n) {
            return Ok(p);
        }
        let mut p = vec![0u8; self.page_size];
        ler_exato(&mut self.arquivo, n * self.page_size as u64, &mut p)?;
        // O CRC e conferido na LEITURA DO ARQUIVO, e nao na do cache: a pagina
        // que esta em RAM ja passou por aqui, e conferir de novo pagaria o
        // mesmo CRC que este cache existe para nao pagar.
        if pag_crc(&p) != Campos(&p).u32(28) {
            return Err(PhxError::Corrompido(format!(
                "CRC invalido na pagina {n} de {}",
                self.caminho.display()
            )));
        }
        self.cache.por(n, &p);
        Ok(p)
    }''', 1)

s = s.replace('''    fn gravar_pagina(&mut self, n: u64, p: &mut [u8]) -> Result<()> {
        pag_selar(p);
        escrever_em(&mut self.arquivo, n * self.page_size as u64, p)
    }''','''    fn gravar_pagina(&mut self, n: u64, p: &mut [u8]) -> Result<()> {
        pag_selar(p);
        escrever_em(&mut self.arquivo, n * self.page_size as u64, p)?;
        // Guardar a pagina RECEM-GRAVADA e o que mais rende numa carga: a folha
        // que acabou de receber uma chave e quase sempre a que vai receber a
        // proxima, e sem isto ela voltaria do arquivo com CRC e tudo.
        self.cache.por(n, p);
        Ok(())
    }''', 1)

s = s.replace('''        if self.pagina_livre != 0 {
            let n = self.pagina_livre;
            let mut p = vec![0u8; self.page_size];
            ler_exato(&mut self.arquivo, n * self.page_size as u64, &mut p)?;
            self.pagina_livre = pag_prox(&p);
            return Ok(n);
        }''','''        if self.pagina_livre != 0 {
            let n = self.pagina_livre;
            let mut p = vec![0u8; self.page_size];
            ler_exato(&mut self.arquivo, n * self.page_size as u64, &mut p)?;
            self.pagina_livre = pag_prox(&p);
            // A pagina volta da lista de livres para ser reescrita do zero: o
            // que o cache tem dela e o conteudo de antes de ela ser liberada.
            self.cache.esquecer(n);
            return Ok(n);
        }''', 1)
p.write_text(s)
print("ok")

//! BM25 escrito a mao, e o indice de documentos que se apoia nele.
//!
//! Mora AQUI, no crate da memoria, e nao num crate de busca novo: o agente ja tem um lugar
//! que responde «o que eu sei sobre X», e um segundo indice ao lado teria tokenizador,
//! tetos e formato proprios divergindo do primeiro. O tokenizador e o mesmo da memoria
//! (`palavras`); o que o BM25 acrescenta e dobrar o acento, porque aqui a pergunta casa
//! palavra inteira (`funcao` tem de achar `função`), e la casa por substring.
//!
//! Por que BM25 e nao a contagem de termos da memoria: num corpus de documentos a palavra
//! comum («arquivo», «sistema») aparece em quase tudo, e contar ocorrencia faria o trecho
//! longo e generico ganhar do curto e certeiro. O BM25 pesa o termo pela raridade (idf) e
//! satura a repeticao (k1), normalizando pelo tamanho do trecho (b). FTS5 e tantivy fazem
//! isso e ficaram de fora por estarem fora do Cargo.lock; o FTS do PostgreSQL exigiria o
//! banco para uma pasta de arquivos.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Os parametros classicos (Robertson/Zaragoza; os padroes do Lucene e do Elasticsearch).
pub const K1: f64 = 1.2;
pub const B: f64 = 0.75;

/// Troca a letra acentuada latina pela base. So o que aparece em portugues, espanhol,
/// frances e alemao: o resto passa como veio, e casa consigo mesmo.
pub fn dobrar_acento(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        'ý' | 'ÿ' => 'y',
        outro => outro,
    }
}

/// Os termos do texto para o BM25: as palavras da memoria, em minusculas e sem acento, de
/// 3 letras ou mais -- o mesmo corte da busca da memoria, pelo mesmo motivo: «de», «do»,
/// «em» estao em todo trecho em portugues, e a pergunta «senha do cofre» devolveria
/// qualquer paragrafo com «do» quando nada fala de cofre.
pub fn termos(texto: &str) -> Vec<String> {
    crate::palavras(texto)
        .filter(|p| p.chars().count() >= 3)
        .map(|p| p.to_lowercase().chars().map(dobrar_acento).collect())
        .collect()
}

/// O indice invertido em memoria: por termo, os documentos e a frequencia nele.
#[derive(Debug, Clone, Default)]
pub struct Bm25 {
    tamanhos: Vec<u32>,
    media: f64,
    postings: HashMap<String, Vec<(u32, u32)>>,
}

impl Bm25 {
    pub fn novo<S: AsRef<str>>(textos: impl IntoIterator<Item = S>) -> Self {
        let mut idx = Self::default();
        let mut total = 0u64;
        for (d, t) in textos.into_iter().enumerate() {
            let ts = termos(t.as_ref());
            total += ts.len() as u64;
            idx.tamanhos.push(ts.len() as u32);
            let mut freq: HashMap<String, u32> = HashMap::new();
            for termo in ts {
                *freq.entry(termo).or_default() += 1;
            }
            for (termo, f) in freq {
                idx.postings.entry(termo).or_default().push((d as u32, f));
            }
        }
        idx.media = if idx.tamanhos.is_empty() {
            0.0
        } else {
            total as f64 / idx.tamanhos.len() as f64
        };
        idx
    }

    pub fn len(&self) -> usize {
        self.tamanhos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tamanhos.is_empty()
    }

    /// Idf da variante do Lucene: `ln(1 + (N - df + 0,5) / (df + 0,5))`. A classica fica
    /// NEGATIVA para termo presente em mais da metade dos documentos, e a pergunta com uma
    /// palavra comum passaria a EMPURRAR para baixo quem a contem.
    fn idf(&self, df: usize) -> f64 {
        let n = self.tamanhos.len() as f64;
        let df = df as f64;
        (1.0 + (n - df + 0.5) / (df + 0.5)).ln()
    }

    /// Os `n` documentos de maior pontuacao, so os que casam algum termo; no empate, o de
    /// indice menor (a ordem de entrada), para a mesma pergunta dar sempre a mesma lista.
    pub fn buscar(&self, consulta: &str, n: usize) -> Vec<(usize, f64)> {
        let mut qs = termos(consulta);
        qs.sort();
        qs.dedup();
        let mut pontos: HashMap<u32, f64> = HashMap::new();
        for q in &qs {
            let Some(lista) = self.postings.get(q) else {
                continue;
            };
            let idf = self.idf(lista.len());
            for &(d, f) in lista {
                let f = f as f64;
                let tam = self.tamanhos[d as usize] as f64;
                let norma = K1 * (1.0 - B + B * tam / self.media.max(1.0));
                *pontos.entry(d).or_default() += idf * f * (K1 + 1.0) / (f + norma);
            }
        }
        let mut v: Vec<(usize, f64)> = pontos.into_iter().map(|(d, p)| (d as usize, p)).collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(n);
        v
    }
}

// ---------------------------------------------------------------- indice de documentos

/// Extensoes de texto que se indexam. Binario (pdf, docx) fica de fora desta versao: o
/// conversor existe no agente, mas indexar o que ele extrai pediria rodar processo aqui.
pub const EXTENSOES: &[&str] = &[
    "md", "markdown", "txt", "rst", "adoc", "org", "html", "htm", "csv", "json", "toml", "yaml",
    "yml", "rs", "py", "js", "ts", "go", "java", "c", "h", "cpp", "sh", "sql",
];
/// Arquivo maior que isto e quase sempre gerado (log, dump), e afogaria o resto.
pub const BYTES_POR_ARQUIVO: u64 = 2 * 1024 * 1024;
pub const ARQUIVOS_MAX: usize = 20_000;
/// Trecho alvo: perto de um paragrafo longo. Menor fragmenta a resposta; maior dilui o
/// termo raro num bloco onde ele e uma linha entre cinquenta.
pub const TRECHO_ALVO: usize = 1_200;
pub const VERSAO_INDICE: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArquivoIndexado {
    pub caminho: PathBuf,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trecho {
    /// Indice em `arquivos`.
    pub arquivo: usize,
    /// Linha (1-based) onde o trecho comeca.
    pub linha: usize,
    pub texto: String,
}

/// O que vai para o disco: os trechos com o texto, e nao o indice invertido. Reconstruir
/// as listas ao carregar custa milissegundos para dezenas de milhares de trechos, e o
/// arquivo continua legivel e conferivel (o SHA-256 de cada arquivo de origem esta nele).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct IndiceDeDocumentos {
    pub versao: u32,
    pub arquivos: Vec<ArquivoIndexado>,
    pub trechos: Vec<Trecho>,
}

/// Um achado, com o trecho inteiro e de onde ele veio.
#[derive(Debug, Clone, PartialEq)]
pub struct AchadoDoc {
    pub caminho: PathBuf,
    pub linha: usize,
    pub pontos: f64,
    pub texto: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Relatorio {
    pub arquivos: usize,
    pub trechos: usize,
    /// Fora por tamanho, por nao ser UTF-8 ou por passar do teto de arquivos.
    pub pulados: usize,
}

/// Parte o texto em trechos de ~`TRECHO_ALVO` caracteres, cortando em linha em branco
/// quando da (o paragrafo e a unidade de sentido), e em linha quando o bloco nao tem pausa.
pub fn trechos_de(texto: &str) -> Vec<(usize, String)> {
    let mut v = Vec::new();
    let mut atual = String::new();
    let mut inicio = 1usize;
    for (i, linha) in texto.lines().enumerate() {
        let n = i + 1;
        let pausa = linha.trim().is_empty();
        if atual.is_empty() {
            if pausa {
                continue;
            }
            inicio = n;
        }
        if (pausa && atual.len() >= TRECHO_ALVO / 2) || atual.len() >= TRECHO_ALVO * 2 {
            v.push((inicio, std::mem::take(&mut atual).trim_end().to_string()));
            if pausa {
                continue;
            }
            inicio = n;
        }
        atual.push_str(linha);
        atual.push('\n');
    }
    if !atual.trim().is_empty() {
        v.push((inicio, atual.trim_end().to_string()));
    }
    v
}

fn pular_pasta(nome: &str) -> bool {
    nome.starts_with('.') || matches!(nome, "target" | "node_modules" | "__pycache__")
}

fn arquivos_de(raiz: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(raiz) else {
        return;
    };
    let mut entradas: Vec<_> = rd.flatten().collect();
    entradas.sort_by_key(|e| e.file_name());
    for e in entradas {
        let p = e.path();
        // `symlink_metadata`: link para fora da pasta nao vira porta para o disco inteiro.
        let Ok(m) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        let nome = e.file_name().to_string_lossy().into_owned();
        if m.is_dir() {
            if !pular_pasta(&nome) {
                arquivos_de(&p, saida);
            }
        } else if m.is_file()
            && p.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| EXTENSOES.contains(&x.to_ascii_lowercase().as_str()))
        {
            saida.push(p);
        }
    }
}

impl IndiceDeDocumentos {
    pub fn carregar(arquivo: &Path) -> Result<Self, String> {
        let b = std::fs::read(arquivo).map_err(|e| format!("{}: {e}", arquivo.display()))?;
        let i: Self =
            serde_json::from_slice(&b).map_err(|e| format!("{}: {e}", arquivo.display()))?;
        if i.versao != VERSAO_INDICE {
            return Err(format!(
                "{}: versao {} do indice, esperada {VERSAO_INDICE}; rode o indexar de novo",
                arquivo.display(),
                i.versao
            ));
        }
        i.conferir()
            .map_err(|e| format!("{}: {e}; rode o indexar de novo", arquivo.display()))?;
        Ok(i)
    }

    /// O indice e coerente: todo trecho aponta para um arquivo que existe. Um arquivo
    /// editado a mao, ou de uma gravacao que nao era a nossa, entrava com `arquivo: 999`
    /// e o `motor()` entrava em panico na primeira busca -- derrubando o servidor por um
    /// arquivo de cache.
    pub fn conferir(&self) -> Result<(), String> {
        for (n, t) in self.trechos.iter().enumerate() {
            if t.arquivo >= self.arquivos.len() {
                return Err(format!(
                    "indice incoerente: o trecho {n} aponta para o arquivo {} e ha {}",
                    t.arquivo,
                    self.arquivos.len()
                ));
            }
        }
        Ok(())
    }

    /// Grava pela troca atomica da base: indexar que cai no meio deixa o indice anterior.
    pub fn gravar(&self, arquivo: &Path) -> Result<(), String> {
        phxclaw_types::arquivo::gravar_atomico(
            arquivo,
            &serde_json::to_vec(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("{}: {e}", arquivo.display()))
    }

    /// Indexa `pasta` (recursivo), SUBSTITUINDO o que o indice ja tinha dela e mantendo as
    /// outras pastas: reindexar uma pasta nao apaga a outra.
    pub fn indexar_pasta(&mut self, pasta: &Path) -> Result<Relatorio, String> {
        let raiz = pasta
            .canonicalize()
            .map_err(|e| format!("{}: {e}", pasta.display()))?;
        self.versao = VERSAO_INDICE;
        // Tira o que era desta pasta, renumerando os trechos que ficam.
        let manter: Vec<bool> = self
            .arquivos
            .iter()
            .map(|a| !a.caminho.starts_with(&raiz))
            .collect();
        let mut novo_indice = Vec::new();
        let mut arquivos = Vec::new();
        for (i, a) in self.arquivos.drain(..).enumerate() {
            novo_indice.push(arquivos.len());
            if manter[i] {
                arquivos.push(a);
            }
        }
        self.trechos
            .retain(|t| manter.get(t.arquivo).copied().unwrap_or(false));
        for t in &mut self.trechos {
            t.arquivo = novo_indice[t.arquivo];
        }
        self.arquivos = arquivos;

        let mut lista = Vec::new();
        arquivos_de(&raiz, &mut lista);
        let mut rel = Relatorio::default();
        for p in lista {
            if self.arquivos.len() >= ARQUIVOS_MAX {
                rel.pulados += 1;
                continue;
            }
            let Ok(m) = std::fs::metadata(&p) else {
                rel.pulados += 1;
                continue;
            };
            if m.len() > BYTES_POR_ARQUIVO {
                rel.pulados += 1;
                continue;
            }
            let Ok(b) = std::fs::read(&p) else {
                rel.pulados += 1;
                continue;
            };
            let Ok(texto) = std::str::from_utf8(&b) else {
                rel.pulados += 1;
                continue;
            };
            let id = self.arquivos.len();
            for (linha, texto) in trechos_de(texto) {
                self.trechos.push(Trecho {
                    arquivo: id,
                    linha,
                    texto,
                });
                rel.trechos += 1;
            }
            self.arquivos.push(ArquivoIndexado {
                caminho: p,
                sha256: format!("{:x}", Sha256::digest(&b)),
                bytes: m.len(),
            });
            rel.arquivos += 1;
        }
        Ok(rel)
    }

    /// O indice invertido dos trechos, para buscar muitas vezes sem refazer.
    pub fn motor(&self) -> Bm25 {
        Bm25::novo(self.trechos.iter().map(|t| {
            // O nome do arquivo entra no texto indexado: quem pergunta por «instalacao»
            // quer o INSTALACAO.md mesmo quando o paragrafo nao repete a palavra.
            let nome = self
                .arquivos
                .get(t.arquivo)
                .and_then(|a| a.caminho.file_stem())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            format!("{nome}\n{}", t.texto)
        }))
    }

    pub fn buscar(&self, motor: &Bm25, consulta: &str, n: usize) -> Vec<AchadoDoc> {
        motor
            .buscar(consulta, n)
            .into_iter()
            .filter_map(|(i, pontos)| {
                let t = self.trechos.get(i)?;
                Some(AchadoDoc {
                    caminho: self.arquivos.get(t.arquivo)?.caminho.clone(),
                    linha: t.linha,
                    pontos,
                    texto: t.texto.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn termo_raro_ganha_do_comum_repetido() {
        // O 0 repete a palavra comum; o 1 tem so a rara. Sem o idf, a repeticao ganharia.
        let docs = [
            "arquivo arquivo arquivo arquivo arquivo arquivo arquivo arquivo",
            "a chave ed25519 do servidor fica guardada no cofre do operador",
            "um arquivo de configuracao",
            "outro arquivo de log",
            "mais um arquivo qualquer",
        ];
        let m = Bm25::novo(docs);
        let r = m.buscar("arquivo ed25519", 3);
        assert_eq!(r[0].0, 1, "{r:?}");
        assert!(m.buscar("inexistente", 3).is_empty());
        // Acento dobrado nos dois lados.
        let m = Bm25::novo(["Instalação do serviço", "outra coisa"]);
        assert_eq!(m.buscar("instalacao servico", 1)[0].0, 0);
    }

    /// Parecer do DBA (01/10/2026), defeito 4: trecho apontando para arquivo que nao existe
    /// derrubava o processo (`index out of bounds`) no `motor()`. Reposto: panico; agora o
    /// `carregar` recusa com o motivo, e as buscas sobre um indice montado na mao pulam o
    /// trecho em vez de cair.
    #[test]
    fn indice_incoerente_devolve_erro_e_nunca_panico() {
        let d = std::env::temp_dir().join(format!("phx-bm25-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&d).unwrap();
        let arq = d.join("indice.json");
        let mut i = IndiceDeDocumentos {
            versao: VERSAO_INDICE,
            arquivos: vec![],
            trechos: vec![Trecho {
                arquivo: 7,
                linha: 1,
                texto: "orfao".into(),
            }],
        };
        i.gravar(&arq).unwrap();
        let e = IndiceDeDocumentos::carregar(&arq).unwrap_err();
        assert!(e.contains("incoerente") && e.contains("indexar"), "{e}");
        // Mesmo sem passar pelo carregar: busca e reindexacao nao caem.
        let m = i.motor();
        assert!(i.buscar(&m, "orfao", 3).is_empty());
        assert!(i.indexar_pasta(&d).is_ok());
        assert!(i.conferir().is_ok());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn trechos_cortam_no_paragrafo_e_guardam_a_linha() {
        // Paragrafos curtos se juntam ate o alvo; o longo fecha o trecho na pausa seguinte.
        let p = "a ".repeat(400);
        let texto = format!("um\n\ndois\n{p}\n\n\nfim\n");
        let t = trechos_de(&texto);
        assert_eq!(t.len(), 2, "{t:?}");
        assert_eq!(t[0].0, 1);
        assert!(t[0].1.starts_with("um\n\ndois\n"), "{:?}", t[0]);
        assert_eq!(t[1], (7, "fim".to_string()));
    }
}

//! O `.fts`: o indice de texto, sobre a arvore que ja existe.
//!
//! O desenho esta em `docs/FTS.md`. Este modulo e a peca que grava e le, e ele
//! **nao decide quando gravar** -- o despejo em lote e do chamador (§4.3).
//!
//! # Por que ele nao tem formato proprio
//!
//! Um indice invertido classico guarda, por termo, uma lista de identificadores
//! comprimida por delta. Aqui a lista de um termo e o que o
//! [`NdxFile::buscar`] ja devolve: a arvore B+ anexa o rowid a toda chave, e
//! todas as chaves de um termo saem juntas. O formato proprio compraria
//! TAMANHO, e tamanho nao e o gargalo medido -- o gargalo e tocar na linha
//! (`DESEMPENHO.md` §20). Reusar a arvore traz de graca o CRC-32 por pagina, o
//! cache de leitura que comprou 2,40x e o `verificar`.
//!
//! # A restricao que moldou a chave, e o falso positivo que ela evita
//!
//! O `.ndx` usa chave de LARGURA FIXA (`DescritorIndice::key_len`). Termo e
//! palavra, e palavra tem tamanho livre -- entao ou se trunca, ou nao cabe.
//!
//! Truncar sozinho criaria **falso positivo**: `transportadora` e
//! `transportadoras` cortadas no mesmo ponto viram a mesma chave, e a busca
//! por uma acharia a outra. E o `FTS.md` §7.1 e explicito: *indice que acha a
//! MAIS e pior que indice que acha a menos*, porque achar a menos e atraso e
//! achar a mais e mentira.
//!
//! Por isso a chave carrega o **comprimento real** ao lado:
//!
//! ```text
//! chave = [TERMO_LEN bytes do termo dobrado, truncado] [1 byte: bytes reais, saturado em 255]
//! ```
//!
//! Duas palavras so colidem se compartilharem os 24 primeiros bytes **e**
//! tiverem o mesmo comprimento. Quando isso acontece -- e quando a palavra
//! procurada passa de `TERMO_LEN` --, o achado vem com
//! [`Achado::conferir`] ligado, e quem chamou confere lendo a linha. **A
//! duvida aparece como bandeira, e nunca como resposta errada.**

use std::path::Path;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::termo::{dobrar, termos, termos_sem_dobrar};
use phxsql_core::types::ColumnType;
use phxsql_core::RowId;

use crate::ndx::NdxFile;

/// Extensao do arquivo, ao lado do `.ndx`.
pub const EXT_FTS: &str = "fts";

/// Bytes do termo que o esquema sintetico PEDE.
///
/// 24 cobre a esmagadora maioria das palavras do portugues; o que passar disso
/// entra truncado e sai com [`Achado::conferir`] ligado.
///
/// **Este numero nao e a largura da chave, e a diferenca ja custou uma
/// rodada.** O `keyenc` poe um byte de prefixo em cada componente
/// (`largura_componente = 1 + largura_chave`), entao `Str(24)` ocupa 25 e o
/// `UInt1` do comprimento ocupa 2 -- a chave tem **27** bytes, e nao 25. Por
/// isso [`FtsFile::termo_len`] sai do DESCRITOR do arquivo e nao daqui: a
/// largura tem um dono so, e o dono e o `.ndx`.
pub const TERMO_PEDIDO: u16 = 24;

/// O que uma busca devolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Achado {
    /// Os rowids que o indice aponta, na ordem em que a arvore os deu.
    pub rowids: Vec<RowId>,
    /// A palavra procurada passou de [`TERMO_LEN`]?
    ///
    /// Ligado, quem chamou **tem de conferir** cada linha antes de devolve-la:
    /// o indice pode estar apontando uma palavra diferente que compartilha o
    /// prefixo e o comprimento. Desligado, o indice e exato e nao ha o que
    /// conferir -- que e o caso comum e o que compra os ~900x.
    pub conferir: bool,
}

/// O esquema sintetico do arquivo: uma "coluna" de termo e uma de comprimento,
/// e **um indice interno por indice de texto declarado na tabela**.
///
/// Ele existe so para o `NdxFile::criar` calcular a largura da chave; nenhuma
/// linha e gravada por ele. Os indices sao **livres**, e nao unicos: um termo
/// aparece em muitas linhas, e e disso que a lista de ocorrencias e feita.
///
/// # Por que um indice interno por indice declarado, e nao um arquivo por um
///
/// Porque duas colunas de texto da mesma tabela podem ter `dobrar` diferente,
/// e ai as chaves nao podem dividir o mesmo espaco: `fenix` dobrada e `Fênix`
/// crua sao termos distintos que apontariam para a mesma faixa. O `NdxFile` ja
/// sabe carregar varios indices num arquivo so -- e um arquivo por indice
/// multiplicaria descritores, caches e `fsync` por nada.
fn esquema_do_indice(quantos: usize) -> Schema {
    let mut idx = Vec::with_capacity(quantos.max(1));
    for i in 0..quantos.max(1) {
        idx.push(IndexDef::new(
            format!("porTermo{i}"),
            vec![IndexColumn::asc(0), IndexColumn::asc(1)],
        ));
    }
    Schema::new(
        "fts",
        vec![
            Column::new("termo", ColumnType::Str(TERMO_PEDIDO)),
            Column::new("bytes", ColumnType::UInt1),
        ],
        idx,
    )
    .expect("esquema do .fts")
}

/// O indice de texto de uma tabela.
pub struct FtsFile {
    ndx: NdxFile,
    /// Largura total da chave, tirada do descritor do arquivo.
    largura: usize,
    /// `dobrar` de cada indice interno, na ordem em que a tabela os declara.
    ///
    /// Fica aqui, e nao no arquivo: quem manda e o esquema da TABELA, que e
    /// onde a declaracao mora. Guardar uma segunda copia no `.fts` faria duas
    /// fontes para a mesma verdade, e a segunda envelheceria calada -- e o
    /// `.fts` e derivado, entao ele nunca e a fonte.
    dobra: Vec<bool>,
}

impl FtsFile {
    /// Cria o arquivo com um indice interno por `dobrar` da lista.
    ///
    /// A lista vem do esquema da tabela, na ordem dos indices de texto
    /// declarados. Falha se o arquivo ja existir, como o resto da familia.
    pub fn criar(caminho: impl AsRef<Path>, dobra: Vec<bool>) -> Result<FtsFile> {
        let caminho = caminho.as_ref();
        if caminho.exists() {
            return Err(PhxError::Esquema(format!(
                "{} ja existe; use FtsFile::abrir",
                caminho.display()
            )));
        }
        let ndx = NdxFile::criar(caminho, &esquema_do_indice(dobra.len()))?;
        Ok(FtsFile::com(ndx, dobra))
    }

    /// Abre um `.fts` que ja existe. A lista de `dobrar` vem do esquema da
    /// TABELA -- o `.fts` e derivado, e derivado nunca e a fonte.
    pub fn abrir(caminho: impl AsRef<Path>, dobra: Vec<bool>) -> Result<FtsFile> {
        let ndx = NdxFile::abrir(caminho)?;
        if ndx.indices().len() != dobra.len() {
            return Err(PhxError::Corrompido(format!(
                "o .fts tem {} indices e o esquema declara {}; \
                 reconstrua o indice de texto com `reindexar`",
                ndx.indices().len(),
                dobra.len()
            )));
        }
        Ok(FtsFile::com(ndx, dobra))
    }

    fn com(ndx: NdxFile, dobra: Vec<bool>) -> FtsFile {
        let largura = ndx.indices()[0].key_len;
        FtsFile {
            ndx,
            largura,
            dobra,
        }
    }

    /// Quantos indices de texto este arquivo carrega.
    pub fn quantos(&self) -> usize {
        self.dobra.len()
    }

    /// Os termos de um texto, dobrados ou nao conforme o indice pediu.
    fn termos_de(&self, idx: usize, texto: &str) -> Vec<String> {
        if self.dobra.get(idx).copied().unwrap_or(true) {
            termos(texto)
        } else {
            termos_sem_dobrar(texto)
        }
    }

    /// Quantos bytes do termo cabem na chave, medidos no arquivo.
    ///
    /// E a largura total menos o byte do comprimento. Sai do descritor porque
    /// a largura e do `.ndx`: cravar o numero aqui faria duas fontes para a
    /// mesma verdade, e a segunda envelheceria calada.
    pub fn termo_len(&self) -> usize {
        self.largura - 1
    }

    /// A chave de um termo ja dobrado.
    ///
    /// O corte e por BYTE e nao por caractere, e isso e seguro porque estes
    /// bytes nunca voltam a ser texto: eles so se comparam. Cortar por
    /// caractere custaria uma varredura do UTF-8 a cada chave, no laco quente.
    ///
    /// O comprimento REAL vai no ultimo byte, saturado. E ele que impede o
    /// falso positivo de duas palavras com o mesmo prefixo.
    fn chave(&self, termo_dobrado: &str) -> Vec<u8> {
        let b = termo_dobrado.as_bytes();
        let cabe = self.termo_len();
        let mut k = vec![0u8; self.largura];
        let n = b.len().min(cabe);
        k[..n].copy_from_slice(&b[..n]);
        k[cabe] = u8::try_from(b.len()).unwrap_or(u8::MAX);
        k
    }

    /// Poe no indice todos os termos de `texto`, apontando para `rowid`.
    ///
    /// Devolve quantos termos entraram -- o numero que o `FTS.md` §4.1 mediu
    /// como ~14 num texto de 200 bytes, e que decidiu o despejo em lote.
    pub fn indexar(&mut self, idx: usize, rowid: RowId, texto: &str) -> Result<usize> {
        let ts = self.termos_de(idx, texto);
        for t in &ts {
            let c = self.chave(t);
            self.ndx.inserir(idx, &c, rowid)?;
        }
        Ok(ts.len())
    }

    /// Tira do indice os termos de `texto` que apontam para `rowid`.
    ///
    /// Recebe o texto de novo, e nao o le do disco, por uma razao de desenho:
    /// quem exclui **ja tem a linha na mao** (o `.trash` a guarda inteira antes
    /// de sumir), e faze-lo reler seria pagar duas vezes o que a §20 do
    /// `DESEMPENHO.md` mediu como 43% do custo de uma busca.
    pub fn desindexar(&mut self, idx: usize, rowid: RowId, texto: &str) -> Result<usize> {
        let mut fora = 0;
        for t in self.termos_de(idx, texto) {
            let c = self.chave(&t);
            if self.ndx.remover(idx, &c, rowid)? {
                fora += 1;
            }
        }
        Ok(fora)
    }

    /// Procura uma palavra. Ela e dobrada aqui, como na gravacao.
    ///
    /// Dobrar nos dois lados e o que faz `Fenix`, `fenix` e `FÊNIX` acharem a
    /// mesma coisa -- e dobrar so num lado seria o defeito classico de indice,
    /// que aparece como "acha as vezes".
    pub fn procurar(&mut self, idx: usize, palavra: &str) -> Result<Achado> {
        let cru = palavra.trim();
        let t = if self.dobra.get(idx).copied().unwrap_or(true) {
            dobrar(cru)
        } else {
            cru.to_string()
        };
        if t.is_empty() {
            return Ok(Achado {
                rowids: Vec::new(),
                conferir: false,
            });
        }
        let conferir = t.len() > self.termo_len();
        let c = self.chave(&t);
        Ok(Achado {
            rowids: self.ndx.buscar(idx, &c)?,
            conferir,
        })
    }

    pub fn sincronizar(&mut self) -> Result<()> {
        self.ndx.sincronizar()
    }

    pub fn fechar(&mut self) -> Result<()> {
        self.ndx.fechar()
    }

    /// Quantas chaves um indice guarda. Serve a bancada e ao `verificar`.
    pub fn qtd_chaves(&self, idx: usize) -> u64 {
        self.ndx.indices()[idx].qtd_chaves
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Um `.fts` com um indice que dobra.
    fn novo(nome: &str) -> (FtsFile, std::path::PathBuf) {
        com_dobra(nome, vec![true])
    }

    fn com_dobra(nome: &str, dobra: Vec<bool>) -> (FtsFile, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("phx-fts-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let c = dir.join(format!("t.{EXT_FTS}"));
        let n = dobra.len();
        (FtsFile::criar(&c, dobra).unwrap(), c).tap(|f| assert_eq!(f.0.quantos(), n))
    }

    trait Tap: Sized {
        fn tap(self, f: impl FnOnce(&Self)) -> Self {
            f(&self);
            self
        }
    }
    impl<T> Tap for T {}

    #[test]
    fn acha_a_palavra_que_indexou() {
        let (mut f, _) = novo("basico");
        f.indexar(0, 1, "pedido do cliente fenix").unwrap();
        f.indexar(0, 2, "nota fiscal comum").unwrap();
        assert_eq!(f.procurar(0, "fenix").unwrap().rowids, vec![1]);
        assert_eq!(f.procurar(0, "nota").unwrap().rowids, vec![2]);
        assert!(f.procurar(0, "inexistente").unwrap().rowids.is_empty());
    }

    /// A razao de o `.fts` existir, e o que o `custo-da-busca-de-palavra`
    /// mediu que a varredura de hoje NAO faz: procurar sem acento acha com.
    #[test]
    fn a_dobra_vale_nos_dois_lados() {
        let (mut f, _) = novo("dobra");
        f.indexar(0, 7, "a Fênix renasce").unwrap();
        for grafia in ["fenix", "Fênix", "FENIX", "fÊnIx"] {
            assert_eq!(
                f.procurar(0, grafia).unwrap().rowids,
                vec![7],
                "procurar {grafia:?} tinha de achar"
            );
        }
    }

    /// Dois indices na MESMA tabela, um dobrando e o outro nao.
    ///
    /// E por isso que os indices internos existem: se dividissem o espaco de
    /// chaves, `fenix` dobrada e `Fênix` crua cairiam na mesma faixa e um
    /// acharia o que era do outro.
    #[test]
    fn dois_indices_com_dobra_diferente_nao_se_misturam() {
        let (mut f, _) = com_dobra("mistura", vec![true, false]);
        f.indexar(0, 1, "a Fênix").unwrap();
        f.indexar(1, 1, "a Fênix").unwrap();

        // O que dobra acha pelas duas grafias.
        assert_eq!(f.procurar(0, "fenix").unwrap().rowids, vec![1]);
        assert_eq!(f.procurar(0, "Fênix").unwrap().rowids, vec![1]);
        // O que nao dobra acha so pela grafia exata.
        assert_eq!(f.procurar(1, "Fênix").unwrap().rowids, vec![1]);
        assert!(
            f.procurar(1, "fenix").unwrap().rowids.is_empty(),
            "sem dobra, `fenix` nao pode achar `Fênix` -- senao a escolha do \
             dono nao teve efeito nenhum"
        );
    }

    /// O falso positivo que a chave de largura fixa criaria sozinha.
    ///
    /// # A primeira versao deste teste PASSAVA COM O DEFEITO REPOSTO
    ///
    /// Ela usava `transportadora` e `transportadoras`. As duas cabem inteiras
    /// na chave, entao **nunca truncam** -- o zero do preenchimento ja as
    /// separava, e o byte de comprimento nunca era exercitado. *Teste que
    /// passa por engano e pior que teste que falta.*
    #[test]
    fn prefixo_igual_com_tamanho_diferente_nao_colide() {
        let (mut f, _) = novo("prefixo");
        let cabe = f.termo_len();
        let base = "a".repeat(cabe);
        let uma = format!("{base}x");
        let outra = format!("{base}xy");
        assert!(uma.len() > cabe && outra.len() > cabe, "tem de truncar");

        f.indexar(0, 1, &uma).unwrap();
        f.indexar(0, 2, &outra).unwrap();
        assert_eq!(
            f.procurar(0, &uma).unwrap().rowids,
            vec![1],
            "sem o byte de comprimento na chave, esta busca acha as DUAS"
        );
        assert_eq!(f.procurar(0, &outra).unwrap().rowids, vec![2]);
    }

    /// A honestidade da §7.1: a duvida vira BANDEIRA, e nao resposta errada.
    #[test]
    fn palavra_longa_pede_conferencia_e_a_curta_nao() {
        let (mut f, _) = novo("conferir");
        let longa = "a".repeat(f.termo_len() + 5);
        f.indexar(0, 1, &longa).unwrap();
        let achado = f.procurar(0, &longa).unwrap();
        assert_eq!(achado.rowids, vec![1]);
        assert!(achado.conferir, "palavra longa tem de pedir conferencia");
        assert!(
            !f.procurar(0, "curta").unwrap().conferir,
            "palavra curta e exata, e conferir custaria ler a linha a toa"
        );
    }

    #[test]
    fn desindexar_tira_a_linha_e_deixa_as_outras() {
        let (mut f, _) = novo("desindexar");
        f.indexar(0, 1, "pedido fenix").unwrap();
        f.indexar(0, 2, "pedido comum").unwrap();
        assert_eq!(f.procurar(0, "pedido").unwrap().rowids, vec![1, 2]);
        assert_eq!(f.desindexar(0, 1, "pedido fenix").unwrap(), 2);
        assert_eq!(f.procurar(0, "pedido").unwrap().rowids, vec![2]);
        assert!(f.procurar(0, "fenix").unwrap().rowids.is_empty());
    }

    #[test]
    fn palavra_repetida_na_linha_vira_uma_chave_so() {
        let (mut f, _) = novo("repetida");
        assert_eq!(f.indexar(0, 1, "pedido pedido pedido").unwrap(), 1);
        assert_eq!(f.qtd_chaves(0), 1);
        assert_eq!(f.procurar(0, "pedido").unwrap().rowids, vec![1]);
    }

    #[test]
    fn sobrevive_a_fechar_e_abrir() {
        let (mut f, caminho) = novo("reabrir");
        f.indexar(0, 42, "a fenix guardada").unwrap();
        f.fechar().unwrap();
        drop(f);
        let mut g = FtsFile::abrir(&caminho, vec![true]).unwrap();
        assert_eq!(g.procurar(0, "fenix").unwrap().rowids, vec![42]);
    }

    /// Abrir com uma lista de tamanho diferente e DIVERGENCIA, e ela grita.
    ///
    /// O `.fts` e derivado: quem manda e o esquema da tabela. Se ele declarar
    /// dois indices de texto e o arquivo tiver um, seguir em frente indexaria
    /// no lugar errado -- e um indice que aponta o que nao devia e pior que
    /// indice nenhum.
    #[test]
    fn abrir_com_quantidade_diferente_recusa() {
        let (f, caminho) = novo("divergente");
        drop(f);
        let e = match FtsFile::abrir(&caminho, vec![true, true]) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("abrir com quantidade diferente tinha de recusar"),
        };
        assert!(e.contains("reindexar"), "{e}");
    }

    #[test]
    fn criar_por_cima_recusa_em_vez_de_apagar() {
        let (f, caminho) = novo("porcima");
        drop(f);
        assert!(FtsFile::criar(&caminho, vec![true]).is_err());
    }
}

//! Conjunto de volumes de um arquivo paginado.
//!
//! Uma tabela grande se parte em `Tabela_001.reg`, `Tabela_002.reg`, ... Este
//! modulo esconde essa divisao: quem chama pede "leia tantos bytes no offset X
//! do volume N" e nao precisa saber quantos arquivos existem nem quais estao
//! abertos.
//!
//! # Abertura preguicosa
//!
//! Uma tabela de 999 volumes nao pode manter 999 descritores de arquivo
//! abertos. Os volumes sao abertos sob demanda e mantidos num cache LRU; o
//! menos usado e fechado quando o teto e atingido. E o mesmo *lazy open* que o
//! `FileManager` do Clarion(R) faz.

use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::paginacao::Paginacao;

/// Quantos volumes ficam abertos ao mesmo tempo.
pub const LIMITE_ABERTOS_PADRAO: usize = 64;

pub struct Volumes {
    diretorio: PathBuf,
    nome: String,
    ext: &'static str,
    paginacao: Paginacao,
    abertos: HashMap<u32, File>,
    ordem: VecDeque<u32>,
    limite: usize,
    /// Espelho do conjunto, com outra extensao. Toda escrita vai para os dois.
    ///
    /// A leitura NAO consulta o espelho: quem le vai ao arquivo principal, e
    /// so quem descobre corrupcao pede a segunda chance de proposito. Ler
    /// sempre dos dois dobraria o custo de toda leitura para pagar por um caso
    /// que quase nunca acontece.
    espelho: Option<Box<Volumes>>,
}

impl Volumes {
    pub fn novo(
        diretorio: impl AsRef<Path>,
        nome: impl Into<String>,
        ext: &'static str,
        paginacao: Paginacao,
    ) -> Volumes {
        Volumes {
            diretorio: diretorio.as_ref().to_path_buf(),
            nome: nome.into(),
            ext,
            paginacao,
            abertos: HashMap::new(),
            ordem: VecDeque::new(),
            limite: LIMITE_ABERTOS_PADRAO,
            espelho: None,
        }
    }

    /// Liga o espelho: um conjunto irmao com outra extensao.
    ///
    /// # O que o espelho protege, e o que nao protege
    ///
    /// Protege contra o dado ficar RUIM: escrita pela metade, bit virado,
    /// defeito nosso que estraga um slot. Nesses casos o outro lado ainda tem
    /// a versao boa, e o `reparar` a devolve.
    ///
    /// NAO protege contra o disco morrer. Os dois arquivos estao no mesmo
    /// disco, no mesmo diretorio. Quem precisa sobreviver a perda do disco
    /// precisa de backup em outro lugar -- e por isso o backup existe
    /// separado, e nao e a mesma coisa.
    pub fn com_espelho(mut self, ext: &'static str) -> Volumes {
        let mut e = Volumes::novo(&self.diretorio, self.nome.clone(), ext, self.paginacao);
        e.limite = self.limite;
        self.espelho = Some(Box::new(e));
        self
    }

    pub fn tem_espelho(&self) -> bool {
        self.espelho.is_some()
    }

    /// Le do ESPELHO, quando o principal falhou. A segunda chance.
    pub fn ler_do_espelho(&mut self, volume: u32, offset: u64, buf: &mut [u8]) -> Result<()> {
        match &mut self.espelho {
            None => Err(PhxError::NaoEncontrado(
                "esta tabela nao tem espelho".into(),
            )),
            Some(e) => e.ler(volume, offset, buf),
        }
    }

    /// Escreve SO no principal, sem tocar no espelho.
    ///
    /// Existe para o reparo e para o teste que precisa estragar um lado so.
    /// Fora disso ninguem deve usar: escrita que nao vai aos dois lugares e
    /// exatamente o que o espelho existe para evitar.
    pub fn escrever_so_no_principal(&mut self, volume: u32, offset: u64, buf: &[u8]) -> Result<()> {
        let f = self.arquivo(volume, true)?;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(buf)?;
        Ok(())
    }

    /// Tamanho do volume no espelho. Zero quando ele ainda nao existe.
    pub fn tamanho_do_espelho(&mut self, volume: u32) -> Result<u64> {
        match &mut self.espelho {
            None => Ok(0),
            Some(e) => {
                if e.existe(volume) {
                    e.tamanho(volume)
                } else {
                    Ok(0)
                }
            }
        }
    }

    /// Copia um trecho do principal para o espelho, para o reparo inverso.
    pub fn escrever_no_espelho(&mut self, volume: u32, offset: u64, buf: &[u8]) -> Result<()> {
        match &mut self.espelho {
            None => Err(PhxError::NaoEncontrado(
                "esta tabela nao tem espelho".into(),
            )),
            Some(e) => {
                e.garantir(volume)?;
                e.escrever(volume, offset, buf)
            }
        }
    }

    /// Caminho de um volume. Sem paginacao o sufixo e vazio.
    pub fn caminho(&self, volume: u32) -> PathBuf {
        self.diretorio.join(format!(
            "{}{}.{}",
            self.nome,
            self.paginacao.sufixo(volume),
            self.ext
        ))
    }

    pub fn existe(&self, volume: u32) -> bool {
        self.caminho(volume).exists()
    }

    pub fn paginacao(&self) -> Paginacao {
        self.paginacao
    }

    pub fn nome(&self) -> &str {
        &self.nome
    }

    pub fn diretorio(&self) -> &Path {
        &self.diretorio
    }

    /// Volumes que existem em disco, em ordem crescente.
    pub fn existentes(&self) -> Vec<u32> {
        if !self.paginacao.ligada() {
            return if self.existe(1) { vec![1] } else { vec![] };
        }
        (1..=self.paginacao.max_arquivos)
            .filter(|v| self.existe(*v))
            .collect()
    }

    fn registrar_uso(&mut self, volume: u32) {
        if let Some(pos) = self.ordem.iter().position(|v| *v == volume) {
            self.ordem.remove(pos);
        }
        self.ordem.push_back(volume);
    }

    fn fechar_menos_usado(&mut self) {
        while self.abertos.len() >= self.limite {
            match self.ordem.pop_front() {
                Some(v) => {
                    self.abertos.remove(&v);
                }
                None => break,
            }
        }
    }

    fn arquivo(&mut self, volume: u32, criar: bool) -> Result<&mut File> {
        if !self.abertos.contains_key(&volume) {
            let caminho = self.caminho(volume);
            if !criar && !caminho.exists() {
                return Err(PhxError::NaoEncontrado(format!(
                    "volume {volume} nao existe: {}",
                    caminho.display()
                )));
            }
            self.fechar_menos_usado();
            let f = OpenOptions::new()
                .read(true)
                .write(true)
                .create(criar)
                .open(&caminho)?;
            self.abertos.insert(volume, f);
        }
        self.registrar_uso(volume);
        Ok(self.abertos.get_mut(&volume).expect("acabou de ser aberto"))
    }

    /// Cria o volume zerado. Falha se ja existir.
    pub fn criar(&mut self, volume: u32) -> Result<()> {
        let caminho = self.caminho(volume);
        if caminho.exists() {
            return Err(PhxError::Esquema(format!(
                "{} ja existe",
                caminho.display()
            )));
        }
        self.fechar_menos_usado();
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&caminho)?;
        self.abertos.insert(volume, f);
        self.registrar_uso(volume);
        Ok(())
    }

    /// Cria o volume se ainda nao existir. Devolve `true` se criou agora.
    pub fn garantir(&mut self, volume: u32) -> Result<bool> {
        if self.existe(volume) {
            self.arquivo(volume, false)?;
            Ok(false)
        } else {
            self.criar(volume)?;
            Ok(true)
        }
    }

    pub fn ler(&mut self, volume: u32, offset: u64, buf: &mut [u8]) -> Result<()> {
        let f = self.arquivo(volume, false)?;
        f.seek(SeekFrom::Start(offset))?;
        f.read_exact(buf)?;
        Ok(())
    }

    pub fn escrever(&mut self, volume: u32, offset: u64, buf: &[u8]) -> Result<()> {
        let f = self.arquivo(volume, true)?;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(buf)?;
        // O espelho recebe a mesma coisa, no mesmo lugar. Falhar aqui NAO
        // desfaz a escrita boa: o principal ja tem o dado, e um espelho
        // defasado e melhor do que uma gravacao recusada.
        if let Some(e) = &mut self.espelho {
            e.garantir(volume)?;
            let g = e.arquivo(volume, true)?;
            g.seek(SeekFrom::Start(offset))?;
            g.write_all(buf)?;
        }
        Ok(())
    }

    /// Escreve dois blocos seguidos (cabecalho e conteudo) sem reposicionar.
    pub fn escrever_par(
        &mut self,
        volume: u32,
        offset: u64,
        cabecalho: &[u8],
        conteudo: &[u8],
    ) -> Result<()> {
        let f = self.arquivo(volume, true)?;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(cabecalho)?;
        f.write_all(conteudo)?;
        if let Some(e) = &mut self.espelho {
            e.garantir(volume)?;
            let g = e.arquivo(volume, true)?;
            g.seek(SeekFrom::Start(offset))?;
            g.write_all(cabecalho)?;
            g.write_all(conteudo)?;
        }
        Ok(())
    }

    pub fn tamanho(&mut self, volume: u32) -> Result<u64> {
        let f = self.arquivo(volume, false)?;
        Ok(f.metadata()?.len())
    }

    pub fn definir_tamanho(&mut self, volume: u32, tamanho: u64) -> Result<()> {
        let f = self.arquivo(volume, true)?;
        f.set_len(tamanho)?;
        if let Some(e) = &mut self.espelho {
            e.garantir(volume)?;
            e.arquivo(volume, true)?.set_len(tamanho)?;
        }
        Ok(())
    }

    pub fn sincronizar(&mut self) -> Result<()> {
        for f in self.abertos.values_mut() {
            f.flush()?;
            f.sync_all()?;
        }
        if let Some(e) = &mut self.espelho {
            e.sincronizar()?;
        }
        Ok(())
    }

    /// Fecha todos os descritores. Usado antes de apagar arquivos.
    pub fn fechar_todos(&mut self) {
        self.abertos.clear();
        self.ordem.clear();
        if let Some(e) = &mut self.espelho {
            e.fechar_todos();
        }
    }

    /// Apaga todos os volumes do conjunto. Usado pelo reindex, que recria o
    /// `.ndx` do zero.
    pub fn apagar_tudo(&mut self) -> Result<()> {
        let volumes = self.existentes();
        self.fechar_todos();
        for v in volumes {
            std::fs::remove_file(self.caminho(v))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_temp(rotulo: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("phxsql-vol-{}-{rotulo}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn sem_paginacao_usa_arquivo_unico_sem_sufixo() {
        let d = dir_temp("unico");
        let v = Volumes::novo(&d, "cadastroClientes", "reg", Paginacao::DESLIGADA);
        assert_eq!(
            v.caminho(1).file_name().unwrap().to_string_lossy(),
            "cadastroClientes.reg"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn com_paginacao_usa_sufixo_numerado() {
        let d = dir_temp("paginado");
        let p = Paginacao::nova(1_000, 999).unwrap();
        let v = Volumes::novo(&d, "cadastroClientes", "reg", p);
        assert_eq!(
            v.caminho(1).file_name().unwrap().to_string_lossy(),
            "cadastroClientes_001.reg"
        );
        assert_eq!(
            v.caminho(42).file_name().unwrap().to_string_lossy(),
            "cadastroClientes_042.reg"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn le_e_escreve_em_volumes_diferentes() {
        let d = dir_temp("rw");
        let p = Paginacao::nova(10, 99).unwrap();
        let mut v = Volumes::novo(&d, "t", "reg", p);

        v.escrever(1, 0, b"volume um").unwrap();
        v.escrever(3, 0, b"volume tres").unwrap();

        let mut buf = vec![0u8; 9];
        v.ler(1, 0, &mut buf).unwrap();
        assert_eq!(&buf, b"volume um");

        let mut buf = vec![0u8; 11];
        v.ler(3, 0, &mut buf).unwrap();
        assert_eq!(&buf, b"volume tres");

        assert_eq!(v.existentes(), vec![1, 3]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn cache_lru_nao_estoura_o_limite_de_descritores() {
        let d = dir_temp("lru");
        let p = Paginacao::nova(10, 999).unwrap();
        let mut v = Volumes::novo(&d, "t", "reg", p);
        v.limite = 4;

        for vol in 1..=20u32 {
            v.escrever(vol, 0, format!("vol {vol}").as_bytes()).unwrap();
            assert!(
                v.abertos.len() <= 4,
                "abriu {} descritores, limite e 4",
                v.abertos.len()
            );
        }
        // Todos continuam legiveis depois de terem sido fechados e reabertos.
        for vol in 1..=20u32 {
            let esperado = format!("vol {vol}");
            let mut buf = vec![0u8; esperado.len()];
            v.ler(vol, 0, &mut buf).unwrap();
            assert_eq!(String::from_utf8(buf).unwrap(), esperado);
        }
        assert_eq!(v.existentes().len(), 20);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn volume_inexistente_e_erro_na_leitura() {
        let d = dir_temp("faltando");
        let mut v = Volumes::novo(&d, "t", "reg", Paginacao::nova(10, 99).unwrap());
        let mut buf = [0u8; 4];
        assert!(v.ler(7, 0, &mut buf).is_err());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn apagar_tudo_limpa_o_conjunto() {
        let d = dir_temp("apagar");
        let mut v = Volumes::novo(&d, "t", "ndx", Paginacao::nova(10, 99).unwrap());
        for vol in 1..=5u32 {
            v.escrever(vol, 0, b"x").unwrap();
        }
        assert_eq!(v.existentes().len(), 5);
        v.apagar_tudo().unwrap();
        assert!(v.existentes().is_empty());
        std::fs::remove_dir_all(&d).unwrap();
    }
}

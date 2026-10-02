//! `.seq` -- a sequencia NOMEADA, fora de qualquer tabela (pedido 229,
//! `docs/AUTONUMBER.md` §B.2.4 e §C.5).
//!
//! # O que e, e o que NAO e
//!
//! A `Sequence` de coluna continua morando no cabecalho do `.reg` da tabela
//! dela, e continua sendo de graca -- 0,50 µs de `write` dentro do cabecalho
//! que a insercao grava de qualquer jeito. Esta e OUTRA coisa: um contador
//! com nome proprio, que ninguem insere -- pede-se o proximo numero e ele
//! vem, durado em disco antes de a resposta sair. E o `CREATE SEQUENCE` do
//! PostgreSQL e do MariaDB, decidido pela matriz dos quatro motores (PG 4 +
//! MariaDB 3 = 7 tem; MySQL 2 + SQLite 1 = 3 nao tem).
//!
//! # O arquivo: 256 bytes, dois slots de 128, e um arquivo por sequencia
//!
//! Cada slot tem a anatomia do cabecalho do `.reg`: assinatura, versao,
//! campos de largura fixa e um CRC-32 no fim. Nao precisa de nenhuma peca
//! nova, e e o que faz o `backup` copia-lo sem saber que ele existe -- o
//! `listar` do backup leva todo arquivo da pasta.
//!
//! Sao DOIS slots porque a gravacao e NO LUGAR: cada `proximo` escreve o slot
//! que nao e o vigente e faz um `fdatasync`. Medido em 02/10/2026 no disco
//! de teste, isto custa **147 µs** por numero contra **771 µs** do
//! `gravar_duravel` (temporario + `fsync` + `rename` + `fsync` da pasta) --
//! 5,2x. O preco de escrever no lugar e a escrita rasgada, e os dois slots
//! pagam esse preco: uma queda no meio da gravacao estraga no maximo o slot
//! que estava sendo escrito, e o outro -- o vigente ate entao -- continua
//! inteiro com CRC valido. Na abertura vale o slot valido de maior
//! `geracao`. E o slot anterior nunca entrega numero repetido, porque o
//! `proximo` gravado e sempre o que ainda NAO saiu: a queda custa um
//! buraco, nunca uma repeticao.
//!
//! A criacao, essa sim, paga o caminho inteiro -- `fsync` do arquivo no
//! descritor que o escreveu e depois `fsync` da pasta --, porque ai a
//! entrada da pasta e nova. Pelo catalogo o descritor vai no
//! `PorSincronizar`, para o disco ser esperado fora da trava global.
//!
//! # Por que sem cache, e o preco disso
//!
//! O MariaDB escapa do `fsync` por numero com `CACHE 1000`, e a documentacao
//! dele diz o preco: desligar o servidor descarta o cache e deixa buracos. O
//! PostgreSQL nasce com `CACHE 1`. Pela regua (4 x 3) e pelo caso de uso do
//! dono -- numeracao de documento fiscal, onde buraco nao e aceitavel -- o
//! cache NAO EXISTE aqui: cada `proximo` vai ao disco. O teste
//! `custo_de_um_proximo` imprime o custo, nao afirma -- numero de disco de
//! teste nao e numero de bancada.
//!
//! # O que NAO replica
//!
//! O arquivo nao entra no diario de tabela nenhuma, entao a replicacao nao o
//! ve. E a regua de novo: a replicacao logica do PostgreSQL NAO replica
//! sequencias (documentado como limitacao), a do MariaDB replica -- 4 x 3,
//! nao replica. Num cluster `multi`, cada no tem o SEU arquivo, e quem
//! precisa de faixas disjuntas as declara no `inicio`/`passo` de cada no, como
//! o `auto_increment_offset`/`auto_increment_increment` do MariaDB.

use std::path::{Path, PathBuf};

use phxsql_core::crc::crc32;
use phxsql_core::error::{PhxError, Result};

use crate::catalogo::validar_nome;

/// Extensao do arquivo. Nao esta em `Database::EXTENSOES_TODAS` de
/// proposito: o `excluir_tabela` varre aquelas, e uma sequencia com o nome
/// de uma tabela nao e dela.
pub const EXTENSAO: &str = "seq";

const ASSINATURA: &[u8; 4] = b"PSEQ";
const VERSAO: u16 = 1;
/// Tamanho de um slot. Cabe num setor: a escrita e uma so.
pub const SLOT: usize = 128;
/// Tamanho fixo do arquivo: os dois slots.
pub const TAMANHO: usize = 2 * SLOT;
const SINAL_CICLO: u16 = 1;
const SINAL_ESGOTADA: u16 = 2;

/// Como uma sequencia nasce. Os `None` viram o padrao do PostgreSQL e do
/// MariaDB, que convergem: passo 1; minimo 1 e maximo `i64::MAX` quando sobe,
/// `i64::MIN` e -1 quando desce; inicio no minimo (subindo) ou no maximo
/// (descendo); sem ciclo.
#[derive(Debug, Clone, Copy, Default)]
pub struct Definicao {
    pub inicio: Option<i64>,
    pub passo: Option<i64>,
    pub minimo: Option<i64>,
    pub maximo: Option<i64>,
    pub ciclo: bool,
}

/// O estado gravado. `proximo` e o numero que AINDA NAO SAIU -- e e isso que
/// faz a queda entre o `fsync` e a resposta custar um buraco, nunca um
/// numero repetido.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estado {
    pub proximo: i64,
    pub inicio: i64,
    pub passo: i64,
    pub minimo: i64,
    pub maximo: i64,
    pub ciclo: bool,
    /// O ultimo numero da faixa ja saiu e nao ha ciclo: o proximo `proximo`
    /// e erro, ate um `ajustar`.
    pub esgotada: bool,
    /// Quantos numeros ja sairam deste arquivo. Observabilidade: aparece na
    /// listagem, e e o que diz se uma sequencia e usada.
    pub entregues: u64,
    /// Sobe a cada gravacao. E o que escolhe o slot vigente na abertura, e
    /// tambem qual slot a proxima gravacao usa (`geracao % 2`).
    pub geracao: u64,
}

impl Estado {
    fn de(d: Definicao) -> Result<Estado> {
        let passo = d.passo.unwrap_or(1);
        if passo == 0 {
            return Err(PhxError::Esquema(
                "o passo da sequencia nao pode ser 0: ela nunca sairia do lugar".into(),
            ));
        }
        let sobe = passo > 0;
        let minimo = d.minimo.unwrap_or(if sobe { 1 } else { i64::MIN });
        let maximo = d.maximo.unwrap_or(if sobe { i64::MAX } else { -1 });
        if minimo > maximo {
            return Err(PhxError::Esquema(format!(
                "o minimo da sequencia ({minimo}) e maior que o maximo ({maximo})"
            )));
        }
        let inicio = d.inicio.unwrap_or(if sobe { minimo } else { maximo });
        if inicio < minimo || inicio > maximo {
            return Err(PhxError::Esquema(format!(
                "o inicio da sequencia ({inicio}) esta fora da faixa {minimo}..{maximo}"
            )));
        }
        Ok(Estado {
            proximo: inicio,
            inicio,
            passo,
            minimo,
            maximo,
            ciclo: d.ciclo,
            esgotada: false,
            entregues: 0,
            geracao: 0,
        })
    }

    fn bytes(&self) -> [u8; SLOT] {
        let mut b = [0u8; SLOT];
        b[0..4].copy_from_slice(ASSINATURA);
        b[4..6].copy_from_slice(&VERSAO.to_le_bytes());
        let mut sinais = 0u16;
        if self.ciclo {
            sinais |= SINAL_CICLO;
        }
        if self.esgotada {
            sinais |= SINAL_ESGOTADA;
        }
        b[6..8].copy_from_slice(&sinais.to_le_bytes());
        b[8..16].copy_from_slice(&self.proximo.to_le_bytes());
        b[16..24].copy_from_slice(&self.inicio.to_le_bytes());
        b[24..32].copy_from_slice(&self.passo.to_le_bytes());
        b[32..40].copy_from_slice(&self.minimo.to_le_bytes());
        b[40..48].copy_from_slice(&self.maximo.to_le_bytes());
        b[48..56].copy_from_slice(&self.entregues.to_le_bytes());
        b[56..64].copy_from_slice(&self.geracao.to_le_bytes());
        let crc = crc32(&b[..SLOT - 4]);
        b[SLOT - 4..].copy_from_slice(&crc.to_le_bytes());
        b
    }

    /// O slot valido de maior geracao, ou o motivo de nenhum servir. Um
    /// slot invalido ao lado de um valido NAO e erro: e a escrita rasgada
    /// que os dois slots existem para sobreviver.
    fn do_arquivo(b: &[u8], onde: &Path) -> Result<Estado> {
        if b.len() != TAMANHO {
            return Err(PhxError::Corrompido(format!(
                "sequencia {}: tem {} bytes, e o formato tem {TAMANHO}",
                onde.display(),
                b.len()
            )));
        }
        let (a, c) = (
            Estado::de_slot(&b[..SLOT], onde),
            Estado::de_slot(&b[SLOT..], onde),
        );
        match (a, c) {
            (Ok(a), Ok(c)) => Ok(if a.geracao >= c.geracao { a } else { c }),
            (Ok(a), Err(_)) => Ok(a),
            (Err(_), Ok(c)) => Ok(c),
            (Err(e), Err(_)) => Err(e),
        }
    }

    fn de_slot(b: &[u8], onde: &Path) -> Result<Estado> {
        let corrompido =
            |motivo: &str| PhxError::Corrompido(format!("sequencia {}: {motivo}", onde.display()));
        if &b[0..4] != ASSINATURA {
            return Err(corrompido("a assinatura nao e PSEQ"));
        }
        let versao = u16::from_le_bytes([b[4], b[5]]);
        if versao != VERSAO {
            return Err(PhxError::VersaoNaoSuportada {
                arquivo: onde.display().to_string(),
                encontrada: versao,
                suportada: VERSAO,
            });
        }
        let gravado = u32::from_le_bytes([b[124], b[125], b[126], b[127]]);
        if crc32(&b[..SLOT - 4]) != gravado {
            return Err(corrompido("o CRC-32 nao confere"));
        }
        let i64_em = |i: usize| {
            let mut x = [0u8; 8];
            x.copy_from_slice(&b[i..i + 8]);
            i64::from_le_bytes(x)
        };
        let sinais = u16::from_le_bytes([b[6], b[7]]);
        let mut e = [0u8; 8];
        e.copy_from_slice(&b[48..56]);
        let mut g = [0u8; 8];
        g.copy_from_slice(&b[56..64]);
        let estado = Estado {
            proximo: i64_em(8),
            inicio: i64_em(16),
            passo: i64_em(24),
            minimo: i64_em(32),
            maximo: i64_em(40),
            ciclo: sinais & SINAL_CICLO != 0,
            esgotada: sinais & SINAL_ESGOTADA != 0,
            entregues: u64::from_le_bytes(e),
            geracao: u64::from_le_bytes(g),
        };
        // O CRC prova que os bytes sao os gravados; isto prova que os bytes
        // gravados fazem sentido -- um arquivo forjado com CRC certo e passo
        // 0 travaria o `proximo` num laco sem fim.
        if estado.passo == 0 || estado.minimo > estado.maximo {
            return Err(corrompido("passo 0 ou faixa invertida"));
        }
        Ok(estado)
    }
}

/// Uma sequencia aberta: o caminho, o descritor (e nele que o `fdatasync`
/// acontece) e o estado vigente.
#[derive(Debug)]
pub struct Sequencia {
    caminho: PathBuf,
    arquivo: std::fs::File,
    estado: Estado,
}

/// O caminho do arquivo de uma sequencia na pasta `dir`.
pub fn caminho_de(dir: &Path, nome: &str) -> PathBuf {
    dir.join(format!("{nome}.{EXTENSAO}"))
}

/// Existe uma sequencia com este nome na pasta?
pub fn existe(dir: &Path, nome: &str) -> bool {
    caminho_de(dir, nome).is_file()
}

/// Os nomes das sequencias da pasta, em ordem.
pub fn listar(dir: &Path) -> Result<Vec<String>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut nomes: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some(EXTENSAO) {
                return None;
            }
            p.file_stem().and_then(|s| s.to_str()).map(str::to_string)
        })
        .collect();
    nomes.sort();
    Ok(nomes)
}

impl Sequencia {
    /// Cria o arquivo. Recusa nome invalido, nome que ja existe e definicao
    /// que nao fecha (passo 0, faixa invertida, inicio fora dela).
    ///
    /// O nome compartilha o espaco das TABELAS, como no PostgreSQL e no
    /// MariaDB (onde a sequencia E uma relacao): quem chama confere a tabela
    /// homonima, porque e quem sabe listar tabelas.
    pub fn criar(dir: &Path, nome: &str, definicao: Definicao) -> Result<Sequencia> {
        // O ponto e o separador do nome qualificado (`schema.nome`), como na
        // tabela: aceita-lo aqui criaria um arquivo que o caminho qualificado
        // nunca acharia. A recusa mora no `criar_adiando_o_fsync`.
        let caminho = caminho_de(dir, nome);
        let (s, arquivo) = Self::criar_adiando_o_fsync(dir, nome, definicao)?;
        // A entrada da pasta e nova: o arquivo vai ao disco no descritor que
        // o escreveu, e depois a pasta onde ele nasceu.
        crate::sincronia::sync_all(&arquivo, &caminho)?;
        crate::sincronia::sincronizar_os_diretorios(&caminho, &caminho, true)?;
        Ok(s)
    }

    /// O [`Self::criar`] com o `fsync` por fazer: devolve o descritor que
    /// escreveu, para o `PorSincronizar` do catalogo leva-lo ao disco fora
    /// da trava global (pedido 589). O arquivo nasce inteiro, com a geracao
    /// 0 no slot 0 e o slot 1 vazio (CRC nao confere, e ignorado).
    pub fn criar_adiando_o_fsync(
        dir: &Path,
        nome: &str,
        definicao: Definicao,
    ) -> Result<(Sequencia, std::fs::File)> {
        use std::io::Write as _;
        validar_nome("sequencia", nome)?;
        if nome.contains('.') {
            return Err(PhxError::Esquema(format!(
                "{nome} nao serve como nome de sequencia: o ponto separa schema de nome"
            )));
        }
        let caminho = caminho_de(dir, nome);
        if caminho.exists() {
            return Err(PhxError::Duplicado(format!("a sequencia {nome} ja existe")));
        }
        let estado = Estado::de(definicao)?;
        let mut corpo = vec![0u8; TAMANHO];
        corpo[..SLOT].copy_from_slice(&estado.bytes());
        let mut arquivo = crate::util::recriar_do_banco(&caminho, true)?;
        arquivo.write_all(&corpo)?;
        let dup = arquivo.try_clone()?;
        Ok((
            Sequencia {
                caminho,
                arquivo,
                estado,
            },
            dup,
        ))
    }

    fn abrir_arquivo(caminho: &Path) -> Result<std::fs::File> {
        Ok(crate::util::opcoes_do_banco()
            .read(true)
            .write(true)
            .open(caminho)?)
    }

    /// Abre a sequencia, conferindo assinatura, versao e CRC-32.
    pub fn abrir(dir: &Path, nome: &str) -> Result<Sequencia> {
        validar_nome("sequencia", nome)?;
        let caminho = caminho_de(dir, nome);
        let bytes = std::fs::read(&caminho).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PhxError::NaoEncontrado(format!("sequencia {nome} nao existe"))
            } else {
                PhxError::Io(e)
            }
        })?;
        let estado = Estado::do_arquivo(&bytes, &caminho)?;
        let arquivo = Self::abrir_arquivo(&caminho)?;
        Ok(Sequencia {
            caminho,
            arquivo,
            estado,
        })
    }

    /// Apaga o arquivo. Devolve o caminho que saiu, para o `fsync` da pasta
    /// de quem chama (o `PorSincronizar::entradas_que_sairam`).
    pub fn excluir(dir: &Path, nome: &str) -> Result<PathBuf> {
        validar_nome("sequencia", nome)?;
        let caminho = caminho_de(dir, nome);
        std::fs::remove_file(&caminho).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PhxError::NaoEncontrado(format!("sequencia {nome} nao existe"))
            } else {
                PhxError::Io(e)
            }
        })?;
        Ok(caminho)
    }

    pub fn estado(&self) -> &Estado {
        &self.estado
    }

    pub fn caminho(&self) -> &Path {
        &self.caminho
    }

    /// O proximo numero, durado em disco ANTES de ser devolvido.
    ///
    /// Esgotou sem ciclo: erro, como o `nextval: reached maximum value` do
    /// PostgreSQL e o `Sequence has run out` do MariaDB, que convergem. Com
    /// ciclo, volta ao minimo (subindo) ou ao maximo (descendo).
    pub fn proximo(&mut self) -> Result<i64> {
        if self.estado.esgotada {
            return Err(PhxError::LimiteExcedido(format!(
                "a sequencia chegou ao {} da faixa ({}) e nao tem ciclo: ajuste-a ou \
                 recrie-a com ciclo",
                if self.estado.passo > 0 {
                    "maximo"
                } else {
                    "minimo"
                },
                if self.estado.passo > 0 {
                    self.estado.maximo
                } else {
                    self.estado.minimo
                }
            )));
        }
        let valor = self.estado.proximo;
        let dentro = |v: i64| v >= self.estado.minimo && v <= self.estado.maximo;
        match valor.checked_add(self.estado.passo) {
            Some(seguinte) if dentro(seguinte) => self.estado.proximo = seguinte,
            _ if self.estado.ciclo => {
                self.estado.proximo = if self.estado.passo > 0 {
                    self.estado.minimo
                } else {
                    self.estado.maximo
                }
            }
            _ => self.estado.esgotada = true,
        }
        self.estado.entregues += 1;
        // Grava antes de devolver: o numero so e de alguem depois de o disco
        // saber que ele saiu. Se a gravacao falhar, o estado em memoria e
        // descartado junto com o erro -- quem reabrir le o anterior.
        self.gravar()?;
        Ok(valor)
    }

    /// Poe o proximo numero onde o administrador mandou -- inclusive para
    /// tras, que e a porta de repetir numero, e por isso quem chama exige
    /// `administrar`. E o `setval` do PostgreSQL e o `ALTER SEQUENCE ...
    /// RESTART WITH` do MariaDB: dentro da faixa, qualquer valor.
    pub fn ajustar(&mut self, proximo: i64) -> Result<()> {
        if proximo < self.estado.minimo || proximo > self.estado.maximo {
            return Err(PhxError::Esquema(format!(
                "{proximo} esta fora da faixa da sequencia ({}..{})",
                self.estado.minimo, self.estado.maximo
            )));
        }
        self.estado.proximo = proximo;
        self.estado.esgotada = false;
        self.gravar()
    }

    /// Grava o estado no slot que NAO e o vigente e espera o disco. Se a
    /// escrita ou o `fdatasync` falharem, o estado em memoria e o erro saem
    /// juntos: quem reabrir le o slot anterior, que continua valido.
    fn gravar(&mut self) -> Result<()> {
        self.estado.geracao += 1;
        let slot = (self.estado.geracao % 2) * SLOT as u64;
        let bytes = self.estado.bytes();
        crate::util::escrever_em(&mut self.arquivo, slot, &bytes)?;
        crate::sincronia::sync_data(&self.arquivo, &self.caminho)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;

    fn pasta(rotulo: &str) -> DirTemp {
        DirTemp::novo(&format!("seq-{rotulo}"))
    }

    #[test]
    fn nasce_com_os_padroes_dos_dois_motores_que_a_tem() {
        let d = pasta("padrao");
        let s = Sequencia::criar(&d.0, "nf", Definicao::default()).unwrap();
        let e = *s.estado();
        assert_eq!((e.proximo, e.inicio, e.passo), (1, 1, 1));
        assert_eq!((e.minimo, e.maximo), (1, i64::MAX));
        assert!(!e.ciclo && !e.esgotada && e.entregues == 0);

        // Descendo, o espelho: inicio no maximo, faixa ate o MIN.
        let s = Sequencia::criar(
            &d.0,
            "desce",
            Definicao {
                passo: Some(-1),
                ..Definicao::default()
            },
        )
        .unwrap();
        let e = *s.estado();
        assert_eq!((e.proximo, e.minimo, e.maximo), (-1, i64::MIN, -1));
    }

    #[test]
    fn o_proximo_sai_durado_e_a_reabertura_continua_de_onde_parou() {
        let d = pasta("durado");
        let mut s = Sequencia::criar(&d.0, "nf", Definicao::default()).unwrap();
        assert_eq!(s.proximo().unwrap(), 1);
        assert_eq!(s.proximo().unwrap(), 2);
        drop(s);
        let mut s = Sequencia::abrir(&d.0, "nf").unwrap();
        assert_eq!(s.estado().entregues, 2);
        assert_eq!(s.proximo().unwrap(), 3);
        assert_eq!(
            std::fs::metadata(s.caminho()).unwrap().len(),
            TAMANHO as u64
        );
    }

    /// A escrita rasgada: um slot estragado ao lado de um valido nao e erro,
    /// e vale o valido de maior geracao.
    ///
    /// O cenario que isto simula e a queda NO MEIO da gravacao do terceiro
    /// `proximo`: o slot da geracao 3 fica rasgado, e o 3 NUNCA foi
    /// devolvido a ninguem (a resposta so sai depois do `fdatasync`). Entao
    /// voltar ao slot da geracao 2 -- que diz `proximo = 3` -- entrega o 3
    /// pela primeira vez. Buraco nenhum, repeticao nenhuma.
    #[test]
    fn um_slot_rasgado_nao_perde_a_sequencia_e_nao_repete() {
        let d = pasta("rasgado");
        let mut s = Sequencia::criar(&d.0, "nf", Definicao::default()).unwrap();
        assert_eq!(s.proximo().unwrap(), 1); // geracao 1, slot 1
        assert_eq!(s.proximo().unwrap(), 2); // geracao 2, slot 0
        assert_eq!(s.proximo().unwrap(), 3); // geracao 3, slot 1
        let c = s.caminho().to_path_buf();
        drop(s);
        let mut b = std::fs::read(&c).unwrap();
        b[SLOT + 10] ^= 0xff;
        std::fs::write(&c, &b).unwrap();
        let mut s = Sequencia::abrir(&d.0, "nf").unwrap();
        assert_eq!(s.estado().geracao, 2);
        assert_eq!(s.proximo().unwrap(), 3);
        // E com os dois validos, vale o de maior geracao -- nao o slot 0 por
        // ser o primeiro.
        assert_eq!(s.proximo().unwrap(), 4);
        drop(s);
        assert_eq!(Sequencia::abrir(&d.0, "nf").unwrap().estado().proximo, 5);
    }

    #[test]
    fn a_faixa_e_o_passo_mandam() {
        let d = pasta("faixa");
        let mut s = Sequencia::criar(
            &d.0,
            "par",
            Definicao {
                inicio: Some(2),
                passo: Some(2),
                maximo: Some(6),
                ..Definicao::default()
            },
        )
        .unwrap();
        assert_eq!(s.proximo().unwrap(), 2);
        assert_eq!(s.proximo().unwrap(), 4);
        assert_eq!(s.proximo().unwrap(), 6);
        // Esgotou sem ciclo: erro, e o arquivo diz que esgotou.
        let e = s.proximo().unwrap_err();
        assert!(matches!(e, PhxError::LimiteExcedido(_)), "{e}");
        assert!(Sequencia::abrir(&d.0, "par").unwrap().estado().esgotada);
        // O ajuste reabre a faixa.
        s.ajustar(4).unwrap();
        assert_eq!(s.proximo().unwrap(), 4);
        assert!(s.ajustar(7).is_err(), "fora da faixa");
    }

    #[test]
    fn o_ciclo_volta_ao_minimo_e_o_overflow_nao_derruba() {
        let d = pasta("ciclo");
        let mut s = Sequencia::criar(
            &d.0,
            "c",
            Definicao {
                minimo: Some(1),
                maximo: Some(2),
                ciclo: true,
                ..Definicao::default()
            },
        )
        .unwrap();
        assert_eq!(
            (0..5).map(|_| s.proximo().unwrap()).collect::<Vec<_>>(),
            vec![1, 2, 1, 2, 1]
        );
        // Perto do teto do i64 o `checked_add` e o que impede o panico.
        let mut s = Sequencia::criar(
            &d.0,
            "teto",
            Definicao {
                inicio: Some(i64::MAX),
                ..Definicao::default()
            },
        )
        .unwrap();
        assert_eq!(s.proximo().unwrap(), i64::MAX);
        assert!(s.proximo().is_err());
    }

    #[test]
    fn definicao_que_nao_fecha_e_recusada_antes_de_tocar_o_disco() {
        let d = pasta("recusa");
        for def in [
            Definicao {
                passo: Some(0),
                ..Definicao::default()
            },
            Definicao {
                minimo: Some(10),
                maximo: Some(5),
                ..Definicao::default()
            },
            Definicao {
                inicio: Some(0),
                ..Definicao::default()
            },
        ] {
            assert!(Sequencia::criar(&d.0, "x", def).is_err(), "{def:?}");
            assert!(!existe(&d.0, "x"));
        }
        assert!(Sequencia::criar(&d.0, "a.b", Definicao::default()).is_err());
        assert!(Sequencia::criar(&d.0, "../x", Definicao::default()).is_err());
        Sequencia::criar(&d.0, "x", Definicao::default()).unwrap();
        let e = Sequencia::criar(&d.0, "x", Definicao::default()).unwrap_err();
        assert!(matches!(e, PhxError::Duplicado(_)), "{e}");
    }

    #[test]
    fn arquivo_mexido_nao_abre() {
        let d = pasta("crc");
        Sequencia::criar(&d.0, "nf", Definicao::default()).unwrap();
        let c = caminho_de(&d.0, "nf");
        let mut b = std::fs::read(&c).unwrap();
        // Os DOIS slots mexidos: ai nao ha para onde voltar.
        b[8] ^= 1;
        b[SLOT + 8] ^= 1;
        std::fs::write(&c, &b).unwrap();
        let e = Sequencia::abrir(&d.0, "nf").unwrap_err();
        assert!(matches!(e, PhxError::Corrompido(_)), "{e}");
        std::fs::write(&c, b"PSEQ").unwrap();
        assert!(Sequencia::abrir(&d.0, "nf").is_err());
        assert!(matches!(
            Sequencia::abrir(&d.0, "nada").unwrap_err(),
            PhxError::NaoEncontrado(_)
        ));
    }

    #[test]
    fn listar_e_excluir() {
        let d = pasta("lista");
        Sequencia::criar(&d.0, "b", Definicao::default()).unwrap();
        Sequencia::criar(&d.0, "a", Definicao::default()).unwrap();
        std::fs::write(d.0.join("t.reg"), b"nao e sequencia").unwrap();
        assert_eq!(listar(&d.0).unwrap(), vec!["a", "b"]);
        Sequencia::excluir(&d.0, "a").unwrap();
        assert_eq!(listar(&d.0).unwrap(), vec!["b"]);
        assert!(matches!(
            Sequencia::excluir(&d.0, "a").unwrap_err(),
            PhxError::NaoEncontrado(_)
        ));
    }

    /// Mede, nao afirma: o custo de um `proximo` neste disco. E o numero que
    /// o `docs/AUTONUMBER.md` §B.2.4 estimou em 83,5 µs de `fdatasync`; o
    /// motor daqui faz temporario + `fsync` + `rename` + `fsync` da pasta.
    #[test]
    fn custo_de_um_proximo() {
        let d = pasta("custo");
        let mut s = Sequencia::criar(&d.0, "nf", Definicao::default()).unwrap();
        let n = 200;
        let t = std::time::Instant::now();
        for _ in 0..n {
            s.proximo().unwrap();
        }
        let por = t.elapsed().as_micros() as f64 / n as f64;
        eprintln!("custo_de_um_proximo: {por:.1} us por numero ({n} numeros)");
        assert_eq!(s.estado().entregues, n);
    }
}

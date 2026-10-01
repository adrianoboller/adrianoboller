//! As chaves estrangeiras CONFERIDAS que as tabelas de um diretorio declaram
//! -- a pergunta da busca reversa da integridade («quem aponta para mim?»),
//! respondida sem reabrir o `.reg` de cada irma a cada exclusao (pedido 259).
//!
//! # Por que existe
//!
//! Medido em 01/10/2026 (`--example custo-do-excluir 200000 20000`): com a
//! tabela sozinha, listar o diretorio custava **5,99 us de 30,48** por
//! exclusao (19,7%); com 30 irmas sem chave nenhuma, abrir o `.reg` de cada
//! uma custava **418,98 us** -- 14,7x o excluir inteiro --, e 30 `openat`,
//! 60 `read` e 60 `statx` por exclusao. Tudo para descobrir, de novo, uma
//! resposta que ja estava escrita nos esquemas e que nao mudou.
//!
//! Decisao do dono, 17/09/2026: pular a busca quando NENHUMA tabela declara
//! chave apontando para esta, sem tocar formato e sem enfraquecer a regra
//! primordial -- quando ha filha declarada, a busca continua inteira.
//!
//! # Por que isto NAO e o catalogo reverso que a petrea recusou
//!
//! O catalogo recusado e GUARDADO e MANTIDO: cobra de toda criacao e
//! alteracao de tabela, inclusive das que nao tem chave. Aqui nao se mantem
//! nada: a resposta e lembrada junto do CARIMBO do arquivo de onde saiu, e
//! revalidada a cada pergunta por um `statx`. Quem cria ou altera tabela nao
//! paga nada e nao avisa ninguem -- o carimbo muda sozinho, porque o nucleo
//! o muda. E por isso vale tambem para OUTRO processo no mesmo diretorio (a
//! FFI embarcada), que era o defeito do catalogo em memoria do §24.5.
//!
//! # O carimbo, e por que ele nao confia no que acabou de mudar
//!
//! `(dispositivo, inode, tamanho, mtime, ctime)`. A regravacao do esquema no
//! lugar muda `mtime`/`ctime`; a troca por `rename` muda o inode; o `ctime`
//! ninguem forja (`utimes` o poe no agora). O buraco que sobra e o do
//! relogio GROSSO do nucleo: duas mudancas no mesmo tique deixam o mesmo
//! carimbo. Entao nada se lembra enquanto o carimbo for RECENTE --
//! [`MARGEM`] -- e o arquivo que muda o tempo todo e relido toda vez, como
//! antes. E o «racy» do indice do git, pelo mesmo motivo.
//!
//! A ordem e a outra metade: o carimbo se le ANTES do conteudo. Se o arquivo
//! mudar entre os dois, o que se guarda e o carimbo VELHO, e a pergunta
//! seguinte erra o casamento e rele -- o erro possivel e reler a mais, nunca
//! lembrar a menos.
//!
//! Fora do Unix nao ha inode nem `ctime` na `std` estavel: ali nada se
//! lembra, e a busca e a de sempre.
//!
//! O buraco que fica, nomeado: num sistema de arquivos de rede cujo servidor
//! tenha o relogio ATRASADO mais que a margem, um carimbo recem-gravado
//! parece velho daqui. O PhxSql nao promete diretorio de dados em rede, e
//! nenhum teste desta casa pega esse caso.

use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::schema::ForeignKey;

/// Quanto o carimbo tem de ser velho para se confiar nele. Dois tiques do
/// relogio grosso do nucleo sao milissegundos; o FAT grava `mtime` de 2 em 2
/// segundos. Tres segundos cobrem os dois com folga, e o preco de errar para
/// cima e so reler um arquivo que mudou ha pouco.
const MARGEM: Duration = Duration::from_secs(3);

/// Quantos diretorios se lembram ao mesmo tempo. Passou disso, esquece-se
/// tudo e recomeca: o servidor tem poucos diretorios de tabela, e quem cria
/// milhares (a suite de testes) nao pode fazer a memoria crescer sem fim.
const DIRETORIOS_MAX: usize = 256;

static LIDOS_DO_DISCO: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// So para os testes deste modulo: trocar a margem na PROPRIA thread,
    /// sem mexer na das outras. Ver [`com_margem`].
    static MARGEM_DA_THREAD: Cell<Option<Duration>> = const { Cell::new(None) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Carimbo {
    dispositivo: u64,
    inode: u64,
    tamanho: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}

impl Carimbo {
    #[cfg(unix)]
    fn de(caminho: &Path) -> Option<Carimbo> {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::metadata(caminho).ok()?;
        Some(Carimbo {
            dispositivo: m.dev(),
            inode: m.ino(),
            tamanho: m.size(),
            mtime: (m.mtime(), m.mtime_nsec()),
            ctime: (m.ctime(), m.ctime_nsec()),
        })
    }

    #[cfg(not(unix))]
    fn de(_: &Path) -> Option<Carimbo> {
        None
    }

    /// Velho o bastante para que uma mudanca futura caia noutro tique?
    fn confiavel(&self, agora: SystemTime) -> bool {
        let margem = MARGEM_DA_THREAD.with(Cell::get).unwrap_or(MARGEM);
        let Ok(agora) = agora.duration_since(SystemTime::UNIX_EPOCH) else {
            return false;
        };
        let limite = agora.saturating_sub(margem);
        [self.mtime, self.ctime].iter().all(|&(s, ns)| {
            s >= 0 && Duration::new(s as u64, ns.clamp(0, 999_999_999) as u32) < limite
        })
    }
}

/// O que se lembra de uma irma: as chaves conferidas que ela declara.
struct Vista {
    carimbo: Carimbo,
    chaves: Vec<ForeignKey>,
}

/// O que se lembra de um diretorio: a lista das tabelas e onde mora o
/// primeiro volume de cada uma (e de la que o esquema se le). O caminho se
/// acha so quando alguem pergunta por aquela irma -- a propria tabela de
/// quem pergunta nunca precisa dele.
struct Diretorio {
    carimbo: Carimbo,
    tabelas: Vec<(String, Option<PathBuf>)>,
    vistas: HashMap<String, Vista>,
}

fn memoria() -> &'static Mutex<HashMap<PathBuf, Diretorio>> {
    static M: OnceLock<Mutex<HashMap<PathBuf, Diretorio>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Quantos esquemas de irma ja se leram do DISCO neste processo. E o numero
/// que o teste e o medidor olham: com o carimbo valendo, ele para de subir.
pub fn esquemas_lidos_do_disco() -> u64 {
    LIDOS_DO_DISCO.load(Ordering::Relaxed)
}

/// As tabelas de `diretorio` (menos `eu`, que quem pergunta ja tem na mao) e
/// as chaves CONFERIDAS que cada uma declara, de qualquer destino. Quem
/// pergunta filtra pelo destino.
///
/// Mesma resposta da varredura antiga -- `catalogo::tabelas_em` e
/// `RegFile::abrir` de cada irma --, so que lembrada enquanto o carimbo do
/// arquivo nao mudar. Irma que nao abre RECUSA: ver [`abrir_irma`].
pub(crate) fn chaves_das_irmas(
    diretorio: &Path,
    eu: &str,
) -> Result<Vec<(String, Vec<ForeignKey>)>> {
    // O carimbo ANTES do conteudo, sempre: ver o topo do modulo.
    let carimbo_dir = Carimbo::de(diretorio);
    let agora = SystemTime::now();
    // A trava so para TIRAR a lembranca e, no fim, para DEVOLVE-LA: a
    // abertura das irmas e E/S, e nao se faz com a memoria do processo
    // trancada. Duas perguntas ao mesmo tempo no mesmo diretorio calculam as
    // duas, e a ultima a devolver fica -- as duas respostas sao validas.
    let guardado = memoria()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(diretorio)
        .filter(|d| Some(d.carimbo) == carimbo_dir);
    let (mut tabelas, mut entrada) = match (guardado, carimbo_dir) {
        (Some(mut d), _) => (std::mem::take(&mut d.tabelas), Some(d)),
        (None, c) => {
            let tabelas: Vec<(String, Option<PathBuf>)> = crate::catalogo::tabelas_em(diretorio)?
                .into_iter()
                .map(|n| (n, None))
                .collect();
            let entrada = c.filter(|c| c.confiavel(agora)).map(|c| Diretorio {
                carimbo: c,
                tabelas: Vec::new(),
                vistas: HashMap::new(),
            });
            (tabelas, entrada)
        }
    };

    let mut saida = Vec::with_capacity(tabelas.len());
    for (irma, caminho) in tabelas.iter_mut() {
        if irma == eu {
            continue;
        }
        let caminho = match caminho {
            Some(c) => &*c,
            // O diretorio listou um `.reg` desta irma e o primeiro volume nao
            // se acha: e irma que nao abre, e recusa como ela (pedido 631).
            None => match crate::reg::primeiro_volume(diretorio, irma) {
                Some(c) => &*caminho.insert(c),
                None => {
                    return Err(recusa_da_irma(
                        irma,
                        eu,
                        &PhxError::NaoEncontrado("o primeiro volume do .reg sumiu".into()),
                    ))
                }
            },
        };
        let carimbo = Carimbo::de(caminho);
        if let (Some(c), Some(e)) = (carimbo, entrada.as_ref()) {
            if let Some(v) = e.vistas.get(irma).filter(|v| v.carimbo == c) {
                saida.push((irma.clone(), v.chaves.clone()));
                continue;
            }
        }
        LIDOS_DO_DISCO.fetch_add(1, Ordering::Relaxed);
        let chaves: Vec<ForeignKey> = abrir_irma(diretorio, irma, eu)?
            .esquema()
            .chaves_estrangeiras()
            .iter()
            .filter(|fk| fk.verificar)
            .cloned()
            .collect();
        if let (Some(c), Some(e)) = (carimbo, entrada.as_mut()) {
            if c.confiavel(agora) {
                e.vistas.insert(
                    irma.clone(),
                    Vista {
                        carimbo: c,
                        chaves: chaves.clone(),
                    },
                );
            } else {
                e.vistas.remove(irma);
            }
        }
        saida.push((irma.clone(), chaves));
    }

    if let Some(mut e) = entrada {
        e.tabelas = tabelas;
        let mut mem = memoria().lock().unwrap_or_else(|e| e.into_inner());
        if mem.len() >= DIRETORIOS_MAX {
            mem.clear();
        }
        mem.insert(diretorio.to_path_buf(), e);
    }
    Ok(saida)
}

/// Abre a irma `irma` para a busca reversa de `eu` -- ou RECUSA dizendo qual
/// e por que (pedido 631).
///
/// # Por que a irma que nao abre recusa, e nao fica de fora
///
/// Ate 01/10/2026 ela ficava de fora, com o argumento de que «o defeito dela
/// e dela, e uma tabela quebrada nao pode trancar o banco inteiro». Mas a
/// pergunta que a busca reversa faz e «alguem aponta para mim?», e a irma
/// que nao abre e justamente a que nao responde: pula-la e responder «nao»
/// por ela. Com o `.reg` da filha ilegivel, a mae excluia a linha que tinha
/// filha -- a regra primordial furada por omissao (revisao SEC, M3). Falha
/// FECHADA: quem nao sabe se ha filha nao mata o pai.
///
/// # Por que isto nao vira recusa eterna
///
/// * A tabela em troca interrompida (pedido 625) ABRE: o `RegFile::abrir`
///   termina a troca decidida pelo `terminar_troca_interrompida`, e a sobra
///   da FASE A nao atrapalha a abertura.
/// * A irma quebrada sai pelo `excluir_tabela` DELA, que pula a propria
///   tabela na busca -- e o recado diz isso.
///
/// E a UNICA copia desta decisao: a busca reversa do `excluir` (este
/// modulo), a do `ao_alterar` (`Table::planejar_ao_alterar_com`) e a do
/// `excluir_tabela`/renomear (`Database::quem_aponta_para`) passam por aqui.
pub(crate) fn abrir_irma(diretorio: &Path, irma: &str, eu: &str) -> Result<crate::reg::RegFile> {
    crate::reg::RegFile::abrir(diretorio, irma).map_err(|e| recusa_da_irma(irma, eu, &e))
}

/// O recado da irma que nao abre. O erro dela vai junto como CAUSA: e ele
/// que diz o que consertar nela, e a frase de fora so diz por que a operacao
/// em `eu` parou.
fn recusa_da_irma(irma: &str, eu: &str, causa: &PhxError) -> PhxError {
    PhxError::Integridade(format!(
        "{eu}: a tabela {irma}, no mesmo diretorio, nao abre ({causa}), e sem o \
         esquema dela nao ha como saber se ela declara chave estrangeira para \
         {eu} -- nunca se mata o pai que pode ter filhos, entao a operacao fica \
         recusada ate {irma} abrir de novo (repare {irma}, ou apague-a se ela \
         nao tem conserto)"
    ))
}

/// Roda `f` com outra margem NESTA thread. So para teste: a margem de
/// verdade e o que impede o carimbo de um tique grosso de mentir.
#[cfg(test)]
fn com_margem<T>(margem: Duration, f: impl FnOnce() -> T) -> T {
    let antes = MARGEM_DA_THREAD.with(|m| m.replace(Some(margem)));
    let r = f();
    MARGEM_DA_THREAD.with(|m| m.set(antes));
    r
}

#[cfg(all(test, unix))]
mod testes {
    use super::*;
    use crate::table::Table;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
    use phxsql_core::types::ColumnType;
    use phxsql_core::value::Value;

    // O guarda da casa, que apaga no `Drop` (pedido 150).
    fn dir(rotulo: &str) -> crate::apoio_teste::DirTemp {
        crate::apoio_teste::DirTemp::novo(&format!("irmas-{rotulo}"))
    }

    fn mae() -> Schema {
        Schema::new(
            "mae",
            vec![Column::new("id", ColumnType::Int8).obrigatoria()],
            vec![IndexDef::new("pk", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap()
    }

    fn irma(nome: &str) -> Schema {
        Schema::new(
            nome,
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("mid", ColumnType::Int8),
            ],
            vec![
                IndexDef::new("pk", vec![IndexColumn::asc(0)]).unico(),
                IndexDef::new("por_mae", vec![IndexColumn::asc(1)]),
            ],
        )
        .unwrap()
    }

    /// Quantos esquemas se leram do disco durante `f`, NESTA thread de teste.
    /// O contador e do processo; os testes que o olham sao os desta casa e
    /// rodam um de cada vez pela trava abaixo.
    fn lidos_em(f: impl FnOnce()) -> u64 {
        let antes = esquemas_lidos_do_disco();
        f();
        esquemas_lidos_do_disco() - antes
    }

    static UM_DE_CADA_VEZ: Mutex<()> = Mutex::new(());

    /// **Pedido 259, prova real:** com o carimbo VELHO (margem zero nesta
    /// thread), a segunda exclusao numa tabela com tres irmas sem chave nao
    /// le esquema nenhum do disco -- a primeira le os tres.
    ///
    /// Reponha o defeito fazendo `chaves_das_irmas` reler sempre (a busca de
    /// antes): a segunda volta tambem le 3, e o `assert_eq!(.., 0)` cai.
    #[test]
    fn a_segunda_exclusao_nao_rele_o_esquema_das_irmas() {
        let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let d = dir("lembra");
        let mut m = Table::criar(&d.0, mae()).unwrap();
        for i in 1..=4 {
            m.inserir(&[Value::Int(i)]).unwrap();
        }
        for n in ["i1", "i2", "i3"] {
            Table::criar(&d.0, irma(n)).unwrap();
        }
        com_margem(Duration::ZERO, || {
            assert_eq!(lidos_em(|| assert!(m.excluir_de_vez(1, "t").unwrap())), 3);
            assert_eq!(lidos_em(|| assert!(m.excluir_de_vez(2, "t").unwrap())), 0);
        });
    }

    /// **O teste que importa: a chave declarada DEPOIS de lembrado o esquema
    /// tranca o pai.** A filha ja tem a linha que aponta para a mae 2, mas
    /// ainda sem chave declarada; a mae exclui a 1 e lembra o esquema dela
    /// (margem zero: o carimbo vale na hora). Depois a chave se declara -- o
    /// esquema se regrava NO LUGAR: mesmo inode, mesmo tamanho, so o
    /// `mtime`/`ctime` andam. A exclusao da mae 2 tem de recusar: a petrea da
    /// integridade.
    ///
    /// Reponha o defeito comparando o carimbo sem os tempos (so inode e
    /// tamanho): a lembranca velha diz «ninguem aponta», a mae 2 sai com
    /// filha, e o `unwrap_err` cai.
    #[test]
    fn a_chave_declarada_depois_tranca_o_pai() {
        use std::os::unix::fs::MetadataExt;
        let _t = UM_DE_CADA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let d = dir("tranca");
        let mut m = Table::criar(&d.0, mae()).unwrap();
        for i in 1..=4 {
            m.inserir(&[Value::Int(i)]).unwrap();
        }
        m.sincronizar().unwrap();
        let mut f = Table::criar(&d.0, irma("filha")).unwrap();
        f.inserir(&[Value::Int(10), Value::Int(2)]).unwrap();
        f.sincronizar().unwrap();
        let reg = d.0.join("filha.reg");
        let antes = std::fs::metadata(&reg).unwrap();

        com_margem(Duration::ZERO, || {
            assert!(m.excluir_de_vez(1, "t").unwrap());
            m.sincronizar().unwrap();
            // Um tique grosso do nucleo inteiro entre lembrar e mudar: o que
            // este teste prova e o carimbo, e nao a margem.
            std::thread::sleep(Duration::from_millis(50));
            f.redeclarar_chaves_estrangeiras(vec![ForeignKey::new(
                "fk_mae",
                vec![1],
                "mae",
                vec!["id".into()],
            )])
            .unwrap();
            let depois = std::fs::metadata(&reg).unwrap();
            assert_eq!(
                (antes.ino(), antes.size()),
                (depois.ino(), depois.size()),
                "a declaracao nao regravou no lugar: o teste nao prova o caso"
            );
            let e = m.excluir_de_vez(2, "t").unwrap_err().to_string();
            assert!(e.contains("filhas"), "{e}");
        });
    }

    /// O carimbo recente NAO se lembra: e o que segura o tique grosso.
    #[test]
    fn carimbo_recente_nao_e_confiavel_e_velho_e() {
        let c = Carimbo {
            dispositivo: 1,
            inode: 2,
            tamanho: 3,
            mtime: (1_000, 0),
            ctime: (1_000, 0),
        };
        let perto = SystemTime::UNIX_EPOCH + Duration::from_secs(1_001);
        let longe = SystemTime::UNIX_EPOCH + Duration::from_secs(1_010);
        assert!(!c.confiavel(perto));
        assert!(c.confiavel(longe));
        // Basta UM dos dois ser recente.
        let c2 = Carimbo {
            ctime: (1_009, 0),
            ..c
        };
        assert!(!c2.confiavel(longe));
    }
}

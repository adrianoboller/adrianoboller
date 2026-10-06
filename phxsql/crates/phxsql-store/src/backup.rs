//! Copia de seguranca dos dados, com manifesto conferivel.
//!
//! # O que faz uma copia ser confiavel
//!
//! Copiar arquivo e facil. O dificil e saber, seis meses depois, que a copia
//! presta. Por isso o backup daqui nao e so uma copia: e uma copia mais um
//! `backup.json` com o SHA-256 de cada arquivo, e um comando que le tudo de
//! volta e confere. Backup que ninguem consegue conferir e esperanca, nao
//! copia de seguranca.
//!
//! # Consistencia
//!
//! "Consistente" aqui quer dizer uma coisa precisa: **o que esta no destino
//! e o que estava na raiz num instante em que nenhum escritor andava** -- o
//! `retrato_ms` do manifesto. Como toda escrita passa pela trava unica de
//! dados, nao ha registro pela metade nem commit pela metade nesse instante.
//!
//! Desde o pedido 513 (passo 2) isso se faz em DUAS passadas, e e' por isso
//! que a escrita nao espera mais a copia inteira:
//!
//! * a **fase 1** ([`copiar_fase_1`]) corre sem trava: copia tudo e anota a
//!   ficha de cada arquivo (`stat` ANTES de ler);
//! * a **fase 2** ([`acertar_fase_2`]) corre sem escritor -- no servidor,
//!   sob a ficha de LEITURA da trava mais o portao do retrato
//!   (`phxsql-server`, `retrato.rs`), que exclui so' quem escreve: refaz o
//!   `stat` de tudo e recopia o que mudou, o que nasceu, tira o que sumiu.
//!
//! A decisao de recopiar e' UMA funcao pura, [`precisa_recopiar`], com tres
//! redes: o `stat` (tamanho, `mtime`, inode), o diario da tabela que andou
//! (para relogio que recua) e o «racily clean» do git (`mtime` a menos de 2 s
//! do `stat` da fase 1 recopia sempre). E' o caso do MariaDB para tabela sem
//! redo -- copiar por fora e recopiar sob o bloqueio curto o que estava em
//! uso --, porque o nosso `.log` e' logico e o replay fisico do PostgreSQL nao
//! se aplica (contrato em `docs/propostas/207-e-513p2-contrato-01-10-2026.md`).
//!
//! Este modulo nao sabe de trava nenhuma -- ele so le `raiz` e escreve no
//! destino. A copia em uma passada ([`executar`], [`executar_zip`]) e' a fase
//! 1 sozinha, para quem ja exclui o escritor por fora (a CLI, com o servidor
//! parado) e para o zip sem espaco para a arvore temporaria.
//!
//! # E a transacao, que passou a existir, nao muda isto
//!
//! Ela **reforca**, e por um motivo que vale escrever: como nada vai a disco
//! antes do `COMMIT`, uma transacao aberta durante a copia nao tem nada para
//! aparecer pela metade -- o conjunto de escrita dela esta em RAM. E o
//! `COMMIT` inteiro roda dentro de UMA tomada da mesma trava que o backup
//! segura, entao ele acontece antes ou depois da copia, nunca no meio.
//!
//! O que a transacao acrescenta ao diretorio copiado e a marca
//! `transacao_<id>.tx`, quando ha um commit em curso -- e ela viaja junto de
//! proposito: restaurado o diretorio, a recuperacao completa aquele commit
//! exatamente como faria no original. Ver `docs/TRANSACOES.md` §2.3.
//!
//! # O que a copia leva
//!
//! Tudo que esta debaixo da raiz de dados: os cinco arquivos de cada tabela,
//! os volumes numerados e os diretorios de database e schema. O `config.json`
//! NAO vai junto -- ele tem o token e os hashes de senha, e backup de dado
//! costuma ir para lugar diferente de backup de segredo.
//!
//! # Escrever e sincronizar sao DOIS passos, de proposito -- pedido 524/513
//!
//! Quem chama segura a trava de dados (a ficha de leitura, desde o 513) do
//! inicio ao fim da COPIA (ela le `raiz`, e e isso que a trava protege). O `fsync`, que so' toca o DESTINO
//! e nao le `raiz` nenhuma, nao precisa da trava -- e a catraca
//! `alcancam-fsync-2` (`bancada/concorrencia/mapa-da-trava.py`) e quem cobra
//! isso: ela conta secoes que alcancam `sync_all` com a trava na mao, e SO'
//! DESCE. `executar`/`executar_zip` escrevem (sem sincronizar) e devolvem os
//! descritores ainda abertos (pedido 552, abaixo); [`sincronizar_copias`] e
//! [`finalizar_zip`] rodam DEPOIS, com a trava ja solta,
//! chamado por quem tinha o `_trava` no escopo.
//!
//! # O manifesto -- e o ZIP -- so existem DEPOIS do `fsync` das copias --
//! pedido 524, condicao C2
//!
//! `backup.json` e' quem diz "backup completo": `op_backups` lista pasta com
//! manifesto como backup pronto, e `conferir` aprova o que ele descreve.
//! Grava-lo ANTES do `fsync` das copias (como este modulo fazia ate a
//! condicao C2) deixava, numa recusa no meio, uma pasta com manifesto
//! completo e copia incompleta -- o cache do nucleo mentindo exatamente como
//! o pedido 509 P1 ja mediu. Por isso o manifesto so' nasce em
//! [`finalizar_manifesto`], chamado DEPOIS de [`sincronizar_copias`] confirmar cada
//! copia. No ZIP -- um arquivo so, entao nao ha "antes/depois" dentro dele --
//! o equivalente e' o NOME: [`executar_zip`] escreve num `.part` que
//! `op_backups` nao reconhece, e so' [`finalizar_zip`] sincroniza e RENOMEIA
//! para o nome final, depois do `fsync`.
//!
//! # O `fsync` e no MESMO descritor que escreveu -- pedido 552
//!
//! O que atravessa a fronteira da trava nao e mais a lista de CAMINHOS: sao
//! os `File`s abertos de quem escreveu ([`Copias`], [`ZipParcial`]). Fechar e
//! reabrir para o `fsync` deixava o nucleo livre para despejar o inode no
//! intervalo, e com ele o erro de *writeback* guardado no `address_space`
//! (o caso *fsyncgate*): o descritor novo responderia Ok sem o dado. Um
//! descritor aberto segura o inode na memoria, e o erro chega ao `fsync`.
//!
//! O preco e descritor aberto ate o fim da corrida, e por isso ha um teto
//! ([`teto_de_abertos`]): as copias alem dele voltam ao caminho antigo
//! (fechar e reabrir), dito em [`sincronizar_arquivo`]. Nenhuma corrida
//! derruba o servidor por `EMFILE` para ganhar esta garantia.
//!
//! # E o diretorio tambem sincroniza, antes do manifesto -- pedido 579
//!
//! A remocao do `backup.json` velho (577) e as entradas das copias e das
//! pastas novas sao dado de DIRETORIO. [`sincronizar_copias`] faz o `fsync`
//! de cada pasta tocada depois do das copias, FORA da trava, e so entao
//! [`finalizar_manifesto`] publica -- e sincroniza a pasta de novo, porque o
//! nome do manifesto novo tambem e uma entrada de diretorio.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::hash::{para_hex, sha256};
use phxsql_core::json::Json;

// O dev/inode que identifica a copia (e a pasta nascida, pedido 593) mora no
// motor da `Pasta`: as duas conferencias sao a mesma pergunta.
use crate::util::identidade;

/// Nome do manifesto dentro do destino.
pub const MANIFESTO: &str = "backup.json";

/// Grava `dados` em `alvo`, SEM sincronizar -- o `fsync` e' o passo de
/// depois, [`sincronizar_copias`], porque quem chama pode estar com a trava de
/// dados na mao (ver a nota do modulo).
///
/// Nasce pelo motor da permissao (pedido 542): 0600 no destino, qualquer que
/// seja o `umask` e a permissao da origem. Era `File::create`, e a copia saia
/// `0644` -- ate a do `.lgpd`, que nasceu `0600` justamente por guardar dado
/// pessoal em claro. Backup costuma ir para disco de rede ou USB, que e onde
/// mais gente alcanca o arquivo.
///
/// Pedido 555: a recusa DEPOIS de o arquivo nascer apaga o que ficou pela
/// metade -- no ZIP, o `.part` ficava na pasta para sempre, porque a rotacao
/// so reconhece o nome final. A recusa do proprio `open` NAO apaga nada: ali
/// nada nosso nasceu, e o nome pode ser de outro (pedido 569). Mora aqui, e
/// nao no chamador, para as tres gravacoes do backup pagarem o mesmo motor.
///
/// Devolve o `File` ABERTO (pedido 552): o `fsync` de depois tem de ser neste
/// mesmo descritor, e nao num reaberto -- ver a nota do modulo.
///
/// Pedido 569: abre pelo [`crate::util::recriar_no_destino`], e nao pelo
/// motor da raiz de dados: o arquivo regular de OUTRO dono (ou com `nlink >
/// 1`) que ja esta no nome nao recebe o conteudo do banco -- o nome sai e o
/// nosso nasce. Vale para as tres gravacoes: copia, `.part` e manifesto.
fn escrever_sem_sync(alvo: &Path, dados: &[u8]) -> Result<File> {
    let arquivo = crate::util::recriar_no_destino(alvo, false)?;
    let escrito = (|| -> Result<()> {
        let mut w = std::io::BufWriter::new(&arquivo);
        w.write_all(dados)?;
        w.flush()?;
        Ok(())
    })();
    if let Err(e) = escrito {
        drop(arquivo);
        descartar_parcial(alvo);
        return Err(e);
    }
    Ok(arquivo)
}

/// O `fsync` de UM arquivo ja gravado cujo descritor ja FECHOU -- so para a
/// copia que passou do [`teto_de_abertos`] (pedido 552).
/// `read(true).write(true)`: o mesmo par que `NdxFile::abrir` usa, para o
/// `sync_all` valer em qualquer SO, nao so' onde reabrir so' para ler ja
/// bastaria.
///
/// **O risco que fica, dito:** reabrir pode perder o erro de *writeback* se
/// o inode sair da memoria no intervalo entre escrever e este `fsync` (o
/// caso *fsyncgate*). So alcanca a corrida com mais copias do que o teto, e
/// so as que passaram dele. Nao medido: forcar o despejo do inode com o erro
/// pendente pede um disco que recusa de verdade (`CAP_SYS_ADMIN`).
///
/// **E a reabertura nao confia no nome -- irmao do pedido 570.** Ela roda
/// FORA da trava, e o nome pode ter virado outra coisa desde a escrita: um
/// link para uma FIFO dava `EINVAL` no `fsync` e marcava o caminho como
/// «um fsync anterior foi recusado»; um link para outro arquivo sincronizava
/// o outro, e a copia de verdade nunca. Agora `alvo` e o nome alcancado pelo
/// descritor da pasta ([`crate::util::Pasta`]), a abertura nao segue link nem
/// espera leitor, e o `fstat` tem de dar o MESMO inode que a escrita
/// anotou -- senao a copia foi trocada, e isso sobe como erro do backup.
/// `mostrar` e o caminho real, o da mensagem e o da marca.
fn sincronizar_arquivo(alvo: &Path, anotado: Option<(u64, u64)>, mostrar: &Path) -> Result<()> {
    let mut abrir = OpenOptions::new();
    abrir.read(true).write(true);
    let arquivo = crate::util::sem_seguir_nem_esperar(&mut abrir)
        .open(alvo)
        .map_err(|e| no_nome_real(e, alvo, mostrar))?;
    let aberto = arquivo.metadata()?;
    if !aberto.is_file() || identidade(&aberto) != anotado {
        return Err(PhxError::Io(std::io::Error::other(format!(
            "{}: a copia foi trocada depois de escrita (o nome ja nao e o \
             arquivo que o backup gravou), e o backup nao sincroniza o que \
             nao escreveu. Confira quem mexe nesta pasta e repita",
            mostrar.display()
        ))));
    }
    // `sem_abortar`: o destino do backup nao e o disco do banco que o
    // gancho do 509 protege -- pedido 524, condicao C1 do parecer do DBA.
    crate::sincronia::sync_all_sem_abortar(&arquivo, mostrar)
}

/// O erro de quem abriu pelo `/proc/self/fd/N/nome` da [`crate::util::Pasta`]
/// dito pelo caminho REAL: `/proc/self/fd/7/c.reg` nao diz nada a quem le o
/// log. So o texto muda; o tipo do erro fica.
fn no_nome_real(e: impl Into<PhxError>, por_dentro: &Path, real: &Path) -> PhxError {
    let e = e.into();
    let (antes, depois) = (por_dentro.display().to_string(), real.display().to_string());
    match e {
        PhxError::Io(io) if antes != depois && io.to_string().contains(&antes) => PhxError::Io(
            std::io::Error::new(io.kind(), io.to_string().replace(&antes, &depois)),
        ),
        PhxError::Io(io) if antes != depois => {
            PhxError::Io(std::io::Error::new(io.kind(), format!("{depois}: {io}")))
        }
        outro => outro,
    }
}

/// Sincroniza cada caminho de `caminhos`, NA ORDEM da lista -- as COPIAS de
/// [`executar`], sem o manifesto: o `backup.json` so nasce depois, em
/// [`finalizar_manifesto`] (pedido 524, condicao C2). Se um `fsync` no meio
/// recusar, os que faltam nunca sincronizam e o `Err` sobe antes de o
/// manifesto chegar a existir.
///
/// Chamada DEPOIS de soltar `travar_dados()` -- ver a nota do modulo.
///
/// Pedido 552: cada copia sincroniza no descritor de quem a escreveu, que
/// [`executar`] devolveu aberto; so a que passou do teto reabre.
///
/// Pedido 579: depois das copias, o `fsync` de cada PASTA que a corrida
/// tocou -- a do destino (de onde saiu o `backup.json` velho), a de cada
/// copia e a mae de cada pasta criada. Sem isto, uma queda depois do
/// manifesto novo podia voltar com o velho, ou sem o nome de uma copia.
pub fn sincronizar_copias(copias: &Copias) -> Result<()> {
    for ((caminho, aberto), (por_dentro, anotado)) in copias
        .caminhos
        .iter()
        .zip(&copias.abertos)
        .zip(&copias.reabrir)
    {
        match aberto {
            Some(arquivo) => crate::sincronia::sync_all_sem_abortar(arquivo, caminho)?,
            None => sincronizar_arquivo(por_dentro, *anotado, caminho)?,
        }
    }
    for pasta in pastas_tocadas(copias) {
        sincronizar_pasta(&pasta)?;
    }
    Ok(())
}

/// O `fsync` de uma pasta do destino, pelo motor do `sincronia` (a marca da
/// recusa e a arma de teste sao as mesmas dos arquivos). O nome passado e o
/// do MANIFESTO dentro dela: a recusa marca a propria pasta, e nao a mae --
/// que no backup agendado e a pasta dos backups vizinhos.
///
/// Pedido 593: no DESCRITOR que a corrida abriu, e nao num `open` pelo nome.
/// Roda fora da trava, e o nome pode ter virado um link desde a escrita: o
/// `fsync` caia na pasta do outro lado dele, e as entradas das copias -- que
/// estao na pasta que a corrida abriu -- nunca sincronizavam. Sem descritor
/// (fora do Linux, sem `/proc`), fica o `open` pelo nome de antes.
fn sincronizar_pasta(pasta: &crate::util::Pasta) -> Result<()> {
    let de_quem = pasta.real().join(MANIFESTO);
    match pasta.descritor() {
        Some(d) => crate::sincronia::sync_all_sem_abortar(d, &de_quem),
        None => crate::sincronia::sincronizar_pasta_sem_abortar(pasta.real(), &de_quem),
    }
}

/// As pastas cujas ENTRADAS esta corrida mudou, sem repetir: o destino, a
/// pasta de cada copia, e a mae de cada pasta criada (a entrada da pasta nova
/// mora na mae dela). Da mais funda para a mais rasa: a do destino -- a que
/// perdeu o `backup.json` velho -- fica perto do fim, logo antes do manifesto.
///
/// Pedido 593: devolve as pastas ABERTAS, e nao os nomes -- o `fsync` cai no
/// descritor por onde a corrida escreveu. Sem repetir pelo caminho real.
fn pastas_tocadas(copias: &Copias) -> Vec<Arc<crate::util::Pasta>> {
    let mut pastas: Vec<Arc<crate::util::Pasta>> = Vec::new();
    let mut anotar = |p: &Arc<crate::util::Pasta>| {
        if !pastas.iter().any(|q| q.real() == p.real()) {
            pastas.push(p.clone());
        }
    };
    for &i in &copias.onde {
        anotar(&copias.ancoras[i]);
    }
    for p in &copias.pastas {
        anotar(p.mae());
    }
    if let Some(destino) = copias.ancoras.first() {
        anotar(destino);
    }
    pastas.sort_by_key(|p| std::cmp::Reverse(p.real().components().count()));
    pastas
}

/// Quantas copias de UMA corrida podem ficar com o descritor aberto ate o
/// `fsync` -- pedido 552.
///
/// O servidor ja segura um descritor por arquivo de tabela aberta, e aceita
/// conexoes enquanto o backup roda: segurar todas as copias poderia gastar o
/// `RLIMIT_NOFILE` e fazer o `accept` de outra thread recusar com `EMFILE`.
/// No Linux o teto e um QUARTO da folga medida agora (o limite brando menos
/// os descritores ja abertos), nunca acima de [`MAXIMO_DE_ABERTOS`]; fora do
/// Linux, sem `/proc`, e o fixo [`ABERTOS_SEM_MEDIDA`] -- o limite brando
/// do macOS e 256.
fn teto_de_abertos() -> usize {
    #[cfg(target_os = "linux")]
    if let Some(folga) = folga_de_descritores() {
        return (folga / 4).min(MAXIMO_DE_ABERTOS);
    }
    ABERTOS_SEM_MEDIDA
}

/// Teto de descritores de copia seguros por corrida, mesmo com folga de sobra.
const MAXIMO_DE_ABERTOS: usize = 1024;

/// O teto quando a folga nao se mede.
const ABERTOS_SEM_MEDIDA: usize = 64;

/// O limite brando de descritores menos os abertos agora, lidos do `/proc`.
/// `None` se algum dos dois nao se le -- e ai vale o fixo.
#[cfg(target_os = "linux")]
fn folga_de_descritores() -> Option<usize> {
    let limites = std::fs::read_to_string("/proc/self/limits").ok()?;
    let linha = limites.lines().find(|l| l.starts_with("Max open files"))?;
    let brando = match linha.split_whitespace().nth(3)? {
        "unlimited" => usize::MAX,
        n => n.parse().ok()?,
    };
    let em_uso = std::fs::read_dir("/proc/self/fd").ok()?.count();
    Some(brando.saturating_sub(em_uso))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arquivo {
    /// Caminho relativo a raiz dos dados, sempre com barra normal.
    pub caminho: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Default)]
pub struct Relatorio {
    pub arquivos: Vec<Arquivo>,
    pub bytes: u64,
    /// Tamanho do ZIP, quando a copia foi para arquivo unico.
    pub comprimido: u64,
    /// Preenchido so na conferencia: o que nao bate.
    pub divergencias: Vec<String>,
    /// O database UNICO desta copia; `None` quando ela e da raiz inteira.
    ///
    /// # Por que o manifesto precisa dizer isto
    ///
    /// Os caminhos de dentro nao contam: uma copia da raiz lista
    /// `Z/clientes.reg` e uma copia do banco Z lista `clientes.reg`, mas um
    /// banco que so tenha schemas lista `matriz/clientes.reg` -- que se
    /// escreve igualzinho a uma raiz com o database `matriz`. Adivinhar pelo
    /// formato do caminho acerta quase sempre, e "quase sempre" numa
    /// restauracao quer dizer restaurar um schema como se fosse um banco.
    pub database: Option<String>,
    /// O instante do RETRATO -- o da fase 2 (pedido 513, passo 2), quando
    /// nenhum escritor anda e cada arquivo do destino e' o da raiz naquele
    /// instante. `quando_ms` e' quando o backup COMECOU; os dois diferem
    /// pela duracao da fase 1. `None` so' na conferencia e no manifesto
    /// antigo, que o `conferir` e o `restaurar` aceitam como antes.
    pub retrato_ms: Option<i64>,
}

impl Relatorio {
    pub fn ok(&self) -> bool {
        self.divergencias.is_empty()
    }

    /// O manifesto, com os dois carimbos saindo do MESMO numero.
    ///
    /// # Por que `quando_ms` existe, se `quando` ja dizia a hora
    ///
    /// Porque o `quando` e texto de TELA -- `2026-08-29 03:00:04,132` --, e o
    /// PITR precisa do instante como NUMERO para comparar com o carimbo de
    /// cada evento do diario. Ler o instante de uma cadeia formatada para
    /// gente amarra o motor ao jeito de escrever: no dia em que o
    /// `instante_iso` mudar de virgula para ponto, a restauracao a um instante
    /// para de achar o comeco -- e para calada. **Quando um gerador depende de
    /// uma lista, a lista sai do codigo**; aqui a regra e a mesma, com um
    /// numero no lugar da lista.
    ///
    /// Os dois saem do mesmo `quando_ms` de proposito: dois campos de tempo
    /// preenchidos por dois caminhos sao dois campos que um dia divergem.
    ///
    /// **Acrescimo, como o `escopo`**: manifesto gravado antes disto nao tem
    /// `quando_ms` e continua restaurando igual -- o que ele nao faz e PITR, e
    /// a recusa nomeia o campo em vez de adivinhar a hora pelo texto.
    pub fn para_json(&self, quando_ms: i64) -> Json {
        Json::objeto(vec![
            ("phxsql", Json::texto_de(env!("CARGO_PKG_VERSION"))),
            (
                "quando",
                Json::texto_de(phxsql_core::datahora::instante_iso(quando_ms)),
            ),
            ("quando_ms", Json::Numero(quando_ms as f64)),
            // O retrato (pedido 513, passo 2): o instante em que o conteudo
            // e' consistente. Manifesto de antes nao o tem e segue valendo.
            (
                "retrato_ms",
                match self.retrato_ms {
                    Some(t) => Json::Numero(t as f64),
                    None => Json::Nulo,
                },
            ),
            ("arquivos", Json::de_u64(self.arquivos.len() as u64)),
            ("bytes", Json::de_u64(self.bytes)),
            // Os dois campos entraram JUNTO com a restauracao. Manifesto
            // gravado antes nao os tem e continua valendo: quem le cai na
            // deducao pelo formato dos caminhos. Backup ja gravado nao se
            // reescreve, e leitor antigo ignora campo que nao conhece.
            (
                "escopo",
                Json::texto_de(match &self.database {
                    Some(_) => "database",
                    None => "raiz",
                }),
            ),
            (
                "database",
                match &self.database {
                    Some(n) => Json::texto_de(n),
                    None => Json::Nulo,
                },
            ),
            (
                "conteudo",
                Json::Lista(
                    self.arquivos
                        .iter()
                        .map(|a| {
                            Json::objeto(vec![
                                ("caminho", Json::texto_de(&a.caminho)),
                                ("bytes", Json::de_u64(a.bytes)),
                                ("sha256", Json::texto_de(&a.sha256)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}

/// Lista os arquivos debaixo de `raiz`, em ordem, com caminho relativo.
///
/// Ordenado de proposito: dois backups da mesma coisa tem de dar manifestos
/// comparaveis, e a ordem que o sistema de arquivos devolve nao e estavel.
pub(crate) fn listar(raiz: &Path) -> Result<Vec<PathBuf>> {
    let mut achados = Vec::new();
    let mut pilha = vec![raiz.to_path_buf()];
    while let Some(dir) = pilha.pop() {
        let leitura = std::fs::read_dir(&dir).map_err(|e| {
            PhxError::NaoEncontrado(format!("nao consegui ler {}: {e}", dir.display()))
        })?;
        for entrada in leitura {
            let entrada = entrada?;
            let caminho = entrada.path();
            if caminho.is_dir() {
                pilha.push(caminho);
            } else if entrada.file_name() == crate::trava_de_instancia::NOME_DO_ARQUIVO {
                // A trava de instancia (pedido 635) e estado do processo que
                // grava, nao dado: copia-la nao serve a ninguem, e no Windows
                // a trava do nucleo nem deixa le-la.
            } else {
                achados.push(caminho);
            }
        }
    }
    achados.sort();
    Ok(achados)
}

pub(crate) fn relativo(raiz: &Path, arquivo: &Path) -> String {
    arquivo
        .strip_prefix(raiz)
        .unwrap_or(arquivo)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Nome do arquivo: `BancoNome_Admin_Data_HoraMin.zip`.
///
/// Traz quem fez e quando no proprio nome porque e assim que se acha o
/// arquivo certo numa pasta com trezentos backups -- sem abrir nenhum.
pub fn nome_do_zip(banco: &str, admin: &str, quando_ms: i64) -> String {
    let dias = (quando_ms.div_euclid(86_400_000)) as i32;
    let (ano, mes, dia) = phxsql_core::datahora::civil_de_dias(dias);
    let minutos = quando_ms.rem_euclid(86_400_000) / 60_000;
    let limpo = |s: &str, padrao: &str| -> String {
        let t: String = s
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        if t.is_empty() {
            padrao.to_string()
        } else {
            t
        }
    };
    format!(
        "{}_{}_{ano:04}-{mes:02}-{dia:02}_{:02}{:02}.zip",
        limpo(banco, "dados"),
        limpo(admin, "sistema"),
        minutos / 60,
        minutos % 60
    )
}

/// Copia para um unico arquivo ZIP, com o manifesto dentro.
///
/// Um arquivo so viaja melhor do que uma arvore de diretorios: cabe em anexo,
/// sobe para nuvem inteiro, e o Windows(R), o Linux e o celular abrem sem
/// instalar nada. O manifesto vai dentro, entao a copia carrega a propria
/// conferencia.
///
/// `banco` vazio copia a raiz inteira. E a copia em UMA passada: quem chama
/// segura a trava de dados durante ESTA chamada (ela le `raiz`) -- a CLI, e
/// o servidor quando nao ha espaco para a arvore temporaria das duas fases
/// ([`copiar_fase_1_para_zip`]). O conteudo volta escrito num nome PARCIAL
/// (pedido 524, condicao C2 -- ver [`finalizar_zip`]), e o caminho devolvido
/// e o nome FINAL, que so existe depois de chamar `finalizar_zip`.
pub fn executar_zip(
    raiz: &Path,
    pasta: &Path,
    banco: &str,
    admin: &str,
    quando_ms: i64,
) -> Result<(ZipParcial, Relatorio)> {
    let origem = origem_do_zip(raiz, banco)?;
    // Pedido 577: a pasta que esta chamada cria sai junto com o erro -- o
    // `.part` ja sai pelo `escrever_sem_sync`, e sem isto sobrava a pasta
    // vazia que ninguem pediu. Pelo motor do 576: so' a que NASCEU aqui, e
    // com `remove_dir`, que so' remove vazia e nao segue link.
    conferir_destino(raiz, pasta, false)?;
    let mut criadas = Copias::default();
    if let Err(e) = criar_pasta_da_corrida(pasta, &mut criadas.pastas) {
        descartar_corrida(&criadas);
        return Err(e);
    }
    let retrato_ms = Some(crate::util::agora_ms());
    let feito = montar_zip(&origem, pasta, banco, admin, quando_ms, retrato_ms);
    if feito.is_err() {
        descartar_corrida(&criadas);
    }
    let (alvo, arquivo, r) = feito?;
    let zip = ZipParcial {
        alvo,
        arquivo,
        pastas: criadas.pastas,
    };
    Ok((zip, r))
}

/// A origem de um zip: a raiz inteira, ou o database `banco` dentro dela.
fn origem_do_zip(raiz: &Path, banco: &str) -> Result<PathBuf> {
    let origem = if banco.is_empty() {
        raiz.to_path_buf()
    } else {
        crate::catalogo::validar_nome("database", banco)?;
        raiz.join(banco)
    };
    if !origem.is_dir() {
        return Err(PhxError::NaoEncontrado(format!(
            "{} nao existe",
            origem.display()
        )));
    }
    Ok(origem)
}

/// A FASE 1 de um zip em duas passadas: a arvore vai para
/// `pasta/<nome do zip>.retrato.part/` sem trava nenhuma, e [`concluir_zip`]
/// a comprime DEPOIS da fase 2 e a apaga.
///
/// Precisa de espaco para a arvore inteira antes de comprimir. `livre` e o
/// que quem chama mediu na pasta (bytes; `None` = nao mediu): abaixo do
/// tamanho da origem mais 10%, devolve `Ok(None)` e quem chama cai na copia
/// em uma passada ([`executar_zip`]) DIZENDO -- pela pétrea «guarda nova
/// entra pedida», quem tem zip funcionando hoje nao passa a falhar.
pub fn copiar_fase_1_para_zip(
    raiz: &Path,
    pasta: &Path,
    banco: &str,
    admin: &str,
    quando_ms: i64,
    livre: Option<u64>,
) -> Result<Option<Fase1>> {
    let origem = origem_do_zip(raiz, banco)?;
    conferir_destino(raiz, pasta, false)?;
    if let Some(livre) = livre {
        let precisa = tamanho_da_arvore(&origem)?;
        if livre < precisa.saturating_add(precisa / 10) {
            return Ok(None);
        }
    }
    let nome = nome_do_zip(
        if banco.is_empty() { "dados" } else { banco },
        admin,
        quando_ms,
    );
    let arvore = pasta.join(arvore_parcial_de(&nome));
    let pedido = PedidoDeZip {
        pasta: pasta.to_path_buf(),
        banco: banco.to_string(),
        admin: admin.to_string(),
        quando_ms,
    };
    // Teto ZERO de descritores: a arvore temporaria nao recebe `fsync` (o
    // zip e' quem sincroniza), entao nao ha por que segurar descritor nenhum.
    copiar_fase_1_com_teto(&origem, &arvore, 0, Some(pedido)).map(Some)
}

/// O nome da arvore temporaria de um zip: `<nome>.retrato.part/`. Um
/// diretorio com esse sufixo e' nosso, e orfao se velho (ver
/// [`limpar_parciais_orfaos`]).
fn arvore_parcial_de(nome_do_zip: &str) -> String {
    format!("{nome_do_zip}.retrato.part")
}

/// A soma dos tamanhos de tudo debaixo de `origem`, pelo `stat` (sem ler).
fn tamanho_da_arvore(origem: &Path) -> Result<u64> {
    let mut total = 0u64;
    for arquivo in listar(origem)? {
        if let Ok(m) = std::fs::metadata(&arquivo) {
            total = total.saturating_add(m.len());
        }
    }
    Ok(total)
}

/// DEPOIS da fase 2 e fora da trava: comprime a arvore temporaria no `.part`
/// do zip e a apaga. O que volta e o mesmo par de [`executar_zip`], para
/// [`finalizar_zip`] concluir.
pub fn concluir_zip(mut fase: Fase1) -> Result<(ZipParcial, Relatorio)> {
    let Some(pedido) = fase.zip.take() else {
        return Err(PhxError::Esquema(
            "esta corrida e' em arvore, nao em zip: use `terminar`".into(),
        ));
    };
    let retrato_ms = fase.r.retrato_ms;
    let feito = montar_zip(
        &fase.arvore,
        &pedido.pasta,
        &pedido.banco,
        &pedido.admin,
        pedido.quando_ms,
        retrato_ms,
    );
    // A arvore sai sempre -- deu certo ou nao --, pelo motor do 576: so' o
    // que esta corrida fez nascer, pelo descritor da mae.
    descartar_corrida(&fase.copias);
    let (alvo, arquivo, r) = feito?;
    // As pastas que nasceram ACIMA da arvore (a propria `pasta`, se nova)
    // ficam com o zip: saem com ele numa recusa do `finalizar_zip`.
    let pastas = fase
        .copias
        .pastas
        .drain(..)
        .filter(|p| !p.real().starts_with(&fase.arvore))
        .collect();
    Ok((
        ZipParcial {
            alvo,
            arquivo,
            pastas,
        },
        r,
    ))
}

/// O ZIP escrito com nome PARCIAL e ainda nao sincronizado -- o que
/// [`executar_zip`] devolve e [`finalizar_zip`] conclui.
///
/// Carrega o `File` de quem escreveu o `.part` (pedido 552: o `fsync` e
/// neste descritor, sem reabrir) e as pastas que a corrida criou (pedido
/// 579: se o `rename` final recusa, elas saem junto com o `.part`).
///
/// Derefere para o caminho FINAL, que so existe depois de [`finalizar_zip`]
/// -- e o que os chamadores mostram e anotam.
#[derive(Debug)]
pub struct ZipParcial {
    alvo: PathBuf,
    arquivo: File,
    pastas: Vec<crate::util::Nascida>,
}

impl std::ops::Deref for ZipParcial {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.alvo
    }
}

impl AsRef<Path> for ZipParcial {
    fn as_ref(&self) -> &Path {
        &self.alvo
    }
}

/// O corpo de [`executar_zip`] depois da pasta criada, separado para o erro
/// dele passar pela faxina da pasta num lugar so' (pedido 577) -- e o de
/// [`concluir_zip`], que o chama sobre a arvore temporaria: um laco so' le e
/// comprime, venha a origem da raiz viva ou da copia da fase 2.
fn montar_zip(
    origem: &Path,
    pasta: &Path,
    banco: &str,
    admin: &str,
    quando_ms: i64,
    retrato_ms: Option<i64>,
) -> Result<(PathBuf, File, Relatorio)> {
    let alvo = pasta.join(nome_do_zip(
        if banco.is_empty() { "dados" } else { banco },
        admin,
        quando_ms,
    ));

    let mut zip = phxsql_core::zip::Zip::novo(quando_ms);
    let mut r = Relatorio {
        database: (!banco.is_empty()).then(|| banco.to_string()),
        retrato_ms,
        ..Relatorio::default()
    };
    for arquivo in listar(origem)? {
        let rel = relativo(origem, &arquivo);
        let dados = std::fs::read(&arquivo)?;
        zip.acrescentar(&rel, &dados);
        r.bytes += dados.len() as u64;
        r.arquivos.push(Arquivo {
            caminho: rel,
            bytes: dados.len() as u64,
            sha256: para_hex(&sha256(&dados)),
        });
    }
    // O manifesto entra por ultimo, ja sabendo de todos os outros.
    zip.acrescentar(MANIFESTO, r.para_json(quando_ms).escrever().as_bytes());

    let bytes = zip.terminar();
    r.comprimido = bytes.len() as u64;
    // Mesma garantia do irmao `executar` (pedido 524): o ZIP e um arquivo
    // so, mas e o UNICO arquivo do backup inteiro -- sem `fsync`, "concluido"
    // podia estar so no cache do nucleo. SEM sincronizar aqui, pelo mesmo
    // motivo do `executar`: quem chama ainda pode estar com a trava de
    // dados na mao, e o `sync_all` e' o passo de depois.
    //
    // NOME PARCIAL (condicao C2): grava em `<nome>.part`, nao no nome final.
    // `op_backups` so reconhece `.zip`, entao o `.part` fica invisivel para
    // quem lista backups ate o `rename` de [`finalizar_zip`] -- uma recusa
    // no meio nunca deixa um `.zip` pela metade parecendo pronto.
    let arquivo = escrever_sem_sync(&parcial(&alvo), &bytes)?;
    Ok((alvo, arquivo, r))
}

/// Idade a partir da qual um `.part` nosso na pasta e ORFAO -- pedido 555.
///
/// O `.part` vivo so existe entre o fim da escrita (o `mtime`) e o `rename`
/// de [`finalizar_zip`], e esse intervalo e so o `fsync`. Mas o `fsync` roda
/// FORA da trava de dados, entao outro backup para a mesma pasta pode
/// terminar no meio dele: sem a idade, a faxina de um apagaria o `.part` que
/// o outro ainda vai renomear. Uma hora cobre `fsync` lento em USB ou disco
/// de rede e o relogio de um compartilhamento um pouco adiantado.
const IDADE_DE_ORFAO: std::time::Duration = std::time::Duration::from_secs(3600);

/// Apaga o `.part` pelo NOME, sem abrir -- o caminho de erro do pedido 555.
///
/// `remove_file` nunca segue link simbolico nem abre FIFO (pedidos 568/570):
/// se alguem trocou o nome, some o link, e o alvo dele fica intacto. Falhar
/// aqui nao muda nada para quem chamou -- o erro que importa e o que ja vai
/// subir -- e o que sobrar a faxina de [`limpar_parciais_orfaos`] alcanca.
fn descartar_parcial(p: &Path) {
    let _ = std::fs::remove_file(p);
}

/// O nome tem a cara de um `.part` NOSSO: `Banco_Admin_Data_HoraMin.zip.part`
/// -- o mesmo crivo de [`escolher_para_apagar`], com o sufixo do parcial.
fn e_nome_de_parcial(nome: &str) -> bool {
    nome.strip_suffix(".part")
        .is_some_and(|zip| zip.ends_with(".zip") && zip.matches('_').count() >= 3)
}

/// O nome tem a cara da arvore temporaria de um zip NOSSO:
/// `Banco_Admin_Data_HoraMin.zip.retrato.part` (pedido 513, passo 2).
fn e_nome_de_arvore_parcial(nome: &str) -> bool {
    nome.strip_suffix(".retrato.part")
        .is_some_and(|zip| zip.ends_with(".zip") && zip.matches('_').count() >= 3)
}

/// A faxina dos `.part` que um backup que falhou deixou para tras -- pedido
/// 555. Roda em [`finalizar_zip`], depois do `rename` que deu certo, entao
/// alcanca os tres chamadores (CLI, `backup` pelo protocolo e o agendado)
/// pelo MESMO motor, e FORA da trava de dados no servidor.
///
/// So apaga o que e comprovadamente nosso, e por isso sao quatro crivos:
///
/// 1. o NOME tem o formato dos nossos -- backup nao apaga arquivo que nao
///    criou, a mesma regra da rotacao;
/// 2. o `lstat` diz arquivo REGULAR -- link simbolico, FIFO e dispositivo
///    ficam, e nenhum e aberto (pedidos 568/570: seguir o link apagaria o
///    que ele aponta; abrir a FIFO pararia o backup);
/// 3. o DONO e o mesmo do zip que acabamos de criar (so Unix) -- arquivo de
///    outro usuario plantado com o nosso formato de nome nao e nosso (569);
/// 4. o `mtime` e mais velho que [`IDADE_DE_ORFAO`] -- o `.part` de um
///    backup vizinho ainda no `fsync` nao e orfao.
///
/// Devolve quantos apagou. Erro de leitura da pasta nao sobe: a faxina e
/// acessorio do backup que ja deu certo, e nao pode transforma-lo em falha.
fn limpar_parciais_orfaos(pasta: &Path, nosso: &std::fs::Metadata) -> usize {
    let Ok(dir) = std::fs::read_dir(pasta) else {
        return 0;
    };
    let agora = std::time::SystemTime::now();
    let mut apagados = 0;
    for entrada in dir.flatten() {
        let nome = entrada.file_name();
        let caminho = entrada.path();
        let Ok(m) = std::fs::symlink_metadata(&caminho) else {
            continue;
        };
        if !mesmo_dono(&m, nosso) {
            continue;
        }
        // A arvore temporaria de um zip em duas passadas (pedido 513, passo
        // 2) que o servidor deixou ao cair no meio: so' com o nosso sufixo,
        // so' diretorio de verdade (link nao e' seguido pelo `remove_dir_all`
        // no proprio nome, e o `lstat` ja o recusou), so' velha.
        if nome.to_str().is_some_and(e_nome_de_arvore_parcial) {
            let velha = m
                .modified()
                .ok()
                .and_then(|t| agora.duration_since(t).ok())
                .is_some_and(|idade| idade >= IDADE_DE_ORFAO);
            if m.file_type().is_dir() && velha && std::fs::remove_dir_all(&caminho).is_ok() {
                apagados += 1;
            }
            continue;
        }
        if !nome.to_str().is_some_and(e_nome_de_parcial) {
            continue;
        }
        if !m.file_type().is_file() {
            continue;
        }
        let velho = m
            .modified()
            .ok()
            .and_then(|t| agora.duration_since(t).ok())
            .is_some_and(|idade| idade >= IDADE_DE_ORFAO);
        if velho && std::fs::remove_file(&caminho).is_ok() {
            apagados += 1;
        }
    }
    apagados
}

fn mesmo_dono(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        a.uid() == b.uid()
    }
    #[cfg(not(unix))]
    {
        let _ = (a, b);
        true
    }
}

/// Recusa, antes de escrever byte nenhum, o destino que se mistura com a
/// raiz de dados -- pedido 554.
///
/// DENTRO dela (ou igual) sempre: a copia copiaria a copia, e o arquivo de
/// backup apareceria no meio dos dados. ACIMA dela so na copia em arvore
/// (`acima_tambem`): a arvore vai para dentro de uma pasta que CONTEM a raiz,
/// e a faxina de uma corrida que falha (`descartar_corrida`) andaria por
/// cima do banco vivo. E o mesmo corte do `pg_basebackup`, que recusa `-D`
/// que nao esteja vazio -- e o ancestral do PGDATA nunca esta. O zip e UM
/// arquivo com nome proprio: numa pasta acima da raiz ele nao alcanca nada
/// dela, e a marca de um `fsync` recusado ali nao chega ao banco (a lista do
/// destino e outra -- ver `sincronia`, pedido 554).
///
/// Confere pela grafia E pelo disco: um link ou um `..` no caminho escondia
/// a raiz do `starts_with` de texto, que era a unica conferencia antes.
fn conferir_destino(raiz: &Path, destino: &Path, acima_tambem: bool) -> Result<()> {
    let pares = [
        (absoluto(destino), absoluto(raiz)),
        (real_ate_onde_existe(destino), real_ate_onde_existe(raiz)),
    ];
    for (d, r) in &pares {
        let relacao = if d.starts_with(r) {
            Some("fica dentro da")
        } else if acima_tambem && r.starts_with(d) {
            Some("contem a")
        } else {
            None
        };
        if let Some(relacao) = relacao {
            return Err(misturado(raiz, destino, relacao));
        }
    }
    Ok(())
}

/// A recusa das duas conferencias do destino -- a do nome
/// ([`conferir_destino`]) e a do descritor ([`conferir_destino_aberto`]) --,
/// num lugar so: o usuario recebe a mesma frase, qualquer que tenha pego.
fn misturado(raiz: &Path, destino: &Path, relacao: &str) -> PhxError {
    PhxError::Esquema(format!(
        "o destino do backup {} {relacao} raiz de dados {}: o backup \
         iria se misturar com o banco vivo. Escolha uma pasta fora \
         da raiz e que nao a contenha",
        destino.display(),
        raiz.display()
    ))
}

/// A mesma pergunta de [`conferir_destino`], feita ao DESCRITOR que a
/// corrida abriu -- pedido 611 (S7).
///
/// O [`conferir_destino`] le o nome; o `Pasta::abrir`, depois, segue o nome
/// de novo (por desenho: o destino e escolha de quem chama). Entre os dois,
/// trocar um link do caminho fazia o nome conferido ser um e a pasta aberta
/// outra: medido contra o SO, as copias caiam em `dados/loja/rh/` -- o
/// schema `rh` dentro do database `loja` (`tests/destino-trocado-na-janela.rs`).
/// Por isso a conferencia que vale e esta: o dev/inode da pasta ABERTA e o de
/// cada pasta acima dela (subindo pelo descritor) contra o da raiz, e o da
/// raiz e de cada pasta acima dela contra o da pasta aberta. A do nome fica
/// antes, para a recusa comum sair antes de criar pasta nenhuma.
///
/// Fora do Linux a `Pasta` nao tem descritor e a subida e pelo nome; fora do
/// Unix nao ha inode e a lista vem vazia -- fica so a conferencia pelo nome,
/// com a janela, dita.
fn conferir_destino_aberto(
    raiz: &Path,
    destino: &Path,
    aberto: &crate::util::Pasta,
    acima_tambem: bool,
) -> Result<()> {
    let do_destino = aberto.ancestrais()?;
    let da_raiz = crate::util::ancestrais(raiz)?;
    let (Some(eu), Some(r)) = (do_destino.first(), da_raiz.first()) else {
        return Ok(());
    };
    if do_destino.contains(r) {
        return Err(misturado(raiz, destino, "fica dentro da"));
    }
    if acima_tambem && da_raiz.contains(eu) {
        return Err(misturado(raiz, destino, "contem a"));
    }
    Ok(())
}

/// A grafia absoluta, sem tocar no disco.
fn absoluto(p: &Path) -> PathBuf {
    crate::volume::absoluto_lexico(p).unwrap_or_else(|| p.to_path_buf())
}

/// O caminho resolvido no disco ate o ultimo pedaco que existe, com o resto
/// (o que a corrida ainda vai criar) colado por cima: o destino quase sempre
/// ainda nao existe, e e o pai dele que pode ser um link para dentro da raiz.
fn real_ate_onde_existe(p: &Path) -> PathBuf {
    let lexico = absoluto(p);
    let mut resto = Vec::new();
    let mut atual = lexico.as_path();
    loop {
        if let Ok(mut real) = std::fs::canonicalize(atual) {
            for nome in resto.iter().rev() {
                real.push(nome);
            }
            return real;
        }
        match (atual.parent(), atual.file_name()) {
            (Some(pai), Some(nome)) => {
                resto.push(nome.to_os_string());
                atual = pai;
            }
            _ => return lexico,
        }
    }
}

/// O nome PARCIAL de um zip de backup: `<nome>.part`, no mesmo diretorio.
///
/// `.part` nao bate no filtro `extension() == "zip"` de `op_backups`
/// (`servidor.rs`), entao um zip pela metade nunca aparece na lista.
fn parcial(alvo: &Path) -> PathBuf {
    let nome = alvo
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    alvo.with_file_name(format!("{nome}.part"))
}

/// Sincroniza o ZIP parcial e o TROCA de nome para o final -- so agora ele
/// existe para quem lista backups (pedido 524, condicao C2).
///
/// O `rename` passa pelo motor do pedido 467, com o `fsync` do diretorio --
/// na versao que devolve `Err` em vez de derrubar o servidor, porque o
/// destino do backup nao e o disco do banco.
///
/// Pedido 555: se o `fsync` ou o `rename` recusam, o `.part` sai junto com o
/// erro -- um zip que nao chegou ao nome final nunca vai chegar, e a rotacao
/// nao o enxerga. Deu certo, a faxina dos orfaos de corridas anteriores roda
/// aqui, uma vez por backup, pelo motor de [`limpar_parciais_orfaos`].
///
/// Pedido 552: o `fsync` e no descritor que [`executar_zip`] escreveu. E
/// pedido 579: na recusa saem tambem as pastas que a corrida criou, pelo
/// motor do 576 (`remove_dir`, so vazia) -- antes ficava a pasta vazia.
pub fn finalizar_zip(zip: &ZipParcial) -> Result<()> {
    let alvo = zip.alvo.as_path();
    let parcial = parcial(alvo);
    let trocado = crate::sincronia::sync_all_sem_abortar(&zip.arquivo, &parcial)
        .and_then(|()| crate::sincronia::trocar_duravel_sem_abortar(&parcial, alvo));
    if let Err(e) = trocado {
        // Depois de um `rename` que passou (recusa so no `fsync` do
        // diretorio) o `.part` ja nao existe, e apagar nao acha nada.
        descartar_parcial(&parcial);
        descartar_pastas(&zip.pastas);
        return Err(e);
    }
    // Pedido 524: a entrada de cada pasta que a corrida CRIOU mora na mae
    // dela, e o `fsync` da pasta do zip (no `rename`) nao a alcanca -- uma
    // queda levava a cadeia inteira, com o zip dentro, depois do «concluido».
    // O mesmo ajudante da arvore ([`sincronizar_pasta`], no descritor da mae
    // que a corrida abriu), fora da trava. Depois do `rename`: o zip final ja
    // existe e uma recusa aqui tira so o aviso de «concluido», nao o dado.
    for nascida in &zip.pastas {
        if let Err(e) = sincronizar_pasta(nascida.mae()) {
            descartar_pastas(&zip.pastas);
            return Err(e);
        }
    }
    if let (Some(pasta), Ok(nosso)) = (alvo.parent(), std::fs::symlink_metadata(alvo)) {
        limpar_parciais_orfaos(pasta, &nosso);
    }
    Ok(())
}

/// Dos arquivos da pasta, quais apagar para sobrarem `manter`.
///
/// Separada da faxina de verdade para poder ser testada sem mexer em disco --
/// e porque a regra que importa aqui e "o que NAO apagar".
///
/// So entram arquivos com a cara dos nossos: `.zip` com pelo menos tres
/// sublinhados, que e o formato `Banco_Admin_Data_HoraMin.zip`. Backup nao
/// apaga arquivo que nao criou; alguem pode ter guardado outra coisa na pasta.
pub fn escolher_para_apagar(nomes: &[String], manter: usize) -> Vec<String> {
    if manter == 0 {
        return Vec::new();
    }
    let mut nossos: Vec<&String> = nomes
        .iter()
        .filter(|n| n.ends_with(".zip") && n.matches('_').count() >= 3)
        .collect();
    if nossos.len() <= manter {
        return Vec::new();
    }
    // O nome ja ordena por data: Banco_Admin_AAAA-MM-DD_HHMM.zip.
    nossos.sort();
    let sobra = nossos.len() - manter;
    nossos.into_iter().take(sobra).cloned().collect()
}

/// Copia a raiz de dados para o destino e escreve o manifesto -- SEM
/// sincronizar nada.
///
/// E a copia em UMA passada: so' a [`copiar_fase_1`], sem a fase 2 -- quem
/// chama exclui todo escritor durante ESTA chamada (a CLI, processo a parte
/// com o servidor parado; os testes), entao nao ha janela entre as fases e
/// nada a acertar. Uma fase 2 aqui recopiaria todo arquivo «racy» (escrito
/// nos ultimos 2 s) sem motivo, e a prova pelo `strace`
/// (`tests/fsync-no-descritor-que-escreveu.rs`) acusou: duas aberturas
/// onde o `fsync` no descritor que escreveu pede UMA. O servidor chama as
/// duas fases separadas, com a trava de dados so' na segunda (pedido 513,
/// passo 2). Devolve, alem do
/// relatorio, as [`Copias`] (sem o manifesto) para [`concluir`] rodar depois,
/// com a trava ja solta: ele sincroniza cada uma e so entao escreve e
/// sincroniza o `backup.json`, e numa falha apaga o que esta corrida criou
/// (pedido 576). A falha DENTRO desta funcao ja sai com a mesma faxina feita.
/// Ver a nota "Escrever e sincronizar sao DOIS passos" no topo do modulo.
///
/// # Por que a gravacao NAO fica em cache -- pedido 524
///
/// Ate aqui era `std::fs::write` puro sem fsync NENHUM depois -- nem aqui,
/// nem em lugar nenhum: um backup que se diz "concluido" sem estar no disco
/// do destino e o pior momento para mentir, e e justamente quando o
/// operador costuma apagar o backup ANTERIOR, contando com este.
/// [`sincronizar_copias`] fecha essa parte; esta funcao so' garante que os BYTES
/// certos estao no arquivo, o que o `escrever_sem_sync` (e o SHA-256 tirado
/// de `dados`, nao relido do disco) ja bastam para provar.
///
/// # Por que o MANIFESTO nao nasce aqui -- pedido 524, condicao C2
///
/// Gravar o `backup.json` sob a trava, ANTES do primeiro `fsync` das copias,
/// deixava uma pasta com manifesto completo no disco mesmo quando uma copia
/// nunca chegou a sincronizar: `op_backups` a lista, e o `verificar` do
/// mesmo boot aprova pelo cache do nucleo, que e a mentira que o 509 P1 ja
/// mediu. Sem manifesto, a pasta nunca parece um backup pronto -- nem para
/// quem lista, nem para quem confere.
pub fn executar(raiz: &Path, destino: &Path, _quando_ms: i64) -> Result<(Relatorio, Copias)> {
    executar_com_teto(raiz, destino, teto_de_abertos())
}

/// [`executar`] com o teto de descritores dado -- separado para a prova do
/// caminho alem do teto nao precisar de milhares de arquivos.
fn executar_com_teto(raiz: &Path, destino: &Path, teto: usize) -> Result<(Relatorio, Copias)> {
    let fase = copiar_fase_1_com_teto(raiz, destino, teto, None)?;
    Ok(terminar(fase))
}

// ------------------------------------------------- as DUAS passadas (513/2)
//
// O backup copia a raiz em duas fases. A FASE 1 corre SEM a trava de dados:
// anota a ficha de cada arquivo (`stat` ANTES de ler) e o copia. A FASE 2
// corre sem escritor (no servidor, sob a ficha de leitura e o portao do
// retrato): faz o `stat` de tudo de novo e recopia so o que mudou -- e o que,
// pelas tres redes de [`precisa_recopiar`], PODE ter mudado sem o `stat`
// dizer --, copia o que nasceu, apaga do destino o que sumiu. O retrato e o
// instante da fase 2, como o `BLOCK_COMMIT` do `mariabackup`: o que ha no
// destino depois dela e exatamente o que ha na raiz naquele instante, byte a
// byte, porque cada arquivo ou foi recopiado agora ou provadamente nao mudou.
//
// E o caso do MariaDB para tabela SEM redo (copiar por fora, recopiar sob o
// bloqueio curto o que estava em uso): o nosso `.log` e logico e a imagem e
// opcional, entao o replay fisico do PostgreSQL nao se aplica (contrato
// `docs/propostas/207-e-513p2-contrato-01-10-2026.md` §513.3).

/// A ficha de um arquivo da raiz no instante do `stat` -- o que a fase 2
/// compara com o `stat` de agora.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ficha {
    pub bytes: u64,
    /// O `mtime`, em nanossegundos desde a epoca (negativo antes dela).
    pub mtime_ns: i128,
    /// dev/inode, onde ha; o arquivo TROCADO (apagado e recriado com o mesmo
    /// nome) muda de inode mesmo com tamanho e `mtime` iguais.
    pub identidade: Option<(u64, u64)>,
    /// O instante do proprio `stat`, em nanossegundos desde a epoca -- a
    /// regua do «racily clean».
    pub lido_em_ns: i128,
}

/// A janela do «racily clean» (a regra do `git`): arquivo cujo `mtime` estava
/// a menos disto do `stat` da fase 1 e recopiado SEMPRE, porque uma escrita
/// no mesmo tique do relogio do sistema de arquivos deixa `mtime` e tamanho
/// iguais. Medido no kernel 6.18 (carimbo fino): 0 repeticoes em 20.000
/// escritas seguidas; kernels sem carimbo fino (anteriores ao 6.13) usam
/// relogio grosso, e e para eles que a janela existe. Dois segundos cobrem a
/// granularidade de 1 s do ext3/FAT com folga.
pub const JANELA_RACY_NS: i128 = 2_000_000_000;

fn ns_desde_a_epoca(t: std::time::SystemTime) -> i128 {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as i128,
        Err(e) => -(e.duration().as_nanos() as i128),
    }
}

/// A [`Ficha`] de `caminho`, pelo `stat` de agora.
fn ficha_de(caminho: &Path) -> Result<Ficha> {
    let m = std::fs::metadata(caminho)?;
    let lido_em_ns = ns_desde_a_epoca(std::time::SystemTime::now());
    Ok(Ficha {
        bytes: m.len(),
        mtime_ns: m.modified().map(ns_desde_a_epoca).unwrap_or(i128::MIN),
        identidade: identidade(&m),
        lido_em_ns,
    })
}

/// A decisao da fase 2 para UM arquivo que a fase 1 ja copiou: recopiar?
///
/// Tres redes, e basta uma acusar:
///
/// 1. o `stat` mudou -- tamanho, `mtime` ou inode;
/// 2. o diario da tabela do arquivo cresceu desde a fase 1
///    (`eventos_agora != eventos_antes`): e a rede para o relogio que RECUA,
///    quando a escrita deixa o `mtime` igual ou mais velho;
/// 3. o «racily clean»: o `mtime` da fase 1 estava a menos de
///    [`JANELA_RACY_NS`] do proprio `stat` (ou no futuro) -- uma escrita no
///    mesmo tique nao mudaria o `mtime`, entao recopia sem perguntar.
///
/// Pura de proposito: e a unica decisao da fase 2, e se prova sem disco.
pub fn precisa_recopiar(
    antes: &Ficha,
    agora: &Ficha,
    eventos_antes: u64,
    eventos_agora: u64,
) -> bool {
    if antes.bytes != agora.bytes
        || antes.mtime_ns != agora.mtime_ns
        || antes.identidade != agora.identidade
    {
        return true;
    }
    if eventos_agora != eventos_antes {
        return true;
    }
    antes.lido_em_ns.saturating_sub(antes.mtime_ns) < JANELA_RACY_NS
}

/// O que a fase 1 anotou de cada arquivo, pelo caminho relativo.
#[derive(Debug, Default)]
pub struct Inventario {
    fichas: BTreeMap<String, Ficha>,
}

impl Inventario {
    /// Quantos arquivos a fase 1 copiou.
    pub fn len(&self) -> usize {
        self.fichas.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fichas.is_empty()
    }

    /// A soma dos tamanhos no `stat` da fase 1.
    pub fn bytes(&self) -> u64 {
        self.fichas.values().map(|f| f.bytes).sum()
    }
}

/// O pedido de ZIP de uma corrida em duas fases: a arvore temporaria vira o
/// arquivo unico DEPOIS da trava ([`concluir_zip`]).
#[derive(Debug)]
struct PedidoDeZip {
    pasta: PathBuf,
    banco: String,
    admin: String,
    quando_ms: i64,
}

/// A corrida entre a fase 1 e a fase 2: o que ja esta no destino e a ficha de
/// cada arquivo no instante em que foi lido.
#[derive(Debug)]
pub struct Fase1 {
    /// A origem (a raiz, ou `raiz/<banco>` no zip de um banco so).
    origem: PathBuf,
    /// Onde a arvore esta sendo escrita (o destino, ou a arvore temporaria
    /// do zip).
    arvore: PathBuf,
    teto: usize,
    r: Relatorio,
    copias: Copias,
    inventario: Inventario,
    ancora_de: BTreeMap<PathBuf, usize>,
    zip: Option<PedidoDeZip>,
}

/// O que a fase 2 fez, para a resposta e para a bancada.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Acerto {
    pub ms: u64,
    /// Arquivos recopiados, copiados pela primeira vez ou apagados do destino.
    pub arquivos: u64,
    /// Bytes escritos na fase 2.
    pub bytes: u64,
    /// Arquivos que nasceram, sumiram ou trocaram de inode durante a fase 1 --
    /// o que o bloqueio de manutencao existe para evitar, contado e publicado
    /// em vez de suposto.
    pub fora_do_bloqueio: u64,
    /// Arquivos conferidos pelo `stat` (todos os da raiz na fase 2).
    pub conferidos: u64,
}

/// A FASE 1 da copia em arvore: sem trava nenhuma, copia tudo e anota a ficha
/// de cada arquivo. O servidor chama com o retrato ligado so para o bloqueio
/// de manutencao; a consistencia vem inteira da fase 2.
pub fn copiar_fase_1(raiz: &Path, destino: &Path) -> Result<Fase1> {
    copiar_fase_1_com_teto(raiz, destino, teto_de_abertos(), None)
}

fn copiar_fase_1_com_teto(
    origem: &Path,
    arvore: &Path,
    teto: usize,
    zip: Option<PedidoDeZip>,
) -> Result<Fase1> {
    if !origem.is_dir() {
        return Err(PhxError::NaoEncontrado(format!(
            "a raiz de dados {} nao existe",
            origem.display()
        )));
    }
    conferir_destino(origem, arvore, true)?;
    let mut fase = Fase1 {
        origem: origem.to_path_buf(),
        arvore: arvore.to_path_buf(),
        teto,
        // Na passada unica o retrato e' a copia inteira (quem chama exclui o
        // escritor), e o instante dele e' o do comeco; a fase 2, quando ha,
        // o reescreve com o instante dela.
        r: Relatorio {
            retrato_ms: Some(crate::util::agora_ms()),
            ..Relatorio::default()
        },
        copias: Copias {
            destino: arvore.to_path_buf(),
            ..Copias::default()
        },
        inventario: Inventario::default(),
        ancora_de: BTreeMap::new(),
        zip,
    };
    // Pedido 576: a recusa no meio da copia (disco cheio, `EFBIG`, nome
    // ocupado) sai daqui com o que esta corrida fez nascer ja apagado --
    // senao ficava uma pasta de copias sem manifesto que ninguem reconhece.
    if let Err(e) = fase.copiar_tudo() {
        descartar_corrida(&fase.copias);
        return Err(e);
    }
    Ok(fase)
}

/// A FASE 2: sem escritor (quem chama garante), confere o `stat` de cada
/// arquivo da origem contra a ficha da fase 1 e acerta o destino. `eventos` e
/// o diario que andou por tabela desde a fase 1 (`congelamento::chave` ->
/// eventos), a terceira rede de [`precisa_recopiar`]; vazio na copia em uma
/// passada. O `retrato_ms` do manifesto e o instante desta chamada.
pub fn acertar_fase_2(fase: &mut Fase1, eventos: &BTreeMap<PathBuf, u64>) -> Result<Acerto> {
    let inicio = std::time::Instant::now();
    fase.r.retrato_ms = Some(crate::util::agora_ms());
    let feito = fase.acertar(eventos);
    match feito {
        Ok(mut acerto) => {
            acerto.ms = inicio.elapsed().as_millis() as u64;
            Ok(acerto)
        }
        Err(e) => {
            descartar_corrida(&fase.copias);
            Err(e)
        }
    }
}

// HIPOTESE MORTA (pedido 646): sincronizar as copias da fase 1 ANTES da fase
// 2, ainda fora da trava (a antiga `sincronizar_fase_1`), para o escritor que
// espera a fase 2 voltar a um disco ja limpo. A 561 MiB parecia comprar
// ~100 ms. A 1.123 MiB (bancada `retrato-com-escritor.py`, 3 voltas por
// lado, 02/10/2026) o maximo de uma escrita DURANTE a fase 1 foi 398
// [370-439] ms (A) e 404 [312-1.563] ms (B) COM o `fsync` antes, contra 32
// [11-72] e 89 [30-107] ms SEM -- o `fsync` do grosso disputa E/S com o
// escritor que ainda anda --, e DEPOIS da fase 1 as faixas se cruzam nos
// dois cenarios: nada comprado. Reconferido em 06/10/2026 com a maquina
// carregada (load 6-8 em 4 nucleos, p99 da fase 1 5-8x o de 02/10), 5 voltas
// de cada binario na mesma janela: 224 [137-444] contra 498 [342-557] ms (A) e
// 228 [178-6.336] contra 460 [359-5.197] ms (B) -- metade na mediana, faixas
// se cruzando, e o depois da fase 1 sem diferenca (rotulos
// `duas_passadas_1gb_646*` do `resultados.json`); o aceite de <= ~100 ms so'
// se viu com a maquina quieta. No cenario B ainda pagava duas vezes os ~27%
// que a fase 2 reescreve. Por isso nenhuma das fases sincroniza: as copias
// ficam sujas e o [`concluir`] sincroniza tudo depois da fase 2, fora da
// trava. A garantia nao muda: o manifesto so' nasce depois do `fsync` (524
// C2) e o velho sai antes da primeira copia (577), de modo que uma queda na
// fase 2 deixa um destino sem manifesto, que `op_backups` nao lista e o
// `restaurar` recusa.

/// Fecha a corrida em arvore: o relatorio (em ordem de caminho, para dois
/// backups da mesma coisa darem manifestos comparaveis) e as copias para
/// [`concluir`].
pub fn terminar(mut fase: Fase1) -> (Relatorio, Copias) {
    fase.r.arquivos.sort_by(|a, b| a.caminho.cmp(&b.caminho));
    (fase.r, fase.copias)
}

impl Fase1 {
    /// O que a fase 1 anotou. So para medir e para teste.
    pub fn inventario(&self) -> &Inventario {
        &self.inventario
    }

    /// O laco da fase 1 -- o corpo de [`copiar_fase_1`], separado para o erro
    /// dele passar pela faxina da corrida num lugar so'.
    fn copiar_tudo(&mut self) -> Result<()> {
        // Pedido 568: daqui para dentro o destino se percorre pelo descritor
        // de cada pasta, sem seguir link em componente nenhum. Na arvore o
        // proprio `arvore` e o unico nome que se segue: ele e escolha de quem
        // chama. No zip, NAO: o `<nome>.retrato.part` e' nome nosso, previsivel,
        // numa pasta onde terceiros escrevem -- ver [`nascer_arvore_do_zip`].
        let aberto = if self.zip.is_some() {
            nascer_arvore_do_zip(&self.arvore, &mut self.copias.pastas)?
        } else {
            criar_pasta_da_corrida(&self.arvore, &mut self.copias.pastas)?;
            crate::util::Pasta::abrir(&self.arvore)?
        };
        // Pedido 611 (S7): o nome ja foi conferido la em cima, mas quem
        // recebe as copias e o descritor -- e e ele que se confere, antes da
        // primeira.
        conferir_destino_aberto(&self.origem, &self.arvore, &aberto, true)?;
        self.copias.ancoras.push(Arc::new(aberto));
        self.ancora_de.insert(PathBuf::new(), 0);
        let arquivos = listar(&self.origem)?;
        // Pedido 577: o manifesto de uma corrida ANTERIOR sai antes da
        // primeira escrita. Pasta reaproveitada que falha no meio fica com
        // copias ja sobrescritas (o nome nao e' desta corrida, a faxina do
        // 576 nao as tira, e esta certo nao tirar), e o `backup.json` velho
        // continuaria dizendo «pronto» com SHA que nao bate mais. Sem ele,
        // `op_backups` nao a lista e o `restaurar` a recusa inteira ("nao e
        // um backup do PhxSql") em vez de confiar num manifesto que mente.
        // So' depois do `listar`: se a leitura da raiz recusa, nenhuma copia
        // mudou e o backup velho continua valendo.
        invalidar_manifesto_velho(&self.copias)?;
        for arquivo in arquivos {
            self.copiar_um(&arquivo)?;
        }
        Ok(())
    }

    /// `stat` ANTES de ler, e depois a copia: a ficha diz o que o arquivo era
    /// no maximo no instante em que a leitura comecou. Uma escrita entre o
    /// `stat` e o `read` muda o `mtime` para depois da ficha, e a fase 2 a ve.
    fn copiar_um(&mut self, arquivo: &Path) -> Result<Option<u64>> {
        let rel = relativo(&self.origem, arquivo);
        let ficha = match ficha_de(arquivo) {
            Ok(f) => f,
            // Sumiu entre o `listar` e o `stat` (tabela apagada no meio): nao
            // ha o que copiar, e a fase 2 nao o vera na origem.
            Err(PhxError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let dados = match std::fs::read(arquivo) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let bytes = dados.len() as u64;
        self.gravar(&rel, &dados, ficha)?;
        Ok(Some(bytes))
    }

    /// Grava `dados` como a copia de `rel` -- a primeira vez (fase 1, ou
    /// arquivo que nasceu) empilha nas [`Copias`]; a segunda (fase 2) troca
    /// NO LUGAR, no mesmo indice, para as listas paralelas continuarem
    /// paralelas e o `fsync` de depois alcancar o descritor novo.
    fn gravar(&mut self, rel: &str, dados: &[u8], ficha: Ficha) -> Result<()> {
        let alvo = self.arvore.join(rel);
        let rel_p = Path::new(rel);
        let Some(nome) = rel_p.file_name() else {
            return Ok(());
        };
        let pasta = entrar_na_pasta(&mut self.copias, &mut self.ancora_de, rel_p.parent())?;
        let por_dentro = self.copias.ancoras[pasta].por_dentro(nome);
        // O `lstat` ANTES de escrever e' o que separa a copia que nasceu
        // agora da que ja existia (destino reaproveitado): so' a primeira e'
        // desta corrida, e so' ela pode sair numa falha.
        let nasce = std::fs::symlink_metadata(&por_dentro).is_err();
        let arquivo = escrever_sem_sync(&por_dentro, dados)
            .map_err(|e| no_nome_real(e, &por_dentro, &alvo))?;
        if nasce {
            self.copias.nascidas.push(por_dentro.clone());
        }
        let anotado = identidade(&arquivo.metadata()?);
        let bytes = dados.len() as u64;
        let registro = Arquivo {
            caminho: rel.to_string(),
            bytes,
            sha256: para_hex(&sha256(dados)),
        };
        let indice = self.copias.indice.get(rel).copied();
        match indice {
            Some(i) => {
                self.r.bytes = self.r.bytes - self.r.arquivos[i].bytes + bytes;
                self.r.arquivos[i] = registro;
                self.copias.reabrir[i] = (por_dentro, anotado);
                // O `recriar_no_destino` reabre o MESMO inode (a copia e'
                // nossa) e o trunca: o descritor velho apontava para o que
                // acabou de ser reescrito, e o novo o substitui. A copia que
                // ja estava alem do teto continua fechada.
                let tinha = self.copias.abertos[i].is_some();
                self.copias.abertos[i] = tinha.then_some(arquivo);
            }
            None => {
                // Pedido 552: o descritor viaja ate o `fsync`, ate o teto;
                // alem dele fecha aqui e a copia volta ao caminho de reabrir
                // -- que confere o inode anotado agora (irmao do 570).
                self.copias.reabrir.push((por_dentro, anotado));
                self.copias.onde.push(pasta);
                let seguras = self.copias.abertos.iter().filter(|a| a.is_some()).count();
                self.copias
                    .abertos
                    .push((seguras < self.teto).then_some(arquivo));
                self.copias.caminhos.push(alvo);
                self.copias
                    .indice
                    .insert(rel.to_string(), self.r.arquivos.len());
                self.r.bytes += bytes;
                self.r.arquivos.push(registro);
            }
        }
        self.inventario.fichas.insert(rel.to_string(), ficha);
        Ok(())
    }

    /// Tira do destino a copia de `rel`, que sumiu da origem durante a fase
    /// 1, e a tira das listas paralelas no mesmo indice.
    fn apagar(&mut self, rel: &str) {
        let Some(i) = self.copias.indice.remove(rel) else {
            return;
        };
        let (por_dentro, _) = self.copias.reabrir.remove(i);
        descartar_parcial(&por_dentro);
        self.copias.nascidas.retain(|p| p != &por_dentro);
        self.copias.caminhos.remove(i);
        self.copias.abertos.remove(i);
        self.copias.onde.remove(i);
        let registro = self.r.arquivos.remove(i);
        self.r.bytes -= registro.bytes;
        for v in self.copias.indice.values_mut() {
            if *v > i {
                *v -= 1;
            }
        }
        self.inventario.fichas.remove(rel);
    }

    /// O corpo de [`acertar_fase_2`].
    fn acertar(&mut self, eventos: &BTreeMap<PathBuf, u64>) -> Result<Acerto> {
        let mut acerto = Acerto::default();
        let mut vistos: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for arquivo in listar(&self.origem)? {
            let rel = relativo(&self.origem, &arquivo);
            acerto.conferidos += 1;
            let agora = match ficha_de(&arquivo) {
                Ok(f) => f,
                Err(PhxError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            };
            vistos.insert(rel.clone());
            let recopia = match self.inventario.fichas.get(&rel) {
                None => {
                    acerto.fora_do_bloqueio += 1;
                    true
                }
                Some(antes) => {
                    if antes.identidade != agora.identidade {
                        acerto.fora_do_bloqueio += 1;
                    }
                    let n = eventos_da_tabela(eventos, &self.origem, &rel);
                    precisa_recopiar(antes, &agora, 0, n)
                }
            };
            if recopia {
                if let Some(bytes) = self.copiar_um(&arquivo)? {
                    acerto.arquivos += 1;
                    acerto.bytes += bytes;
                }
            }
        }
        let sumidos: Vec<String> = self
            .inventario
            .fichas
            .keys()
            .filter(|rel| !vistos.contains(*rel))
            .cloned()
            .collect();
        for rel in sumidos {
            self.apagar(&rel);
            acerto.arquivos += 1;
            acerto.fora_do_bloqueio += 1;
        }
        Ok(acerto)
    }
}

/// Os eventos, em `eventos` (por `congelamento::chave`), da tabela a que o
/// arquivo `rel` pertence -- zero se nenhuma tabela dele andou.
///
/// Um arquivo de tabela e `<dir>/<nome><sufixo>.<ext>`, com o sufixo do
/// volume vazio, `#NNNN` ou `_NNNN`: pertence a `<dir>/<nome>` quando o
/// caminho (pela mesma regua da chave: absoluto, em minusculas) comeca pela
/// chave e o que sobra comeca por `.`, `#` ou `_`. Tabela `a` e arquivo
/// `a_b.reg` de `a_b` casam de mais -- custa uma recopia a mais, nunca uma
/// a menos.
fn eventos_da_tabela(eventos: &BTreeMap<PathBuf, u64>, origem: &Path, rel: &str) -> u64 {
    if eventos.is_empty() {
        return 0;
    }
    let caminho = origem.join(rel);
    let (Some(dir), Some(nome)) = (caminho.parent(), caminho.file_name()) else {
        return 0;
    };
    let do_arquivo = crate::congelamento::chave(dir, &nome.to_string_lossy())
        .to_string_lossy()
        .into_owned();
    eventos
        .iter()
        .filter(|(tabela, _)| {
            let t = tabela.to_string_lossy();
            do_arquivo
                .strip_prefix(t.as_ref())
                .is_some_and(|resto| resto.starts_with(['.', '#', '_']))
        })
        .map(|(_, n)| *n)
        .sum()
}

/// O indice, em `copias.ancoras`, da pasta `rel` (relativa ao destino) --
/// aberta componente a componente pela [`crate::util::Pasta`], criando o que
/// falta, e guardada para as copias seguintes da mesma pasta (o `listar`
/// devolve em ordem, entao cada pasta se abre uma vez so). Pedido 568: um
/// link em qualquer componente recusa aqui, antes de uma copia cair do outro
/// lado dele.
fn entrar_na_pasta(
    copias: &mut Copias,
    ancora_de: &mut BTreeMap<PathBuf, usize>,
    rel: Option<&Path>,
) -> Result<usize> {
    let mut atual = PathBuf::new();
    let mut indice = 0;
    for componente in rel.into_iter().flat_map(Path::components) {
        atual.push(componente);
        if let Some(&i) = ancora_de.get(&atual) {
            indice = i;
            continue;
        }
        let nova = crate::util::Pasta::entrar(
            &copias.ancoras[indice],
            componente.as_os_str(),
            &mut copias.pastas,
        )?;
        copias.ancoras.push(Arc::new(nova));
        indice = copias.ancoras.len() - 1;
        ancora_de.insert(atual.clone(), indice);
    }
    Ok(indice)
}

/// Apaga o `backup.json` que ja estava no destino -- pedido 577.
///
/// Pedido 611 (S6): pela pasta que a corrida ABRIU ([`no_destino`]), e nao
/// por `destino.join`. Pelo nome, a troca do destino por um link entre o
/// `Pasta::abrir` e este passo fazia o `remove_file` atravessar o link e
/// apagar o manifesto de OUTRO backup -- que o `restaurar` passava a recusar
/// inteiro --, enquanto as copias iam para a pasta aberta. Medido contra o
/// SO com a troca na janela (`tests/destino-trocado-na-janela.rs`).
///
/// `remove_file` nao segue link nem abre FIFO no ULTIMO nome (pedidos
/// 568/570): se o nome for um link, some o link e o alvo fica. Um DIRETORIO nesse nome nao e'
/// manifesto de ninguem (o `restaurar` e o `conferir` pedem arquivo) e nao se
/// apaga -- nada recursivo aqui. A recusa do `remove_file` SOBE: seguir
/// sobrescrevendo copias com o manifesto velho no lugar e' o defeito que este
/// passo existe para impedir, e ate aqui nenhuma copia mudou.
///
/// O `unlink` nao ganha `fsync` do diretorio AQUI -- ele roda sob a trava de
/// dados, e a catraca `alcancam-fsync-2` so' desce. Ganha em
/// [`sincronizar_copias`], fora da trava e antes do manifesto novo (pedido
/// 579). **O que fica, nao medido:** uma queda da MAQUINA entre este `unlink`
/// e aquele `fsync` ainda pode devolver o manifesto velho junto de copias ja
/// sobrescritas -- e o [`conferir`] o acusa pelo SHA. Fechar essa janela pede
/// o `fsync` sob a trava, ou a remocao antes dela em cada chamador.
fn invalidar_manifesto_velho(copias: &Copias) -> Result<()> {
    let manifesto = no_destino(copias, MANIFESTO);
    match std::fs::symlink_metadata(&manifesto) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
        Ok(m) if m.is_dir() => Ok(()),
        Ok(_) => Ok(std::fs::remove_file(&manifesto)?),
    }
}

/// O que UMA corrida de [`executar`] fez no destino -- pedido 576.
///
/// Existe porque a pasta do backup, no `op backup`, e' escolhida pelo usuario
/// e pode ter outra coisa dentro: numa falha, a unica faxina segura e' a que
/// sabe, pela lista da propria corrida, o que ela criou. Varrer a pasta ou
/// apagar a arvore reabriria a porta do link (pedidos 568/569/570).
///
/// Derefere para a lista de copias escritas, que e' o que
/// [`sincronizar_copias`] percorre.
#[derive(Debug, Default)]
pub struct Copias {
    /// A pasta do backup: a que perdeu o `backup.json` velho (pedido 579).
    destino: PathBuf,
    /// Toda copia escrita, na ordem -- inclusive as que sobrescreveram um
    /// nome que ja existia; todas precisam de `fsync`.
    caminhos: Vec<PathBuf>,
    /// Paralela a `caminhos`: o descritor de quem escreveu, ainda aberto
    /// para o `fsync` (pedido 552); `None` na copia que passou do teto.
    abertos: Vec<Option<File>>,
    /// Paralela a `caminhos`: o nome pelo descritor da pasta e o inode que a
    /// escrita deixou -- o que a copia alem do teto reabre e confere (irmao
    /// do pedido 570).
    reabrir: Vec<(PathBuf, Option<(u64, u64)>)>,
    /// Das copias, as cujo NOME nasceu nesta corrida: so' estas saem numa
    /// falha. A que sobrescreveu um nome antigo nao e' nossa para apagar.
    /// Guardadas pelo nome ALCANCADO PELO DESCRITOR da pasta (pedido 568): a
    /// faxina nao atravessa um link que alguem pos no meio do caminho.
    nascidas: Vec<PathBuf>,
    /// As pastas do destino abertas por descritor (pedido 568); a primeira e
    /// o proprio destino. Vivem ate o fim da corrida: os nomes de `reabrir` e
    /// `nascidas` so valem enquanto elas estao abertas. `Arc` porque a pasta
    /// nascida guarda a mae dela (pedido 593).
    ancoras: Vec<Arc<crate::util::Pasta>>,
    /// Paralela a `caminhos`: o indice, em `ancoras`, da pasta da copia --
    /// a que recebe o `fsync` da entrada dela (pedido 593).
    onde: Vec<usize>,
    /// Diretorios que esta corrida criou, do mais externo para o mais
    /// interno, cada um com a mae aberta e o inode (pedido 593).
    pastas: Vec<crate::util::Nascida>,
    /// Caminho relativo -> indice nas listas paralelas: e' o que deixa a
    /// fase 2 trocar uma copia NO LUGAR (pedido 513, passo 2).
    indice: BTreeMap<String, usize>,
}

impl std::ops::Deref for Copias {
    type Target = [PathBuf];
    fn deref(&self) -> &[PathBuf] {
        &self.caminhos
    }
}

/// Cria `p` (com os pais que faltarem) e anota em `criadas` os que ESTA
/// chamada fez nascer -- pedido 576.
///
/// Anota pelo que existe DEPOIS da chamada, com erro ou sem: o `create_dir_all`
/// que recusa no meio ja deixou os de cima criados, e sem anota-los a faxina
/// nao os alcancaria.
///
/// Pedido 593: anota ABRINDO -- a mae da mais externa pelo nome (e o caminho
/// que quem chama escolheu, como o proprio destino na `Pasta`) e cada nascida
/// dali para dentro pelo descritor da de cima, sem seguir link. A faxina as
/// remove por esse descritor, conferindo o inode; a que nao abre (um link ja
/// plantado no nome) nao se anota, e nao sai.
fn criar_pasta_da_corrida(p: &Path, criadas: &mut Vec<crate::util::Nascida>) -> Result<()> {
    let mut faltam = Vec::new();
    let mut atual = Some(p);
    while let Some(a) = atual {
        if a.as_os_str().is_empty() || std::fs::symlink_metadata(a).is_ok() {
            break;
        }
        faltam.push(a.to_path_buf());
        atual = a.parent();
    }
    let criado = crate::util::criar_diretorio_do_banco(p);
    let mae_de_todas = faltam.last().and_then(|a| a.parent()).map(|m| {
        if m.as_os_str().is_empty() {
            Path::new(".")
        } else {
            m
        }
    });
    if let Some(Ok(mae)) = mae_de_todas.map(crate::util::Pasta::abrir) {
        let mut mae = Arc::new(mae);
        let mut nenhuma = Vec::new();
        for a in faltam.iter().rev() {
            let Some(nome) = a.file_name() else { break };
            let Ok(filha) = crate::util::Pasta::entrar(&mae, nome, &mut nenhuma) else {
                break;
            };
            criadas.push(crate::util::Nascida::anotar(mae, nome, &filha));
            mae = Arc::new(filha);
        }
    }
    Ok(criado?)
}

/// A arvore temporaria do zip (`<pasta>/<nome>.retrato.part/`) -- pedido 651
/// (a). Ela tem de NASCER nesta corrida, e e' o descritor de quem nasceu que
/// recebe as copias.
///
/// Medido antes do conserto (testes `retrato_part_plantado_*`): o nome e'
/// previsivel (banco, admin e minuto) e a pasta dos zips e' lugar onde
/// terceiros escrevem. Com o `create_dir_all` + `Pasta::abrir` da arvore
/// comum, um link plantado nesse nome era SEGUIDO -- a copia do `.reg`
/// reescrevia o arquivo de mesmo nome na pasta-isca e o zip levava o
/// conteudo dela --, e uma pasta de verdade plantada ja cheia era
/// aproveitada, com o intruso entrando no zip (e virando tabela na
/// restauracao). O link fisico no nome de uma copia ja era recusado pelo
/// motor do 569 ([`crate::util::recriar_no_destino`]).
///
/// Por isso: `mkdir` SEM `-p` no ultimo nome, pelo descritor da mae
/// (`AlreadyExists` para link, pasta ou arquivo que ja estava la), entrada
/// sem seguir link ([`crate::util::Pasta::entrar`]), e o que abriu tem de
/// estar VAZIO e ser do dono do processo -- quem trocar a nossa pasta
/// recem-nascida por outra entre o `mkdir` e a abertura e' pego aqui. A
/// janela que sobra e' a de quem tem o MESMO uid, que ja pode tudo no
/// destino. O nome ocupado recusa, e nao se apaga: nao e' desta corrida (um
/// orfao velho nosso sai pela faxina de [`limpar_parciais_orfaos`]).
fn nascer_arvore_do_zip(
    arvore: &Path,
    pastas: &mut Vec<crate::util::Nascida>,
) -> Result<crate::util::Pasta> {
    let recusa = |por_que: &str| {
        PhxError::Esquema(format!(
            "{}: {por_que}. O zip em duas passadas so copia para a pasta \
             temporaria que ele mesmo fez nascer -- seguir um link ou \
             aproveitar uma pasta que ja estava ali levaria a copia do banco \
             para fora do destino, ou poria no zip o que outro deixou nela. \
             Tire esse nome da pasta dos zips (ou espere o minuto seguinte) e \
             repita",
            arvore.display()
        ))
    };
    let (Some(pasta), Some(nome)) = (arvore.parent(), arvore.file_name()) else {
        return Err(recusa("a pasta temporaria do zip nao tem pasta mae"));
    };
    criar_pasta_da_corrida(pasta, pastas)?;
    let mae = Arc::new(crate::util::Pasta::abrir(pasta)?);
    match crate::util::criar_diretorio_novo_do_banco(&mae.por_dentro(nome)) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(recusa(
                "o nome ja existe (um link, uma pasta ou um arquivo que esta \
                 corrida nao criou)",
            ));
        }
        Err(e) => return Err(e.into()),
    }
    let mut nenhuma = Vec::new();
    let filha = crate::util::Pasta::entrar(&mae, nome, &mut nenhuma)
        .map_err(|_| recusa("o nome foi trocado logo depois de nascer"))?;
    let vazia = std::fs::read_dir(filha.por_dentro(std::ffi::OsStr::new(".")))?
        .next()
        .is_none();
    if !vazia || !crate::util::do_processo(&filha.metadados()?) {
        return Err(recusa(
            "a pasta aberta nao e' a que esta corrida criou (tem conteudo, ou \
             outro dono)",
        ));
    }
    pastas.push(crate::util::Nascida::anotar(mae, nome, &filha));
    Ok(filha)
}

/// A faxina de uma corrida que falhou -- pedido 576, pelo motor do 555.
///
/// So' apaga o que a corrida anotou: cada copia pelo NOME
/// ([`descartar_parcial`], `remove_file`, que nunca segue link nem abre
/// FIFO), e depois cada pasta criada, da mais interna para a mais externa,
/// com `remove_dir` -- que so' remove diretorio VAZIO e recusa link. Pasta
/// com qualquer coisa dentro que nao era nossa fica, com a coisa dentro.
/// Falhar aqui nao muda o erro que sobe: e' esse que importa a quem chamou.
fn descartar_corrida(copias: &Copias) {
    for arquivo in &copias.nascidas {
        descartar_parcial(arquivo);
    }
    descartar_pastas(&copias.pastas);
}

/// A metade das pastas de [`descartar_corrida`], separada para o
/// [`finalizar_zip`] -- que nao tem copia nenhuma, so o `.part` e as pastas
/// -- pagar o MESMO motor (pedido 579) em vez de um segundo laco de
/// `remove_dir`.
///
/// Pedido 593: cada uma sai pelo descritor da MAE e so se ainda e a que
/// nasceu (dev/inode) -- [`crate::util::Nascida::remover`], com a janela que
/// sobra dita la. Pelo nome real, um link posto numa pasta do meio fazia o
/// `remove_dir` apagar a pasta vazia de outro do lado de la.
fn descartar_pastas(pastas: &[crate::util::Nascida]) {
    for pasta in pastas.iter().rev() {
        pasta.remover();
    }
}

/// Os dois passos de depois da copia -- [`sincronizar_copias`] e
/// [`finalizar_manifesto`] -- com a faxina do pedido 576 no erro.
///
/// E' o caminho dos tres chamadores (CLI, `backup` pelo protocolo e o
/// agendado): uma recusa no `fsync` de uma copia ou na escrita do manifesto
/// deixava as copias na pasta sem `backup.json`, e nem `op_backups` nem a
/// rotacao as reconhecem -- ficavam para sempre. Roda FORA da trava de dados,
/// como os dois passos que envolve.
///
/// `destino` fica na assinatura pelos chamadores; o manifesto vai para a
/// pasta que a corrida ABRIU (pedido 593), a mesma das copias.
pub fn concluir(destino: &Path, quando_ms: i64, r: &Relatorio, copias: &Copias) -> Result<()> {
    let _ = destino;
    let manifesto = no_destino(copias, MANIFESTO);
    let manifesto_nasce = std::fs::symlink_metadata(&manifesto).is_err();
    let feito = sincronizar_copias(copias).and_then(|()| finalizar_manifesto(copias, quando_ms, r));
    if feito.is_err() {
        // O manifesto que nasceu agora e nao sincronizou diria «pronto» sobre
        // copias que acabam de sair; o que ja existia antes nao e' nosso.
        if manifesto_nasce {
            descartar_parcial(&manifesto);
        }
        descartar_corrida(copias);
    }
    feito
}

/// Escreve e sincroniza o `backup.json` -- so DEPOIS de [`sincronizar_copias`] ter
/// confirmado cada copia no disco (pedido 524, condicao C2). O conteudo sai
/// do `Relatorio` que [`executar`] ja montou em RAM, sem reler nada.
///
/// Antes desta chamada o destino nao tem manifesto: uma recusa de `fsync`
/// numa copia, no meio do caminho, nunca chega aqui, e a pasta fica sem
/// `backup.json` -- o sinal de "nao terminou" que `op_backups` e
/// [`conferir`] ja sabem ler.
///
/// O `fsync` e no descritor que escreveu (pedido 552), e depois vem o da
/// pasta (pedido 579): o nome do manifesto e uma entrada de diretorio, e sem
/// ela duravel a queda podia voltar sem ele -- ou com o velho.
///
/// Pedido 593: o manifesto nasce na pasta que a corrida ABRIU e sincroniza no
/// descritor dela, e nao pelo nome do destino -- o `fsync` pelo nome, fora da
/// trava, caia do outro lado de um link posto no lugar. Por isso recebe as
/// [`Copias`], e nao o caminho.
pub fn finalizar_manifesto(copias: &Copias, quando_ms: i64, r: &Relatorio) -> Result<()> {
    let manifesto = no_destino(copias, MANIFESTO);
    let real = copias.destino.join(MANIFESTO);
    let arquivo = escrever_sem_sync(&manifesto, r.para_json(quando_ms).escrever().as_bytes())
        .map_err(|e| no_nome_real(e, &manifesto, &real))?;
    crate::sincronia::sync_all_sem_abortar(&arquivo, &real)?;
    match copias.ancoras.first() {
        Some(destino) => sincronizar_pasta(destino),
        None => crate::sincronia::sincronizar_pasta_sem_abortar(&copias.destino, &real),
    }
}

/// O nome `nome` dentro do destino, pelo descritor que a corrida abriu; sem
/// ele (corrida que nem chegou a abrir), pelo caminho real.
fn no_destino(copias: &Copias, nome: &str) -> PathBuf {
    match copias.ancoras.first() {
        Some(d) => d.por_dentro(std::ffi::OsStr::new(nome)),
        None => copias.destino.join(nome),
    }
}

/// Le o manifesto e confere cada arquivo do destino, byte a byte.
///
/// Acha as tres coisas que estragam um backup: arquivo que sumiu, arquivo que
/// mudou e arquivo que apareceu sem estar no manifesto.
pub fn conferir(destino: &Path) -> Result<Relatorio> {
    let texto = std::fs::read_to_string(destino.join(MANIFESTO)).map_err(|e| {
        PhxError::NaoEncontrado(format!("{} nao tem {MANIFESTO}: {e}", destino.display()))
    })?;
    let manifesto = Json::analisar(&texto)?;

    let mut esperados: BTreeMap<String, (u64, String)> = BTreeMap::new();
    if let Some(lista) = manifesto.campo("conteudo").and_then(Json::lista) {
        for a in lista {
            esperados.insert(
                a.texto_ou("caminho", "").to_string(),
                (
                    a.campo("bytes").and_then(Json::inteiro).unwrap_or(0) as u64,
                    a.texto_ou("sha256", "").to_string(),
                ),
            );
        }
    }

    let mut r = Relatorio::default();
    let mut vistos = BTreeMap::new();
    for arquivo in listar(destino)? {
        let rel = relativo(destino, &arquivo);
        if rel == MANIFESTO {
            continue;
        }
        let dados = std::fs::read(&arquivo)?;
        let sha = para_hex(&sha256(&dados));
        vistos.insert(rel.clone(), ());
        match esperados.get(&rel) {
            None => r
                .divergencias
                .push(format!("{rel}: existe no destino e nao esta no manifesto")),
            Some((bytes, esperado)) => {
                if *bytes != dados.len() as u64 {
                    r.divergencias.push(format!(
                        "{rel}: o manifesto diz {bytes} bytes, o arquivo tem {}",
                        dados.len()
                    ));
                } else if *esperado != sha {
                    r.divergencias
                        .push(format!("{rel}: o conteudo mudou (SHA-256 diferente)"));
                }
            }
        }
        r.bytes += dados.len() as u64;
        r.arquivos.push(Arquivo {
            caminho: rel,
            bytes: dados.len() as u64,
            sha256: sha,
        });
    }

    for caminho in esperados.keys() {
        if !vistos.contains_key(caminho) {
            r.divergencias
                .push(format!("{caminho}: esta no manifesto e sumiu do destino"));
        }
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    fn temp(nome: &str) -> crate::apoio_teste::DirTemp {
        crate::apoio_teste::DirTemp::novo(&format!("bkp-{nome}"))
    }

    /// **Pedido 554: a recusa do `fsync` no destino do backup nao alcanca a
    /// raiz de dados.**
    ///
    /// O leiaute de fabrica: raiz em `<base>/dados`, backups em
    /// `<base>/backups`. O primeiro backup CRIA `backups/`, e o `fsync` da mae
    /// dela -- `<base>`, ancestral da raiz -- e o que a arma recusa. Antes do
    /// conserto a marca ia para a lista do banco, conferida por prefixo, e o
    /// `fsync` seguinte de qualquer arquivo da raiz (o de um COMMIT) recusava
    /// ate o processo reiniciar. Com o defeito reposto (`recusar` no lugar de
    /// `recusar_fora` na via `sem_abortar`), este teste FALHA no `sync_all`
    /// da tabela.
    ///
    /// E o comportamento velho fica: a recusa no destino continua virando
    /// erro do backup, e repetir o `fsync` da MESMA pasta continua recusado
    /// (o que o 509 compra).
    #[cfg(unix)]
    #[test]
    fn recusa_no_destino_nao_alcanca_a_raiz_de_dados() {
        let base = temp("fsync-554");
        let raiz = base.join("dados");
        let destino = base.join("backups").join("corrida");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);

        let (r, copias) = executar(&raiz, &destino, 1_787_000_000_000).unwrap();
        // A marca de uma pasta e gravada pelo manifesto DENTRO dela: armar
        // `<base>/backup.json` alcanca so o `fsync` de `<base>`, a mae de
        // `backups/` -- nem as copias, nem as outras pastas.
        let mae = base.join(MANIFESTO);
        crate::sincronia::falha_de_teste::armar(
            &mae,
            crate::sincronia::falha_de_teste::Onde::Fsync,
            1,
        );
        let feito = concluir(&destino, 1_787_000_000_000, &r, &copias);
        crate::sincronia::falha_de_teste::desarmar(&mae);
        let e = feito.expect_err("a recusa no destino tem de virar erro do backup");
        assert!(matches!(e, PhxError::Io(_)), "familia errada: {e}");

        // O COMMIT seguinte: o `fsync` de um arquivo da raiz, pelo caminho
        // do banco (com gancho), tem de passar.
        let tabela = raiz.join("Z/cadastroClientes.reg");
        let arquivo = File::options().write(true).open(&tabela).unwrap();
        crate::sincronia::sync_all(&arquivo, &tabela).unwrap_or_else(|e| {
            panic!("a recusa no destino do backup parou a escrita do banco: {e}")
        });
        assert!(
            crate::sincronia::recusado_em(&tabela).is_none(),
            "a raiz herdou a marca do destino"
        );

        // A repeticao na MESMA pasta continua recusada, sem tocar no disco.
        let e = crate::sincronia::sincronizar_pasta_sem_abortar(&base, &mae)
            .expect_err("repetir o fsync da pasta que recusou responderia Ok sem o dado");
        assert!(e.to_string().contains("554"), "{e}");
    }

    /// **Pedido 611 (S7): a conferencia do descritor nao depende do nome.**
    ///
    /// Sem corrida: a pasta ABERTA e uma (dentro da raiz, ou acima dela) e o
    /// nome passado e outro, inocente -- o retrato exato do que a troca na
    /// janela deixa (a prova com a troca de verdade, contra o SO, esta em
    /// `tests/destino-trocado-na-janela.rs`). Pelo nome as duas passariam; pelo
    /// descritor as duas recusam. E o comportamento velho fica: a pasta de
    /// fora passa, e a acima da raiz passa quando `acima_tambem` e falso (o
    /// zip).
    #[cfg(target_os = "linux")]
    #[test]
    fn o_destino_se_confere_pelo_que_se_abriu_e_nao_pelo_nome() {
        let base = temp("destino-611");
        let raiz = base.join("dados");
        std::fs::create_dir_all(raiz.join("loja")).unwrap();
        std::fs::create_dir_all(base.join("fora")).unwrap();
        let inocente = base.join("fora");
        let abrir = |p: &Path| crate::util::Pasta::abrir(p).unwrap();

        let e = conferir_destino_aberto(&raiz, &inocente, &abrir(&raiz.join("loja")), true)
            .expect_err("a pasta aberta fica DENTRO da raiz");
        assert!(e.to_string().contains("fica dentro da"), "{e}");
        let e = conferir_destino_aberto(&raiz, &inocente, &abrir(&raiz), false)
            .expect_err("a pasta aberta E a raiz");
        assert!(e.to_string().contains("fica dentro da"), "{e}");
        let e = conferir_destino_aberto(&raiz, &inocente, &abrir(&base), true)
            .expect_err("a pasta aberta CONTEM a raiz");
        assert!(e.to_string().contains("contem a"), "{e}");

        conferir_destino_aberto(&raiz, &inocente, &abrir(&inocente), true).unwrap();
        conferir_destino_aberto(&raiz, &inocente, &abrir(&base), false).unwrap();
    }

    /// Pedido 554: o destino que se mistura com a raiz e recusado ANTES de
    /// escrever byte nenhum -- igual, dentro, acima (na arvore) e por um
    /// link que leva para dentro. O zip numa pasta acima da raiz continua
    /// valendo: e um arquivo so, que nao alcanca nada dela.
    #[cfg(unix)]
    #[test]
    fn destino_que_se_mistura_com_a_raiz_e_recusado_antes() {
        let base = temp("destino-554");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let recusa = |destino: &Path, o_que: &str| {
            let e = executar(&raiz, destino, 1)
                .map(|_| ())
                .expect_err(o_que)
                .to_string();
            assert!(e.contains("raiz de dados"), "{o_que}: {e}");
        };
        recusa(&raiz, "a propria raiz");
        recusa(&raiz.join("dentro"), "dentro da raiz");
        recusa(&base, "a pasta que contem a raiz");
        assert!(
            !base.join("Z").exists(),
            "a recusa veio depois de copiar para cima da raiz"
        );
        std::os::unix::fs::symlink(&raiz, base.join("atalho")).unwrap();
        recusa(&base.join("atalho/copia"), "um link para dentro da raiz");
        recusa(&raiz.join("../dados/x"), "um `..` que volta para a raiz");

        let e = executar_zip(&raiz, &raiz.join("zips"), "", "x", 1)
            .map(|_| ())
            .expect_err("zip dentro da raiz");
        assert!(e.to_string().contains("raiz de dados"), "{e}");

        // Comportamento velho: irma da raiz passa, e o zip acima tambem.
        backup_pronto(&raiz, &base.join("copia"), 1);
        let (zip, _) = executar_zip(&raiz, &base, "", "x", 1).unwrap();
        finalizar_zip(&zip).unwrap();
    }

    fn dados_de_exemplo(raiz: &Path) {
        std::fs::create_dir_all(raiz.join("Z/schemaX")).unwrap();
        std::fs::write(raiz.join("Z/cadastroClientes.reg"), b"registros aqui").unwrap();
        std::fs::write(raiz.join("Z/cadastroClientes.ndx"), b"indices aqui").unwrap();
        std::fs::write(raiz.join("Z/cadastroClientes_002.reg"), b"volume dois").unwrap();
        std::fs::write(raiz.join("Z/schemaX/pedidos.reg"), b"pedidos do schema").unwrap();
    }

    /// Os TRES passos que um chamador de verdade faz (`executar`,
    /// `sincronizar_copias`, `finalizar_manifesto`), para os testes que so querem
    /// um backup pronto e nao estao provando a condicao C2 em si.
    fn backup_pronto(raiz: &Path, destino: &Path, quando_ms: i64) -> Relatorio {
        let (r, caminhos) = executar(raiz, destino, quando_ms).unwrap();
        sincronizar_copias(&caminhos).unwrap();
        finalizar_manifesto(&caminhos, quando_ms, &r).unwrap();
        r
    }

    /// Os DOIS passos do ZIP (`executar_zip`, `finalizar_zip`), pelo mesmo
    /// motivo do `backup_pronto`.
    fn zip_pronto(
        raiz: &Path,
        pasta: &Path,
        banco: &str,
        admin: &str,
        quando_ms: i64,
    ) -> (PathBuf, Relatorio) {
        let (alvo, r) = executar_zip(raiz, pasta, banco, admin, quando_ms).unwrap();
        finalizar_zip(&alvo).unwrap();
        (alvo.to_path_buf(), r)
    }

    #[test]
    fn copia_tudo_e_confere() {
        let base = temp("copia");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);

        let r = backup_pronto(&raiz, &destino, 1_787_000_000_000);
        assert_eq!(r.arquivos.len(), 4);
        assert!(r.bytes > 0);
        assert!(destino.join(MANIFESTO).is_file());
        // A hierarquia veio junto.
        assert!(destino.join("Z/schemaX/pedidos.reg").is_file());
        assert!(destino.join("Z/cadastroClientes_002.reg").is_file());

        let c = conferir(&destino).unwrap();
        assert!(
            c.ok(),
            "backup recem-feito tem de conferir: {:?}",
            c.divergencias
        );
        assert_eq!(c.arquivos.len(), 4);
    }

    /// Pedido 524/513: um `fsync` recusado na copia tem de VOLTAR como erro,
    /// e nao como "concluido" -- e a prova so vale forjando a recusa
    /// (`falha_de_teste`), porque montar um disco que recusa de verdade
    /// exige `CAP_SYS_ADMIN` (`bancada/catastrofes/`).
    ///
    /// `executar` so ESCREVE (pedido 513: quem chama pode estar com a trava
    /// de dados na mao, e o `fsync` sob a trava e o que a catraca
    /// `alcancam-fsync-2` proibe); e [`sincronizar_copias`], chamado DEPOIS, quem
    /// tem de acusar a recusa. Com o defeito reposto (`std::fs::write` sem
    /// `sincronia::sync_all` em `sincronizar_arquivo`), este teste FALHA: a
    /// arma nunca dispara, e o `expect_err` entra em panico porque
    /// `sincronizar_copias` devolveu `Ok`.
    #[test]
    fn fsync_recusado_na_copia_vira_erro() {
        let base = temp("fsync-recusado");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);

        let (_, caminhos) = executar(&raiz, &destino, 1_787_000_000_000).unwrap();
        // A arma mira o ARQUIVO copiado, nao a pasta: desde o 579 as pastas
        // tambem sincronizam, e armada no `destino` a recusa da pasta dava o
        // mesmo Err -- a copia sem `fsync` passava (guarda `backup-sem-fsync`,
        // NAO PEGOU 1/2 em 30/09/2026).
        let copia = destino.join("Z/cadastroClientes.reg");
        assert!(
            copia.is_file(),
            "o teste supoe a copia em {}",
            copia.display()
        );
        crate::sincronia::falha_de_teste::armar(
            &copia,
            crate::sincronia::falha_de_teste::Onde::Fsync,
            1,
        );
        let e = sincronizar_copias(&caminhos)
            .expect_err("fsync recusado tem de virar Err, nao Ok silencioso");
        crate::sincronia::falha_de_teste::desarmar(&copia);
        assert!(
            matches!(e, PhxError::Io(_)),
            "familia errada para uma recusa de fsync: {e}"
        );
    }

    /// O mesmo teste do `executar`, para o irmao `executar_zip` -- caminho
    /// IRMAO que a mesma arma tem de alcancar, senao o ZIP mente sozinho.
    #[test]
    fn fsync_recusado_no_zip_vira_erro() {
        let base = temp("fsync-recusado-zip");
        let raiz = base.join("dados");
        let pasta = base.join("zips");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);

        let (zip, _r) = executar_zip(&raiz, &pasta, "", "admin", 1_787_000_000_000).unwrap();
        crate::sincronia::falha_de_teste::armar(
            &pasta,
            crate::sincronia::falha_de_teste::Onde::Fsync,
            1,
        );
        let e = finalizar_zip(&zip)
            .expect_err("fsync recusado no zip tem de virar Err, nao Ok silencioso");
        crate::sincronia::falha_de_teste::desarmar(&pasta);
        assert!(
            matches!(e, PhxError::Io(_)),
            "familia errada para uma recusa de fsync: {e}"
        );
        assert!(
            !zip.is_file(),
            "o nome FINAL nao pode existir sem o fsync ter passado"
        );
    }

    /// Pedido 524, condicao C2 do parecer do DBA: uma recusa de `fsync` NUMA
    /// COPIA nao pode deixar o destino com `backup.json` -- senao
    /// `op_backups` listaria, e `conferir` aprovaria, uma pasta que nunca
    /// terminou.
    ///
    /// Com o defeito reposto (manifesto escrito dentro de `executar`, antes
    /// do `fsync`), este teste FALHA: o `backup.json` existe mesmo com a
    /// copia recusada.
    #[test]
    fn manifesto_nao_nasce_se_uma_copia_nao_sincroniza() {
        let base = temp("c2-manifesto");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);

        let (r, caminhos) = executar(&raiz, &destino, 1_787_000_000_000).unwrap();
        crate::sincronia::falha_de_teste::armar(
            &destino,
            crate::sincronia::falha_de_teste::Onde::Fsync,
            1,
        );
        let erro_sync = sincronizar_copias(&caminhos);
        crate::sincronia::falha_de_teste::desarmar(&destino);
        assert!(erro_sync.is_err(), "a copia tinha de recusar sincronizar");
        assert!(
            !destino.join(MANIFESTO).is_file(),
            "o manifesto nao pode existir sem toda copia sincronizada"
        );
        // O chamador de verdade para aqui (o `?` sobe antes de chegar em
        // `finalizar_manifesto`); a chamada abaixo so' prova que ela tambem
        // nao inventa um manifesto por conta propria.
        let _ = r;
    }

    /// O mesmo C2, do lado do ZIP: uma recusa nao pode deixar o nome FINAL
    /// visivel. E nem o `.part` fica (pedido 555): zip que nao chegou ao nome
    /// final nunca chega, e a rotacao nao o enxergaria para apagar.
    #[test]
    fn zip_final_nao_nasce_se_o_fsync_recusa() {
        let base = temp("c2-zip");
        let raiz = base.join("dados");
        let pasta = base.join("zips");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);

        let (zip, _r) = executar_zip(&raiz, &pasta, "", "admin", 1_787_000_000_000).unwrap();
        crate::sincronia::falha_de_teste::armar(
            &pasta,
            crate::sincronia::falha_de_teste::Onde::Fsync,
            1,
        );
        let erro = finalizar_zip(&zip);
        crate::sincronia::falha_de_teste::desarmar(&pasta);
        assert!(erro.is_err(), "o fsync do zip tinha de recusar");
        assert!(!zip.is_file(), "o nome final nao pode aparecer");
        // A pasta nasceu nesta corrida e ficou vazia: desde o 579 ela sai
        // junto -- entao «nao existe» tambem quer dizer «sem .part».
        assert!(
            std::fs::read_dir(&pasta).map_or(true, |l| l
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().ends_with(".part"))),
            "o .part do fsync recusado ficou na pasta"
        );
        assert!(!pasta.exists(), "a pasta vazia que a corrida criou ficou");
    }

    /// Pedido 552: alem do teto de descritores, a copia fecha na escrita e
    /// volta ao caminho de reabrir -- e o backup continua inteiro. As que
    /// cabem no teto seguem com o descritor de quem escreveu.
    #[test]
    fn alem_do_teto_a_copia_reabre_e_o_backup_fica_inteiro() {
        let base = temp("teto-de-abertos");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);

        let (r, copias) = executar_com_teto(&raiz, &destino, 2).unwrap();
        let seguros = copias.abertos.iter().filter(|a| a.is_some()).count();
        assert_eq!(seguros, 2, "o teto nao segurou o numero de descritores");
        assert_eq!(copias.abertos.len(), 4);
        concluir(&destino, 1_787_000_000_000, &r, &copias).unwrap();
        let c = conferir(&destino).unwrap();
        assert!(c.ok(), "{:?}", c.divergencias);
    }

    /// **Irmao do pedido 570: a copia reaberta para o `fsync` e a que o
    /// backup ESCREVEU, nao o que estiver no nome.**
    ///
    /// Alem do teto, a copia fecha e reabre pelo nome FORA da trava. Trocado
    /// o nome por um link para um arquivo de fora nesse intervalo, o `fsync`
    /// caia no de fora, respondia Ok, e o manifesto dizia «pronto» sobre uma
    /// copia que nunca sincronizou. Agora a reabertura nao segue link e
    /// confere o inode que a escrita anotou: a corrida recusa, sem manifesto.
    #[cfg(unix)]
    #[test]
    fn a_copia_trocada_antes_do_fsync_recusa_em_vez_de_sincronizar_outra() {
        let base = temp("570-irmao-reabre");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let (r, copias) = executar_com_teto(&raiz, &destino, 0).unwrap();
        let de_fora = base.join("de-fora.txt");
        std::fs::write(&de_fora, b"de fora").unwrap();
        let copia = destino.join("Z/cadastroClientes.reg");
        std::fs::remove_file(&copia).unwrap();
        std::os::unix::fs::symlink(&de_fora, &copia).unwrap();

        let e = concluir(&destino, 1, &r, &copias)
            .expect_err("o fsync caiu no arquivo de fora e o backup se disse pronto");
        assert!(e.to_string().contains(&*copia.to_string_lossy()), "{e}");
        assert!(
            !destino.join(MANIFESTO).exists(),
            "o manifesto nasceu sobre uma copia que nao sincronizou"
        );
    }

    /// O teto medido no Linux nunca e zero com a folga deste processo, nem
    /// passa do fixo: zero faria todo backup voltar a reabrir calado.
    #[test]
    fn o_teto_de_abertos_fica_entre_um_e_o_fixo() {
        let t = teto_de_abertos();
        assert!((1..=MAXIMO_DE_ABERTOS).contains(&t), "teto {t}");
    }

    /// As pastas do `fsync` do 579: sem repetir, da mais funda para a mais
    /// rasa, com o destino e a mae da pasta criada.
    #[test]
    fn as_pastas_tocadas_incluem_o_destino_e_a_mae_da_criada() {
        let base = temp("pastas-tocadas");
        let raiz = base.join("dados");
        let destino = base.join("nova/copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let (_, copias) = executar(&raiz, &destino, 0).unwrap();
        let p: Vec<PathBuf> = pastas_tocadas(&copias)
            .iter()
            .map(|p| p.real().to_path_buf())
            .collect();
        for esperada in [
            destino.join("Z/schemaX"),
            destino.join("Z"),
            destino.clone(),
            base.join("nova"),
            base.to_path_buf(),
        ] {
            assert!(p.contains(&esperada), "faltou {esperada:?} em {p:?}");
        }
        let mut sem_repetir = p.clone();
        sem_repetir.dedup();
        assert_eq!(sem_repetir.len(), p.len());
        assert!(p
            .windows(2)
            .all(|w| w[0].components().count() >= w[1].components().count()));
    }

    #[test]
    fn acha_arquivo_alterado() {
        let base = temp("alterado");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        backup_pronto(&raiz, &destino, 1_787_000_000_000);

        // Mesmo tamanho, conteudo diferente: so o SHA pega.
        std::fs::write(destino.join("Z/cadastroClientes.reg"), b"registros AQUI").unwrap();
        let c = conferir(&destino).unwrap();
        assert!(!c.ok());
        assert!(
            c.divergencias[0].contains("SHA-256"),
            "{:?}",
            c.divergencias
        );
    }

    #[test]
    fn acha_arquivo_sumido_e_arquivo_a_mais() {
        let base = temp("sumido");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        backup_pronto(&raiz, &destino, 1_787_000_000_000);

        std::fs::remove_file(destino.join("Z/cadastroClientes.ndx")).unwrap();
        std::fs::write(destino.join("Z/intruso.reg"), b"nao estava no manifesto").unwrap();

        let c = conferir(&destino).unwrap();
        assert_eq!(c.divergencias.len(), 2, "{:?}", c.divergencias);
        assert!(c.divergencias.iter().any(|d| d.contains("sumiu")));
        assert!(c
            .divergencias
            .iter()
            .any(|d| d.contains("nao esta no manifesto")));
    }

    #[test]
    fn acha_tamanho_diferente() {
        let base = temp("tamanho");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        backup_pronto(&raiz, &destino, 1_787_000_000_000);
        std::fs::write(destino.join("Z/cadastroClientes.reg"), b"curto").unwrap();
        let c = conferir(&destino).unwrap();
        assert!(c.divergencias[0].contains("bytes"), "{:?}", c.divergencias);
    }

    #[test]
    fn nao_copia_para_dentro_de_si_mesmo() {
        let base = temp("dentro");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        // Copiar a raiz para dentro dela copiaria a copia, sem parar.
        assert!(executar(&raiz, &raiz.join("copia"), 0).is_err());
        assert!(executar(&raiz, &raiz, 0).is_err());
    }

    #[test]
    fn destino_sem_manifesto_nao_e_backup() {
        let base = temp("semmanifesto");
        std::fs::create_dir_all(base.join("qualquer")).unwrap();
        assert!(conferir(&base.join("qualquer")).is_err());
    }

    #[test]
    fn o_manifesto_e_estavel() {
        // Dois backups dos mesmos dados listam na mesma ordem. Sem isso,
        // comparar dois manifestos seria comparar ruido.
        let base = temp("estavel");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let a = backup_pronto(&raiz, &base.join("c1"), 1_787_000_000_000);
        let b = backup_pronto(&raiz, &base.join("c2"), 1_787_000_001_000);
        assert_eq!(a.arquivos, b.arquivos);
    }
    #[test]
    fn o_nome_do_zip_traz_banco_admin_data_e_hora() {
        // 2026-08-27 20:43 UTC
        let ms = (phxsql_core::datahora::dias_de_civil(2026, 8, 27) as i64) * 86_400_000
            + 20 * 3_600_000
            + 43 * 60_000;
        assert_eq!(
            nome_do_zip("Comercial", "adriano", ms),
            "Comercial_adriano_2026-08-27_2043.zip"
        );
        // Nome com o que nao cabe em arquivo sai limpo, nao quebrado.
        assert_eq!(
            nome_do_zip("Com/ercial", "ana maria", ms),
            "Comercial_anamaria_2026-08-27_2043.zip"
        );
        // Vazio vira o padrao, nunca um nome comecando com sublinhado.
        assert_eq!(nome_do_zip("", "", ms), "dados_sistema_2026-08-27_2043.zip");
    }

    #[test]
    fn o_zip_leva_tudo_e_o_manifesto_dentro() {
        let base = temp("zip");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);

        let (alvo, r) = zip_pronto(
            &raiz,
            &base.join("copias"),
            "",
            "adriano",
            1_787_000_000_000,
        );
        assert!(alvo.is_file());
        assert!(alvo.to_string_lossy().ends_with(".zip"));
        assert_eq!(r.arquivos.len(), 4);
        assert!(r.comprimido > 0);

        let bytes = std::fs::read(&alvo).unwrap();
        assert_eq!(&bytes[..4], b"PK\x03\x04");
        let texto = String::from_utf8_lossy(&bytes);
        assert!(texto.contains("backup.json"), "o manifesto vai dentro");
        assert!(
            texto.contains("Z/schemaX/pedidos.reg"),
            "a hierarquia vai junto"
        );
    }

    #[test]
    fn da_para_copiar_um_banco_so() {
        let base = temp("zipbanco");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        std::fs::create_dir_all(raiz.join("Financeiro")).unwrap();
        std::fs::write(raiz.join("Financeiro/contas.reg"), b"nao deve entrar").unwrap();

        let (alvo, r) = zip_pronto(&raiz, &base.join("c"), "Z", "ana", 1_787_000_000_000);
        assert!(alvo
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("Z_ana_"));
        assert_eq!(r.arquivos.len(), 4, "so os quatro do Z");
        let bytes = std::fs::read(&alvo).unwrap();
        let texto = String::from_utf8_lossy(&bytes);
        assert!(!texto.contains("contas.reg"), "o outro banco ficou de fora");
    }

    #[test]
    fn nome_de_banco_hostil_nao_vira_caminho() {
        let base = temp("ziphostil");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        for mau in ["../..", "/etc", "a/b"] {
            assert!(
                executar_zip(&raiz, &base.join("c"), mau, "x", 0).is_err(),
                "{mau:?} passou"
            );
        }
    }

    #[test]
    fn a_retencao_guarda_os_mais_novos_e_nao_toca_no_alheio() {
        let nomes: Vec<String> = [
            "dados_noturno_2026-08-20_0300.zip",
            "dados_noturno_2026-08-24_0300.zip",
            "dados_noturno_2026-08-21_0300.zip",
            "dados_noturno_2026-08-27_2116.zip",
            "dados_noturno_2026-08-22_0300.zip",
            // Nao sao nossos: ficam, aconteca o que acontecer.
            "relatorio-do-contador.zip",
            "backup.json",
            "notas_fiscais.zip",
            "dados_noturno.zip",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let apagar = escolher_para_apagar(&nomes, 3);
        assert_eq!(
            apagar,
            vec![
                "dados_noturno_2026-08-20_0300.zip".to_string(),
                "dados_noturno_2026-08-21_0300.zip".to_string(),
            ],
            "apaga os dois mais velhos e so eles"
        );
        for alheio in [
            "relatorio-do-contador.zip",
            "notas_fiscais.zip",
            "backup.json",
        ] {
            assert!(!apagar.iter().any(|a| a == alheio), "{alheio} nao e nosso");
        }
    }

    #[test]
    fn manter_zero_nao_apaga_nada_e_poucos_tambem_nao() {
        let nomes: Vec<String> = (20..25)
            .map(|d| format!("dados_x_2026-08-{d}_0300.zip"))
            .collect();
        assert!(
            escolher_para_apagar(&nomes, 0).is_empty(),
            "zero = guarda tudo"
        );
        assert!(
            escolher_para_apagar(&nomes, 5).is_empty(),
            "cabe todo mundo"
        );
        assert!(escolher_para_apagar(&nomes, 99).is_empty());
        assert_eq!(escolher_para_apagar(&nomes, 1).len(), 4);
    }

    // ------------------------------------------------ o passo 2 do 513

    fn ficha(bytes: u64, mtime_ns: i128, lido_em_ns: i128) -> Ficha {
        Ficha {
            bytes,
            mtime_ns,
            identidade: Some((1, 42)),
            lido_em_ns,
        }
    }

    /// **A decisao da fase 2, pura.** O arquivo que nao mudou e cujo `mtime`
    /// e velho NAO se recopia (e o que faz a fase 2 ser barata); o que mudou
    /// no `stat`, sim.
    #[test]
    fn a_fase_2_so_recopia_o_que_o_stat_acusa() {
        let s = 1_000_000_000_000i128; // 1000 s depois da epoca
        let antes = ficha(10, s, s + 60 * JANELA_RACY_NS);
        assert!(
            !precisa_recopiar(&antes, &antes, 0, 0),
            "nada mudou e recopiou"
        );
        assert!(
            precisa_recopiar(&antes, &ficha(11, s, s), 0, 0),
            "o tamanho mudou"
        );
        assert!(
            precisa_recopiar(&antes, &ficha(10, s + 1, s), 0, 0),
            "o mtime mudou"
        );
        let mut trocado = antes.clone();
        trocado.identidade = Some((1, 43));
        assert!(
            precisa_recopiar(&antes, &trocado, 0, 0),
            "o inode mudou (arquivo trocado)"
        );
    }

    /// **O «racily clean»**: `stat` igual, mas o `mtime` da fase 1 estava a
    /// menos de 2 s do proprio `stat` -- uma escrita no mesmo tique do relogio
    /// nao mudaria nada, entao recopia. Guarda `backup-fase-2-sem-racy`: so'
    /// comparando o `stat`, este teste cai.
    #[test]
    fn o_arquivo_racy_e_recopiado_mesmo_com_o_stat_igual() {
        let s = 1_000_000_000_000i128;
        let racy = ficha(10, s, s + JANELA_RACY_NS / 2);
        assert!(precisa_recopiar(&racy, &racy, 0, 0), "racy e nao recopiou");
        let no_futuro = ficha(10, s + 5 * JANELA_RACY_NS, s);
        assert!(
            precisa_recopiar(&no_futuro, &no_futuro, 0, 0),
            "mtime no futuro e nao recopiou"
        );
        let na_borda = ficha(10, s, s + JANELA_RACY_NS);
        assert!(
            !precisa_recopiar(&na_borda, &na_borda, 0, 0),
            "exatamente 2 s nao e racy"
        );
    }

    /// **A rede dos eventos**: `stat` igual e velho, mas o diario da tabela
    /// andou (relogio que recuou) -- recopia. Guarda
    /// `backup-fase-2-sem-eventos`: tirando a rede, este teste cai.
    #[test]
    fn a_tabela_que_andou_e_recopiada_mesmo_com_o_stat_igual() {
        let s = 1_000_000_000_000i128;
        let velha = ficha(10, s, s + 60 * JANELA_RACY_NS);
        assert!(!precisa_recopiar(&velha, &velha, 0, 0));
        assert!(
            precisa_recopiar(&velha, &velha, 0, 3),
            "a tabela andou e nao recopiou"
        );
    }

    /// **As duas passadas acertam o que mudou entre elas**: arquivo
    /// alterado, arquivo novo e arquivo sumido entre a fase 1 e a fase 2 --
    /// o destino sai igual a origem, o `conferir` aprova, e o acerto conta
    /// os tres (dois deles `fora_do_bloqueio`). Sem a fase 2 (guarda
    /// `backup-sem-fase-2`), o destino e o de antes das mudancas.
    #[test]
    fn a_fase_2_acerta_o_alterado_o_novo_e_o_sumido() {
        let base = temp("duas-fases");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        // Os arquivos de exemplo acabaram de nascer: todos racy. Para a prova
        // medir as TRES redes e nao a janela, envelhece o mtime deles.
        envelhecer(&raiz);

        let mut fase = copiar_fase_1(&raiz, &destino).unwrap();
        assert_eq!(fase.inventario().len(), 4);

        // Entre as fases: altera um, cria um, apaga um.
        std::fs::write(
            raiz.join("Z/cadastroClientes.reg"),
            b"registros aqui, e mais",
        )
        .unwrap();
        std::fs::write(raiz.join("Z/novo.reg"), b"nasceu no meio").unwrap();
        std::fs::remove_file(raiz.join("Z/cadastroClientes_002.reg")).unwrap();

        let acerto = acertar_fase_2(&mut fase, &BTreeMap::new()).unwrap();
        assert_eq!(acerto.conferidos, 4, "{acerto:?}");
        assert_eq!(acerto.arquivos, 3, "{acerto:?}");
        assert_eq!(acerto.fora_do_bloqueio, 2, "novo + sumido: {acerto:?}");
        assert_eq!(acerto.bytes, 22 + 14, "{acerto:?}");
        let (r, copias) = terminar(fase);
        assert!(r.retrato_ms.is_some());
        assert_eq!(r.arquivos.len(), 4);
        concluir(&destino, 1_787_000_000_000, &r, &copias).unwrap();

        assert_eq!(
            std::fs::read(destino.join("Z/cadastroClientes.reg")).unwrap(),
            b"registros aqui, e mais"
        );
        assert!(destino.join("Z/novo.reg").is_file());
        assert!(!destino.join("Z/cadastroClientes_002.reg").exists());
        let c = conferir(&destino).unwrap();
        assert!(c.ok(), "{:?}", c.divergencias);
        let manifesto =
            Json::analisar(&std::fs::read_to_string(destino.join(MANIFESTO)).unwrap()).unwrap();
        assert!(manifesto.inteiro_ou("retrato_ms", 0) > 0);
    }

    /// **Sem nada mudado, a fase 2 nao recopia nada** -- e o que faz a
    /// escrita esperar so' o `stat` de tudo.
    #[test]
    fn sem_mudanca_a_fase_2_so_confere() {
        let base = temp("duas-fases-quietas");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        envelhecer(&raiz);
        let mut fase = copiar_fase_1(&raiz, &base.join("copia")).unwrap();
        let acerto = acertar_fase_2(&mut fase, &BTreeMap::new()).unwrap();
        assert_eq!(acerto.arquivos, 0, "{acerto:?}");
        assert_eq!(acerto.conferidos, 4, "{acerto:?}");
    }

    /// **A rede dos eventos no acerto de verdade**: `stat` igual, mas a
    /// tabela `Z/cadastroClientes` consta como tocada -- os arquivos DELA
    /// (`.reg`, `.ndx` e o volume `_002`) recopiam; o do schema nao.
    #[test]
    fn a_fase_2_recopia_os_arquivos_da_tabela_tocada() {
        let base = temp("duas-fases-eventos");
        let raiz = base.join("dados");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        envelhecer(&raiz);
        let mut fase = copiar_fase_1(&raiz, &base.join("copia")).unwrap();
        let mut eventos = BTreeMap::new();
        eventos.insert(
            crate::congelamento::chave(&raiz.join("Z"), "cadastroClientes"),
            2u64,
        );
        let acerto = acertar_fase_2(&mut fase, &eventos).unwrap();
        assert_eq!(acerto.arquivos, 3, "{acerto:?}");
        assert_eq!(acerto.fora_do_bloqueio, 0, "{acerto:?}");
    }

    /// **O zip em duas passadas**: a arvore temporaria nasce em `pasta`,
    /// vira o `.zip` depois da fase 2 e some. E sem espaco a fase 1 nem
    /// comeca -- devolve `None`, e quem chama cai na passada unica dizendo.
    #[test]
    fn o_zip_em_duas_passadas_comprime_a_arvore_e_a_apaga() {
        let base = temp("duas-fases-zip");
        let raiz = base.join("dados");
        let pasta = base.join("zips");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);

        assert!(
            copiar_fase_1_para_zip(&raiz, &pasta, "", "ana", 1_787_000_000_000, Some(0))
                .unwrap()
                .is_none(),
            "sem espaco a fase 1 tinha de recusar"
        );
        assert!(!pasta.exists(), "a recusa por espaco nao cria nada");

        let mut fase = copiar_fase_1_para_zip(&raiz, &pasta, "", "ana", 1_787_000_000_000, None)
            .unwrap()
            .expect("com espaco a fase 1 roda");
        let arvore: Vec<_> = std::fs::read_dir(&pasta)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            arvore.iter().any(|n| n.ends_with(".zip.retrato.part")),
            "a arvore temporaria nao esta na pasta: {arvore:?}"
        );
        std::fs::write(raiz.join("Z/cadastroClientes.reg"), b"mudou entre as fases").unwrap();
        let acerto = acertar_fase_2(&mut fase, &BTreeMap::new()).unwrap();
        assert!(acerto.arquivos >= 1, "{acerto:?}");
        let (zip, r) = concluir_zip(fase).unwrap();
        finalizar_zip(&zip).unwrap();
        assert!(zip.is_file());
        assert!(r.retrato_ms.is_some());
        let bytes = std::fs::read(&*zip).unwrap();
        let texto = String::from_utf8_lossy(&bytes);
        assert!(
            texto.contains("mudou entre as fases"),
            "o zip e' o da fase 2"
        );
        let sobras: Vec<_> = std::fs::read_dir(&pasta)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".retrato.part"))
            .collect();
        assert!(sobras.is_empty(), "a arvore temporaria ficou: {sobras:?}");
    }

    /// **Pedido 646: o `fsync` do grosso fica TODO para o `concluir`, depois
    /// da fase 2**, inclusive o das copias que a fase 2 nao reescreveu. A fase
    /// 1 sincronizava essas antes da trava e o `concluir` as pulava; medido a
    /// 1 GB, isso so' trouxe picos de ~0,4 s para as escritas durante a
    /// copia. Duas provas: (1) com toda recusa de `fsync` armada no destino,
    /// as duas fases passam -- se alguem voltar a sincronizar o grosso nelas,
    /// a recusa sobe e o teste cai; (2) recusa forjada num `.ndx` que a fase 2
    /// NAO tocou -- se alguem voltar a pular as copias "ja sincronizadas", o
    /// `concluir` devolve Ok e o teste cai.
    #[test]
    fn o_concluir_sincroniza_tambem_o_que_a_fase_2_nao_reescreveu() {
        use crate::sincronia::falha_de_teste::{armar, desarmar, Onde};
        let base = temp("duas-fases-sync-tudo");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        envelhecer(&raiz);
        armar(&destino, Onde::Fsync, u32::MAX);
        let fase = copiar_fase_1(&raiz, &destino);
        std::fs::write(raiz.join("Z/cadastroClientes.reg"), b"mudou").unwrap();
        let acerto =
            fase.and_then(|mut f| acertar_fase_2(&mut f, &BTreeMap::new()).map(|a| (f, a)));
        desarmar(&destino);
        let (fase, acerto) = acerto.expect("nenhuma das duas fases pode pagar `fsync` (646)");
        assert_eq!(acerto.arquivos, 1, "so' o .reg mudou: {acerto:?}");
        let (r, copias) = terminar(fase);

        let intocado = destino.join("Z/cadastroClientes.ndx");
        assert!(intocado.is_file());
        crate::sincronia::falha_de_teste::armar(
            &intocado,
            crate::sincronia::falha_de_teste::Onde::Fsync,
            1,
        );
        let feito = concluir(&destino, 1, &r, &copias);
        crate::sincronia::falha_de_teste::desarmar(&intocado);
        assert!(
            feito.is_err(),
            "o concluir tem de sincronizar a copia que a fase 2 nao reescreveu"
        );
    }

    /// **Pedido 646: queda na fase 2.** Com a fase 1 sem `fsync` nenhum e o
    /// processo morrendo no meio da fase 2 (aqui: a corrida simplesmente nao
    /// chega ao `concluir`), o destino fica SEM manifesto -- e e isso que o
    /// `op_backups` (que so' lista pasta com `backup.json`) e o `restaurar`
    /// leem como «nao e um backup». A garantia e a mesma que havia com o
    /// `fsync` antes da trava, que e o motivo de a reversao nao perder nada.
    #[test]
    fn queda_na_fase_2_deixa_destino_sem_manifesto_e_o_restaurar_recusa() {
        let base = temp("queda-fase-2");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        envelhecer(&raiz);
        // Um backup velho no mesmo destino: o 577 o invalida na fase 1.
        let velho = copiar_fase_1(&raiz, &destino).unwrap();
        let (r_velho, copias_velhas) = terminar(velho);
        concluir(&destino, 1, &r_velho, &copias_velhas).unwrap();
        assert!(destino.join(MANIFESTO).is_file(), "o velho esta pronto");
        drop(copias_velhas);

        let mut fase = copiar_fase_1(&raiz, &destino).unwrap();
        std::fs::write(raiz.join("Z/cadastroClientes.reg"), b"mudou").unwrap();
        acertar_fase_2(&mut fase, &BTreeMap::new()).unwrap();
        // A queda: nada de `concluir`, nem faxina (o processo morreu).
        let _ = terminar(fase);

        assert!(
            !destino.join(MANIFESTO).exists(),
            "sem concluir nao pode haver manifesto: o op_backups listaria"
        );
        assert!(
            crate::restaurar::conteudo(&destino).is_err(),
            "o restaurar tinha de recusar o destino sem manifesto"
        );
        assert!(
            crate::restaurar::Preparada::preparar(&destino, &base.join("base"), "").is_err(),
            "o restaurar tinha de recusar o destino sem manifesto"
        );
        assert!(conferir(&destino).is_err());
    }

    /// **Pedido 646: ninguem grava o manifesto antes do `fsync`.** Um
    /// `fsync` recusado no `concluir` depois da fase 2 nao pode deixar
    /// manifesto novo: aqui um `backup.json` sentinela esta no destino (o que
    /// a faxina nao e dona de apagar) e tem de continuar com o conteudo dele.
    /// Com `finalizar_manifesto` antes de `sincronizar_copias`, o sentinela
    /// vira o manifesto de verdade e o teste cai.
    #[test]
    fn manifesto_nao_e_gravado_antes_do_fsync_das_duas_fases() {
        let base = temp("manifesto-depois-do-fsync");
        let raiz = base.join("dados");
        let destino = base.join("copia");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        envelhecer(&raiz);
        let mut fase = copiar_fase_1(&raiz, &destino).unwrap();
        std::fs::write(raiz.join("Z/cadastroClientes.reg"), b"mudou").unwrap();
        acertar_fase_2(&mut fase, &BTreeMap::new()).unwrap();
        let (r, copias) = terminar(fase);
        std::fs::write(destino.join(MANIFESTO), "SENTINELA").unwrap();

        let recopiada = destino.join("Z/cadastroClientes.reg");
        crate::sincronia::falha_de_teste::armar(
            &recopiada,
            crate::sincronia::falha_de_teste::Onde::Fsync,
            1,
        );
        let feito = concluir(&destino, 1, &r, &copias);
        crate::sincronia::falha_de_teste::desarmar(&recopiada);
        assert!(feito.is_err(), "o fsync recusado tinha de subir");
        let texto = std::fs::read_to_string(destino.join(MANIFESTO)).unwrap_or_default();
        assert!(
            texto == "SENTINELA" || texto.is_empty(),
            "o manifesto foi gravado antes do fsync das copias: {texto}"
        );
    }

    /// O zip em duas passadas do teste adverso do 651: a fase 1 na pasta dos
    /// zips, a fase 2 sem mudanca, e o zip fechado. Devolve o erro de
    /// qualquer passo -- recusar e' uma resposta valida da prova.
    fn zip_em_duas_passadas(raiz: &Path, pasta: &Path, quando: i64) -> Result<PathBuf> {
        let mut fase = copiar_fase_1_para_zip(raiz, pasta, "", "ana", quando, None)?
            .expect("sem medida de espaco a fase 1 roda");
        acertar_fase_2(&mut fase, &BTreeMap::new())?;
        let (zip, _) = concluir_zip(fase)?;
        finalizar_zip(&zip)?;
        Ok(zip.to_path_buf())
    }

    /// O conteudo de uma pasta, nome e bytes, para provar que ficou intacta.
    fn retrato_da_pasta(p: &Path) -> Vec<(String, Vec<u8>)> {
        let mut v: Vec<_> = listar(p)
            .unwrap()
            .into_iter()
            .map(|a| (relativo(p, &a), std::fs::read(&a).unwrap()))
            .collect();
        v.sort();
        v
    }

    /// **Pedido 651 (a): a arvore temporaria do zip (`<nome>.retrato.part/`)
    /// plantada como LINK SIMBOLICO para uma pasta-isca.** O nome e' previsivel
    /// (banco, admin e minuto) e a pasta dos zips e' lugar onde terceiros
    /// escrevem. Seguir o link punha as copias do banco DENTRO da isca
    /// (reescrevendo o que tivesse o mesmo nome) e o zip levava o conteudo da
    /// isca -- vazamento para quem le a pasta dos zips. Aceite: a isca
    /// intacta e nada dela no zip, ou a recusa.
    #[cfg(unix)]
    #[test]
    fn retrato_part_plantado_como_link_nao_leva_a_copia_para_fora() {
        let base = temp("retrato-part-link");
        let raiz = base.join("dados");
        let pasta = base.join("zips");
        let isca = base.join("isca");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        std::fs::create_dir_all(isca.join("Z")).unwrap();
        std::fs::write(isca.join("segredo.txt"), b"SEGREDO-DA-ISCA").unwrap();
        std::fs::write(isca.join("Z/cadastroClientes.reg"), b"REG-DA-ISCA").unwrap();
        let antes = retrato_da_pasta(&isca);
        std::fs::create_dir_all(&pasta).unwrap();
        let quando = 1_787_000_000_000;
        let nome = arvore_parcial_de(&nome_do_zip("dados", "ana", quando));
        std::os::unix::fs::symlink(&isca, pasta.join(&nome)).unwrap();

        let feito = zip_em_duas_passadas(&raiz, &pasta, quando);
        assert_eq!(
            retrato_da_pasta(&isca),
            antes,
            "a isca foi mexida pelo link"
        );
        if let Ok(zip) = &feito {
            let bytes = std::fs::read(zip).unwrap();
            let texto = String::from_utf8_lossy(&bytes);
            assert!(!texto.contains("SEGREDO-DA-ISCA"), "a isca foi para o zip");
        }
        let e = feito.expect_err("o link no lugar da arvore temporaria tinha de ser recusado");
        assert!(e.to_string().contains(".retrato.part"), "{e}");
        assert!(
            std::fs::symlink_metadata(pasta.join(&nome))
                .unwrap()
                .file_type()
                .is_symlink(),
            "o link plantado e' de quem o plantou: a recusa nao o apaga"
        );
    }

    /// **Pedido 651 (a): a arvore temporaria plantada como pasta DE VERDADE,
    /// ja com um arquivo intruso e com um LINK FISICO para a isca no nome de
    /// uma copia.** O intruso nao pode entrar no zip (viraria tabela na
    /// restauracao), e o link fisico nao pode receber os bytes do banco.
    #[cfg(unix)]
    #[test]
    fn retrato_part_plantado_ja_cheio_nao_entra_no_zip() {
        let base = temp("retrato-part-cheio");
        let raiz = base.join("dados");
        let pasta = base.join("zips");
        std::fs::create_dir_all(&raiz).unwrap();
        dados_de_exemplo(&raiz);
        let quando = 1_787_000_000_000;
        let plantada = pasta.join(arvore_parcial_de(&nome_do_zip("dados", "ana", quando)));
        std::fs::create_dir_all(plantada.join("Z")).unwrap();
        std::fs::write(plantada.join("Z/intruso.reg"), b"TABELA-INTRUSA").unwrap();
        let isca = base.join("isca.txt");
        std::fs::write(&isca, b"ISCA-DO-LINK-FISICO").unwrap();
        std::fs::hard_link(&isca, plantada.join("Z/cadastroClientes.ndx")).unwrap();

        let feito = zip_em_duas_passadas(&raiz, &pasta, quando);
        assert_eq!(std::fs::read(&isca).unwrap(), b"ISCA-DO-LINK-FISICO");
        if let Ok(zip) = &feito {
            let bytes = std::fs::read(zip).unwrap();
            let texto = String::from_utf8_lossy(&bytes);
            assert!(
                !texto.contains("TABELA-INTRUSA"),
                "o intruso foi para o zip"
            );
        }
        let e = feito.expect_err("a arvore temporaria que ja existia tinha de ser recusada");
        assert!(e.to_string().contains(".retrato.part"), "{e}");
        assert_eq!(
            std::fs::read(plantada.join("Z/intruso.reg")).unwrap(),
            b"TABELA-INTRUSA",
            "a recusa nao apaga o que nao e' desta corrida"
        );
    }

    /// Poe o `mtime` de cada arquivo da raiz 10 min no passado: fora da
    /// janela racy, para as provas medirem o `stat` e os eventos.
    fn envelhecer(raiz: &Path) {
        let velho = std::time::SystemTime::now() - std::time::Duration::from_secs(600);
        for a in listar(raiz).unwrap() {
            let f = File::options().write(true).open(&a).unwrap();
            f.set_modified(velho).unwrap();
        }
    }
}

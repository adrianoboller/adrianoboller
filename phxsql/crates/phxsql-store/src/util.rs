//! Utilitarios comuns aos quatro arquivos.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use phxsql_core::error::{PhxError, Result};

/// Instante atual em segundos desde a epoca Unix.
pub fn agora() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Instante atual em milissegundos desde a epoca Unix.
pub fn agora_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn ler_exato<F: Read + Seek>(f: &mut F, offset: u64, buf: &mut [u8]) -> Result<()> {
    f.seek(SeekFrom::Start(offset))?;
    f.read_exact(buf)?;
    Ok(())
}

pub fn escrever_em<F: Write + Seek>(f: &mut F, offset: u64, buf: &[u8]) -> Result<()> {
    f.seek(SeekFrom::Start(offset))?;
    f.write_all(buf)?;
    Ok(())
}

/// Leitura de campos little-endian de uma fatia.
pub struct Campos<'a>(pub &'a [u8]);

impl Campos<'_> {
    pub fn u16(&self, off: usize) -> u16 {
        u16::from_le_bytes(self.0[off..off + 2].try_into().unwrap())
    }
    pub fn u32(&self, off: usize) -> u32 {
        u32::from_le_bytes(self.0[off..off + 4].try_into().unwrap())
    }
    pub fn u64(&self, off: usize) -> u64 {
        u64::from_le_bytes(self.0[off..off + 8].try_into().unwrap())
    }
}

pub fn por_u16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}
pub fn por_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}
pub fn por_u64(buf: &mut [u8], off: usize, v: u64) {
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}
pub fn por_i64(buf: &mut [u8], off: usize, v: i64) {
    buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

pub fn conferir_magic(arquivo: &str, esperado: &'static [u8; 8], achado: &[u8]) -> Result<()> {
    if achado != esperado {
        let mut e = [0u8; 8];
        e.copy_from_slice(&achado[..8]);
        return Err(PhxError::BadMagic {
            arquivo: arquivo.to_string(),
            esperado,
            encontrado: e,
        });
    }
    Ok(())
}

// ------------------------------------------ a permissao do banco (542)

/// O modo de todo arquivo que o banco cria: so o dono le e escreve.
pub const MODO_DO_ARQUIVO: u32 = 0o600;

/// O modo de todo diretorio que o banco cria: so o dono entra.
pub const MODO_DO_DIRETORIO: u32 = 0o700;

/// As opcoes de abertura de TODO arquivo que o banco cria -- o motor unico
/// da permissao dos arquivos do banco (pedido 542).
///
/// # Por que 0600, e por que em todo arquivo
///
/// Um arquivo do banco pode guardar dado pessoal em claro quando a cifra
/// esta desligada -- que e o padrao: a linha inteira no `.reg`, a chave do
/// indice no `.ndx` e no `.fts`, o antes e o depois no `.log`, o valor no
/// `.lgpd`. A permissao restrita e a unica protecao que existe nesse caso.
/// Ate o pedido 345 so o `.lgpd` apertava; o 345 levou o `.fts`, e o `.reg`,
/// o `.ndx`, o `.log`, o `.trash`, o `.memo`, o `.bin` e o `.reason`
/// continuavam nascendo `0644` em diretorio `0755` -- e a copia do backup
/// regravava tudo `0644`, ate o que nasceu `0600` (medido com `stat`,
/// pedido 542).
///
/// A decisao e do papel J (`docs/propostas/pesquisa-rodada-2026-09-24.md`
/// §2): «outros nao leem» e convergencia dos tres maduros (aceite
/// automatico), e o grupo tambem nao alcanca o dado, PG 4 + MariaDB 3 = 7
/// contra MySQL 2 + SQLite 1 = 3. E os tres impoem o modo em vez de herdar o
/// `umask` do processo -- so o SQLite herda.
///
/// # Por que na CRIACAO, e por um motor so
///
/// `mode` vale no `open` que cria: nao ha janela entre nascer aberto e
/// apertar, que e a janela que o `config.rs` ja fechou para os segredos. O
/// `umask` so TIRA bit, entao o 0600 pedido aqui sai 0600 com qualquer
/// `umask` que deixe o dono escrever -- o `022` da instalacao inclusive.
/// Todo caminho que cria arquivo do banco passa por aqui ou pelos irmaos
/// abaixo ([`recriar_do_banco`], [`escrever_do_banco`],
/// [`copiar_do_banco`], [`criar_diretorio_do_banco`]); um `set_permissions`
/// depois de cada `File::create` espalharia a decisao, e o lugar que alguem
/// esquecesse nasceria aberto sem ninguem ver.
///
/// # O que ele NAO faz
///
/// Nao aperta o que ja existe: um arquivo de uma base antiga, aberto para
/// escrever, fica com a permissao que tinha. Apertar sozinho tiraria o
/// acesso de quem hoje le por grupo, e isso e guarda imposta -- a base antiga
/// ganha um ALERTA ([`permissao_larga`]), nao um `chmod` calado. E fora do
/// Unix nao ha modo: no Windows vale a ACL herdada da pasta, como sempre
/// valeu.
pub fn opcoes_do_banco() -> OpenOptions {
    #[allow(unused_mut)]
    let mut opcoes = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opcoes.mode(MODO_DO_ARQUIVO);
    }
    opcoes
}

/// Cria ou TRUNCA `caminho` para escrita -- e ele sai 0600 mesmo que ja
/// existisse.
///
/// O `mode` de [`opcoes_do_banco`] so vale para o arquivo que NASCE; um que
/// ja estava la (o `.ndx` que o `reindexar` refaz, o temporario que uma
/// gravacao interrompida deixou, a copia de backup por cima da de ontem)
/// entraria com a permissao antiga. Aqui o CONTEUDO e novo, entao a
/// permissao tambem: apertar isto nao e apertar calado uma base antiga, e o
/// arquivo nascendo de novo.
///
/// # E NAO atravessa link simbolico no ultimo nome -- revisao SEC do 542
///
/// O `open` com `create + truncate` segue o link, e o `fchmod` de depois
/// tambem: um link plantado no destino do backup (`dest/loja/c.reg ->
/// vitima`) fazia o backup gravar o `.reg` NA vitima, fora do destino, e
/// muda-la para 0600 -- medido pela SEC contra o `phxsqld` vivo, `0o644 ->
/// 0o600` e o conteudo virando `PHXREG`. A escrita de fora ja existia com o
/// `File::create` de antes; o `chmod` de fora nasceu com este motor, e por
/// isso o conserto e daqui. O destino de backup e, por desenho, lugar onde
/// outros escrevem (disco de rede, USB).
///
/// Sem `O_NOFOLLOW`: a `std` nao o expoe, e o numero muda de arquitetura
/// (`0o400000` no x86, `0o100000` no ARM, `0x100` nos BSD) -- digitado a mao,
/// o errado nao recusaria nada e ninguem veria. O mesmo efeito sai de tres
/// passos que a `std` tem:
///
/// 1. `create_new` (`O_EXCL`), que nunca segue link no ultimo nome -- nem o
///    pendurado, que com `O_CREAT` CRIARIA o alvo fora do banco. E o caso
///    comum, e custa o mesmo `open` de antes;
/// 2. se ja existe: `lstat` do nome, e so arquivo REGULAR segue -- link,
///    FIFO (cujo `open` de escrita ficaria parado esperando leitor) e
///    dispositivo recusam sem abrir;
/// 3. abre SEM criar e SEM truncar, e so trunca (`set_len(0)`, pelo
///    descritor) se o `fstat` do que abriu e o MESMO inode do `lstat`: quem
///    trocar o nome por um link entre o passo 2 e o 3 e pego aqui, antes de
///    o alvo perder um byte ou o modo.
///
/// Truncar o mesmo inode, e nao apagar e criar de novo, e de proposito: o
/// `.ndx` que o `reindexar` refaz pode ter outro punho aberto neste processo,
/// e ele tem de ver o arquivo novo, como via com o `O_TRUNC`.
///
/// # O que o 570 acrescentou: a abertura nao segue link nem espera leitor
///
/// Entre o `lstat` do passo 2 e o `open` do passo 3 ha uma janela: trocado o
/// nome por um link para uma FIFO nela, o `open` de escrita (que seguia o
/// link) ficava parado esperando leitor -- com a trava de dados na mao, o
/// servidor inteiro parado (medido pelo juiz: 1 em 12 corridas, um `inserir`
/// 5,0 s sem resposta). O passo 3 abre agora com `O_NOFOLLOW | O_NONBLOCK`
/// ([`sem_seguir_nem_esperar`]): o link recusa (`ELOOP`), a FIFO sem leitor
/// recusa (`ENXIO`), e a FIFO com leitor abre sem parar e cai no `fstat`, que
/// exige arquivo REGULAR e o mesmo inode antes de truncar. O numero do
/// `O_NOFOLLOW` digitado a mao continua perigoso -- o errado nao recusaria
/// nada --, e por isso so existe nas arquiteturas conferidas ([`bandeiras`]),
/// com o teste que prova a recusa contra o nucleo onde a suite roda.
///
/// O link num nome INTERMEDIARIO (`dest/loja -> fora`, pedido 568) nao e
/// deste motor: quem precisa (o backup) chega ao nome pela [`Pasta`], que
/// abre diretorio a diretorio sem seguir link, e passa aqui o nome ja
/// alcancado pelo descritor.
pub fn recriar_do_banco(caminho: &Path, ler: bool) -> std::io::Result<File> {
    recriar(caminho, ler, Modo::Banco)
}

/// O arquivo do banco que OUTROS processos seguram abertos e que, por isso,
/// se REABRE em vez de recriar: a trava de instancia (pedido 648). Cria com
/// 0600 se nao existe; se existe, abre o MESMO inode, para ler e escrever,
/// SEM truncar e SEM mexer na permissao -- truncar apagaria o pid de quem
/// segura a trava antes de o `flock` dizer que ela esta ocupada.
///
/// E' o terceiro modo do mesmo motor de [`recriar_do_banco`] (`create_new`,
/// `lstat` do nome, `O_NOFOLLOW` e `fstat` do que abriu), e nao uma
/// abertura propria: a trava abria com `create(true).write(true)`, que segue
/// link e escrevia (`set_len(0)` + pid) no alvo de um `.phxsql.trava ->
/// isca`. HIPOTESE MORTA: `recriar_no_destino` (trocar o nome alheio por um
/// arquivo novo) -- trocar o inode da trava desfaz a exclusao mutua, porque
/// quem ja a segura fica com o inode velho e o recem-chegado trava o novo.
/// Por isso um arquivo de DOIS nomes (`nlink > 1`, o link fisico plantado
/// para a trava escrever o pid no inode da vitima) RECUSA, e nao se troca.
pub fn reabrir_do_banco(caminho: &Path, ler: bool) -> std::io::Result<File> {
    recriar(caminho, ler, Modo::Reabrir)
}

/// O que o motor de [`recriar_do_banco`] faz com o arquivo que ja esta no nome.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Modo {
    /// Trunca o mesmo inode.
    Banco,
    /// Troca o nome alheio por um arquivo novo (569).
    Destino,
    /// Abre o mesmo inode sem truncar; link fisico recusa (648).
    Reabrir,
}

/// [`recriar_do_banco`] para um nome num DESTINO onde outros escrevem -- a
/// pasta do backup (pedido 569).
///
/// A diferenca e uma so: o arquivo regular que ja esta no nome mas NAO e
/// nosso -- de outro dono, ou com outro nome no mesmo inode (`nlink > 1`, o
/// link fisico) -- nao e truncado e reescrito: o NOME sai e o nosso nasce com
/// `create_new`. Reescrever o inode alheio entregava o conteudo do banco a
/// quem o plantou (medido pela SEC: a copia do `c.reg` com uid 1234, e no
/// ZIP o `.part` plantado virava o `.zip` final). Trocar o nome, e nao
/// recusar, e o conserto que a SEC pediu: o reuso do destino que ja e nosso
/// continua igual, truncando o mesmo inode.
///
/// Por que nao vale para o banco inteiro: la, o arquivo de outro dono e o da
/// base do servico mexida por um `sudo` do administrador -- trocar o nome
/// daria o arquivo ao root e tiraria o acesso do servico. A pasta do backup e
/// lugar onde terceiros escrevem por desenho; a raiz de dados, nao.
pub fn recriar_no_destino(caminho: &Path, ler: bool) -> std::io::Result<File> {
    recriar(caminho, ler, Modo::Destino)
}

/// O corpo de [`recriar_do_banco`] e [`recriar_no_destino`]: um motor so, e
/// `destino` decide so o que fazer com o arquivo regular que nao e nosso.
fn recriar(caminho: &Path, ler: bool, modo: Modo) -> std::io::Result<File> {
    let destino = modo == Modo::Destino;
    // Duas voltas no maximo: a segunda so depois de o nome alheio sair
    // (569). Se alguem o plantar de novo no intervalo, o `create_new` recusa
    // -- o nosso conteudo nunca cai no inode dele.
    let mut tirou_o_alheio = false;
    loop {
        match opcoes_do_banco()
            .read(ler)
            .write(true)
            .create_new(true)
            .open(caminho)
        {
            Ok(novo) => {
                apertar_permissao(&novo);
                return Ok(novo);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && !tirou_o_alheio => {}
            Err(e) => return Err(e),
        }
        let nome = std::fs::symlink_metadata(caminho)?;
        if !nome.file_type().is_file() {
            return Err(recusa_do_nome(caminho, &nome));
        }
        if modo == Modo::Reabrir && !um_nome_so(&nome) {
            return Err(recusa_de_varios_nomes(caminho));
        }
        if destino && !e_nosso(&nome) {
            std::fs::remove_file(caminho)?;
            tirou_o_alheio = true;
            continue;
        }
        let mut abrir = OpenOptions::new();
        abrir.read(ler).write(true);
        let arquivo = sem_seguir_nem_esperar(&mut abrir)
            .open(caminho)
            .map_err(|e| {
                if trocado_na_janela(&e) {
                    recusa_do_nome(caminho, &nome)
                } else {
                    e
                }
            })?;
        // Quem decide e o `fstat` do que ABRIU, nunca o nome: o mesmo inode
        // do `lstat`, regular, e (no destino) ainda nosso.
        let aberto = arquivo.metadata()?;
        if !aberto.is_file() || !mesmo_arquivo(&aberto, &nome) || (destino && !e_nosso(&aberto)) {
            return Err(recusa_do_nome(caminho, &nome));
        }
        if modo == Modo::Reabrir {
            // Os dois nomes tambem se conferem no que ABRIU: o link fisico
            // plantado entre o `lstat` e o `open` so aparece aqui.
            if !um_nome_so(&aberto) {
                return Err(recusa_de_varios_nomes(caminho));
            }
            return Ok(arquivo);
        }
        arquivo.set_len(0)?;
        apertar_permissao(&arquivo);
        return Ok(arquivo);
    }
}

/// A recusa do `open` que so acontece porque o nome mudou entre o `lstat` e
/// ele: `ELOOP` (virou link, e o `O_NOFOLLOW` recusou) ou `ENXIO` (virou
/// FIFO sem leitor, e o `O_NONBLOCK` recusou em vez de esperar). Numeros do
/// Linux, o unico SO onde as bandeiras existem aqui.
fn trocado_na_janela(e: &std::io::Error) -> bool {
    cfg!(target_os = "linux") && matches!(e.raw_os_error(), Some(6) | Some(40))
}

/// O arquivo e NOSSO para reescrever no lugar: um nome so (`nlink == 1`) e,
/// onde o dono do processo se le, do mesmo dono. Fora do Unix a `std` nao da
/// dono nem contagem de nomes, e vale o que valia.
fn e_nosso(m: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        m.nlink() == 1 && uid_do_processo().is_none_or(|u| u == m.uid())
    }
    #[cfg(not(unix))]
    {
        let _ = m;
        true
    }
}

/// O arquivo tem um nome so. Fora do Unix a `std` nao conta nomes.
fn um_nome_so(m: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        m.nlink() == 1
    }
    #[cfg(not(unix))]
    {
        let _ = m;
        true
    }
}

/// A recusa de [`reabrir_do_banco`] diante de um link fisico.
fn recusa_de_varios_nomes(caminho: &Path) -> std::io::Error {
    std::io::Error::other(format!(
        "{}: o arquivo tem mais de um nome (link fisico), e o banco nao grava \
         nele -- escreveria no inode de outro arquivo. Tire o outro nome (ou \
         o arquivo) e repita",
        caminho.display()
    ))
}

/// O uid com que este processo CRIA arquivo (o *fsuid*, quarto numero da
/// linha `Uid:` do `/proc/self/status`), lido uma vez. A `std` nao tem
/// `geteuid`, e chama-lo pede FFI e `unsafe`; o `/proc` diz o mesmo sem os
/// dois. `None` fora do Linux ou sem `/proc`: o crivo do dono cala e fica o
/// do `nlink`, dito em [`e_nosso`].
fn uid_do_processo() -> Option<u32> {
    static UID: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    *UID.get_or_init(|| {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let linha = status.lines().find(|l| l.starts_with("Uid:"))?;
        linha.split_whitespace().nth(4)?.parse().ok()
    })
}

// ------------------------- abrir sem seguir link (pedidos 568 e 570)

/// As bandeiras do `open` que a `std` nao expoe: `O_NOFOLLOW`, `O_DIRECTORY`
/// e `O_NONBLOCK`, nesta ordem.
///
/// O numero muda de arquitetura, e o errado nao recusaria nada, calado. Por
/// isso so as conferidas no `fcntl.h` do nucleo tem valor: as do
/// `asm-generic` (x86, x86_64, riscv64, loongarch64, s390x) e as que o
/// sobrescrevem do mesmo jeito (arm, aarch64, powerpc). O teste
/// `sem_seguir_recusa_link_e_fifo_de_verdade` prova a recusa contra o nucleo
/// onde a suite roda. Fora delas (mips e sparc mudam ate o `O_NONBLOCK`, e
/// todo SO que nao e Linux): `None`, e os chamadores ficam com o `lstat` de
/// antes, que tem a janela dita.
pub(crate) const fn bandeiras() -> Option<(i32, i32, i32)> {
    #[cfg(all(
        target_os = "linux",
        any(
            target_arch = "x86",
            target_arch = "x86_64",
            target_arch = "riscv64",
            target_arch = "loongarch64",
            target_arch = "s390x"
        )
    ))]
    {
        Some((0o400000, 0o200000, 0o4000))
    }
    #[cfg(all(
        target_os = "linux",
        any(
            target_arch = "arm",
            target_arch = "aarch64",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )
    ))]
    {
        Some((0o100000, 0o40000, 0o4000))
    }
    #[cfg(not(all(
        target_os = "linux",
        any(
            target_arch = "x86",
            target_arch = "x86_64",
            target_arch = "riscv64",
            target_arch = "loongarch64",
            target_arch = "s390x",
            target_arch = "arm",
            target_arch = "aarch64",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )
    )))]
    {
        None
    }
}

/// Acrescenta `O_NOFOLLOW | O_NONBLOCK` a quem vai ABRIR um nome que ja
/// existe: link no ultimo nome recusa, FIFO sem leitor recusa na hora em vez
/// de parar a thread (pedido 570). Em arquivo regular o `O_NONBLOCK` nao
/// muda nada no Linux. Sem [`bandeiras`], devolve as opcoes como vieram.
pub fn sem_seguir_nem_esperar(opcoes: &mut OpenOptions) -> &mut OpenOptions {
    #[cfg(unix)]
    if let Some((nofollow, _, nonblock)) = bandeiras() {
        use std::os::unix::fs::OpenOptionsExt;
        opcoes.custom_flags(nofollow | nonblock);
    }
    opcoes
}

/// Um diretorio ABERTO, por onde se chega aos nomes de dentro sem atravessar
/// link simbolico em componente nenhum -- pedido 568.
///
/// O defeito: com `dest/loja -> base/rh` plantado no destino do backup, o
/// `create_dir_all` aceitava o link como pasta que ja existe, e o `c.reg` da
/// `loja` era gravado POR CIMA do `c.reg` vivo do `rh` (medido pela SEC: o
/// `varrer` de `rh` caiu de 2 para 1 registro). Conferir cada componente com
/// `lstat` e depois abrir pelo nome deixa a janela entre os dois; o que os
/// primos fazem (o `my_open_parent_dir_nosymlinks` do MySQL e do MariaDB) e
/// abrir componente a componente, relativo ao descritor do diretorio de cima
/// (`openat`), com `O_NOFOLLOW`.
///
/// A `std` nao tem `openat`, e chama-lo pede FFI e `unsafe`. O mesmo efeito
/// sai do `/proc/self/fd/N/nome`: o nucleo resolve o `N` pelo DESCRITOR (nao
/// pelo nome que o diretorio tinha quando abriu), e so o `nome` se resolve por
/// nome -- que e exatamente o `openat(N, nome)`. Quem abre com `O_NOFOLLOW`
/// recusa o link nesse `nome`. Sem `/proc` ou sem [`bandeiras`], a `Pasta`
/// cai no caminho real com o `lstat` de cada componente: fecha o link ja
/// plantado, e a janela da troca fica, dita.
#[derive(Debug)]
pub struct Pasta {
    /// O descritor do diretorio; `None` quando nao se chega por ele.
    dir: Option<File>,
    /// O caminho real: para a mensagem, e para quem nao tem `/proc`.
    real: std::path::PathBuf,
}

impl Pasta {
    /// Abre `caminho` SEGUINDO link: o destino e escolha de quem chama
    /// (`/backups -> /mnt/usb` e instalacao comum). Daqui para dentro, nada
    /// se segue.
    pub fn abrir(caminho: &Path) -> std::io::Result<Pasta> {
        let dir = if bandeiras().is_some() && Path::new("/proc/self/fd").is_dir() {
            Some(abrir_diretorio(caminho, false)?)
        } else {
            None
        };
        Ok(Pasta {
            dir,
            real: caminho.to_path_buf(),
        })
    }

    /// O nome `nome` DENTRO desta pasta, alcancado pelo descritor: e o que se
    /// passa ao `open`, ao `remove_file`, ao `lstat`. So o ultimo componente
    /// se resolve por nome, e quem abre decide se o segue.
    pub fn por_dentro(&self, nome: &std::ffi::OsStr) -> std::path::PathBuf {
        match &self.dir {
            #[cfg(unix)]
            Some(d) => {
                use std::os::fd::AsRawFd;
                Path::new("/proc/self/fd")
                    .join(d.as_raw_fd().to_string())
                    .join(nome)
            }
            _ => self.real.join(nome),
        }
    }

    /// O caminho real desta pasta: o da mensagem, e o de quem nao tem `/proc`.
    pub fn real(&self) -> &Path {
        &self.real
    }

    /// O descritor aberto, quando a pasta chega por ele -- e nele que o
    /// `fsync` da pasta tem de cair (pedido 593), e nao num `open` pelo nome
    /// feito depois, que segue o link que alguem pos no lugar.
    pub fn descritor(&self) -> Option<&File> {
        self.dir.as_ref()
    }

    /// O dev/inode desta pasta e de cada pasta ACIMA dela, ate a raiz do
    /// sistema -- pedido 611 (S7). Sobe pelo DESCRITOR (`/proc/self/fd/N/..`,
    /// `../..`, ...): o nucleo resolve o `N` pela pasta aberta, e nao pelo nome
    /// que ela tinha, entao a resposta e sobre onde a corrida vai escrever, e
    /// nao sobre o que o nome aponta agora. Sem descritor, sobe pelo caminho
    /// real, e a janela da troca fica -- a mesma da [`Pasta`].
    pub fn ancestrais(&self) -> std::io::Result<Vec<(u64, u64)>> {
        match &self.dir {
            #[cfg(unix)]
            Some(d) => {
                use std::os::fd::AsRawFd;
                ancestrais(&Path::new("/proc/self/fd").join(d.as_raw_fd().to_string()))
            }
            _ => ancestrais(&self.real),
        }
    }

    /// O `fstat` do descritor; sem ele, o `lstat` do caminho real.
    pub fn metadados(&self) -> std::io::Result<std::fs::Metadata> {
        match &self.dir {
            Some(d) => d.metadata(),
            None => std::fs::symlink_metadata(&self.real),
        }
    }

    /// Entra na subpasta `nome` de `mae`, criando-a 0700 se falta, e anota em
    /// `criadas` a que nasceu aqui -- para a faxina do 576, que a remove pelo
    /// descritor de `mae` (pedido 593). Link, arquivo, ou qualquer coisa que
    /// nao e diretorio no nome RECUSA: o backup nao grava atraves dele.
    ///
    /// Associada, e nao metodo: a nascida guarda a `mae` (`Arc`), e e por ela
    /// que a remocao chega ao nome sem atravessar link no caminho.
    pub fn entrar(
        mae: &std::sync::Arc<Pasta>,
        nome: &std::ffi::OsStr,
        criadas: &mut Vec<Nascida>,
    ) -> std::io::Result<Pasta> {
        let alvo = mae.por_dentro(nome);
        let real = mae.real.join(nome);
        let nasce = std::fs::symlink_metadata(&alvo).is_err();
        if nasce {
            criar_diretorio_do_banco(&alvo)?;
        }
        let recusa = || {
            std::io::Error::other(format!(
                "{}: o nome e um link simbolico ou nao e pasta, e o backup nao \
                 grava atraves dele -- gravaria por cima de arquivos fora do \
                 destino, de outro database inclusive. Tire o que esta nesse \
                 nome e repita",
                real.display()
            ))
        };
        let aberta = match &mae.dir {
            Some(_) => abrir_diretorio(&alvo, true)
                .map(|d| Pasta {
                    dir: Some(d),
                    real: real.clone(),
                })
                .map_err(|e| match std::fs::symlink_metadata(&alvo) {
                    Ok(m) if !m.is_dir() => recusa(),
                    _ => e,
                }),
            None => match std::fs::symlink_metadata(&alvo) {
                Ok(m) if m.is_dir() => Ok(Pasta {
                    dir: None,
                    real: real.clone(),
                }),
                Ok(_) => Err(recusa()),
                Err(e) => Err(e),
            },
        };
        if nasce {
            // A identidade de quem nasceu e a do descritor que a corrida vai
            // usar; se ele nao abriu, a do nome logo depois do `mkdir` -- a
            // pasta anotada mesmo assim, para a faxina ainda alcanca-la.
            let id = match &aberta {
                Ok(p) => p.metadados().ok(),
                Err(_) => std::fs::symlink_metadata(&alvo).ok(),
            };
            criadas.push(Nascida {
                mae: mae.clone(),
                nome: nome.to_os_string(),
                id: id.as_ref().and_then(identidade),
            });
        }
        aberta
    }
}

/// Uma pasta que a corrida do backup fez nascer -- pedido 593.
///
/// Guarda a MAE aberta e o dev/inode da nascida, e nao so o caminho: a
/// faxina de uma corrida que falhou roda fora da trava, e pelo nome real um
/// link posto numa pasta do meio fazia o `remove_dir` cair na pasta de outro
/// (vazia, que e o que o `rmdir` aceita). Pelo descritor da mae, so o ultimo
/// nome se resolve por nome -- e ele se confere pelo inode antes.
#[derive(Debug)]
pub struct Nascida {
    mae: std::sync::Arc<Pasta>,
    nome: std::ffi::OsString,
    id: Option<(u64, u64)>,
}

impl Nascida {
    /// Anota `filha`, ja aberta, como nascida em `mae` com o nome `nome` --
    /// para quem criou a pasta por outro caminho (o `create_dir_all` do
    /// destino) e so depois a abriu.
    pub fn anotar(mae: std::sync::Arc<Pasta>, nome: &std::ffi::OsStr, filha: &Pasta) -> Nascida {
        Nascida {
            mae,
            nome: nome.to_os_string(),
            id: filha.metadados().ok().as_ref().and_then(identidade),
        }
    }

    /// A pasta mae, aberta -- a que recebe o `fsync` da entrada nova.
    pub fn mae(&self) -> &std::sync::Arc<Pasta> {
        &self.mae
    }

    /// O caminho real da nascida (o da mae mais o nome): so' para decidir a
    /// quem ela pertence, nunca para remove-la -- isso e' pelo descritor.
    pub fn real(&self) -> std::path::PathBuf {
        self.mae.real().join(&self.nome)
    }

    /// Remove a pasta se ela ainda e a que nasceu: pelo descritor da mae,
    /// conferindo dev/inode com `lstat` antes, e com `remove_dir` -- que so
    /// remove VAZIA e, no ultimo nome, nao segue link (`rmdir` de link da
    /// `ENOTDIR`). A `std` nao tem `unlinkat`; o `/proc/self/fd/N/nome` da
    /// mae e o mesmo efeito, sem `unsafe`.
    ///
    /// **A janela que sobra, dita:** entre o `lstat` e o `rmdir`, trocar o
    /// nome por OUTRA pasta vazia (um `rename` dentro da mesma mae) faz
    /// remover essa outra. So vazia, e so dentro da mae que a corrida abriu:
    /// nao atravessa link para fora. Fechar pede o `unlinkat` sobre o
    /// descritor da propria nascida, que a `std` nao da. Fora do Linux (sem
    /// `/proc`), a mae e o caminho real e a conferencia e o `lstat` dele --
    /// o comportamento de antes, com o inode conferido onde ha Unix.
    pub fn remover(&self) -> bool {
        let alvo = self.mae.por_dentro(&self.nome);
        match std::fs::symlink_metadata(&alvo) {
            Ok(m) if m.is_dir() && identidade(&m) == self.id => std::fs::remove_dir(&alvo).is_ok(),
            _ => false,
        }
    }
}

/// Teto de niveis de [`ancestrais`]: o `PATH_MAX` do Linux (4096) nao cabe
/// mais que ~1365 `../`, e uma arvore que nao chega a raiz dentro disso e
/// laco ou defeito -- recusa em vez de girar.
const TETO_DE_NIVEIS: usize = 1024;

/// O dev/inode de `inicio` e de cada pasta acima, pelo `..` do NUCLEO (que
/// nao e o `parent()` lexico: atravessa link e ponto de montagem como o disco
/// os ve), ate a raiz do sistema, onde `..` e ela mesma. Fora do Unix a `std`
/// nao da o inode, e a lista volta vazia -- quem confere fica com a
/// conferencia pelo nome, dita.
pub fn ancestrais(inicio: &Path) -> std::io::Result<Vec<(u64, u64)>> {
    let mut vistos: Vec<(u64, u64)> = Vec::new();
    let mut atual = inicio.to_path_buf();
    for _ in 0..TETO_DE_NIVEIS {
        let Some(id) = identidade(&std::fs::metadata(&atual)?) else {
            return Ok(Vec::new());
        };
        if vistos.last() == Some(&id) {
            return Ok(vistos);
        }
        vistos.push(id);
        atual.push("..");
    }
    Err(std::io::Error::other(format!(
        "{}: mais de {TETO_DE_NIVEIS} pastas acima sem chegar a raiz do sistema",
        inicio.display()
    )))
}

/// O que identifica um arquivo ou pasta para quem o reencontra depois:
/// dispositivo e inode. Fora do Unix a `std` nao os da, e vale `None` dos
/// dois lados -- a conferencia vira a do tipo, que ja passou.
pub fn identidade(m: &std::fs::Metadata) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some((m.dev(), m.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = m;
        None
    }
}

/// O `open` de um diretorio com `O_DIRECTORY` -- e `O_NOFOLLOW` quando
/// `sem_seguir`, que e todo componente menos a raiz da [`Pasta`]. So e
/// chamado onde [`bandeiras`] existe.
fn abrir_diretorio(caminho: &Path, sem_seguir: bool) -> std::io::Result<File> {
    let mut opcoes = OpenOptions::new();
    opcoes.read(true);
    #[cfg(unix)]
    if let Some((nofollow, diretorio, _)) = bandeiras() {
        use std::os::unix::fs::OpenOptionsExt;
        opcoes.custom_flags(diretorio | if sem_seguir { nofollow } else { 0 });
    }
    #[cfg(not(unix))]
    let _ = sem_seguir;
    opcoes.open(caminho)
}

/// O que o `fstat` do descritor aberto e o `lstat` do nome dizem do MESMO
/// arquivo. Fora do Unix nao ha inode para comparar, e nao ha link plantado
/// pelo mesmo caminho: vale o `is_file` do `lstat`, que ja passou.
fn mesmo_arquivo(aberto: &std::fs::Metadata, nome: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        aberto.dev() == nome.dev() && aberto.ino() == nome.ino()
    }
    #[cfg(not(unix))]
    {
        let _ = (aberto, nome);
        true
    }
}

/// A recusa de [`recriar_do_banco`], dizendo O QUE estava no nome. O erro
/// cru do `open` nao serviria: seguir o link daria certo, e o problema e
/// justamente esse.
fn recusa_do_nome(caminho: &Path, nome: &std::fs::Metadata) -> std::io::Error {
    let o_que = if nome.file_type().is_symlink() {
        "um link simbolico"
    } else if nome.is_file() {
        "um arquivo que foi trocado enquanto se abria"
    } else {
        "algo que nao e arquivo regular"
    };
    std::io::Error::other(format!(
        "{}: o nome e {o_que}, e o banco nao grava atraves dele -- gravaria (e \
         apertaria para 0600) um arquivo fora do banco. Tire o que esta nesse \
         nome e repita",
        caminho.display()
    ))
}

/// O `std::fs::write` do banco: [`recriar_do_banco`] e o corpo inteiro.
pub fn escrever_do_banco(caminho: &Path, corpo: impl AsRef<[u8]>) -> std::io::Result<()> {
    recriar_do_banco(caminho, false)?.write_all(corpo.as_ref())
}

/// O `std::fs::copy` do banco -- SEM herdar a permissao da origem.
///
/// O `fs::copy` da `std` copia o modo junto: a origem `0644` de uma base
/// antiga faria a copia nascer `0644`, e a copia de backup e o caso que o
/// pedido 542 mediu. Aqui o destino nasce pelo motor, e o conteudo vai por
/// `io::copy` (no Linux, `copy_file_range`).
///
/// Devolve o descritor que ESCREVEU, ainda aberto (pedido 582): quem precisa
/// do `fsync` da copia o faz NESTE, e nao num reaberto pelo caminho -- a
/// licao do pedido 552 (o caso *fsyncgate*): fechado, o nucleo pode despejar
/// o inode junto com o erro de *writeback*, e o descritor novo responde Ok
/// sem o dado. Quem nao precisa so o solta.
pub fn copiar_do_banco(de: &Path, para: &Path) -> std::io::Result<File> {
    let mut origem = File::open(de)?;
    let mut destino = recriar_do_banco(para, false)?;
    std::io::copy(&mut origem, &mut destino)?;
    Ok(destino)
}

/// O `create_dir_all` do banco: cada nivel que nasce AQUI nasce 0700; o que
/// ja existia fica como estava (ver [`opcoes_do_banco`], «o que ele NAO
/// faz»).
///
/// O diretorio e a primeira porta: o MariaDB grava arquivo `0660` e fecha o
/// grupo pelo diretorio `0700`. Aqui os dois fecham, e o diretorio e o que
/// protege o arquivo que alguem criar ali por fora deste motor.
pub fn criar_diretorio_do_banco(caminho: &Path) -> std::io::Result<()> {
    let mut construtor = std::fs::DirBuilder::new();
    construtor.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        construtor.mode(MODO_DO_DIRETORIO);
    }
    construtor.create(caminho)
}

/// Deixa o arquivo ABERTO legivel so pelo dono -- pelo descritor (`fchmod`),
/// sem janela de caminho trocado entre abrir e apertar.
///
/// Era o motor do pedido 345, por caminho, chamado pelo `.lgpd` e pelo
/// `.fts` DEPOIS de criar; desde o 542 ele e a metade de
/// [`recriar_do_banco`], e os dois arquivos nascem pelo motor como todos os
/// outros.
///
/// Silencioso de proposito: num sistema de arquivos que nao tem modo Unix (um
/// volume FAT, um compartilhamento de rede), falhar aqui derrubaria a
/// gravacao por causa de uma protecao que aquele disco nao sabe oferecer -- e
/// ficar sem o arquivo e pior que ficar sem a permissao.
pub fn apertar_permissao(arquivo: &File) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = arquivo.set_permissions(std::fs::Permissions::from_mode(MODO_DO_ARQUIVO));
    }
    #[cfg(not(unix))]
    {
        let _ = arquivo;
    }
}

/// O ALERTA da base que ja existe com permissao larga: `None` quando tudo
/// debaixo de `raiz` (ela inclusive) e so do dono.
///
/// Nao recusa e nao aperta -- decisao do J pela regua: recusar arrancar e so
/// o PostgreSQL (4) contra MySQL, MariaDB e SQLite (6), e a lei da casa
/// («guarda nova entra pedida, nao imposta») empurra para o mesmo lado. O
/// texto diz quantos, um exemplo e o comando que fecha; quem chama o diz UMA
/// vez, no arranque. Link simbolico DENTRO da arvore nao conta: o modo dele
/// nao e o do alvo, e o alvo pode nem ser do banco.
///
/// Mas a PROPRIA raiz se segue (revisao SEC do 542): `config.base` apontando
/// para um link (`/var/lib/phxsql -> /mnt/dados`) e a instalacao comum, e o
/// `lstat` do topo calava o alerta inteiro nela -- medido contra o `phxsqld`
/// vivo, 1 alerta pelo caminho real e 0 pelo link, na mesma base.
pub fn permissao_larga(raiz: &Path) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let (mut arquivos, mut diretorios) = (0u64, 0u64);
        let mut exemplo: Option<(std::path::PathBuf, u32)> = None;
        let mut pilha = vec![(raiz.to_path_buf(), true)];
        while let Some((atual, topo)) = pilha.pop() {
            let lido = if topo {
                std::fs::metadata(&atual)
            } else {
                std::fs::symlink_metadata(&atual)
            };
            let Ok(meta) = lido else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            let modo = meta.permissions().mode() & 0o777;
            if modo & 0o077 != 0 {
                if meta.is_dir() {
                    diretorios += 1;
                } else {
                    arquivos += 1;
                }
                if exemplo.is_none() {
                    exemplo = Some((atual.clone(), modo));
                }
            }
            if meta.is_dir() {
                if let Ok(itens) = std::fs::read_dir(&atual) {
                    pilha.extend(itens.flatten().map(|i| (i.path(), false)));
                }
            }
        }
        let (caminho, modo) = exemplo?;
        Some(format!(
            "a raiz de dados {} tem {arquivos} arquivo(s) e {diretorios} \
             diretorio(s) que outros usuarios da maquina alcancam (ex.: {} em \
             {modo:o}). Desde o pedido 542 o banco cria tudo 0600/0700, mas nao \
             aperta o que ja existe: apertar sozinho tiraria o acesso de quem \
             hoje le por grupo. Para fechar: chmod -R go-rwx {}",
            raiz.display(),
            caminho.display(),
            raiz.display()
        ))
    }
    #[cfg(not(unix))]
    {
        let _ = raiz;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As bandeiras digitadas a mao ([`bandeiras`]) provadas contra o nucleo
    /// desta arquitetura: o numero errado nao recusaria nada, calado, e so o
    /// `open` de verdade diz qual e o certo. Link no ultimo nome da `ELOOP`,
    /// FIFO sem leitor da `ENXIO` na hora (sem parar a thread), e arquivo
    /// aberto como diretorio da `ENOTDIR`.
    #[cfg(target_os = "linux")]
    #[test]
    fn sem_seguir_recusa_link_e_fifo_de_verdade() {
        let Some((_, diretorio, _)) = bandeiras() else {
            eprintln!("arquitetura sem bandeiras conferidas -- prova pulada");
            return;
        };
        let d = crate::apoio_teste::DirTemp::novo("util-sem-seguir");
        let regular = d.join("regular");
        std::fs::write(&regular, b"x").unwrap();
        let link = d.join("link");
        std::os::unix::fs::symlink(&regular, &link).unwrap();
        let mut abrir = OpenOptions::new();
        abrir.write(true);
        let e = sem_seguir_nem_esperar(&mut abrir).open(&link).unwrap_err();
        assert_eq!(e.raw_os_error(), Some(40), "o O_NOFOLLOW nao recusou: {e}");
        assert!(sem_seguir_nem_esperar(&mut abrir).open(&regular).is_ok());

        let fifo = d.join("fifo");
        if std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .is_ok_and(|s| s.success())
        {
            let e = sem_seguir_nem_esperar(&mut abrir).open(&fifo).unwrap_err();
            assert_eq!(e.raw_os_error(), Some(6), "o O_NONBLOCK nao valeu: {e}");
        }

        assert!(abrir_diretorio(&d, true).is_ok());
        let e = abrir_diretorio(&regular, true).unwrap_err();
        assert_eq!(
            e.raw_os_error(),
            Some(20),
            "o O_DIRECTORY {diretorio:o} nao valeu: {e}"
        );
    }
}

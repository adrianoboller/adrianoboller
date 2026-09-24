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
/// Fica de fora, dito: o link num nome INTERMEDIARIO (`dest/loja -> fora`).
/// O `create_dir_all` o aceita como diretorio que ja existe, e o arquivo vai
/// para o outro lado -- nasce novo, ou, se la ja houver um arquivo regular com
/// o nome de um arquivo do banco (`c.reg`, `_database.json`), e truncado e
/// apertado. Fechar isso pede abrir diretorio a diretorio sem seguir link
/// (`openat`), que a `std` nao da; fica como achado para pedido proprio.
pub fn recriar_do_banco(caminho: &Path, ler: bool) -> std::io::Result<File> {
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
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    let nome = std::fs::symlink_metadata(caminho)?;
    if !nome.file_type().is_file() {
        return Err(recusa_do_nome(caminho, &nome));
    }
    let arquivo = OpenOptions::new().read(ler).write(true).open(caminho)?;
    if !mesmo_arquivo(&arquivo.metadata()?, &nome) {
        return Err(recusa_do_nome(caminho, &nome));
    }
    arquivo.set_len(0)?;
    apertar_permissao(&arquivo);
    Ok(arquivo)
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
pub fn copiar_do_banco(de: &Path, para: &Path) -> std::io::Result<u64> {
    let mut origem = File::open(de)?;
    let mut destino = recriar_do_banco(para, false)?;
    std::io::copy(&mut origem, &mut destino)
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

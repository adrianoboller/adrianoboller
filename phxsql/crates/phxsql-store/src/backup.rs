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
//! "Consistente" aqui quer dizer uma coisa precisa: **nenhuma escrita acontece
//! durante a copia**. Quem chama segura a trava unica de dados do inicio ao
//! fim, e como toda escrita passa por essa mesma trava, nao ha registro pela
//! metade no meio do caminho.
//!
//! E menos do que um snapshot de verdade -- uma escrita longa faz o backup
//! esperar, e o backup faz a escrita esperar.
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
//! Quem chama segura `travar_dados()` do inicio ao fim da COPIA (ela le
//! `raiz`, e e isso que a trava protege). O `fsync`, que so' toca o DESTINO
//! e nao le `raiz` nenhuma, nao precisa da trava -- e a catraca
//! `alcancam-fsync-2` (`bancada/concorrencia/mapa-da-trava.py`) e quem cobra
//! isso: ela conta secoes que alcancam `sync_all` com a trava na mao, e SO'
//! DESCE. `executar`/`executar_zip` escrevem (sem sincronizar) e devolvem os
//! caminhos escritos; [`sincronizar_copias`] roda DEPOIS, com a trava ja solta,
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

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::hash::{para_hex, sha256};
use phxsql_core::json::Json;

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
fn escrever_sem_sync(alvo: &Path, dados: &[u8]) -> Result<()> {
    let arquivo = crate::util::recriar_do_banco(alvo, false)?;
    let escrito = (|| -> Result<()> {
        let mut w = std::io::BufWriter::new(&arquivo);
        w.write_all(dados)?;
        w.flush()?;
        Ok(())
    })();
    if escrito.is_err() {
        drop(arquivo);
        descartar_parcial(alvo);
    }
    escrito
}

/// O `fsync` de UM arquivo ja gravado -- reabre porque quem escreveu pode
/// ja ter fechado o `File` (a lista de caminhos atravessa a fronteira da
/// trava). `read(true).write(true)`: o mesmo par que `NdxFile::abrir` usa,
/// para o `sync_all` valer em qualquer SO, nao so' onde reabrir so' para
/// ler ja bastaria.
///
/// **Nao medido:** reabrir pode perder o erro de *writeback* se o inode
/// sair da memoria no intervalo entre escrever e este `fsync` (o caso
/// *fsyncgate*) -- achado do parecer do DBA de 24/09/2026, fsync depois de
/// reabrir, pedido a abrir.
fn sincronizar_arquivo(alvo: &Path) -> Result<()> {
    let arquivo = OpenOptions::new().read(true).write(true).open(alvo)?;
    // `sem_abortar`: o destino do backup nao e o disco do banco que o
    // gancho do 509 protege -- pedido 524, condicao C1 do parecer do DBA.
    crate::sincronia::sync_all_sem_abortar(&arquivo, alvo)
}

/// Sincroniza cada caminho de `caminhos`, NA ORDEM da lista -- as COPIAS de
/// [`executar`], sem o manifesto: o `backup.json` so nasce depois, em
/// [`finalizar_manifesto`] (pedido 524, condicao C2). Se um `fsync` no meio
/// recusar, os que faltam nunca sincronizam e o `Err` sobe antes de o
/// manifesto chegar a existir.
///
/// Chamada DEPOIS de soltar `travar_dados()` -- ver a nota do modulo.
pub fn sincronizar_copias(caminhos: &[PathBuf]) -> Result<()> {
    for caminho in caminhos {
        sincronizar_arquivo(caminho)?;
    }
    Ok(())
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
/// `banco` vazio copia a raiz inteira. Quem chama segura a trava de dados
/// durante ESTA chamada (ela le `raiz`); o conteudo volta escrito num nome
/// PARCIAL (pedido 524, condicao C2 -- ver [`finalizar_zip`]), e o caminho
/// devolvido e o nome FINAL, que so existe depois de chamar `finalizar_zip`.
pub fn executar_zip(
    raiz: &Path,
    pasta: &Path,
    banco: &str,
    admin: &str,
    quando_ms: i64,
) -> Result<(PathBuf, Relatorio)> {
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
    crate::util::criar_diretorio_do_banco(pasta)?;
    let alvo = pasta.join(nome_do_zip(
        if banco.is_empty() { "dados" } else { banco },
        admin,
        quando_ms,
    ));

    let mut zip = phxsql_core::zip::Zip::novo(quando_ms);
    let mut r = Relatorio {
        database: (!banco.is_empty()).then(|| banco.to_string()),
        ..Relatorio::default()
    };
    for arquivo in listar(&origem)? {
        let rel = relativo(&origem, &arquivo);
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
    escrever_sem_sync(&parcial(&alvo), &bytes)?;
    Ok((alvo, r))
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
        if !nome.to_str().is_some_and(e_nome_de_parcial) {
            continue;
        }
        let caminho = entrada.path();
        let Ok(m) = std::fs::symlink_metadata(&caminho) else {
            continue;
        };
        if !m.file_type().is_file() || !mesmo_dono(&m, nosso) {
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
pub fn finalizar_zip(alvo: &Path) -> Result<()> {
    let parcial = parcial(alvo);
    let trocado = sincronizar_arquivo(&parcial)
        .and_then(|()| crate::sincronia::trocar_duravel_sem_abortar(&parcial, alvo));
    if let Err(e) = trocado {
        // Depois de um `rename` que passou (recusa so no `fsync` do
        // diretorio) o `.part` ja nao existe, e apagar nao acha nada.
        descartar_parcial(&parcial);
        return Err(e);
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
/// Quem chama e responsavel por segurar a trava de dados durante ESTA
/// chamada (ela le `raiz`). Devolve, alem do relatorio, as [`Copias`] (sem o
/// manifesto) para [`concluir`] rodar depois, com a trava ja solta: ele
/// sincroniza cada uma e so entao escreve e sincroniza o `backup.json`, e
/// numa falha apaga o que esta corrida criou (pedido 576). A falha DENTRO
/// desta funcao ja sai com a mesma faxina feita. Ver a nota "Escrever e sincronizar sao DOIS passos" no
/// topo do modulo.
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
    if !raiz.is_dir() {
        return Err(PhxError::NaoEncontrado(format!(
            "a raiz de dados {} nao existe",
            raiz.display()
        )));
    }
    // Copiar para dentro da propria raiz copiaria a copia, sem parar.
    if destino.starts_with(raiz) {
        return Err(PhxError::Esquema(
            "o destino do backup nao pode ficar dentro da raiz de dados".into(),
        ));
    }
    let mut r = Relatorio::default();
    let mut copias = Copias::default();
    // Pedido 576: a recusa no meio da copia (disco cheio, `EFBIG`, nome
    // ocupado) sai daqui com o que esta corrida fez nascer ja apagado --
    // senao ficava uma pasta de copias sem manifesto que ninguem reconhece.
    if let Err(e) = copiar_arvore(raiz, destino, &mut r, &mut copias) {
        descartar_corrida(&copias);
        return Err(e);
    }
    Ok((r, copias))
}

/// O laco de [`executar`], separado para o erro dele passar pela faxina da
/// corrida num lugar so'.
fn copiar_arvore(
    raiz: &Path,
    destino: &Path,
    r: &mut Relatorio,
    copias: &mut Copias,
) -> Result<()> {
    criar_pasta_da_corrida(destino, &mut copias.pastas)?;
    for arquivo in listar(raiz)? {
        let rel = relativo(raiz, &arquivo);
        let dados = std::fs::read(&arquivo)?;
        let alvo = destino.join(&rel);
        if let Some(pai) = alvo.parent() {
            criar_pasta_da_corrida(pai, &mut copias.pastas)?;
        }
        // O `lstat` ANTES de escrever e' o que separa a copia que nasceu
        // agora da que ja existia (destino reaproveitado): so' a primeira e'
        // desta corrida, e so' ela pode sair numa falha.
        let nasce = std::fs::symlink_metadata(&alvo).is_err();
        escrever_sem_sync(&alvo, &dados)?;
        if nasce {
            copias.nascidas.push(alvo.clone());
        }
        copias.caminhos.push(alvo);
        r.bytes += dados.len() as u64;
        r.arquivos.push(Arquivo {
            caminho: rel,
            bytes: dados.len() as u64,
            sha256: para_hex(&sha256(&dados)),
        });
    }
    Ok(())
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
    /// Toda copia escrita, na ordem -- inclusive as que sobrescreveram um
    /// nome que ja existia; todas precisam de `fsync`.
    caminhos: Vec<PathBuf>,
    /// Das copias, as cujo NOME nasceu nesta corrida: so' estas saem numa
    /// falha. A que sobrescreveu um nome antigo nao e' nossa para apagar.
    nascidas: Vec<PathBuf>,
    /// Diretorios que esta corrida criou, do mais externo para o mais interno.
    pastas: Vec<PathBuf>,
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
fn criar_pasta_da_corrida(p: &Path, criadas: &mut Vec<PathBuf>) -> Result<()> {
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
    for a in faltam.into_iter().rev() {
        if std::fs::symlink_metadata(&a).is_ok_and(|m| m.is_dir()) {
            criadas.push(a);
        }
    }
    Ok(criado?)
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
    for pasta in copias.pastas.iter().rev() {
        let _ = std::fs::remove_dir(pasta);
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
pub fn concluir(destino: &Path, quando_ms: i64, r: &Relatorio, copias: &Copias) -> Result<()> {
    let manifesto = destino.join(MANIFESTO);
    let manifesto_nasce = std::fs::symlink_metadata(&manifesto).is_err();
    let feito =
        sincronizar_copias(copias).and_then(|()| finalizar_manifesto(destino, quando_ms, r));
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
pub fn finalizar_manifesto(destino: &Path, quando_ms: i64, r: &Relatorio) -> Result<()> {
    let manifesto = destino.join(MANIFESTO);
    escrever_sem_sync(&manifesto, r.para_json(quando_ms).escrever().as_bytes())?;
    sincronizar_arquivo(&manifesto)
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
        finalizar_manifesto(destino, quando_ms, &r).unwrap();
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
        (alvo, r)
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
        crate::sincronia::falha_de_teste::armar(
            &destino,
            crate::sincronia::falha_de_teste::Onde::Fsync,
            1,
        );
        let e = sincronizar_copias(&caminhos)
            .expect_err("fsync recusado tem de virar Err, nao Ok silencioso");
        crate::sincronia::falha_de_teste::desarmar(&destino);
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
        assert!(
            std::fs::read_dir(&pasta)
                .unwrap()
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().ends_with(".part")),
            "o .part do fsync recusado ficou na pasta"
        );
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
}

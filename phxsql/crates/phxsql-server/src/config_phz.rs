//! O `config.json` gravado como `.phz` -- pedido 450, etapa 2.
//!
//! # O que isto E e o que NAO E
//!
//! Decisao do dono, 24/09/2026, com o custo apresentado antes e confirmado: o
//! arquivo de configuracao passa a poder morar num `.phz` (um 7z de uma
//! entrada, escrito pelo `phxzip`), com uma senha FIXA no binario. Isso e
//! barreira contra quem abre o arquivo num editor ou num visualizador. **Nao e
//! cifra**: o repositorio e publico, `strings` no binario entrega a senha, e
//! ela e a mesma em toda instalacao. Nenhum documento chama isto de cifra.
//!
//! O parecer SEC de 24/09/2026 (`docs/propostas/parecer-sec-phxzip-2026-09-24.md`)
//! tira a conclusao que manda neste modulo: com a senha publica e o sal vazio,
//! o `.phz` nao da sigilo **nem integridade** -- quem consegue escrever o
//! arquivo forja uma configuracao com todos os CRCs certos. A autenticidade do
//! `config.phz` vem de FORA dele: da permissao do arquivo, que o servidor grava
//! 0600 desde o primeiro byte (`config::gravar_privado`), igual ao
//! `config.json`. O `.phz` nao acrescenta nem tira nada disso.
//!
//! # Qual arquivo vale: a regra dos dois
//!
//! Do caminho pedido (`--config`, padrao `config.json`) sai um PAR: o claro e
//! o `.phz` ao lado, com o mesmo nome e a extensao trocada ([`par`]).
//!
//! * So um existe: vale ele. Quem migrou continua subindo com o MESMO
//!   `--config config.json` do systemd, sem editar a unidade.
//! * Os dois existem: o servidor **nao sobe**, e diz os dois caminhos e as
//!   duas saidas. Fonte de verdade ambigua nao se resolve por palpite -- nem
//!   pela data, que um `cp -p` ou um backup restaurado desmentem. O caso real
//!   e o administrador que extraiu o `.phz` com o 7-Zip para editar: escolher
//!   o `.phz` perderia a edicao calado; escolher o `.json` deixaria o `.phz`
//!   velho esperando o proximo engano.
//!
//!   Uma excecao so, a do pedido 481 (`terceiro_no_par`): numa pasta com
//!   sticky bit onde outros gravam, o nome que e de um TERCEIRO -- nem de
//!   quem roda o servidor, nem do root, nem do dono da pasta -- e ignorado,
//!   com aviso, quando o outro e de quem roda ou do root. E o unico caso em
//!   que o sistema operacional prova quem plantou o arquivo; em qualquer
//!   duvida, a recusa de sempre.
//! * Nenhum: o erro de sempre, com o texto de sempre.
//!
//! # A forma de gravar segue a forma de onde se leu
//!
//! «Guarda nova entra pedida, nao imposta»: quem sobe de um `config.json` em
//! claro continua GRAVANDO em claro -- a tela, o `ALTER SERVER`, o cadastro,
//! os nos do cluster. Cerca de sessenta roteiros da bancada e os testes
//! escrevem o `config.json` e o leem de volta depois de gravar pela tela; um
//! servidor que o trocasse por um `.phz` sozinho quebraria todos eles de um
//! dia para o outro. O arranque DIZ que o arquivo esta em claro e qual comando
//! o empacota ([`aviso_de_arranque`]); a migracao e pedida
//! (`phxsqld --empacotar-config`, [`empacotar_arquivo`]) e a volta tambem
//! (`phxsqld --desempacotar-config`, [`desempacotar_arquivo`]), para que editar
//! um campo que a tela nao grava nunca exija a senha.
//!
//! # Um motor so
//!
//! Seis lugares liam o `config.json` com `read_to_string` e um gravava: o
//! `Config::ler`, as tres portas de gravacao, a comparacao da tela com o
//! arquivo e o valor anterior do diario das diretivas. Todos passam por
//! [`ler_texto`] e [`gravar_texto`] -- a forma se decide AQUI, pela extensao,
//! e a porta que alguem esquecesse leria bytes do 7z como se fossem JSON.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use phxsql_core::error::{PhxError, Result};
use phxzip::Limites;

/// A senha do `config.phz`.
///
/// Pedido 450 (`docs/PENDENCIAS.md`): senha FIXA no codigo, decisao do dono de
/// 24/09/2026, depois de o integrador apresentar o custo medido -- o
/// repositorio e publico, `strings` no binario a entrega, e ela e a mesma em
/// toda instalacao. Isto NAO e cifra: e barreira contra quem abre o arquivo
/// num editor. Choque registrado com a petrea «senha nunca em texto puro»; a
/// decisao e do dono, e esta linha a cita.
///
/// Ela nunca vai a log nem a resposta do protocolo -- nenhuma mensagem deste
/// modulo a interpola, e o teste `a_senha_nao_aparece_em_mensagem_nenhuma`
/// (e o irmao pelo binario, em `tests/config-phz.rs`) reprova quem passar a
/// interpolar. `pub` porque a prova contra o 7-Zip do sistema precisa dela,
/// e porque esconder o que o repositorio publica nao protegeria nada.
pub const SENHA_DO_PHZ: &str = "PhxSql.450.barreira-de-editor.nao-e-cifra";

/// As rodadas do 7zAES com que o SERVIDOR grava: 2^10.
///
/// Com a senha publica, as rodadas nao compram protecao nenhuma -- existem
/// para encarecer quem adivinha senha, e aqui nao ha o que adivinhar. So custam
/// tempo, e custam a CADA leitura: o `Config::ler` do arranque e as tres
/// leituras de um `config_gravar` pela tela. Medido em 24/09/2026 pelo
/// `phxsqld --usuarios` em binario de depuracao (mediana): subir do claro
/// custa 2,6 ms, do `.phz` com 2^10 custa 7,6 ms, e do mesmo arquivo
/// regravado pelo 7-Zip, com as 2^19 dele, 1.987 ms. O 7-Zip le de 0 a 24,
/// entao o arquivo continua abrindo nele.
pub const CICLOS_AO_GRAVAR: u8 = 10;

/// O que sobra no nome do `.json` em claro depois de `--empacotar-config`.
/// O nome diz o que houve: ninguem acha um `config.json.migrado-para-phz` e
/// pensa que e lixo de compilacao.
pub const SUFIXO_DO_CLARO_MIGRADO: &str = "migrado-para-phz";
/// O que sobra no nome do `.phz` depois de `--desempacotar-config`.
pub const SUFIXO_DO_PHZ_ABERTO: &str = "aberto-em-json";

/// Folga do arquivo gravado sobre o teto do conteudo: o LZMA2 de dado que nao
/// comprime guarda o bloco cru com alguns bytes por pedaco, e o 7z acrescenta
/// o cabecalho.
const FOLGA_DO_ARQUIVO: u64 = 1 << 20;

/// Quantas copias numeradas de uma migracao anterior se procuram antes de
/// desistir de achar um nome livre.
const COPIAS_NUMERADAS: u32 = 99;

/// Os limites com que o servidor ABRE um `config.phz`.
///
/// O parecer SEC pede, para o que vem do disco sem garantia de origem, ciclos
/// no padrao do 7-Zip e cabecalho pequeno -- nunca os 24 do
/// [`Limites::confiavel`], que custam ~42 s por derivacao em debug JA no
/// abrir. O `.phz` de configuracao tem uma entrada e um cabecalho de centenas
/// de bytes, entao cada teto sai do tamanho do que ele e:
///
/// * `ciclos: 19` -- o que o 7-Zip grava; o administrador que reempacota com
///   ele continua abrindo, e o servidor grava com [`CICLOS_AO_GRAVAR`].
/// * `cabecalho: 64 KiB`, `entradas: 16` -- o cabecalho de uma entrada tem
///   centenas de bytes; 16 deixa um `.phz` com duas entradas cair no erro que
///   diz isso (`MAIS_DE_UMA_ENTRADA`) em vez de num teto de contagem.
/// * `derivacoes: 2` -- uma para o 7-Zip, uma de folga para sal diferente
///   entre cabecalho e conteudo.
/// * `modelo: 64 KiB` -- o LZMA2 nao passa de 28 KiB; o LZMA do 7-Zip usa 16.
/// * `entrada`/`bloco`: o [`phxzip::phz::TETO_PADRAO`] (16 MiB), o mesmo do
///   motor -- um JSON de configuracao tem KiB, e o teto e para a bomba.
pub fn limites_de_leitura() -> Limites {
    Limites {
        entrada: phxzip::phz::TETO_PADRAO,
        bloco: phxzip::phz::TETO_PADRAO,
        cabecalho: 64 << 10,
        ciclos: phxzip::CICLOS_PADRAO,
        derivacoes: 2,
        entradas: 16,
        modelo: 64 << 10,
    }
}

/// O caminho tem a extensao `.phz` (sem diferenca de caixa)?
pub fn e_phz(caminho: &Path) -> bool {
    caminho
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(phxzip::EXTENSAO))
}

/// O par (claro, `.phz`) do caminho pedido.
///
/// `config.json` -> (`config.json`, `config.phz`); `config.phz` ->
/// (`config.json`, `config.phz`); um nome qualquer, como `meu.conf`, ->
/// (`meu.conf`, `meu.phz`). O claro de um `.phz` pedido pelo nome e sempre o
/// `.json`, porque e o unico nome que se pode deduzir dele.
pub fn par(pedido: &Path) -> (PathBuf, PathBuf) {
    if e_phz(pedido) {
        (pedido.with_extension("json"), pedido.to_path_buf())
    } else {
        (
            pedido.to_path_buf(),
            pedido.with_extension(phxzip::EXTENSAO),
        )
    }
}

/// Qual arquivo do par vale -- a regra dos dois, no topo do modulo.
pub fn resolver(pedido: &Path) -> Result<PathBuf> {
    decidir(pedido).map(|d| d.lido)
}

/// O que o arranque decidiu sobre o par: o arquivo que vale e, quando o outro
/// nome foi descartado por ser de um TERCEIRO (pedido 481), qual e de quem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decisao {
    /// O arquivo que vale -- o mesmo que [`resolver`] devolve.
    pub lido: PathBuf,
    /// O outro nome do par, ignorado, e o uid do dono dele. `None` no caso
    /// comum, em que so um dos dois existe.
    pub ignorado: Option<(PathBuf, u32)>,
}

impl Decisao {
    /// O aviso do arranque sobre o nome ignorado.
    ///
    /// Sai da MESMA decisao que escolheu o arquivo. A frente anterior o
    /// recalculava no `main` com uma segunda consulta ao disco, que podia ver
    /// outra coisa e calar (revisao SEC do 481, achado BAIXO). O
    /// `Config::ler` o poe nos `avisos`, que o `main` ja imprime.
    pub fn aviso(&self) -> Option<String> {
        let (ignorado, dono) = self.ignorado.as_ref()?;
        Some(format!(
            "{} e de OUTRO usuario (uid {dono}) -- nem de quem roda o servidor, \
             nem do root, nem do dono da pasta -- numa pasta com sticky bit onde \
             outros gravam: foi ignorado como arquivo de TERCEIRO, e o servidor \
             subiu de {}. Confira quem mais grava nesta pasta; o servidor nao \
             apaga arquivo de ninguem",
            ignorado.display(),
            self.lido.display()
        ))
    }
}

/// A [`Decisao`] do par do caminho pedido -- o motor UNICO de «qual vale».
/// [`resolver`], o `Config::ler` e a troca de forma consomem o resultado;
/// nenhum refaz a comparacao por conta propria.
pub fn decidir(pedido: &Path) -> Result<Decisao> {
    let (claro, phz) = par(pedido);
    let lido = match (claro.exists(), phz.exists()) {
        (true, true) => return decidir_os_dois(&claro, &phz),
        (false, true) => phz,
        (true, false) => claro,
        // Nenhum: o proprio pedido, e quem le da o erro de sempre.
        (false, false) => pedido.to_path_buf(),
    };
    Ok(Decisao {
        lido,
        ignorado: None,
    })
}

/// Os dois presentes: a recusa de sempre, salvo a UNICA excecao que o
/// sistema operacional prova sem palpite ([`terceiro_no_par`]).
fn decidir_os_dois(claro: &Path, phz: &Path) -> Result<Decisao> {
    let (lido, ignorado, dono) = match terceiro_no_par(&Fatos::do_disco(claro, phz)) {
        Some((Vale::Claro, dono)) => (claro, phz, dono),
        Some((Vale::Phz, dono)) => (phz, claro, dono),
        None => return Err(par_ambiguo(claro, phz)),
    };
    Ok(Decisao {
        lido: lido.to_path_buf(),
        ignorado: Some((ignorado.to_path_buf(), dono)),
    })
}

/// Qual dos dois nomes do par o arranque le quando a excecao vale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vale {
    Claro,
    Phz,
}

/// O que se le do disco para decidir o par.
///
/// Separado da decisao para que ela se prove caso a caso SEM `root`: o ramo
/// que falha fechado sobrevivia a mutacao porque so o disco de verdade o
/// exercitava, e so como root (revisao SEC do 481, MEDIO 1). `None` em
/// qualquer campo e «nao sei» -- e «nao sei» e a recusa.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Fatos {
    /// O uid EFETIVO de quem roda este processo.
    euid: Option<u32>,
    /// Dono e `st_mode` da pasta do par.
    pasta: Option<(u32, u32)>,
    /// O dono do NOME de cada um (`lstat`, sem seguir link): numa pasta com
    /// sticky bit, e o dono do nome -- e nao o do alvo de um link que um
    /// terceiro plantou -- quem pode apaga-lo ou troca-lo.
    dono_claro: Option<u32>,
    dono_phz: Option<u32>,
}

const RAIZ: u32 = 0;
/// `S_ISVTX` numa pasta: so o dono do nome, o dono da pasta ou o root apagam
/// ou renomeiam um nome dela.
const STICKY: u32 = 0o1000;
/// Escrita para o grupo ou para os outros.
const GRAVAVEL_POR_OUTROS: u32 = 0o022;

/// A excecao do pedido 481, e so ela: qual nome vale e o uid do ignorado.
///
/// Os dois presentes sao a recusa de sempre ([`par_ambiguo`]). O arranque so
/// ignora um dos dois quando TUDO isto vale, e cada condicao tem o seu
/// motivo e o seu teste:
///
/// * a pasta tem **sticky bit** -- sem ele, quem criou um nome tambem apaga
///   ou troca o outro, e nenhum dos dois merece mais confianca (o
///   `MANUAL.txt` ja prometia isto, e a frente anterior nao cumpria);
/// * a pasta e **gravavel pelo grupo ou pelos outros** -- so ai existe o
///   terceiro que cria um nome sem poder tocar no do servico. Onde so o dono
///   grava, um arquivo de outro dono veio de um `chown` do root, e isso e
///   decisao de quem administra;
/// * um dos nomes e de **confianca** -- do uid efetivo do processo ou do
///   root. O ROOT NUNCA E TERCEIRO: e o administrador que roda `sudo 7z x`
///   para trocar um token vazado, e trata-lo como terceiro subia do `.phz`
///   VELHO com o token revogado valendo (revisao SEC do 481, achado ALTO,
///   provado pelo sistema operacional);
/// * o outro e de um **terceiro** -- nem de confianca, nem do dono da pasta,
///   que apaga e renomeia qualquer nome dela, com ou sem sticky bit.
///
/// Qualquer outro caso -- inclusive os dois de terceiros, e o «nao sei» de
/// qualquer fato -- e a recusa. Falhar fechado e o padrao.
fn terceiro_no_par(f: &Fatos) -> Option<(Vale, u32)> {
    let euid = f.euid?;
    let (dono_da_pasta, modo_da_pasta) = f.pasta?;
    let (claro, phz) = (f.dono_claro?, f.dono_phz?);
    if modo_da_pasta & STICKY == 0 {
        return None;
    }
    if modo_da_pasta & GRAVAVEL_POR_OUTROS == 0 {
        return None;
    }
    let de_confianca = |u: u32| u == euid || u == RAIZ;
    let de_terceiro = |u: u32| !de_confianca(u) && u != dono_da_pasta;
    if de_confianca(claro) && de_terceiro(phz) {
        return Some((Vale::Claro, phz));
    }
    if de_confianca(phz) && de_terceiro(claro) {
        return Some((Vale::Phz, claro));
    }
    None
}

impl Fatos {
    #[cfg(unix)]
    fn do_disco(claro: &Path, phz: &Path) -> Fatos {
        use std::os::unix::fs::MetadataExt as _;
        let dono_do_nome = |p: &Path| std::fs::symlink_metadata(p).ok().map(|m| m.uid());
        Fatos {
            euid: euid_do_processo(),
            pasta: std::fs::metadata(pasta_do_par(claro))
                .ok()
                .map(|m| (m.uid(), m.mode())),
            dono_claro: dono_do_nome(claro),
            dono_phz: dono_do_nome(phz),
        }
    }

    /// Fora do Unix nao ha dono nem sticky bit que a `std` leia: tudo «nao
    /// sei», e o par presente e a recusa de sempre.
    #[cfg(not(unix))]
    fn do_disco(_claro: &Path, _phz: &Path) -> Fatos {
        Fatos::default()
    }
}

/// A pasta onde o par mora. De `config.json` sem pasta, o `parent()` e `""`,
/// que nao se le -- a pasta e a de trabalho.
#[cfg(unix)]
fn pasta_do_par(claro: &Path) -> &Path {
    match claro.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}

/// O uid EFETIVO deste processo, sem `libc` (petrea: zero dependencias
/// externas) e sem escrever nada na pasta.
///
/// A frente anterior criava e apagava um arquivo-sonda na pasta do par: podia
/// deixar lixo, precisava de escrita ali, e num NFS com `root_squash` o dono
/// do arquivo novo mente sobre quem o criou (revisao SEC do 481, BAIXO). No
/// Linux o kernel da o uid efetivo no `/proc/self/status`; onde nao ha isso
/// (macOS, os BSD), a resposta e «nao sei», e o par presente e a recusa de
/// sempre.
#[cfg(unix)]
fn euid_do_processo() -> Option<u32> {
    if cfg!(any(target_os = "linux", target_os = "android")) {
        euid_do_status(&std::fs::read_to_string("/proc/self/status").ok()?)
    } else {
        None
    }
}

/// A linha `Uid:` do `/proc/<pid>/status` traz quatro numeros -- real,
/// EFETIVO, salvo e o do sistema de arquivos (`proc(5)`). O efetivo e o
/// segundo; o real nao serve, porque um binario com setuid tem os dois
/// diferentes.
#[cfg_attr(not(unix), allow(dead_code))]
fn euid_do_status(status: &str) -> Option<u32> {
    let campos = status.lines().find_map(|l| l.strip_prefix("Uid:"))?;
    campos.split_whitespace().nth(1)?.parse().ok()
}

/// A mensagem do impasse -- os dois presentes e nenhum deles descartavel.
/// `resolver` e os testes do par a montam pela MESMA funcao.
fn par_ambiguo(claro: &Path, phz: &Path) -> PhxError {
    PhxError::ConfigAmbiguo(format!(
        "existem os dois, {} e {}, e o servidor nao escolhe por palpite qual \
         deles e a configuracao. Se vale o .json (por exemplo, voce o \
         extraiu do .phz para editar): retire o {} e rode \
         `phxsqld --empacotar-config --config {}`. Se vale o .phz: retire \
         (apague ou renomeie) o {}",
        claro.display(),
        phz.display(),
        phz.display(),
        claro.display(),
        claro.display()
    ))
}

/// A troca de forma grava no OUTRO nome do par. Quando o arranque ignorou
/// esse nome por ser de terceiro, ele esta ocupado por um arquivo que nao e
/// do servico -- e a troca parava no «apareceu durante a troca», que manda
/// procurar uma corrida que nao houve (revisao SEC do 481, BAIXO).
fn recusar_troca_sobre_terceiro(d: &Decisao) -> Result<()> {
    let Some((ignorado, dono)) = &d.ignorado else {
        return Ok(());
    };
    Err(PhxError::ConfigAmbiguo(format!(
        "{} e de outro usuario (uid {dono}) numa pasta com sticky bit: o \
         arranque o ignora e sobe de {}, mas a troca gravaria no nome dele. \
         Nada foi mudado. Retire-o -- ali, so o dono dele, o dono da pasta ou \
         o root conseguem -- e rode a troca de novo",
        ignorado.display(),
        d.lido.display()
    )))
}

/// O texto do arquivo de configuracao, na forma que a extensao diz.
///
/// O claro e lido como sempre foi, com o mesmo erro. O `.phz` e lido com a
/// memoria limitada a 17 MiB ([`ler_limitado`]) e aberto com [`limites_de_leitura`];
/// toda recusa sai com o nome estavel do erro do `phxzip` entre colchetes.
pub fn ler_texto(caminho: &Path) -> Result<String> {
    if !e_phz(caminho) {
        return ler_claro(caminho);
    }
    let bytes = ler_limitado(caminho)?;
    let (_, conteudo) =
        phxzip::desempacotar_com_limites(&bytes, SENHA_DO_PHZ, limites_de_leitura())
            .map_err(|e| recusa(caminho, &e))?;
    String::from_utf8(conteudo).map_err(|_| {
        PhxError::Corrompido(format!(
            "{}: o .phz abriu, e o que esta dentro nao e texto UTF-8 -- nao e um \
             arquivo de configuracao [CONTEUDO_NAO_E_TEXTO]",
            caminho.display()
        ))
    })
}

/// Grava o texto na forma que a extensao diz, 0600 e por troca atomica.
///
/// O `.phz` e conferido NA MEMORIA antes de tocar o disco: empacotado, e
/// aberto de volta com os mesmos limites da leitura. Um defeito no escritor
/// que gravasse um `.phz` que o proprio servidor nao abre derrubaria o
/// proximo arranque -- e isso se descobre aqui, com o arquivo antigo intacto.
pub fn gravar_texto(caminho: &Path, texto: &str) -> Result<()> {
    let empacotado;
    let corpo: &[u8] = if e_phz(caminho) {
        empacotado = empacotar_conferido(caminho, texto)?;
        &empacotado
    } else {
        texto.as_bytes()
    };
    crate::config::gravar_privado(caminho, corpo)
        .map_err(|e| PhxError::Esquema(format!("nao gravei {}: {e}", caminho.display())))
}

/// O que `--empacotar-config` ou `--desempacotar-config` fizeram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Troca {
    /// O arquivo que vale ja estava na forma pedida; nada mudou.
    JaEstava(PathBuf),
    /// `de` virou `para`, e o original ficou guardado em `guardado`.
    Feita {
        de: PathBuf,
        para: PathBuf,
        guardado: PathBuf,
    },
}

/// `phxsqld --empacotar-config`: o `.json` em claro vira `.phz`.
///
/// Quem chama e o `main`, DEPOIS de o `Config::ler` ter subido o arquivo --
/// entao o que se empacota e exatamente o que o servidor aceita. A ordem e a
/// que nunca PERDE a configuracao: o `.phz` e gravado, relido do disco e
/// comparado byte a byte com o `.json`; so entao o `.json` sai do caminho, e
/// sai RENOMEADO ([`SUFIXO_DO_CLARO_MIGRADO`]), nunca apagado. O que acontece
/// quando um passo falha, e quando o processo cai no meio, esta em [`trocar`].
pub fn empacotar_arquivo(pedido: &Path) -> Result<Troca> {
    let (claro, phz) = par(pedido);
    let decisao = existente(pedido)?;
    if e_phz(&decisao.lido) {
        return Ok(Troca::JaEstava(decisao.lido));
    }
    recusar_troca_sobre_terceiro(&decisao)?;
    let texto = ler_claro(&claro)?;
    phxsql_core::json::Json::analisar(&texto).map_err(|e| {
        PhxError::Esquema(format!(
            "{} nao e JSON, e nao foi empacotado: {e}",
            claro.display()
        ))
    })?;
    let bytes = empacotar_conferido(&phz, &texto)?;
    trocar(&claro, &phz, &bytes, SUFIXO_DO_CLARO_MIGRADO, &texto)
}

/// `phxsqld --desempacotar-config`: o `.phz` volta a `.json` em claro, 0600.
///
/// E a porta de quem precisa editar um campo que a tela nao grava (o token, o
/// cadastro, a seguranca) sem precisar da senha nem do 7-Zip. O `.phz` sai do
/// caminho RENOMEADO ([`SUFIXO_DO_PHZ_ABERTO`]); o arranque seguinte sobe do
/// `.json` e avisa que ele esta em claro.
pub fn desempacotar_arquivo(pedido: &Path) -> Result<Troca> {
    let (claro, phz) = par(pedido);
    let decisao = existente(pedido)?;
    if !e_phz(&decisao.lido) {
        return Ok(Troca::JaEstava(decisao.lido));
    }
    recusar_troca_sobre_terceiro(&decisao)?;
    let texto = ler_texto(&phz)?;
    trocar(&phz, &claro, texto.as_bytes(), SUFIXO_DO_PHZ_ABERTO, &texto)
}

/// O que o arranque tem a dizer sobre a FORMA do arquivo que subiu.
///
/// Em claro: que esta em claro, e o comando que o empacota. Em `.phz`: se a
/// copia em claro de uma migracao ainda esta ao lado, que ela continua la --
/// ela carrega o token e os hashes, e o servidor nao apaga arquivo de ninguem.
///
/// Recebe o PEDIDO alem do arquivo lido porque e do pedido que a migracao
/// tira o nome da copia ([`copias_em_claro`], a mesma funcao dos dois lados).
/// Tirar do arquivo lido errava calado fora do `.json`: com `--config
/// meu.conf` a copia e `meu.conf.migrado-para-phz`, e o par do `meu.phz`
/// procurava `meu.json.migrado-para-phz` -- e nunca avisava (revisao SEC de
/// 24/09/2026, A1).
pub fn aviso_de_arranque(pedido: &Path, real: &Path) -> Option<String> {
    if !e_phz(real) {
        return Some(format!(
            "{} esta EM CLARO: qualquer editor le o token e os hashes dele. Para \
             grava-lo como .phz (pedido 450 -- barreira contra editor, NAO e \
             cifra): phxsqld --empacotar-config --config {}",
            real.display(),
            real.display()
        ));
    }
    let copias: Vec<String> = copias_em_claro(pedido)
        .iter()
        .map(|c| c.display().to_string())
        .collect();
    if copias.is_empty() {
        return None;
    }
    Some(format!(
        "a copia EM CLARO de antes da migracao para .phz continua ao lado: {}. \
         Confira que o servidor subiu com a configuracao certa e apague-a -- o \
         servidor nao apaga arquivo seu",
        copias.join(", ")
    ))
}

/// As copias em claro que `--empacotar-config` deixou para este pedido.
///
/// O nome sai do MESMO lugar que a migracao usa para grava-lo: o claro do
/// [`par`] do pedido, e os [`candidatos`] dele. Duas receitas do mesmo nome
/// divergem calado -- foi o que fez o aviso nunca achar a copia do
/// `meu.conf`.
pub fn copias_em_claro(pedido: &Path) -> Vec<PathBuf> {
    let (claro, _) = par(pedido);
    candidatos(&claro, SUFIXO_DO_CLARO_MIGRADO)
        .filter(|c| std::fs::symlink_metadata(c).is_ok())
        .collect()
}

// ------------------------------------------------------------ por dentro

/// A [`decidir`] que exige um arquivo de verdade: trocar a forma de uma
/// configuracao que nao existe e erro, e nao «ja estava».
fn existente(pedido: &Path) -> Result<Decisao> {
    let decisao = decidir(pedido)?;
    if decisao.lido.exists() {
        return Ok(decisao);
    }
    let (claro, phz) = par(pedido);
    Err(PhxError::NaoEncontrado(format!(
        "nao ha configuracao para trocar de forma: nem {} nem {} existem",
        claro.display(),
        phz.display()
    )))
}

/// O claro, lido como sempre foi -- o mesmo erro que os seis leitores davam.
///
/// Com um acrescimo: bytes de 7z num arquivo que nao se chama `.phz` nao sao
/// UTF-8, e o erro cru («stream did not contain valid UTF-8») mandaria
/// procurar um problema de codificacao. Aqui ele diz o que e.
fn ler_claro(caminho: &Path) -> Result<String> {
    std::fs::read_to_string(caminho).map_err(|e| {
        let dica = if e.kind() == std::io::ErrorKind::InvalidData && comeca_como_7z(caminho) {
            " -- os bytes sao de um 7z: se for um .phz, de a ele a extensao .phz"
        } else {
            ""
        };
        PhxError::NaoEncontrado(format!("nao consegui ler {}: {e}{dica}", caminho.display()))
    })
}

fn comeca_como_7z(caminho: &Path) -> bool {
    let mut cabeca = [0u8; 6];
    std::fs::File::open(caminho)
        .and_then(|mut f| f.read_exact(&mut cabeca))
        .map(|()| &cabeca == b"7z\xbc\xaf\x27\x1c")
        .unwrap_or(false)
}

/// Le o `.phz` inteiro, recusando o que passa do teto ANTES de alocar o
/// resto: um `config.phz` de gigabytes (um disco trocado, um arquivo errado
/// com o nome certo) nao pode derrubar o arranque por memoria.
fn ler_limitado(caminho: &Path) -> Result<Vec<u8>> {
    let teto = phxzip::phz::TETO_PADRAO + FOLGA_DO_ARQUIVO;
    let arquivo = std::fs::File::open(caminho).map_err(|e| {
        PhxError::NaoEncontrado(format!("nao consegui ler {}: {e}", caminho.display()))
    })?;
    let mut bytes = Vec::new();
    arquivo
        .take(teto + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| {
            PhxError::NaoEncontrado(format!("nao consegui ler {}: {e}", caminho.display()))
        })?;
    if bytes.len() as u64 > teto {
        return Err(PhxError::LimiteExcedido(format!(
            "{}: passa de {teto} bytes, e um .phz de configuracao tem KiB -- \
             recusado antes de abrir [GRANDE_DEMAIS]",
            caminho.display()
        )));
    }
    Ok(bytes)
}

/// A recusa do `phxzip`, com o caminho, a frase e o nome estavel.
///
/// A frase do `phxzip` nunca carrega a senha (ele a recebe e nao a devolve);
/// a de senha errada ganha aqui o que o administrador precisa saber para
/// agir, sem dize-la.
fn recusa(caminho: &Path, e: &phxzip::Erro) -> PhxError {
    let dica = match e {
        phxzip::Erro::SenhaErrada | phxzip::Erro::SenhaErradaOuCorrompido => {
            " -- o servidor so abre o .phz gravado com a senha do pedido 450 \
             (SENHA_DO_PHZ, em crates/phxsql-server/src/config_phz.rs); quem \
             reempacota com o 7-Zip usa essa"
        }
        _ => "",
    };
    PhxError::Corrompido(format!(
        "{}: o .phz nao abriu: {e}{dica} [{}]",
        caminho.display(),
        e.nome()
    ))
}

/// O nome da entrada dentro do `.phz`: o do `.json` do par. E o que o 7-Zip
/// mostra e o nome com que ele extrai.
fn nome_da_entrada(caminho: &Path) -> String {
    let (claro, _) = par(caminho);
    claro
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config.json".to_string())
}

/// Empacota e abre de volta na memoria, com os limites da leitura.
fn empacotar_conferido(caminho: &Path, texto: &str) -> Result<Vec<u8>> {
    let bytes = phxzip::empacotar_com_ciclos(
        &nome_da_entrada(caminho),
        texto.as_bytes(),
        SENHA_DO_PHZ,
        CICLOS_AO_GRAVAR,
    )
    .map_err(|e| {
        PhxError::Esquema(format!(
            "nao empacotei {}: {e} [{}]",
            caminho.display(),
            e.nome()
        ))
    })?;
    match phxzip::desempacotar_com_limites(&bytes, SENHA_DO_PHZ, limites_de_leitura()) {
        Ok((_, de_volta)) if de_volta == texto.as_bytes() => Ok(bytes),
        Ok(_) => Err(PhxError::Corrompido(format!(
            "{}: o .phz empacotado abriu com outro conteudo; nada foi gravado \
             [CONFERENCIA_NA_MEMORIA]",
            caminho.display()
        ))),
        Err(e) => Err(recusa(caminho, &e)),
    }
}

/// Os nomes que a copia guardada pode ter: `x.sufixo`, `x.sufixo.2`, ...
fn candidatos<'a>(original: &'a Path, sufixo: &'a str) -> impl Iterator<Item = PathBuf> + 'a {
    let base = original
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    (1..=COPIAS_NUMERADAS).map(move |n| {
        if n == 1 {
            original.with_file_name(format!("{base}.{sufixo}"))
        } else {
            original.with_file_name(format!("{base}.{sufixo}.{n}"))
        }
    })
}

/// O primeiro nome livre para guardar o original. Uma copia de uma troca
/// anterior NUNCA e sobrescrita: renomear por cima dela seria apagar calado.
fn destino_livre(original: &Path, sufixo: &str) -> Result<PathBuf> {
    candidatos(original, sufixo)
        .find(|c| std::fs::symlink_metadata(c).is_err())
        .ok_or_else(|| {
            PhxError::Conflito(format!(
                "ja existem {COPIAS_NUMERADAS} copias {}.{sufixo}* ao lado; \
                 retire as antigas antes de trocar de novo",
                original.display()
            ))
        })
}

/// A troca em si, igual nos dois sentidos: grava o NOVO, confere do disco,
/// guarda o VELHO por renomeacao.
///
/// O que ela garante, separado pelo que a interrompe:
///
/// * **Erro tratado** (a conferencia nao bate, a copia nao se guarda): o novo
///   e retirado e o velho continua valendo ([`desfazer`]). Se nem a retirada
///   der, o erro diz que ficaram os dois e manda retirar um; se o velho SUMIU
///   no meio, o novo fica, e o erro diz que ele e o que sobrou.
/// * **Queda ou `kill`** entre gravar o novo e soltar o nome velho: ficam OS
///   DOIS, com o mesmo conteudo, e o arranque recusa (`CONFLITO`) ate alguem
///   retirar um. Nada se perde -- qualquer um dos dois e a configuracao
///   inteira. Antes de o novo existir, a queda deixa no maximo o temporario
///   (`config.phz.tmp`, [`crate::config::temporario_de`]), que ninguem le e a
///   gravacao seguinte apaga.
/// * **Config com outro nome no disco** (link simbolico ou fisico): recusado
///   antes de tudo ([`recusar_outro_nome`]).
///
/// Nao ha `fsync` do diretorio depois do `rename` -- como em toda gravacao de
/// configuracao desta casa --, entao uma queda de ENERGIA logo depois do
/// sucesso pode voltar a um dos dois estados de cima. Nenhum perde dado.
fn trocar(velho: &Path, novo: &Path, corpo: &[u8], sufixo: &str, texto: &str) -> Result<Troca> {
    recusar_outro_nome(velho)?;
    // O par ja foi resolvido, mas entre o resolver e aqui o disco pode ter
    // mudado; gravar por cima de um arquivo que apareceu seria apagar calado.
    // `symlink_metadata`, e nao `exists`: um link quebrado no lugar do novo
    // tambem e alguem, e o `rename` do temporario o apagaria.
    if std::fs::symlink_metadata(novo).is_ok() {
        return Err(PhxError::Conflito(format!(
            "{} apareceu durante a troca; nada foi mudado",
            novo.display()
        )));
    }
    let guardado = destino_livre(velho, sufixo)?;
    crate::config::gravar_privado(novo, corpo)
        .map_err(|e| PhxError::Esquema(format!("nao gravei {}: {e}", novo.display())))?;
    match ler_texto(novo) {
        Ok(relido) if relido == texto => {}
        Ok(_) => return Err(desfazer(velho, novo, "relido, voltou com outro conteudo")),
        Err(e) => return Err(desfazer(velho, novo, &format!("relido, nao abriu: {e}"))),
    }
    // Guardar o velho em tres passos, e cada um tem o seu porque:
    //
    // * `hard_link`, e nao `rename`: o `rename` troca por cima do que tiver
    //   surgido no destino entre o `destino_livre` e aqui, e o `hard_link`
    //   recusa (revisao SEC de 24/09/2026, B1). Uma copia nunca e sobrescrita.
    // * `0600` no mesmo passo: a copia guardada e o config em claro, com o
    //   token e os hashes. O `rename` levava junto o `0644` de uma instalacao
    //   aberta, e ninguem mais regravava a copia -- ficava legivel para
    //   sempre (achado A1 da mesma revisao).
    // * so entao o nome velho sai. Qualquer falha tira o nome extra, e o
    //   velho continua inteiro no lugar.
    if let Err(e) = std::fs::hard_link(velho, &guardado) {
        return Err(desfazer(
            velho,
            novo,
            &format!(
                "nao consegui guardar a copia como {}: {e}",
                guardado.display()
            ),
        ));
    }
    let apertar_e_soltar = apertar(&guardado).and_then(|()| std::fs::remove_file(velho));
    if let Err(e) = apertar_e_soltar {
        let _ = std::fs::remove_file(&guardado);
        return Err(desfazer(
            velho,
            novo,
            &format!("nao consegui fechar a copia {}: {e}", guardado.display()),
        ));
    }
    Ok(Troca::Feita {
        de: velho.to_path_buf(),
        para: novo.to_path_buf(),
        guardado,
    })
}

/// O erro de uma troca que nao se completou, desfazendo o que der.
///
/// O arquivo NOVO so sai se o VELHO ainda estiver la: se o velho sumiu no
/// meio (outro processo, ou o temporario que colidia com ele -- o
/// `--config servidor.tmp` da revisao SEC), o novo e a unica copia da
/// configuracao, e apaga-lo deixaria a pasta vazia. A mensagem diz qual dos
/// dois ficou valendo, e nunca afirma o que nao conferiu.
fn desfazer(velho: &Path, novo: &Path, motivo: &str) -> PhxError {
    if std::fs::symlink_metadata(velho).is_err() {
        return PhxError::Esquema(format!(
            "a troca parou ({motivo}), e {} SUMIU durante ela: {} foi mantido e \
             e a configuracao que sobrou -- confira-o antes de subir",
            velho.display(),
            novo.display()
        ));
    }
    match std::fs::remove_file(novo) {
        Ok(()) => PhxError::Esquema(format!(
            "a troca parou ({motivo}). Desfeito: {} foi retirado e {} continua \
             valendo",
            novo.display(),
            velho.display()
        )),
        Err(e) => PhxError::Esquema(format!(
            "a troca parou ({motivo}), e nao consegui retirar {} ({e}): ficaram \
             os dois, com o mesmo conteudo, e o servidor nao sobe assim -- \
             retire um deles",
            novo.display()
        )),
    }
}

/// Recusa trocar a forma de um config que tem OUTRO nome no disco.
///
/// A troca renomeia o NOME pedido. Se ele e um link simbolico, o que sai do
/// caminho e o link, e o arquivo de verdade -- com o token e os hashes --
/// fica onde estava, na permissao que tinha, enquanto a saida afirma «o
/// original foi guardado» (achado A1 da revisao SEC de 24/09/2026, provado
/// pelo binario). Um link fisico e o mesmo caso por outra porta: o segundo
/// nome continua lendo o conteudo depois da troca. Nos dois, a saida nomeia o
/// arquivo real, e a troca se refaz apontando o `--config` para ele.
fn recusar_outro_nome(velho: &Path) -> Result<()> {
    let meta = std::fs::symlink_metadata(velho).map_err(|e| {
        PhxError::NaoEncontrado(format!("nao consegui ler {}: {e}", velho.display()))
    })?;
    if meta.file_type().is_symlink() {
        let alvo = std::fs::canonicalize(velho)
            .or_else(|_| std::fs::read_link(velho))
            .map(|p| p.display().to_string())
            .unwrap_or_else(|e| format!("(o alvo nao se le: {e})"));
        return Err(PhxError::Conflito(format!(
            "{} e um LINK SIMBOLICO para {alvo}: a troca renomearia so o link e \
             deixaria {alvo}, com o token e os hashes, onde esta. Nada foi \
             mudado; rode a troca com --config {alvo}",
            velho.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if meta.nlink() > 1 {
            return Err(PhxError::Conflito(format!(
                "{} tem {} nomes no disco (link fisico): depois da troca o \
                 conteudo continuaria legivel pelos outros. Nada foi mudado; \
                 retire os outros nomes antes",
                velho.display(),
                meta.nlink()
            )));
        }
    }
    Ok(())
}

/// A copia guardada nasce `0600`, como todo arquivo de configuracao que esta
/// casa grava. Fora do Unix a permissao nao se expressa em modo, e a copia
/// fica com a da pasta.
fn apertar(caminho: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(caminho, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = caminho;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;
    use crate::config::Config;
    use phxsql_core::json::Json;

    /// O `config.json` das provas. O token e reconhecivel de proposito: a
    /// barreira contra editor e justamente ele NAO aparecer nos bytes do
    /// `.phz`.
    const CONFIG: &str = "{\n  \"token\": \"token-de-prova-450\",\n  \"max_linhas\": 10\n}\n";
    const ASSINATURA_7Z: &[u8] = b"7z\xbc\xaf\x27\x1c";

    fn contem(palheiro: &[u8], agulha: &[u8]) -> bool {
        palheiro.windows(agulha.len()).any(|w| w == agulha)
    }

    fn muda(campo: &str, valor: Json) -> Vec<(String, Json)> {
        vec![(campo.to_string(), valor)]
    }

    /// Refaz os dois CRCs do cabecalho de assinatura depois de mexer no
    /// cabecalho seguinte -- o que o atacante tambem faz, porque CRC nao e MAC.
    fn refazer_crcs(d: &mut [u8]) {
        let off = u64::from_le_bytes(d[12..20].try_into().unwrap()) as usize;
        let tam = u64::from_le_bytes(d[20..28].try_into().unwrap()) as usize;
        let crc = phxsql_core::crc32(&d[32 + off..32 + off + tam]);
        d[28..32].copy_from_slice(&crc.to_le_bytes());
        let crc_inicio = phxsql_core::crc32(&d[12..32]);
        d[8..12].copy_from_slice(&crc_inicio.to_le_bytes());
    }

    /// Todo erro da leitura hostil, com o nome que ele tem de trazer. Serve a
    /// prova (f) e a prova (e): a segunda confere que nenhum deles carrega a
    /// senha.
    fn recusas_hostis(d: &Path) -> Vec<(&'static str, PhxError)> {
        let mut saida = Vec::new();
        let mut hostil = |nome: &'static str, bytes: &[u8]| {
            let p = d.join(format!("{nome}.phz"));
            std::fs::write(&p, bytes).unwrap();
            let e = ler_texto(&p).expect_err(nome);
            saida.push((nome, e));
        };

        // Ciclos do 7zAES trocados para 24 no cabecalho, CRCs refeitos.
        let mut ciclos =
            phxzip::empacotar_com_ciclos("config.json", CONFIG.as_bytes(), SENHA_DO_PHZ, 4)
                .unwrap();
        let off = 32 + u64::from_le_bytes(ciclos[12..20].try_into().unwrap()) as usize;
        let marca = [0x24, 0x06, 0xf1, 0x07, 0x01, 0x12, 0x40 | 4];
        let onde = off
            + ciclos[off..]
                .windows(marca.len())
                .position(|w| w == marca)
                .expect("o coder 7zAES do cabecalho nao esta onde devia");
        ciclos[onde + 6] = 0x40 | 24;
        refazer_crcs(&mut ciclos);
        hostil("CICLOS_DEMAIS", &ciclos);

        // Um cabecalho de 80 KB: um nome de 40.000 letras. Comprimido, o
        // arquivo fica pequeno; e o cabecalho DECODIFICADO que passa do teto.
        let nome_longo = "a".repeat(40_000);
        let cabecalho =
            phxzip::empacotar_com_ciclos(&nome_longo, CONFIG.as_bytes(), SENHA_DO_PHZ, 4).unwrap();
        hostil("GRANDE_DEMAIS", &cabecalho);

        hostil("NAO_E_7Z", b"{\"token\":\"isto e json com o nome errado\"}");

        let outra = phxzip::empacotar_com_ciclos(
            "config.json",
            CONFIG.as_bytes(),
            "outra-senha-qualquer",
            4,
        )
        .unwrap();
        hostil("SENHA_ERRADA", &outra);

        let mut sem = phxzip::Escritor::novo(phxzip::Opcoes {
            metodo: phxzip::Metodo::Lzma2,
            senha: None,
            ciclos: 0,
            cifrar_cabecalho: false,
        })
        .unwrap();
        sem.arquivo("config.json", CONFIG.as_bytes(), None).unwrap();
        hostil("SEM_CIFRA", &sem.terminar());

        let mut duas = phxzip::Escritor::novo(phxzip::Opcoes {
            metodo: phxzip::Metodo::Lzma2,
            senha: Some(SENHA_DO_PHZ.into()),
            ciclos: 4,
            cifrar_cabecalho: true,
        })
        .unwrap();
        duas.arquivo("config.json", CONFIG.as_bytes(), None)
            .unwrap();
        duas.arquivo("outro.json", b"{}", None).unwrap();
        hostil("MAIS_DE_UMA_ENTRADA", &duas.terminar());

        let binario =
            phxzip::empacotar_com_ciclos("config.json", &[0xff, 0xfe, 0x00, 0x80], SENHA_DO_PHZ, 4)
                .unwrap();
        hostil("CONTEUDO_NAO_E_TEXTO", &binario);

        // Um arquivo acima do teto: recusado pelo tamanho, antes de abrir.
        let grande = vec![0u8; (phxzip::phz::TETO_PADRAO + FOLGA_DO_ARQUIVO + 1) as usize];
        hostil("GRANDE_DEMAIS", &grande);

        saida
    }

    /// **(a)** O servidor le o `.phz` que ele mesmo gravou -- pela migracao e
    /// pela tela --, e o arquivo continua `.phz` depois de gravar.
    ///
    /// Derruba: `gravar_texto` gravando o texto cru sem olhar a extensao. O
    /// `config_gravar` deixa um JSON legivel dentro do `config.phz`, a
    /// assinatura do 7z some e o `Config::ler` seguinte recusa `NAO_E_7Z`.
    #[test]
    fn o_servidor_le_o_phz_que_ele_mesmo_gravou() {
        let d = DirTemp::novo("phz-a");
        let claro = d.join("config.json");
        let phz = d.join("config.phz");
        std::fs::write(&claro, CONFIG).unwrap();

        let troca = empacotar_arquivo(&claro).unwrap();
        let guardado = d.join("config.json.migrado-para-phz");
        assert_eq!(
            troca,
            Troca::Feita {
                de: claro.clone(),
                para: phz.clone(),
                guardado: guardado.clone()
            }
        );
        assert!(!claro.exists(), "o .json ficou no caminho: seriam os dois");
        assert_eq!(std::fs::read_to_string(&guardado).unwrap(), CONFIG);

        let bytes = std::fs::read(&phz).unwrap();
        assert!(bytes.starts_with(ASSINATURA_7Z));
        assert!(
            !contem(&bytes, b"token-de-prova-450"),
            "o token esta legivel dentro do .phz"
        );

        // O MESMO --config de antes da migracao sobe do .phz.
        let c = Config::ler(&claro).unwrap();
        assert_eq!(c.caminho.as_deref(), Some(phz.as_path()));
        assert_eq!(c.max_linhas, 10);
        assert_eq!(c.token, "token-de-prova-450");

        // Gravar pela tela mantem a forma.
        Config::gravar_campos(&phz, &muda("max_linhas", Json::de_i64(50))).unwrap();
        let depois = std::fs::read(&phz).unwrap();
        assert!(
            depois.starts_with(ASSINATURA_7Z),
            "a gravacao pela tela trocou o .phz por texto"
        );
        assert!(!contem(&depois, b"token-de-prova-450"));
        assert!(!claro.exists(), "a gravacao pela tela fez nascer o .json");
        let relido = Config::ler(&claro).unwrap();
        assert_eq!(relido.max_linhas, 50);
        assert_eq!(relido.token, "token-de-prova-450");

        // A comparacao da tela com o arquivo le o .phz tambem.
        let fora = crate::config::divergencias_do_arquivo(&phz, &c.para_json());
        assert!(
            fora.iter()
                .any(|(campo, v)| campo == "max_linhas" && v.inteiro() == Some(50)),
            "{fora:?}"
        );
    }

    /// **(c)** O `config.json` em claro continua subindo, continua em claro
    /// depois de gravar pela tela, e o arranque diz como empacotar.
    ///
    /// Derruba: `gravar_texto` empacotando sempre -- o `config.json` vira
    /// bytes de 7z, e o `read_to_string` de todo roteiro da bancada quebra. E
    /// `aviso_de_arranque` calado para o claro.
    #[test]
    fn config_json_em_claro_continua_em_claro_e_o_arranque_avisa() {
        let d = DirTemp::novo("phz-c");
        let claro = d.join("config.json");
        std::fs::write(&claro, CONFIG).unwrap();

        let c = Config::ler(&claro).unwrap();
        assert_eq!(c.caminho.as_deref(), Some(claro.as_path()));
        let aviso = aviso_de_arranque(&claro, &claro).expect("o claro subiu calado");
        assert!(aviso.contains("EM CLARO"), "{aviso}");
        assert!(
            aviso.contains("phxsqld --empacotar-config --config"),
            "{aviso}"
        );

        Config::gravar_campos(&claro, &muda("max_linhas", Json::de_i64(50))).unwrap();
        let texto = std::fs::read_to_string(&claro).expect("o .json deixou de ser texto");
        let j = Json::analisar(&texto).unwrap();
        assert_eq!(j.campo("max_linhas").and_then(Json::inteiro), Some(50));
        assert!(
            !d.join("config.phz").exists(),
            "nasceu um .phz sem ninguem pedir"
        );
    }

    /// **(d)** Os dois presentes, MESMO dono (o caso comum: a mesma conta
    /// administra os dois): o servidor nao sobe, a mensagem nomeia os dois e
    /// as duas saidas, e nenhuma troca de forma toca em nenhum deles.
    ///
    /// **O comportamento VELHO que o pedido 481 nao pode afrouxar** -- o
    /// unico que mudou ali e o TIPO do erro (`Conflito` -> `ConfigAmbiguo`,
    /// porque a mensagem antiga "conflito de escrita" nao descrevia um
    /// impasse de arranque). A garantia -- nao escolher por palpite quando
    /// os dois tem o mesmo dono -- continua identica.
    ///
    /// Derruba: o `decidir` escolhendo um dos dois (qualquer um) no caso
    /// `(true, true)` -- o `Config::ler` passa a subir, calado (guarda
    /// `config-phz-dois-presentes-escolhe-calado`).
    #[test]
    fn os_dois_presentes_mesmo_dono_nao_sobem_e_a_troca_nao_toca_em_nenhum() {
        let d = DirTemp::novo("phz-d");
        let claro = d.join("config.json");
        let phz = d.join("config.phz");
        std::fs::write(&claro, CONFIG).unwrap();
        gravar_texto(&phz, "{\"token\":\"t\",\"max_linhas\":99}").unwrap();
        let antes = (std::fs::read(&claro).unwrap(), std::fs::read(&phz).unwrap());

        for pedido in [&claro, &phz] {
            let e = Config::ler(pedido).expect_err("subiu com os dois presentes");
            let texto = e.to_string();
            assert!(matches!(e, PhxError::ConfigAmbiguo(_)), "{texto}");
            assert!(texto.contains(&claro.display().to_string()), "{texto}");
            assert!(texto.contains(&phz.display().to_string()), "{texto}");
            assert!(texto.contains("--empacotar-config"), "{texto}");
            assert!(empacotar_arquivo(pedido).is_err());
            assert!(desempacotar_arquivo(pedido).is_err());
        }
        let depois = (std::fs::read(&claro).unwrap(), std::fs::read(&phz).unwrap());
        assert!(antes == depois, "um dos dois mudou");
        assert_eq!(std::fs::read_dir(&*d).unwrap().count(), 2, "nasceu arquivo");
    }

    // ------------------------------------------ a regra do par (pedido 481)
    //
    // A decisao se prova pelos FATOS, sem `root`: cada condicao da excecao
    // tem o seu teste, e cada um cai com a condicao tirada (as guardas
    // `config-phz-par-*` do catalogo). O disco de verdade -- `chown`, sticky
    // bit, o servidor rodando como o uid 65534 -- se prova pelo binario, em
    // `tests/config-phz.rs`.

    /// O usuario de servico (`User=phxsql` do MANUAL §7.4).
    const SERVICO: u32 = 65534;
    const TERCEIRO: u32 = 65533;
    const OUTRO: u32 = 1000;
    /// O `/tmp` classico: do root, 1777.
    const TMP: (u32, u32) = (RAIZ, 0o1777);

    /// Os fatos de um par com os dois nomes presentes. O `st_mode` da pasta
    /// leva o tipo (`S_IFDIR`) junto, como o `metadata` devolve -- a regra
    /// tem de ignora-lo.
    fn fatos(euid: u32, pasta: (u32, u32), claro: u32, phz: u32) -> Fatos {
        Fatos {
            euid: Some(euid),
            pasta: Some((pasta.0, 0o040000 | pasta.1)),
            dono_claro: Some(claro),
            dono_phz: Some(phz),
        }
    }

    /// **Pedido 481, o que ele pede.** Numa pasta com sticky bit onde outros
    /// gravam, o nome de um terceiro nao trava mais o arranque: vale o do
    /// servico (ou o do root), dos dois lados do par.
    ///
    /// Derruba: a excecao desligada (guarda `config-phz-terceiro-nao-e-ignorado`).
    #[test]
    fn par_terceiro_numa_pasta_com_sticky_e_ignorado() {
        // O B2 do parecer SEC do 450: o `.json` e do servico, um terceiro
        // plantou o `.phz`.
        assert_eq!(
            terceiro_no_par(&fatos(SERVICO, TMP, SERVICO, TERCEIRO)),
            Some((Vale::Claro, TERCEIRO))
        );
        // O espelho: o `.json` plantado, o `.phz` do servico.
        assert_eq!(
            terceiro_no_par(&fatos(SERVICO, TMP, TERCEIRO, SERVICO)),
            Some((Vale::Phz, TERCEIRO))
        );
        // O do root tambem e de confianca: o administrador escreveu o `.json`.
        assert_eq!(
            terceiro_no_par(&fatos(SERVICO, TMP, RAIZ, TERCEIRO)),
            Some((Vale::Claro, TERCEIRO))
        );
        // Gravavel so pelo grupo (1770) tambem e pasta onde outros gravam.
        assert_eq!(
            terceiro_no_par(&fatos(SERVICO, (RAIZ, 0o1770), SERVICO, TERCEIRO)),
            Some((Vale::Claro, TERCEIRO))
        );
    }

    /// **O achado ALTO da revisao SEC do 481.** A instalacao documentada
    /// (MANUAL §7.4: `/opt/phxsql`, `User=phxsql`): o `.phz` e do servico,
    /// que o grava por troca atomica; o administrador roda `sudo 7z x`, troca
    /// o token vazado, e o `.json` nasce do ROOT. A frente anterior chamava o
    /// root de terceiro e subia do `.phz` VELHO -- o token revogado
    /// continuava valendo. O root e o administrador, nunca um terceiro.
    ///
    /// Derruba: o root fora de `de_confianca` -- inclusive numa pasta com
    /// sticky bit (guarda `config-phz-par-root-vira-terceiro`).
    #[test]
    fn par_root_nunca_e_terceiro() {
        // A pasta do servico, 0755, sem sticky: o caso do MANUAL.
        assert_eq!(
            terceiro_no_par(&fatos(SERVICO, (SERVICO, 0o755), RAIZ, SERVICO)),
            None
        );
        // E numa pasta com sticky onde outros gravam, que NAO e do root --
        // na do root, o root ja e o dono da pasta, e isso nao provaria nada.
        for pasta in [(SERVICO, 0o1777), (OUTRO, 0o1777)] {
            assert_eq!(
                terceiro_no_par(&fatos(SERVICO, pasta, RAIZ, SERVICO)),
                None,
                "{pasta:?}"
            );
            assert_eq!(
                terceiro_no_par(&fatos(SERVICO, pasta, SERVICO, RAIZ)),
                None,
                "{pasta:?}"
            );
        }
    }

    /// Sem sticky bit, quem criou um nome tambem apaga ou troca o outro:
    /// nenhum dos dois merece mais confianca. O MANUAL ja prometia isto, e a
    /// frente anterior nao conferia o bit.
    ///
    /// Derruba: a conferencia do sticky bit tirada (guarda
    /// `config-phz-par-sem-sticky-escolhe`).
    #[test]
    fn par_sem_sticky_bit_recusa() {
        for modo in [0o777, 0o775, 0o757] {
            assert_eq!(
                terceiro_no_par(&fatos(SERVICO, (RAIZ, modo), SERVICO, TERCEIRO)),
                None,
                "{modo:o}"
            );
        }
    }

    /// Sticky bit numa pasta que so o dono grava: nenhum terceiro cria nome
    /// ali. O arquivo de outro dono veio de um `chown` do root, e isso e
    /// decisao de quem administra, nao um arquivo plantado.
    #[test]
    fn par_pasta_que_outros_nao_gravam_recusa() {
        for modo in [0o1755, 0o1750, 0o1700] {
            assert_eq!(
                terceiro_no_par(&fatos(SERVICO, (RAIZ, modo), SERVICO, TERCEIRO)),
                None,
                "{modo:o}"
            );
        }
    }

    /// O dono da pasta apaga e renomeia qualquer nome dela, com ou sem sticky
    /// bit: um arquivo dele nao e o de alguem que so consegue CRIAR.
    #[test]
    fn par_dono_da_pasta_nao_e_terceiro() {
        let pasta = (OUTRO, 0o1777);
        assert_eq!(
            terceiro_no_par(&fatos(SERVICO, pasta, SERVICO, OUTRO)),
            None
        );
        assert_eq!(
            terceiro_no_par(&fatos(SERVICO, pasta, OUTRO, SERVICO)),
            None
        );
    }

    /// **O ramo que falha fechado (revisao SEC do 481, MEDIO 1).** Nenhum dos
    /// dois e de confianca, ou os dois sao do mesmo dono: nao ha «o seu» para
    /// escolher, e a recusa de sempre vale. A SEC trocou este ramo por
    /// «escolhe o .json» e 15+8 testes passaram; estes caem.
    ///
    /// Derruba: o `None` final de `terceiro_no_par` virando uma escolha
    /// (guarda `config-phz-par-falha-aberto`).
    #[test]
    fn par_sem_um_lado_de_confianca_recusa() {
        // Os dois de terceiros.
        assert_eq!(terceiro_no_par(&fatos(SERVICO, TMP, OUTRO, TERCEIRO)), None);
        // O mesmo dono nos dois, qualquer que seja.
        for dono in [SERVICO, RAIZ, TERCEIRO] {
            assert_eq!(
                terceiro_no_par(&fatos(SERVICO, TMP, dono, dono)),
                None,
                "{dono}"
            );
        }
        // O servico e o root, cada um com um: os dois de confianca, e nenhum
        // e plantado -- e o ALTO numa pasta com sticky.
        assert_eq!(
            terceiro_no_par(&fatos(SERVICO, (OUTRO, 0o1777), RAIZ, SERVICO)),
            None
        );
    }

    /// **«Nao sei» e a recusa.** O uid de quem roda nao se leu (fora do
    /// Linux, `/proc` ilegivel), ou o dono de um nome, ou a pasta: sem o
    /// fato, nao se sabe quem e «o seu», e a frente anterior tambem nao tinha
    /// teste para isto (revisao SEC do 481, MEDIO 1).
    ///
    /// Derruba: o uid desconhecido tomado como root (guarda
    /// `config-phz-par-sem-euid-escolhe`).
    #[test]
    fn par_sem_um_fato_recusa() {
        let controle = fatos(SERVICO, TMP, RAIZ, TERCEIRO);
        assert!(
            terceiro_no_par(&controle).is_some(),
            "o controle tem de escolher, senao a prova nao prova nada"
        );
        let sem = [
            Fatos {
                euid: None,
                ..controle
            },
            Fatos {
                pasta: None,
                ..controle
            },
            Fatos {
                dono_claro: None,
                ..controle
            },
            Fatos {
                dono_phz: None,
                ..controle
            },
        ];
        for f in sem {
            assert_eq!(terceiro_no_par(&f), None, "{f:?}");
        }
        // E fora do Unix os fatos nascem todos «nao sei».
        assert_eq!(terceiro_no_par(&Fatos::default()), None);
    }

    /// O `Uid:` do `/proc/self/status` traz real, efetivo, salvo e o do
    /// sistema de arquivos: vale o EFETIVO, o segundo.
    #[test]
    fn euid_sai_do_campo_efetivo_do_status() {
        let status = "Name:\tphxsqld\nUmask:\t0022\nUid:\t1000\t65534\t1000\t65534\n\
                      Gid:\t1000\t1000\t1000\t1000\n";
        assert_eq!(euid_do_status(status), Some(65534));
        assert_eq!(euid_do_status("Name:\tphxsqld\n"), None);
        assert_eq!(euid_do_status("Uid:\t1000\n"), None);
        assert_eq!(euid_do_status("Uid:\tx\ty\tz\tw\n"), None);
    }

    /// O uid lido do `/proc` contra o que o KERNEL diz, e nao contra a
    /// propria funcao: um arquivo novo nasce do uid de quem o cria. A frente
    /// anterior conferia a sonda com ela mesma (revisao SEC do 481, MEDIO 3).
    /// O caso realista -- o servico como usuario comum -- se prova pelo
    /// binario rodando como o uid 65534, em `tests/config-phz.rs`.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn euid_do_processo_bate_com_o_dono_de_um_arquivo_novo() {
        use std::os::unix::fs::MetadataExt as _;
        let d = DirTemp::novo("phz-euid");
        let novo = d.join("novo");
        std::fs::write(&novo, b"").unwrap();
        assert_eq!(
            euid_do_processo(),
            Some(std::fs::metadata(&novo).unwrap().uid())
        );
        // `config.json` sem pasta mora na pasta de trabalho.
        assert_eq!(pasta_do_par(Path::new("config.json")), Path::new("."));
        assert_eq!(pasta_do_par(&d.join("config.json")), &*d);
    }

    /// Onde o `/proc/self/status` nao existe, o uid e «nao sei» -- e nao um
    /// palpite.
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
    #[test]
    fn fora_do_linux_o_euid_e_nao_sei() {
        assert_eq!(euid_do_processo(), None);
    }

    /// **O mesmo dono nos dois, numa pasta com sticky bit onde outros
    /// gravam** -- pelo disco de verdade e sem `root`: a pasta e os dois
    /// nomes sao do proprio teste. Passa pelas duas primeiras condicoes e cai
    /// no ramo que falha fechado; a mutacao da SEC (o ramo escolhendo o
    /// `.json`) o derruba.
    #[cfg(unix)]
    #[test]
    fn mesmo_dono_em_pasta_com_sticky_continua_recusando() {
        use std::os::unix::fs::PermissionsExt as _;
        let d = DirTemp::novo("phz-sticky-mesmo-dono");
        std::fs::set_permissions(&*d, std::fs::Permissions::from_mode(0o1777)).unwrap();
        let claro = d.join("config.json");
        let phz = d.join("config.phz");
        std::fs::write(&claro, CONFIG).unwrap();
        gravar_texto(&phz, "{\"token\":\"t\",\"max_linhas\":99}").unwrap();

        // Os fatos sao os do disco: a pasta com sticky, os dois do processo.
        let f = Fatos::do_disco(&claro, &phz);
        assert_eq!(f.pasta.map(|(_, modo)| modo & 0o7777), Some(0o1777));
        assert!(f.euid.is_some(), "{f:?}");
        assert_eq!(f.dono_claro, f.euid, "{f:?}");
        assert_eq!(f.dono_phz, f.euid, "{f:?}");

        let antes = (std::fs::read(&claro).unwrap(), std::fs::read(&phz).unwrap());
        for pedido in [&claro, &phz] {
            let e = Config::ler(pedido).expect_err("subiu com os dois do mesmo dono");
            assert!(matches!(e, PhxError::ConfigAmbiguo(_)), "{e}");
            assert!(matches!(
                empacotar_arquivo(pedido),
                Err(PhxError::ConfigAmbiguo(_))
            ));
            assert!(matches!(
                desempacotar_arquivo(pedido),
                Err(PhxError::ConfigAmbiguo(_))
            ));
        }
        let depois = (std::fs::read(&claro).unwrap(), std::fs::read(&phz).unwrap());
        assert!(antes == depois, "um dos dois mudou");
    }

    /// A mensagem do impasse diz o que e e o que fazer -- e nao «conflito de
    /// escrita», o prefixo da janela de conflito de ESCRITA entre sessoes
    /// (pedido 481).
    #[test]
    fn a_recusa_do_par_diz_configuracao_ambigua_e_nao_conflito_de_escrita() {
        let e = par_ambiguo(Path::new("/x/config.json"), Path::new("/x/config.phz"));
        let texto = e.to_string();
        assert!(
            texto.contains("configuracao ambigua: existem os dois"),
            "{texto}"
        );
        assert!(!texto.contains("conflito"), "{texto}");
        assert_eq!(e.codigo(), 5002);
    }

    /// A troca vai e volta sem apagar nada: o `.phz` aberto fica guardado, e
    /// a segunda migracao guarda o `.json` num nome NOVO em vez de sobrescrever
    /// a copia da primeira.
    ///
    /// Derruba: `destino_livre` devolvendo sempre o primeiro nome -- o
    /// `rename` do Unix troca por cima calado, e a copia da primeira migracao
    /// (com `max_linhas` 10) vira a da segunda (77).
    #[test]
    fn a_troca_vai_e_volta_sem_apagar_copia_nenhuma() {
        let d = DirTemp::novo("phz-ida-volta");
        let claro = d.join("config.json");
        let phz = d.join("config.phz");
        std::fs::write(&claro, CONFIG).unwrap();

        empacotar_arquivo(&claro).unwrap();
        assert_eq!(
            empacotar_arquivo(&claro).unwrap(),
            Troca::JaEstava(phz.clone())
        );
        let volta = desempacotar_arquivo(&phz).unwrap();
        assert_eq!(
            volta,
            Troca::Feita {
                de: phz.clone(),
                para: claro.clone(),
                guardado: d.join("config.phz.aberto-em-json"),
            }
        );
        assert_eq!(std::fs::read_to_string(&claro).unwrap(), CONFIG);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(&claro).unwrap().permissions().mode() & 0o777;
            assert_eq!(modo, 0o600, "o .json aberto nasceu {modo:o}");
        }
        assert_eq!(
            desempacotar_arquivo(&claro).unwrap(),
            Troca::JaEstava(claro.clone())
        );

        // Editado em claro e empacotado de novo.
        std::fs::write(&claro, CONFIG.replace("10", "77")).unwrap();
        let de_novo = empacotar_arquivo(&claro).unwrap();
        let segunda = d.join("config.json.migrado-para-phz.2");
        assert_eq!(
            de_novo,
            Troca::Feita {
                de: claro.clone(),
                para: phz.clone(),
                guardado: segunda.clone(),
            }
        );
        let primeira = std::fs::read_to_string(d.join("config.json.migrado-para-phz")).unwrap();
        assert!(
            primeira.contains("10"),
            "a copia da primeira migracao foi sobrescrita"
        );
        assert!(std::fs::read_to_string(&segunda).unwrap().contains("77"));
        assert_eq!(Config::ler(&claro).unwrap().max_linhas, 77);

        // O arranque do .phz lembra das copias em claro que ficaram.
        let aviso = aviso_de_arranque(&claro, &phz).expect("as copias em claro ficaram caladas");
        assert!(aviso.contains("config.json.migrado-para-phz"), "{aviso}");
        assert!(aviso.contains("config.json.migrado-para-phz.2"), "{aviso}");
    }

    /// **(f)** `.phz` hostil ou corrompido recusa com o nome do erro, sem
    /// panico e dentro dos limites -- e os ciclos 24 recusam ANTES de derivar.
    ///
    /// Derruba: `limites_de_leitura` com `Limites::confiavel()` (ciclos 24 e
    /// cabecalho de 128 MiB). O `.phz` de ciclos 24 passa a derivar 2^24 (~42 s
    /// em debug) e sai `SENHA_ERRADA`; o de cabecalho de 80 KB abre.
    #[test]
    fn phz_hostil_recusa_com_erro_nomeado_sem_panico() {
        let d = DirTemp::novo("phz-f");
        let t0 = std::time::Instant::now();
        let recusas = recusas_hostis(&d);
        let gasto = t0.elapsed();
        let nomes: Vec<&str> = recusas.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            nomes,
            [
                "CICLOS_DEMAIS",
                "GRANDE_DEMAIS",
                "NAO_E_7Z",
                "SENHA_ERRADA",
                "SEM_CIFRA",
                "MAIS_DE_UMA_ENTRADA",
                "CONTEUDO_NAO_E_TEXTO",
                "GRANDE_DEMAIS",
            ]
        );
        for (nome, e) in &recusas {
            let texto = e.to_string();
            assert!(
                texto.contains(&format!("[{nome}]")),
                "esperava [{nome}], veio: {texto}"
            );
        }
        // Oito aberturas, nenhuma derivando 2^24: com o teto certo o conjunto
        // inteiro cabe folgado em poucos segundos mesmo em debug.
        assert!(
            gasto < std::time::Duration::from_secs(10),
            "as recusas levaram {gasto:?}"
        );
        assert!(matches!(
            recusas.last(),
            Some((_, PhxError::LimiteExcedido(_)))
        ));

        // Cortado em qualquer ponto: erro, nunca panico.
        let bom = phxzip::empacotar_com_ciclos("config.json", CONFIG.as_bytes(), SENHA_DO_PHZ, 4)
            .unwrap();
        let p = d.join("cortado.phz");
        for corte in 0..bom.len() {
            std::fs::write(&p, &bom[..corte]).unwrap();
            let e = ler_texto(&p).expect_err("um .phz cortado abriu");
            assert!(e.to_string().contains(".phz nao abriu"), "{e}");
        }
        // E o Config::ler de um .phz hostil devolve o erro, sem subir.
        std::fs::write(d.join("config.phz"), &bom[..bom.len() / 2]).unwrap();
        assert!(Config::ler(d.join("config.json")).is_err());
    }

    /// **(e)** A senha nao aparece em mensagem nenhuma que este modulo
    /// produz: recusas, aviso de arranque, conflito, `Debug` do `Config` e o
    /// JSON da configuracao que o protocolo devolve.
    ///
    /// Derruba: interpolar a senha em qualquer mensagem -- por exemplo, a dica
    /// de senha errada em `recusa` dizendo qual e a senha certa.
    #[test]
    fn a_senha_nao_aparece_em_mensagem_nenhuma() {
        let d = DirTemp::novo("phz-e");
        let mut textos: Vec<String> = recusas_hostis(&d)
            .into_iter()
            .map(|(_, e)| e.to_string())
            .collect();

        let claro = d.join("config.json");
        std::fs::write(&claro, CONFIG).unwrap();
        textos.extend(aviso_de_arranque(&claro, &claro));
        let troca = empacotar_arquivo(&claro).unwrap();
        textos.push(format!("{troca:?}"));
        let phz = d.join("config.phz");
        textos.extend(aviso_de_arranque(&claro, &phz));
        let c = Config::ler(&claro).unwrap();
        textos.push(format!("{c:?}"));
        textos.push(c.para_json().escrever());

        std::fs::write(&claro, CONFIG).unwrap();
        textos.push(Config::ler(&claro).unwrap_err().to_string());
        textos.push(empacotar_arquivo(&claro).unwrap_err().to_string());

        assert!(textos.len() >= 14, "{}", textos.len());
        for t in &textos {
            assert!(!t.contains(SENHA_DO_PHZ), "a senha vazou em: {t}");
        }
    }

    #[cfg(unix)]
    fn modo(p: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    fn abrir_para_todos(p: &Path) {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            modo(p),
            0o644,
            "a fixture nao reproduziu a instalacao aberta"
        );
    }

    /// **O `config.phz` regravado pela tela nasce 0600, sem herdar o 0644.**
    ///
    /// E daqui que vem a autenticidade do config (parecer SEC): o `.phz` nao
    /// da integridade, a permissao da. A gravacao das duas formas passa pela
    /// MESMA linha de `gravar_texto`, e esta prova e a irma da
    /// `o_config_regravado_nasce_0600_sem_herdar_o_original` do `config.rs`,
    /// que confere o claro -- as duas caem com a mesma troca (guarda
    /// `config-json-escreve-aberto-e-herda` do catalogo).
    #[cfg(unix)]
    #[test]
    fn o_phz_regravado_nasce_0600_sem_herdar_o_original() {
        let d = DirTemp::novo("phz-0600");
        let phz = d.join("config.phz");
        gravar_texto(&phz, CONFIG).unwrap();
        abrir_para_todos(&phz);

        Config::gravar_campos(&phz, &muda("max_linhas", Json::de_i64(50))).unwrap();

        assert_eq!(modo(&phz), 0o600, "o config.phz ficou legivel por outros");
        assert!(
            !crate::config::temporario_de(&phz).exists(),
            "o temporario ficou para tras"
        );
        assert_eq!(Config::ler(&phz).unwrap().max_linhas, 50);
    }

    /// **A troca de forma grava o arquivo novo 0600, sem herdar o 0644 do
    /// original** -- nos dois sentidos. E o outro lugar onde o arquivo de
    /// configuracao nasce (`trocar`), e por isso tem guarda propria no
    /// catalogo (`config-phz-troca-escreve-aberto-e-herda`).
    ///
    /// Derruba: `trocar` gravando com `std::fs::write` e copiando a permissao
    /// do original -- o `.phz` de uma instalacao em 0644 nasce 0644.
    #[cfg(unix)]
    #[test]
    fn a_troca_de_forma_grava_0600_sem_herdar_o_original() {
        let d = DirTemp::novo("phz-troca-0600");
        let claro = d.join("config.json");
        let phz = d.join("config.phz");
        std::fs::write(&claro, CONFIG).unwrap();
        abrir_para_todos(&claro);

        empacotar_arquivo(&claro).unwrap();
        assert_eq!(modo(&phz), 0o600, "o .phz da migracao nasceu aberto");

        abrir_para_todos(&phz);
        desempacotar_arquivo(&phz).unwrap();
        assert_eq!(modo(&claro), 0o600, "o .json da volta nasceu aberto");
    }

    /// **A copia em claro que a migracao guarda nasce 0600** -- achado A1 da
    /// revisao SEC de 24/09/2026, caso 1. Ela e o config inteiro, com o token
    /// e os hashes, e o `rename` levava junto o `0644` de uma instalacao
    /// aberta; como ninguem mais a regrava, ficava legivel para sempre. A
    /// resposta vem do sistema operacional, nos dois sentidos da troca.
    ///
    /// Derruba: tirar o `apertar` da troca -- a copia sai `644`.
    #[cfg(unix)]
    #[test]
    fn a_copia_guardada_pela_troca_nasce_0600() {
        let d = DirTemp::novo("phz-copia-0600");
        let claro = d.join("config.json");
        let phz = d.join("config.phz");
        std::fs::write(&claro, CONFIG).unwrap();
        abrir_para_todos(&claro);

        let Troca::Feita { guardado, .. } = empacotar_arquivo(&claro).unwrap() else {
            panic!("a migracao nao se fez");
        };
        assert_eq!(
            modo(&guardado),
            0o600,
            "a copia em claro, com o token, ficou legivel por outros"
        );
        assert_eq!(std::fs::read_to_string(&guardado).unwrap(), CONFIG);

        abrir_para_todos(&phz);
        let Troca::Feita { guardado, .. } = desempacotar_arquivo(&phz).unwrap() else {
            panic!("a volta nao se fez");
        };
        assert_eq!(modo(&guardado), 0o600, "o .phz guardado ficou aberto");
    }

    /// **Config que e link simbolico nao se migra; a recusa nomeia o alvo.**
    /// Caso 2 do A1: a troca renomeava o LINK, dizia «o original foi
    /// guardado», e o arquivo de verdade ficava `644` com o token. E o link
    /// FISICO e o mesmo caso por outra porta.
    ///
    /// Derruba: tirar o `recusar_outro_nome` da troca -- a migracao do link
    /// «da certo», e o `etc/config.json` real continua la, aberto.
    #[cfg(unix)]
    #[test]
    fn config_que_e_link_nao_se_migra_e_a_recusa_nomeia_o_alvo() {
        let d = DirTemp::novo("phz-link");
        let etc = d.join("etc");
        let run = d.join("run");
        std::fs::create_dir_all(&etc).unwrap();
        std::fs::create_dir_all(&run).unwrap();
        let real = etc.join("config.json");
        std::fs::write(&real, CONFIG).unwrap();
        abrir_para_todos(&real);
        let link = run.join("config.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let e = empacotar_arquivo(&link).expect_err("migrou um link simbolico");
        let texto = e.to_string();
        let alvo = std::fs::canonicalize(&real).unwrap();
        assert!(texto.contains("LINK SIMBOLICO"), "{texto}");
        assert!(texto.contains(&alvo.display().to_string()), "{texto}");
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            std::fs::read_dir(&run).unwrap().count(),
            1,
            "nasceu arquivo em run/"
        );
        assert_eq!(std::fs::read_to_string(&real).unwrap(), CONFIG);

        // O link fisico: dois nomes, o mesmo conteudo.
        let outro = d.join("outro-nome.json");
        std::fs::hard_link(&real, &outro).unwrap();
        let e = empacotar_arquivo(&real).expect_err("migrou um config com dois nomes");
        assert!(e.to_string().contains("link fisico"), "{e}");
        assert!(!etc.join("config.phz").exists());
    }

    /// **O aviso acha a copia com o MESMO nome que a migracao usou.** Caso 3
    /// do A1: com `--config meu.conf`, a copia e `meu.conf.migrado-para-phz`,
    /// e o aviso tirava o nome do arquivo lido (`meu.phz` -> `meu.json`) --
    /// nunca a achava.
    ///
    /// Derruba: o aviso procurar pelo par do arquivo LIDO.
    #[test]
    fn o_aviso_acha_a_copia_de_um_config_que_nao_se_chama_json() {
        let d = DirTemp::novo("phz-meu-conf");
        let pedido = d.join("meu.conf");
        std::fs::write(&pedido, CONFIG).unwrap();
        empacotar_arquivo(&pedido).unwrap();
        let copia = d.join("meu.conf.migrado-para-phz");
        assert!(copia.exists());

        let real = Config::ler(&pedido).unwrap().caminho.unwrap();
        assert_eq!(real, d.join("meu.phz"));
        let aviso = aviso_de_arranque(&pedido, &real).expect("a copia em claro ficou calada");
        assert!(aviso.contains(&copia.display().to_string()), "{aviso}");
    }

    /// **`--config servidor.tmp` migra sem se apagar.** Medio 1 da revisao
    /// SEC: o temporario do `gravar_privado` era `with_extension("tmp")` -- o
    /// do `servidor.phz` e o PROPRIO `servidor.tmp` --, a gravacao comecava
    /// apagando o config, a copia nao se guardava (ENOENT), o desfazer
    /// apagava o `.phz`, e a pasta ficava VAZIA com a mensagem dizendo
    /// «servidor.tmp continua valendo».
    ///
    /// Derruba: `temporario_de` voltar a trocar a extensao.
    #[test]
    fn config_com_extensao_tmp_migra_sem_se_apagar() {
        let d = DirTemp::novo("phz-servidor-tmp");
        let pedido = d.join("servidor.tmp");
        std::fs::write(&pedido, CONFIG).unwrap();

        let troca = empacotar_arquivo(&pedido);
        assert!(
            troca.is_ok(),
            "a migracao do servidor.tmp falhou: {troca:?}"
        );
        let guardado = d.join("servidor.tmp.migrado-para-phz");
        assert_eq!(std::fs::read_to_string(&guardado).unwrap(), CONFIG);
        assert!(d.join("servidor.phz").exists());
        assert_eq!(Config::ler(&pedido).unwrap().max_linhas, 10);
        assert!(!crate::config::temporario_de(&d.join("servidor.phz")).exists());
    }

    /// **O desfazer so apaga o novo se o velho ainda existe.** Se o velho
    /// sumiu no meio da troca, o novo e a unica copia da configuracao, e a
    /// mensagem diz isso em vez de afirmar que o velho «continua valendo».
    ///
    /// Derruba: `desfazer` apagando o novo sem olhar o velho -- a pasta fica
    /// vazia.
    #[test]
    fn o_desfazer_nao_apaga_a_unica_copia_que_sobrou() {
        let d = DirTemp::novo("phz-desfazer");
        let velho = d.join("config.json");
        let novo = d.join("config.phz");
        gravar_texto(&novo, CONFIG).unwrap();

        let texto = desfazer(&velho, &novo, "a prova").to_string();
        assert!(novo.exists(), "o desfazer apagou a unica copia: {texto}");
        assert!(texto.contains("SUMIU"), "{texto}");
        assert!(!texto.contains("continua valendo"), "{texto}");

        // O caso comum: o velho esta la, e o novo sai.
        std::fs::write(&velho, CONFIG).unwrap();
        let texto = desfazer(&velho, &novo, "a prova").to_string();
        assert!(!novo.exists(), "{texto}");
        assert!(texto.contains("continua valendo"), "{texto}");
    }

    #[test]
    fn o_par_troca_so_a_extensao() {
        let p = Path::new;
        assert_eq!(
            par(p("/x/config.json")),
            (p("/x/config.json").into(), p("/x/config.phz").into())
        );
        assert_eq!(
            par(p("/x/config.PHZ")),
            (p("/x/config.json").into(), p("/x/config.PHZ").into())
        );
        assert_eq!(
            par(p("meu.conf")),
            (p("meu.conf").into(), p("meu.phz").into())
        );
        assert!(e_phz(p("a.Phz")) && !e_phz(p("a.phz.aberto-em-json")));
    }
}

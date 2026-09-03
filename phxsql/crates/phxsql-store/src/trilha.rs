//! `.lgpd` -- quem mexeu e quem viu o dado pessoal, coluna a coluna.
//!
//! Os tres diarios que ja existiam cobrem a EXCLUSAO e a INCLUSAO por inteiro:
//! o `.log` registra toda operacao com rowid e instante, a `.trash` guarda a
//! linha inteira antes de ela sumir, e o `.reason` guarda quem excluiu e por
//! que. O que nenhum dos tres tem onde dizer e o que esta trilha guarda:
//!
//! * a **alteracao**, com o valor ANTES e o valor DEPOIS, **por coluna** -- o
//!   evento do `.log` tem tamanho fixo e nao cabe um valor; a imagem da linha
//!   no diario, quando ligada, guarda a linha nova e nao o par;
//! * o **acesso**, que nenhum dos tres registra, porque ler nao muda nada.
//!
//! ```text
//! cadastroClientes.reg + .ndx + .bin + .memo + .log + .trash + .reason + .lgpd
//! ```
//!
//! # O que NAO entra aqui, e por que
//!
//! Insercao, exclusao (fisica ou suave) e restauracao **nao geram trilha**.
//! Nao e economia: e que os tres ja estao registrados, com data, hora e autor,
//! nos arquivos acima. Um segundo registro do mesmo evento noutro arquivo cria
//! duas verdades sobre o mesmo fato, e a que ficar para tras vira a que engana
//! quem audita. A trilha cobre o buraco, nao o que ja esta coberto.
//!
//! # Registro (56 bytes de cabecalho + cinco textos)
//!
//! ```text
//! [carimbo i64 ms][tipo u8][flags u8][antes_len u16]
//! [rowid u64][usuario u32]
//! [uuid do evento 16 bytes]
//! [depois_len u16][ident_len u16][linhas u32]
//! [coluna_len u16][ip_len u8][reservado u8][crc32 u32]
//! [coluna][antes][depois][identidade][ip]        (utf-8, nesta ordem)
//! ```
//!
//! O desenho e o do `.reason`, de proposito: cabecalho de tamanho fixo que diz
//! onde o proximo registro comeca, textos de tamanho variavel atras, CRC-32
//! sobre os dois, e o UUID v7 do proprio evento -- que ordena por tempo e
//! serve de tempero do nonce sem gastar byte novo.
//!
//! # A identidade da linha: rowid E chave, como no `.reason`
//!
//! Os dois, e nao um. O `rowid` e a POSICAO fisica da linha neste servidor:
//! serve para achar a linha agora, e nao atravessa replicacao -- o mesmo
//! cliente tem rowid diferente em cada no. A `identidade` e a chave em TEXTO
//! (`cpf=012...`, `id=42`), que e o que um auditor pergunta e o que sobrevive
//! a linha: o registro continua legivel depois de a linha ser excluida, e
//! continua significando a mesma pessoa em qualquer servidor.
//!
//! Guardar so o rowid daria um registro que aponta para o nada seis meses
//! depois. Guardar so a chave tiraria o caminho de volta para a linha viva. O
//! `.reason` ja resolveu isso assim, e repetir a solucao dele custa 8 bytes.
//!
//! # A coluna vai pelo NOME, e nao pela posicao
//!
//! Uma posicao (`coluna 4`) so se le com o esquema da epoca na mao, e o
//! esquema muda: coluna acrescentada, renomeada, tirada da tela. A trilha
//! sobrevive ao esquema como sobrevive a linha, entao ela carrega o nome. E o
//! mesmo motivo de a `identidade` ser texto.
//!
//! # O registro de ACESSO e por OPERACAO, nunca por linha
//!
//! Uma varredura de 10.000 linhas com seis colunas marcadas geraria 60.000
//! registros por consulta se a trilha fosse por celula lida -- a trilha
//! ficaria maior que a tabela em poucas horas, e o custo cairia em cima da
//! leitura, que e o caminho quente. Um registro por operacao responde a
//! pergunta que o auditor faz de verdade ("quem viu o prontuario do fulano?")
//! porque guarda o CRITERIO da consulta na `identidade`: quem leu, quando, de
//! que IP, quais colunas marcadas a consulta tocou, quantas linhas voltaram e
//! com que filtro. Os dois custos estao medidos em `docs/LGPD.md`.
//!
//! # O arquivo mais perigoso da tabela
//!
//! Ele concentra, em claro, exatamente o que a lei manda proteger: o valor de
//! antes e o de depois das colunas marcadas. Por isso ele **nasce so quando
//! precisa** (tabela sem coluna marcada nunca cria o arquivo), nasce com
//! permissao `0600`, e entra na mesma cifra e no mesmo interruptor dos outros
//! tres diarios (`crate::cofre`). Ver `docs/LGPD.md`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use phxsql_core::crc::crc32;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::paginacao::Paginacao;
use phxsql_core::uuid::Uuid;
use phxsql_core::value::Value;
use phxsql_core::RowId;

use crate::cofre::{self, Cabecalho};
use crate::util::{agora_ms, por_u16, por_u32, por_u64, Campos};
use crate::volume::Volumes;

pub const MAGIC_TRILHA: &[u8; 8] = b"PHXLGP\0\0";
pub const EXT_LGPD: &str = "lgpd";

/// Bytes do cabecalho de cada registro, antes dos cinco textos.
pub const REGISTRO_CAB: usize = 56;

/// Teto de cada valor guardado (antes e depois), em bytes.
///
/// A trilha e a PROVA de que o valor mudou, e nao uma segunda copia da tabela.
/// Um `Memo` de dois megabytes gravado inteiro duas vezes por alteracao faria
/// o arquivo mais perigoso da tabela ser tambem o maior dela. Mil e vinte e
/// quatro bytes mostram a mudanca; quem precisa do dado inteiro tem a linha.
pub const VALOR_MAX: usize = 1024;
/// Teto do nome da coluna -- ou da lista delas, no registro de acesso.
pub const COLUNA_MAX: usize = 2000;
/// Teto da identidade da linha, ou do criterio da consulta.
pub const IDENTIDADE_MAX: usize = 512;
/// Teto do endereco de origem. Um IPv6 com escopo cabe folgado.
pub const IP_MAX: usize = 64;

/// Bit 0 das flags: o texto de `antes` e uma marca de redacao, nao o valor.
pub const FLAG_ANTES_REDIGIDO: u8 = 1;
/// Bit 1 das flags: idem para `depois`.
pub const FLAG_DEPOIS_REDIGIDO: u8 = 2;

/// O que aconteceu com o dado pessoal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    /// Uma coluna marcada mudou de valor. Um registro POR COLUNA que mudou.
    Alteracao,
    /// Uma operacao leu colunas marcadas. Um registro por OPERACAO.
    Acesso,
}

impl Tipo {
    fn tag(self) -> u8 {
        match self {
            Tipo::Alteracao => 1,
            Tipo::Acesso => 2,
        }
    }

    fn de_tag(t: u8) -> Result<Tipo> {
        Ok(match t {
            1 => Tipo::Alteracao,
            2 => Tipo::Acesso,
            outro => {
                return Err(PhxError::Corrompido(format!(
                    "tipo desconhecido no .lgpd: {outro}"
                )))
            }
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Tipo::Alteracao => "alteracao",
            Tipo::Acesso => "acesso",
        }
    }
}

/// Um evento da trilha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evento {
    /// Identidade do evento. v7: ordenar por ele e ordenar por tempo.
    pub uuid: Uuid,
    /// Milissegundos desde 1970-01-01T00:00:00Z.
    pub carimbo: i64,
    pub tipo: Tipo,
    /// Posicao fisica da linha NESTE servidor. Zero num acesso que varreu.
    pub rowid: RowId,
    /// Quem fez. Zero = nao informado (token de servico, ou escrita local).
    pub usuario: u32,
    /// A coluna que mudou; ou, no acesso, as colunas marcadas que a operacao
    /// tocou, separadas por virgula.
    pub coluna: String,
    /// O valor antes. Vazio no acesso.
    pub antes: String,
    /// O valor depois. Vazio no acesso.
    pub depois: String,
    /// A chave da linha, em texto; ou, no acesso, o criterio da consulta.
    pub identidade: String,
    /// De onde veio o pedido. Vazio quando a escrita nasceu aqui dentro.
    pub ip: String,
    /// Linhas que a operacao devolveu. So no acesso.
    pub linhas: u32,
    /// Ver [`FLAG_ANTES_REDIGIDO`].
    pub flags: u8,
}

impl Evento {
    /// Data e hora em ISO (`AAAA-MM-DD HH:MM:SS,mmm`).
    pub fn instante_iso(&self) -> String {
        phxsql_core::datahora::instante_iso(self.carimbo)
    }

    /// O valor de antes foi redigido por ser segredo?
    pub fn antes_redigido(&self) -> bool {
        self.flags & FLAG_ANTES_REDIGIDO != 0
    }

    /// O valor de depois foi redigido por ser segredo?
    pub fn depois_redigido(&self) -> bool {
        self.flags & FLAG_DEPOIS_REDIGIDO != 0
    }

    /// Bytes de texto claro que este registro carrega.
    fn texto_len(&self) -> usize {
        self.coluna.len()
            + self.antes.len()
            + self.depois.len()
            + self.identidade.len()
            + self.ip.len()
    }

    fn claro(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.texto_len());
        v.extend_from_slice(self.coluna.as_bytes());
        v.extend_from_slice(self.antes.as_bytes());
        v.extend_from_slice(self.depois.as_bytes());
        v.extend_from_slice(self.identidade.as_bytes());
        v.extend_from_slice(self.ip.as_bytes());
        v
    }

    fn escrever(&self, cab: &Cabecalho, offset: u64) -> Vec<u8> {
        let mut buf = vec![0u8; REGISTRO_CAB];
        buf[0..8].copy_from_slice(&self.carimbo.to_le_bytes());
        buf[8] = self.tipo.tag();
        buf[9] = self.flags;
        // Os tamanhos sao os do TEXTO CLARO, e nao os do que vai ao disco: e
        // por eles que os cinco textos se separam depois de decifrar.
        por_u16(&mut buf, 10, self.antes.len() as u16);
        por_u64(&mut buf, 12, self.rowid);
        por_u32(&mut buf, 20, self.usuario);
        buf[24..40].copy_from_slice(self.uuid.bytes());
        por_u16(&mut buf, 40, self.depois.len() as u16);
        por_u16(&mut buf, 42, self.identidade.len() as u16);
        por_u32(&mut buf, 44, self.linhas);
        por_u16(&mut buf, 48, self.coluna.len() as u16);
        buf[50] = self.ip.len() as u8;

        let corpo = cab.selar(
            tempero(self.uuid.bytes()),
            offset,
            &associado(&buf),
            &self.claro(),
        );
        buf.extend_from_slice(&corpo);

        // O CRC cobre o cabecalho SEM o proprio campo, e o corpo COMO ELE VAI
        // AO DISCO: um valor adulterado tem de ser pego como qualquer outro
        // dado, e a varredura confere o arquivo sem precisar da chave.
        let mut crc = crc32(&buf[..52]);
        crc = phxsql_core::crc::crc32_with(crc, &buf[REGISTRO_CAB..]);
        por_u32(&mut buf, 52, crc);
        buf
    }

    /// Quanto ocupa no disco o registro que comeca neste cabecalho.
    ///
    /// Sai so do cabecalho, que e sempre claro: e o que deixa caminhar pelo
    /// arquivo sem ter a chave.
    fn ocupa_do_cabecalho(cab: &Cabecalho, c: &[u8]) -> usize {
        let claro = u16::from_le_bytes([c[10], c[11]]) as usize
            + u16::from_le_bytes([c[40], c[41]]) as usize
            + u16::from_le_bytes([c[42], c[43]]) as usize
            + u16::from_le_bytes([c[48], c[49]]) as usize
            + c[50] as usize;
        REGISTRO_CAB + cab.ocupa(claro)
    }

    /// Le a partir de `src`, que precisa ter o registro inteiro.
    fn ler(src: &[u8], cab: &Cabecalho, offset: u64, nome: &str) -> Result<Evento> {
        if src.len() < REGISTRO_CAB {
            return Err(PhxError::Corrompido("registro de .lgpd truncado".into()));
        }
        let c = Campos(src);
        let n_antes = c.u16(10) as usize;
        let n_depois = c.u16(40) as usize;
        let n_ident = c.u16(42) as usize;
        let n_coluna = c.u16(48) as usize;
        let n_ip = src[50] as usize;
        let total = Evento::ocupa_do_cabecalho(cab, src);
        if src.len() < total {
            return Err(PhxError::Corrompido(
                "registro de .lgpd menor que os tamanhos que declara".into(),
            ));
        }
        let mut crc = crc32(&src[..52]);
        crc = phxsql_core::crc::crc32_with(crc, &src[REGISTRO_CAB..total]);
        if crc != c.u32(52) {
            return Err(PhxError::Corrompido(
                "registro de .lgpd com CRC invalido".into(),
            ));
        }
        let uuid = Uuid::de_bytes(src[24..40].try_into().unwrap());
        let claro = cab.abrir(
            tempero(uuid.bytes()),
            offset,
            &associado(&src[..REGISTRO_CAB]),
            &src[REGISTRO_CAB..total],
            nome,
        )?;
        if claro.len() < n_coluna + n_antes + n_depois + n_ident + n_ip {
            return Err(PhxError::Corrompido(
                "registro de .lgpd com menos texto do que declara".into(),
            ));
        }
        let mut p = 0usize;
        let mut pedaco = |n: usize| -> Result<String> {
            let s = String::from_utf8(claro[p..p + n].to_vec())
                .map_err(|e| PhxError::Corrompido(format!(".lgpd nao e UTF-8 valido: {e}")))?;
            p += n;
            Ok(s)
        };
        Ok(Evento {
            uuid,
            carimbo: c.u64(0) as i64,
            tipo: Tipo::de_tag(src[8])?,
            flags: src[9],
            rowid: c.u64(12),
            usuario: c.u32(20),
            linhas: c.u32(44),
            coluna: pedaco(n_coluna)?,
            antes: pedaco(n_antes)?,
            depois: pedaco(n_depois)?,
            identidade: pedaco(n_ident)?,
            ip: pedaco(n_ip)?,
        })
    }
}

/// O dado associado da etiqueta: o cabecalho do registro, menos o CRC.
///
/// O CRC fica de fora porque depende do corpo, que depende da etiqueta, que
/// depende do dado associado.
fn associado(cab: &[u8]) -> [u8; REGISTRO_CAB] {
    let mut aad = [0u8; REGISTRO_CAB];
    aad.copy_from_slice(&cab[..REGISTRO_CAB]);
    aad[52..56].fill(0);
    aad
}

/// Os quatro bytes de tempero do nonce saem do UUID do proprio registro.
///
/// Nao ha byte novo a gravar: o UUID v7 ja esta no cabecalho e ja e unico por
/// definicao. Ele cobre o unico caso em que o offset se repetiria -- o
/// registro que entra por cima de um rabo estragado por uma queda.
fn tempero(uuid: &[u8; 16]) -> [u8; 4] {
    [uuid[12], uuid[13], uuid[14], uuid[15]]
}

// ------------------------------------------------------------------ redacao

/// Pedacos de nome que denunciam uma coluna guardadora de segredo.
///
/// A lista e casada por CONTEM, e nao por igualdade, e isso e deliberado: aqui
/// o falso positivo custa uma linha de trilha que diz "(redigido)" onde podia
/// dizer um valor, e o falso negativo custa uma senha gravada em claro no
/// arquivo mais perigoso da tabela. Entre os dois erros, este codigo escolhe
/// sempre o primeiro.
const NOMES_DE_SEGREDO: &[&str] = &[
    "senha",
    "password",
    "passwd",
    "pwd",
    "hash",
    "token",
    "segredo",
    "secret",
    "credencial",
    "credential",
    "apikey",
];

/// O nome desta coluna diz que ela guarda segredo?
pub fn nome_de_segredo(coluna: &str) -> bool {
    let n = coluna.to_ascii_lowercase();
    NOMES_DE_SEGREDO.iter().any(|s| n.contains(s))
}

/// O valor de uma coluna marcada, pronto para a trilha -- e se foi redigido.
///
/// # Redige ANALISANDO, nunca recortando
///
/// Sao duas conferencias, e as duas olham ESTRUTURA e nao texto solto:
///
/// 1. **A coluna declarada.** O nome dela vem do esquema, e o esquema e o
///    lugar em que alguem declarou o que aquilo e. Uma coluna `senha_hash`
///    marcada como dado pessoal nao tem valor que possa ir para a trilha, e
///    isso se decide antes de olhar o conteudo -- inclusive quando o conteudo
///    e a senha ainda em texto puro, que e justamente o caso pior.
/// 2. **O valor que se ANALISA como hash.** `senha::e_hash` nao procura
///    padrao dentro do texto: ele DESTRINCHA a linha nos quatro campos do
///    formato (`pbkdf2-sha256$iteracoes$sal$derivado`), confere o algoritmo,
///    o numero de iteracoes e o hexadecimal dos dois lados. Se destrincha, e
///    um hash -- venha da coluna que vier, chame-se ela como se chamar. E o
///    que pega o hash gravado numa coluna de nome inocente.
///
/// O que nao se analisa nao vira texto, vira tamanho: `Value::Bin` ja sai como
/// `"N bytes"` do proprio `para_texto`, e e o certo -- uma biometria e
/// exatamente o dado que a lei manda proteger, e coloca-la na trilha seria
/// concentrar o pior num arquivo so.
pub fn valor_para_trilha(coluna: &str, v: &Value) -> (String, bool) {
    let redigir = |quanto: usize| (format!("(redigido: {quanto} bytes)"), true);
    if v.e_null() {
        // Nulo nao e segredo: e a ausencia de valor, e esconde-la apagaria a
        // informacao mais util da trilha -- que o campo foi preenchido, ou
        // esvaziado, por alguem.
        return (String::new(), false);
    }
    if nome_de_segredo(coluna) {
        return redigir(bytes_do_valor(v));
    }
    match v {
        Value::Str(s) | Value::Memo(s) if phxsql_core::senha::e_hash(s) => redigir(s.len()),
        _ => (cortar(&v.para_texto(), VALOR_MAX), false),
    }
}

/// Quantos bytes o valor ocupa, para a marca de redacao dizer o tamanho sem
/// dizer o conteudo.
fn bytes_do_valor(v: &Value) -> usize {
    match v {
        Value::Str(s) | Value::Memo(s) => s.len(),
        Value::Bin(b) => b.len(),
        outro => outro.para_texto().len(),
    }
}

/// Corta em `max` BYTES sem partir caractere no meio.
///
/// Cortar por `char` custaria percorrer a string inteira; cortar por byte cru
/// produziria UTF-8 invalido que nem volta da leitura. Este anda para tras ate
/// o inicio de um caractere, o que sao no maximo tres passos.
fn cortar(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut fim = max;
    while fim > 0 && !s.is_char_boundary(fim) {
        fim -= 1;
    }
    s[..fim].to_string()
}

// ------------------------------------------------------------- interruptor

/// A trilha registra ALTERACAO de coluna marcada?
static ALTERACOES: AtomicBool = AtomicBool::new(true);
/// A trilha registra ACESSO a coluna marcada?
static ACESSOS: AtomicBool = AtomicBool::new(true);

/// Liga ou desliga os dois lados da trilha, para o processo inteiro.
///
/// # Por que esta nasce LIGADA, se guarda nova entra pedida
///
/// A regra da casa e que protecao nova nao se impoe a quem nao pediu, porque
/// quebrar todo cliente antigo nao e proteger. Aqui ela **nao e quebrada**, e
/// vale entender por que antes de mudar isto:
///
/// a trilha so acontece em tabela que tem coluna marcada como dado pessoal, e
/// marcar e um ato deliberado de quem cadastrou o campo. **Nenhuma tabela que
/// existe hoje sem marca muda de comportamento** -- nao ganha arquivo, nao
/// paga custo, nao responde diferente. Quem marcou uma coluna ja declarou que
/// aquilo e dado pessoal; a trilha e a consequencia legal dessa declaracao, e
/// e por isso que ela vem ligada em vez de esperar um segundo pedido.
///
/// O interruptor existe para quem precise desligar -- uma carga de migracao,
/// um ambiente de teste, um disco pequeno --, e nao para quem precise ligar.
pub fn definir(alteracoes: bool, acessos: bool) {
    ALTERACOES.store(alteracoes, Ordering::Relaxed);
    ACESSOS.store(acessos, Ordering::Relaxed);
}

/// A trilha de alteracao esta ligada?
pub fn alteracoes_ligadas() -> bool {
    ALTERACOES.load(Ordering::Relaxed)
}

/// A trilha de acesso esta ligada?
pub fn acessos_ligados() -> bool {
    ACESSOS.load(Ordering::Relaxed)
}

// ------------------------------------------------------------------ arquivo

/// O `.lgpd` de uma tabela.
///
/// # Por que este nasce preguicoso, e os outros tres nao
///
/// `LogFile`, `LixeiraFile` e `MotivoFile` criam o arquivo na hora em que a
/// tabela e criada, porque toda tabela tem eventos, exclusoes e motivos --
/// mais cedo ou mais tarde. A trilha e o contrario: a maioria das tabelas de
/// um banco (as de apoio, as de dominio, as de configuracao) nao tem UMA
/// coluna marcada, e para essas o arquivo nunca teria nada dentro.
///
/// Criar um `.lgpd` vazio em toda tabela custaria um arquivo a mais por tabela
/// no disco, no backup e no `zip` -- e, pior, faria a pergunta "esta tabela
/// tem trilha?" ser respondida com "tem arquivo" em vez de "tem dado
/// pessoal". Aqui o arquivo so aparece quando o primeiro evento aparece, e a
/// presenca dele ja e a resposta.
pub struct TrilhaFile {
    volumes: Volumes,
    cabs: HashMap<u32, Cabecalho>,
    volume_atual: u32,
    /// O volume 1 ja existe no disco?
    nasceu: bool,
    /// Usuario aplicado aos registros gravados daqui em diante.
    pub usuario: u32,
    /// IP aplicado aos registros gravados daqui em diante.
    pub ip: String,
}

impl TrilhaFile {
    /// Abre sem tocar no disco quando o arquivo nao existe.
    ///
    /// **Nao cria**, ao contrario do `.reason`. Tabela sem coluna marcada
    /// nunca chega a `registrar`, entao nunca ganha arquivo -- e tabela
    /// gravada antes desta versao abre exatamente como abria, porque arquivo
    /// ausente e tabela sem trilha, nunca erro.
    pub fn abrir(
        diretorio: impl AsRef<Path>,
        nome: &str,
        paginacao: Paginacao,
    ) -> Result<TrilhaFile> {
        // Ver `crate::diario`: o corte do volume e dele, e sem configuracao
        // manda o esquema.
        let paginacao = crate::diario::paginacao(paginacao);
        let volumes = Volumes::novo(&diretorio, nome, EXT_LGPD, paginacao);
        let existentes = volumes.existentes();
        let nasceu = !existentes.is_empty();
        let volume_atual = existentes.last().copied().unwrap_or(1);
        let mut t = TrilhaFile {
            volumes,
            cabs: HashMap::new(),
            volume_atual,
            nasceu,
            usuario: 0,
            ip: String::new(),
        };
        if nasceu {
            t.cab(volume_atual)?;
        }
        Ok(t)
    }

    /// O arquivo ja existe no disco?
    pub fn existe(&self) -> bool {
        self.nasceu
    }

    fn cab(&mut self, volume: u32) -> Result<Cabecalho> {
        if let Some(c) = self.cabs.get(&volume) {
            return Ok(*c);
        }
        let cab = cofre::ler_cabecalho_do_volume(&mut self.volumes, volume, MAGIC_TRILHA)?;
        self.cabs.insert(volume, cab);
        Ok(cab)
    }

    fn gravar_cab(&mut self, cab: Cabecalho) -> Result<()> {
        cofre::gravar_cabecalho_no_volume(&mut self.volumes, &cab, MAGIC_TRILHA)?;
        self.cabs.insert(cab.volume, cab);
        Ok(())
    }

    /// Cria o volume, com a permissao restrita, e devolve o cabecalho novo.
    fn nascer(&mut self, volume: u32) -> Result<Cabecalho> {
        if volume == 1 {
            self.volumes.criar(1)?;
        } else {
            self.volumes.garantir(volume)?;
        }
        apertar_permissao(&self.volumes.caminho(volume));
        let cab = Cabecalho::novo(volume)?;
        self.gravar_cab(cab)?;
        self.nasceu = true;
        self.volume_atual = volume;
        Ok(cab)
    }

    /// Grava uma alteracao de UMA coluna marcada.
    ///
    /// `antes` e `depois` ja chegam prontos de [`valor_para_trilha`]: o
    /// julgamento sobre o que pode virar texto e daquela funcao, e nao deste
    /// arquivo, para que exista UM lugar so decidindo isso.
    #[allow(clippy::too_many_arguments)]
    pub fn registrar_alteracao(
        &mut self,
        rowid: RowId,
        coluna: &str,
        antes: (String, bool),
        depois: (String, bool),
        identidade: &str,
    ) -> Result<Evento> {
        let mut flags = 0u8;
        if antes.1 {
            flags |= FLAG_ANTES_REDIGIDO;
        }
        if depois.1 {
            flags |= FLAG_DEPOIS_REDIGIDO;
        }
        let e = Evento {
            uuid: Uuid::v7(),
            carimbo: agora_ms(),
            tipo: Tipo::Alteracao,
            rowid,
            usuario: self.usuario,
            coluna: cortar(coluna, COLUNA_MAX),
            antes: cortar(&antes.0, VALOR_MAX),
            depois: cortar(&depois.0, VALOR_MAX),
            identidade: cortar(identidade, IDENTIDADE_MAX),
            ip: cortar(&self.ip, IP_MAX),
            linhas: 0,
            flags,
        };
        self.anexar(&e)?;
        Ok(e)
    }

    /// Grava UM registro de acesso, por operacao.
    ///
    /// `criterio` e o que responde "quem viu o prontuario do fulano?": a chave
    /// pedida, o filtro da varredura, o `WHERE` do SQL. Sem ele o registro
    /// diria apenas que alguem leu alguma coisa, que nao e auditoria.
    pub fn registrar_acesso(
        &mut self,
        rowid: RowId,
        colunas: &str,
        criterio: &str,
        linhas: u32,
    ) -> Result<Evento> {
        let e = Evento {
            uuid: Uuid::v7(),
            carimbo: agora_ms(),
            tipo: Tipo::Acesso,
            rowid,
            usuario: self.usuario,
            coluna: cortar(colunas, COLUNA_MAX),
            antes: String::new(),
            depois: String::new(),
            identidade: cortar(criterio, IDENTIDADE_MAX),
            ip: cortar(&self.ip, IP_MAX),
            linhas,
            flags: 0,
        };
        self.anexar(&e)?;
        Ok(e)
    }

    fn anexar(&mut self, e: &Evento) -> Result<()> {
        let paginacao = self.volumes.paginacao();
        let atual = if self.nasceu {
            self.cab(self.volume_atual)?
        } else {
            self.nascer(1)?
        };
        let vazio = atual.fim <= atual.cab_len as u64;
        let ocupa = (REGISTRO_CAB + atual.ocupa(e.texto_len())) as u64;
        let (volume, virou) = paginacao.volume_externo(self.volume_atual, atual.fim, ocupa, vazio);

        let cab = if virou {
            if paginacao.ligada() && volume > paginacao.max_arquivos {
                return Err(PhxError::LimiteExcedido(format!(
                    "a trilha de {} chegou ao teto de {} volumes",
                    self.volumes.nome(),
                    paginacao.max_arquivos
                )));
            }
            self.nascer(volume)?
        } else {
            atual
        };

        // O offset entra no nonce: e ele o numero de ordem que um arquivo
        // append-only nunca reaproveita.
        let bytes = e.escrever(&cab, cab.fim);
        self.volumes.escrever(volume, cab.fim, &bytes)?;
        self.gravar_cab(cab.com(cab.fim + bytes.len() as u64, cab.quantos + 1))
    }

    /// Total de registros em todos os volumes. Zero quando nao ha arquivo.
    pub fn total(&mut self) -> Result<u64> {
        if !self.nasceu {
            return Ok(0);
        }
        let mut t = 0;
        for v in self.volumes.existentes() {
            t += self.cab(v)?.quantos;
        }
        Ok(t)
    }

    /// Le em ordem cronologica. `limite` zero devolve tudo.
    pub fn ler(&mut self, pular: u64, limite: u64) -> Result<Vec<Evento>> {
        let mut saida = Vec::new();
        if !self.nasceu {
            return Ok(saida);
        }
        let mut vistos = 0u64;
        for volume in self.volumes.existentes() {
            let cab = self.cab(volume)?;
            let nome = self.volumes.caminho(volume).display().to_string();
            let mut offset = cab.cab_len as u64;
            while offset + REGISTRO_CAB as u64 <= cab.fim {
                let mut cabecalho = [0u8; REGISTRO_CAB];
                self.volumes.ler(volume, offset, &mut cabecalho)?;
                let n = Evento::ocupa_do_cabecalho(&cab, &cabecalho);
                if offset + n as u64 > cab.fim {
                    return Err(PhxError::Corrompido(format!(
                        "registro de .lgpd em {} passa do fim do volume",
                        self.volumes.caminho(volume).display()
                    )));
                }
                if vistos >= pular {
                    let mut buf = vec![0u8; n];
                    self.volumes.ler(volume, offset, &mut buf)?;
                    saida.push(Evento::ler(&buf, &cab, offset, &nome)?);
                    if limite > 0 && saida.len() as u64 >= limite {
                        return Ok(saida);
                    }
                }
                vistos += 1;
                offset += n as u64;
            }
        }
        Ok(saida)
    }

    /// A trilha de uma linha, em ordem cronologica.
    pub fn de(&mut self, rowid: RowId) -> Result<Vec<Evento>> {
        Ok(self
            .ler(0, 0)?
            .into_iter()
            .filter(|e| e.rowid == rowid)
            .collect())
    }

    /// Confere o CRC de todos os registros e a contagem dos cabecalhos.
    pub fn verificar(&mut self) -> Result<u64> {
        if !self.nasceu {
            return Ok(0);
        }
        let quantos = self.ler(0, 0)?.len() as u64;
        let declarado = self.total()?;
        if quantos != declarado {
            return Err(PhxError::Corrompido(format!(
                "{}: os cabecalhos do .lgpd declaram {declarado} registros, \
                 e o arquivo tem {quantos}",
                self.volumes.nome()
            )));
        }
        Ok(quantos)
    }

    pub fn sincronizar(&mut self) -> Result<()> {
        if !self.nasceu {
            return Ok(());
        }
        self.volumes.sincronizar()
    }

    pub fn fechar_todos(&mut self) {
        self.volumes.fechar_todos();
    }

    pub fn apagar_tudo(&mut self) -> Result<()> {
        if !self.nasceu {
            return Ok(());
        }
        self.nasceu = false;
        self.cabs.clear();
        self.volumes.apagar_tudo()
    }

    pub fn volumes_existentes(&self) -> Vec<u32> {
        self.volumes.existentes()
    }
}

/// Deixa o arquivo legivel so pelo dono.
///
/// O `.lgpd` guarda valor de dado pessoal em claro quando a cifra esta
/// desligada -- que e o padrao. A permissao restrita e a unica protecao que
/// existe nesse caso, e ela e a mesma que o `dblink` ja aplica ao cadastro de
/// ligacoes, pelo mesmo motivo.
///
/// Silencioso de proposito: num sistema de arquivos que nao tem modo Unix (um
/// volume FAT, um compartilhamento de rede), falhar aqui derrubaria a
/// gravacao da trilha por causa de uma protecao que aquele disco nao sabe
/// oferecer -- e ficar sem trilha e pior que ficar sem a permissao.
fn apertar_permissao(caminho: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(caminho, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = caminho;
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    // Pedido 150: guarda de Drop, nao `rm` no fim do corpo.
    fn temp(nome: &str) -> crate::apoio_teste::DirTemp {
        crate::apoio_teste::DirTemp::novo(&format!("trilha-{nome}"))
    }

    fn claro(s: &str) -> (String, bool) {
        (s.to_string(), false)
    }

    #[test]
    fn grava_e_le_de_volta() {
        let d = temp("ida-e-volta");
        let mut t = TrilhaFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        t.usuario = 7;
        t.ip = "192.0.2.10".into();
        t.registrar_alteracao(1, "email", claro("a@x.com"), claro("b@y.com"), "id=42")
            .unwrap();
        t.registrar_acesso(0, "nome,cpf", "cidade=Blumenau", 137)
            .unwrap();

        let lidos = t.ler(0, 0).unwrap();
        assert_eq!(lidos.len(), 2);
        assert_eq!(lidos[0].tipo, Tipo::Alteracao);
        assert_eq!(lidos[0].coluna, "email");
        assert_eq!(lidos[0].antes, "a@x.com");
        assert_eq!(lidos[0].depois, "b@y.com");
        assert_eq!(lidos[0].identidade, "id=42");
        assert_eq!(lidos[0].ip, "192.0.2.10");
        assert_eq!(lidos[0].usuario, 7);
        assert_eq!(lidos[1].tipo, Tipo::Acesso);
        assert_eq!(lidos[1].coluna, "nome,cpf");
        assert_eq!(lidos[1].linhas, 137);
        assert_eq!(t.total().unwrap(), 2);
        assert_eq!(t.verificar().unwrap(), 2);
    }

    /// Registros de tamanhos diferentes um atras do outro: se o avanco do
    /// offset usasse tamanho fixo, o segundo sairia deslocado.
    #[test]
    fn tamanhos_diferentes_seguidos() {
        let d = temp("tamanhos");
        let mut t = TrilhaFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        let valores = ["", "a", "um valor bem mais longo que os outros", "xy"];
        for (i, v) in valores.iter().enumerate() {
            t.ip = "x".repeat(i + 1);
            t.registrar_alteracao(i as u64 + 1, "c", claro(v), claro("z"), "id=1")
                .unwrap();
        }
        let lidos = t.ler(0, 0).unwrap();
        assert_eq!(lidos.len(), valores.len());
        for (i, v) in valores.iter().enumerate() {
            assert_eq!(lidos[i].antes, *v);
            assert_eq!(lidos[i].rowid, i as u64 + 1);
            assert_eq!(lidos[i].ip.len(), i + 1);
        }
    }

    /// Adulterar o valor tem de ser pego. O CRC cobre os textos -- se cobrisse
    /// so o cabecalho, trocar um salario por outro passaria batido.
    #[test]
    fn valor_adulterado_nao_passa() {
        let e = Evento {
            uuid: Uuid::v7(),
            carimbo: 1,
            tipo: Tipo::Alteracao,
            rowid: 1,
            usuario: 1,
            coluna: "salario".into(),
            antes: "1000".into(),
            depois: "2000".into(),
            identidade: "id=1".into(),
            ip: "10.0.0.1".into(),
            linhas: 0,
            flags: 0,
        };
        // Cabecalho em claro: o que este teste prova e o CRC, e ele vale nos
        // dois modos -- a cifra so muda o que esta dentro do corpo.
        let cab = Cabecalho::novo(1).unwrap();
        let mut bytes = e.escrever(&cab, 64);
        assert!(Evento::ler(&bytes, &cab, 64, "t").is_ok());
        // O primeiro byte de `antes`, que vem depois de `coluna`.
        let pos = REGISTRO_CAB + e.coluna.len();
        bytes[pos] = b'9';
        assert!(Evento::ler(&bytes, &cab, 64, "t").is_err());
    }

    #[test]
    fn abrir_nao_cria_arquivo() {
        let d = temp("preguicoso");
        let mut t = TrilhaFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        assert!(!t.existe(), "abrir criou o .lgpd sem ninguem gravar nada");
        assert_eq!(t.total().unwrap(), 0);
        assert!(t.ler(0, 0).unwrap().is_empty());
        assert!(
            std::fs::read_dir(&d).unwrap().next().is_none(),
            "abrir deixou arquivo no diretorio"
        );
        // E, gravando, nasce.
        t.registrar_acesso(1, "cpf", "id=1", 1).unwrap();
        assert!(t.existe());
        assert_eq!(t.total().unwrap(), 1);
    }

    /// O arquivo mais perigoso da tabela nao pode nascer legivel para todos.
    #[cfg(unix)]
    #[test]
    fn nasce_com_permissao_restrita() {
        use std::os::unix::fs::PermissionsExt;
        let d = temp("permissao");
        let mut t = TrilhaFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        t.registrar_alteracao(1, "cpf", claro("a"), claro("b"), "id=1")
            .unwrap();
        let caminho = d.join("t.lgpd");
        let modo = std::fs::metadata(&caminho).unwrap().permissions().mode();
        assert_eq!(
            modo & 0o777,
            0o600,
            "o .lgpd nasceu com {:o}, e nao 0600",
            modo & 0o777
        );
    }

    #[test]
    fn valor_gigante_e_cortado_sem_quebrar_utf8() {
        let d = temp("corte");
        let mut t = TrilhaFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        // "ç" tem 2 bytes: o corte cai no meio dele se for cru.
        let longo = "ç".repeat(VALOR_MAX);
        t.registrar_alteracao(1, "obs", claro(&longo), claro(""), "id=1")
            .unwrap();
        let lidos = t.ler(0, 0).unwrap();
        assert!(lidos[0].antes.len() <= VALOR_MAX);
        assert!(longo.starts_with(&lidos[0].antes));
    }

    #[test]
    fn uuid_do_evento_e_crescente() {
        let d = temp("uuid");
        let mut t = TrilhaFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        for i in 0..50 {
            t.registrar_acesso(i, "cpf", "", 1).unwrap();
        }
        let lidos = t.ler(0, 0).unwrap();
        for par in lidos.windows(2) {
            assert!(
                par[0].uuid.bytes() < par[1].uuid.bytes(),
                "o v7 do evento saiu fora de ordem"
            );
        }
    }

    // ------------------------------------------------------------- redacao

    #[test]
    fn coluna_de_senha_nao_entrega_o_valor() {
        for nome in ["senha", "SENHA", "senha_hash", "user_password", "api_token"] {
            let (texto, redigido) = valor_para_trilha(nome, &Value::Str("batatafrita123".into()));
            assert!(redigido, "{nome} nao foi redigida");
            assert!(
                !texto.contains("batatafrita"),
                "{nome} deixou a senha no texto: {texto}"
            );
            assert!(texto.contains("bytes"), "{nome}: {texto}");
        }
    }

    /// O hash e pego pela ANALISE, e nao pelo nome: uma coluna chamada
    /// `observacao` que guarde um hash nao pode entregar o hash.
    #[test]
    fn hash_em_coluna_de_nome_inocente_e_redigido() {
        let hash = phxsql_core::senha::cifrar_com("segredo", 10_000);
        let (texto, redigido) = valor_para_trilha("observacao", &Value::Str(hash.clone()));
        assert!(redigido, "o hash passou por uma coluna de nome inocente");
        assert!(!texto.contains("pbkdf2"), "o hash vazou: {texto}");
        assert!(!texto.contains(&hash));
    }

    #[test]
    fn valor_comum_passa_inteiro() {
        let (texto, redigido) = valor_para_trilha("email", &Value::Str("ana@x.com".into()));
        assert!(!redigido);
        assert_eq!(texto, "ana@x.com");
    }

    /// Binario nao vira texto: vira tamanho. Uma biometria e exatamente o dado
    /// que nao pode ser concentrado na trilha.
    #[test]
    fn binario_vira_tamanho() {
        let (texto, _) = valor_para_trilha("foto", &Value::Bin(vec![7u8; 4096]));
        assert_eq!(texto, "4096 bytes");
    }

    /// Nulo continua distinguivel de vazio redigido: a trilha precisa mostrar
    /// que alguem ESVAZIOU o campo.
    #[test]
    fn nulo_nao_e_segredo() {
        let (texto, redigido) = valor_para_trilha("senha", &Value::Null);
        assert!(!redigido);
        assert!(texto.is_empty());
    }
}

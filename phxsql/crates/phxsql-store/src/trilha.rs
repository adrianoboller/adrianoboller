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
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

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
/// Bit 2 das flags: o valor de `antes` NAO pode ser lido, e o texto e a marca
/// disso -- nao o valor, nem um vazio que passaria por «campo em branco».
///
/// # Por que existe, e por que nao ha irmao para `depois`
///
/// A trilha so pode gravar o que consegue afirmar. Um `Memo` marcado cujo
/// bloco do `.memo` nao abre (CRC estragado, volume perdido) nao tem valor
/// velho a gravar -- e gravar `""` ali afirmaria que o campo estava em branco,
/// que e uma frase falsa sobre o dado. Registro de auditoria que afirma um
/// fato falso e pior que registro ausente.
///
/// `depois` nao ganha o mesmo bit porque nao ha por onde: ele e o valor que o
/// chamador tem na mao ao gravar a linha, nunca uma leitura de disco. Bit
/// reservado que nada liga e bit que envelhece calado.
pub const FLAG_ANTES_INDISPONIVEL: u8 = 4;

/// O texto que acompanha [`FLAG_ANTES_INDISPONIVEL`].
///
/// O bit e que decide; este texto e para quem le o arquivo com o olho. Quem
/// decide pela frase quebra calado no dia em que alguem melhorar a redacao --
/// e por isso [`Evento::antes_indisponivel`] le o bit, nunca isto.
pub const INDISPONIVEL: &str = "(indisponivel: o valor anterior nao pode ser lido)";

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

    /// O valor de antes nao pode ser lido? Ver [`FLAG_ANTES_INDISPONIVEL`].
    pub fn antes_indisponivel(&self) -> bool {
        self.flags & FLAG_ANTES_INDISPONIVEL != 0
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

        let crc = crc_do_registro(&buf);
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
        if crc_do_registro(&src[..total]) != c.u32(52) {
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

/// O CRC de um registro inteiro (`reg` vai do cabecalho ao fim do corpo).
///
/// Cobre o cabecalho SEM o proprio campo, e o corpo COMO ELE VAI AO DISCO: um
/// valor adulterado tem de ser pego como qualquer outro dado, e a varredura
/// confere o arquivo sem precisar da chave.
///
/// UMA funcao para quem grava, quem le e quem varre para o expurgo: tres
/// copias da mesma conta seriam tres lugares para ela divergir, e a que
/// divergisse no expurgo derrubaria volume com registro que nao confere.
fn crc_do_registro(reg: &[u8]) -> u32 {
    let crc = crc32(&reg[..52]);
    phxsql_core::crc::crc32_with(crc, &reg[REGISTRO_CAB..])
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

/// Onde o volume ativo fecha por tamanho, se ninguem configurar: 64 MiB.
///
/// Decisao do papel C (parecer do pedido 368, formato B). A trilha deixou de
/// seguir o corte do `.reg` e dos diarios: o volume dela e a unidade do
/// EXPURGO, e unidade de expurgo de 1 GiB -- o padrao do `bytes_por_arquivo`
/// -- levaria decadas para fechar numa tabela de pouco movimento.
pub const CORTE_PADRAO_BYTES: u64 = 64 * 1024 * 1024;
/// A idade do primeiro registro a partir da qual o ativo fecha na passada.
pub const CORTE_PADRAO_DIAS: u32 = 30;
/// Piso do corte por tamanho -- o mesmo dos diarios, pelo mesmo motivo: abaixo
/// disto o volume viraria um arquivo por registro.
pub const CORTE_MINIMO_BYTES: u64 = crate::diario::CORTE_MINIMO;

/// O corte por tamanho vigente, em bytes. Ver [`definir_corte`].
static CORTE_BYTES: AtomicU64 = AtomicU64::new(CORTE_PADRAO_BYTES);
/// O corte por idade vigente, em dias. Ver [`definir_corte`].
static CORTE_DIAS: AtomicU32 = AtomicU32::new(CORTE_PADRAO_DIAS);

/// Onde o volume ativo da trilha fecha: `bytes` por tamanho (subido ao piso) e
/// `dias` de idade do primeiro registro. Vale para as trilhas abertas daqui
/// para a frente; e chamado no arranque, pelo `config.json`
/// (`lgpd.volume_mib`, `lgpd.volume_dias`).
///
/// Um global do processo, e nao um campo do esquema, pelo mesmo motivo do
/// corte dos diarios (`crate::diario`): e uma decisao de como este servidor
/// rola os arquivos dele, e nao do dado -- e no esquema seria uma versao nova
/// do `PSCH` para decidir isso.
pub fn definir_corte(bytes: u64, dias: u32) {
    CORTE_BYTES.store(bytes.max(CORTE_MINIMO_BYTES), Ordering::Relaxed);
    CORTE_DIAS.store(dias.max(1), Ordering::Relaxed);
}

/// O corte vigente: `(bytes, dias)`.
pub fn corte() -> (u64, u32) {
    (
        CORTE_BYTES.load(Ordering::Relaxed),
        CORTE_DIAS.load(Ordering::Relaxed),
    )
}

/// Numero de volume que nao existe: o ativo e lido com ele antes de se saber
/// qual e o dele. Volumes da trilha comecam em 1.
const SONDA: u32 = 0;

/// O arquivo em `caminho` e mais CURTO que o cabecalho que ele teria?
///
/// Cabecalho em claro tem [`cofre::CAB_V2`] bytes; cifrado, [`cofre::CAB_V3`],
/// e a versao nos bytes 8..10 diz qual. Menor que 64 nao tem cabecalho
/// nenhum; de 64 a 127 com versao 3 e o cifrado cortado. Os dois so saem de
/// um nascimento que a queda interrompeu: o cabecalho e escrito de uma vez,
/// logo depois do `create_new`, e nenhum registro vem antes dele.
fn nascimento_interrompido(caminho: &Path) -> Result<bool> {
    let tamanho = std::fs::metadata(caminho)?.len();
    if tamanho < cofre::CAB_V2 as u64 {
        return Ok(true);
    }
    if tamanho >= cofre::CAB_V3 as u64 {
        return Ok(false);
    }
    let mut inicio = [0u8; 10];
    std::io::Read::read_exact(&mut std::fs::File::open(caminho)?, &mut inicio)?;
    Ok(u16::from_le_bytes([inicio[8], inicio[9]]) >= 3)
}

/// O `.lgpd` de uma tabela.
///
/// # Os nomes (pedido 368, formato B)
///
/// O volume **ativo** -- o unico que recebe escrita -- e sempre
/// `<tabela>.lgpd`, de nome fixo. Os **fechados** sao `<tabela>_NNN.lgpd`, com
/// NNN igual ao `volume` do cabecalho: no minimo 3 digitos, sem teto, nunca
/// reusado. Fechar e RENOMEAR o ativo para `_NNN` e fazer nascer o ativo
/// `NNN+1`; nenhum byte de registro ou de cabecalho muda com isso.
///
/// A trilha e independente da paginacao do `.reg`: nao segue
/// `registros_por_arquivo`, `max_arquivos` nem `diario_volume_mib`. Antes ela
/// seguia, e dali vinham dois defeitos -- a tabela sem paginacao tinha a
/// trilha num arquivo so, que nunca fechava e portanto nunca se expurgava; e a
/// paginada herdava o `max_arquivos` da tabela como teto, e no teto a
/// `atualizar` gravava a linha e falhava na trilha (achado A1 do parecer do
/// DBA).
///
/// # Quando o ativo fecha
///
/// - por **tamanho**, no append: o registro que nao cabe em [`corte`] faz o
///   ativo fechar antes de ele entrar -- uma comparacao, e nada mais, no
///   caminho quente;
/// - por **idade**, fora do laco quente: [`TrilhaFile::fechar_se_velho`], na
///   passada da retencao e na op do administrador;
/// - por **pedido** do administrador: [`TrilhaFile::fechar_ativo`].
///
/// # Como se acha o que existe
///
/// Abrir custa UM `stat` (`<tabela>.lgpd` existe?) e, se existe, a leitura do
/// cabecalho, que diz o numero N do ativo. Ler, contar e expurgar andam do
/// ativo para BAIXO ate faltar -- o expurgo tira sempre o comeco, entao os
/// fechados sao contiguos logo abaixo dele. Sem o ativo no disco (uma queda
/// entre o `rename` e o nascimento do seguinte, ou uma trilha gravada antes
/// do formato B numa tabela paginada), o numero sai de UMA listagem do
/// diretorio: o maior fechado + 1. E a mesma regra nos dois casos, e e a
/// regra pela qual o ativo nasce.
///
/// # A migracao, sem reescrita
///
/// - trilha de arquivo unico gravada antes: ja e `<tabela>.lgpd`, volume 1 --
///   vira o ativo como esta;
/// - trilha paginada `_001` a `_K`, sem `<tabela>.lgpd`: os K viram fechados,
///   e o ativo nasce `K+1` no primeiro evento. Sufixo de largura diferente de
///   3 (`_0001`) continua no nome em que nasceu (ver `largura_legada`).
///
/// # Por que este nasce preguicoso, e os outros tres nao
///
/// `LogFile`, `LixeiraFile` e `MotivoFile` criam o arquivo na hora em que a
/// tabela e criada, porque toda tabela tem eventos, exclusoes e motivos --
/// mais cedo ou mais tarde. A trilha e o contrario: a maioria das tabelas de
/// um banco nao tem UMA coluna marcada, e para essas o arquivo nunca teria
/// nada dentro. Aqui o arquivo so aparece quando o primeiro evento aparece, e
/// a presenca dele ja e a resposta a «esta tabela tem dado pessoal?».
pub struct TrilhaFile {
    volumes: Volumes,
    cabs: HashMap<u32, Cabecalho>,
    /// O numero do volume ativo. Vale quando `resolvido`.
    ativo: u32,
    /// `<tabela>.lgpd` existe no disco, com cabecalho? O de nascimento
    /// interrompido (ver `interrompido`) conta como ausente.
    ativo_existe: bool,
    /// `<tabela>.lgpd` existe e e MENOR que o cabecalho que ele teria: a
    /// queda pegou o nascimento entre o `create_new` e o cabecalho no disco.
    /// Por construcao nao tem registro; o proximo nascimento o substitui.
    interrompido: bool,
    /// O numero do ativo ja e conhecido -- lido do cabecalho, ou achado pela
    /// listagem do diretorio?
    resolvido: bool,
    /// A largura do sufixo que a trilha usava ANTES do formato B, quando ela
    /// nao era 3 (`_0001`). `None` na tabela sem paginacao e na de 3 digitos,
    /// cujos nomes antigos ja sao os nomes novos.
    largura_legada: Option<u8>,
    /// Os volumes legados ja foram procurados nesta instancia?
    legado_conferido: bool,
    /// O corte por tamanho desta instancia, tirado de [`corte`] na abertura.
    corte_bytes: u64,
    /// O corte por idade desta instancia, em dias.
    corte_dias: u32,
    /// Usuario aplicado aos registros gravados daqui em diante.
    pub usuario: u32,
    /// IP aplicado aos registros gravados daqui em diante.
    pub ip: String,
}

impl TrilhaFile {
    /// Abre sem tocar no disco quando o arquivo nao existe.
    ///
    /// **Nao cria**, ao contrario do `.reason`. Tabela sem coluna marcada
    /// nunca chega a `registrar`, entao nunca ganha arquivo -- e arquivo
    /// ausente e tabela sem trilha, nunca erro.
    ///
    /// `paginacao` e a da tabela, e so serve para uma coisa: saber a largura
    /// do sufixo em que uma trilha gravada ANTES do formato B pode estar. O
    /// corte da trilha nao sai dela.
    pub fn abrir(
        diretorio: impl AsRef<Path>,
        nome: &str,
        paginacao: Paginacao,
    ) -> Result<TrilhaFile> {
        let largura_legada = (paginacao.ligada()
            && paginacao.digitos != phxsql_core::paginacao::DIGITOS_PADRAO)
            .then_some(paginacao.digitos);
        let (corte_bytes, corte_dias) = corte();
        let volumes = Volumes::novo_da_trilha(&diretorio, nome, EXT_LGPD, SONDA);
        // O ativo mais curto que o cabecalho e nascimento interrompido (papel
        // C, B2): tratado como AUSENTE, cai na mesma regra da queda entre o
        // `rename` e o nascimento -- o numero sai da listagem, maior fechado
        // + 1, que e o N que ele ia ter. Sem isto a tabela inteira nao abria
        // («failed to fill whole buffer»), leitura inclusive.
        let interrompido =
            volumes.existe(SONDA) && nascimento_interrompido(&volumes.caminho(SONDA))?;
        let ativo_existe = volumes.existe(SONDA) && !interrompido;
        let mut t = TrilhaFile {
            volumes,
            cabs: HashMap::new(),
            ativo: 0,
            ativo_existe,
            interrompido,
            resolvido: false,
            largura_legada,
            legado_conferido: false,
            corte_bytes,
            corte_dias,
            usuario: 0,
            ip: String::new(),
        };
        if ativo_existe {
            let cab = cofre::ler_cabecalho_do_volume(&mut t.volumes, SONDA, MAGIC_TRILHA)?;
            // O descritor aberto com o numero de sonda sai: dali em diante o
            // ativo e chamado pelo numero dele.
            t.volumes.fechar_todos();
            if cab.volume == SONDA {
                return Err(PhxError::Corrompido(format!(
                    "{}: o cabecalho do volume ativo diz volume 0",
                    t.volumes.caminho(SONDA).display()
                )));
            }
            t.ativo = cab.volume;
            t.resolvido = true;
            t.volumes.definir_ativo_da_trilha(cab.volume);
            t.cabs.insert(cab.volume, cab);
        }
        Ok(t)
    }

    /// A trilha tem algum arquivo no disco -- o ativo, ou um fechado?
    ///
    /// Sem o ativo, a resposta sai de uma listagem do diretorio, e nao de
    /// cache: e o caso raro (a queda entre o `rename` e o nascimento do
    /// seguinte, ou a trilha paginada de antes do formato B).
    pub fn existe(&self) -> bool {
        self.ativo_existe || self.maior_fechado_no_disco().is_ok_and(|m| m > 0)
    }

    /// O maior numero de volume fechado no diretorio, pelo NOME
    /// (`<tabela>_<digitos>.lgpd`, de qualquer largura). Zero se nao ha.
    fn maior_fechado_no_disco(&self) -> Result<u32> {
        let prefixo = format!("{}_", self.volumes.nome());
        let fim = format!(".{EXT_LGPD}");
        let mut maior = 0u32;
        let entradas = match std::fs::read_dir(self.volumes.diretorio()) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e.into()),
        };
        for entrada in entradas.flatten() {
            let nome = entrada.file_name();
            let Some(nome) = nome.to_str() else {
                continue;
            };
            let Some(meio) = nome
                .strip_prefix(prefixo.as_str())
                .and_then(|r| r.strip_suffix(fim.as_str()))
            else {
                continue;
            };
            if meio.is_empty() || !meio.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            if let Ok(n) = meio.parse::<u32>() {
                maior = maior.max(n);
            }
        }
        Ok(maior)
    }

    /// Descobre o numero do ativo, quando ele ainda nao e conhecido, e o nome
    /// dos volumes gravados antes do formato B.
    ///
    /// Sem o ativo no disco, o numero dele e o maior fechado + 1, por UMA
    /// listagem do diretorio -- uma vez na vida deste ativo: depois dela o
    /// numero fica nesta instancia, e o ativo que nascer o grava no cabecalho.
    fn resolver(&mut self) -> Result<()> {
        if !self.resolvido {
            let maior = self.maior_fechado_no_disco()?;
            self.ativo = maior + 1;
            self.volumes.definir_ativo_da_trilha(self.ativo);
            self.resolvido = true;
        }
        if !self.legado_conferido {
            self.legado_conferido = true;
            if let Some(largura) = self.largura_legada {
                self.procurar_legado(largura);
            }
        }
        Ok(())
    }

    /// Anda do ativo para baixo pelos nomes canonicos; o primeiro numero que
    /// falta no nome canonico e existe no nome antigo (`_0001`) e o mais alto
    /// dos volumes de antes do formato B -- abaixo dele, todos estao no nome
    /// antigo, porque o formato B nunca cria nome com outra largura.
    fn procurar_legado(&mut self, largura: u8) {
        let mut n = self.ativo;
        while n > 1 {
            n -= 1;
            if self.volumes.caminho_fechado_da_trilha(n).exists() {
                continue;
            }
            if self.volumes.caminho_legado_da_trilha(n, largura).exists() {
                self.volumes.definir_legado_da_trilha(largura, n);
            }
            return;
        }
    }

    fn cab(&mut self, volume: u32) -> Result<Cabecalho> {
        if let Some(c) = self.cabs.get(&volume) {
            return Ok(*c);
        }
        let cab = cofre::ler_cabecalho_do_volume(&mut self.volumes, volume, MAGIC_TRILHA)?;
        // O numero do nome E o do cabecalho: e o que o rastro e o bilhete
        // citam, e arquivo que diz outro numero nao e deste volume.
        //
        // Isto NAO separa a trilha de `x` da de uma tabela `x_001`: o ativo
        // dela e `x_001.lgpd` com volume 1 no cabecalho -- nome e numero do
        // fechado 1 de `x` --, e o expurgo de `x` o apagava (papel C, P2a).
        // Quem separa e a declaracao: o catalogo recusa criar tabela com nome
        // que ele leria como volume (`exigir_nome_que_volta`).
        if cab.volume != volume {
            return Err(PhxError::Corrompido(format!(
                "{}: o cabecalho diz volume {}, e o nome diz {volume}",
                self.volumes.caminho(volume).display(),
                cab.volume
            )));
        }
        self.cabs.insert(volume, cab);
        Ok(cab)
    }

    fn gravar_cab(&mut self, cab: Cabecalho) -> Result<()> {
        cofre::gravar_cabecalho_no_volume(&mut self.volumes, &cab, MAGIC_TRILHA)?;
        self.cabs.insert(cab.volume, cab);
        Ok(())
    }

    /// Faz nascer o ativo, com o numero que a resolucao deu, e a permissao
    /// restrita. `criar` recusa se o nome fixo ja existe: nascer nunca
    /// sobrescreve um ativo.
    fn nascer_ativo(&mut self) -> Result<Cabecalho> {
        self.resolver()?;
        if self.interrompido {
            // Confere de novo antes de apagar: so sai o arquivo que AINDA e
            // mais curto que um cabecalho -- o que tem registro nunca e este.
            let caminho = self.volumes.caminho(self.ativo);
            if !nascimento_interrompido(&caminho)? {
                return Err(PhxError::Corrompido(format!(
                    "{}: era nascimento interrompido na abertura e deixou de ser",
                    caminho.display()
                )));
            }
            self.volumes.apagar_volume(self.ativo)?;
            self.interrompido = false;
        }
        self.volumes.criar(self.ativo)?;
        apertar_permissao(&self.volumes.caminho(self.ativo));
        let cab = Cabecalho::novo(self.ativo)?;
        self.gravar_cab(cab)?;
        self.ativo_existe = true;
        Ok(cab)
    }

    /// Fecha o volume ativo: ele passa a `<tabela>_NNN.lgpd`, e o ativo NNN+1
    /// nasce no lugar. Devolve o numero que fechou; `None` quando nao ha ativo
    /// ou ele esta vazio -- fechar volume sem registro so criaria arquivo.
    ///
    /// E o fechamento pedido pelo administrador, e o que o tamanho e a idade
    /// chamam.
    pub fn fechar_ativo(&mut self) -> Result<Option<u32>> {
        if !self.ativo_existe {
            return Ok(None);
        }
        if self.cab(self.ativo)?.quantos == 0 {
            return Ok(None);
        }
        let fechado = self.volumes.fechar_ativo_da_trilha()?;
        self.ativo = fechado + 1;
        self.ativo_existe = false;
        self.nascer_ativo()?;
        Ok(Some(fechado))
    }

    /// Fecha o ativo se o PRIMEIRO registro dele e mais velho que o corte por
    /// idade (`lgpd.volume_dias`), contado a partir de `agora`.
    ///
    /// E o que faz a tabela de pouco movimento fechar volume -- e so o volume
    /// fechado se expurga. Roda fora do laco quente: na passada diaria da
    /// retencao e na op do administrador, nunca no `registrar`.
    pub fn fechar_se_velho(&mut self, agora: i64) -> Result<Option<u32>> {
        if !self.ativo_existe {
            return Ok(None);
        }
        let Some(primeiro) = self.primeiro_carimbo(self.ativo)? else {
            return Ok(None);
        };
        if agora - primeiro <= self.corte_dias as i64 * 86_400_000 {
            return Ok(None);
        }
        self.fechar_ativo()
    }

    /// Grava uma alteracao de UMA coluna marcada.
    ///
    /// `antes` e `depois` ja chegam prontos de [`valor_para_trilha`]: o
    /// julgamento sobre o que pode virar texto e daquela funcao, e nao deste
    /// arquivo, para que exista UM lugar so decidindo isso.
    ///
    /// # Por que `antes` e `Option` e `depois` nao
    ///
    /// `None` quer dizer **o valor velho nao pode ser lido**, e o registro sai
    /// com [`FLAG_ANTES_INDISPONIVEL`] em vez de um `""` que afirmaria que o
    /// campo estava em branco.
    ///
    /// A assimetria e a verdade do caminho, e nao descuido: `antes` pode vir
    /// de um bloco do `.memo`/`.bin` que nao abre; `depois` e o valor que o
    /// chamador tem na mao ao gravar a linha, e nao ha de onde faltar. Um
    /// `Option` nos dois lados criaria um estado que ninguem sabe produzir --
    /// e estado que ninguem produz e estado que ninguem prova.
    #[allow(clippy::too_many_arguments)]
    pub fn registrar_alteracao(
        &mut self,
        rowid: RowId,
        coluna: &str,
        antes: Option<(String, bool)>,
        depois: (String, bool),
        identidade: &str,
    ) -> Result<Evento> {
        let mut flags = 0u8;
        let antes = match antes {
            Some((texto, redigido)) => {
                if redigido {
                    flags |= FLAG_ANTES_REDIGIDO;
                }
                cortar(&texto, VALOR_MAX)
            }
            None => {
                flags |= FLAG_ANTES_INDISPONIVEL;
                INDISPONIVEL.to_string()
            }
        };
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
            antes,
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

    /// Acrescenta um registro no ativo -- e, se ele nao cabe mais, fecha o
    /// ativo antes. Sem teto de volumes: o numero do volume da trilha nao tem
    /// fim, e e isso que tirou o `LimiteExcedido` do caminho da `atualizar`.
    ///
    /// Um registro maior que o corte inteiro fica sozinho no volume dele, em
    /// vez de recusado: o ativo VAZIO nao fecha.
    fn anexar(&mut self, e: &Evento) -> Result<()> {
        let mut cab = if self.ativo_existe {
            self.cab(self.ativo)?
        } else {
            self.nascer_ativo()?
        };
        let ocupa = (REGISTRO_CAB + cab.ocupa(e.texto_len())) as u64;
        let vazio = cab.fim <= cab.cab_len as u64;
        if !vazio && cab.fim + ocupa > self.corte_bytes {
            self.fechar_ativo()?;
            cab = self.cab(self.ativo)?;
        }
        // O offset entra no nonce: e ele o numero de ordem que um arquivo
        // append-only nunca reaproveita.
        let bytes = e.escrever(&cab, cab.fim);
        self.volumes.escrever(self.ativo, cab.fim, &bytes)?;
        self.gravar_cab(cab.com(cab.fim + bytes.len() as u64, cab.quantos + 1))
    }

    /// Os volumes que existem, do mais velho ao ativo.
    ///
    /// O ativo de nascimento interrompido tem o nome no diretorio e nenhum
    /// cabecalho: a listagem o acha, e ele sai daqui.
    fn volumes_vivos(&mut self) -> Result<Vec<u32>> {
        self.resolver()?;
        let mut v = self.volumes.existentes();
        if !self.ativo_existe {
            let ativo = self.ativo;
            v.retain(|n| *n != ativo);
        }
        Ok(v)
    }

    /// Total de registros em todos os volumes. Zero quando nao ha arquivo.
    pub fn total(&mut self) -> Result<u64> {
        let mut t = 0;
        for v in self.volumes_vivos()? {
            t += self.cab(v)?.quantos;
        }
        Ok(t)
    }

    /// Le em ordem cronologica. `limite` zero devolve tudo.
    pub fn ler(&mut self, pular: u64, limite: u64) -> Result<Vec<Evento>> {
        let mut saida = Vec::new();
        let mut vistos = 0u64;
        for volume in self.volumes_vivos()? {
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
        if !self.ativo_existe && !self.resolvido {
            return Ok(());
        }
        self.volumes.sincronizar()
    }

    /// Quantos arquivos o `.lgpd` ja mandou ao disco de verdade. Ver
    /// `Volumes::sincronizados` -- conta o ARQUIVO, e nao a chamada.
    pub fn sincronizados(&self) -> u64 {
        self.volumes.sincronizados()
    }

    pub fn fechar_todos(&mut self) {
        self.volumes.fechar_todos();
    }

    pub fn apagar_tudo(&mut self) -> Result<()> {
        if self.volumes_vivos()?.is_empty() {
            return Ok(());
        }
        self.volumes.apagar_tudo()?;
        self.ativo_existe = false;
        self.interrompido = false;
        self.resolvido = false;
        self.legado_conferido = false;
        self.cabs.clear();
        Ok(())
    }

    /// Os volumes que existem no disco, do mais velho ao ativo.
    pub fn volumes_existentes(&mut self) -> Result<Vec<u32>> {
        self.volumes_vivos()
    }
}

// ------------------------------------------------------------------ expurgo

/// Quanto a varredura do expurgo pede ao disco de uma vez.
///
/// A varredura anda pelo volume de cabecalho em cabecalho, e o `ler` desta
/// trilha pede dois `read` por registro -- num volume de 1 GiB, milhoes de
/// chamadas de sistema com a trava na mao. Um bloco grande troca isso por uma
/// leitura por bloco; o registro que atravessa a borda do bloco faz o bloco
/// recomecar nele. O custo esta medido em `docs/LGPD.md` §10.
const BLOCO_DO_EXPURGO: usize = 256 * 1024;

/// Um volume que o expurgo derruba INTEIRO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeVencido {
    pub volume: u32,
    /// Registros que saem com ele -- o `quantos` do cabecalho, conferido
    /// registro a registro pela varredura.
    pub registros: u64,
    /// O carimbo do registro MAIS NOVO do volume. E ele, e so ele, que decide
    /// se o volume sai. `None` so num volume fechado sem registro nenhum.
    pub mais_novo: Option<i64>,
    /// Bytes do volume ate o fim logico.
    pub bytes: u64,
    /// O bilhete: o UUID do primeiro registro e o fim logico, relidos do disco
    /// antes do `unlink`. Ver [`TrilhaFile::apagar_expurgados`].
    bilhete: ([u8; 16], u64),
}

/// Onde o expurgo parou, e por que.
///
/// Existe porque «nao apaguei nada» tem tres causas, e duas delas podem
/// deixar dado vencido no disco. Quem pediu o expurgo tem de saber qual -- relatorio
/// que diz so o que fez esconde o que nao fez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parada {
    /// A tabela nao tem `.lgpd`: nunca houve evento de dado pessoal nela.
    SemTrilha,
    /// Chegou ao volume que ainda recebe escrita. O que esta nele sai depois
    /// que ele fechar -- por tamanho, por idade ou por pedido.
    VolumeAtivo,
    /// Um volume fechado tem registro de DEPOIS do limite: ele fica inteiro, e
    /// os seguintes tambem, para a trilha que sobra nao ter buraco no meio.
    Fronteira,
}

impl Parada {
    /// A chave da parada no protocolo. Quem le decide por ela, nunca por
    /// frase.
    pub fn nome(self) -> &'static str {
        match self {
            Parada::SemTrilha => "sem_trilha",
            Parada::VolumeAtivo => "volume_ativo",
            Parada::Fronteira => "fronteira",
        }
    }
}

/// O que o expurgo decidiu -- antes de derrubar qualquer coisa.
///
/// # As tres fases, e por que o tipo muda entre elas
///
/// 1. **planejar e gravar o rastro** (`Table::preparar_expurgo_da_trilha`):
///    le, decide, e grava no `.reason` quem pediu, quando e o que vai sair;
/// 2. **levar o rastro ao disco** ([`Expurgo::selar`]): e so ele que devolve
///    um [`ExpurgoSelado`];
/// 3. **derrubar os volumes** (`Table::concluir_expurgo_da_trilha`), que so
///    aceita um [`ExpurgoSelado`].
///
/// A ordem e a do `esvaziar_lixeira` -- o motivo tem de sobreviver ao dado --,
/// e aqui ela e TIPO, e nao comentario: nao ha como chamar a fase 3 sem ter
/// passado pela 2. As fases sao separadas porque o servidor faz a 2 fora da
/// trava global: o `fsync` do rastro nao precisa segurar o servidor inteiro.
#[derive(Debug, Clone)]
pub struct Expurgo {
    /// O limite pedido: sai o volume cujo registro mais novo e ANTERIOR a ele.
    pub limite: i64,
    /// O que sai, em ordem crescente de volume -- sempre um PREFIXO da trilha.
    pub volumes: Vec<VolumeVencido>,
    pub parada: Parada,
    /// O volume em que a varredura parou (o ativo ou o de fronteira). Zero
    /// em [`Parada::SemTrilha`].
    pub parou_no_volume: u32,
    /// O carimbo do primeiro registro do volume em que parou, QUANDO ele e
    /// anterior ao limite: dado vencido que ficou porque mora num volume que
    /// nao pode sair inteiro. `None` quando o que ficou esta todo no prazo.
    pub retido_vencido_desde: Option<i64>,
    /// Onde o rastro foi gravado. `None` enquanto nao ha rastro -- e nao ha
    /// quando nao ha volume a derrubar.
    pub(crate) rastro: Option<crate::motivo::VolumeDoRegistro>,
    /// O ativo que o plano fez nascer porque faltava. O selo o leva ao disco
    /// antes do primeiro `unlink`: e o cabecalho dele que guarda o numero
    /// depois que os fechados sairem.
    pub(crate) ativo_nascido: Option<AtivoNascido>,
}

/// O ativo que o plano do expurgo fez nascer. Ver [`Expurgo::ativo_nascido`].
#[derive(Debug, Clone)]
pub(crate) struct AtivoNascido {
    diretorio: std::path::PathBuf,
    nome: String,
    volume: u32,
}

impl Expurgo {
    /// Registros que saem, somados.
    pub fn registros(&self) -> u64 {
        self.volumes.iter().map(|v| v.registros).sum()
    }

    /// O texto do rastro no `.reason` -- o campo `identidade` do registro de
    /// expurgo.
    ///
    /// # O que ele carrega, e o que NAO carrega
    ///
    /// Quais volumes, quantos registros, o carimbo do mais novo que saiu e o
    /// limite pedido. **Nenhuma chave de linha**: a trilha guarda a chave
    /// primaria em texto em cada registro, e o rastro de que ela foi apagada
    /// nao pode ser o lugar onde ela sobrevive. Quem separa este expurgo do da
    /// lixeira e o bit `motivo::FLAG_EXPURGO_DA_TRILHA` do registro, e nao
    /// este texto: ele e para quem le, e decidir pela frase e o que a casa
    /// proibe.
    ///
    /// # O rastro e a INTENCAO selada; quem manda e o diretorio
    ///
    /// Ele vai ao disco antes do primeiro `unlink`, entao pode sobrar rastro
    /// sem apagamento: uma queda entre as fases, uma recusa da fase 3 (o
    /// bilhete que nao confere), ou um `unlink` que falha no meio -- e ai o
    /// erro devolve a lista do que saiu. O rastro diz o que foi pedido; o que
    /// saiu de fato e o que falta no diretorio.
    pub fn rastro(&self) -> String {
        let mais_novo = self
            .volumes
            .iter()
            .filter_map(|v| v.mais_novo)
            .max()
            .map(phxsql_core::datahora::instante_iso)
            .unwrap_or_else(|| "-".into());
        format!(
            ".lgpd volumes {} ({} volume(s), {} registro(s)); mais novo {}; limite {}",
            faixas(self.volumes.iter().map(|v| v.volume)),
            self.volumes.len(),
            self.registros(),
            mais_novo,
            phxsql_core::datahora::instante_iso(self.limite)
        )
    }

    /// Fase 2: leva o rastro ao disco. E a UNICA porta para a fase 3.
    ///
    /// Sem volume a derrubar nao ha rastro, e selar e so trocar de tipo. Com
    /// volume e sem rastro gravado, recusa: seria apagar dado sem deixar o
    /// «quem, quando e por que» -- exatamente o que o `esvaziar_lixeira`
    /// existe para nunca fazer.
    pub fn selar(self) -> Result<ExpurgoSelado> {
        let mut sincronizados = 0;
        if !self.volumes.is_empty() {
            let Some(onde) = &self.rastro else {
                return Err(PhxError::Esquema(
                    "expurgo da trilha sem rastro gravado no .reason: nada sai sem o \
                     registro de quem pediu e por que"
                        .into(),
                ));
            };
            sincronizados = onde.sincronizar()?;
            // Por um descritor proprio, como o rastro: fora da trava global,
            // sem consumir as marcas de escrita do processo.
            if let Some(a) = &self.ativo_nascido {
                let mut v = Volumes::novo_da_trilha(&a.diretorio, &a.nome, EXT_LGPD, a.volume);
                v.sincronizar_volume(a.volume)?;
                sincronizados += v.sincronizados();
            }
        }
        Ok(ExpurgoSelado {
            expurgo: self,
            sincronizados,
        })
    }
}

/// Um [`Expurgo`] cujo rastro ja esta no disco. So ele abre a fase 3.
#[derive(Debug, Clone)]
pub struct ExpurgoSelado {
    expurgo: Expurgo,
    sincronizados: u64,
}

impl ExpurgoSelado {
    pub fn expurgo(&self) -> &Expurgo {
        &self.expurgo
    }

    /// Quantos arquivos o selo levou ao disco: 0 sem volume a derrubar; com
    /// volume, 1 (o rastro) -- e 2 quando o plano fez nascer o ativo que
    /// faltava, que vai ao disco junto (o rastro e o ativo nascido).
    /// E o numero que prova que a fase 2 foi ao disco, e nao so trocou de tipo.
    pub fn sincronizados(&self) -> u64 {
        self.sincronizados
    }

    pub fn em_expurgo(self) -> Expurgo {
        self.expurgo
    }
}

/// `1,2,3,7` vira `1-3,7`. O numero de volumes cresce sem teto numa trilha
/// velha, e o rastro cabe em 512 bytes; a faixa e o que o mantem legivel.
fn faixas(volumes: impl Iterator<Item = u32>) -> String {
    let mut saida: Vec<String> = Vec::new();
    let mut corrente: Option<(u32, u32)> = None;
    for v in volumes {
        corrente = match corrente {
            Some((ini, fim)) if v == fim + 1 => Some((ini, v)),
            Some((ini, fim)) => {
                saida.push(uma_faixa(ini, fim));
                Some((v, v))
            }
            None => Some((v, v)),
        };
    }
    if let Some((ini, fim)) = corrente {
        saida.push(uma_faixa(ini, fim));
    }
    saida.join(",")
}

fn uma_faixa(ini: u32, fim: u32) -> String {
    if ini == fim {
        ini.to_string()
    } else {
        format!("{ini}-{fim}")
    }
}

/// Um registro lido pela varredura do expurgo: so o que ela precisa.
struct Visto {
    carimbo: i64,
    uuid: [u8; 16],
    ocupa: usize,
}

/// O bloco da varredura: um pedaco do volume em memoria, e onde ele comeca.
struct Bloco {
    buf: Vec<u8>,
    inicio: u64,
}

impl Bloco {
    fn novo() -> Bloco {
        Bloco {
            buf: Vec::new(),
            inicio: 0,
        }
    }

    /// Os `n` bytes a partir de `offset`, lendo um bloco novo se preciso.
    /// Quem chama garante `offset + n <= fim`.
    fn trecho(
        &mut self,
        volumes: &mut Volumes,
        volume: u32,
        offset: u64,
        n: usize,
        fim: u64,
    ) -> Result<&[u8]> {
        let cabe =
            offset >= self.inicio && offset + n as u64 <= self.inicio + self.buf.len() as u64;
        if !cabe {
            let quanto = (fim - offset).min(BLOCO_DO_EXPURGO.max(n) as u64) as usize;
            self.buf.resize(quanto, 0);
            volumes.ler(volume, offset, &mut self.buf)?;
            self.inicio = offset;
        }
        let de = (offset - self.inicio) as usize;
        Ok(&self.buf[de..de + n])
    }
}

/// O caminho de um volume, para a mensagem de erro.
fn caminho_do(volumes: &Volumes, volume: u32) -> String {
    volumes.caminho(volume).display().to_string()
}

impl TrilhaFile {
    /// O volume que recebe escrita. Nasce ou nao, e o numero que a resolucao
    /// deu -- e maior que todo fechado, por construcao.
    fn volume_ativo(&mut self) -> Result<u32> {
        self.resolver()?;
        Ok(self.ativo)
    }

    /// Decide o que um expurgo com este limite derruba -- **sem derrubar
    /// nada**, e sem precisar da chave da cifra para decidir.
    ///
    /// # A regra, inteira
    ///
    /// Anda pelos volumes do mais velho para o mais novo e para no PRIMEIRO
    /// que nao pode sair. Sai o volume fechado cujo registro MAIS NOVO e
    /// anterior a `limite`; fica o volume ativo, sempre, e fica o de fronteira
    /// (algum registro dele e do limite em diante) -- e com ele todos os
    /// seguintes. O que sai e sempre um PREFIXO: a trilha que sobra comeca
    /// num instante e nao tem buraco no meio, entao «temos tudo desde X»
    /// continua sendo uma frase verdadeira.
    ///
    /// # Por que o mais NOVO, e nao o mais velho nem o ultimo
    ///
    /// O mais velho derrubaria o volume de fronteira, com registro dentro do
    /// prazo. O ULTIMO seria o mais novo so se o relogio nunca voltasse; um
    /// ajuste de NTP para tras faz um registro do meio ser mais novo que o
    /// ultimo. A varredura confere todos, e para no primeiro registro do
    /// limite em diante -- o volume de fronteira custa o que tem antes dele, e
    /// o volume que vai sair custa uma leitura inteira, uma vez na vida dele.
    ///
    /// Cada registro que conta para SAIR tem o CRC conferido: um carimbo
    /// estragado que parecesse velho derrubaria um volume dentro do prazo. O
    /// CRC cobre o corpo como ele vai ao disco, entao nada disso precisa da
    /// chave. Registro que nao confere para o expurgo com erro, e nada sai.
    pub fn planejar_expurgo(&mut self, limite: i64) -> Result<Expurgo> {
        let mut e = Expurgo {
            limite,
            volumes: Vec::new(),
            parada: Parada::SemTrilha,
            parou_no_volume: 0,
            retido_vencido_desde: None,
            rastro: None,
            ativo_nascido: None,
        };
        // Sem ativo e com fechados (a queda entre o `rename` e o nascimento,
        // ou o nascimento interrompido), o numero do ativo so existe na
        // LISTAGEM -- e o expurgo vai apagar justamente o que a listagem le.
        // Depois dele o proximo evento nasceria no 1 de novo (papel C, P3).
        // Entao o ativo nasce AQUI, com o numero gravado no cabecalho, antes
        // de qualquer volume sair; e o selo o leva ao disco junto do rastro.
        if !self.ativo_existe && self.maior_fechado_no_disco()? > 0 {
            let cab = self.nascer_ativo()?;
            e.ativo_nascido = Some(AtivoNascido {
                diretorio: self.volumes.diretorio().to_path_buf(),
                nome: self.volumes.nome().to_string(),
                volume: cab.volume,
            });
        }
        let existentes = self.volumes_vivos()?;
        if existentes.is_empty() {
            return Ok(e);
        }
        let ativo = self.volume_ativo()?;
        for v in existentes {
            if v >= ativo {
                break;
            }
            match self.varrer_para_expurgo(v, limite)? {
                Some(vencido) => e.volumes.push(vencido),
                None => {
                    e.parada = Parada::Fronteira;
                    e.parou_no_volume = v;
                    e.retido_vencido_desde = self.primeiro_carimbo(v)?.filter(|c| *c < limite);
                    return Ok(e);
                }
            }
        }
        e.parada = Parada::VolumeAtivo;
        e.parou_no_volume = ativo;
        if self.ativo_existe {
            e.retido_vencido_desde = self.primeiro_carimbo(ativo)?.filter(|c| *c < limite);
        }
        Ok(e)
    }

    /// O registro em `offset`, com o CRC conferido.
    fn registro_conferido(
        &mut self,
        volume: u32,
        cab: &Cabecalho,
        offset: u64,
        bloco: &mut Bloco,
    ) -> Result<Visto> {
        if offset + REGISTRO_CAB as u64 > cab.fim {
            return Err(PhxError::Corrompido(format!(
                "registro de .lgpd truncado em {} (offset {offset})",
                caminho_do(&self.volumes, volume)
            )));
        }
        let c = bloco.trecho(&mut self.volumes, volume, offset, REGISTRO_CAB, cab.fim)?;
        let n = Evento::ocupa_do_cabecalho(cab, c);
        if offset + n as u64 > cab.fim {
            return Err(PhxError::Corrompido(format!(
                "registro de .lgpd em {} passa do fim do volume",
                caminho_do(&self.volumes, volume)
            )));
        }
        let reg = bloco.trecho(&mut self.volumes, volume, offset, n, cab.fim)?;
        if crc_do_registro(reg) != u32::from_le_bytes([reg[52], reg[53], reg[54], reg[55]]) {
            return Err(PhxError::Corrompido(format!(
                "registro de .lgpd com CRC invalido em {} (offset {offset}): o \
                 expurgo nao decide por carimbo que nao confere",
                caminho_do(&self.volumes, volume)
            )));
        }
        let mut uuid = [0u8; 16];
        uuid.copy_from_slice(&reg[24..40]);
        let mut carimbo = [0u8; 8];
        carimbo.copy_from_slice(&reg[0..8]);
        Ok(Visto {
            carimbo: i64::from_le_bytes(carimbo),
            uuid,
            ocupa: n,
        })
    }

    /// `Some` com o volume inteiro quando TODO registro dele e anterior ao
    /// limite; `None` no primeiro que nao e.
    fn varrer_para_expurgo(&mut self, volume: u32, limite: i64) -> Result<Option<VolumeVencido>> {
        let cab = self.cab(volume)?;
        let mut bloco = Bloco::novo();
        let mut offset = cab.cab_len as u64;
        let mut registros = 0u64;
        let mut mais_novo: Option<i64> = None;
        let mut primeiro = [0u8; 16];
        while offset < cab.fim {
            let r = self.registro_conferido(volume, &cab, offset, &mut bloco)?;
            if r.carimbo >= limite {
                return Ok(None);
            }
            if registros == 0 {
                primeiro = r.uuid;
            }
            registros += 1;
            mais_novo = Some(mais_novo.map_or(r.carimbo, |m| m.max(r.carimbo)));
            offset += r.ocupa as u64;
        }
        if registros != cab.quantos {
            return Err(PhxError::Corrompido(format!(
                "{}: o cabecalho declara {} registros e a varredura achou {registros}; \
                 o expurgo nao derruba volume que nao sabe contar",
                caminho_do(&self.volumes, volume),
                cab.quantos
            )));
        }
        Ok(Some(VolumeVencido {
            volume,
            registros,
            mais_novo,
            bytes: cab.fim,
            bilhete: (primeiro, cab.fim),
        }))
    }

    /// O carimbo do primeiro registro do volume, conferido. `None` em volume
    /// sem registro.
    fn primeiro_carimbo(&mut self, volume: u32) -> Result<Option<i64>> {
        let cab = self.cab(volume)?;
        if cab.quantos == 0 || cab.fim <= cab.cab_len as u64 {
            return Ok(None);
        }
        let mut bloco = Bloco::novo();
        let r = self.registro_conferido(volume, &cab, cab.cab_len as u64, &mut bloco)?;
        Ok(Some(r.carimbo))
    }

    /// O bilhete do volume como ele esta no disco AGORA -- cabecalho relido,
    /// sem o cache desta instancia.
    fn bilhete_no_disco(&mut self, volume: u32) -> Result<([u8; 16], u64)> {
        let cab = cofre::ler_cabecalho_do_volume(&mut self.volumes, volume, MAGIC_TRILHA)?;
        let mut uuid = [0u8; 16];
        if cab.quantos > 0 && cab.fim >= cab.cab_len as u64 + REGISTRO_CAB as u64 {
            self.volumes
                .ler(volume, cab.cab_len as u64 + 24, &mut uuid)?;
        }
        Ok((uuid, cab.fim))
    }

    /// Fase 3 do expurgo: derruba os volumes planejados -- e so eles, e so
    /// inteiros. Devolve os que sairam agora.
    ///
    /// # As duas guardas, no ponto do dano
    ///
    /// O plano ja exclui o volume ativo; esta funcao confere de novo, porque
    /// entre o plano e o `unlink` o servidor soltou a trava global (a fase 2
    /// roda fora dela) e porque a guarda que importa e a que esta encostada no
    /// `remove_file`. E confere o BILHETE de cada volume -- o UUID do primeiro
    /// registro e o fim logico, relidos do disco: uma tabela excluida e
    /// recriada com o mesmo nome no meio do caminho teria volumes com os mesmos
    /// numeros e outro conteudo. Qualquer divergencia recusa ANTES do primeiro
    /// `unlink`: expurgo pela metade nao acontece por esta porta.
    ///
    /// Volume planejado que ja nao existe e pulado: o estado pedido -- ele fora
    /// do disco -- ja vale.
    pub fn apagar_expurgados(&mut self, selado: &ExpurgoSelado) -> Result<Vec<u32>> {
        let e = selado.expurgo();
        let existentes = self.volumes_vivos()?;
        let ativo = self.volume_ativo()?;
        let mut alvos = Vec::with_capacity(e.volumes.len());
        for v in &e.volumes {
            if v.volume >= ativo {
                return Err(PhxError::Esquema(format!(
                    "recusado: o volume {} de {} e o que recebe escrita, e o expurgo \
                     nunca derruba o volume ativo",
                    v.volume,
                    self.volumes.nome()
                )));
            }
            if !existentes.contains(&v.volume) {
                continue;
            }
            if self.bilhete_no_disco(v.volume)? != v.bilhete {
                return Err(PhxError::Esquema(format!(
                    "o volume {} da trilha de {} mudou entre o planejamento e o \
                     expurgo; nenhum volume foi apagado",
                    v.volume,
                    self.volumes.nome()
                )));
            }
            alvos.push(v.volume);
        }
        let mut saiu = Vec::with_capacity(alvos.len());
        for v in &alvos {
            // O erro no meio devolve a lista PARCIAL: o rastro ja selado diz o
            // que foi pedido, e so o diretorio diz o que saiu -- quem recebe o
            // erro precisa das duas coisas para nao afirmar um expurgo que nao
            // aconteceu inteiro.
            if let Err(e) = self.volumes.apagar_volume(*v) {
                let frase = format!(
                    "o expurgo da trilha de {} parou no volume {v} ({e}); ja tinham \
                     saido {saiu:?}. O rastro no .reason diz o que foi pedido; o que \
                     saiu e o que o diretorio diz",
                    self.volumes.nome()
                );
                return Err(match e {
                    PhxError::Io(io) => PhxError::Io(std::io::Error::new(io.kind(), frase)),
                    _ => PhxError::Esquema(frase),
                });
            }
            self.cabs.remove(v);
            saiu.push(*v);
        }
        Ok(saiu)
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

    /// O mesmo, do lado do `antes`, que aceita a ausencia. Ver
    /// [`TrilhaFile::registrar_alteracao`].
    fn velho(s: &str) -> Option<(String, bool)> {
        Some(claro(s))
    }

    #[test]
    fn grava_e_le_de_volta() {
        let d = temp("ida-e-volta");
        let mut t = TrilhaFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        t.usuario = 7;
        t.ip = "192.0.2.10".into();
        t.registrar_alteracao(1, "email", velho("a@x.com"), claro("b@y.com"), "id=42")
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
            t.registrar_alteracao(i as u64 + 1, "c", velho(v), claro("z"), "id=1")
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
        t.registrar_alteracao(1, "cpf", velho("a"), claro("b"), "id=1")
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
        t.registrar_alteracao(1, "obs", velho(&longo), claro(""), "id=1")
            .unwrap();
        let lidos = t.ler(0, 0).unwrap();
        assert!(lidos[0].antes.len() <= VALOR_MAX);
        assert!(longo.starts_with(&lidos[0].antes));
    }

    /// O `None` no `antes` atravessa o arquivo: grava a marca, liga o bit, e a
    /// leitura devolve os dois. Sem o bit, quem le teria de decidir pela FRASE
    /// -- e frase muda no dia em que alguem melhorar a redacao.
    #[test]
    fn antes_indisponivel_vai_e_volta_com_o_bit() {
        let d = temp("indisponivel");
        let mut t = TrilhaFile::abrir(&d, "t", Paginacao::DESLIGADA).unwrap();
        t.registrar_alteracao(1, "laudo", None, claro("LAUDO_NOVO"), "id=1")
            .unwrap();
        let lidos = t.ler(0, 0).unwrap();
        assert_eq!(lidos.len(), 1);
        assert!(lidos[0].antes_indisponivel());
        assert_eq!(lidos[0].antes, INDISPONIVEL);
        assert_eq!(lidos[0].depois, "LAUDO_NOVO");
        // E os dois bits vizinhos continuam apagados: um `flags |= 4` que
        // acertasse o bit errado passaria despercebido sem esta linha.
        assert!(!lidos[0].antes_redigido() && !lidos[0].depois_redigido());

        // A prova pelo contrario, no mesmo arquivo: um `antes` presente e
        // VAZIO (coluna que estava nula) nao liga o bit. Sem ela, o teste
        // acima passaria ate numa implementacao que ligasse o bit sempre.
        t.registrar_alteracao(2, "laudo", velho(""), claro("X"), "id=2")
            .unwrap();
        let lidos = t.ler(0, 0).unwrap();
        assert!(!lidos[1].antes_indisponivel(), "nulo virou indisponivel");
        assert_eq!(lidos[1].antes, "");
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

    // ------------------------------------------------------------- expurgo

    /// Um instante do calendario em ms. So para os testes lerem como data.
    fn em(data: &str) -> i64 {
        phxsql_core::datahora::ms_de_instante_iso(data).unwrap()
    }

    /// Registro de 200 bytes cravados, com o carimbo que o teste mandar.
    ///
    /// O tamanho fixo e o que deixa o teste dizer QUAL volume recebe qual
    /// registro: com `POR_VOLUME` bytes, cabem exatamente tres por volume.
    const TAMANHO: usize = 200;
    const POR_VOLUME: u64 = 64 + 3 * TAMANHO as u64;

    fn paginada() -> Paginacao {
        Paginacao::nova(1_000, 999)
            .unwrap()
            .com_bytes_por_arquivo(POR_VOLUME)
            .unwrap()
    }

    fn gravar_em(t: &mut TrilhaFile, carimbo: i64, cpf: &str) {
        let identidade = format!("cpf={cpf}");
        let fixo = REGISTRO_CAB + "cpf".len() + identidade.len() + 1;
        let e = Evento {
            uuid: Uuid::v7(),
            carimbo,
            tipo: Tipo::Alteracao,
            rowid: 1,
            usuario: 7,
            coluna: "cpf".into(),
            antes: "a".repeat(TAMANHO - fixo),
            depois: "b".into(),
            identidade,
            ip: String::new(),
            linhas: 0,
            flags: 0,
        };
        t.anexar(&e).unwrap();
    }

    /// Uma trilha com os volumes pedidos, cada um com os carimbos dados. O
    /// ULTIMO da lista e o ativo.
    /// A trilha com o corte por tamanho do teste. O corte e da INSTANCIA
    /// (tirado do global na abertura), entao o teste o fixa aqui sem mexer no
    /// global que os outros testes do binario usam.
    fn aberta(d: &Path, pag: Paginacao) -> TrilhaFile {
        let mut t = TrilhaFile::abrir(d, "t", pag).unwrap();
        t.corte_bytes = POR_VOLUME;
        t
    }

    fn trilha_com(d: &Path, volumes: &[&[&str]]) -> TrilhaFile {
        let mut t = aberta(d, paginada());
        let mut n = 0;
        for (i, vol) in volumes.iter().enumerate() {
            // So o volume vira quando ENCHE: um volume do meio com menos de
            // tres registros faria o seguinte comecar dentro dele, e o teste
            // estaria provando outra trilha que nao a desenhada.
            let ultimo = i + 1 == volumes.len();
            assert!(!vol.is_empty() && vol.len() <= 3 && (ultimo || vol.len() == 3));
            for c in *vol {
                n += 1;
                gravar_em(&mut t, em(c), &format!("{n:011}"));
            }
        }
        t
    }

    /// Os arquivos da trilha no diretorio, pelo SISTEMA DE ARQUIVOS -- e nao
    /// pelo `existentes`, que e o que esta sendo provado.
    fn no_disco(d: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(d)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".lgpd"))
            .collect();
        v.sort();
        v
    }

    /// O rastro, como a `Table` o grava: um `.reason` de verdade ao lado.
    fn com_rastro(d: &Path, mut e: Expurgo) -> Expurgo {
        if !e.volumes.is_empty() {
            let mut m = crate::motivo::MotivoFile::criar(d, "t", paginada()).unwrap();
            m.registrar(crate::motivo::Tipo::Expurgo, 0, "teste", &e.rastro())
                .unwrap();
            e.rastro = Some(m.volume_do_ultimo());
        }
        e
    }

    fn expurgar(d: &Path, t: &mut TrilhaFile, limite: i64) -> (Expurgo, Vec<u32>) {
        let e = com_rastro(d, t.planejar_expurgo(limite).unwrap());
        let selado = e.selar().unwrap();
        let saiu = t.apagar_expurgados(&selado).unwrap();
        (selado.em_expurgo(), saiu)
    }

    /// **A regra inteira, num caso so.** Cinco volumes: dois todos velhos, um
    /// de FRONTEIRA (comeca velho e termina novo), um novo e o ativo. Saem os
    /// dois primeiros, inteiros; o de fronteira fica -- e com ele tudo o que
    /// vem depois, inclusive o volume 4, que e novo.
    ///
    /// **Defeito reposto** (criterio pelo registro mais VELHO em vez do mais
    /// novo): o volume 3 comeca em 2012 e sai junto, e as asserções do
    /// `volumes` e do disco caem.
    #[test]
    fn o_expurgo_derruba_so_volume_inteiro_e_para_na_fronteira() {
        let d = temp("expurgo-fronteira");
        let mut t = trilha_com(
            &d,
            &[
                &["2010-01-01", "2010-02-01", "2010-03-01"],
                &["2011-01-01", "2011-02-01", "2011-03-01"],
                &["2012-01-01", "2020-01-01", "2020-02-01"],
                &["2021-01-01", "2021-02-01", "2021-03-01"],
                &["2022-01-01"],
            ],
        );
        assert_eq!(no_disco(&d).len(), 5, "{:?}", no_disco(&d));

        let (e, saiu) = expurgar(&d, &mut t, em("2015-01-01"));
        assert_eq!(saiu, vec![1, 2], "{e:?}");
        assert_eq!(e.registros(), 6);
        assert_eq!(e.volumes[1].mais_novo, Some(em("2011-03-01")));
        assert_eq!(e.parada, Parada::Fronteira);
        assert_eq!(e.parou_no_volume, 3);
        assert_eq!(
            e.retido_vencido_desde,
            Some(em("2012-01-01")),
            "o volume de fronteira guarda um registro vencido, e a resposta tem de dize-lo"
        );
        assert_eq!(
            no_disco(&d),
            vec!["t.lgpd", "t_003.lgpd", "t_004.lgpd"],
            "o disco nao e o que o expurgo diz"
        );
    }

    /// O relogio que VOLTA no meio do volume: o registro do meio e o mais
    /// novo, e o ultimo e velho. Quem decidisse pelo ULTIMO registro -- o
    /// atalho barato, porque o volume e append-only -- derrubaria um volume
    /// com registro de 2020 dentro de um prazo que acaba em 2015.
    ///
    /// **Defeito reposto** (decidir so pelo ultimo registro): o volume 1 sai.
    #[test]
    fn o_relogio_que_volta_nao_engana_o_criterio() {
        let d = temp("expurgo-relogio");
        let mut t = trilha_com(
            &d,
            &[&["2010-01-01", "2020-01-01", "2010-01-02"], &["2021-01-01"]],
        );
        let (e, saiu) = expurgar(&d, &mut t, em("2015-01-01"));
        assert!(saiu.is_empty(), "saiu {saiu:?}: {e:?}");
        assert_eq!(e.parada, Parada::Fronteira);
        assert_eq!(no_disco(&d), vec!["t.lgpd", "t_001.lgpd"]);
    }

    /// **O volume ativo nunca sai**, nem com todo registro vencido. Os tres
    /// volumes aqui sao de 2010 e o limite e agora: saem os dois fechados, e o
    /// ativo fica -- e a resposta diz que ele guarda dado vencido.
    ///
    /// **Defeito reposto** (tirar o `if v >= ativo { break; }` do plano): o
    /// ativo entra no plano, a guarda da fase 3 recusa o expurgo inteiro, e o
    /// `unwrap` do `apagar_expurgados` cai.
    #[test]
    fn o_volume_ativo_nunca_sai_mesmo_vencido() {
        let d = temp("expurgo-ativo");
        let mut t = trilha_com(
            &d,
            &[
                &["2010-01-01", "2010-01-02", "2010-01-03"],
                &["2010-02-01", "2010-02-02", "2010-02-03"],
                &["2010-03-01"],
            ],
        );
        let (e, saiu) = expurgar(&d, &mut t, crate::util::agora_ms());
        assert_eq!(saiu, vec![1, 2]);
        assert_eq!(e.parada, Parada::VolumeAtivo);
        assert_eq!(e.parou_no_volume, 3);
        assert_eq!(e.retido_vencido_desde, Some(em("2010-03-01")));
        assert_eq!(no_disco(&d), vec!["t.lgpd"]);
    }

    /// A SEGUNDA guarda do volume ativo, a que esta encostada no `unlink`: um
    /// plano que traga o ativo -- adulterado aqui a mao, como o traria um
    /// defeito no planejador -- e recusado INTEIRO, antes do primeiro volume
    /// sair. Nem os dois volumes legitimos saem.
    ///
    /// **Defeito reposto** (tirar a guarda `v.volume >= ativo` do
    /// `apagar_expurgados`): o volume 3 some do disco e o teste cai no
    /// `is_err`.
    #[test]
    fn a_fase_de_apagar_recusa_o_volume_ativo_mesmo_num_plano_adulterado() {
        let d = temp("expurgo-adulterado");
        let mut t = trilha_com(
            &d,
            &[
                &["2010-01-01", "2010-01-02", "2010-01-03"],
                &["2010-02-01", "2010-02-02", "2010-02-03"],
                &["2010-03-01"],
            ],
        );
        let mut e = t.planejar_expurgo(crate::util::agora_ms()).unwrap();
        assert_eq!(e.volumes.len(), 2);
        let bilhete = t.bilhete_no_disco(3).unwrap();
        e.volumes.push(VolumeVencido {
            volume: 3,
            registros: 1,
            mais_novo: Some(em("2010-03-01")),
            bytes: bilhete.1,
            bilhete,
        });
        let selado = com_rastro(&d, e).selar().unwrap();
        let r = t.apagar_expurgados(&selado);
        assert!(r.is_err(), "o volume ativo foi aceito para apagar: {r:?}");
        assert_eq!(
            no_disco(&d),
            vec!["t.lgpd", "t_001.lgpd", "t_002.lgpd"],
            "a recusa tem de vir ANTES do primeiro unlink"
        );
    }

    /// O bilhete: entre o plano e o `unlink` o servidor solta a trava global,
    /// e o volume 1 que ele vai apagar tem de ser o MESMO que ele planejou.
    /// Aqui o primeiro registro do volume 1 muda de UUID no disco -- o que
    /// uma tabela excluida e recriada com o mesmo nome faria -- e nada sai.
    ///
    /// **Defeito reposto** (tirar a conferencia do bilhete): os volumes 1 e 2
    /// saem e o teste cai no `is_err`.
    #[test]
    fn o_bilhete_recusa_volume_trocado_entre_o_plano_e_o_unlink() {
        let d = temp("expurgo-bilhete");
        let mut t = trilha_com(
            &d,
            &[
                &["2010-01-01", "2010-01-02", "2010-01-03"],
                &["2010-02-01", "2010-02-02", "2010-02-03"],
                &["2021-01-01"],
            ],
        );
        let selado = com_rastro(&d, t.planejar_expurgo(em("2015-01-01")).unwrap())
            .selar()
            .unwrap();
        assert_eq!(selado.expurgo().volumes.len(), 2);
        // O UUID do primeiro registro do volume 1: cabecalho do volume (64) +
        // 24 bytes dentro do registro.
        let caminho = d.join("t_001.lgpd");
        let mut bruto = std::fs::read(&caminho).unwrap();
        bruto[64 + 24] ^= 0xFF;
        std::fs::write(&caminho, &bruto).unwrap();

        let mut outra = TrilhaFile::abrir(&d, "t", paginada()).unwrap();
        let r = outra.apagar_expurgados(&selado);
        assert!(
            r.is_err(),
            "apagou um volume que nao era o planejado: {r:?}"
        );
        assert_eq!(no_disco(&d).len(), 3, "{:?}", no_disco(&d));
    }

    /// **O `unlink` que falha no meio devolve a lista PARCIAL.** O rastro ja
    /// selado diz o que foi pedido; so o diretorio diz o que saiu -- e quem
    /// recebe o erro precisa saber que o volume 1 ja nao existe.
    ///
    /// A falha e do sistema de arquivos, e nao de gancho de teste: o volume 2
    /// sai do nome dele DEPOIS do plano (o descritor aberto no plano continua
    /// lendo o arquivo, e o bilhete confere) e um DIRETORIO toma o nome -- o
    /// `unlink` dele devolve `EISDIR`.
    ///
    /// **Defeito reposto** (o erro do `unlink` sobe cru, com `?`): a mensagem
    /// nao diz que o volume 1 ja saiu, e a asserção dela cai.
    #[test]
    fn o_unlink_que_falha_no_meio_diz_o_que_ja_saiu() {
        let d = temp("expurgo-parcial");
        let mut t = trilha_com(
            &d,
            &[
                &["2010-01-01", "2010-01-02", "2010-01-03"],
                &["2010-02-01", "2010-02-02", "2010-02-03"],
                &["2010-03-01", "2010-03-02", "2010-03-03"],
                &["2021-01-01"],
            ],
        );
        let selado = com_rastro(&d, t.planejar_expurgo(em("2015-01-01")).unwrap())
            .selar()
            .unwrap();
        assert_eq!(selado.expurgo().volumes.len(), 3);
        std::fs::rename(d.join("t_002.lgpd"), d.join("fora.bin")).unwrap();
        std::fs::create_dir(d.join("t_002.lgpd")).unwrap();

        let erro = t.apagar_expurgados(&selado).unwrap_err().to_string();
        assert!(
            erro.contains("parou no volume 2") && erro.contains("ja tinham saido [1]"),
            "o erro nao diz o que ja saiu: {erro}"
        );
        assert_eq!(
            no_disco(&d),
            vec!["t.lgpd", "t_002.lgpd", "t_003.lgpd"],
            "o volume 1 saiu, o 2 falhou e o 3 nao foi tentado"
        );
    }

    /// **A trilha continua depois do expurgo** -- a prova de que nao ha
    /// numeracao contigua escondida em lugar nenhum. Depois de sairem os
    /// volumes 1 e 2: a leitura devolve o resto na ordem, a paginacao por
    /// `pular`/`limite` anda sem buraco, o total e o `verificar` batem, a
    /// trilha REABERTA enxerga o mesmo, e a escrita seguinte continua no
    /// volume ativo e vira para o 6 -- nunca volta ao 1.
    #[test]
    fn a_trilha_continua_depois_do_expurgo() {
        let d = temp("expurgo-continua");
        let mut t = trilha_com(
            &d,
            &[
                &["2010-01-01", "2010-01-02", "2010-01-03"],
                &["2010-02-01", "2010-02-02", "2010-02-03"],
                &["2021-01-01", "2021-01-02", "2021-01-03"],
                &["2021-02-01", "2021-02-02", "2021-02-03"],
                &["2021-03-01"],
            ],
        );
        let (_, saiu) = expurgar(&d, &mut t, em("2015-01-01"));
        assert_eq!(saiu, vec![1, 2]);

        let mut t = aberta(&d, paginada());
        assert!(t.existe());
        assert_eq!(t.volumes_existentes().unwrap(), vec![3, 4, 5]);
        let todos = t.ler(0, 0).unwrap();
        let carimbos: Vec<i64> = todos.iter().map(|e| e.carimbo).collect();
        assert_eq!(
            carimbos,
            [
                "2021-01-01",
                "2021-01-02",
                "2021-01-03",
                "2021-02-01",
                "2021-02-02",
                "2021-02-03",
                "2021-03-01"
            ]
            .iter()
            .map(|c| em(c))
            .collect::<Vec<_>>()
        );
        assert_eq!(t.total().unwrap(), 7);
        assert_eq!(t.verificar().unwrap(), 7);
        // Pagina de dois em dois atravessando a borda dos volumes 3 e 4.
        let mut paginado = Vec::new();
        for pular in (0..7).step_by(2) {
            paginado.extend(t.ler(pular, 2).unwrap());
        }
        assert_eq!(paginado, todos, "a paginacao pulou ou repetiu registro");

        // Mais tres: um enche o volume 5, dois viram para o 6.
        for i in 0..3 {
            gravar_em(&mut t, em("2021-04-01") + i, "99999999999");
        }
        assert_eq!(t.volumes_existentes().unwrap(), vec![3, 4, 5, 6]);
        assert_eq!(t.total().unwrap(), 10);
        assert_eq!(t.verificar().unwrap(), 10);
    }

    /// **O que sai e sempre um PREFIXO -- a trilha que sobra nao tem buraco.**
    /// O relogio voltou: o volume 2 tem um registro de 2020 (fronteira) e o 3
    /// e todo de 2010 de novo. Derrubar o 3 deixaria a trilha com 2 e 4 e sem
    /// o meio, e «temos tudo desde X» deixaria de ser verdade. Sai so o 1.
    ///
    /// **Defeito reposto** (seguir adiante depois da fronteira em vez de
    /// parar): o volume 3 sai e o disco fica com um buraco no meio.
    #[test]
    fn o_expurgo_so_derruba_prefixo_e_nao_abre_buraco() {
        let d = temp("expurgo-prefixo");
        let mut t = trilha_com(
            &d,
            &[
                &["2010-01-01", "2010-01-02", "2010-01-03"],
                &["2010-02-01", "2020-01-01", "2010-02-03"],
                &["2010-03-01", "2010-03-02", "2010-03-03"],
                &["2021-01-01"],
            ],
        );
        let (e, saiu) = expurgar(&d, &mut t, em("2015-01-01"));
        assert_eq!(saiu, vec![1], "{e:?}");
        assert_eq!(e.parada, Parada::Fronteira);
        assert_eq!(e.parou_no_volume, 2);
        assert_eq!(no_disco(&d), vec!["t.lgpd", "t_002.lgpd", "t_003.lgpd"]);
    }

    /// **A tabela SEM paginacao passa a expurgar** (formato B, pedido 368).
    /// Antes, a trilha dela era um arquivo so, que era o ativo para sempre, e
    /// o expurgo respondia `arquivo_unico` sem apagar nada. Agora o ativo fecha
    /// por idade -- o primeiro registro passou de `lgpd.volume_dias` --, vira
    /// `t_001.lgpd`, o ativo 2 nasce no nome fixo, e o volume 1 sai inteiro.
    ///
    /// **Defeito reposto** (o `fechar_se_velho` nao fecha): nada sai, e o
    /// `assert_eq!(saiu, vec![1])` cai.
    #[test]
    fn a_tabela_sem_paginacao_fecha_por_idade_e_expurga() {
        let d = temp("expurgo-sem-paginacao");
        let mut t = aberta(&d, Paginacao::DESLIGADA);
        for c in ["2010-01-01", "2010-01-02", "2010-01-03"] {
            gravar_em(&mut t, em(c), "1");
        }
        assert_eq!(no_disco(&d), vec!["t.lgpd"]);
        // Antes de fechar, o expurgo para no ativo e diz que ha dado vencido.
        let (e, saiu) = expurgar(&d, &mut t, crate::util::agora_ms());
        assert!(saiu.is_empty());
        assert_eq!(e.parada, Parada::VolumeAtivo);
        assert_eq!(e.retido_vencido_desde, Some(em("2010-01-01")));

        assert_eq!(t.fechar_se_velho(crate::util::agora_ms()).unwrap(), Some(1));
        assert_eq!(no_disco(&d), vec!["t.lgpd", "t_001.lgpd"]);
        let (e, saiu) = expurgar(&d, &mut t, crate::util::agora_ms());
        assert_eq!(saiu, vec![1], "{e:?}");
        assert_eq!(e.registros(), 3);
        assert_eq!(no_disco(&d), vec!["t.lgpd"]);
        // O que sobrou e o ativo NOVO: volume 2, vazio.
        let mut t = aberta(&d, Paginacao::DESLIGADA);
        assert_eq!(t.volumes_existentes().unwrap(), vec![2]);
        assert_eq!(t.total().unwrap(), 0);
    }

    /// O fechamento por idade conta do PRIMEIRO registro do ativo e so fecha
    /// ACIMA do corte: com 30 dias, 29 dias nao fecham e 31 fecham.
    ///
    /// **Defeito reposto** (fechar sem olhar a idade): a primeira chamada ja
    /// fecha, e o `assert_eq!(.., None)` cai.
    #[test]
    fn o_fechamento_por_idade_so_passa_do_corte() {
        let d = temp("fecha-idade");
        let mut t = aberta(&d, Paginacao::DESLIGADA);
        t.corte_dias = 30;
        let comeco = em("2026-01-01");
        gravar_em(&mut t, comeco, "1");
        gravar_em(&mut t, comeco + 20 * 86_400_000, "2");
        assert_eq!(
            t.fechar_se_velho(comeco + 29 * 86_400_000).unwrap(),
            None,
            "fechou com 29 dias"
        );
        assert_eq!(no_disco(&d), vec!["t.lgpd"]);
        assert_eq!(
            t.fechar_se_velho(comeco + 31 * 86_400_000).unwrap(),
            Some(1),
            "nao fechou com 31 dias"
        );
        assert_eq!(no_disco(&d), vec!["t.lgpd", "t_001.lgpd"]);
        // O ativo novo esta vazio: fechar de novo nao faz nada.
        assert_eq!(t.fechar_se_velho(comeco + 400 * 86_400_000).unwrap(), None);
    }

    /// **O fechamento pedido**: o ativo com registro vira `t_NNN.lgpd`, com NNN
    /// igual ao volume do CABECALHO, e o ativo seguinte nasce no nome fixo com
    /// o numero seguinte gravado no cabecalho dele. Nenhum byte do volume
    /// fechado muda -- e renomear. O ativo vazio nao fecha.
    #[test]
    fn o_fechamento_pedido_renomeia_e_nasce_o_seguinte() {
        let d = temp("fecha-pedido");
        let mut t = aberta(&d, Paginacao::DESLIGADA);
        assert_eq!(t.fechar_ativo().unwrap(), None, "fechou sem ativo");
        gravar_em(&mut t, em("2026-01-01"), "1");
        let antes = std::fs::read(d.join("t.lgpd")).unwrap();
        assert_eq!(t.fechar_ativo().unwrap(), Some(1));
        assert_eq!(
            std::fs::read(d.join("t_001.lgpd")).unwrap(),
            antes,
            "fechar mudou os bytes do volume"
        );
        let cab = crate::cofre::ler_cabecalho(
            &std::fs::read(d.join("t.lgpd")).unwrap(),
            MAGIC_TRILHA,
            "t.lgpd",
            3,
        )
        .unwrap();
        assert_eq!((cab.volume, cab.quantos), (2, 0));
        assert_eq!(t.fechar_ativo().unwrap(), None, "fechou o ativo vazio");
        gravar_em(&mut t, em("2026-01-02"), "2");
        let mut t = aberta(&d, Paginacao::DESLIGADA);
        assert_eq!(t.volumes_existentes().unwrap(), vec![1, 2]);
        assert_eq!(t.ler(0, 0).unwrap().len(), 2);
    }

    /// **A queda entre o `rename` e o nascimento do novo ativo.** O ativo 3
    /// virou `t_003.lgpd` e o processo morreu antes de o 4 nascer: nao ha
    /// `t.lgpd`. A trilha reaberta acha os tres pela listagem (o maior
    /// fechado + 1 e o ativo), le tudo, e o proximo registro faz nascer o
    /// ativo 4 -- nunca de novo o 1.
    ///
    /// **Defeito reposto** (sem o ativo, a resolucao supoe o volume 1 em vez
    /// de listar o diretorio): a trilha reaberta nao ve os tres fechados, e a
    /// primeira asserção cai.
    #[test]
    fn a_queda_entre_o_rename_e_o_nascimento_cai_na_mesma_regra() {
        let d = temp("queda-no-fecho");
        let t = trilha_com(
            &d,
            &[
                &["2026-01-01", "2026-01-02", "2026-01-03"],
                &["2026-02-01", "2026-02-02", "2026-02-03"],
                &["2026-03-01"],
            ],
        );
        drop(t);
        // A queda: o rename aconteceu, o nascimento nao.
        std::fs::rename(d.join("t.lgpd"), d.join("t_003.lgpd")).unwrap();
        assert_eq!(no_disco(&d), vec!["t_001.lgpd", "t_002.lgpd", "t_003.lgpd"]);

        let mut t = aberta(&d, paginada());
        assert!(t.existe(), "a trilha sumiu sem o ativo");
        assert_eq!(t.volumes_existentes().unwrap(), vec![1, 2, 3]);
        assert_eq!(t.total().unwrap(), 7);
        assert_eq!(t.verificar().unwrap(), 7);
        gravar_em(&mut t, em("2026-04-01"), "8");
        let cab = crate::cofre::ler_cabecalho(
            &std::fs::read(d.join("t.lgpd")).unwrap(),
            MAGIC_TRILHA,
            "t.lgpd",
            3,
        )
        .unwrap();
        assert_eq!(cab.volume, 4, "o ativo nasceu com o numero errado");
        let mut t = aberta(&d, paginada());
        assert_eq!(t.volumes_existentes().unwrap(), vec![1, 2, 3, 4]);
        assert_eq!(t.total().unwrap(), 8);
    }

    fn cabecalho_do_ativo(d: &Path) -> Cabecalho {
        crate::cofre::ler_cabecalho(
            &std::fs::read(d.join("t.lgpd")).unwrap(),
            MAGIC_TRILHA,
            "t.lgpd",
            3,
        )
        .unwrap()
    }

    /// **O ativo mais curto que o cabecalho e nascimento interrompido** (papel
    /// C, B2/P1). O ativo 3 nasceu no fechamento e a queda levou o cabecalho
    /// dele antes do disco: `t.lgpd` com 0 byte, e com 40. A trilha abre, le
    /// os 4 registros dos fechados, e o proximo registro faz nascer o 3 --
    /// o numero que ele ia ter, pela listagem.
    ///
    /// **Defeito reposto** (a abertura sem a regra): `abrir` le o cabecalho
    /// de um arquivo vazio e cai em «failed to fill whole buffer» -- a tabela
    /// inteira nao abriria, leitura inclusive.
    #[test]
    fn o_ativo_mais_curto_que_o_cabecalho_e_nascimento_interrompido() {
        for tamanho in [0u64, 40] {
            let d = temp(&format!("nascimento-interrompido-{tamanho}"));
            let mut t = trilha_com(
                &d,
                &[&["2026-01-01", "2026-01-02", "2026-01-03"], &["2026-01-04"]],
            );
            assert_eq!(t.fechar_ativo().unwrap(), Some(2));
            drop(t);
            std::fs::OpenOptions::new()
                .write(true)
                .open(d.join("t.lgpd"))
                .unwrap()
                .set_len(tamanho)
                .unwrap();

            let mut t = aberta(&d, paginada());
            assert!(t.existe());
            assert_eq!(t.volumes_existentes().unwrap(), vec![1, 2]);
            assert_eq!(t.total().unwrap(), 4, "tamanho {tamanho}");
            assert_eq!(t.ler(0, 0).unwrap().len(), 4);
            gravar_em(&mut t, em("2026-01-05"), "5");
            let cab = cabecalho_do_ativo(&d);
            assert_eq!((cab.volume, cab.quantos), (3, 1), "tamanho {tamanho}");
            let mut t = aberta(&d, paginada());
            assert_eq!(t.volumes_existentes().unwrap(), vec![1, 2, 3]);
            assert_eq!(t.total().unwrap(), 5);
        }
    }

    /// A regra do tamanho pelos dois lados: menos de 64 bytes e interrompido;
    /// de 64 a 127 so quando a versao escrita e a 3 (o cabecalho cifrado
    /// cortado) -- um v2 de 64 bytes e um ativo vazio legitimo.
    #[test]
    fn nascimento_interrompido_pelo_tamanho_e_pela_versao() {
        let d = temp("interrompido-regua");
        let p = d.join("x.lgpd");
        let caso = |bytes: &[u8]| {
            std::fs::write(&p, bytes).unwrap();
            nascimento_interrompido(&p).unwrap()
        };
        assert!(caso(&[]));
        assert!(caso(&[7u8; 63]));
        let mut v3 = vec![0u8; 100];
        v3[8] = 3;
        assert!(caso(&v3), "v3 cortado nao e cabecalho");
        let mut v2 = vec![0u8; 64];
        v2[8] = 2;
        assert!(!caso(&v2), "v2 de 64 bytes e um ativo vazio de verdade");
        assert!(!caso(&[0u8; 128]));
    }

    /// **O expurgo depois da queda nao reusa o numero** (papel C, P3). Sem o
    /// ativo -- a queda entre o `rename` e o nascimento --, o numero dele so
    /// existe na listagem dos fechados, e o expurgo apaga justamente os
    /// fechados. O plano faz nascer o ativo 3 ANTES de o 1 e o 2 sairem, e o
    /// selo o leva ao disco junto do rastro (dois `fsync`).
    ///
    /// **Defeito reposto** (sem o nascimento no plano): depois do expurgo o
    /// diretorio fica vazio, o proximo registro nasce no volume 1 de novo e a
    /// asserção do numero cai.
    #[test]
    fn o_expurgo_depois_da_queda_nao_reusa_o_numero() {
        let d = temp("expurgo-sem-ativo");
        let mut t = trilha_com(
            &d,
            &[
                &["2010-01-01", "2010-01-02", "2010-01-03"],
                &["2010-02-01", "2010-02-02", "2010-02-03"],
            ],
        );
        assert_eq!(t.fechar_ativo().unwrap(), Some(2));
        drop(t);
        std::fs::remove_file(d.join("t.lgpd")).unwrap();
        assert_eq!(no_disco(&d), vec!["t_001.lgpd", "t_002.lgpd"]);

        let mut t = aberta(&d, paginada());
        let e = com_rastro(&d, t.planejar_expurgo(em("2015-01-01")).unwrap());
        let selado = e.selar().unwrap();
        // Conferido por ULTIMO: o defeito do nascimento tem de cair na
        // asserção do numero, que e o dano, e nao nesta.
        let sincronizados = selado.sincronizados();
        assert_eq!(t.apagar_expurgados(&selado).unwrap(), vec![1, 2]);

        let mut t = aberta(&d, paginada());
        gravar_em(&mut t, em("2026-01-01"), "7");
        assert_eq!(
            cabecalho_do_ativo(&d).volume,
            3,
            "o numero voltou ao comeco: volume reusado"
        );
        assert_eq!(no_disco(&d), vec!["t.lgpd"]);
        assert_eq!(
            sincronizados, 2,
            "o ativo nascido nao foi ao disco com o rastro"
        );
    }

    /// O numero do NOME e o do CABECALHO: um arquivo no lugar de `t_001.lgpd`
    /// cujo cabecalho diz outro volume e recusado, e nao lido como se fosse
    /// desta trilha. (Nao cobre a tabela `t_001`, cujo ativo tem volume 1 no
    /// cabecalho: essa quem barra e a declaracao, no catalogo.)
    ///
    /// **Defeito reposto** (sem conferir o numero no `cab`): a leitura soma o
    /// arquivo trocado e o `is_err` cai.
    #[test]
    fn volume_com_outro_numero_no_cabecalho_e_recusado() {
        let d = temp("numero-trocado");
        let t = trilha_com(
            &d,
            &[&["2026-01-01", "2026-01-02", "2026-01-03"], &["2026-02-01"]],
        );
        drop(t);
        std::fs::copy(d.join("t.lgpd"), d.join("t_001.lgpd")).unwrap();
        let mut t = aberta(&d, paginada());
        let r = t.total();
        assert!(
            matches!(r, Err(PhxError::Corrompido(_))),
            "leu como volume 1 um arquivo que diz ser o 2: {r:?}"
        );
    }

    /// O numero do volume nao tem teto nem largura fixa: o 999 fecha como
    /// `t_999.lgpd` e o 1000 como `t_1000.lgpd`, e a leitura anda por eles.
    #[test]
    fn a_numeracao_nao_tem_teto() {
        let d = temp("sem-teto-de-numero");
        let mut t = aberta(&d, Paginacao::DESLIGADA);
        t.resolver().unwrap();
        t.ativo = 999;
        t.volumes.definir_ativo_da_trilha(999);
        gravar_em(&mut t, em("2026-01-01"), "1");
        assert_eq!(t.fechar_ativo().unwrap(), Some(999));
        gravar_em(&mut t, em("2026-01-02"), "2");
        assert_eq!(t.fechar_ativo().unwrap(), Some(1000));
        assert_eq!(no_disco(&d), vec!["t.lgpd", "t_1000.lgpd", "t_999.lgpd"]);
        let mut t = aberta(&d, Paginacao::DESLIGADA);
        assert_eq!(t.volumes_existentes().unwrap(), vec![999, 1000, 1001]);
        assert_eq!(t.total().unwrap(), 2);
    }

    /// Tabela sem trilha: o plano nao cria arquivo -- a presenca do `.lgpd`
    /// continua sendo a resposta a «esta tabela tem dado pessoal?».
    #[test]
    fn sem_trilha_o_expurgo_nao_cria_arquivo() {
        let d = temp("expurgo-sem-trilha");
        let mut t = aberta(&d, paginada());
        let (e, saiu) = expurgar(&d, &mut t, crate::util::agora_ms());
        assert!(saiu.is_empty());
        assert_eq!(e.parada, Parada::SemTrilha);
        assert!(std::fs::read_dir(&d).unwrap().next().is_none());
    }

    /// Registro estragado num volume que ia sair: o expurgo PARA com erro, e
    /// nada sai. Um carimbo que nao confere nao decide apagamento.
    #[test]
    fn registro_estragado_para_o_expurgo_e_nada_sai() {
        let d = temp("expurgo-estragado");
        let t = trilha_com(
            &d,
            &[&["2010-01-01", "2010-01-02", "2010-01-03"], &["2021-01-01"]],
        );
        let caminho = d.join("t_001.lgpd");
        let mut bruto = std::fs::read(&caminho).unwrap();
        // Um byte do corpo do segundo registro.
        bruto[64 + TAMANHO + REGISTRO_CAB + 10] ^= 0x01;
        std::fs::write(&caminho, &bruto).unwrap();
        let mut t2 = TrilhaFile::abrir(&d, "t", paginada()).unwrap();
        let r = t2.planejar_expurgo(crate::util::agora_ms());
        assert!(
            matches!(r, Err(PhxError::Corrompido(_))),
            "o plano aceitou registro estragado: {r:?}"
        );
        assert_eq!(no_disco(&d).len(), 2);
        drop(t);
    }

    /// A fase 2 vai ao disco de verdade: o selo de um expurgo com volume
    /// conta UM `fsync` confirmado, e o de um expurgo vazio nenhum -- e sem
    /// rastro gravado, recusa.
    ///
    /// **Defeito reposto** (tirar o `onde.sincronizar()?` do `selar`): o
    /// contador fica em zero e a primeira asserção cai.
    #[test]
    fn selar_leva_o_rastro_ao_disco_e_recusa_sem_rastro() {
        let d = temp("expurgo-selo");
        let mut t = trilha_com(
            &d,
            &[&["2010-01-01", "2010-01-02", "2010-01-03"], &["2021-01-01"]],
        );
        let e = t.planejar_expurgo(em("2015-01-01")).unwrap();
        assert!(
            e.clone().selar().is_err(),
            "selou um expurgo com volume e sem rastro"
        );
        let selado = com_rastro(&d, e).selar().unwrap();
        assert_eq!(selado.sincronizados(), 1);
        let vazio = t
            .planejar_expurgo(em("2000-01-01"))
            .unwrap()
            .selar()
            .unwrap();
        assert_eq!(vazio.sincronizados(), 0);
    }

    /// O rastro diz volumes, contagem e instantes -- e NENHUMA chave de linha,
    /// embora cada registro que sai carregue a sua.
    #[test]
    fn o_rastro_nao_carrega_chave_de_linha() {
        let d = temp("expurgo-rastro");
        let mut t = trilha_com(
            &d,
            &[
                &["2010-01-01", "2010-01-02", "2010-01-03"],
                &["2010-02-01", "2010-02-02", "2010-02-03"],
                &["2021-01-01"],
            ],
        );
        let e = t.planejar_expurgo(em("2015-01-01")).unwrap();
        let rastro = e.rastro();
        assert!(rastro.starts_with(".lgpd volumes 1-2 "), "{rastro}");
        assert!(rastro.contains("6 registro(s)"), "{rastro}");
        assert!(rastro.contains("limite 2015-01-01"), "{rastro}");
        assert!(!rastro.contains("cpf"), "o rastro vazou a chave: {rastro}");
        assert!(!rastro.contains("00000000001"), "{rastro}");
    }

    #[test]
    fn faixas_juntam_o_que_e_seguido() {
        assert_eq!(faixas([1, 2, 3, 7, 9, 10].into_iter()), "1-3,7,9-10");
        assert_eq!(faixas([5].into_iter()), "5");
        assert_eq!(faixas(std::iter::empty()), "");
    }
}

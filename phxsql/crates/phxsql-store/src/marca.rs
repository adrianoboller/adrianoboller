//! A marca `transacao_<id>.tx` e a recuperacao que a completa.
//!
//! # Por que mora no store, e nao no servidor
//!
//! Pedido 563. A marca nasceu no servidor (`BEGIN`/`COMMIT`, e depois a
//! cascata solta do pedido 540), e o embutido -- o FFI e o CLI, que chamam o
//! `Table::atualizar` sem servidor nenhum -- cascateava o `ao_alterar` SEM
//! marca: uma queda no meio deixava a filha na chave velha, e o arranque do
//! 522 reconstruia o indice dela com a orfa dentro. Levar a marca ao embutido
//! copiando o motor seria a copia que diverge; entao o MOTOR mudou de casa, e
//! o servidor reexporta e chama o que esta aqui. Fica no servidor so o que e
//! dele: a passada com gatilhos, a janela de durabilidade, as residentes e o
//! contador de id.
//!
//! O formato e o de sempre (`docs/FORMATO.md` §16): mudar de casa nao mudou
//! um byte.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use phxsql_core::cifra::XNONCE_LEN;
use phxsql_core::crc::crc32;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::uuid::{Uuid, Uuid256};
use phxsql_core::value::Value;

use crate::catalogo::Database;
use crate::cofre::{self, Material};
use crate::table::{nome_simples, MaesEmProgresso, Sobreposicao, Table};

/// A assinatura do arquivo de marca. Oito bytes, como todo arquivo do motor.
pub const MAGIC: &[u8; 8] = b"PHXTX\0\0\0";

/// A versao mais nova do formato da marca. Ver `docs/FORMATO.md` §16.
///
/// A **v4** (pedido 354) acrescenta o material de cifra no cabecalho e sela o
/// payload de **cada operacao**. Ela so e escrita quando o cofre esta ligado:
/// com ele desligado a marca continua nascendo [`VERSAO_CASCATA_EM_CLARO`],
/// byte por byte como antes, porque guarda nova entra pedida e nao imposta --
/// e porque um servidor anterior continua sabendo ler a marca de quem nunca
/// pediu cifra.
///
/// As tres anteriores **continuam sendo lidas**: marca deixada por um servidor
/// anterior e commit que ja comecou, e descarta-la seria jogar fora uma
/// transacao confirmada por causa de uma mudanca nossa.
pub const VERSAO: u32 = 4;

/// A v3 (ACID-C): cascata ACHATADA na lista, cabecalho **sem** material de
/// cifra. E a versao que a marca tem quando o cofre esta desligado.
///
/// Ela muda a SEMANTICA da cascata: a mae e cada filha sao uma operacao da
/// marca, e a operacao carrega o byte `cascata_na_lista` dizendo «aplique-me
/// SEM cascatear, os elos ja estao aqui».
pub const VERSAO_CASCATA_EM_CLARO: u32 = 3;

/// A v2: linha antiga presente, cascata IMPLICITA (replanejada na reaplicacao).
/// Ainda aceita na leitura.
pub const VERSAO_LINHA_ANTIGA_SEM_CASCATA: u32 = 2;

/// A primeira versao do formato, ainda aceita na leitura.
pub const VERSAO_SEM_LINHA_ANTIGA: u32 = 1;

/// Quanto o cabecalho ocupa ate o CRC, nas versoes 1 a 3: magic, versao, id,
/// carimbo e o numero de operacoes.
const CAB_ATE_CRC: usize = 8 + 4 + 8 + 8 + 4;

/// Onde o material de cifra entra, na v4: logo depois do numero de operacoes.
const MATERIAL_EM: usize = CAB_ATE_CRC;

/// Quanto o cabecalho da v4 ocupa ate o CRC: o mesmo de antes, mais o material.
const CAB_ATE_CRC_CIFRADA: usize = CAB_ATE_CRC + cofre::MATERIAL_LEN;

/// A parte ESTAVEL do cabecalho que a prova da chave amarra: magic, versao e
/// id.
///
/// O id entra de proposito, e ele e o mesmo do NOME do arquivo: assim a prova
/// so fecha na marca que nasceu com aquele nome, e renomear
/// `transacao_7.tx` para `transacao_8.tx` deixa de ser uma troca invisivel.
/// Ficam de fora o carimbo e o contador, pela mesma regra do resto da casa --
/// prova amarra o que identifica o arquivo, nao o que ele conta.
const ROTULO: usize = 8 + 4 + 8;

/// O prefixo do nome do arquivo de marca, dentro do diretorio do database.
pub const PREFIXO: &str = "transacao_";
/// A extensao do arquivo de marca.
pub const EXTENSAO: &str = "tx";

// ------------------------------------------------------------- as operacoes

/// O que uma escrita empilhada faz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acao {
    Inserir,
    Atualizar,
    ExcluirSuave,
    ExcluirDeVez,
    Restaurar,
}

impl Acao {
    pub fn tag(self) -> u8 {
        match self {
            Acao::Inserir => 1,
            Acao::Atualizar => 2,
            Acao::ExcluirSuave => 3,
            Acao::ExcluirDeVez => 4,
            Acao::Restaurar => 5,
        }
    }

    pub fn de_tag(t: u8) -> Option<Acao> {
        Some(match t {
            1 => Acao::Inserir,
            2 => Acao::Atualizar,
            3 => Acao::ExcluirSuave,
            4 => Acao::ExcluirDeVez,
            5 => Acao::Restaurar,
            _ => return None,
        })
    }

    pub fn nome(self) -> &'static str {
        match self {
            Acao::Inserir => "inserir",
            Acao::Atualizar => "atualizar",
            Acao::ExcluirSuave => "excluir",
            Acao::ExcluirDeVez => "excluir_de_vez",
            Acao::Restaurar => "restaurar",
        }
    }
}

/// Uma escrita empilhada, na ordem em que foi pedida.
#[derive(Debug, Clone)]
pub struct Escrita {
    /// `database/tabela_qualificada`, para a passada saber onde aplicar.
    pub database: String,
    pub tabela: String,
    pub acao: Acao,
    /// O slot que esta operacao VAI escrever.
    ///
    /// Para o `atualizar` e o `excluir` e o rowid que veio no pedido. Para o
    /// `inserir` e o rowid que o `.reg` vai atribuir -- previsivel porque ele
    /// sempre anexa no fim e porque a tabela esta reservada por esta
    /// transacao. E essa previsibilidade que torna a recuperacao exata.
    pub rowid: u64,
    /// A linha ja tipada. Vazia no `excluir` e no `restaurar`.
    pub linha: Vec<Value>,
    /// A linha como o DISCO a tem, antes desta transacao. So no `atualizar`.
    ///
    /// Ela existe por um motivo unico e medido: a cascata do `ao_alterar` e
    /// planejada pelo delta da mae, e a reaplicacao nao tem delta -- a mae ja
    /// pode estar no valor de destino. Sem a linha antiga a recuperacao nao
    /// consegue nem PERGUNTAR quais filhas ficaram para tras.
    ///
    /// Vem do disco, e nao da visao da transacao, e isso e a resposta certa:
    /// a unica operacao que precisa dela na reaplicacao e a PRIMEIRA a tocar
    /// aquela linha, e para essa o valor de disco e o valor de antes. Da
    /// segunda em diante o `atualizar` da reaplicacao ja acha delta de
    /// verdade e cascateia sozinho.
    pub linha_antiga: Vec<Value>,
    /// O motivo da exclusao ou da restauracao. Vazio no resto.
    pub motivo: String,
    /// ACID-C: esta escrita e um elo de uma cascata do `ao_alterar` que ja foi
    /// ACHATADA nesta lista -- ou a mae dela. Aplique-a SEM cascatear: os elos
    /// (filha, neta...) sao escritas proprias, logo adiante na lista.
    ///
    /// Falso em toda escrita comum (insercao, exclusao, e o `atualizar` que nao
    /// mexe em chave conferida), e ai a passada cascateia como antes -- que, sem
    /// filha, e um `is_empty()` de graca. Ver `docs/ACID.md` §2.4.
    pub cascata_na_lista: bool,
    /// Pedido 537: este e um ELO planejado no `empilhar`, e a `linha` dele e
    /// a filha como ela estava ENTAO, com a chave nova. O que vale dele e so a
    /// chave: o COMMIT o refaz sobre a linha ATUAL antes da marca
    /// (`Servidor::refazer_o_elo`), e a coluna que outra conexao mudou na
    /// filha nesse meio tempo nao volta ao valor velho.
    ///
    /// So em memoria, e de proposito: a marca recebe a linha ja refeita, e a
    /// passada e a recuperacao aplicam a linha inteira como sempre -- o
    /// formato nao muda. Falso em todo o resto, inclusive no elo que o COMMIT
    /// acrescenta, que ja nasce sobre a linha atual.
    pub elo_do_empilhar: bool,
    /// Pedido 562: esta escrita e um ELO que a cascata do `ao_alterar` pos na
    /// lista (a filha, a neta...) -- e nao a mae nem uma escrita do cliente.
    /// A passada nao junta o AFTER dela: a cascata nao dispara gatilho
    /// (`docs/INTEGRIDADE.md` §7.3), e a decisao mora num lugar so para a
    /// alteracao solta e o COMMIT darem o mesmo resultado.
    ///
    /// So em memoria, como o `elo_do_empilhar`: a recuperacao nao roda
    /// gatilho nenhum, entao a marca nao precisa saber -- o formato nao muda.
    pub elo_da_cascata: bool,
}

// ------------------------------------------------------- a linha em bytes

// As etiquetas de cada variante de `Value`. Numero fixo para sempre, pela
// mesma regra do codigo de erro: etiqueta que muda de significado quebra a
// leitura de uma marca gravada por uma versao anterior -- e a marca so e lida
// justamente no dia em que o processo caiu, que e o pior dia para descobrir.
const T_NULL: u8 = 0;
const T_BOOL: u8 = 1;
const T_INT: u8 = 2;
const T_UINT: u8 = 3;
const T_REAL: u8 = 4;
const T_DECIMAL: u8 = 5;
const T_DATE: u8 = 6;
const T_TIME: u8 = 7;
const T_DATETIME: u8 = 8;
const T_STR: u8 = 9;
const T_BIN: u8 = 10;
const T_MEMO: u8 = 11;
const T_UUID: u8 = 12;
const T_UUID256: u8 = 13;

/// A linha em bytes, para a marca `.tx`.
///
/// # Por que uma codificacao propria, e nao JSON
///
/// Porque JSON PERDE aqui, e isso foi medido no proprio codigo: o
/// `valor_para_json` escreve `Time` e `DateTime` como texto ISO, e o
/// `json_para_valor` desses dois so aceita numero. A volta nao fecha, e uma
/// recuperacao que reconstroi a linha errada e pior do que uma que nao
/// reconstroi nada. Aqui a etiqueta manda, e a volta e exata por construcao --
/// ha teste de ida e volta para as catorze variantes.
pub fn codificar_linha(linha: &[Value]) -> Vec<u8> {
    let mut b = Vec::with_capacity(linha.len() * 12);
    b.extend_from_slice(&(linha.len() as u32).to_le_bytes());
    for v in linha {
        match v {
            Value::Null => b.push(T_NULL),
            Value::Bool(x) => {
                b.push(T_BOOL);
                b.push(*x as u8);
            }
            Value::Int(x) => {
                b.push(T_INT);
                b.extend_from_slice(&x.to_le_bytes());
            }
            Value::UInt(x) => {
                b.push(T_UINT);
                b.extend_from_slice(&x.to_le_bytes());
            }
            Value::Real(x) => {
                b.push(T_REAL);
                b.extend_from_slice(&x.to_bits().to_le_bytes());
            }
            Value::Decimal(x) => {
                b.push(T_DECIMAL);
                b.extend_from_slice(&x.to_le_bytes());
            }
            Value::Date(x) => {
                b.push(T_DATE);
                b.extend_from_slice(&x.to_le_bytes());
            }
            Value::Time(x) => {
                b.push(T_TIME);
                b.extend_from_slice(&x.to_le_bytes());
            }
            Value::DateTime(x) => {
                b.push(T_DATETIME);
                b.extend_from_slice(&x.to_le_bytes());
            }
            Value::Str(s) => texto(&mut b, T_STR, s.as_bytes()),
            Value::Memo(s) => texto(&mut b, T_MEMO, s.as_bytes()),
            Value::Bin(x) => texto(&mut b, T_BIN, x),
            Value::Uuid(u) => {
                b.push(T_UUID);
                b.extend_from_slice(u.bytes());
            }
            Value::Uuid256(u) => {
                b.push(T_UUID256);
                b.extend_from_slice(u.bytes());
            }
        }
    }
    b
}

fn texto(b: &mut Vec<u8>, tag: u8, bytes: &[u8]) {
    b.push(tag);
    b.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    b.extend_from_slice(bytes);
}

/// A volta da [`codificar_linha`].
pub fn decodificar_linha(b: &[u8]) -> Result<Vec<Value>> {
    decodificar_linha_em(b).map(|(l, _)| l)
}

/// A volta da [`codificar_linha`], dizendo tambem quantos bytes consumiu.
///
/// O tamanho consumido e o que permite guardar a linha e o motivo no MESMO
/// bloco, com um CRC so cobrindo os dois -- sem um segundo contador no
/// formato para as duas partes sairem de sincronia um dia.
pub fn decodificar_linha_em(b: &[u8]) -> Result<(Vec<Value>, usize)> {
    let mut leitor = Leitor { b, i: 0 };
    let n = leitor.u32()? as usize;
    let mut linha = Vec::with_capacity(n.min(4096));
    for _ in 0..n {
        let tag = leitor.u8()?;
        linha.push(match tag {
            T_NULL => Value::Null,
            T_BOOL => Value::Bool(leitor.u8()? != 0),
            T_INT => Value::Int(i64::from_le_bytes(leitor.fixo::<8>()?)),
            T_UINT => Value::UInt(u64::from_le_bytes(leitor.fixo::<8>()?)),
            T_REAL => Value::Real(f64::from_bits(u64::from_le_bytes(leitor.fixo::<8>()?))),
            T_DECIMAL => Value::Decimal(i128::from_le_bytes(leitor.fixo::<16>()?)),
            T_DATE => Value::Date(i32::from_le_bytes(leitor.fixo::<4>()?)),
            T_TIME => Value::Time(i32::from_le_bytes(leitor.fixo::<4>()?)),
            T_DATETIME => Value::DateTime(i64::from_le_bytes(leitor.fixo::<8>()?)),
            T_STR => Value::Str(leitor.texto()?),
            T_MEMO => Value::Memo(leitor.texto()?),
            T_BIN => Value::Bin(leitor.bytes()?.to_vec()),
            T_UUID => Value::Uuid(Uuid::de_bytes(leitor.fixo::<16>()?)),
            T_UUID256 => Value::Uuid256(Uuid256::de_bytes(leitor.fixo::<32>()?)),
            outro => {
                return Err(PhxError::Corrompido(format!(
                    "etiqueta de valor {outro} desconhecida na marca de transacao"
                )))
            }
        });
    }
    Ok((linha, leitor.i))
}

struct Leitor<'a> {
    b: &'a [u8],
    i: usize,
}

impl Leitor<'_> {
    fn faltou(&self, quanto: usize) -> PhxError {
        PhxError::Corrompido(format!(
            "marca de transacao truncada: faltam {quanto} bytes a partir de {}",
            self.i
        ))
    }
    fn u8(&mut self) -> Result<u8> {
        let v = *self.b.get(self.i).ok_or_else(|| self.faltou(1))?;
        self.i += 1;
        Ok(v)
    }
    fn fixo<const N: usize>(&mut self) -> Result<[u8; N]> {
        if self.i + N > self.b.len() {
            return Err(self.faltou(N));
        }
        let mut a = [0u8; N];
        a.copy_from_slice(&self.b[self.i..self.i + N]);
        self.i += N;
        Ok(a)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.fixo::<4>()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.fixo::<8>()?))
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.fixo::<2>()?))
    }
    fn bytes(&mut self) -> Result<&[u8]> {
        let n = self.u32()? as usize;
        if self.i + n > self.b.len() {
            return Err(self.faltou(n));
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
    fn texto(&mut self) -> Result<String> {
        let s = self.bytes()?;
        String::from_utf8(s.to_vec())
            .map_err(|_| PhxError::Corrompido("texto invalido na marca de transacao".into()))
    }
}

// -------------------------------------------------------------- a marca .tx

/// Uma operacao lida de volta de uma marca.
#[derive(Debug, Clone)]
pub struct OperacaoDaMarca {
    pub tabela: String,
    pub acao: Acao,
    pub rowid: u64,
    pub linha: Vec<Value>,
    /// Vazia na marca v1 e em tudo que nao e `atualizar`.
    pub linha_antiga: Vec<Value>,
    pub motivo: String,
    /// ACID-C (v3): esta operacao aplica SEM refazer a cascata, porque os elos
    /// dela ja sao operacoes proprias desta marca. Falso em marca v1/v2 -- ali
    /// a cascata e IMPLICITA e o `recascatear` da reaplicacao a refaz.
    pub cascata_na_lista: bool,
}

/// A marca inteira, lida de volta.
#[derive(Debug)]
pub struct Marca {
    pub id: u64,
    pub carimbo_ms: i64,
    pub operacoes: Vec<OperacaoDaMarca>,
}

/// O caminho da marca desta transacao, dentro do diretorio do database.
pub fn caminho_da_marca(diretorio: &Path, id: u64) -> PathBuf {
    diretorio.join(format!("{PREFIXO}{id}.{EXTENSAO}"))
}

/// O material de cifra das marcas deste processo, derivado UMA vez.
///
/// `None` quer dizer «ainda nao derivei», e nao «em claro»: quem decide se ha
/// cifra e o cofre, conferido a cada marca por [`material_da_marca`].
static MATERIAL: Mutex<Option<Material>> = Mutex::new(None);

/// O material de cifra que esta marca usa.
///
/// # Por que UM por processo, e nao um por marca
///
/// Porque `Material::novo` sorteia um sal novo a cada chamada, e sal novo e
/// PBKDF2 de verdade: o cache de chaves do cofre e por (sal, iteracoes) e
/// nunca acertaria. **Medido nesta maquina, em release, com as 210.000
/// iteracoes do padrao:** um sal novo custa **236,4 ms**, e a marca inteira
/// custa **0,32 ms** em claro. Sal por marca cobraria isso a cada `COMMIT` --
/// **740 vezes** o custo da marca -- e trocaria o desenho inteiro da transacao
/// por confidencialidade. No `.reg` o sal por arquivo e de graca porque tabela
/// nasce uma vez; marca nasce sempre.
///
/// Com a chave derivada, a marca cifrada custa **0,298 ms** contra os 0,317 ms
/// da mesma marca em claro -- dentro do ruido das repeticoes. O selo em si nao
/// aparece; o que aparecia era o PBKDF2.
///
/// # E o que a troca custa em seguranca, que e nada
///
/// O par (chave, nonce) e a unica coisa que nao pode repetir, e as duas pontas
/// fecham sem sal por marca: o nonce de [`nonce_da_operacao`] carrega o **id
/// da transacao**, que `Transacoes::abrir` faz crescer e nunca reemite dentro
/// de um processo, e dois processos sorteiam sais diferentes -- logo chaves
/// diferentes. O que o sal por arquivo compraria aqui ja esta comprado pelo
/// contador que a transacao tem de qualquer jeito.
///
/// # A janela que sobra, nomeada
///
/// Trocar a senha do cofre com o processo DE PE nao troca este material, porque
/// a chave ja esta derivada aqui dentro. A marca escrita depois disso abre com
/// a senha velha, e o arranque seguinte -- com a senha nova -- cai na terceira
/// resposta de [`ler_marca`]: **para e nao apaga**. E barulhento, que e o
/// oposto de perder a transacao em silencio. Hoje `CifraConfig::aplicar` so e
/// chamada no arranque, entao a janela nao se abre sozinha.
fn material_da_marca() -> Result<Material> {
    // O portao vem ANTES do trabalho: com o cofre desligado nao se toma trava
    // nem se deriva nada, e a marca em claro custa exatamente o que custava.
    if !cofre::ligado() {
        return Ok(Material::EM_CLARO);
    }
    let mut guarda = MATERIAL
        .lock()
        .map_err(|_| PhxError::Esquema("a trava do material da marca ficou envenenada".into()))?;
    if let Some(m) = *guarda {
        return Ok(m);
    }
    let novo = Material::novo()?;
    *guarda = Some(novo);
    Ok(novo)
}

/// O nonce da operacao `i` da marca `id`.
///
/// # Por que o indice basta, e nao ha byte sorteado por operacao
///
/// Repetir o par (chave, nonce) e o unico jeito de quebrar isto sem quebrar a
/// matematica, e aqui as tres coordenadas ja separam tudo o que existe: o
/// **indice** separa duas operacoes da mesma marca, o **id** separa duas
/// marcas do mesmo processo (ele nunca reemite), e o **sal** de
/// [`material_da_marca`] separa dois processos. Guardar um tempero sorteado
/// por operacao custaria oito bytes por linha para separar o que ja esta
/// separado.
fn nonce_da_operacao(id: u64, i: usize) -> [u8; XNONCE_LEN] {
    // `quem` e `contador` sao do `.reg`, onde um pedaco se reescreve no lugar;
    // aqui o slot nasce e morre com o arquivo, entao ficam em zero.
    cofre::nonce_de_pedaco(i as u64, 0, 0, id)
}

/// O dado associado que amarra o pedaco selado ao lugar dele.
///
/// Tabela, acao e rowid continuam viajando **em claro** -- e preciso saber
/// onde reaplicar antes de abrir o que se reaplica --, e ate aqui so o CRC os
/// protegia. CRC nao e selo: quem edita o arquivo recalcula os quatro bytes e
/// ninguem percebe. Como dado associado eles entram na etiqueta: um rowid
/// trocado de 7 para 8 deixa de abrir, em vez de reaplicar a linha certa no
/// slot errado.
fn aad_da_operacao(id: u64, tabela: &[u8], tag: u8, rowid: u64) -> Vec<u8> {
    let mut a = Vec::with_capacity(tabela.len() + 17);
    a.extend_from_slice(&id.to_le_bytes());
    a.extend_from_slice(tabela);
    a.push(tag);
    a.extend_from_slice(&rowid.to_le_bytes());
    a
}

/// Cria a marca NOVA e fechada para o resto da maquina -- 0600 no Unix; no
/// Windows vale a ACL da pasta, como sempre valeu.
///
/// A permissao vai na CRIACAO, e nao depois: entre criar aberta e apertar ha
/// uma janela em que qualquer conta le a linha inteira, e essa janela e a
/// unica coisa que este arquivo existe para nao ter. E o mesmo
/// `create_new + mode(0o600)` do `config.rs`.
///
/// O `create_new` faz parte da garantia, e por dois motivos. O primeiro e o
/// daquele arquivo: `mode` so vale para o que NASCE aqui, e um arquivo que ja
/// existisse entraria com a permissao que tinha. O segundo e mais forte e e
/// nosso: uma marca ja no disco com este id e um `COMMIT` esperando
/// recuperacao, e truncar por cima dela apagaria a intencao de uma transacao
/// que ja aconteceu.
///
/// O modo sai do [`crate::util::opcoes_do_banco`] desde que a marca mudou
/// para o store (pedido 563): ali mora a decisao do 0600 de todo arquivo do
/// banco (pedido 542), e um `mode(0o600)` escrito aqui seria a segunda copia
/// dela.
fn criar_privado(caminho: &Path, id: u64) -> Result<std::fs::File> {
    let mut opcoes = crate::util::opcoes_do_banco();
    opcoes.write(true).create_new(true);
    opcoes.open(caminho).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            // O erro cru aqui e "File exists", que manda procurar problema de
            // disco. O que ha e outra coisa, e quem le precisa saber qual.
            PhxError::Esquema(format!(
                "ja existe a marca da transacao {id} em {}: ela e um COMMIT que \
                 espera recuperacao, e grava-la por cima apagaria a intencao dele -- \
                 suba o servidor, ou reabra a base no embutido, para a recuperacao completa-la \
                 antes de tentar de novo",
                caminho.display()
            ))
        } else {
            PhxError::Io(e)
        }
    })
}

/// Grava a marca e **sincroniza**, antes de a passada tocar em qualquer
/// arquivo de dado.
///
/// # Por que a ordem e esta
///
/// E a mesma da lixeira, e pelo mesmo motivo escrito la: grava e sincroniza a
/// INTENCAO antes de mexer no alvo, porque *«a ordem inversa tem uma janela em
/// que o registro nao existe em lugar nenhum, e essa janela nao tem conserto
/// depois.»*
pub fn gravar_marca(
    diretorio: &Path,
    id: u64,
    carimbo_ms: i64,
    ops: &[Escrita],
) -> Result<PathBuf> {
    // O material e conferido AQUI, na criacao, e vale para a marca inteira.
    // Com o cofre desligado ele e `EM_CLARO`, e ai a marca nasce v3 -- os
    // mesmos bytes de antes, para quem nunca pediu cifra.
    let material = material_da_marca()?;
    let versao = if material.cifrado() {
        VERSAO
    } else {
        VERSAO_CASCATA_EM_CLARO
    };

    let mut b = Vec::with_capacity(4096);
    b.extend_from_slice(MAGIC);
    b.extend_from_slice(&versao.to_le_bytes());
    b.extend_from_slice(&id.to_le_bytes());
    b.extend_from_slice(&carimbo_ms.to_le_bytes());
    b.extend_from_slice(&(ops.len() as u32).to_le_bytes());
    if material.cifrado() {
        // O rotulo sai copiado porque `gravar` precisa do buffer emprestado
        // por inteiro -- sao os 20 bytes de magic, versao e id.
        let rotulo = b[..ROTULO].to_vec();
        b.resize(CAB_ATE_CRC_CIFRADA, 0);
        material.gravar(&mut b, MATERIAL_EM, &rotulo);
    }
    b.extend_from_slice(&crc32(&b).to_le_bytes());

    for (i, e) in ops.iter().enumerate() {
        let inicio = b.len();
        let nome = e.tabela.as_bytes();
        b.extend_from_slice(&(nome.len() as u16).to_le_bytes());
        b.extend_from_slice(nome);
        b.push(e.acao.tag());
        b.extend_from_slice(&e.rowid.to_le_bytes());
        let mut payload = codificar_linha(&e.linha);
        let motivo = e.motivo.as_bytes();
        payload.extend_from_slice(&(motivo.len() as u32).to_le_bytes());
        payload.extend_from_slice(motivo);
        // A linha antiga vai no FIM do payload, e nao entre os campos que ja
        // existiam: assim o leitor da v1 e o da v2 percorrem os mesmos bytes
        // ate aqui, e o CRC continua cobrindo o bloco inteiro de uma vez.
        payload.extend_from_slice(&codificar_linha(&e.linha_antiga));
        // ACID-C (v3): o byte da cascata vai DEPOIS da linha antiga, pelo mesmo
        // motivo -- o leitor da v1/v2 nunca chega ate aqui, e o CRC continua
        // cobrindo o payload inteiro de uma vez.
        payload.push(u8::from(e.cascata_na_lista));
        // O selo e POR OPERACAO, no MESMO bloco que o CRC ja cobria: a unidade
        // de dano continua sendo a operacao, e nao o arquivo. Selar a marca
        // inteira criaria uma segunda unidade de falha, maior do que a que o
        // formato ja tem -- e ai uma etiqueta que nao fechasse custaria a
        // transacao toda onde hoje custa o que o CRC daquele bloco custa.
        let guardado = material.selar(
            &nonce_da_operacao(id, i),
            &aad_da_operacao(id, nome, e.acao.tag(), e.rowid),
            &payload,
        );
        b.extend_from_slice(&(guardado.len() as u32).to_le_bytes());
        b.extend_from_slice(&guardado);
        let crc = crc32(&b[inicio..]);
        b.extend_from_slice(&crc.to_le_bytes());
    }

    let caminho = caminho_da_marca(diretorio, id);
    let mut f = criar_privado(&caminho, id)?;
    // O `sync_all` e a peca, e nao um detalhe: sem ele a marca pode estar so
    // no cache do sistema quando a passada comecar, e a queda deixaria o dado
    // meio gravado sem nenhuma intencao no disco para completar.
    //
    // E ele passa pelo motor do pedido 509 (pedido 503, item 3): o `fsync`
    // recusado derruba o servidor ali mesmo, como o PANIC do PostgreSQL no
    // WAL. Antes, o `sync_all` cru que falhava deixava a marca INTEIRA no
    // disco enquanto o COMMIT respondia «abortada», e o arranque seguinte a
    // aplicava por cima de gravacao mais nova. Onde nao ha gancho (a
    // biblioteca), a marca que nao se confirmou SAI do disco antes do erro.
    let feito = f
        .write_all(&b)
        .map_err(PhxError::from)
        .and_then(|()| crate::sincronia::sync_all(&f, &caminho));
    if let Err(e) = feito {
        drop(f);
        let _ = std::fs::remove_file(&caminho);
        return Err(e);
    }
    Ok(caminho)
}

/// O que a leitura de uma marca pode responder. **Sao tres, e a terceira e a
/// peca.**
///
/// Duas respostas bastavam enquanto a marca era sempre legivel: ou ela
/// confere, ou nao confere. Com o selo do pedido 354 nasce um terceiro caso
/// que **nao e nenhum dos dois** -- a marca esta inteira, o CRC fecha, e
/// mesmo assim este servidor nao consegue abri-la. Trata-lo como «nao
/// confere» apagaria uma transacao confirmada por falta de uma senha, que e
/// trocar confidencialidade por durabilidade -- o oposto do que o selo foi
/// por.
#[derive(Debug)]
pub enum Leitura {
    /// A marca confere e abriu. Reaplique-a e depois apague-a.
    Aberta(Marca),
    /// CRC, assinatura, versao ou tamanho nao fecham: um commit que **nunca
    /// comecou**, porque a marca e sincronizada inteira antes de qualquer
    /// escrita. Pode apagar; o disco continua como estava.
    NaoConfere,
    /// Cifrada, e esta chave nao a abre. **PARA, e NAO apaga.**
    ///
    /// O texto diz qual dos casos e -- nao ha chave nenhuma no cofre, ou a
    /// que ha nao e a que gravou o arquivo. Os dois pedem a mesma coisa de
    /// quem opera (ponha a senha certa e suba de novo) e os dois proibem a
    /// mesma coisa (apagar).
    SemChave(String),
}

impl Leitura {
    /// A marca, quando ela abriu. `None` nas outras duas respostas.
    pub fn marca(self) -> Option<Marca> {
        match self {
            Leitura::Aberta(m) => Some(m),
            _ => None,
        }
    }
}

/// Le a marca de volta. Ver [`Leitura`] para as tres respostas.
pub fn ler_marca(caminho: &Path) -> Result<Leitura> {
    #[cfg(debug_assertions)]
    if LEITURA_FALHA_DE_TESTE.with(|f| f.replace(false)) {
        return Err(std::io::Error::other("releitura da marca falhou (teste)").into());
    }
    let b = std::fs::read(caminho)?;
    if b.len() < CAB_ATE_CRC + 4 || &b[..8] != MAGIC {
        return Ok(Leitura::NaoConfere);
    }
    // A versao tem de ser lida ANTES do CRC do cabecalho, e nao depois: na v4
    // o material de cifra entrou entre o contador e o CRC, entao e a versao
    // que diz onde o CRC esta.
    let versao = u32::from_le_bytes([b[8], b[9], b[10], b[11]]);
    let ate_crc = match versao {
        VERSAO => CAB_ATE_CRC_CIFRADA,
        VERSAO_CASCATA_EM_CLARO | VERSAO_LINHA_ANTIGA_SEM_CASCATA | VERSAO_SEM_LINHA_ANTIGA => {
            CAB_ATE_CRC
        }
        _ => return Ok(Leitura::NaoConfere),
    };
    if b.len() < ate_crc + 4 {
        return Ok(Leitura::NaoConfere);
    }
    let crc_lido = u32::from_le_bytes([b[ate_crc], b[ate_crc + 1], b[ate_crc + 2], b[ate_crc + 3]]);
    if crc32(&b[..ate_crc]) != crc_lido {
        return Ok(Leitura::NaoConfere);
    }
    let nome_do_arquivo = caminho.display().to_string();
    let material = if versao == VERSAO {
        match Material::ler(&b, MATERIAL_EM, &nome_do_arquivo, &b[..ROTULO]) {
            Ok(m) => m,
            // A terceira resposta. Qualquer recusa daqui vem de uma marca que
            // se declarou CIFRADA -- sem a flag, `Material::ler` devolve
            // `EM_CLARO` sem nem tocar no cofre --, e entao nao ha como saber
            // se o que esta dentro presta. Os dois erros errados nao custam o
            // mesmo: parar numa marca podre enche o relatorio; apagar uma
            // marca boa apaga uma transacao confirmada.
            Err(e) => return Ok(Leitura::SemChave(e.to_string())),
        }
    } else {
        Material::EM_CLARO
    };

    let mut leitor = Leitor { b: &b, i: 12 };
    let id = leitor.u64()?;
    let carimbo_ms = i64::from_le_bytes(leitor.fixo::<8>()?);
    let n = leitor.u32()? as usize;
    // Pula o material (quando ha) e o CRC do cabecalho de uma vez so.
    leitor.i = ate_crc + 4;

    let mut operacoes = Vec::with_capacity(n.min(65_536));
    for i in 0..n {
        let inicio = leitor.i;
        let Ok(tam) = leitor.u16() else {
            return Ok(Leitura::NaoConfere);
        };
        let Ok(nome) = ler_exato(&mut leitor, tam as usize) else {
            return Ok(Leitura::NaoConfere);
        };
        let Ok(tag) = leitor.u8() else {
            return Ok(Leitura::NaoConfere);
        };
        let Some(acao) = Acao::de_tag(tag) else {
            return Ok(Leitura::NaoConfere);
        };
        let Ok(rowid) = leitor.u64() else {
            return Ok(Leitura::NaoConfere);
        };
        let Ok(guardado) = leitor.bytes().map(<[u8]>::to_vec) else {
            return Ok(Leitura::NaoConfere);
        };
        let fim = leitor.i;
        let Ok(crc) = leitor.u32() else {
            return Ok(Leitura::NaoConfere);
        };
        if crc32(&b[inicio..fim]) != crc {
            return Ok(Leitura::NaoConfere);
        }
        // Abre o selo desta operacao. A chave JA se provou certa no cabecalho,
        // entao etiqueta que nao fecha aqui e dado alterado -- a mesma
        // resposta do CRC quebrado, e nao a terceira.
        let payload = match material.abrir(
            &nonce_da_operacao(id, i),
            &aad_da_operacao(id, &nome, tag, rowid),
            &guardado,
            &nome_do_arquivo,
        ) {
            Ok(p) => p,
            Err(_) => return Ok(Leitura::NaoConfere),
        };
        let Ok(tabela) = String::from_utf8(nome) else {
            return Ok(Leitura::NaoConfere);
        };
        // O payload e a linha seguida do motivo -- um bloco so, para o CRC
        // cobrir os dois de uma vez.
        let (linha, consumido) = match decodificar_linha_em(&payload) {
            Ok(v) => v,
            Err(_) => return Ok(Leitura::NaoConfere),
        };
        let mut m = Leitor {
            b: &payload,
            i: consumido,
        };
        let motivo = m.texto().unwrap_or_default();
        // A linha antiga existe da v2 em diante; a v1 nao a tem (e ali a
        // reaplicacao nao sabia replanejar a cascata). Guardo onde ela terminou
        // para achar o byte da cascata logo em seguida.
        let (linha_antiga, apos_antiga) = if versao >= VERSAO_LINHA_ANTIGA_SEM_CASCATA {
            match decodificar_linha_em(&payload[m.i..]) {
                Ok((v, consumido)) => (v, m.i + consumido),
                Err(_) => return Ok(Leitura::NaoConfere),
            }
        } else {
            (Vec::new(), m.i)
        };
        // ACID-C: o byte da cascata vem logo depois da linha antiga, e existe
        // da v3 em diante -- a v4 so acrescentou o selo, nao mexeu no payload.
        // Na v1/v2 ele nao existe, e a cascata e IMPLICITA (falso aqui, e o
        // `recascatear` da reaplicacao a refaz).
        let cascata_na_lista = versao >= VERSAO_CASCATA_EM_CLARO
            && payload.get(apos_antiga).is_some_and(|&byte| byte != 0);
        operacoes.push(OperacaoDaMarca {
            tabela,
            acao,
            rowid,
            linha,
            linha_antiga,
            motivo,
            cascata_na_lista,
        });
    }
    Ok(Leitura::Aberta(Marca {
        id,
        carimbo_ms,
        operacoes,
    }))
}

fn ler_exato(l: &mut Leitor<'_>, n: usize) -> Result<Vec<u8>> {
    if l.i + n > l.b.len() {
        return Err(l.faltou(n));
    }
    let v = l.b[l.i..l.i + n].to_vec();
    l.i += n;
    Ok(v)
}

// ------------------------------------------------------------ a recuperacao

/// O que a recuperacao achou e fez, para o relatorio do arranque.
///
/// **Cada linha daqui e medida.** O relatorio do capitulo tinha linha de
/// pagina refeita; aqui nao ha pagina suja confirmada para refazer, entao a
/// linha nao existe -- inventar uma que sempre imprime zero seria pior que
/// nao ter.
#[derive(Debug, Default)]
pub struct Relatorio {
    pub achadas: usize,
    pub descartadas: usize,
    pub completadas: usize,
    pub reaplicadas: u64,
    pub ja_aplicadas: u64,
    /// Indices que a queda deixou para tras e que a recuperacao reconstruiu.
    pub indices_reconstruidos: usize,
    /// Indices marcados que o arranque NAO conseguiu reconstruir (pedido
    /// 522): a tabela continua recusando, e cada linha diz qual e por que.
    pub indices_pendentes: Vec<String>,
    pub impossiveis: Vec<String>,
    /// Marcas CIFRADAS que este servidor nao conseguiu abrir. **Ficaram no
    /// disco**, e cada linha diz qual e por que -- ver [`Leitura::SemChave`].
    pub paradas: Vec<String>,
    /// Marcas que o ARRANQUE nao conseguiu LER -- erro de E/S, e nao marca
    /// que nao confere (pedido 503, 1a). **Ficaram no disco**, e o servidor
    /// nao sobe enquanto houver uma: a marca pode ser um commit confirmado, e
    /// apaga-la ou subir por cima dela sao as duas maneiras de perde-lo.
    pub sem_leitura: Vec<String>,
    pub ms: u64,
}

impl Relatorio {
    /// Junta o relatorio de um database ao da instancia -- o laco do arranque
    /// do servidor, que chama a [`Database::recuperar_marcas`] base a base.
    /// O tempo nao se soma: quem junta mede o laco inteiro.
    pub fn somar(&mut self, outro: Relatorio) {
        self.achadas += outro.achadas;
        self.descartadas += outro.descartadas;
        self.completadas += outro.completadas;
        self.reaplicadas += outro.reaplicadas;
        self.ja_aplicadas += outro.ja_aplicadas;
        self.indices_reconstruidos += outro.indices_reconstruidos;
        self.indices_pendentes.extend(outro.indices_pendentes);
        self.impossiveis.extend(outro.impossiveis);
        self.paradas.extend(outro.paradas);
        self.sem_leitura.extend(outro.sem_leitura);
    }

    pub fn houve(&self) -> bool {
        self.achadas > 0 || self.indices_reconstruidos > 0 || !self.indices_pendentes.is_empty()
    }

    /// Ha marca que o arranque nao leu: o servidor nao pode subir (503, 1a).
    pub fn impede_subir(&self) -> bool {
        !self.sem_leitura.is_empty()
    }

    /// O bloco que o arranque imprime.
    pub fn texto(&self, base: &Path) -> String {
        let mut s = format!(
            "PHXSQL Recovery -- base {}\n\
             \x20 transacoes achadas ............ {}\n\
             \x20 marcas ilegiveis descartadas .. {}   (commit que nunca comecou)\n\
             \x20 transacoes completadas ........ {}\n\
             \x20 operacoes reaplicadas ......... {}\n\
             \x20 operacoes ja aplicadas ........ {}\n",
            base.display(),
            self.achadas,
            self.descartadas,
            self.completadas,
            self.reaplicadas,
            self.ja_aplicadas
        );
        // So aparece quando ha: uma linha que imprime zero em toda subida
        // treina quem opera a nao ler o relatorio.
        if self.indices_reconstruidos > 0 {
            s.push_str(&format!(
                "\x20 indices reconstruidos ......... {}\n",
                self.indices_reconstruidos
            ));
        }
        if !self.indices_pendentes.is_empty() {
            s.push_str(&format!(
                "\x20 indices MARCADOS sem conserto .. {}   (a tabela recusa ate o `reindexar`)\n",
                self.indices_pendentes.len()
            ));
            for i in &self.indices_pendentes {
                s.push_str(&format!("     ! {i}\n"));
            }
        }
        if !self.impossiveis.is_empty() {
            s.push_str(&format!(
                "\x20 operacoes IMPOSSIVEIS ......... {}\n",
                self.impossiveis.len()
            ));
            for i in &self.impossiveis {
                s.push_str(&format!("     ! {i}\n"));
            }
        }
        // Barulhento de proposito, e com o caminho de cada uma: e a unica
        // linha deste relatorio que descreve trabalho PARADO esperando quem
        // opera, e nao trabalho ja resolvido.
        if !self.paradas.is_empty() {
            s.push_str(&format!(
                "\x20 marcas PARADAS sem a chave .... {}   (NAO foram apagadas)\n",
                self.paradas.len()
            ));
            for p in &self.paradas {
                s.push_str(&format!("     ! {p}\n"));
            }
        }
        if !self.sem_leitura.is_empty() {
            s.push_str(&format!(
                "\x20 marcas que NAO SE LERAM ....... {}   (NAO foram apagadas; o servidor NAO sobe)\n",
                self.sem_leitura.len()
            ));
            for p in &self.sem_leitura {
                s.push_str(&format!("     ! {p}\n"));
            }
        }
        s.push_str(&format!(
            "\x20 tempo ......................... {} ms",
            self.ms
        ));
        s
    }
}

#[cfg(debug_assertions)]
thread_local! {
    /// So em `debug`: a proxima `ler_marca` DESTA thread falha com erro de E/S.
    ///
    /// `debug_assertions`, e nao `cfg(test)`, pelo mesmo motivo do
    /// `ndx::panico_de_teste`: quem arma e o teste do SERVIDOR, e o
    /// `cfg(test)` nao atravessa crate. Em `release` o gancho nao existe.
    static LEITURA_FALHA_DE_TESTE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// So em `debug`: arma a falha de leitura acima -- pedido 451, M4.
#[doc(hidden)]
pub fn falhar_a_proxima_leitura_de_teste() {
    #[cfg(debug_assertions)]
    LEITURA_FALHA_DE_TESTE.with(|f| f.set(true));
}

// ----------------------------------------- a marca da cascata do embutido

impl Escrita {
    /// A operacao como a recuperacao a leria de volta desta marca.
    ///
    /// Existe para a cascata do `Table::atualizar` aplicar os elos pelo MESMO
    /// [`aplicar_uma`] da recuperacao, e nao por um segundo aplicador: a
    /// lista gravada e a lista aplicada, e aplicada pelo mesmo codigo que a
    /// completa depois de uma queda.
    pub fn como_operacao(&self) -> OperacaoDaMarca {
        OperacaoDaMarca {
            tabela: self.tabela.clone(),
            acao: self.acao,
            rowid: self.rowid,
            linha: self.linha.clone(),
            linha_antiga: self.linha_antiga.clone(),
            motivo: self.motivo.clone(),
            cascata_na_lista: self.cascata_na_lista,
        }
    }
}

/// O id da proxima marca que o EMBUTIDO grava em `dir`: o relogio, ou um
/// acima da maior marca que ja esta la -- o que for maior.
///
/// O servidor tem contador proprio (`Transacoes`, semeado com o relogio no
/// arranque); o embutido nao tem processo de longa duracao para guardar um,
/// e um `Table` nao conversa com outro. O relogio basta para crescer entre
/// aberturas, e o «maior + 1» cobre o relogio que voltou e duas marcas no
/// mesmo milissegundo. A colisao que sobra -- dois processos no mesmo
/// diretorio no mesmo instante -- o `create_new` do [`gravar_marca`] recusa
/// dizendo qual e, sem gravar por cima de marca nenhuma.
pub fn proximo_id_no_diretorio(dir: &Path) -> u64 {
    let sufixo = format!(".{EXTENSAO}");
    let maior = marcas_em(dir)
        .iter()
        .filter_map(|c| {
            c.file_name()?
                .to_str()?
                .strip_prefix(PREFIXO)?
                .strip_suffix(sufixo.as_str())?
                .parse::<u64>()
                .ok()
        })
        .max()
        .unwrap_or(0);
    (crate::util::agora_ms().max(1) as u64).max(maior.saturating_add(1))
}

/// As maes da cascata do embutido: a propria tabela que mudou de chave (o
/// `self` do `Table::atualizar`, que nao cabe no mapa porque esta emprestado)
/// e as filhas que a cascata ja abriu.
///
/// Sem a mae aqui, a filha conferiria a chave nova abrindo a mae num segundo
/// descritor, e o descritor novo nao ve a pagina que o primeiro ainda tem em
/// RAM -- era por isso que o `aplicar_ao_alterar` pagava um `sincronizar` da
/// mae antes das filhas. Emprestar o handle e o conserto do P0 valendo aqui
/// tambem, e poupa aquele `fsync`.
pub(crate) struct ComAMae<'a> {
    pub(crate) mae: &'a mut Table,
    pub(crate) resto: MaesAbertas<'a>,
}

impl MaesEmProgresso for ComAMae<'_> {
    fn mae(&mut self, tabela_ref: &str) -> Option<&mut Table> {
        if self.mae.nome().eq_ignore_ascii_case(tabela_ref) {
            return Some(&mut *self.mae);
        }
        self.resto.mae(tabela_ref)
    }

    fn prefixo(&self, tabela: &str) -> Option<Arc<Sobreposicao>> {
        self.resto.prefixo(tabela)
    }
}

// ------------------------------------------------ as maes que a marca abriu

/// Empresta a conferencia de FK as MAES que a MESMA passada de commit (ou a
/// recuperacao) ja abriu -- o conserto do P0 (`docs/ACID.md` §0). Sem isto, a
/// filha abria a mae num SEGUNDO descritor, que a guarda de visibilidade do
/// `.ndx` recusa enquanto o primeiro handle tem escrita pendente.
///
/// Reusar o handle e o mesmo desenho do InnoDB, que nunca teve o buraco porque
/// nunca houve dois objetos para a mesma tabela na transacao. A ordem preserva
/// a petrea sozinha: a passada aplica na ordem empilhada, entao o pai so esta
/// visivel aqui se foi aplicado ANTES da filha -- filha antes do pai continua
/// recusada, como deve.
pub struct MaesAbertas<'a> {
    pub abertas: &'a mut HashMap<String, Table>,
}

impl MaesEmProgresso for MaesAbertas<'_> {
    fn mae(&mut self, tabela_ref: &str) -> Option<&mut Table> {
        // A chave do mapa pode vir qualificada; `tabela_ref` ja chega simples.
        // O `find` fecha o emprestimo imutavel antes do `get_mut`.
        let chave = self
            .abertas
            .keys()
            .find(|k| nome_simples(k).eq_ignore_ascii_case(tabela_ref))
            .cloned()?;
        self.abertas.get_mut(&chave)
    }

    /// Na passada e na recuperacao os handles nao carregam sobreposicao -- a
    /// lista ja foi aplicada no disco deles -- e isto devolve `None`. Na
    /// pre-conferencia do COMMIT (pedido 448) eles carregam o prefixo, e o
    /// plano do `ao_alterar` abre a filha enxergando-o. O MESMO mapa serve
    /// aos tres: e a mesma pergunta, com o prefixo onde ele estiver.
    fn prefixo(&self, tabela: &str) -> Option<Arc<Sobreposicao>> {
        self.abertas
            .iter()
            .find(|(k, _)| nome_simples(k).eq_ignore_ascii_case(tabela))
            .and_then(|(_, t)| t.sobreposicao().cloned())
    }
}

/// Onde a marca esta sendo tratada -- e e isso que decide se a operacao
/// IMPOSSIVEL apaga a marca.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NoArranque {
    /// No arranque nada esta congelado nem reservado (os dois registros sao
    /// do PROCESSO), entao a operacao que nao entra agora nao entra nunca: a
    /// marca sai, e o relatorio a conta em `operacoes IMPOSSIVEIS`. E o
    /// comportamento de sempre -- MENOS quando a tabela nao foi ao disco
    /// (pedido 503, item 2): isso nao e operacao que nao entra, e dado que
    /// ainda nao esta duravel, e a marca e o que o traz de volta.
    Sim,
    /// Com o servidor de pe, a operacao impossivel pode ser PASSAGEIRA -- a
    /// tabela esta congelada por uma reescrita, e volta a atender quando ela
    /// acabar. Apagar a marca aqui trocaria «completa no proximo arranque»
    /// por «perdida para sempre», numa transacao que JA esta confirmada.
    Nao,
    /// A marca EM VOO que este processo acabou de gravar e sincronizar
    /// (pedido 451, M4 da segunda revisao do DBA). Como o [`NoArranque::Nao`],
    /// e mais uma coisa: ela nao pode deixar de se reler. Se deixa -- erro de
    /// E/S, falta de descritor --, nao e «commit que nunca comecou», e apaga-la
    /// jogaria fora a transacao confirmada: ela FICA, e a falha vai para as
    /// impossiveis, que o reparo da trava troca pela queda (H5). O arranque, com
    /// descritores novos, a le e completa.
    Gravada,
}

/// Trata UMA marca achada: completa, descarta ou deixa parada. Devolve se ela
/// pode sair do disco.
///
/// E o corpo do laco do [`Database::recuperar_marcas`] e tambem o do
/// `completar_marca_em_voo` do servidor -- um
/// lugar so, porque um segundo caminho para «completar um commit» seria um
/// segundo lugar para errar. O que muda entre os dois e so o [`NoArranque`].
pub fn tratar_marca(
    db: &Database,
    caminho: &Path,
    r: &mut Relatorio,
    arranque: NoArranque,
) -> bool {
    r.achadas += 1;
    match ler_marca(caminho) {
        Ok(Leitura::Aberta(marca)) => {
            let antes = r.impossiveis.len();
            let no_disco = completar(db, &marca, r);
            r.completadas += 1;
            no_disco && (arranque == NoArranque::Sim || r.impossiveis.len() == antes)
        }
        // A terceira resposta: cifrada, e esta chave nao a abre. **Nao se
        // apaga.** Apagar aqui trocaria confidencialidade por durabilidade --
        // uma transacao confirmada sumiria por falta de uma senha, e o selo
        // existe para proteger o dado, nao para custar o dado.
        Ok(Leitura::SemChave(motivo)) => {
            r.paradas.push(format!("{}: {motivo}", caminho.display()));
            false
        }
        // A marca que ESTE processo gravou e sincronizou, e que nao se releu:
        // o que falhou foi a leitura, e nao o commit. Ver `NoArranque::Gravada`.
        Err(e) if arranque == NoArranque::Gravada => {
            r.impossiveis.push(format!(
                "{}: a marca que este processo gravou e sincronizou nao se releu \
                 ({e}); ela fica no disco para o arranque",
                caminho.display()
            ));
            false
        }
        // No ARRANQUE, a marca que nem se leu nao e «commit que nunca comecou»:
        // o erro e do disco, e nao do arquivo (pedido 503, 1a). O PostgreSQL
        // (FATAL ao ler o WAL e o controle), o MySQL e o MariaDB (o InnoDB
        // aborta a recuperacao) convergem em NAO descartar e NAO subir por
        // cima -- aceite automatico. Ela fica, e o servidor nao sobe.
        Err(e) if arranque == NoArranque::Sim => {
            r.sem_leitura.push(format!(
                "{}: a marca nao se leu ({e}). Confira o disco e suba de novo: \
                 ela pode ser uma transacao confirmada",
                caminho.display()
            ));
            false
        }
        Ok(Leitura::NaoConfere) if arranque == NoArranque::Gravada => {
            r.impossiveis.push(format!(
                "{}: a marca que este processo gravou e sincronizou nao confere \
                 mais; ela fica no disco para o arranque",
                caminho.display()
            ));
            false
        }
        // Marca que nao confere, ou que nem da para abrir: commit que nunca
        // comecou.
        _ => {
            r.descartadas += 1;
            true
        }
    }
}

/// Reaplica o que falta de UMA marca. Devolve se as tabelas dela foram ao
/// disco -- e so entao a marca pode sair, em qualquer [`NoArranque`].
fn completar(db: &Database, marca: &Marca, r: &mut Relatorio) -> bool {
    let mut tabelas: HashMap<String, Table> = HashMap::new();
    for op in &marca.operacoes {
        // Garante o handle no mapa, aberto e preparado UMA vez.
        if !tabelas.contains_key(&op.tabela) {
            match db.abrir_qualificada(&op.tabela) {
                Ok(mut t) => {
                    // **O `.ndx` deixado para tras pela queda, e ele foi
                    // achado pela prova por SOQUETE -- nenhum teste
                    // unitario o via.**
                    //
                    // Um `SIGKILL` no meio da passada deixa levantada a
                    // marca de «o indice ficou para tras», e enquanto ela
                    // estiver la TODA operacao de indice recusa. A
                    // recuperacao entao nao completava o commit: reabria a
                    // tabela, tentava inserir, e recebia «reconstrua com
                    // reparar indice». O commit ficava pela metade e a
                    // tabela ficava inutilizavel ate alguem reparar a mao
                    // -- sem ninguem ser avisado, porque o servidor subia
                    // normalmente.
                    //
                    // Reconstruir aqui e o unico caminho honesto: o indice
                    // ja era intrustavel ANTES de a recuperacao chegar, e
                    // o relatorio CONTA quantos foram reconstruidos.
                    if t.indice_precisa_reconstruir() {
                        match t.reindexar() {
                            Ok(_) => r.indices_reconstruidos += 1,
                            Err(erro) => {
                                r.impossiveis.push(format!(
                                    "transacao {}: o indice de {} ficou para tras \
                                     e nao reconstruiu ({erro})",
                                    marca.id, op.tabela
                                ));
                                continue;
                            }
                        }
                    }
                    // A cascata desta tabela pode reconstruir o `.ndx`
                    // da FILHA que ficou sujo -- pedido 172.
                    //
                    // O `reindexar` acima cobre a tabela nomeada na marca.
                    // A filha da cascata nao esta nomeada em marca nenhuma,
                    // porque a cascata nunca vira `Escrita`: a maquina
                    // rodava e nao alcancava a tabela que ia consertar. Sem
                    // isto o commit saia em `operacoes IMPOSSIVEIS` com a
                    // mae no valor novo e parte das filhas no velho.
                    //
                    // Ligado SO aqui, e por isso nasce desligado: no
                    // caminho normal de escrita, indice sujo quer dizer
                    // «outro descritor tem escrita pendente», e reconstruir
                    // seria reparar arquivo sao.
                    t.ligar_reconstrucao_do_indice_da_filha(true);
                    tabelas.insert(op.tabela.clone(), t);
                }
                Err(erro) => {
                    r.impossiveis.push(format!(
                        "transacao {}: nao consegui abrir {} ({erro})",
                        marca.id, op.tabela
                    ));
                    continue;
                }
            }
        }
        // Retira a tabela do mapa enquanto reaplica, para que a conferencia de
        // FK possa emprestar as MAES que a MESMA marca ja reaplicou -- o mesmo
        // conserto do P0 da passada de commit. Sem isto, uma queda no meio de
        // um commit de pai+filha nao completava: a filha reabria a mae num
        // segundo descritor e batia na guarda de visibilidade do `.ndx`. Ela
        // volta ao mapa em seguida, para o `sincronizar` do fim alcanca-la.
        let mut t = tabelas
            .remove(&op.tabela)
            .expect("a tabela acabou de ser inserida no mapa");
        {
            let mut maes = MaesAbertas {
                abertas: &mut tabelas,
            };
            match aplicar_uma(&mut t, op, &mut maes) {
                Ok(true) => r.reaplicadas += 1,
                Ok(false) => r.ja_aplicadas += 1,
                Err(e) => r.impossiveis.push(format!(
                    "transacao {}: {} rowid {} em {} ({e})",
                    marca.id,
                    op.acao.nome(),
                    op.rowid,
                    op.tabela
                )),
            }
        }
        tabelas.insert(op.tabela.clone(), t);
    }
    let mut no_disco = true;
    for (nome, mut t) in tabelas {
        // O relatorio CONTA o que a cascata reconstruiu, junto do que a marca
        // reconstruiu: reparar em silencio seria trocar um recado ruim por
        // nenhum recado.
        r.indices_reconstruidos += t.indices_da_cascata_reconstruidos();
        // Era `let _ =` (pedido 503, item 2): o erro sumia e quem chama
        // apagava a marca -- o bilhete de um commit cujo dado nao foi ao
        // disco, na mesma ordem do fecho da janela (sincronizar, apagar a
        // marca) que o pedido 509 consertou no servidor. E o irmao dele. O
        // `fsync` recusado nem volta aqui no servidor (o processo cai na
        // recusa); o que volta e o erro de antes do disco -- o `.pag` que nao
        // se grava, o volume que nao abre, a pagina do `.ndx` no disco cheio
        // --, e com ele a marca FICA.
        if let Err(e) = t.sincronizar() {
            no_disco = false;
            r.impossiveis.push(format!(
                "transacao {}: {nome} nao foi ao disco ({e}); a marca fica para a \
                 proxima recuperacao",
                marca.id
            ));
        }
    }
    no_disco
}

/// Aplica uma operacao da marca. `Ok(false)` = ja estava aplicada.
///
/// `maes` empresta a conferencia de FK as tabelas que a mesma marca ja
/// reaplicou -- o conserto do P0 valendo tambem na recuperacao (ver
/// [`completar`] e `docs/ACID.md` §0).
pub(crate) fn aplicar_uma(
    t: &mut Table,
    op: &OperacaoDaMarca,
    maes: &mut dyn MaesEmProgresso,
) -> Result<bool> {
    match op.acao {
        Acao::Inserir => {
            // O slot ja existe? Entao a passada chegou nele e nao ha o que
            // fazer -- a reaplicacao e idempotente pelo rowid, e e por isso
            // que a marca guarda o rowid alvo e nao so a linha.
            if op.rowid <= t.slots() {
                if t.ler(op.rowid)?.is_some() {
                    return Ok(false);
                }
                // Slot dentro da faixa e LIVRE: o `.reg` nao reaproveita slot,
                // entao nao ha como refazer esta linha no lugar dela. E a
                // unica lacuna deste desenho, e ela esta escrita na §5.4 do
                // documento em vez de escondida.
                return Err(PhxError::Corrompido(format!(
                    "o slot {} ja foi consumido e esta livre; o .reg nao \
                     reaproveita slot, entao esta linha nao volta para o \
                     lugar dela",
                    op.rowid
                )));
            }
            let saiu = t.inserir_com_maes(&op.linha, maes)?;
            if saiu != op.rowid {
                return Err(PhxError::Corrompido(format!(
                    "a marca dizia rowid {} e a insercao saiu {saiu}",
                    op.rowid
                )));
            }
            Ok(true)
        }
        Acao::Atualizar => {
            if t.ler(op.rowid)?.is_none() {
                return Err(PhxError::NaoEncontrado(format!(
                    "rowid {} nao existe para atualizar",
                    op.rowid
                )));
            }
            // **ACID-C (marca v3): a cascata viaja ACHATADA na marca.** Cada elo
            // (filha, neta...) e uma operacao PROPRIA desta mesma marca, na
            // ordem pai-antes-de-filha. Reaplicar SEM cascatear evita gravar a
            // filha duas vezes; o `recascatear` abaixo fica so para a marca
            // v1/v2, em que a cascata era IMPLICITA. Idempotente pelo rowid: o
            // elo que ja tinha sido gravado antes da queda so regrava os mesmos
            // valores.
            if op.cascata_na_lista {
                t.atualizar_sem_cascata_com_maes(op.rowid, &op.linha, maes)?;
                return Ok(true);
            }
            t.atualizar_com_maes(op.rowid, &op.linha, maes)?;
            // **O `atualizar` sozinho NAO refaz a cascata, e isso esta
            // medido.** A cascata do `ao_alterar` e planejada pelo delta da
            // mae; se a queda foi depois de a mae ir para o disco e antes de a
            // cascata rodar, a reaplicacao acha `antes == depois`, o plano sai
            // vazio, a filha fica para tras -- e esta funcao devolvia `Ok`,
            // somando em `reaplicadas`, com o relatorio do arranque dizendo
            // que o commit foi completado.
            //
            // Com a linha antiga na mao da para perguntar. E idempotente:
            // cascata que ja rodou nao deixa filha na chave antiga.
            if !op.linha_antiga.is_empty() {
                t.recascatear(&op.linha_antiga, &op.linha)?;
            }
            Ok(true)
        }
        // O irmao da passada: as exclusoes emprestam as FILHAS que a mesma
        // marca ja reaplicou, e a restauracao as MAES (pedido 448).
        Acao::ExcluirSuave => Ok(t.excluir_suave_com_maes(op.rowid, &op.motivo, maes)?),
        Acao::ExcluirDeVez => Ok(t.excluir_de_vez_com_maes(op.rowid, &op.motivo, maes)?),
        Acao::Restaurar => Ok(t.restaurar_com_maes(op.rowid, &op.motivo, maes)?),
    }
}

// ------------------------------------------------------------ a recuperacao

impl Database {
    /// Varre ESTE database atras de marcas orfas, **completa** o que achar e,
    /// DEPOIS, reconstroi o indice que o processo anterior deixou marcado.
    ///
    /// # Por que ela anda para a FRENTE, e nunca para tras
    ///
    /// Nao e escolha estetica. Desfazer exigiria devolver slots ja gravados, e
    /// o `.reg` nunca reaproveita slot -- a regra que decide tudo neste
    /// desenho. Andar para a frente e a unica direcao que o formato permite, e
    /// o `.tx` e o que torna isso possivel: sem ele, nao se sabe para onde ir.
    ///
    /// A reaplicacao e **idempotente pelo rowid**: cada operacao diz o slot que
    /// devia ter escrito. Slot ja ocupado -- passa adiante. Slot livre e no fim
    /// da tabela -- grava.
    ///
    /// # Quem chama, e quando (pedido 563)
    ///
    /// O arranque do servidor, database por database; a abertura do embutido
    /// (`phx_base_abrir`), sem punho de tabela vivo; e o `reindex` do CLI,
    /// antes de reconstruir. Nunca o `Instancia::abrir_database` generico: o
    /// servidor o chama EM SERVICO, com marca de commit ja aplicado esperando o
    /// fecho da janela (`marcas_pendentes`), e completar ali reaplicaria linha
    /// velha por cima de escrita nova.
    ///
    /// # Onde ela procura
    ///
    /// Na raiz do database -- onde o servidor grava, com o nome QUALIFICADO --
    /// e no diretorio de cada schema, onde o `Table::atualizar` de uma tabela
    /// de schema grava a marca da cascata dele, com o nome simples: a chave
    /// estrangeira nao atravessa diretorio, entao a mae e as filhas moram ali.
    ///
    /// # E a ordem das duas metades
    ///
    /// As marcas ANTES do indice marcado, e e isso que impede o arranque de
    /// calar a orfa: a tabela nomeada numa marca -- e a filha da cascata dela
    /// -- se reconstroi no `completar`, com a cascata completada; o que sobra
    /// para o passe do 522 e o resto.
    ///
    /// # O passe do indice marcado -- pedido 522
    ///
    /// # Por que isto passou a existir
    ///
    /// Ate o 522 o `fechar` baixava a marca sem `fsync`, e o arranque so via
    /// marcado o indice da tabela que a queda pegou NO MEIO de uma escrita --
    /// uma, no maximo, e ela recusava ate alguem mandar `reindexar`. O `fechar`
    /// deixou de baixar: so o fecho da janela, depois dos `fsync`, grava o 0. Um
    /// processo que cai -- e o servidor nao tem outro jeito de parar, sai por
    /// sinal -- deixa marcada TODA tabela escrita desde o ultimo fecho da janela,
    /// e o processo novo nao sabe se a maquina caiu junto. Deixa-las recusando
    /// ate o operador descobrir faria de toda parada sob carga uma indisponibilidade
    /// das tabelas mais quentes; reconstruir aqui, com a porta ainda fechada, e o
    /// preco dito no `FORMATO.md` (§ a marca de sujo), medido e contado no
    /// relatorio.
    ///
    /// O motor e o [`Database::reconstruir_indices_marcados`],
    /// o mesmo que a restauracao chama: a pergunta «esta arvore abre marcada?»
    /// tem uma resposta so.
    ///
    /// # O que isto NAO resolve
    ///
    /// Reconstroi do `.reg` que o nucleo devolve. No mesmo boot depois do `abort`
    /// do pedido 509 o nucleo pode devolver o que o disco perdeu, e o indice novo
    /// sai coerente com um `.reg` que nao esta no disco -- e a metade do 509 que
    /// continua aberta, e a sentinela dela tem de decidir ANTES deste passe.
    pub fn recuperar_marcas(&self) -> Relatorio {
        let comeco = std::time::Instant::now();
        let mut r = Relatorio::default();
        let mut schemas: Vec<Database> = Vec::new();
        for s in self.schemas().unwrap_or_default() {
            if let Ok(d) = self.diretorio(Some(&s)) {
                schemas.push(Database::no_diretorio(&d));
            }
        }
        for db in std::iter::once(self).chain(schemas.iter()) {
            completar_as_marcas_de(db, &mut r);
        }
        let (feitas, pendentes) = self.reconstruir_indices_marcados();
        r.indices_reconstruidos += feitas;
        r.indices_pendentes.extend(pendentes);
        r.ms = comeco.elapsed().as_millis() as u64;
        r
    }
}

/// Completa -- ou descarta -- cada marca do diretorio de `db`, na ordem do id,
/// e apaga a que pode sair. O corpo unico dos dois lacos de abertura, o da
/// [`Database::recuperar_marcas`] e o da [`recuperar_no_diretorio`].
fn completar_as_marcas_de(db: &Database, r: &mut Relatorio) {
    for caminho in marcas_em(db.caminho()) {
        if tratar_marca(db, &caminho, r, NoArranque::Sim) {
            let _ = std::fs::remove_file(&caminho);
        }
    }
}

/// As marcas de `dir`, na ordem em que foram criadas -- a do id no nome.
///
/// Duas marcas no mesmo diretorio sao completadas nessa ordem: a segunda pode
/// depender do que a primeira gravou.
pub fn marcas_em(dir: &Path) -> Vec<PathBuf> {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut marcas: Vec<PathBuf> = entradas
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(PREFIXO) && n.ends_with(&format!(".{EXTENSAO}")))
        })
        .collect();
    marcas.sort();
    marcas
}

/// Completa as marcas de UM diretorio de tabelas -- o `reindex` do CLI, que
/// recebe o diretorio e nao o database. E o mesmo laco da
/// [`Database::recuperar_marcas`], sem o passe do indice marcado: quem chama
/// reconstroi a tabela dele logo em seguida.
pub fn recuperar_no_diretorio(dir: &Path) -> Relatorio {
    let comeco = std::time::Instant::now();
    let mut r = Relatorio::default();
    completar_as_marcas_de(&Database::no_diretorio(dir), &mut r);
    r.ms = comeco.elapsed().as_millis() as u64;
    r
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::apoio_teste::DirTemp;

    fn dir(rotulo: &str) -> DirTemp {
        DirTemp::novo(&format!("tx-{rotulo}"))
    }

    /// A ida e volta das CATORZE variantes.
    ///
    /// Este teste existe porque a alternativa obvia -- guardar a linha em JSON
    /// -- perde: `valor_para_json` escreve `Time` e `DateTime` como texto ISO
    /// e `json_para_valor` desses dois so aceita numero. A volta nao fecha, e
    /// uma recuperacao que reconstroi a linha errada e pior do que nenhuma.
    #[test]
    fn a_linha_volta_igual_nas_catorze_variantes() {
        let linha = vec![
            Value::Null,
            Value::Bool(true),
            Value::Int(-42),
            Value::UInt(u64::MAX),
            Value::Real(1.5),
            Value::Decimal(-123456789012345678),
            Value::Date(19_000),
            Value::Time(8_640_000 - 1),
            Value::DateTime(-1_000_000),
            Value::Str("Blumenau".into()),
            Value::Bin(vec![0, 1, 254, 255]),
            Value::Memo("um memorando\ncom quebra".into()),
            Value::Uuid(Uuid::de_bytes([7u8; 16])),
            Value::Uuid256(Uuid256::de_bytes([9u8; 32])),
        ];
        let b = codificar_linha(&linha);
        let volta = decodificar_linha(&b).expect("a volta tem de fechar");
        assert_eq!(volta, linha);
    }

    #[test]
    fn a_marca_volta_com_as_operacoes_na_ordem() {
        let d = dir("marca");
        let ops = vec![
            Escrita {
                database: "loja".into(),
                tabela: "clientes".into(),
                acao: Acao::Inserir,
                rowid: 7,
                linha: vec![Value::Int(1), Value::Str("Ana".into())],
                linha_antiga: Vec::new(),
                motivo: String::new(),
                cascata_na_lista: false,
                elo_do_empilhar: false,
                elo_da_cascata: false,
            },
            Escrita {
                database: "loja".into(),
                tabela: "public.pedidos".into(),
                acao: Acao::ExcluirSuave,
                rowid: 3,
                linha: Vec::new(),
                linha_antiga: Vec::new(),
                motivo: "pedido do titular".into(),
                cascata_na_lista: false,
                elo_do_empilhar: false,
                elo_da_cascata: false,
            },
        ];
        let caminho = gravar_marca(&d, 99, 1_700_000_000_000, &ops).unwrap();
        let m = ler_marca(&caminho)
            .unwrap()
            .marca()
            .expect("a marca tem de conferir");
        assert_eq!(m.id, 99);
        assert_eq!(m.operacoes.len(), 2);
        assert_eq!(m.operacoes[0].tabela, "clientes");
        assert_eq!(m.operacoes[0].acao, Acao::Inserir);
        assert_eq!(m.operacoes[0].rowid, 7);
        assert_eq!(m.operacoes[0].linha[1], Value::Str("Ana".into()));
        assert_eq!(m.operacoes[1].tabela, "public.pedidos");
        assert_eq!(m.operacoes[1].motivo, "pedido do titular");
    }

    /// Um byte trocado no meio de uma operacao faz a marca inteira ser
    /// recusada -- e recusada e o mesmo que «commit que nunca comecou».
    #[test]
    fn um_byte_trocado_derruba_a_marca_inteira() {
        let d = dir("crc");
        let ops = vec![Escrita {
            database: "loja".into(),
            tabela: "clientes".into(),
            acao: Acao::Inserir,
            rowid: 1,
            linha: vec![Value::Str("Blumenau".into())],
            linha_antiga: Vec::new(),
            motivo: String::new(),
            cascata_na_lista: false,
            elo_do_empilhar: false,
            elo_da_cascata: false,
        }];
        let caminho = gravar_marca(&d, 1, 0, &ops).unwrap();
        let mut b = std::fs::read(&caminho).unwrap();
        let meio = b.len() - 6;
        b[meio] ^= 0xFF;
        std::fs::write(&caminho, &b).unwrap();
        assert!(
            matches!(ler_marca(&caminho).unwrap(), Leitura::NaoConfere),
            "marca com CRC quebrado nao pode ser lida como boa"
        );
    }

    /// Marca truncada no meio do `fsync` tambem e recusada.
    #[test]
    fn marca_truncada_e_recusada() {
        let d = dir("truncada");
        let ops = vec![Escrita {
            database: "loja".into(),
            tabela: "clientes".into(),
            acao: Acao::Inserir,
            rowid: 1,
            linha: vec![Value::Str("Joinville".into())],
            linha_antiga: Vec::new(),
            motivo: String::new(),
            cascata_na_lista: false,
            elo_do_empilhar: false,
            elo_da_cascata: false,
        }];
        let caminho = gravar_marca(&d, 2, 0, &ops).unwrap();
        let b = std::fs::read(&caminho).unwrap();
        std::fs::write(&caminho, &b[..b.len() - 5]).unwrap();
        assert!(matches!(ler_marca(&caminho).unwrap(), Leitura::NaoConfere));
    }

    fn uma_escrita(cidade: &str) -> Vec<Escrita> {
        vec![Escrita {
            database: "loja".into(),
            tabela: "clientes".into(),
            acao: Acao::Inserir,
            rowid: 1,
            linha: vec![Value::Str(cidade.into())],
            linha_antiga: Vec::new(),
            motivo: String::new(),
            cascata_na_lista: false,
            elo_do_empilhar: false,
            elo_da_cascata: false,
        }]
    }

    /// A marca nasce **fechada para o resto da maquina** -- pedido 354.
    ///
    /// Ela guarda a linha INTEIRA do `COMMIT`, e a `linha_antiga` do
    /// `atualizar` sai decifrada do `.reg` para entrar aqui. Nascer 0644 punha
    /// isso a disposicao de qualquer conta da maquina enquanto o commit
    /// durasse -- e no dia de uma queda, para sempre.
    ///
    /// Prova real: trocar o [`criar_privado`] de volta por
    /// `std::fs::File::create` faz o modo sair **100644** e este teste falhar.
    #[cfg(unix)]
    #[test]
    fn a_marca_nasce_so_para_o_dono() {
        use std::os::unix::fs::PermissionsExt as _;
        let d = dir("modo");
        let caminho = gravar_marca(&d, 11, 0, &uma_escrita("Blumenau")).unwrap();
        let modo = std::fs::metadata(&caminho).unwrap().permissions().mode();
        assert_eq!(
            modo & 0o777,
            0o600,
            "a marca nasceu {:o}, e ela carrega a linha inteira do commit",
            modo & 0o777
        );
    }

    /// Gravar duas vezes o mesmo id **recusa**, em vez de truncar por cima.
    ///
    /// Uma marca ja no disco com aquele id e um `COMMIT` esperando
    /// recuperacao. O `create_new` que traz o 0600 e o mesmo que impede
    /// apagar a intencao dela, e a recusa NOMEIA o que ha -- o erro cru
    /// "File exists" mandaria procurar problema de disco.
    #[test]
    fn gravar_por_cima_de_marca_pendente_recusa_nomeando() {
        let d = dir("ja-existe");
        gravar_marca(&d, 12, 0, &uma_escrita("Blumenau")).unwrap();
        let erro = gravar_marca(&d, 12, 0, &uma_escrita("Joinville")).unwrap_err();
        let texto = erro.to_string();
        assert!(
            texto.contains("ja existe a marca da transacao 12") && texto.contains("COMMIT"),
            "a recusa tem de dizer o que ha: {texto}"
        );
    }

    /// Com o cofre DESLIGADO a marca continua nascendo na v3, byte por byte.
    ///
    /// E o teste do comportamento **velho**, que e o que mais importa numa
    /// guarda nova: quem nunca pediu cifra nao paga formato novo, e um
    /// servidor anterior continua sabendo ler a marca desta base.
    #[test]
    fn sem_cofre_a_marca_continua_na_versao_anterior() {
        let d = dir("v3");
        let caminho = gravar_marca(&d, 13, 0, &uma_escrita("Blumenau")).unwrap();
        let b = std::fs::read(&caminho).unwrap();
        assert_eq!(
            u32::from_le_bytes([b[8], b[9], b[10], b[11]]),
            VERSAO_CASCATA_EM_CLARO
        );
        // E o cabecalho continua com os 36 bytes de sempre: 32 de campos mais
        // o CRC. Se o material tivesse entrado, seriam 76.
        let primeira_op = u16::from_le_bytes([b[CAB_ATE_CRC + 4], b[CAB_ATE_CRC + 5]]);
        assert_eq!(primeira_op as usize, "clientes".len());
    }
}

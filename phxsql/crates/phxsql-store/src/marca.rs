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

/// A versao do `gravar_marca` antigo (pedido 354), que hoje so as provas de
/// leitura chamam. Ver `docs/FORMATO.md` §16.
///
/// Nao e a versao com que o `COMMIT` grava: desde o pedido 709 a marca nasce
/// posicional -- [`VERSAO_POSICIONAL_CIFRADA`] com o cofre ligado e
/// [`VERSAO_POSICIONAL_EM_CLARO`] sem ele. Este comentario dizia que sem cofre a marca
/// continuava v3 e que um servidor anterior a lia; deixou de valer no 709
/// (pedido 738), e a v7 nenhum binario anterior a ela le.
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

/// A v5 (pedido 682): o GRUPO DA REPLICA, em claro. Cabecalho da v3; cada
/// operacao e um evento vindo da origem, e o payload da v3 ganha no fim o
/// carimbo e a origem de la, a posicao local do evento e a imagem.
///
/// # Por que uma versao propria, e nao um byte novo na v3
///
/// Porque o leitor de antes PULA o que sobra do payload depois do byte da
/// cascata: com um byte novo na v3 ele leria o evento como um `inserir` de
/// linha vazia e o gravaria. Com a versao nova ele cai em
/// [`Leitura::NaoConfere`] e descarta -- o que um servidor anterior fazia com
/// o grupo partido de qualquer jeito, porque nem marca havia. E a v3 continua
/// nascendo byte por byte igual para todo `COMMIT`: a versao nova so existe
/// na marca que a replica grava.
pub const VERSAO_REPLICA_EM_CLARO: u32 = 5;

/// A v6: o grupo da replica com o cofre ligado -- cabecalho da v4, payload
/// selado como o dela. A imagem traz o dado de la aberto (o 613 so deixa
/// replicar coluna marcada com cofre), e marca em claro o poria no disco.
pub const VERSAO_REPLICA_CIFRADA: u32 = 6;

/// A v7 (pedido 709): a marca do `COMMIT` com o BILHETE POSICIONAL, em claro.
/// Cabecalho da v3 mais o `tx` que todos os eventos dela levam; cada operacao
/// leva no fim do payload a VERSAO que o slot tinha antes dela.
///
/// # Por que a versao do slot, e nao a linha antiga
///
/// Porque a pergunta da recuperacao e «a passada ja passou desta operacao?»,
/// e o conteudo nao a responde: o `COMMIT` que muda «0» para «a» seguido de
/// uma solta que volta a «0» deixa o slot igual ao de antes, e a exclusao
/// suave e a restauracao so tem dois estados. A versao do slot so anda (FORMATO
/// §1, bytes 8..16), e e o LSN da linha: slot com versao MAIOR que a de antes
/// ja recebeu esta operacao, ou recebeu escrita depois dela, e nos dois casos
/// reaplicar grava por cima de estado mais novo -- o que PostgreSQL, InnoDB e
/// SQLite convergem em nunca fazer (`recuperacao-e-replica-desenho-unico.md`
/// §4, linha 2).
///
/// # Por que versao nova, e nao byte novo na v3
///
/// O mesmo motivo da v5: o leitor de antes PULA o que sobra do payload depois
/// do byte da cascata e leria a v7 como v3, reaplicando sem condicao. Com a
/// versao nova ele cai em [`Leitura::NaoConfere`] -- e ai APAGA uma transacao
/// confirmada: voltar o binario com marca v7 de pe e a armadilha ja escrita
/// para a v5/v6, «so para frente». Nenhum binario foi selado com a v3 como
/// unica, e e por isso que entra agora.
pub const VERSAO_POSICIONAL_EM_CLARO: u32 = 7;

/// A v8: a v7 com o cofre ligado -- cabecalho da v4 (material) mais o `tx`.
pub const VERSAO_POSICIONAL_CIFRADA: u32 = 8;

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
    /// Pedido 682: esta operacao e um evento que a replica recebeu da origem
    /// (marca v5/v6). `None` em toda marca de `COMMIT`.
    pub replica: Option<EventoDaReplica>,
    /// Pedido 709 (marca v7/v8): a versao que o slot tinha ANTES desta
    /// operacao, 0 na inclusao. `None` nas versoes anteriores -- ali a
    /// recuperacao faz o que sempre fez.
    pub versao_antes: Option<u64>,
}

/// O que a marca do grupo da replica guarda de cada evento, alem de tabela,
/// acao e rowid -- pedido 682.
///
/// # Por que o evento, e nao a linha
///
/// Porque a recuperacao tem de reaplicar pelo MESMO caminho da replica
/// ([`Table::aplicar_evento`]), e nao pelo `inserir` de um `COMMIT`: o diario
/// daqui tem de continuar o de la evento a evento -- carimbo, operacao, rowid
/// --, senao a conferencia de continuidade da rodada seguinte rompe a tabela.
/// E a replica nao julga a chave estrangeira (pedido 300 §2.7); o `inserir`
/// do `COMMIT` julga.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventoDaReplica {
    pub operacao: crate::log::Operacao,
    /// O instante em que a escrita nasceu, no relogio de LA.
    pub carimbo_ms: i64,
    /// O hash do servidor onde ela nasceu.
    pub origem: u16,
    /// Quantos eventos o diario DAQUI tinha antes deste. E a idempotencia da
    /// recuperacao: o rowid nao basta, porque uma alteracao reaplicada
    /// grava a mesma linha e acrescenta um evento que a origem nao tem.
    pub posicao: u64,
    /// A imagem como chegou pelo fio.
    pub imagem: Vec<u8>,
}

/// Um evento do grupo da replica, como o servidor o entrega a
/// [`gravar_marca_da_replica`]. Emprestado, e nao copiado: o grupo pode ter
/// o tamanho do teto da transacao, e a marca so precisa dos bytes ate o
/// `write`.
#[derive(Debug, Clone, Copy)]
pub struct EventoDoGrupo<'a> {
    /// O nome qualificado, o mesmo que `Database::abrir_qualificada` recebe.
    pub tabela: &'a str,
    pub operacao: crate::log::Operacao,
    pub rowid: u64,
    pub carimbo_ms: i64,
    pub origem: u16,
    /// Ver [`EventoDaReplica::posicao`].
    pub posicao: u64,
    pub imagem: &'a [u8],
}

/// A marca inteira, lida de volta.
#[derive(Debug)]
pub struct Marca {
    pub id: u64,
    pub carimbo_ms: i64,
    /// Pedido 709 (v7/v8): o id de transacao que todos os eventos desta marca
    /// levam no diario, reservado sob a trava antes do primeiro. 0 nas
    /// versoes anteriores, e na marca gravada fora de unidade.
    pub tx: u64,
    pub operacoes: Vec<OperacaoDaMarca>,
}

/// O bilhete posicional da marca v7/v8 -- pedido 709. Ver
/// [`VERSAO_POSICIONAL_EM_CLARO`].
#[derive(Debug, Clone, Copy)]
pub struct Bilhete<'a> {
    /// Ver [`Marca::tx`].
    pub tx: u64,
    /// Uma por operacao, na ordem: ver [`OperacaoDaMarca::versao_antes`].
    /// Sai de [`versoes_antes`].
    pub versoes_antes: &'a [u64],
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
///
/// # E o cabecalho, nas v6/v8 (pedido 735)
///
/// O carimbo, o `n_operacoes` e o `tx` da v8 so tinham o CRC, e quem baixava o
/// `n` de 3 para 2 e refazia o CRC tinha uma marca que ABRIA com duas
/// operacoes -- a recuperacao aplicava meia transacao, dada como completada;
/// trocar o `tx` juntava eventos de transacoes diferentes. Nas versoes que
/// nasceram com o 682 e o 709 eles entram aqui, com o indice da operacao. A v4
/// continua lida como sempre foi: muda-la quebraria a marca de pe de um
/// binario ja selado.
fn aad_da_operacao(
    id: u64,
    tabela: &[u8],
    tag: u8,
    rowid: u64,
    cabecalho: Option<CabecalhoNoSelo>,
) -> Vec<u8> {
    let mut a = Vec::with_capacity(tabela.len() + 17 + 28);
    a.extend_from_slice(&id.to_le_bytes());
    a.extend_from_slice(tabela);
    a.push(tag);
    a.extend_from_slice(&rowid.to_le_bytes());
    if let Some(c) = cabecalho {
        a.extend_from_slice(&c.carimbo_ms.to_le_bytes());
        a.extend_from_slice(&c.n_operacoes.to_le_bytes());
        a.extend_from_slice(&c.tx.to_le_bytes());
        a.extend_from_slice(&(c.indice as u64).to_le_bytes());
    }
    a
}

/// O que do cabecalho entra no selo de cada operacao das v6/v8 -- pedido 735.
#[derive(Clone, Copy)]
struct CabecalhoNoSelo {
    carimbo_ms: i64,
    n_operacoes: u32,
    tx: u64,
    indice: usize,
}

/// A versao sela o cabecalho? -- as que nasceram com o selo dele (pedido 735).
fn sela_o_cabecalho(versao: u32) -> bool {
    matches!(versao, VERSAO_REPLICA_CIFRADA | VERSAO_POSICIONAL_CIFRADA)
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
///
/// Grava a marca SEM bilhete (v3/v4): e o formato de um binario anterior, e
/// so as provas o chamam -- para conferir que a marca deixada por ele continua
/// sendo lida e completada. Todo caminho de escrita grava pela
/// [`gravar_marca_posicional`].
pub fn gravar_marca(
    diretorio: &Path,
    id: u64,
    carimbo_ms: i64,
    ops: &[Escrita],
) -> Result<PathBuf> {
    gravar_com(
        &caminho_da_marca(diretorio, id),
        id,
        carimbo_ms,
        ops,
        Fim::Nada,
    )
}

/// Grava a marca v7/v8, com o bilhete posicional -- pedido 709. A ordem e a
/// do [`gravar_marca`]: grava e sincroniza antes de a passada tocar em
/// arquivo de dado.
pub fn gravar_marca_posicional(
    diretorio: &Path,
    id: u64,
    carimbo_ms: i64,
    ops: &[Escrita],
    bilhete: Bilhete<'_>,
) -> Result<PathBuf> {
    if bilhete.versoes_antes.len() != ops.len() {
        return Err(PhxError::Esquema(format!(
            "a marca {id} tem {} operacao(oes) e {} versao(oes) de antes",
            ops.len(),
            bilhete.versoes_antes.len()
        )));
    }
    gravar_com(
        &caminho_da_marca(diretorio, id),
        id,
        carimbo_ms,
        ops,
        Fim::Bilhete(bilhete),
    )
}

/// O estado de um slot que [`versoes_antes`] precisa: a versao (`None` =
/// livre ou alem do fim) e a marca de exclusao suave (`None` = a tabela nao
/// tem, ou o slot esta livre).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EstadoDoSlot {
    pub versao: Option<u64>,
    pub suave: Option<bool>,
}

/// O estado do slot `rowid` de `t`, lido do disco -- o cabecalho do slot e,
/// so quando `com_suave`, a linha sem os externos.
pub fn estado_do_slot(t: &mut Table, rowid: u64, com_suave: bool) -> Result<EstadoDoSlot> {
    if rowid == 0 || rowid > t.slots() {
        return Ok(EstadoDoSlot {
            versao: None,
            suave: None,
        });
    }
    let versao = t.versao(rowid)?;
    let suave = match (com_suave, versao, t.esquema().coluna_softdeleted()) {
        (true, Some(_), Some(i)) => t
            .ler_sem_externos(rowid)?
            .map(|l| matches!(l.get(i), Some(Value::Bool(true)))),
        _ => None,
    };
    Ok(EstadoDoSlot { versao, suave })
}

/// A versao que cada operacao de `ops` vai encontrar no slot dela -- o
/// bilhete da marca v7/v8 (pedido 709). `estado` le o slot do disco, com a
/// trava na mao, e e chamado UMA vez por linha: as operacoes seguintes da
/// mesma linha se deduzem das anteriores, como a passada as fara.
///
/// # A regra quando nao se sabe: contar a escrita
///
/// A exclusao suave e a restauracao nao gravam quando a linha ja esta no
/// estado pedido (`Table::marcar`), e a alteracao pode mudar a coluna de
/// sistema. Onde a deducao nao sabe o estado, ela conta que a operacao
/// GRAVA. Os dois erros nao custam o mesmo: contar a mais faz a recuperacao
/// reaplicar uma operacao que ja tinha entrado -- o comportamento de antes,
/// os mesmos valores --, e contar a menos a faria pular uma operacao
/// confirmada que nao entrou.
pub fn versoes_antes(
    ops: &[Escrita],
    mut estado: impl FnMut(&str, u64, bool) -> Result<EstadoDoSlot>,
) -> Result<Vec<u64>> {
    // (tabela, rowid) -> (versao, suave): o estado DEDUZIDO depois das
    // operacoes ja vistas.
    let mut slots: HashMap<(&str, u64), (u64, Option<bool>)> = HashMap::new();
    let mut saida = Vec::with_capacity(ops.len());
    for e in ops {
        let chave = (e.tabela.as_str(), e.rowid);
        let atual = match slots.get(&chave) {
            Some(v) => *v,
            None if e.acao == Acao::Inserir => (0, None),
            None => {
                let precisa = matches!(e.acao, Acao::ExcluirSuave | Acao::Restaurar);
                let lido = estado(&e.tabela, e.rowid, precisa)?;
                (lido.versao.unwrap_or(0), lido.suave)
            }
        };
        saida.push(atual.0);
        let (v, suave) = atual;
        let depois = match e.acao {
            // A linha nova pode nascer com a coluna de sistema ligada, e a
            // alteracao pode mexer nela: a marca suave depois delas nao se sabe.
            Acao::Inserir => (1, None),
            Acao::Atualizar => (v + 1, None),
            Acao::ExcluirSuave if suave == Some(true) => (v, suave),
            Acao::ExcluirSuave => (v + 1, Some(true)),
            Acao::Restaurar if suave == Some(false) => (v, suave),
            Acao::Restaurar => (v + 1, Some(false)),
            Acao::ExcluirDeVez => (0, None),
        };
        slots.insert(chave, depois);
    }
    Ok(saida)
}

/// A acao da marca que corresponde a um evento do diario. A exclusao da
/// replica e FISICA porque foi fisica na origem -- a suave chega como
/// alteracao, que e o que ela e no `.reg`.
fn acao_do_evento(op: crate::log::Operacao) -> Acao {
    match op {
        crate::log::Operacao::Inclusao => Acao::Inserir,
        crate::log::Operacao::Alteracao => Acao::Atualizar,
        crate::log::Operacao::Exclusao => Acao::ExcluirDeVez,
    }
}

/// O caminho de volta de [`acao_do_evento`]. `None` = a acao nao vem de um
/// evento, e a marca que a traz como evento da replica nao confere.
fn evento_da_acao(acao: Acao) -> Option<crate::log::Operacao> {
    match acao {
        Acao::Inserir => Some(crate::log::Operacao::Inclusao),
        Acao::Atualizar => Some(crate::log::Operacao::Alteracao),
        Acao::ExcluirDeVez => Some(crate::log::Operacao::Exclusao),
        Acao::ExcluirSuave | Acao::Restaurar => None,
    }
}

/// Grava a marca do GRUPO DA REPLICA e sincroniza, antes do primeiro evento
/// do grupo tocar em arquivo de dado -- pedido 682.
///
/// E a marca da cascata e do `COMMIT`, e nao uma segunda: o mesmo arquivo
/// `transacao_<id>.tx`, o mesmo selo, o mesmo `create_new` 0600, o mesmo
/// `fsync` pelo motor do 509, e a mesma recuperacao do arranque
/// ([`Database::recuperar_marcas`]) que completa o grupo antes de a porta
/// abrir. Muda so o que cada operacao carrega -- ver [`EventoDaReplica`].
pub fn gravar_marca_da_replica(
    diretorio: &Path,
    id: u64,
    carimbo_ms: i64,
    eventos: &[EventoDoGrupo<'_>],
) -> Result<PathBuf> {
    let ops: Vec<Escrita> = eventos
        .iter()
        .map(|e| Escrita {
            database: String::new(),
            tabela: e.tabela.to_string(),
            acao: acao_do_evento(e.operacao),
            rowid: e.rowid,
            linha: Vec::new(),
            linha_antiga: Vec::new(),
            motivo: String::new(),
            cascata_na_lista: false,
            elo_do_empilhar: false,
            elo_da_cascata: false,
        })
        .collect();
    gravar_com(
        &caminho_da_marca(diretorio, id),
        id,
        carimbo_ms,
        &ops,
        Fim::Replica(eventos),
    )
}

/// O prefixo da marca do grupo do BIDIRECIONAL -- pedido 698.
///
/// # Por que outro nome, e nao outra versao
///
/// O conteudo e o da marca da replica, byte por byte, pelo mesmo
/// [`gravar_com`]: muda so QUEM a completa. A da replica se completa pelo
/// rowid e pela posicao do diario daqui ([`aplicar_evento_da_marca`]); a do
/// bidirecional, pela CHAVE e pelo «mais recente vence», que moram no
/// servidor e precisam do mapa de toques. Com o mesmo `transacao_`, a
/// recuperacao do store a leria como replica e gravaria pelo rowid de LA --
/// que no bidirecional e de outro servidor. Com outro nome a recuperacao do
/// store nem a ve, e um servidor anterior a deixa quieta em vez de
/// descarta-la.
pub const PREFIXO_DO_BIDI: &str = "bidi_";

/// Grava a marca do grupo do bidirecional e sincroniza, antes do primeiro
/// evento do grupo -- pedido 698. Ver [`PREFIXO_DO_BIDI`].
///
/// O `rowid` de cada evento e o de LA, e vai so para a marca ficar inteira:
/// quem completa casa pela chave, nunca por ele.
pub fn gravar_marca_do_bidi(
    diretorio: &Path,
    id: u64,
    carimbo_ms: i64,
    eventos: &[EventoDoGrupo<'_>],
) -> Result<PathBuf> {
    let ops: Vec<Escrita> = eventos
        .iter()
        .map(|e| Escrita {
            database: String::new(),
            tabela: e.tabela.to_string(),
            acao: acao_do_evento(e.operacao),
            rowid: e.rowid,
            linha: Vec::new(),
            linha_antiga: Vec::new(),
            motivo: String::new(),
            cascata_na_lista: false,
            elo_do_empilhar: false,
            elo_da_cascata: false,
        })
        .collect();
    let caminho = diretorio.join(format!("{PREFIXO_DO_BIDI}{id}.{EXTENSAO}"));
    gravar_com(&caminho, id, carimbo_ms, &ops, Fim::Replica(eventos))
}

/// Confere, SEM completar, as marcas do bidirecional de `db` -- pedido 723, o
/// palco da restauracao. Quem completa a do bidi e o servidor, pela chave e
/// pelo mapa de toques, e o palco nao tem servidor; entao ele so pergunta,
/// por evento, se o diario do palco ja o tem (o carimbo e a origem de la, que
/// o `aplicar_por_chave` grava):
///
/// - **todos**: a passada terminou e a marca so esperava o `fsync` -- sai;
/// - **nenhum**: o grupo nunca entrou nesta copia, e o destino o pede de novo
///   pelas posicoes dele -- sai tambem;
/// - **parte**: a copia pegou o grupo no meio, e restaurar entregaria meia
///   venda ate o proximo arranque do destino. Recusa nomeando a marca.
///
/// A marca que nao se le, ou cifrada sem a chave, tambem recusa: sem ler nao
/// ha como saber em qual dos tres casos ela esta. Devolve quantas sairam.
pub fn conferir_os_grupos_do_bidi(db: &Database) -> Result<usize> {
    let mut sairam = 0;
    for caminho in marcas_do_bidi_em(db.caminho()) {
        let marca = match ler_marca(&caminho)? {
            Leitura::Aberta(m) => m,
            Leitura::NaoConfere => {
                std::fs::remove_file(&caminho)?;
                sairam += 1;
                continue;
            }
            Leitura::SemChave(motivo) => {
                return Err(PhxError::Corrompido(format!(
                    "a copia traz a marca do bidirecional {} e esta chave nao a abre \
                     ({motivo}): sem le-la nao ha como saber se o grupo dela entrou \
                     inteiro. NADA foi restaurado",
                    caminho.display()
                )))
            }
        };
        let mut presentes = 0usize;
        let mut total = 0usize;
        let mut abertas: HashMap<String, Table> = HashMap::new();
        // Pedido 743: os eventos se contam por CESTO `(tabela, carimbo,
        // origem)`, e nao um a um. Os eventos de uma venda dividem o
        // milissegundo -- medido, 6 de 7 --, e o evento um a um respondia
        // «presente» para os cinco itens com UM no diario: na ultima tabela do
        // grupo, «parte» virava «todos» e a copia restaurava meia venda com
        // `ok: true`. O cesto so se enche com tantos eventos do diario quantos
        // a marca tem nele.
        let mut cestos: Vec<((String, i64, u16), usize)> = Vec::new();
        for op in &marca.operacoes {
            let Some(ev) = &op.replica else { continue };
            total += 1;
            let chave = (op.tabela.clone(), ev.carimbo_ms, ev.origem);
            match cestos.iter_mut().find(|(c, _)| *c == chave) {
                Some((_, n)) => *n += 1,
                None => cestos.push((chave, 1)),
            }
        }
        for ((tabela, carimbo, origem), n) in &cestos {
            if !abertas.contains_key(tabela) {
                abertas.insert(tabela.clone(), db.abrir_qualificada(tabela)?);
            }
            let t = abertas.get_mut(tabela).expect("acabou de entrar no mapa");
            presentes += eventos_de_la_no_diario(t, *carimbo, *origem, marca.carimbo_ms, *n)?;
        }
        // Nenhum evento no diario, mas a queda pode ter pego a PRIMEIRA
        // inclusao entre o slot e o evento (o 700): a linha no `.reg` sem
        // inclusao no diario e parte do grupo que entrou, e nao «nenhum».
        let mut orfa = None;
        if presentes == 0 {
            for (nome, t) in abertas.iter_mut() {
                let ultimo = t.slots();
                if ultimo > 0
                    && t.ler_sem_externos(ultimo)?.is_some()
                    && !inclusao_no_diario_pelo_rowid(t, ultimo)?
                {
                    orfa = Some(format!("{nome} rowid {ultimo}"));
                }
            }
        }
        if let Some(onde) = orfa {
            return Err(PhxError::Corrompido(format!(
                "a copia pegou no meio o grupo do bidirecional da marca {}: nenhum \
                 evento dele esta no diario, mas a linha {onde} esta no .reg sem a \
                 inclusao -- a queda entre o slot e o evento. NADA foi restaurado",
                caminho.display()
            )));
        }
        if presentes != 0 && presentes != total {
            return Err(PhxError::Corrompido(format!(
                "a copia pegou no meio o grupo do bidirecional da marca {}: {presentes} \
                 de {total} evento(s) estao no diario. Restaurada assim, ela mostraria \
                 a transacao pela metade -- use uma copia de antes ou de depois do \
                 grupo. NADA foi restaurado",
                caminho.display()
            )));
        }
        drop(abertas);
        std::fs::remove_file(&caminho)?;
        sairam += 1;
    }
    Ok(sairam)
}

/// Quantos eventos nascidos em `carimbo` na `origem` o diario de `t` tem, ate
/// `ate` -- pedido 743. Le de tras para a frente ate o primeiro evento de id
/// anterior a marca: o id sai do relogio deslocado 16 bits (pedido 676), e
/// todo evento aplicado por esta marca nasceu depois dela. Volume sem id
/// (zero) nao tem esse corte, e a leitura segue ate o comeco.
///
/// Conta, e nao responde «tem?»: `(carimbo, origem)` nao identifica UM evento
/// -- os de uma mesma transacao dividem o milissegundo --, entao a pergunta
/// certa e quantos do cesto entraram.
fn eventos_de_la_no_diario(
    t: &mut Table,
    carimbo: i64,
    origem: u16,
    desde_ms: i64,
    ate: usize,
) -> Result<usize> {
    const LOTE: u64 = 1024;
    let piso = (desde_ms.max(0) as u64) << 16;
    let mut achados = 0usize;
    let mut fim = t.eventos()?;
    while fim > 0 {
        let ini = fim.saturating_sub(LOTE);
        for e in t.diario(ini, fim - ini)?.iter().rev() {
            if e.carimbo == carimbo && e.origem == origem {
                achados += 1;
                if achados >= ate {
                    return Ok(achados);
                }
            }
            if e.tx != 0 && e.tx < piso {
                return Ok(achados);
            }
        }
        fim = ini;
    }
    Ok(achados)
}

/// As marcas do bidirecional de `dir`, na ordem do id -- o mesmo laco de
/// [`marcas_em`], com o outro prefixo.
pub fn marcas_do_bidi_em(dir: &Path) -> Vec<PathBuf> {
    marcas_com_prefixo(dir, PREFIXO_DO_BIDI)
}

/// `caminho` e uma marca do bidirecional? -- pedido 700. Pelo nome, que e o
/// que separa QUEM a completa: a do bidirecional casa pela chave, no
/// servidor, e nunca pelo rowid do [`tratar_marca`].
pub fn e_marca_do_bidi(caminho: &Path) -> bool {
    caminho
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with(PREFIXO_DO_BIDI))
}

/// O que vai no fim de cada operacao, depois do byte da cascata -- e e isso
/// que decide a versao da marca.
#[derive(Clone, Copy)]
enum Fim<'a> {
    /// Nada: a v3/v4 de um binario anterior ([`gravar_marca`]).
    Nada,
    /// O evento da replica (v5/v6), um por operacao, na mesma ordem.
    Replica(&'a [EventoDoGrupo<'a>]),
    /// O bilhete posicional (v7/v8).
    Bilhete(Bilhete<'a>),
}

/// O corpo unico das portas de gravacao.
fn gravar_com(
    caminho: &Path,
    id: u64,
    carimbo_ms: i64,
    ops: &[Escrita],
    fim: Fim<'_>,
) -> Result<PathBuf> {
    // O material e conferido AQUI, na criacao, e vale para a marca inteira.
    // Com o cofre desligado ele e `EM_CLARO`, e ai a marca nasce em claro --
    // a v7, ou a v3 de um binario anterior.
    let material = material_da_marca()?;
    let versao = match (material.cifrado(), fim) {
        (true, Fim::Nada) => VERSAO,
        (false, Fim::Nada) => VERSAO_CASCATA_EM_CLARO,
        (true, Fim::Replica(_)) => VERSAO_REPLICA_CIFRADA,
        (false, Fim::Replica(_)) => VERSAO_REPLICA_EM_CLARO,
        (true, Fim::Bilhete(_)) => VERSAO_POSICIONAL_CIFRADA,
        (false, Fim::Bilhete(_)) => VERSAO_POSICIONAL_EM_CLARO,
    };
    let replica = match fim {
        Fim::Replica(r) => Some(r),
        _ => None,
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
    // Pedido 709: o `tx` vai DEPOIS do material, logo antes do CRC -- o
    // material continua no mesmo lugar da v4, e o leitor dele nao muda.
    if let Fim::Bilhete(bl) = fim {
        b.extend_from_slice(&bl.tx.to_le_bytes());
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
        // Pedido 682 (v5/v6): o evento da replica vai DEPOIS do byte da
        // cascata, dentro do payload -- o selo e o CRC o cobrem junto, e a
        // unidade de dano continua sendo a operacao.
        if let Some(ev) = replica.and_then(|r| r.get(i)) {
            payload.extend_from_slice(&ev.carimbo_ms.to_le_bytes());
            payload.extend_from_slice(&ev.origem.to_le_bytes());
            payload.extend_from_slice(&ev.posicao.to_le_bytes());
            payload.extend_from_slice(&(ev.imagem.len() as u32).to_le_bytes());
            payload.extend_from_slice(ev.imagem);
        }
        // Pedido 709 (v7/v8): a versao de antes, no mesmo lugar -- o fim do
        // payload, sob o mesmo selo e o mesmo CRC.
        if let Fim::Bilhete(bl) = fim {
            payload.extend_from_slice(&bl.versoes_antes[i].to_le_bytes());
        }
        // O selo e POR OPERACAO, no MESMO bloco que o CRC ja cobria: a unidade
        // de dano continua sendo a operacao, e nao o arquivo. Selar a marca
        // inteira criaria uma segunda unidade de falha, maior do que a que o
        // formato ja tem -- e ai uma etiqueta que nao fechasse custaria a
        // transacao toda onde hoje custa o que o CRC daquele bloco custa.
        let guardado = material.selar(
            &nonce_da_operacao(id, i),
            &aad_da_operacao(
                id,
                nome,
                e.acao.tag(),
                e.rowid,
                sela_o_cabecalho(versao).then_some(CabecalhoNoSelo {
                    carimbo_ms,
                    n_operacoes: ops.len() as u32,
                    tx: match fim {
                        Fim::Bilhete(bl) => bl.tx,
                        _ => 0,
                    },
                    indice: i,
                }),
            ),
            &payload,
        );
        b.extend_from_slice(&(guardado.len() as u32).to_le_bytes());
        b.extend_from_slice(&guardado);
        let crc = crc32(&b[inicio..]);
        b.extend_from_slice(&crc.to_le_bytes());
    }

    let caminho = caminho.to_path_buf();
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
        VERSAO | VERSAO_REPLICA_CIFRADA => CAB_ATE_CRC_CIFRADA,
        VERSAO_CASCATA_EM_CLARO
        | VERSAO_LINHA_ANTIGA_SEM_CASCATA
        | VERSAO_SEM_LINHA_ANTIGA
        | VERSAO_REPLICA_EM_CLARO => CAB_ATE_CRC,
        VERSAO_POSICIONAL_EM_CLARO => CAB_ATE_CRC + 8,
        VERSAO_POSICIONAL_CIFRADA => CAB_ATE_CRC_CIFRADA + 8,
        // Pedido 734: versao MAIOR que a conhecida e marca de um binario mais
        // novo -- o que voltou depois de grava-la --, e nao «commit que nunca
        // comecou». Para e nao apaga, como a cifrada sem chave: apaga-la
        // jogaria fora uma transacao confirmada, e faria de toda versao nova
        // da marca um «nao volte o binario».
        v if v > VERSAO_POSICIONAL_CIFRADA => {
            return Ok(Leitura::SemChave(format!(
                "a marca e da versao {v}, mais nova que este binario (que le ate a \
                 {VERSAO_POSICIONAL_CIFRADA}): ela foi gravada por um binario mais novo e \
                 pode ser uma transacao confirmada -- suba o binario que a gravou"
            )))
        }
        _ => return Ok(Leitura::NaoConfere),
    };
    let posicional = matches!(
        versao,
        VERSAO_POSICIONAL_EM_CLARO | VERSAO_POSICIONAL_CIFRADA
    );
    if b.len() < ate_crc + 4 {
        return Ok(Leitura::NaoConfere);
    }
    let crc_lido = u32::from_le_bytes([b[ate_crc], b[ate_crc + 1], b[ate_crc + 2], b[ate_crc + 3]]);
    if crc32(&b[..ate_crc]) != crc_lido {
        return Ok(Leitura::NaoConfere);
    }
    let nome_do_arquivo = caminho.display().to_string();
    let da_replica = matches!(versao, VERSAO_REPLICA_EM_CLARO | VERSAO_REPLICA_CIFRADA);
    let material = if matches!(
        versao,
        VERSAO | VERSAO_REPLICA_CIFRADA | VERSAO_POSICIONAL_CIFRADA
    ) {
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
    let tx = if posicional {
        u64::from_le_bytes(b[ate_crc - 8..ate_crc].try_into().expect("oito bytes"))
    } else {
        0
    };
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
            &aad_da_operacao(
                id,
                &nome,
                tag,
                rowid,
                sela_o_cabecalho(versao).then_some(CabecalhoNoSelo {
                    carimbo_ms,
                    n_operacoes: n as u32,
                    tx,
                    indice: i,
                }),
            ),
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
        // Pedido 682: o evento da replica, logo depois do byte da cascata.
        // Faltar qualquer pedaco dele e marca que nao confere -- o mesmo que
        // o CRC quebrado, porque a operacao inteira esta sob o mesmo CRC.
        let replica = if da_replica {
            let Some(operacao) = evento_da_acao(acao) else {
                return Ok(Leitura::NaoConfere);
            };
            let mut r = Leitor {
                b: &payload,
                i: apos_antiga + 1,
            };
            let lido = (|| -> Result<EventoDaReplica> {
                let carimbo_ms = i64::from_le_bytes(r.fixo::<8>()?);
                let origem = r.u16()?;
                let posicao = r.u64()?;
                let imagem = r.bytes()?.to_vec();
                Ok(EventoDaReplica {
                    operacao,
                    carimbo_ms,
                    origem,
                    posicao,
                    imagem,
                })
            })();
            match lido {
                Ok(e) => Some(e),
                Err(_) => return Ok(Leitura::NaoConfere),
            }
        } else {
            None
        };
        // Pedido 709: a versao de antes, os oito ultimos bytes do payload.
        let versao_antes = if posicional {
            let mut r = Leitor {
                b: &payload,
                i: apos_antiga + 1,
            };
            match r.u64() {
                Ok(v) => Some(v),
                Err(_) => return Ok(Leitura::NaoConfere),
            }
        } else {
            None
        };
        operacoes.push(OperacaoDaMarca {
            replica,
            versao_antes,
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
        tx,
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
    /// Arvores sobre coluna marcada EM CLARO com o cofre ligado (pedido 339,
    /// condicao A do papel C): avisadas, nunca convertidas. Ver
    /// [`Database::indices_em_claro_sobre_coluna_marcada`].
    pub indices_em_claro: Vec<String>,
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
        self.indices_em_claro.extend(outro.indices_em_claro);
        self.impossiveis.extend(outro.impossiveis);
        self.paradas.extend(outro.paradas);
        self.sem_leitura.extend(outro.sem_leitura);
    }

    pub fn houve(&self) -> bool {
        self.achadas > 0
            || self.indices_reconstruidos > 0
            || !self.indices_pendentes.is_empty()
            || !self.indices_em_claro.is_empty()
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
        if !self.indices_em_claro.is_empty() {
            s.push_str(&format!(
                "\x20 indices EM CLARO sob o cofre .. {}   (coluna marcada; o `reindexar` sela)\n",
                self.indices_em_claro.len()
            ));
            for i in &self.indices_em_claro {
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
            replica: None,
            versao_antes: None,
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
    proximo_id_acima_de(maior_id_de_marca_em(dir))
}

/// O gerador UNICO de id de marca -- pedido 714: o relogio, ou um acima do
/// `maior` id que ja esta no disco, o que for maior. O embutido o chama com o
/// maior do diretorio dele; o servidor, no arranque, com o maior de todos os
/// databases, e dai em diante soma um.
///
/// Sem o piso do disco, o relogio recuado entre dois arranques fazia a marca
/// nova nascer ABAIXO de uma retida da vida anterior (cifrada sem a chave,
/// linha perdida): completada antes dela no arranque seguinte, contra a ordem
/// de criacao -- ou com o mesmo id, e o `create_new` recusava o `COMMIT`.
pub fn proximo_id_acima_de(maior: u64) -> u64 {
    (crate::util::agora_ms().max(1) as u64).max(maior.saturating_add(1))
}

/// O id de uma marca pelo nome do arquivo, de qualquer familia
/// (`transacao_`, `bidi_`). `None` = nao e marca.
fn id_da_marca(caminho: &Path) -> Option<u64> {
    let nome = caminho.file_name()?.to_str()?;
    let resto = nome
        .strip_prefix(PREFIXO)
        .or_else(|| nome.strip_prefix(PREFIXO_DO_BIDI))?;
    resto
        .strip_suffix(EXTENSAO)?
        .strip_suffix('.')?
        .parse::<u64>()
        .ok()
}

/// O maior id de marca de `dir`, das duas familias. 0 = nenhuma.
pub fn maior_id_de_marca_em(dir: &Path) -> u64 {
    marcas_com_prefixo(dir, PREFIXO)
        .iter()
        .chain(marcas_com_prefixo(dir, PREFIXO_DO_BIDI).iter())
        .filter_map(|c| id_da_marca(c))
        .max()
        .unwrap_or(0)
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
    // Pedido 701 (b): a marca se completa dentro de UMA unidade de transacao.
    // No arranque nao ha trava tomada, e fora de unidade cada evento ganhava
    // um id so dele (`log::tx_do_evento`): a replica encadeada recebia o
    // resto do grupo em pedacos, um por evento. No reparo de um panico a
    // tomada da trava ja tem a unidade -- a mesma que gravou a primeira
    // metade --, e ela fica.
    let unidade = UnidadeDaMarca::abrir();
    // Pedido 702: a marca do COMMIT completada no ARRANQUE adota o id da
    // metade que entrou antes da queda -- o irmao do `adotar_tx_na_unidade`
    // que a marca da replica faz pela posicao. So com a unidade propria: no
    // reparo de um panico a da tomada ja e a da primeira metade.
    //
    // Pedido 709: a marca v7/v8 TRAZ o id, reservado sob a trava antes do
    // primeiro evento -- nao ha o que adivinhar pela cauda. Ele so nao se
    // adota quando alguma tabela da marca ja tem cauda com id MAIOR: houve
    // escrita depois, a passada terminou, e um evento com o id velho depois
    // de um novo poria o diario fora de ordem.
    if unidade.0 {
        let tx = if marca.tx != 0 {
            (maior_cauda(db, marca) <= marca.tx).then_some(marca.tx)
        } else {
            id_da_metade_que_entrou(db, marca)
        };
        if let Some(tx) = tx {
            crate::log::semear_tx(tx);
            crate::log::adotar_tx_na_unidade(tx);
        }
    }
    let mut tabelas: HashMap<String, Table> = HashMap::new();
    let mut replica_parou = false;
    let mut reter = false;
    // Pedido 710: a versao de antes da PRIMEIRA operacao da marca em cada
    // linha -- a partir dela se sabe quantos eventos a linha deve ter.
    let mut primeira: HashMap<(&str, u64), u64> = HashMap::new();
    for op in &marca.operacoes {
        if let Some(v) = op.versao_antes {
            primeira.entry((op.tabela.as_str(), op.rowid)).or_insert(v);
        }
    }
    for (k, op) in marca.operacoes.iter().enumerate() {
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
                                replica_parou |= op.replica.is_some();
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
                    replica_parou |= op.replica.is_some();
                    continue;
                }
            }
        }
        // Pedido 682: na marca do grupo da replica, a primeira operacao que
        // nao entra PARA as seguintes. Elas viriam depois de um evento que o
        // diario nao tem -- e a recusa de posicao so olha o comprimento, que
        // a operacao seguinte ja acharia «certo» por cima de outra historia.
        if op.replica.is_some() && replica_parou {
            r.impossiveis.push(format!(
                "transacao {}: {} rowid {} em {} nao foi reaplicada -- um evento \
                 anterior do mesmo grupo da replica nao entrou",
                marca.id,
                op.acao.nome(),
                op.rowid,
                op.tabela
            ));
            continue;
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
            let bilhete = Faces {
                tx: marca.tx,
                primeira: primeira
                    .get(&(op.tabela.as_str(), op.rowid))
                    .copied()
                    .unwrap_or(0),
            };
            match aplicar_na_recuperacao(&mut t, op, &marca.operacoes[k + 1..], &mut maes, bilhete)
            {
                Ok(Desfecho::Aplicou) => r.reaplicadas += 1,
                Ok(Desfecho::JaEstava) => r.ja_aplicadas += 1,
                // Pedido 699: o diario diz que o evento entrou e o `.reg` nao
                // tem a linha. A marca FICA -- ela e a unica copia, aqui, da
                // imagem do que se perdeu --, e as operacoes seguintes do
                // grupo param como em qualquer recusa.
                Ok(Desfecho::LinhaPerdida(motivo)) => {
                    replica_parou = true;
                    reter = true;
                    r.impossiveis.push(format!(
                        "transacao {}: {} rowid {} em {} ({motivo}); a marca fica no disco",
                        marca.id,
                        op.acao.nome(),
                        op.rowid,
                        op.tabela
                    ))
                }
                Err(e) => {
                    replica_parou |= op.replica.is_some();
                    r.impossiveis.push(format!(
                        "transacao {}: {} rowid {} em {} ({e})",
                        marca.id,
                        op.acao.nome(),
                        op.rowid,
                        op.tabela
                    ))
                }
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
    no_disco && !reter
}

/// O id de transacao que a metade da marca do `COMMIT` que ENTROU no diario
/// levou antes da queda -- pedido 702. `None` = nao ha metade provada, ou a
/// passada terminou; o resto entao leva um id novo, como antes.
///
/// # Por que se procura no diario, e nao na marca
///
/// A marca nasce antes do primeiro evento, e o id so se tira no primeiro
/// evento (`log::tx_do_evento`): ela nao o sabe. Gravar o id nela mudaria o
/// formato para responder uma pergunta que o diario ja responde -- a passada
/// e uma so por vez sob a trava, entao a metade que entrou e a CAUDA de cada
/// tabela que ela tocou.
///
/// # O que conta como prova de que a cauda e desta marca
///
/// Errar para o lado de adotar juntaria esta transacao a ANTERIOR na replica
/// encadeada -- pior que o pedaco que o pedido conserta. Entao so a operacao
/// que se reconhece sem ambiguidade ancora o id ([`evento_da_operacao`]), duas
/// caudas que apontam ids diferentes nao ancoram nada, e duas condicoes
/// devolvem `None` mesmo com a ancora:
///
/// - alguma tabela da marca tem cauda com id MAIOR: houve commit depois, logo
///   a passada desta terminou e a marca so esperava a janela;
/// - os eventos com o id somam ao menos uma por operacao: a passada terminou,
///   e o que a recuperacao regrava e redundante -- ele nao se pendura num
///   grupo que a replica ja fechou.
fn id_da_metade_que_entrou(db: &Database, marca: &Marca) -> Option<u64> {
    if marca.operacoes.is_empty() || marca.operacoes.iter().any(|o| o.replica.is_some()) {
        return None;
    }
    let mut nomes: Vec<&str> = Vec::new();
    for op in &marca.operacoes {
        if !nomes.contains(&op.tabela.as_str()) {
            nomes.push(&op.tabela);
        }
    }
    let mut abertas = Vec::with_capacity(nomes.len());
    for nome in nomes {
        abertas.push((nome, db.abrir_qualificada(nome).ok()?));
    }
    let mut ancora: Option<u64> = None;
    let mut maior_cauda = 0u64;
    for (nome, t) in abertas.iter_mut() {
        let total = t.eventos().ok()?;
        if total == 0 {
            continue;
        }
        let (ev, imagem) = t.diario_com_imagem(total - 1, 1).ok()?.into_iter().next()?;
        maior_cauda = maior_cauda.max(ev.tx);
        let reconhecida = ev.tx != 0
            && marca
                .operacoes
                .iter()
                .filter(|op| op.tabela == *nome)
                .any(|op| evento_da_operacao(t, &ev, &imagem, op, marca.carimbo_ms));
        if reconhecida {
            if ancora.is_some_and(|a| a != ev.tx) {
                return None;
            }
            ancora = Some(ev.tx);
        }
    }
    let tx = ancora?;
    if maior_cauda > tx {
        return None;
    }
    let mut do_grupo = 0usize;
    for (nome, t) in abertas.iter_mut() {
        let ops_aqui = marca
            .operacoes
            .iter()
            .filter(|op| op.tabela == *nome)
            .count() as u64;
        let total = t.eventos().ok()?;
        let k = ops_aqui.min(total);
        do_grupo += t
            .diario(total - k, k)
            .ok()?
            .iter()
            .filter(|e| e.tx == tx)
            .count();
    }
    (do_grupo < marca.operacoes.len()).then_some(tx)
}

/// O maior id de transacao na cauda das tabelas que a marca nomeia -- pedido
/// 709. Tabela que nao abre conta como `u64::MAX`: sem a cauda nao ha como
/// provar que o id da marca ainda e o mais novo, e quem chama nao adota.
fn maior_cauda(db: &Database, marca: &Marca) -> u64 {
    let mut nomes: Vec<&str> = Vec::new();
    for op in &marca.operacoes {
        if !nomes.contains(&op.tabela.as_str()) {
            nomes.push(&op.tabela);
        }
    }
    let mut maior = 0u64;
    for nome in nomes {
        let cauda = (|| -> Result<u64> {
            let mut t = db.abrir_qualificada(nome)?;
            let total = t.eventos()?;
            if total == 0 {
                return Ok(0);
            }
            Ok(t.diario(total - 1, 1)?.first().map_or(0, |e| e.tx))
        })();
        maior = maior.max(cauda.unwrap_or(u64::MAX));
    }
    maior
}

/// O evento `ev` (com a `imagem`) e o que a operacao `op` da marca grava? So
/// as que se reconhecem sem ambiguidade -- pedido 702:
///
/// - a inclusao, pelo rowid: o `.reg` nao reaproveita slot, entao ha UMA
///   inclusao por rowid na vida da tabela;
/// - a exclusao de vez, pelo rowid e por nascer depois da marca: a linha nao
///   se apaga de vez duas vezes;
/// - a alteracao que muda alguma coisa, pela imagem igual a linha nova e
///   diferente da antiga: a do commit ANTERIOR na mesma linha tem a imagem da
///   antiga desta.
///
/// A exclusao suave e a restauracao gravam alteracao com a imagem do proprio
/// motor, e a alteracao que nao muda nada nao se distingue da anterior: nao
/// ancoram, e a marca que so tem delas leva um id novo, como antes.
fn evento_da_operacao(
    t: &mut Table,
    ev: &crate::log::Evento,
    imagem: &[u8],
    op: &OperacaoDaMarca,
    carimbo_da_marca: i64,
) -> bool {
    use crate::log::Operacao;
    if ev.rowid != op.rowid {
        return false;
    }
    match op.acao {
        Acao::Inserir => ev.operacao == Operacao::Inclusao,
        Acao::ExcluirDeVez => ev.operacao == Operacao::Exclusao && ev.carimbo >= carimbo_da_marca,
        Acao::Atualizar => {
            ev.operacao == Operacao::Alteracao
                && ev.carimbo >= carimbo_da_marca
                && !op.linha_antiga.is_empty()
                && op.linha != op.linha_antiga
                && !imagem.is_empty()
                && t.valores_da_imagem(imagem).is_ok_and(|v| v == op.linha)
        }
        Acao::ExcluirSuave | Acao::Restaurar => false,
    }
}

/// A unidade de transacao que a recuperacao de UMA marca abre quando ninguem
/// abriu -- pedido 701 (b) --, e a cascata do embutido tambem (pedido 716).
/// Fecha no `Drop`, para o `?` e o panico no meio nao deixarem a thread presa
/// numa unidade que nao e de mais ninguem.
pub(crate) struct UnidadeDaMarca(bool);

impl UnidadeDaMarca {
    pub(crate) fn abrir() -> UnidadeDaMarca {
        let propria = !crate::log::unidade_aberta();
        if propria {
            crate::log::abrir_unidade();
        }
        UnidadeDaMarca(propria)
    }
}

impl Drop for UnidadeDaMarca {
    fn drop(&mut self) {
        if self.0 {
            crate::log::fechar_unidade();
        }
    }
}

/// O que a reaplicacao de UMA operacao da marca fez.
#[derive(Debug, PartialEq, Eq)]
enum Desfecho {
    Aplicou,
    JaEstava,
    /// O diario daqui tem o evento e o `.reg` nao tem o que ele diz -- pedido
    /// 699. Nao e «ja estava»: e dado que o disco perdeu, e a marca que o
    /// descreve nao pode sair.
    LinhaPerdida(String),
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
    match aplicar_na_recuperacao(t, op, &[], maes, Faces::SEM_BILHETE)? {
        Desfecho::Aplicou => Ok(true),
        Desfecho::JaEstava => Ok(false),
        Desfecho::LinhaPerdida(motivo) => Err(PhxError::Corrompido(motivo)),
    }
}

/// O slot da inclusao `rowid` ja foi consumido? `Ok(true)` = sim, e a linha
/// esta nele; `Ok(false)` = ainda nao, ele vem depois do ultimo.
///
/// # Uma resposta so, para a marca do `COMMIT` e para a da replica
///
/// As duas perguntam a mesma coisa antes de gravar -- «a passada ja chegou
/// neste slot?» -- e a da replica nao perguntava (pedido 699): conferia so o
/// diario, e a queda entre o `.reg` e o evento fazia a reaplicacao gravar a
/// linha num slot NOVO, duplicada.
///
/// Slot dentro da faixa e LIVRE recusa: o `.reg` nao reaproveita slot, entao
/// nao ha como refazer esta linha no lugar dela. E a unica lacuna deste
/// desenho, e ela esta escrita na §5.4 do documento em vez de escondida.
fn slot_ja_consumido(t: &mut Table, rowid: u64) -> Result<bool> {
    if rowid > t.slots() {
        return Ok(false);
    }
    if t.ler(rowid)?.is_some() {
        return Ok(true);
    }
    Err(PhxError::Corrompido(format!(
        "o slot {rowid} ja foi consumido e esta livre; o .reg nao \
         reaproveita slot, entao esta linha nao volta para o \
         lugar dela"
    )))
}

/// O corpo do [`aplicar_uma`], com o desfecho inteiro -- a recuperacao precisa
/// distinguir a linha perdida de uma recusa qualquer.
///
/// `seguintes` sao as operacoes da MESMA marca depois desta: o slot que o
/// diario daqui ainda nao explica pode estar explicado por uma delas (pedido
/// 701) -- a exclusao seguinte que tambem nao chegou ao diario, a alteracao
/// seguinte que chegou ao `.reg` e nao ao evento.
fn aplicar_na_recuperacao(
    t: &mut Table,
    op: &OperacaoDaMarca,
    seguintes: &[OperacaoDaMarca],
    maes: &mut dyn MaesEmProgresso,
    faces: Faces,
) -> Result<Desfecho> {
    if let Some(ev) = &op.replica {
        let seguintes = Seguintes {
            tabela: &op.tabela,
            ops: seguintes,
        };
        return aplicar_evento_da_marca(t, op.rowid, ev, seguintes);
    }
    let feito = |b: bool| {
        if b {
            Desfecho::Aplicou
        } else {
            Desfecho::JaEstava
        }
    };
    if let Some(antes) = op.versao_antes {
        if a_linha_ja_passou(t, op, antes)? {
            return so_o_diario(t, op, antes, faces);
        }
    }
    match op.acao {
        Acao::Inserir => {
            // O slot ja existe? Entao a passada chegou nele e o `.reg` nao
            // tem o que fazer -- a reaplicacao e idempotente pelo rowid, e e
            // por isso que a marca guarda o rowid alvo e nao so a linha. O
            // diario pode nao ter chegado (pedido 710): `so_o_diario`.
            if slot_ja_consumido(t, op.rowid)? {
                return so_o_diario(t, op, 0, faces);
            }
            let saiu = t.inserir_com_maes(&op.linha, maes)?;
            if saiu != op.rowid {
                return Err(PhxError::Corrompido(format!(
                    "a marca dizia rowid {} e a insercao saiu {saiu}",
                    op.rowid
                )));
            }
            Ok(Desfecho::Aplicou)
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
                return Ok(Desfecho::Aplicou);
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
            Ok(Desfecho::Aplicou)
        }
        // O irmao da passada: as exclusoes emprestam as FILHAS que a mesma
        // marca ja reaplicou, e a restauracao as MAES (pedido 448).
        Acao::ExcluirSuave => Ok(feito(t.excluir_suave_com_maes(op.rowid, &op.motivo, maes)?)),
        Acao::ExcluirDeVez => Ok(feito(
            t.excluir_de_vez_com_maes(op.rowid, &op.motivo, maes)?,
        )),
        Acao::Restaurar => Ok(feito(t.restaurar_com_maes(op.rowid, &op.motivo, maes)?)),
    }
}

/// A linha da operacao `op` ja passou dela? -- pedido 709, a pergunta da
/// marca v7/v8.
///
/// A versao do slot so anda, e `antes` e a que ele tinha quando a passada ia
/// chegar nesta operacao. Versao MAIOR: a passada ja gravou esta operacao, ou
/// gravou e alguem escreveu depois -- uma escrita SOLTA, sem bilhete, durante
/// a janela de durabilidade. Nos dois casos reaplicar regrava por cima de
/// estado mais novo (o F1: o «a» do `COMMIT` voltava por cima do «b» da
/// solta) e acrescenta ao diario um evento que nenhum cliente fez (o F3).
///
/// Slot livre dentro da faixa: a linha saiu de vez -- por esta marca ou depois
/// dela -- e o `.reg` nao reaproveita slot, entao nada desta operacao tem onde
/// entrar. Alem do fim, ou na versao de antes, a operacao nao entrou: segue o
/// caminho de sempre, que a aplica (ou recusa dizendo por que).
///
/// A inclusao nao passa por aqui: o slot consumido ja a responde
/// ([`slot_ja_consumido`]).
fn a_linha_ja_passou(t: &mut Table, op: &OperacaoDaMarca, antes: u64) -> Result<bool> {
    if op.acao == Acao::Inserir || op.rowid == 0 || op.rowid > t.slots() {
        return Ok(false);
    }
    Ok(match t.versao(op.rowid)? {
        Some(v) => v > antes,
        None => true,
    })
}

/// O que a regra das duas faces precisa saber da marca alem da operacao --
/// pedido 710.
#[derive(Clone, Copy)]
struct Faces {
    /// Ver [`Marca::tx`]. 0 = a marca nao o traz (v1-v6, ou gravada fora de
    /// unidade), e ai so a inclusao se confere, pelo rowid.
    tx: u64,
    /// A versao de antes da PRIMEIRA operacao desta marca nesta linha.
    primeira: u64,
}

impl Faces {
    /// A operacao aplicada fora de recuperacao (a cascata do embutido): a
    /// passada esta acontecendo agora, e nao ha diario a completar.
    const SEM_BILHETE: Faces = Faces { tx: 0, primeira: 0 };
}

/// O `.reg` ja passou da operacao `op`: falta o diario? -- pedido 710, a
/// regra das duas faces (`recuperacao-e-replica-desenho-unico.md` §3.3) na
/// marca do `COMMIT`.
///
/// A passada grava o slot ANTES do evento, e o `SIGKILL` entre os dois deixa
/// a linha no `.reg` sem o evento. Responder «ja estava» olhando so o `.reg`
/// -- o que a inclusao fazia -- deixava a linha fora do diario para sempre:
/// a replica nunca a recebia, e o `verificar` nao acusa. A resposta e a do
/// 699 na marca da replica: completar SO o evento, sem tocar no `.reg`.
///
/// Como se sabe que falta, por operacao:
///
/// - **inclusao e exclusao de vez** acontecem uma vez por linha: o evento
///   dela esta no diario ou nao esta;
/// - **alteracao, exclusao suave e restauracao**, so com o `tx` da marca
///   (v7/v8) e so se o slot esta EXATAMENTE uma versao depois desta operacao:
///   os eventos da linha com o `tx` da marca contam as escritas dela, e a
///   versao conta as mesmas escritas no `.reg`. Uma a menos no diario e a que
///   a queda levou -- so pode ser a ultima, porque a passada para ali.
///
/// Escrita de id MAIOR na linha (`depois`) quer dizer que a passada terminou
/// e alguem gravou depois: nada a completar.
fn so_o_diario(t: &mut Table, op: &OperacaoDaMarca, antes: u64, faces: Faces) -> Result<Desfecho> {
    use crate::log::Operacao;
    let linha = if faces.tx == 0 {
        None
    } else {
        diario_da_linha(t, op.rowid, faces.tx)?
    };
    let Some(linha) = linha else {
        // Sem o id -- a marca nao o traz, ou o volume do diario e anterior ao
        // id no evento (676) e grava zero --, so a inclusao se reconhece, pela
        // ordem do rowid. Contar pelo id ali acharia o diario sem nada desta
        // marca e completaria de novo cada inclusao: o evento em dobro.
        if op.acao == Acao::Inserir && !inclusao_no_diario_pelo_rowid(t, op.rowid)? {
            t.completar_o_diario_da_inclusao(op.rowid, &[])?;
            return Ok(Desfecho::Aplicou);
        }
        return Ok(Desfecho::JaEstava);
    };
    if linha.depois {
        return Ok(Desfecho::JaEstava);
    }
    let tem = |qual: Operacao| linha.com_o_tx.contains(&qual);
    match op.acao {
        Acao::Inserir if !tem(Operacao::Inclusao) => {
            t.completar_o_diario_da_inclusao(op.rowid, &[])?;
            Ok(Desfecho::Aplicou)
        }
        Acao::ExcluirDeVez if !tem(Operacao::Exclusao) && t.versao(op.rowid)?.is_none() => {
            t.completar_o_diario_da_exclusao(op.rowid, &[])?;
            Ok(Desfecho::Aplicou)
        }
        Acao::Atualizar | Acao::ExcluirSuave | Acao::Restaurar => {
            let devidos = (antes + 1).saturating_sub(faces.primeira);
            if t.versao(op.rowid)? == Some(antes + 1) && linha.com_o_tx.len() as u64 + 1 == devidos
            {
                t.completar_o_diario_da_alteracao(op.rowid)?;
                return Ok(Desfecho::Aplicou);
            }
            Ok(Desfecho::JaEstava)
        }
        _ => Ok(Desfecho::JaEstava),
    }
}

/// O que o diario de `t` tem da linha `rowid` com o id `tx`, e se alguma
/// escrita de id MAIOR a tocou depois -- pedido 710.
struct NaLinha {
    com_o_tx: Vec<crate::log::Operacao>,
    depois: bool,
}

/// Le o diario de `t` de tras para a frente ate o primeiro evento de id
/// menor que `tx`: o id so cresce no diario de uma tabela (pedido 684), entao
/// tudo o que esta marca gravou -- e tudo o que veio depois -- esta nessa
/// cauda. Custa a cauda desde o `COMMIT`, que e a janela de durabilidade.
///
/// `None` = a cauda tem evento SEM id (volume 2/3, anterior ao 676): ali o
/// id nao separa esta marca de nada, e quem chama decide sem ele.
fn diario_da_linha(t: &mut Table, rowid: u64, tx: u64) -> Result<Option<NaLinha>> {
    const LOTE: u64 = 1024;
    let mut r = NaLinha {
        com_o_tx: Vec::new(),
        depois: false,
    };
    let mut fim = t.eventos()?;
    while fim > 0 {
        let ini = fim.saturating_sub(LOTE);
        let lote = t.diario(ini, fim - ini)?;
        for e in lote.iter().rev() {
            if e.tx == 0 {
                return Ok(None);
            }
            if e.tx < tx {
                return Ok(Some(r));
            }
            if e.rowid != rowid {
                continue;
            }
            if e.tx == tx {
                r.com_o_tx.push(e.operacao);
            } else {
                r.depois = true;
            }
        }
        fim = ini;
    }
    Ok(Some(r))
}

/// A inclusao do `rowid` esta no diario? Sem o id da marca, pela ordem: o
/// `.reg` so anexa, entao as inclusoes entram no diario na ordem do rowid, e
/// a primeira inclusao MENOR achada de tras para a frente diz que a desta
/// linha nao esta.
fn inclusao_no_diario_pelo_rowid(t: &mut Table, rowid: u64) -> Result<bool> {
    const LOTE: u64 = 1024;
    let mut fim = t.eventos()?;
    while fim > 0 {
        let ini = fim.saturating_sub(LOTE);
        let lote = t.diario(ini, fim - ini)?;
        for e in lote.iter().rev() {
            if e.operacao == crate::log::Operacao::Inclusao {
                if e.rowid == rowid {
                    return Ok(true);
                }
                if e.rowid < rowid {
                    return Ok(false);
                }
            }
        }
        fim = ini;
    }
    Ok(false)
}

/// Reaplica um evento do grupo da replica -- pedido 682. `Ok(false)` = ja
/// estava aplicado.
///
/// # A idempotencia e pela POSICAO do diario, conferida
///
/// O rowid da `aplicar_uma` nao basta aqui: a alteracao reaplicada grava a
/// mesma linha e acrescenta um evento que a origem nao tem, e o diario daqui
/// deixaria de continuar o de la. Entao:
///
/// - o diario tem exatamente `posicao` eventos: este e o proximo, aplica;
/// - tem mais: o evento em `posicao` tem de ser ESTE (carimbo, operacao e
///   rowid, os campos da conferencia de continuidade da replica). Sendo,
///   ja entrou -- se o `.reg` confirmar (pedido 699,
///   [`conferir_o_reg_do_evento`]); nao sendo, a marca e de outra historia da tabela -- recriada
///   depois da queda, ou escrita por fora -- e reaplicar por cima seria
///   gravar dado alheio. Recusa, e o relatorio a conta como impossivel;
/// - tem menos: falta evento ANTES deste, e aplicar abriria um buraco que o
///   diario nao tem como representar. Recusa tambem.
///
/// O aplicador e o da replica ([`Table::aplicar_evento`]), com o carimbo e a
/// origem de la, e sem julgar a chave estrangeira -- conta a orfa, como a
/// rodada conta.
fn aplicar_evento_da_marca(
    t: &mut Table,
    rowid: u64,
    ev: &EventoDaReplica,
    seguintes: Seguintes<'_>,
) -> Result<Desfecho> {
    let tem = t.eventos()?;
    if tem > ev.posicao {
        let la = t.diario(ev.posicao, 1)?;
        return match la.first() {
            Some(e)
                if e.carimbo == ev.carimbo_ms && e.operacao == ev.operacao && e.rowid == rowid =>
            {
                // Pedido 701 (b): o evento que ja entrou diz o id da unidade
                // que gravou a primeira metade do grupo, e o resto o adota --
                // so se a unidade desta recuperacao ainda nao gravou nada.
                crate::log::adotar_tx_na_unidade(e.tx);
                conferir_o_reg_do_evento(t, rowid, ev, seguintes)
            }
            _ => Err(PhxError::Corrompido(format!(
                "o evento {} do diario de {} nao e o que a marca do grupo da replica \
                 traz ({} rowid {rowid}): a tabela mudou de historia depois da queda, \
                 e reaplicar gravaria por cima de dado de outra historia",
                ev.posicao,
                t.nome(),
                ev.operacao.nome()
            ))),
        };
    }
    if tem < ev.posicao {
        return Err(PhxError::Corrompido(format!(
            "o diario de {} tem {tem} evento(s) e a marca do grupo da replica traz o \
             evento {}: faltam os do meio, e a replicacao os pede de novo da origem",
            t.nome(),
            ev.posicao
        )));
    }
    t.contar_orfas();
    t.forcar_proximo_evento(ev.carimbo_ms, ev.origem);
    // Pedido 699: o diario esta na posicao, mas o `.reg` pode estar a frente
    // dele -- a inclusao grava o slot ANTES do evento, e a queda entre os dois
    // deixa a linha sem evento. Reaplicar pelo `aplicar_evento` gravaria a
    // linha de novo num slot NOVO (o `.reg` nao reaproveita slot), duplicada
    // se a tabela nao tiver indice unico. A pergunta e a mesma do `COMMIT`
    // ([`slot_ja_consumido`]); o que muda e a resposta ao «ja estava»: aqui
    // falta o evento, e ele se completa SEM tocar no `.reg`, senao a rodada
    // seguinte pediria este evento de novo a origem.
    if ev.operacao == crate::log::Operacao::Inclusao && slot_ja_consumido(t, rowid)? {
        t.completar_o_diario_da_inclusao(rowid, &ev.imagem)?;
        return Ok(Desfecho::Aplicou);
    }
    // Pedido 701 (c), o irmao da exclusao: o slot sai do `.reg` ANTES do
    // evento, e a queda entre os dois deixa o slot livre com o diario na
    // posicao. Reaplicar pelo `aplicar_evento` recusava («aqui ele nao
    // existe») e a marca avisava a cada arranque; o evento se completa da
    // lixeira, que guardou a linha antes de o slot sair.
    if ev.operacao == crate::log::Operacao::Exclusao
        && rowid >= 1
        && rowid <= t.slots()
        && t.ler_sem_externos(rowid)?.is_none()
    {
        t.completar_o_diario_da_exclusao(rowid, &ev.imagem)?;
        return Ok(Desfecho::Aplicou);
    }
    t.aplicar_evento(ev.operacao, rowid, &ev.imagem)?;
    Ok(Desfecho::Aplicou)
}

/// O evento ja esta no diario daqui: o `.reg` confirma? -- pedido 699, o
/// inverso da queda entre o `.reg` e o evento.
///
/// A queda de ENERGIA antes do `fsync` pode deixar o diario no disco e o
/// `.reg` nao. Responder «ja estava» olhando so o diario apagava a marca com
/// a linha ausente -- e a marca era, aqui, a unica copia da imagem dela.
///
/// Inclusao e alteracao deixam a linha no slot; a exclusao o deixa livre.
/// Slot livre depois de uma inclusao ou alteracao so e legitimo se um evento
/// POSTERIOR do mesmo diario o excluiu -- e e o diario que diz. E a linha
/// presente tem de ser a do evento (pedido 701, a): presenca sozinha deixava
/// sair a marca com a versao velha no slot.
fn conferir_o_reg_do_evento(
    t: &mut Table,
    rowid: u64,
    ev: &EventoDaReplica,
    seguintes: Seguintes<'_>,
) -> Result<Desfecho> {
    use crate::log::Operacao;
    let presente = rowid >= 1 && rowid <= t.slots() && t.ler_sem_externos(rowid)?.is_some();
    let perdida = match ev.operacao {
        Operacao::Exclusao if presente => Some(format!(
            "o diario de {} tem a exclusao do rowid {rowid} e o .reg ainda tem a linha: \
             a exclusao nao chegou ao disco",
            t.nome()
        )),
        Operacao::Exclusao => None,
        _ if presente && o_reg_tem_a_imagem(t, rowid, ev)? => None,
        // Pedido 701 (a): a linha esta la com OUTRO conteudo. So e legitimo
        // se uma alteracao posterior do mesmo diario o trocou; senao e a
        // queda de energia que levou a versao nova do `.reg` e deixou a velha
        // -- e a marca era a unica copia da nova.
        _ if presente
            && tocada_depois(t, rowid, ev.posicao + 1, Operacao::Alteracao, seguintes)? =>
        {
            None
        }
        op if presente => Some(format!(
            "o diario de {} tem a {} do rowid {rowid} e o .reg tem a linha com \
             outro conteudo: a versao que o diario confirma nao chegou ao disco",
            t.nome(),
            op.nome()
        )),
        _ if tocada_depois(t, rowid, ev.posicao + 1, Operacao::Exclusao, seguintes)? => None,
        op => Some(format!(
            "o diario de {} tem a {} do rowid {rowid} e o .reg nao tem a linha: o \
             disco perdeu o slot que o diario confirma",
            t.nome(),
            op.nome()
        )),
    };
    Ok(match perdida {
        Some(motivo) => Desfecho::LinhaPerdida(motivo),
        None => Desfecho::JaEstava,
    })
}

/// A linha `rowid` do `.reg` e a que a imagem do evento traz? -- pedido 701
/// (a).
///
/// Compara VALORES, e nao bytes: o payload daqui aponta os externos para o
/// `.memo` daqui e sela com a chave daqui, e a imagem traz o conteudo aberto.
/// Os dois passam pelo mesmo decodificador e sao a mesma linha se e so se os
/// valores batem. Evento sem imagem nao tem com o que comparar, e a presenca
/// e tudo o que se sabe -- e o que a recuperacao sabia antes.
fn o_reg_tem_a_imagem(t: &mut Table, rowid: u64, ev: &EventoDaReplica) -> Result<bool> {
    if ev.imagem.is_empty() {
        return Ok(true);
    }
    let de_la = t.valores_da_imagem(&ev.imagem)?;
    Ok(t.ler(rowid)?.as_deref() == Some(de_la.as_slice()))
}

/// As operacoes da marca depois da que se reaplica, e a tabela dela -- ver
/// [`aplicar_na_recuperacao`].
#[derive(Clone, Copy)]
struct Seguintes<'a> {
    tabela: &'a str,
    ops: &'a [OperacaoDaMarca],
}

impl Seguintes<'_> {
    /// Alguma seguinte, nesta tabela, faz `qual` no `rowid`? So as do grupo
    /// da replica: e o evento que diz a operacao.
    fn tem(&self, rowid: u64, qual: crate::log::Operacao) -> bool {
        self.ops.iter().any(|op| {
            op.rowid == rowid
                && op.tabela == self.tabela
                && op.replica.as_ref().is_some_and(|ev| ev.operacao == qual)
        })
    }
}

/// O diario daqui tem a operacao `qual` sobre `rowid` em algum evento a
/// partir de `desde` -- ou uma operacao SEGUINTE da mesma marca, nesta
/// tabela, a tem?
///
/// A marca conta porque a queda pode ter levado o evento da seguinte junto
/// (pedido 701): o diario ainda nao a tem, e ela nao passa sem conferencia
/// propria -- a exclusao seguinte so se completa se a lixeira tiver a linha,
/// a alteracao seguinte reaplica o conteudo dela.
fn tocada_depois(
    t: &mut Table,
    rowid: u64,
    desde: u64,
    qual: crate::log::Operacao,
    seguintes: Seguintes<'_>,
) -> Result<bool> {
    if seguintes.tem(rowid, qual) {
        return Ok(true);
    }
    const LOTE: u64 = 1024;
    let total = t.eventos()?;
    let mut pos = desde;
    while pos < total {
        let lote = t.diario(pos, LOTE)?;
        if lote.is_empty() {
            break;
        }
        if lote.iter().any(|e| e.rowid == rowid && e.operacao == qual) {
            return Ok(true);
        }
        pos += lote.len() as u64;
    }
    Ok(false)
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
            // O schema leva a POLITICA do database (pedido 601): a marca de
            // uma tabela de schema mora na pasta do schema, e um `Database`
            // novo nasceria com o padrao desligado -- o COMMIT completado ali
            // iria sem imagem, o mesmo defeito do 564 um diretorio abaixo.
            if let Ok(d) = self.diretorio(Some(&s)) {
                schemas.push(Database::no_diretorio_com_politica(
                    &d,
                    self.politica_do_diario(),
                ));
            }
        }
        for db in std::iter::once(self).chain(schemas.iter()) {
            completar_as_marcas_de(db, &mut r);
        }
        let (feitas, pendentes) = self.reconstruir_indices_marcados();
        r.indices_reconstruidos += feitas;
        r.indices_pendentes.extend(pendentes);
        // DEPOIS da reconstrucao: a arvore que o arranque acabou de refazer
        // ja nasceu selada e nao entra no aviso.
        r.indices_em_claro = self.indices_em_claro_sobre_coluna_marcada();
        r.ms = comeco.elapsed().as_millis() as u64;
        r
    }
}

impl Database {
    /// O maior id de marca deste database -- a raiz e a pasta de cada schema,
    /// as duas familias. E o piso do contador do servidor (pedido 714).
    pub fn maior_id_de_marca(&self) -> u64 {
        let mut maior = maior_id_de_marca_em(self.caminho());
        for s in self.schemas().unwrap_or_default() {
            if let Ok(d) = self.diretorio(Some(&s)) {
                maior = maior.max(maior_id_de_marca_em(&d));
            }
        }
        maior
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
    marcas_com_prefixo(dir, PREFIXO)
}

fn marcas_com_prefixo(dir: &Path, prefixo: &str) -> Vec<PathBuf> {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut marcas: Vec<PathBuf> = entradas
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(prefixo) && n.ends_with(&format!(".{EXTENSAO}")))
        })
        .collect();
    // Pela ordem NUMERICA do id, e nao pelo texto (pedido 714, o F12): o
    // texto so concorda com o numero enquanto todos os ids tiverem a mesma
    // largura, e o piso do disco ou um relogio de outra decada a muda.
    //
    // E ANTES do id, de que lado da trava a marca nasceu (pedido 715): a
    // ordem de completar e a ordem em que a TRAVA as teria aplicado. A do
    // `COMMIT` e a da cascata solta nascem com a trava na mao, e no maximo
    // uma delas escreve no arranque; a do grupo da replica (v5/v6) nasce FORA
    // dela e so entra depois de toma-la. O id nao diz isso: o do `COMMIT` sai
    // no `BEGIN`, e um `BEGIN` depois da marca do grupo tem id maior e entra
    // antes. Pelo id, o arranque completava o grupo primeiro, o grupo tomava o
    // rowid que o `COMMIT` tinha planejado, e o `COMMIT` saia pela metade dado
    // como completado. Pelo nome nao da: as duas familias dividem o prefixo.
    marcas.sort_by_key(|p| {
        (
            nasce_fora_da_trava(p),
            id_da_marca(p).unwrap_or(u64::MAX),
            p.clone(),
        )
    });
    marcas
}

/// A marca nasceu FORA da trava de dados? -- pedido 715. Pela versao do
/// cabecalho: a v5/v6 (grupo da replica e do bidirecional) se grava antes de
/// tomar a trava; as outras, com ela. Marca que nem se le fica com as da
/// trava: quem decide o que fazer com ela e a leitura de verdade, depois.
fn nasce_fora_da_trava(caminho: &Path) -> bool {
    use std::io::Read;
    let mut cab = [0u8; 12];
    let lido = std::fs::File::open(caminho).and_then(|mut f| f.read_exact(&mut cab));
    lido.is_ok()
        && &cab[..8] == MAGIC
        && matches!(
            u32::from_le_bytes([cab[8], cab[9], cab[10], cab[11]]),
            VERSAO_REPLICA_EM_CLARO | VERSAO_REPLICA_CIFRADA
        )
}

/// Completa as marcas de UM diretorio de tabelas -- o `reindex` do CLI, que
/// recebe o diretorio e nao o database. E o mesmo laco da
/// [`Database::recuperar_marcas`], sem o passe do indice marcado: quem chama
/// reconstroi a tabela dele logo em seguida.
///
/// A `politica` e parametro, e nao padrao, de proposito (pedido 601): quem
/// recebe so o diretorio nao sabe se aquela base replica, e um padrao aqui
/// seria o «desligado» decidido calado -- o COMMIT completado iria sem imagem
/// e a replica pararia nele. Quem chama tem de dizer.
pub fn recuperar_no_diretorio(
    dir: &Path,
    politica: crate::catalogo::PoliticaDoDiario,
) -> Relatorio {
    let comeco = std::time::Instant::now();
    let mut r = Relatorio::default();
    completar_as_marcas_de(&Database::no_diretorio_com_politica(dir, politica), &mut r);
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

    // ------------------------------------------ o bilhete posicional (709)

    /// A v7 volta com o `tx` e a versao de antes de cada operacao, e o resto
    /// da operacao igual ao da v3.
    #[test]
    fn a_marca_v7_volta_com_o_bilhete() {
        let d = dir("v7");
        let mut ops = uma_escrita("Blumenau");
        ops.push(ops[0].clone());
        let bilhete = Bilhete {
            tx: 77 << 16,
            versoes_antes: &[3, 4],
        };
        let caminho = gravar_marca_posicional(&d, 14, 5, &ops, bilhete).unwrap();
        let b = std::fs::read(&caminho).unwrap();
        assert_eq!(
            u32::from_le_bytes([b[8], b[9], b[10], b[11]]),
            VERSAO_POSICIONAL_EM_CLARO
        );
        let m = ler_marca(&caminho).unwrap().marca().expect("a v7 confere");
        assert_eq!((m.id, m.carimbo_ms, m.tx), (14, 5, 77 << 16));
        let v: Vec<_> = m.operacoes.iter().map(|o| o.versao_antes).collect();
        assert_eq!(v, vec![Some(3), Some(4)]);
        assert_eq!(m.operacoes[0].linha, ops[0].linha);
        // A v3 continua sem bilhete: a recuperacao dela faz o de sempre.
        let v3 = gravar_marca(&d, 15, 5, &ops).unwrap();
        let m3 = ler_marca(&v3).unwrap().marca().unwrap();
        assert_eq!(m3.tx, 0);
        assert!(m3.operacoes.iter().all(|o| o.versao_antes.is_none()));
    }

    /// Um bilhete com uma versao a menos nao vira marca: a operacao sem
    /// versao seria reaplicada sem a pergunta que a v7 existe para fazer.
    #[test]
    fn bilhete_de_tamanho_errado_nao_vira_marca() {
        let d = dir("v7-curta");
        let bilhete = Bilhete {
            tx: 1,
            versoes_antes: &[],
        };
        assert!(gravar_marca_posicional(&d, 16, 0, &uma_escrita("x"), bilhete).is_err());
        assert!(marcas_em(&d).is_empty());
    }

    /// **Pedido 734 (M1).** A marca de uma versao MAIS NOVA que este binario
    /// (o binario voltou depois de gravar uma marca da versao seguinte) nao e
    /// «commit que nunca comecou»: para e nao apaga, como a cifrada sem chave.
    /// Os maduros convergem em nao descartar o que nao entendem (o PostgreSQL
    /// da FATAL na versao do controle, o InnoDB recusa o redo de formato novo).
    ///
    /// Prova real: com o `_ => NaoConfere` de volta, a leitura diz «nao
    /// confere» e a recuperacao APAGA a marca.
    #[test]
    fn a_marca_de_versao_mais_nova_para_e_nao_e_apagada() {
        let d = dir("versao-nova");
        let caminho = gravar_marca_posicional(
            &d,
            21,
            0,
            &uma_escrita("x"),
            Bilhete {
                tx: 1,
                versoes_antes: &[0],
            },
        )
        .unwrap();
        let mut b = std::fs::read(&caminho).unwrap();
        b[8..12].copy_from_slice(&(VERSAO_POSICIONAL_CIFRADA + 1).to_le_bytes());
        std::fs::write(&caminho, &b).unwrap();
        match ler_marca(&caminho).unwrap() {
            Leitura::SemChave(motivo) => assert!(motivo.contains("mais nova"), "{motivo}"),
            outra => panic!("a marca de versao mais nova virou {outra:?}"),
        }
        let r = recuperar_no_diretorio(&d, crate::catalogo::PoliticaDoDiario::default());
        assert!(
            caminho.exists(),
            "a recuperacao apagou a marca de versao mais nova"
        );
        assert_eq!((r.descartadas, r.paradas.len()), (0, 1));
    }

    /// As marcas se completam na ordem NUMERICA do id, e nao na do texto --
    /// pedido 714 (o F12): `transacao_10` vem depois de `transacao_9`. E o
    /// maior id olha as duas familias.
    ///
    /// Prova real: voltar o `sort()` do texto poe a 10 antes da 9.
    #[test]
    fn as_marcas_seguem_o_numero_do_id_e_nao_o_texto() {
        let d = dir("ordem");
        for id in [10u64, 9, 100] {
            std::fs::write(caminho_da_marca(&d, id), b"x").unwrap();
        }
        std::fs::write(d.join("bidi_250.tx"), b"x").unwrap();
        let ids: Vec<u64> = marcas_em(&d)
            .iter()
            .filter_map(|c| id_da_marca(c))
            .collect();
        assert_eq!(ids, vec![9, 10, 100]);
        assert_eq!(maior_id_de_marca_em(&d), 250);
        assert!(proximo_id_no_diretorio(&d) > 250);
    }

    fn op(tabela: &str, acao: Acao, rowid: u64) -> Escrita {
        let mut e = uma_escrita("x").remove(0);
        e.tabela = tabela.into();
        e.acao = acao;
        e.rowid = rowid;
        e
    }

    /// As versoes de antes saem do disco UMA vez por linha e se deduzem dai
    /// em diante, como a passada as fara; e onde nao se sabe, conta-se a
    /// escrita (o erro que so reaplica, e nunca o que pula).
    #[test]
    fn as_versoes_de_antes_seguem_a_passada() {
        let ops = vec![
            op("a", Acao::Atualizar, 1),
            op("a", Acao::Atualizar, 1),
            op("a", Acao::ExcluirSuave, 1),
            op("a", Acao::ExcluirSuave, 1),
            op("a", Acao::Restaurar, 1),
            op("b", Acao::Inserir, 9),
            op("b", Acao::Atualizar, 9),
            op("a", Acao::ExcluirSuave, 2),
            op("a", Acao::Restaurar, 2),
        ];
        let mut lidas = Vec::new();
        let v = versoes_antes(&ops, |t, r, com_suave| {
            lidas.push((t.to_string(), r, com_suave));
            Ok(match r {
                1 => EstadoDoSlot {
                    versao: Some(5),
                    suave: None,
                },
                _ => EstadoDoSlot {
                    versao: Some(2),
                    suave: Some(true),
                },
            })
        })
        .unwrap();
        // 1: 5 -> 6 -> 7; a alteracao nao diz o suave, entao a primeira
        // exclusao conta (7 -> 8) e a segunda ja sabe que nao grava (8); a
        // restauracao grava (8 -> 9). A inclusao nasce na 1. A linha 2 ja
        // estava excluida: a exclusao nao grava, a restauracao grava.
        assert_eq!(v, vec![5, 6, 7, 8, 8, 0, 1, 2, 2]);
        assert_eq!(
            lidas,
            vec![("a".to_string(), 1, false), ("a".to_string(), 2, true)],
            "o disco se le uma vez por linha, e a inclusao nao le"
        );
    }

    // ------------------------------------------- o grupo da replica (682)

    use crate::catalogo::PoliticaDoDiario;
    use crate::log::Operacao;
    use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
    use phxsql_core::types::ColumnType;

    fn base_com_imagem(d: &Path) -> Database {
        std::fs::create_dir_all(d).unwrap();
        Database::no_diretorio_com_politica(d, PoliticaDoDiario::com_imagem(true))
    }

    fn itens(db: &Database) -> Table {
        let e = Schema::new(
            "itens",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("venda", ColumnType::Int8),
            ],
            vec![IndexDef::new("pk", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap();
        db.criar_tabela(None, e).unwrap()
    }

    /// `n` eventos de inclusao, com imagem, tirados do diario de uma origem.
    fn eventos_da_origem(d: &Path, n: i64) -> Vec<(crate::log::Evento, Vec<u8>)> {
        let db = base_com_imagem(d);
        let mut t = itens(&db);
        for i in 1..=n {
            t.inserir(&[Value::Int(i), Value::Int(1)]).unwrap();
        }
        let mut t = db.abrir_qualificada("itens").unwrap();
        t.diario_com_imagem(0, 0).unwrap()
    }

    fn grupo<'a>(ev: &'a [(crate::log::Evento, Vec<u8>)]) -> Vec<EventoDoGrupo<'a>> {
        ev.iter()
            .enumerate()
            .map(|(k, (e, img))| EventoDoGrupo {
                tabela: "itens",
                operacao: e.operacao,
                rowid: e.rowid,
                carimbo_ms: e.carimbo,
                origem: 7,
                posicao: k as u64,
                imagem: img,
            })
            .collect()
    }

    /// A ida e volta da v5: a versao propria (o leitor anterior descarta em
    /// vez de ler um `inserir` de linha vazia) e cada campo do evento.
    #[test]
    fn a_marca_do_grupo_da_replica_volta_com_o_evento_inteiro() {
        let d = dir("replica-ida-e-volta");
        let ev = eventos_da_origem(&d.join("origem"), 3);
        let g = grupo(&ev);
        let caminho = gravar_marca_da_replica(&d, 41, 5, &g).unwrap();
        let b = std::fs::read(&caminho).unwrap();
        assert_eq!(
            u32::from_le_bytes([b[8], b[9], b[10], b[11]]),
            VERSAO_REPLICA_EM_CLARO
        );
        let m = ler_marca(&caminho).unwrap().marca().expect("tem de abrir");
        assert_eq!(m.operacoes.len(), 3);
        for (k, op) in m.operacoes.iter().enumerate() {
            let r = op.replica.as_ref().expect("evento da replica");
            assert_eq!(op.acao, Acao::Inserir);
            assert_eq!(op.rowid, ev[k].0.rowid);
            assert_eq!(r.operacao, Operacao::Inclusao);
            assert_eq!(r.carimbo_ms, ev[k].0.carimbo);
            assert_eq!(r.origem, 7);
            assert_eq!(r.posicao, k as u64);
            assert_eq!(r.imagem, ev[k].1);
        }
    }

    /// A recuperacao COMPLETA o grupo que a queda partiu, pelo aplicador da
    /// replica: o diario daqui sai com o carimbo de LA em cada evento, e o
    /// que ja tinha entrado nao entra de novo.
    #[test]
    fn a_recuperacao_completa_o_grupo_da_replica_pela_posicao() {
        let d = dir("replica-completa");
        let ev = eventos_da_origem(&d.join("origem"), 4);
        let db = base_com_imagem(&d.join("central"));
        let mut t = itens(&db);
        // A queda: o primeiro evento entrou, os outros tres nao.
        t.forcar_proximo_evento(ev[0].0.carimbo, 7);
        t.aplicar_evento(ev[0].0.operacao, ev[0].0.rowid, &ev[0].1)
            .unwrap();
        t.sincronizar().unwrap();
        drop(t);
        gravar_marca_da_replica(db.caminho(), 42, 0, &grupo(&ev)).unwrap();
        let r = db.recuperar_marcas();
        assert_eq!((r.reaplicadas, r.ja_aplicadas), (3, 1), "{r:?}");
        assert!(r.impossiveis.is_empty(), "{r:?}");
        assert!(marcas_em(db.caminho()).is_empty(), "a marca completada sai");
        let mut t = db.abrir_qualificada("itens").unwrap();
        let meu = t.diario(0, 0).unwrap();
        assert_eq!(meu.len(), 4);
        for (m, (la, _)) in meu.iter().zip(&ev) {
            assert_eq!(
                (m.carimbo, m.operacao, m.rowid),
                (la.carimbo, la.operacao, la.rowid)
            );
            assert_eq!(m.origem, 7);
        }
    }

    /// O outro sentido da idempotencia: o diario que ja tem um evento
    /// DIFERENTE naquela posicao e de outra historia da tabela, e a marca nao
    /// grava por cima -- conta como impossivel.
    #[test]
    fn a_marca_do_grupo_nao_grava_por_cima_de_outra_historia() {
        let d = dir("replica-outra-historia");
        let ev = eventos_da_origem(&d.join("origem"), 2);
        let db = base_com_imagem(&d.join("central"));
        let mut t = itens(&db);
        t.inserir(&[Value::Int(99), Value::Int(9)]).unwrap();
        t.sincronizar().unwrap();
        drop(t);
        gravar_marca_da_replica(db.caminho(), 43, 0, &grupo(&ev)).unwrap();
        let r = db.recuperar_marcas();
        assert_eq!(r.reaplicadas, 0, "{r:?}");
        assert_eq!(r.impossiveis.len(), 2, "{r:?}");
        let mut t = db.abrir_qualificada("itens").unwrap();
        assert_eq!(t.eventos().unwrap(), 1, "nada entrou por cima");
    }

    /// `itens` SEM indice nenhum: a reaplicacao que grava num slot novo nao
    /// esbarra em unicidade, e a linha sai duplicada -- o caso do pedido 699.
    fn itens_sem_indice(db: &Database) -> Table {
        let e = Schema::new(
            "itens",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("venda", ColumnType::Int8),
            ],
            vec![],
        )
        .unwrap();
        db.criar_tabela(None, e).unwrap()
    }

    /// **Pedido 699.** A queda entre o `.reg` e o evento do diario: o slot da
    /// segunda inclusao ja esta gravado e o diario so tem a primeira. A
    /// recuperacao completa SO o evento -- a linha nao entra de novo num slot
    /// novo. Aqui a queda e um panico no ponto exato (`InserirDepoisDoContador`);
    /// a prova contra o sistema operacional, com `SIGKILL`, e a do servidor
    /// (`venda-inteira-na-queda-da-replica.rs`).
    ///
    /// Vermelho medido sem o `slot_ja_consumido` no `aplicar_evento_da_marca`:
    /// a recuperacao grava a linha 2 de novo no slot 3 e o `aplicar_evento`
    /// recusa a divergencia DEPOIS de gravar -- 5 slots para 4 linhas.
    #[test]
    fn a_queda_entre_o_reg_e_o_diario_nao_duplica_a_linha() {
        let d = dir("replica-reg-sem-evento");
        let ev = eventos_da_origem(&d.join("origem"), 4);
        let db = base_com_imagem(&d.join("central"));
        let mut t = itens_sem_indice(&db);
        t.forcar_proximo_evento(ev[0].0.carimbo, 7);
        t.aplicar_evento(ev[0].0.operacao, ev[0].0.rowid, &ev[0].1)
            .unwrap();
        crate::ndx::panico_de_teste::armar(
            crate::ndx::panico_de_teste::Ponto::InserirDepoisDoContador,
        );
        t.forcar_proximo_evento(ev[1].0.carimbo, 7);
        let caiu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = t.aplicar_evento(ev[1].0.operacao, ev[1].0.rowid, &ev[1].1);
        }));
        crate::ndx::panico_de_teste::desarmar();
        assert!(caiu.is_err(), "o panico de teste nao disparou");
        drop(t);
        let mut t = db.abrir_qualificada("itens").unwrap();
        assert_eq!(
            (t.slots(), t.eventos().unwrap()),
            (2, 1),
            "o arranjo nao reproduziu a queda: o slot 2 tem de estar no .reg e o \
             evento dele fora do diario"
        );
        drop(t);
        gravar_marca_da_replica(db.caminho(), 44, 0, &grupo(&ev)).unwrap();
        let r = db.recuperar_marcas();
        assert!(r.impossiveis.is_empty(), "{r:?}");
        assert!(marcas_em(db.caminho()).is_empty(), "a marca completada sai");
        let mut t = db.abrir_qualificada("itens").unwrap();
        assert_eq!(
            (t.slots(), t.registros()),
            (4, 4),
            "a recuperacao gravou a linha que ja estava no .reg num slot NOVO"
        );
        let meu = t.diario(0, 0).unwrap();
        assert_eq!(meu.len(), 4, "o evento que faltava nao foi completado");
        for (m, (la, _)) in meu.iter().zip(&ev) {
            assert_eq!(
                (m.carimbo, m.operacao, m.rowid),
                (la.carimbo, la.operacao, la.rowid)
            );
        }
    }

    /// **Pedido 699, o inverso.** O diario tem a inclusao e o `.reg` nao tem o
    /// slot -- a queda de energia que levou o `.reg` e deixou o diario. A
    /// recuperacao recusa nomeando, e a marca FICA: ela e a unica copia,
    /// aqui, da imagem da linha perdida.
    ///
    /// Vermelho medido sem o `conferir_o_reg_do_evento`: o evento confere com
    /// o diario, a resposta e «ja estava», e a marca sai com a linha ausente.
    #[test]
    fn evento_no_diario_sem_a_linha_no_reg_segura_a_marca() {
        let d = dir("replica-evento-sem-reg");
        let ev = eventos_da_origem(&d.join("origem"), 2);
        let central = d.join("central");
        let guardado = d.join("reg-com-um");
        let db = base_com_imagem(&central);
        let mut t = itens_sem_indice(&db);
        t.forcar_proximo_evento(ev[0].0.carimbo, 7);
        t.aplicar_evento(ev[0].0.operacao, ev[0].0.rowid, &ev[0].1)
            .unwrap();
        t.sincronizar().unwrap();
        drop(t);
        // O `.reg` como estava com UMA linha...
        std::fs::create_dir_all(&guardado).unwrap();
        let regs: Vec<PathBuf> = std::fs::read_dir(&central)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "reg"))
            .collect();
        assert!(!regs.is_empty(), "nenhum .reg em {}", central.display());
        for r in &regs {
            std::fs::copy(r, guardado.join(r.file_name().unwrap())).unwrap();
        }
        let mut t = db.abrir_qualificada("itens").unwrap();
        t.forcar_proximo_evento(ev[1].0.carimbo, 7);
        t.aplicar_evento(ev[1].0.operacao, ev[1].0.rowid, &ev[1].1)
            .unwrap();
        t.sincronizar().unwrap();
        drop(t);
        // ... e o diario com DUAS: o disco perdeu o slot e ficou com o evento.
        for r in &regs {
            std::fs::copy(guardado.join(r.file_name().unwrap()), r).unwrap();
        }
        let caminho = gravar_marca_da_replica(db.caminho(), 45, 0, &grupo(&ev)).unwrap();
        let r = db.recuperar_marcas();
        assert_eq!(r.ja_aplicadas, 1, "{r:?}");
        assert_eq!(r.impossiveis.len(), 1, "{r:?}");
        assert!(r.impossiveis[0].contains("o .reg nao tem a linha"), "{r:?}");
        assert!(
            caminho.exists(),
            "a marca saiu com a linha ausente do .reg -- era a unica copia dela"
        );
    }

    /// Os eventos `ev` como os de uma origem que mexeu em `itens` do jeito que
    /// `mexer` mandar -- inclusao, alteracao, exclusao --, com imagem.
    fn eventos_de(d: &Path, mexer: impl FnOnce(&mut Table)) -> Vec<(crate::log::Evento, Vec<u8>)> {
        let db = base_com_imagem(d);
        let mut t = itens_sem_indice(&db);
        mexer(&mut t);
        drop(t);
        let mut t = db.abrir_qualificada("itens").unwrap();
        t.diario_com_imagem(0, 0).unwrap()
    }

    fn aplicar(t: &mut Table, e: &(crate::log::Evento, Vec<u8>)) {
        t.forcar_proximo_evento(e.0.carimbo, 7);
        t.aplicar_evento(e.0.operacao, e.0.rowid, &e.1).unwrap();
    }

    fn copiar_os_reg(de: &Path, para: &Path) -> Vec<PathBuf> {
        std::fs::create_dir_all(para).unwrap();
        let regs: Vec<PathBuf> = std::fs::read_dir(de)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "reg"))
            .collect();
        assert!(!regs.is_empty(), "nenhum .reg em {}", de.display());
        for r in &regs {
            std::fs::copy(r, para.join(r.file_name().unwrap())).unwrap();
        }
        regs
    }

    /// **Pedido 701 (a).** O diario tem a alteracao e o `.reg` ficou com a
    /// versao VELHA da linha -- a queda de energia que levou o slot novo e
    /// deixou o evento. A linha esta la, e por isso a presenca nao basta: a
    /// recuperacao compara o conteudo, recusa nomeando, e a marca FICA, porque
    /// e a unica copia da versao nova.
    ///
    /// Vermelho medido sem o `o_reg_tem_a_imagem`: as duas operacoes contam
    /// «ja estava» e a marca sai com a venda 1 no slot que devia dizer 2.
    #[test]
    fn alteracao_no_diario_com_a_versao_velha_no_reg_segura_a_marca() {
        let d = dir("replica-alteracao-velha");
        let ev = eventos_de(&d.join("origem"), |t| {
            t.inserir(&[Value::Int(1), Value::Int(1)]).unwrap();
            t.atualizar(1, &[Value::Int(1), Value::Int(2)]).unwrap();
        });
        assert_eq!(ev.len(), 2);
        let central = d.join("central");
        let db = base_com_imagem(&central);
        let mut t = itens_sem_indice(&db);
        aplicar(&mut t, &ev[0]);
        t.sincronizar().unwrap();
        drop(t);
        let regs = copiar_os_reg(&central, &d.join("reg-velho"));
        let mut t = db.abrir_qualificada("itens").unwrap();
        aplicar(&mut t, &ev[1]);
        t.sincronizar().unwrap();
        drop(t);
        for r in &regs {
            std::fs::copy(d.join("reg-velho").join(r.file_name().unwrap()), r).unwrap();
        }
        let caminho = gravar_marca_da_replica(db.caminho(), 46, 0, &grupo(&ev)).unwrap();
        let r = db.recuperar_marcas();
        assert_eq!(r.ja_aplicadas, 1, "{r:?}");
        assert_eq!(r.impossiveis.len(), 1, "{r:?}");
        assert!(r.impossiveis[0].contains("outro conteudo"), "{r:?}");
        assert!(
            caminho.exists(),
            "a marca saiu com a versao velha no .reg -- era a unica copia da nova"
        );
    }

    /// O controle do de cima: a alteracao que chegou ao `.reg` conta «ja
    /// estava», e a alteracao seguinte do mesmo diario legitima o conteudo
    /// diferente da anterior -- a comparacao nao recusa o que esta certo.
    #[test]
    fn duas_alteracoes_no_disco_contam_ja_aplicadas() {
        let d = dir("replica-duas-alteracoes");
        let ev = eventos_de(&d.join("origem"), |t| {
            t.inserir(&[Value::Int(1), Value::Int(1)]).unwrap();
            t.atualizar(1, &[Value::Int(1), Value::Int(2)]).unwrap();
            t.atualizar(1, &[Value::Int(1), Value::Int(3)]).unwrap();
        });
        let db = base_com_imagem(&d.join("central"));
        let mut t = itens_sem_indice(&db);
        for e in &ev {
            aplicar(&mut t, e);
        }
        t.sincronizar().unwrap();
        drop(t);
        gravar_marca_da_replica(db.caminho(), 47, 0, &grupo(&ev)).unwrap();
        let r = db.recuperar_marcas();
        assert_eq!(r.ja_aplicadas, 3, "{r:?}");
        assert!(r.impossiveis.is_empty(), "{r:?}");
        assert!(marcas_em(db.caminho()).is_empty(), "a marca completada sai");
    }

    /// **Pedido 701 (b).** O arranque completa a marca numa unidade de
    /// transacao SO, e com o id que a primeira metade do grupo ja levou: a
    /// replica encadeada junta pelo id, e o resto com ids novos chegaria la
    /// como outras transacoes.
    ///
    /// Vermelho medido sem a `UnidadeDaMarca`: cada evento completado ganha
    /// um id proprio, e nenhum e o da primeira metade.
    #[test]
    fn o_arranque_completa_o_grupo_com_o_id_da_primeira_metade() {
        let d = dir("replica-um-id-so");
        let ev = eventos_da_origem(&d.join("origem"), 4);
        let db = base_com_imagem(&d.join("central"));
        let mut t = itens(&db);
        // A primeira metade, numa unidade -- como a tomada da trava da rodada.
        crate::log::abrir_unidade();
        aplicar(&mut t, &ev[0]);
        crate::log::fechar_unidade();
        t.sincronizar().unwrap();
        drop(t);
        gravar_marca_da_replica(db.caminho(), 48, 0, &grupo(&ev)).unwrap();
        let r = db.recuperar_marcas();
        assert_eq!((r.reaplicadas, r.ja_aplicadas), (3, 1), "{r:?}");
        let mut t = db.abrir_qualificada("itens").unwrap();
        let meu = t.diario(0, 0).unwrap();
        assert_eq!(meu.len(), 4);
        assert_ne!(meu[0].tx, 0, "o volume tem de guardar o id");
        let ids: Vec<u64> = meu.iter().map(|e| e.tx).collect();
        assert!(
            ids.iter().all(|x| *x == ids[0]),
            "o grupo completado no arranque saiu em pedacos: ids {ids:?}"
        );
    }

    fn op_da_marca(tabela: &str, acao: Acao, rowid: u64) -> OperacaoDaMarca {
        OperacaoDaMarca {
            tabela: tabela.into(),
            acao,
            rowid,
            linha: vec![Value::Int(rowid as i64), Value::Int(1)],
            linha_antiga: Vec::new(),
            motivo: String::new(),
            cascata_na_lista: false,
            replica: None,
            versao_antes: None,
        }
    }

    /// **Pedido 702, as duas guardas do lado de NAO adotar.** O id da metade
    /// que entrou so vale para a passada que a queda interrompeu: a marca
    /// INTEIRA no diario (so esperava a janela) e a que tem um commit DEPOIS
    /// dela devolvem `None` -- adotar ali penduraria regravacao redundante num
    /// grupo que a replica ja fechou. E o controle: com um evento faltando,
    /// adota o id da metade.
    ///
    /// Vermelho medido tirando cada guarda: `Some` na marca inteira, e
    /// `Some` com a cauda maior em `outra`.
    #[test]
    fn a_metade_que_entrou_so_ancora_a_passada_interrompida() {
        let d = dir("702-guardas");
        let db = base_com_imagem(&d);
        let mut t = itens(&db);
        crate::log::abrir_unidade();
        t.inserir(&[Value::Int(1), Value::Int(1)]).unwrap();
        t.inserir(&[Value::Int(2), Value::Int(1)]).unwrap();
        crate::log::fechar_unidade();
        let x = t.diario(0, 0).unwrap()[0].tx;
        assert_ne!(x, 0, "premissa: o volume guarda o id");
        t.sincronizar().unwrap();
        drop(t);
        let marca = |ops: Vec<OperacaoDaMarca>| Marca {
            id: 1,
            carimbo_ms: 0,
            tx: 0,
            operacoes: ops,
        };
        let inteira = marca(vec![
            op_da_marca("itens", Acao::Inserir, 1),
            op_da_marca("itens", Acao::Inserir, 2),
        ]);
        assert_eq!(
            id_da_metade_que_entrou(&db, &inteira),
            None,
            "a marca inteira no diario adotou o id do grupo ja fechado"
        );
        let pela_metade = marca(vec![
            op_da_marca("itens", Acao::Inserir, 1),
            op_da_marca("itens", Acao::Inserir, 2),
            op_da_marca("itens", Acao::Inserir, 3),
        ]);
        assert_eq!(id_da_metade_que_entrou(&db, &pela_metade), Some(x));

        // Um commit DEPOIS, numa tabela que a marca nomeia sem ancorar nela.
        let e = Schema::new(
            "outra",
            vec![
                Column::new("id", ColumnType::Int8).obrigatoria(),
                Column::new("venda", ColumnType::Int8),
            ],
            vec![IndexDef::new("pk", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap();
        let mut o = db.criar_tabela(None, e).unwrap();
        crate::log::abrir_unidade();
        o.inserir(&[Value::Int(1), Value::Int(1)]).unwrap();
        crate::log::fechar_unidade();
        o.sincronizar().unwrap();
        drop(o);
        let com_commit_depois = marca(vec![
            op_da_marca("itens", Acao::Inserir, 1),
            op_da_marca("itens", Acao::Inserir, 2),
            op_da_marca("outra", Acao::ExcluirSuave, 1),
        ]);
        assert_eq!(
            id_da_metade_que_entrou(&db, &com_commit_depois),
            None,
            "houve commit depois da passada e o resto adotou o id antigo"
        );
    }

    /// **Pedido 701 (c).** A queda entre o slot liberado e o evento da
    /// exclusao: a linha saiu do `.reg` e o diario nao tem a exclusao. A
    /// recuperacao completa SO o evento, com a imagem do antes tirada da
    /// lixeira, e a marca sai.
    ///
    /// Vermelho medido sem o ramo da exclusao no `aplicar_evento_da_marca`:
    /// o `aplicar_evento` recusa («aqui ele nao existe») e o diario fica sem
    /// a exclusao.
    #[test]
    fn a_queda_entre_o_slot_e_o_diario_completa_a_exclusao() {
        let d = dir("replica-exclusao-sem-evento");
        let ev = eventos_de(&d.join("origem"), |t| {
            t.inserir(&[Value::Int(1), Value::Int(1)]).unwrap();
            t.inserir(&[Value::Int(2), Value::Int(1)]).unwrap();
            t.excluir_de_vez(1, "teste").unwrap();
        });
        assert_eq!(ev[2].0.operacao, Operacao::Exclusao);
        assert!(!ev[2].1.is_empty(), "a exclusao tem de levar a imagem");
        let db = base_com_imagem(&d.join("central"));
        let mut t = itens_sem_indice(&db);
        aplicar(&mut t, &ev[0]);
        aplicar(&mut t, &ev[1]);
        crate::ndx::panico_de_teste::armar(crate::ndx::panico_de_teste::Ponto::ExcluirDepoisDoSlot);
        t.forcar_proximo_evento(ev[2].0.carimbo, 7);
        let caiu = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = t.aplicar_evento(ev[2].0.operacao, ev[2].0.rowid, &ev[2].1);
        }));
        crate::ndx::panico_de_teste::desarmar();
        assert!(caiu.is_err(), "o panico de teste nao disparou");
        drop(t);
        let mut t = db.abrir_qualificada("itens").unwrap();
        assert_eq!(
            (t.registros(), t.eventos().unwrap()),
            (1, 2),
            "o arranjo nao reproduziu a queda: o slot 1 tem de estar livre e a \
             exclusao fora do diario"
        );
        drop(t);
        let caminho = gravar_marca_da_replica(db.caminho(), 49, 0, &grupo(&ev)).unwrap();
        let r = db.recuperar_marcas();
        assert!(r.impossiveis.is_empty(), "{r:?}");
        assert!(!caminho.exists(), "a marca completada sai");
        let mut t = db.abrir_qualificada("itens").unwrap();
        let meu = t.diario_com_imagem(0, 0).unwrap();
        assert_eq!(meu.len(), 3, "a exclusao que faltava nao foi completada");
        assert_eq!(
            (meu[2].0.operacao, meu[2].0.rowid, meu[2].0.carimbo),
            (Operacao::Exclusao, 1, ev[2].0.carimbo)
        );
        assert_eq!(
            meu[2].1, ev[2].1,
            "a imagem do antes nao e a da linha excluida"
        );
    }
}

//! Transacoes: `BEGIN` / `COMMIT` / `ROLLBACK` / `SAVEPOINT`.
//!
//! O desenho inteiro esta em `docs/TRANSACOES.md`, escrito ANTES deste
//! arquivo e de proposito. Aqui fica o estado e o formato; quem amarra isto ao
//! protocolo e o `servidor.rs`.
//!
//! # A frase que decide tudo
//!
//! **Nada vai a disco antes do `COMMIT`.** Dentro de uma transacao, `inserir`,
//! `atualizar` e `excluir` nao tocam em arquivo nenhum: entram no conjunto de
//! escrita, que e uma lista em RAM. O `ROLLBACK` joga a lista fora -- zero
//! bytes de trabalho -- e o `COMMIT` a aplica numa passada so, com a trava de
//! dados na mao.
//!
//! Isso nao e economia: e a unica forma que o formato permite. O `.reg` nunca
//! reaproveita slot excluido (`store/src/reg.rs`), entao um `INSERT` gravado e
//! depois revertido deixaria um buraco permanente -- e, pior, teria de deixar
//! o MESMO buraco na replica, o que faria a transacao revertida chegar
//! aplicada do outro lado. Os quatro motivos estao na §3.2 do documento.
//!
//! # O que este arquivo guarda
//!
//! * a maquina de estados, com o `ABORT_ONLY` que recusa confirmar trabalho
//!   meio invalido;
//! * o conjunto de escrita e os `SAVEPOINT`, que aqui sao um INDICE na lista
//!   -- nao ha copia de transacao para tirar, entao voltar a um ponto e
//!   truncar um `Vec`;
//! * a marca `transacao_<id>.tx` e a recuperacao, que e a resposta a pergunta
//!   «se a energia cair exatamente aqui, o banco sabe dizer o que aconteceu?».

#[cfg(test)]
use crate::apoio_teste::DirTemp;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use phxsql_core::cifra::XNONCE_LEN;
use phxsql_core::crc::crc32;
use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_core::uuid::{Uuid, Uuid256};
use phxsql_core::value::Value;
use phxsql_store::catalogo::Instancia;
use phxsql_store::cofre::{self, Material};

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

// --------------------------------------------------------------- os estados

/// O estado de uma transacao.
///
/// # Por que os nomes saem em ingles no protocolo
///
/// Porque o campo se chama `transaction_state` e e o mesmo conjunto do
/// `XACT_STATE()` do SQL Server casado com o estado abortado do PostgreSQL(R)
/// -- e e o que uma ferramenta de fora espera ler. O identificador em Rust
/// continua em portugues, como manda a casa, e o ROTULO que a tela mostra sai
/// da fabrica de idiomas, traduzido nos seis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// Nao ha transacao nesta conexao. E o servidor inteiro hoje.
    Ociosa,
    /// Aberta e aceitando trabalho.
    Ativa,
    /// Houve erro de TRANSACAO. So o `ROLLBACK` passa daqui.
    AbortOnly,
    /// A passada de commit esta rodando.
    Confirmando,
    /// A passada terminou e o disco tem tudo.
    Confirmada,
    /// O descarte esta rodando.
    Revertendo,
    /// A lista foi jogada fora.
    Revertida,
}

impl Estado {
    pub fn nome(self) -> &'static str {
        match self {
            Estado::Ociosa => "IDLE",
            Estado::Ativa => "ACTIVE",
            Estado::AbortOnly => "ABORT_ONLY",
            Estado::Confirmando => "COMMITTING",
            Estado::Confirmada => "COMMITTED",
            Estado::Revertendo => "ROLLING_BACK",
            Estado::Revertida => "ROLLED_BACK",
        }
    }

    /// Este estado aceita mais trabalho?
    pub fn aceita_trabalho(self) -> bool {
        self == Estado::Ativa
    }
}

// ---------------------------------------------------------- as classes de erro

/// De que CLASSE e o erro que acabou de acontecer dentro de uma transacao.
///
/// # Por que a aplicacao precisa saber
///
/// Porque a acao dela e outra em cada caso, e adivinhar pelo texto e o que
/// quebra no dia em que alguem melhorar a redacao:
///
/// * **instrucao** -- chave duplicada, tipo errado, linha que nao existe. A
///   instrucao e cancelada, a transacao continua `ACTIVE`, e quem chamou pode
///   corrigir e mandar de novo. E o caso comum.
/// * **transacao** -- falha que poe em duvida o proprio conjunto de escrita
///   (o teto de linhas estourado, E/S no meio da passada). A transacao vai
///   para `ABORT_ONLY` e so o `ROLLBACK` passa.
///
/// A queda da conexao e a terceira, e nao precisa de nome porque nao ha
/// ninguem para avisar: a transacao e desfeita sozinha na saida da conexao.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClasseDoErro {
    Instrucao,
    Transacao,
}

impl ClasseDoErro {
    pub fn nome(self) -> &'static str {
        match self {
            ClasseDoErro::Instrucao => "instrucao",
            ClasseDoErro::Transacao => "transacao",
        }
    }

    /// A classe de um erro qualquer do motor, dentro de uma transacao.
    ///
    /// A regra e curta: **erro do DADO ou do PEDIDO cancela a instrucao; erro
    /// do SISTEMA ou do FORMATO derruba a transacao.** Ela sai da faixa do
    /// codigo, e nao de uma lista escrita a mao, pelo mesmo motivo de
    /// `PhxError::classe`: erro novo cai na classe certa sozinho, e as duas
    /// nao tem como divergir.
    pub fn do_erro(e: &PhxError) -> ClasseDoErro {
        match e.codigo() / 1000 {
            // esquema (2xxx) e dado (3xxx): o pedido esta errado, e so ele.
            2 | 3 => ClasseDoErro::Instrucao,
            // acesso (4xxx): quem pediu nao podia. A transacao continua sa.
            4 => ClasseDoErro::Instrucao,
            // formato (1xxx), sistema (5xxx) e execucao (6xxx): o chao cedeu.
            _ => ClasseDoErro::Transacao,
        }
    }
}

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
}

// ------------------------------------------------------------- a transacao

/// Um ponto de retorno dentro da transacao.
///
/// # Por que ele e quase de graca aqui
///
/// Porque nao ha o que copiar. Num motor que ja gravou as escritas, voltar a
/// um `SAVEPOINT` exige desfazer paginas; aqui o conjunto de escrita e um
/// `Vec` em RAM, e o ponto e o INDICE dele naquele instante. `ROLLBACK TO
/// SAVEPOINT` e um `truncate`, e a transacao continua aberta.
#[derive(Debug, Clone)]
pub struct Ponto {
    pub nome: String,
    /// Quantas escritas havia quando o ponto foi criado.
    pub ate: usize,
}

/// Uma transacao aberta, presa a uma CONEXAO.
#[derive(Debug)]
pub struct Transacao {
    pub id: u64,
    /// A conexao dona. Sem um id de CONEXAO, duas janelas do mesmo usuario
    /// seriam a mesma transacao -- o contrario de exclusivo.
    pub ligacao: u64,
    pub usuario: String,
    pub ip: String,
    /// O database desta transacao. Vazio ate a primeira escrita.
    ///
    /// Uma transacao abrange UM database, porque a marca `.tx` mora dentro do
    /// diretorio dele e e isso que a faz viajar junto no backup e na
    /// restauracao. Ver a §2.3 do documento.
    pub database: String,
    pub estado: Estado,
    pub desde_ms: i64,
    /// Quando o prazo da TRANSACAO INTEIRA estoura (`TIMEOUT`).
    pub expira_ms: i64,
    /// Quanto se aceita esperar por uma trava de outro (`LOCK TIMEOUT`).
    pub lock_timeout_ms: i64,
    /// Quanto UMA operacao pode levar (`STATEMENT TIMEOUT`).
    ///
    /// Os tres prazos sao problemas diferentes e por isso sao tres campos: uma
    /// transacao pode ser curta e mesmo assim esperar demais por uma trava, e
    /// uma operacao pode demorar sem que a transacao tenha estourado.
    pub statement_timeout_ms: i64,
    /// Como esta transacao trava o que toca.
    pub modo: crate::travas::Modo,
    /// O que fazer com tabela fora do escopo declarado.
    pub escopo_modo: crate::travas::EscopoModo,
    /// Leitura repetivel PELA TRAVA (`docs/SOMBRA.md` §5b), pedida na
    /// abertura. Quando ligada, cada tabela que a transacao LE recebe a trava
    /// compartilhada (S) ate o fim -- o escritor espera o leitor, e a releitura
    /// devolve o mesmo estado. Quem nao pede continua exatamente como antes.
    pub leitura_repetivel: bool,
    /// As tabelas que a abertura DECLAROU, em ordem canonica.
    pub declaradas: Vec<String>,
    /// As declaradas mais as que as dependencias do catalogo alcancam.
    ///
    /// Sao mostradas separadas na ficha de proposito: quem declarou quatro
    /// tabelas e ficou com seis precisa ver as duas que entraram sem ele
    /// pedir, e por onde entraram.
    pub efetivas: Vec<String>,
    /// As que entraram por expansao DINAMICA, depois da abertura.
    pub expandidas: Vec<String>,
    /// O que esta transacao esta esperando AGORA, para a ficha de diagnostico.
    /// Vazio quando ela nao espera nada.
    pub esperando: String,
    pub escritas: Vec<Escrita>,
    pub pontos: Vec<Ponto>,
    /// As tabelas reservadas por esta transacao, como `database/tabela` em
    /// caixa baixa -- a mesma chave da reserva de carga.
    pub tabelas: Vec<String>,
    /// Por que a transacao foi para `ABORT_ONLY`. Vazio enquanto ela vive.
    pub motivo_do_aborto: String,
    /// Chaves unicas ja empilhadas, por `tabela|indice`.
    ///
    /// Existe porque a conferencia de unicidade acontece ao EMPILHAR, e nao no
    /// `COMMIT`: o indice em disco ainda nao sabe das linhas que estao na
    /// lista, e duas linhas com a mesma chave dentro da mesma transacao
    /// passariam pela conferencia contra o disco e quebrariam a passada no
    /// meio -- que e justamente o que este desenho nao pode ter.
    chaves: HashMap<String, HashSet<String>>,
}

impl Transacao {
    /// Quantas escritas de INSERCAO ja foram empilhadas nesta tabela.
    ///
    /// E o que soma ao `slots()` para dar o proximo rowid: a segunda insercao
    /// da transacao cai no slot seguinte ao da primeira, que ainda nao foi
    /// gravada.
    pub fn insercoes_em(&self, tabela: &str) -> u64 {
        self.escritas
            .iter()
            .filter(|e| e.acao == Acao::Inserir && e.tabela.eq_ignore_ascii_case(tabela))
            .count() as u64
    }

    /// Este rowid foi criado por uma insercao ainda empilhada?
    ///
    /// Sem isto, `BEGIN; INSERT; UPDATE do que acabou de entrar` seria
    /// recusado por «rowid nao existe» -- e ele existe, so que na lista.
    pub fn nasceu_aqui(&self, tabela: &str, rowid: u64) -> bool {
        self.escritas.iter().any(|e| {
            e.acao == Acao::Inserir && e.rowid == rowid && e.tabela.eq_ignore_ascii_case(tabela)
        })
    }

    /// Esta chave unica ja foi empilhada nesta transacao?
    ///
    /// PERGUNTAR e GUARDAR sao separados de proposito, e a separacao custou um
    /// teste: guardar a chave junto com a conferencia registrava a linha antes
    /// de saber se ela seria aceita -- e uma escrita recusada pela TRAVA
    /// deixava a chave dela na lista. A tentativa seguinte, com a mesma linha,
    /// era acusada de duplicada por si mesma. Agora a chave so entra depois de
    /// a escrita estar empilhada de verdade.
    pub fn chave_ja_empilhada(&self, tabela: &str, indice: &str, chave: &str) -> bool {
        self.chaves
            .get(&format!("{}|{}", tabela.to_lowercase(), indice))
            .is_some_and(|c| c.contains(chave))
    }

    /// Guarda a chave unica de uma linha JA empilhada.
    pub fn guardar_chave(&mut self, tabela: &str, indice: &str, chave: &str) {
        self.chaves
            .entry(format!("{}|{}", tabela.to_lowercase(), indice))
            .or_default()
            .insert(chave.to_string());
    }

    /// Recalcula o conjunto de chaves a partir das escritas que sobraram.
    ///
    /// Chamado depois de um `ROLLBACK TO SAVEPOINT`: sem isto, a chave de uma
    /// linha DESCARTADA continuaria barrando a proxima igual a ela, e o
    /// `SAVEPOINT` deixaria de desfazer de verdade.
    /// `por_escrita` e indexado pela POSICAO na lista de escrita, e nao pela
    /// ordem das insercoes: contar so as insercoes daria um indice que muda de
    /// significado quando alguem intercala um `UPDATE` no meio, e o conjunto
    /// sairia deslocado sem ninguem perceber.
    pub fn refazer_chaves(&mut self, por_escrita: &HashMap<usize, Vec<(String, String)>>) {
        let mut novas: HashMap<String, HashSet<String>> = HashMap::new();
        for (i, e) in self.escritas.iter().enumerate() {
            if e.acao != Acao::Inserir {
                continue;
            }
            let Some(chaves) = por_escrita.get(&i) else {
                continue;
            };
            for (indice, chave) in chaves {
                novas
                    .entry(format!("{}|{}", e.tabela.to_lowercase(), indice))
                    .or_default()
                    .insert(chave.clone());
            }
        }
        self.chaves = novas;
    }

    /// A ficha da transacao para o cliente.
    ///
    /// # Os campos que existem, e os dois que NAO existem
    ///
    /// O capitulo pede `transaction_id`, `transaction_state`,
    /// `transaction_start_time`, `transaction_isolation`,
    /// `transaction_read_only`, a idade e a contagem de linhas. Sete dos oito
    /// saem daqui.
    ///
    /// **`transaction_read_only` nao existe e nao vai fingir que existe.** Uma
    /// transacao so de leitura nao tem o que declarar aqui: leitura nao passa
    /// pela transacao (§4.3 -- ela nem ve as proprias escritas), nao reserva
    /// tabela e nao custa nada. Um campo dizendo `false` para sempre seria um
    /// campo que nunca respondeu pergunta nenhuma.
    pub fn ficha(&self, agora_ms: i64) -> Json {
        Json::objeto(vec![
            ("transaction_id", Json::de_u64(self.id)),
            ("transaction_state", Json::texto_de(self.estado.nome())),
            (
                "transaction_start_time",
                Json::texto_de(phxsql_core::datahora::instante_iso(self.desde_ms)),
            ),
            (
                "transaction_isolation",
                Json::texto_de(if self.leitura_repetivel {
                    NIVEL_DE_ISOLAMENTO_REPETIVEL
                } else {
                    NIVEL_DE_ISOLAMENTO
                }),
            ),
            ("leitura_repetivel", Json::Bool(self.leitura_repetivel)),
            (
                "idade_ms",
                Json::de_u64((agora_ms - self.desde_ms).max(0) as u64),
            ),
            (
                "expira_em_s",
                Json::de_u64(((self.expira_ms - agora_ms).max(0) / 1000) as u64),
            ),
            ("linhas", Json::de_u64(self.escritas.len() as u64)),
            ("database", Json::texto_de(&self.database)),
            (
                "tabelas",
                Json::Lista(self.tabelas.iter().map(Json::texto_de).collect()),
            ),
            // Declarado e EFETIVO aparecem separados: quem declarou quatro
            // tabelas e ficou com seis precisa ver quais duas entraram sem ele
            // pedir. Juntar as duas listas numa so esconderia exatamente a
            // informacao pela qual a separacao existe.
            (
                "tabelas_declaradas",
                Json::Lista(self.declaradas.iter().map(Json::texto_de).collect()),
            ),
            (
                "tabelas_efetivas",
                Json::Lista(self.efetivas.iter().map(Json::texto_de).collect()),
            ),
            (
                "tabelas_expandidas",
                Json::Lista(self.expandidas.iter().map(Json::texto_de).collect()),
            ),
            ("lock_mode", Json::texto_de(self.modo.nome())),
            ("scope_mode", Json::texto_de(self.escopo_modo.nome())),
            (
                "lock_timeout_ms",
                Json::de_u64(self.lock_timeout_ms.max(0) as u64),
            ),
            (
                "statement_timeout_ms",
                Json::de_u64(self.statement_timeout_ms.max(0) as u64),
            ),
            ("esperando", Json::texto_de(&self.esperando)),
            (
                "savepoints",
                Json::Lista(
                    self.pontos
                        .iter()
                        .map(|p| Json::texto_de(&p.nome))
                        .collect(),
                ),
            ),
            ("ligacao", Json::de_u64(self.ligacao)),
            ("usuario", Json::texto_de(&self.usuario)),
            ("ip", Json::texto_de(&self.ip)),
            ("motivo_do_aborto", Json::texto_de(&self.motivo_do_aborto)),
        ])
    }
}

/// O nome do nivel de isolamento, sem enfeite.
///
/// **Nao e ANSI SERIALIZABLE e nao se chama assim.** O que se entrega, com
/// precisao: escrita serializavel POR TABELA (ninguem mais escreve nas tabelas
/// da transacao, e o efeito aparece de uma vez), leitura confirmada e nao
/// bloqueante (nunca ha dado nao confirmado em lugar nenhum, porque ele esta
/// em RAM), e nenhuma leitura repetivel -- a transacao nao tem retrato e nao
/// ve as proprias escritas.
pub const NIVEL_DE_ISOLAMENTO: &str =
    "escrita serializavel por tabela, leitura confirmada e nao bloqueante, sem leitura repetivel";

/// O nivel quando a transacao PEDIU leitura repetivel (`docs/SOMBRA.md` §5b).
///
/// E leitura repetivel POR EXCLUSAO, nao por versao: a transacao segura a
/// trava compartilhada (S) em cada tabela que le, ate o fim, e nenhum escritor
/// grava nessas tabelas enquanto isso -- logo a releitura devolve o mesmo
/// estado e nenhuma linha nasce no meio (sem fantasma). Continua sem
/// SERIALIZABLE: o *write skew* nao esta coberto (`SOMBRA.md` §1.4).
pub const NIVEL_DE_ISOLAMENTO_REPETIVEL: &str =
    "leitura repetivel pela trava (S por tabela lida, ate o fim), sem fantasma; escrita serializavel por tabela";

// ------------------------------------------------------------- o registro

/// O que a abertura declarou -- os tres prazos e os dois modos.
///
/// # Por que parametros NOMEADOS, e nao posicionais
///
/// A forma posicional (`Transaction(a, b, c, 5s)`) nao estende: no dia em que
/// entra o segundo prazo, nao ha onde ele caiba sem quebrar quem ja escreveu.
/// E ela confunde duas coisas de naturezas diferentes -- tabela e duracao --
/// na mesma lista. Nomeado, acrescentar um campo e acrescentar um campo.
#[derive(Debug, Clone)]
pub struct Abertura {
    pub transacao_ms: i64,
    pub lock_ms: i64,
    pub statement_ms: i64,
    pub modo: crate::travas::Modo,
    pub escopo_modo: crate::travas::EscopoModo,
    /// `"leitura_repetivel": true` na abertura. Opt-in: guarda nova entra
    /// pedida, nao imposta.
    pub leitura_repetivel: bool,
}

/// Quem tem transacao aberta, por conexao.
#[derive(Debug, Default)]
pub struct Transacoes {
    dentro: HashMap<u64, Transacao>,
    /// De onde sai o proximo `id`. Semeado com o relogio no arranque para que
    /// dois processos seguidos nao gerem o mesmo nome de marca.
    proximo: u64,
}

impl Transacoes {
    pub fn nova(semente: i64) -> Transacoes {
        Transacoes {
            dentro: HashMap::new(),
            // Milissegundos desde a epoca, que e monotono na pratica e nao
            // repete entre dois arranques do mesmo dia.
            proximo: semente.max(1) as u64,
        }
    }

    /// Abre uma transacao para esta conexao. Recusa se ja houver uma.
    pub fn abrir(
        &mut self,
        ligacao: u64,
        usuario: &str,
        ip: &str,
        agora_ms: i64,
        prazos: &Abertura,
    ) -> Result<u64> {
        if let Some(t) = self.dentro.get(&ligacao) {
            return Err(PhxError::Esquema(format!(
                "esta conexao ja tem a transacao {} aberta desde {}; \
                 transacao aninhada nao existe aqui -- use SAVEPOINT",
                t.id,
                phxsql_core::datahora::instante_iso(t.desde_ms)
            )));
        }
        self.proximo += 1;
        let id = self.proximo;
        self.dentro.insert(
            ligacao,
            Transacao {
                id,
                ligacao,
                usuario: usuario.to_string(),
                ip: ip.to_string(),
                database: String::new(),
                estado: Estado::Ativa,
                desde_ms: agora_ms,
                expira_ms: agora_ms + prazos.transacao_ms,
                lock_timeout_ms: prazos.lock_ms,
                statement_timeout_ms: prazos.statement_ms,
                modo: prazos.modo,
                escopo_modo: prazos.escopo_modo,
                leitura_repetivel: prazos.leitura_repetivel,
                declaradas: Vec::new(),
                efetivas: Vec::new(),
                expandidas: Vec::new(),
                esperando: String::new(),
                escritas: Vec::new(),
                pontos: Vec::new(),
                tabelas: Vec::new(),
                motivo_do_aborto: String::new(),
                chaves: HashMap::new(),
            },
        );
        Ok(id)
    }

    pub fn de(&self, ligacao: u64) -> Option<&Transacao> {
        self.dentro.get(&ligacao)
    }

    pub fn de_mut(&mut self, ligacao: u64) -> Option<&mut Transacao> {
        self.dentro.get_mut(&ligacao)
    }

    pub fn tirar(&mut self, ligacao: u64) -> Option<Transacao> {
        self.dentro.remove(&ligacao)
    }

    pub fn quantas(&self) -> usize {
        self.dentro.len()
    }

    /// A transacao de id `tx`, para o recado de uma trava barrada nomear quem
    /// segura. Sem isto, «tabela em transacao» manda a pessoa procurar sozinha.
    pub fn por_id(&self, tx: u64) -> Option<&Transacao> {
        self.dentro.values().find(|t| t.id == tx)
    }

    /// O recado de uma trava barrada, ja com quem segura e desde quando.
    pub fn recado_da_barrada(&self, b: &crate::travas::Barrada, agora_ms: i64) -> String {
        let onde = match b.rowid {
            // Rowid zero nao e linha: e o FIM da tabela, que e o que duas
            // transacoes disputam quando as duas anexam. Dizer «a linha 0»
            // mandaria alguem procurar uma linha que nao existe.
            Some(crate::travas::FIM_DA_TABELA) => format!("o fim de {}", b.tabela),
            Some(r) => format!("a linha {r} de {}", b.tabela),
            None => format!("a tabela {} inteira", b.tabela),
        };
        match self.por_id(b.transacao) {
            Some(t) => format!(
                "{onde} esta travada ({}) {}",
                b.trava.nome(),
                quem(t, agora_ms)
            ),
            None => format!(
                "{onde} esta travada ({}) pela transacao {}",
                b.trava.nome(),
                b.transacao
            ),
        }
    }

    /// As transacoes vencidas, para o servidor as desfazer.
    pub fn vencidas(&self, agora_ms: i64) -> Vec<u64> {
        self.dentro
            .values()
            .filter(|t| t.expira_ms <= agora_ms)
            .map(|t| t.ligacao)
            .collect()
    }

    pub fn todas(&self, agora_ms: i64) -> Vec<Json> {
        let mut v: Vec<&Transacao> = self.dentro.values().collect();
        v.sort_by_key(|t| t.desde_ms);
        v.into_iter().map(|t| t.ficha(agora_ms)).collect()
    }
}

/// Quem segura, e ha quanto tempo. Escrito uma vez porque aparece em toda
/// recusa de trava, e uma recusa que nao nomeia quem segura manda a pessoa
/// procurar sozinha.
fn quem(t: &Transacao, agora_ms: i64) -> String {
    let ha = ((agora_ms - t.desde_ms).max(0) / 1000) as u64;
    let dono = if t.usuario.is_empty() {
        format!("pela transacao {} (ligacao {})", t.id, t.ligacao)
    } else {
        format!(
            "pela transacao {} de {} (ligacao {})",
            t.id, t.usuario, t.ligacao
        )
    };
    format!(
        "{dono}, aberta em {} ha {ha}s; ela solta no COMMIT ou no ROLLBACK",
        phxsql_core::datahora::instante_iso(t.desde_ms)
    )
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
fn criar_privado(caminho: &Path, id: u64) -> Result<std::fs::File> {
    let mut opcoes = std::fs::OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opcoes.mode(0o600);
    }
    opcoes.open(caminho).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            // O erro cru aqui e "File exists", que manda procurar problema de
            // disco. O que ha e outra coisa, e quem le precisa saber qual.
            PhxError::Esquema(format!(
                "ja existe a marca da transacao {id} em {}: ela e um COMMIT que \
                 espera recuperacao, e grava-la por cima apagaria a intencao dele -- \
                 suba o servidor para a recuperacao completa-la antes de tentar de novo",
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
    f.write_all(&b)?;
    // O `sync_all` e a peca, e nao um detalhe: sem ele a marca pode estar so
    // no cache do sistema quando a passada comecar, e a queda deixaria o dado
    // meio gravado sem nenhuma intencao no disco para completar.
    f.sync_all()?;
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
    pub impossiveis: Vec<String>,
    /// Marcas CIFRADAS que este servidor nao conseguiu abrir. **Ficaram no
    /// disco**, e cada linha diz qual e por que -- ver [`Leitura::SemChave`].
    pub paradas: Vec<String>,
    pub ms: u64,
}

impl Relatorio {
    pub fn houve(&self) -> bool {
        self.achadas > 0
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
        s.push_str(&format!(
            "\x20 tempo ......................... {} ms",
            self.ms
        ));
        s
    }
}

/// Varre a base inteira atras de marcas orfas e **completa** o que achar.
///
/// # Por que ela anda para a FRENTE, e nunca para tras
///
/// Nao e escolha estetica. Desfazer exigiria devolver slots ja gravados, e o
/// `.reg` nunca reaproveita slot -- a regra que decide tudo neste desenho.
/// Andar para a frente e a unica direcao que o formato permite, e o `.tx` e o
/// que torna isso possivel: sem ele, nao se sabe para onde ir.
///
/// A reaplicacao e **idempotente pelo rowid**: cada operacao diz o slot que
/// devia ter escrito. Slot ja ocupado -- passa adiante. Slot livre e no fim da
/// tabela -- grava.
pub fn recuperar(dados: &Instancia) -> Relatorio {
    let comeco = std::time::Instant::now();
    let mut r = Relatorio::default();
    let Ok(bases) = dados.databases() else {
        return r;
    };
    for nome in bases {
        let Ok(db) = dados.abrir_database(&nome) else {
            continue;
        };
        let dir = db.caminho().to_path_buf();
        let Ok(entradas) = std::fs::read_dir(&dir) else {
            continue;
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
        // Ordem estavel: duas marcas na mesma base sao completadas na ordem em
        // que foram criadas, que e a ordem do id no nome.
        marcas.sort();
        for caminho in marcas {
            r.achadas += 1;
            match ler_marca(&caminho) {
                Ok(Leitura::Aberta(marca)) => {
                    completar(&db, &marca, &mut r);
                    r.completadas += 1;
                }
                // A terceira resposta: cifrada, e esta chave nao a abre.
                // **Nao se apaga.** Apagar aqui trocaria confidencialidade por
                // durabilidade -- uma transacao confirmada sumiria por falta de
                // uma senha, e o selo existe para proteger o dado, nao para
                // custar o dado.
                Ok(Leitura::SemChave(motivo)) => {
                    r.paradas.push(format!("{}: {motivo}", caminho.display()));
                    continue;
                }
                // Marca que nao confere, ou que nem da para abrir: commit que
                // nunca comecou.
                _ => r.descartadas += 1,
            }
            let _ = std::fs::remove_file(&caminho);
        }
    }
    r.ms = comeco.elapsed().as_millis() as u64;
    r
}

/// Reaplica o que falta de UMA marca.
fn completar(db: &phxsql_store::catalogo::Database, marca: &Marca, r: &mut Relatorio) {
    let mut tabelas: HashMap<String, phxsql_store::table::Table> = HashMap::new();
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
            let mut maes = crate::servidor::MaesAbertas {
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
    for (_, mut t) in tabelas {
        // O relatorio CONTA o que a cascata reconstruiu, junto do que a marca
        // reconstruiu: reparar em silencio seria trocar um recado ruim por
        // nenhum recado.
        r.indices_reconstruidos += t.indices_da_cascata_reconstruidos();
        let _ = t.sincronizar();
    }
}

/// Aplica uma operacao da marca. `Ok(false)` = ja estava aplicada.
///
/// `maes` empresta a conferencia de FK as tabelas que a mesma marca ja
/// reaplicou -- o conserto do P0 valendo tambem na recuperacao (ver
/// [`completar`] e `docs/ACID.md` §0).
fn aplicar_uma(
    t: &mut phxsql_store::table::Table,
    op: &OperacaoDaMarca,
    maes: &mut dyn phxsql_store::table::MaesEmProgresso,
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
        Acao::ExcluirSuave => Ok(t.excluir_suave(op.rowid, &op.motivo)?),
        Acao::ExcluirDeVez => Ok(t.excluir_de_vez(op.rowid, &op.motivo)?),
        Acao::Restaurar => Ok(t.restaurar(op.rowid, &op.motivo)?),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

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

    #[test]
    fn a_classe_do_erro_separa_instrucao_de_transacao() {
        assert_eq!(
            ClasseDoErro::do_erro(&PhxError::Duplicado("x".into())),
            ClasseDoErro::Instrucao
        );
        assert_eq!(
            ClasseDoErro::do_erro(&PhxError::Tipo("x".into())),
            ClasseDoErro::Instrucao
        );
        assert_eq!(
            ClasseDoErro::do_erro(&PhxError::Autorizacao("x".into())),
            ClasseDoErro::Instrucao
        );
        assert_eq!(
            ClasseDoErro::do_erro(&PhxError::Io(std::io::Error::other("disco"))),
            ClasseDoErro::Transacao
        );
        assert_eq!(
            ClasseDoErro::do_erro(&PhxError::Corrompido("x".into())),
            ClasseDoErro::Transacao
        );
    }

    fn abertura() -> Abertura {
        Abertura {
            transacao_ms: 60_000,
            lock_ms: 500,
            statement_ms: 0,
            modo: crate::travas::Modo::Auto,
            escopo_modo: crate::travas::EscopoModo::Dinamico,
            leitura_repetivel: false,
        }
    }

    #[test]
    fn abrir_duas_vezes_na_mesma_conexao_recusa() {
        let mut t = Transacoes::nova(1_000);
        assert!(t.abrir(1, "adm", "127.0.0.1", 0, &abertura()).is_ok());
        let e = t.abrir(1, "adm", "127.0.0.1", 0, &abertura()).unwrap_err();
        assert!(e.to_string().contains("SAVEPOINT"), "{e}");
        // Outra conexao abre a sua sem esbarrar.
        assert!(t.abrir(2, "adm", "127.0.0.1", 0, &abertura()).is_ok());
        assert_eq!(t.quantas(), 2);
    }

    #[test]
    fn o_recado_da_barrada_nomeia_quem_segura() {
        let mut t = Transacoes::nova(1);
        let id = t.abrir(1, "ana", "10.0.0.1", 0, &abertura()).unwrap();
        let b = crate::travas::Barrada {
            transacao: id,
            tabela: "loja/pedidos".into(),
            rowid: Some(9001),
            trava: crate::travas::Trava::Exclusiva,
        };
        let recado = t.recado_da_barrada(&b, 10_000);
        assert!(recado.contains("9001"), "{recado}");
        assert!(recado.contains("ana"), "{recado}");
        assert!(recado.contains("ROLLBACK"), "{recado}");

        // Transacao que ja saiu: o recado nao mente, so fica mais curto.
        let b2 = crate::travas::Barrada {
            transacao: 999,
            tabela: "loja/pedidos".into(),
            rowid: None,
            trava: crate::travas::Trava::Exclusiva,
        };
        assert!(t.recado_da_barrada(&b2, 10_000).contains("999"));
    }

    #[test]
    fn o_prazo_da_transacao_aparece_nas_vencidas() {
        let mut t = Transacoes::nova(1);
        t.abrir(1, "ana", "10.0.0.1", 0, &abertura()).unwrap();
        assert!(t.vencidas(59_000).is_empty());
        assert_eq!(t.vencidas(60_001), vec![1]);
    }

    /// A ficha mostra DECLARADO e EFETIVO separados. Junta-los numa lista so
    /// esconderia exatamente a informacao pela qual a separacao existe: quais
    /// tabelas entraram sem ninguem pedir.
    #[test]
    fn a_ficha_separa_declarado_de_efetivo() {
        let mut t = Transacoes::nova(1);
        t.abrir(1, "ana", "10.0.0.1", 0, &abertura()).unwrap();
        let tx = t.de_mut(1).unwrap();
        tx.declaradas = vec!["loja/pedidos".into()];
        tx.efetivas = vec!["loja/auditoria".into(), "loja/pedidos".into()];
        let f = tx.ficha(1_000);
        let d: Vec<&str> = f
            .campo("tabelas_declaradas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .filter_map(Json::texto)
            .collect();
        let e: Vec<&str> = f
            .campo("tabelas_efetivas")
            .and_then(Json::lista)
            .unwrap()
            .iter()
            .filter_map(Json::texto)
            .collect();
        assert_eq!(d, vec!["loja/pedidos"]);
        assert_eq!(e, vec!["loja/auditoria", "loja/pedidos"]);
        assert_eq!(f.texto_ou("lock_mode", ""), "AUTO");
        assert_eq!(f.texto_ou("scope_mode", ""), "DYNAMIC");
    }
}

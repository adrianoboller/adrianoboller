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
//! * o laco do arranque sobre os databases e a completude da marca EM VOO. A
//!   marca `transacao_<id>.tx` em si -- o formato, a gravacao, a leitura e a
//!   recuperacao que responde «se a energia cair exatamente aqui, o banco sabe
//!   dizer o que aconteceu?» -- mora em `phxsql_store::marca` desde o pedido
//!   563, para o embutido completar a cascata pelo mesmo motor, e e
//!   reexportada aqui.

#[cfg(test)]
use crate::apoio_teste::DirTemp;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_store::catalogo::Instancia;

// A marca e a recuperacao moram no store desde o pedido 563, para o embutido
// completar a cascata dele pelo MESMO motor. Aqui fica o nome de sempre, para
// quem ja chamava `transacao::gravar_marca` nao mudar -- reexportar, e nao
// copiar: dois motores de marca seriam a copia que diverge.
pub use phxsql_store::marca::{
    caminho_da_marca, codificar_linha, decodificar_linha, decodificar_linha_em, e_marca_do_bidi,
    estado_do_slot, falhar_a_proxima_leitura_de_teste, gravar_marca, gravar_marca_da_replica,
    gravar_marca_do_bidi, gravar_marca_posicional, ler_marca, marcas_do_bidi_em, tratar_marca,
    versoes_antes, Acao, Bilhete, Escrita, EventoDaReplica, EventoDoGrupo, Leitura, Marca,
    NoArranque, OperacaoDaMarca, Relatorio, EXTENSAO, MAGIC, PREFIXO, VERSAO,
    VERSAO_CASCATA_EM_CLARO, VERSAO_LINHA_ANTIGA_SEM_CASCATA, VERSAO_REPLICA_CIFRADA,
    VERSAO_REPLICA_EM_CLARO, VERSAO_SEM_LINHA_ANTIGA,
};

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
    /// A transacao que barrou o ULTIMO `COMMIT` desta, pela trava de um elo
    /// da cascata (pedido 516, condicao C1 do papel C). `None` no comeco de
    /// cada `COMMIT`. E a aresta do grafo de espera que o desempate le -- ver
    /// [`Transacoes::commit_barrado`].
    pub commit_barrado_por: Option<u64>,
}

/// O que o desempate do [`Transacoes::commit_barrado`] decidiu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ciclo {
    /// A transacao que barrou este `COMMIT`.
    pub outra: u64,
    /// Esta e a MAIS NOVA do ciclo: ela cede.
    pub ceder: bool,
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
                commit_barrado_por: None,
            },
        );
        Ok(id)
    }

    /// O numero da marca de uma escrita SOLTA que cascateia (pedido 540) --
    /// a transacao de uma instrucao, que nao entra no registro.
    ///
    /// Sai do MESMO contador das transacoes: o numero e o nome do arquivo
    /// `.tx`, e duas fontes de numero seriam duas chances de uma marca
    /// sobrescrever a outra no mesmo diretorio.
    pub fn numero_de_marca(&mut self) -> u64 {
        self.proximo += 1;
        self.proximo
    }

    /// O `COMMIT` da transacao de `ligacao` foi barrado pela trava que a
    /// transacao `por` segura. Anota a aresta e diz se ela FECHA um ciclo de
    /// `COMMIT`s barrados -- e, fechando, quem cede.
    ///
    /// # Por que existe (condicao C1 do papel C ao 516)
    ///
    /// O `COMMIT` nao espera trava com a trava de dados na mao: tenta uma vez
    /// e recusa com `repetir: true`. Isso matou o abraco com a trava global,
    /// e criou outro: T1 barrado por T2 e T2 barrado por T1, os dois mandados
    /// repetir, repetindo -- 1.870 rodadas em 11 s, medidas pelo papel C,
    /// sem ninguem sair. O modulo `travas` diz «sem espera nao ha grafo de
    /// espera»; a repeticao que o recado manda fazer E a espera, so que no
    /// cliente, e o grafo volta por ela.
    ///
    /// # O desempate
    ///
    /// Segue a corrente `commit_barrado_por` a partir de `por`. Se ela volta a
    /// esta transacao, ha ciclo, e a MAIS NOVA dele cede -- o id cresce na
    /// ordem de abertura, entao a mais nova e a de id maior. Corrente que nao
    /// volta a esta transacao nao e ciclo desta, e ninguem cede aqui.
    ///
    /// **A idade e escolha NOSSA, e nao copia de motor** (R2 da re-checagem
    /// do papel C). O PostgreSQL aborta uma das transacoes do impasse sem
    /// ordem garantida, e a documentacao dele diz para nao contar com qual; o
    /// InnoDB aborta a mais LEVE, pelas linhas alteradas. Aqui a regra e a
    /// idade, no molde do wait-die: e deterministica -- o mesmo ciclo cede
    /// sempre pela mesma, quem quer que o descubra -- e garante progresso,
    /// porque a mais velha nunca cede e cada rodada tira uma transacao do
    /// ciclo.
    ///
    /// # So transacao ATIVA entra na corrente
    ///
    /// A aresta e de quem ainda vai mandar COMMIT de novo. A que esta em
    /// `ABORT_ONLY` (pelo teto, por exemplo) segura as travas ate o ROLLBACK,
    /// mas nao confirma mais: esperar por ela e espera comum, e fazer outra
    /// ceder por causa dela seria abortar sem ciclo (R1). E o `rollback_para`
    /// apaga a aresta, porque o elo que a criou pode ter saido com a lista.
    pub fn commit_barrado(&mut self, ligacao: u64, por: u64) -> Option<Ciclo> {
        let eu = {
            let tx = self.dentro.get_mut(&ligacao)?;
            tx.commit_barrado_por = Some(por);
            tx.id
        };
        let mut no_ciclo = vec![eu];
        let mut atual = por;
        while atual != eu {
            if no_ciclo.contains(&atual) {
                // Um ciclo que nao passa por esta: e dos outros.
                return None;
            }
            no_ciclo.push(atual);
            let outra = self.por_id(atual)?;
            if outra.estado != Estado::Ativa {
                return None;
            }
            atual = outra.commit_barrado_por?;
        }
        let mais_nova = no_ciclo.iter().copied().max().unwrap_or(eu);
        Some(Ciclo {
            outra: por,
            ceder: mais_nova == eu,
        })
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

    /// Depois de um panico com o registro na mao: toda transacao ATIVA vai
    /// para `ABORT_ONLY` -- pedido 458.
    ///
    /// # Por que todas, e nao a que estava no meio
    ///
    /// O conjunto de escrita de uma delas pode ter ficado pela metade no meio
    /// de um empilhamento (a cascata entra em varios passos), e o registro nao
    /// sabe qual: o panico nao diz de que conexao era. Confirmar meia operacao
    /// e o que a atomicidade proibe; pedir ROLLBACK a quem nada perdeu custa
    /// uma repeticao. E o que os tres maduros fazem com o mesmo panico: o
    /// PostgreSQL(R) reinicia TODOS os processos e toda transacao em voo se
    /// desfaz; MySQL(R) e MariaDB caem inteiros. Aqui o servidor fica de pe e
    /// a transacao NOVA funciona -- a diferenca e so que ninguem precisou
    /// reiniciar para isso.
    ///
    /// As que ja estavam confirmando ou revertendo ficam como estao: sao de
    /// quem as conduz, e a marca `.tx` no disco e quem decide o COMMIT que
    /// comecou (a recuperacao do arranque).
    ///
    /// # Por que as travas ficam
    ///
    /// A abortada segura as travas ate o ROLLBACK, a queda ou o prazo -- a
    /// escolha desta casa para todo `ABORT_ONLY`, e nao a do PostgreSQL(R),
    /// que solta no abort. Solta-las aqui nao da: isto roda com `transacoes`
    /// na mao, e tomar `travas` dali inverteria a ordem do
    /// `barrado_por_travas` (`travas`, e dentro dela `transacoes`).
    pub fn abortar_abertas(&mut self) {
        for t in self.dentro.values_mut() {
            if t.estado == Estado::Ativa {
                t.estado = Estado::AbortOnly;
                if t.motivo_do_aborto.is_empty() {
                    t.motivo_do_aborto = "um panico em outra operacao sujou o registro das \
                                          transacoes, e o conjunto de escrita desta nao pode \
                                          ser afirmado inteiro"
                        .into();
                }
            }
        }
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
    ///
    /// A que esta no COMMIT (`Confirmando`) fica de fora -- pedido 559. A
    /// varredura roda no `begin` de OUTRA conexao, sem a trava de dados, e o
    /// COMMIT roda com ela na mao: encerra-la aqui soltava as travas de quem
    /// ainda estava gravando, e outra transacao pegava a linha no meio da
    /// passada. Quem decide o prazo dela e o proprio COMMIT, que o confere
    /// antes da marca (pedido 539); depois da marca a transacao aconteceu, e
    /// prazo nenhum a desfaz.
    pub fn vencidas(&self, agora_ms: i64) -> Vec<u64> {
        self.dentro
            .values()
            .filter(|t| t.expira_ms <= agora_ms && t.estado != Estado::Confirmando)
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
    // O LOGIN do dono da trava nao sai (pedido 549): o recado vai a quem
    // esbarrou, que pode nem ter direito na tabela travada, e o login e dado
    // de outro usuario. Os tres maduros convergem -- o PostgreSQL nomeia o
    // processo e a transacao, MySQL e MariaDB so o «Lock wait timeout»;
    // nenhum nomeia o usuario. Quem administra ve o dono pelo `transacoes`.
    let dono = format!("pela transacao {} (ligacao {})", t.id, t.ligacao);
    format!(
        "{dono}, aberta em {} ha {ha}s; ela solta no COMMIT ou no ROLLBACK",
        phxsql_core::datahora::instante_iso(t.desde_ms)
    )
}

/// Varre a instancia inteira atras de marcas orfas e **completa** o que
/// achar -- o arranque do servidor.
///
/// O motor e o [`phxsql_store::catalogo::Database::recuperar_marcas`], que o
/// embutido tambem chama (pedido 563): aqui so mora o laco sobre os
/// databases. Cada um completa as marcas dele e so DEPOIS reconstroi o indice
/// marcado, e e essa ordem que impede o passe do 522 de calar a orfa -- ver o
/// documento de la.
/// A semente do contador de transacoes e de marcas do servidor -- pedido
/// 714: o relogio, ou o maior id de marca que ja esta no disco, de qualquer
/// database e de qualquer familia. E o MESMO gerador do embutido
/// (`marca::proximo_id_acima_de`), menos um, porque o [`Transacoes`] soma um
/// antes de entregar.
pub fn semente_do_contador(dados: &Instancia) -> i64 {
    let mut maior = 0u64;
    if let Ok(bases) = dados.databases() {
        for nome in bases {
            if let Ok(db) = dados.abrir_database(&nome) {
                maior = maior.max(db.maior_id_de_marca());
            }
        }
    }
    let proximo = phxsql_store::marca::proximo_id_acima_de(maior);
    (proximo - 1).min(i64::MAX as u64) as i64
}

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
        r.somar(db.recuperar_marcas());
    }
    r.ms = comeco.elapsed().as_millis() as u64;
    r
}

/// [`completar_marca_em_voo`] completa UMA marca com o servidor de pe e a trava de dados JA na mao de
/// quem chama -- a do `COMMIT` cuja passada acabou de quebrar depois da marca.
///
/// # Por que com a trava de quem chama, e nao tomando outra
///
/// Porque soltar e tomar de novo abre uma fresta em que outra escrita entra
/// no meio da transacao confirmada. E porque tomar de novo SEM soltar era o
/// defeito do pedido 426: a trava nao e reentrante, a segunda tomada devolvia
/// erro, e esta recuperacao nunca rodava.
///
/// # Por que so a marca DESTE commit
///
/// Porque as outras marcas da base sao de commits que ja aplicaram e esperam o
/// fecho da janela de durabilidade (`marcas_pendentes`): nao ha o que
/// completar nelas, e quem as apaga e quem sincroniza.
///
/// A marca EM VOO do reparo da trava de dados (pedido 451). `gravada` diz se
/// o `gravar_marca` dela ja voltou `Ok`: antes disso, marca que nao existe ou
/// que nao confere e commit que nunca comecou, e sai como sempre; depois,
/// marca que nao se rele FICA -- ver [`NoArranque::Gravada`].
pub fn completar_marca_em_voo(
    dados: &Instancia,
    database: &str,
    caminho: &Path,
    gravada: bool,
) -> Relatorio {
    let politica = if gravada {
        NoArranque::Gravada
    } else {
        NoArranque::Nao
    };
    completar_com(dados, database, caminho, politica)
}

/// O corpo do [`completar_marca_em_voo`]: o que muda entre o arranque e o
/// reparo e so a politica.
fn completar_com(
    dados: &Instancia,
    database: &str,
    caminho: &Path,
    politica: NoArranque,
) -> Relatorio {
    let comeco = std::time::Instant::now();
    let mut r = Relatorio::default();
    match dados.abrir_database(database) {
        Ok(db) => {
            if tratar_marca(&db, caminho, &mut r, politica) {
                let _ = std::fs::remove_file(caminho);
            }
        }
        Err(e) => r.impossiveis.push(format!(
            "o database {database} nao abriu para completar {} ({e})",
            caminho.display()
        )),
    }
    r.ms = comeco.elapsed().as_millis() as u64;
    r
}

/// As tabelas de `diretorio` ligadas a `iniciais` por chave estrangeira -- em
/// QUALQUER direcao e em QUALQUER profundidade --, `iniciais` inclusive.
///
/// # Por que o componente inteiro, e nao so a tabela
///
/// Porque e o que um `COMMIT` pode abrir para GRAVAR por causa delas: a mae,
/// na conferencia da chave do `inserir` e do `atualizar`; a filha, na
/// conferencia do `excluir`; a neta, na cascata do `ao_alterar` -- e a cascata
/// pode alcancar linha que so nasceu depois do `empilhar`. Todas passam pelo
/// portao do congelamento, e a que estiver congelada derruba a passada no
/// meio. Pedido 426.
///
/// Medir um raio so (a tabela e as vizinhas) erra justamente na cascata que
/// desce mais de um nivel; o componente e o que cobre as tres portas sem
/// perguntar qual delas a passada vai usar.
///
/// # A tabela que nao se le sem escrever
///
/// Troca de volume interrompida: o esquema dela nao se le sem cura-la, e
/// curar e escrever. Ela entra LIGADA A TODAS -- o lado seguro, porque uma
/// chave que nao se ve e uma porta que ninguem confere. E rara (so depois de
/// uma queda no meio de uma troca), e a proxima abertura que grava a cura.
pub fn componente_de_chave(diretorio: &Path, todas: &[String], iniciais: &[String]) -> Vec<String> {
    let chave = |n: &str| n.to_ascii_lowercase();
    let mut vizinhas: HashMap<String, Vec<String>> = HashMap::new();
    let mut coringas: Vec<String> = Vec::new();
    for nome in todas {
        match phxsql_store::RegFile::abrir_sem_escrever(diretorio, nome) {
            Ok(Some(reg)) => {
                for fk in reg.esquema().chaves_estrangeiras() {
                    // `vendas.clientes` mora no mesmo diretorio que `clientes`:
                    // a qualificacao e do NOME declarado, e o arquivo e o mesmo.
                    let (_, mae) = phxsql_store::catalogo::separar_qualificado(&fk.tabela_ref);
                    vizinhas.entry(chave(nome)).or_default().push(chave(&mae));
                    vizinhas.entry(chave(&mae)).or_default().push(chave(nome));
                }
            }
            _ => coringas.push(chave(nome)),
        }
    }
    let mut dentro: HashSet<String> = HashSet::new();
    let mut fila: Vec<String> = iniciais.iter().map(|n| chave(n)).collect();
    while let Some(n) = fila.pop() {
        if !dentro.insert(n.clone()) {
            continue;
        }
        if let Some(v) = vizinhas.get(&n) {
            fila.extend(v.iter().cloned());
        }
        // O coringa liga a todas -- e todas ligam ao coringa.
        if coringas.contains(&n) {
            fila.extend(todas.iter().map(|t| chave(t)));
        } else {
            fila.extend(coringas.iter().cloned());
        }
    }
    todas
        .iter()
        .filter(|t| dentro.contains(&chave(t)))
        .cloned()
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use phxsql_core::value::Value;

    fn dir(rotulo: &str) -> DirTemp {
        DirTemp::novo(&format!("tx-{rotulo}"))
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

    /// **O desempate do C1 (pedido 516)**, na regua pura: o ciclo de dois e o
    /// de tres cedem pela MAIS NOVA, qualquer que seja quem o descobre; a
    /// corrente que nao volta e a espera comum, e ninguem cede.
    #[test]
    fn o_ciclo_de_commits_barrados_cede_pela_mais_nova() {
        let mut t = Transacoes::nova(1);
        let a = t.abrir(1, "a", "ip", 0, &abertura()).unwrap();
        let b = t.abrir(2, "b", "ip", 0, &abertura()).unwrap();
        let c = t.abrir(3, "c", "ip", 0, &abertura()).unwrap();
        assert!(a < b && b < c, "o id cresce na ordem de abertura");

        // A corrente sem volta: a barrado por b, que nao esta barrado.
        assert_eq!(t.commit_barrado(1, b), None);
        // b barrado por a fecha o ciclo; b e a mais nova, e cede.
        assert_eq!(
            t.commit_barrado(2, a),
            Some(Ciclo {
                outra: a,
                ceder: true
            })
        );
        // Descoberto pela mais VELHA, o mesmo ciclo nao a faz ceder.
        assert_eq!(
            t.commit_barrado(1, b),
            Some(Ciclo {
                outra: b,
                ceder: false
            })
        );

        // Tres: a -> c -> b -> a. Quem cede e c, a mais nova, e so c.
        let mut t = Transacoes::nova(1);
        let a = t.abrir(1, "a", "ip", 0, &abertura()).unwrap();
        let b = t.abrir(2, "b", "ip", 0, &abertura()).unwrap();
        let c = t.abrir(3, "c", "ip", 0, &abertura()).unwrap();
        assert_eq!(t.commit_barrado(1, c), None);
        assert_eq!(t.commit_barrado(3, b), None);
        assert_eq!(
            t.commit_barrado(2, a),
            Some(Ciclo {
                outra: a,
                ceder: false
            })
        );
        assert_eq!(
            t.commit_barrado(3, b),
            Some(Ciclo {
                outra: b,
                ceder: true
            })
        );
        // O ciclo dos OUTROS nao e desta: d barrada por a, que esta no ciclo
        // a -> c -> b -> a, espera sem ceder.
        let _d = t.abrir(4, "d", "ip", 0, &abertura()).unwrap();
        assert_eq!(t.commit_barrado(4, a), None);

        // R1: a aresta de uma transacao que nao vai mais confirmar nao fecha
        // ciclo. a barrada por b, e a vai a ABORT_ONLY (o teto, por exemplo):
        // b barrada por a e espera comum, e b NAO cede.
        let mut t = Transacoes::nova(1);
        let a = t.abrir(1, "a", "ip", 0, &abertura()).unwrap();
        let b = t.abrir(2, "b", "ip", 0, &abertura()).unwrap();
        assert_eq!(t.commit_barrado(1, b), None);
        t.de_mut(1).unwrap().estado = Estado::AbortOnly;
        assert_eq!(t.commit_barrado(2, a), None);
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
        assert!(recado.contains(&format!("transacao {id}")), "{recado}");
        assert!(recado.contains("ROLLBACK"), "{recado}");
        // Pedido 549: o login do dono NAO sai -- era o comportamento que
        // este teste cravava, e e o que o pedido desfaz.
        assert!(
            !recado.contains("ana"),
            "o recado entregou o login: {recado}"
        );

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

    /// **Pedido 503, item 2 -- o irmao do 509: a tabela que nao foi ao disco
    /// segura a marca, inclusive no arranque.**
    ///
    /// O `completar` fazia `let _ = t.sincronizar()`, e o `recuperar` apagava
    /// a marca assim mesmo: a mesma ordem do fecho da janela (sincronizar,
    /// apagar o bilhete), com o erro engolido. A falha vem do sistema
    /// operacional, e nao de um sinalizador: o `.pag` da tabela vira
    /// DIRETORIO, e o `sincronizar` termina gravando o descritor -- o gatilho
    /// do `fsync_que_falha_no_fio_tambem_segura_as_marcas`. Nao e o `fsync`
    /// forjado do `phxsql_store::sincronia`, de proposito: este binario de
    /// testes sobe servidores, e o servidor registra o `abort` na recusa.
    ///
    /// Com o defeito: a marca sai do disco e o relatorio nao diz nada. Com o
    /// conserto: a marca fica, o relatorio diz por que, e quando o `.pag`
    /// volta a se gravar a recuperacao seguinte a completa e a apaga -- sem
    /// duplicar a linha, porque a reaplicacao e idempotente pelo rowid.
    #[test]
    fn tabela_que_nao_foi_ao_disco_segura_a_marca_no_arranque() {
        use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
        use phxsql_core::types::ColumnType;

        let d = dir("nao-foi-ao-disco");
        let inst = Instancia::nova(&d).unwrap();
        let db = inst.criar_database("loja").unwrap();
        let esquema = Schema::new(
            "clientes",
            vec![
                Column::new("id", ColumnType::Int4).obrigatoria(),
                Column::new("nome", ColumnType::Str(40)),
            ],
            vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
        )
        .unwrap();
        let mut t = db.criar_tabela(None, esquema).unwrap();
        t.sincronizar().unwrap();
        drop(t);
        let marca = gravar_marca(
            db.caminho(),
            7,
            0,
            &[Escrita {
                database: "loja".into(),
                tabela: "clientes".into(),
                acao: Acao::Inserir,
                rowid: 1,
                linha: vec![Value::Int(1), Value::Str("Ana".into())],
                linha_antiga: Vec::new(),
                motivo: String::new(),
                cascata_na_lista: false,
                elo_do_empilhar: false,
                elo_da_cascata: false,
            }],
        )
        .unwrap();
        let pag = db.caminho().join("clientes.pag");
        let _ = std::fs::remove_file(&pag);
        std::fs::create_dir(&pag).unwrap();

        let r = recuperar(&inst);
        assert!(
            marca.exists(),
            "a tabela nao foi ao disco e a marca do commit saiu assim mesmo: o \
             bilhete que a traria de volta se perdeu ({:?})",
            r.impossiveis
        );
        assert!(
            r.impossiveis.iter().any(|i| i.contains("nao foi ao disco")),
            "a marca ficou, e o relatorio tinha de dizer por que: {:?}",
            r.impossiveis
        );

        std::fs::remove_dir(&pag).unwrap();
        let r = recuperar(&inst);
        assert!(
            r.impossiveis.is_empty() && !marca.exists(),
            "com o disco de volta a marca tinha de se completar e sair: {:?}",
            r.impossiveis
        );
        assert_eq!(
            db.abrir_qualificada("clientes").unwrap().registros(),
            1,
            "a segunda recuperacao duplicou a linha"
        );
    }

    /// **Pedido 522: o arranque reconstroi o indice que o processo anterior
    /// so FECHOU.**
    ///
    /// O `fechar` deixa o byte 52 em 1 -- so o fecho da janela grava o 0 --,
    /// e o processo novo nao tem o atestado do velho. Sem este passe, toda
    /// tabela escrita desde o ultimo fecho da janela subia recusando ate
    /// alguem mandar `reindexar`; com ele, sobe reconstruida, sincronizada e
    /// contada. O controle vai junto: a tabela sincronizada nao se reconstroi.
    #[test]
    fn o_arranque_reconstroi_o_indice_que_so_foi_fechado() {
        use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
        use phxsql_core::types::ColumnType;

        let d = dir("522-arranque");
        let inst = Instancia::nova(&d).unwrap();
        let db = inst.criar_database("loja").unwrap();
        let esquema = |nome: &str| {
            Schema::new(
                nome,
                vec![
                    Column::new("id", ColumnType::Int4).obrigatoria(),
                    Column::new("nome", ColumnType::Str(40)),
                ],
                vec![IndexDef::new("porId", vec![IndexColumn::asc(0)]).unico()],
            )
            .unwrap()
        };
        for (nome, sincroniza) in [("fechada", false), ("sincronizada", true)] {
            let mut t = db.criar_tabela(None, esquema(nome)).unwrap();
            for i in 1..=300 {
                t.inserir(&[Value::Int(i), Value::Str(format!("c{i}"))])
                    .unwrap();
            }
            if sincroniza {
                t.sincronizar().unwrap();
            }
        }
        // O processo novo: o atestado do velho nao atravessa o `exec`.
        phxsql_store::ndx::esquecer_atestados_para_teste(&d);
        assert!(
            db.abrir_qualificada("fechada")
                .unwrap()
                .indice_precisa_reconstruir(),
            "premissa: sem atestado, o 1 do fechar manda reconstruir"
        );

        let r = recuperar(&inst);
        assert!(r.indices_pendentes.is_empty(), "{:?}", r.indices_pendentes);
        assert_eq!(
            r.indices_reconstruidos, 1,
            "o arranque tinha de reconstruir a tabela so fechada, e so ela"
        );
        assert!(
            r.houve(),
            "reconstruir e nao contar no relatorio e reparar calado"
        );
        phxsql_store::ndx::esquecer_atestados_para_teste(&d);
        let mut t = db.abrir_qualificada("fechada").unwrap();
        assert!(
            !t.indice_precisa_reconstruir(),
            "a reconstrucao do arranque ficou so no nucleo: a proxima queda \
             antes do primeiro fecho a marcaria de novo"
        );
        for i in [1, 150, 300] {
            assert_eq!(t.buscar("porId", &[Value::Int(i)]).unwrap().len(), 1);
        }
    }
}

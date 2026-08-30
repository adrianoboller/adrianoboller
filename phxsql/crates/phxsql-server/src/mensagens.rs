//! As mensagens que o servidor devolve pelo protocolo, numa TABELA de verdade.
//!
//! # O desenho
//!
//! Cada mensagem tem um `TextName` estavel (`erro.em_carga`, `erro.sem_direito`)
//! e um texto por idioma, numa tabela comum do motor: `phxsys.mensagens`. Ser
//! tabela comum e a decisao central -- a grade do Centro de Controle ja edita
//! tabela, a permissao por base ja protege quem pode mexer, o diario ja conta
//! quem mudou o que. Nenhum mecanismo novo, nenhum arquivo de formato novo.
//!
//! # A resolucao, em tres degraus
//!
//! 1. a celula do idioma configurado (`"idioma"` no `config.json`);
//! 2. vazia? cai para a coluna `Portugues`;
//! 3. linha ausente (ou tabela ausente)? cai para o texto de FABRICA -- o que
//!    esta escrito neste arquivo, que e byte a byte o que o servidor sempre
//!    respondeu.
//!
//! E por isso que **sem config de idioma e sem tabela nada muda**: o degrau 3
//! e o comportamento de sempre, e ha teste que compara byte a byte.
//!
//! # O que NAO muda com o idioma
//!
//! O campo estruturado do erro -- `codigo`, `nome`, `classe`, `repetir` -- e
//! sempre o mesmo. Cliente antigo trata pelo codigo e continua funcionando em
//! qualquer lingua. E o log de acessos grava o texto de fabrica do Display,
//! entao filtro de log (fail2ban) nao quebra por troca de idioma.
//!
//! # Custo no caminho quente
//!
//! Zero no sucesso: mensagem so existe em resposta de ERRO. No erro, um
//! `HashMap` em memoria; a tabela so e relida quando o `mtime` do `.reg`
//! muda, conferido no maximo a cada [`INTERVALO_DE_CONFERENCIA`] -- o mesmo
//! desenho do `recarregar_se_mudou` da blacklist.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use phxsql_core::error::PhxError;

/// As seis colunas de idioma, na ordem das colunas da tabela.
///
/// Os nomes sao exatamente os nomes das colunas -- e o valor aceito no campo
/// `"idioma"` do `config.json`. `Portugues` e o indice 0 de proposito: e o
/// degrau intermediario da resolucao.
pub const IDIOMAS: [&str; 6] = [
    "Portugues",
    "Frances",
    "Ingles",
    "Italiano",
    "Alemao",
    "Espanhol",
];

/// Onde a tabela mora. Um database comum chamado `phxsys`: aparece na arvore,
/// abre na grade, obedece a permissao por base como qualquer outro.
pub const DATABASE: &str = "phxsys";
pub const TABELA: &str = "mensagens";

/// De quanto em quanto tempo vale a pena conferir o `mtime` da tabela.
///
/// Editou pela tela, o texto novo vale em ate este intervalo -- sem reiniciar.
/// O custo e um `stat` por intervalo, e so quando ha erro fluindo.
pub const INTERVALO_DE_CONFERENCIA: Duration = Duration::from_secs(2);

/// Uma mensagem de fabrica: o nome estavel e o texto em cada idioma.
///
/// `textos[0]` e o Portugues e nunca e vazio -- e o texto que o servidor
/// sempre respondeu. Celula vazia nao e semeada e cai para o portugues na
/// resolucao: melhor nenhuma traducao do que uma inventada.
pub struct MensagemFabrica {
    pub nome: &'static str,
    pub textos: [&'static str; 6],
}

/// Todas as mensagens que o servidor devolve pelo protocolo.
///
/// As doze primeiras sao as MOLDURAS dos erros -- o prefixo do `Display` de
/// cada variante de `PhxError`, com `{detalhe}` no lugar da parte variavel.
/// As demais sao os textos que os portoes do servidor criam por inteiro.
/// A parte variavel (`{detalhe}` e os outros parametros) continua no idioma
/// em que o motor a escreveu: traduzir cada `format!` do motor e o passo
/// seguinte, nao este.
pub const FABRICA: &[MensagemFabrica] = &[
    // -------------------------------------------------- molduras dos erros
    MensagemFabrica {
        nome: "erro.corrompido",
        textos: [
            "arquivo corrompido: {detalhe}",
            "fichier corrompu : {detalhe}",
            "corrupted file: {detalhe}",
            "file danneggiato: {detalhe}",
            "beschädigte Datei: {detalhe}",
            "archivo dañado: {detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.assinatura_invalida",
        textos: [
            "assinatura invalida em {detalhe}",
            "signature invalide dans {detalhe}",
            "invalid signature in {detalhe}",
            "firma non valida in {detalhe}",
            "ungültige Signatur in {detalhe}",
            "firma inválida en {detalhe}",
        ],
    },
    // O texto desta variante nao se divide em moldura + detalhe sem picar a
    // frase em tres. A moldura e o texto inteiro; traducao fica para quando
    // alguem precisar dela de verdade -- celula vazia cai para o portugues.
    MensagemFabrica {
        nome: "erro.versao_nao_suportada",
        textos: ["{detalhe}", "", "", "", "", ""],
    },
    MensagemFabrica {
        nome: "erro.esquema_invalido",
        textos: [
            "esquema invalido: {detalhe}",
            "schéma invalide : {detalhe}",
            "invalid schema: {detalhe}",
            "schema non valido: {detalhe}",
            "ungültiges Schema: {detalhe}",
            "esquema inválido: {detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.tipo_invalido",
        textos: [
            "tipo invalido: {detalhe}",
            "type invalide : {detalhe}",
            "invalid type: {detalhe}",
            "tipo non valido: {detalhe}",
            "ungültiger Typ: {detalhe}",
            "tipo inválido: {detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.nao_encontrado",
        textos: [
            "nao encontrado: {detalhe}",
            "introuvable : {detalhe}",
            "not found: {detalhe}",
            "non trovato: {detalhe}",
            "nicht gefunden: {detalhe}",
            "no encontrado: {detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.duplicado",
        textos: [
            "chave duplicada: {detalhe}",
            "clé dupliquée : {detalhe}",
            "duplicate key: {detalhe}",
            "chiave duplicata: {detalhe}",
            "doppelter Schlüssel: {detalhe}",
            "clave duplicada: {detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.conflito",
        textos: [
            "conflito de escrita: {detalhe}",
            "conflit d'écriture : {detalhe}",
            "write conflict: {detalhe}",
            "conflitto di scrittura: {detalhe}",
            "Schreibkonflikt: {detalhe}",
            "conflicto de escritura: {detalhe}",
        ],
    },
    // As duas mensagens que NAO se traduzem, e por motivos diferentes:
    //
    // - `erro.redireciona` comeca com `REDIRECIONA host:porta`, que e o
    //   pedaco que o cliente RECORTA para se reapontar. Traduzir quebraria
    //   todo cliente que trata o redirecionamento -- a moldura e so o
    //   detalhe, nos seis idiomas.
    // - `erro.sinal` carrega a MESSAGE_TEXT que o DONO DO BANCO escreveu no
    //   gatilho. Substitui-la por texto nosso seria apagar a voz dele; o
    //   idioma dessa mensagem e escolha de quem escreveu o gatilho.
    // O cancelamento e a TERCEIRA que nao se traduz por moldura, e por um
    // motivo proprio: o texto ja vem montado do ponto que cancelou, com quem
    // encerrou e o que estava rodando. Traduzir de verdade exigiria mover
    // essa montagem para a tabela -- trabalho que so vale quando alguem
    // pedir a tela noutro idioma e esbarrar nisto.
    MensagemFabrica {
        nome: "erro.cancelado",
        textos: [
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.spare_em_espera",
        textos: [
            "spare em espera: {detalhe}",
            "serveur de secours en attente : {detalhe}",
            "spare on standby: {detalhe}",
            "server di riserva in attesa: {detalhe}",
            "Reserveserver im Wartezustand: {detalhe}",
            "servidor de reserva en espera: {detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.redireciona",
        textos: [
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.sinal",
        textos: [
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
            "{detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.acesso_negado",
        textos: [
            "acesso negado: {detalhe}",
            "accès refusé : {detalhe}",
            "access denied: {detalhe}",
            "accesso negato: {detalhe}",
            "Zugriff verweigert: {detalhe}",
            "acceso denegado: {detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.em_carga",
        textos: [
            "tabela em carga: {detalhe}",
            "table en cours de chargement : {detalhe}",
            "table under bulk load: {detalhe}",
            "tabella in caricamento: {detalhe}",
            "Tabelle wird geladen: {detalhe}",
            "tabla en carga: {detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.em_transacao",
        textos: [
            "tabela em transacao: {detalhe}",
            "table dans une transaction : {detalhe}",
            "table held by a transaction: {detalhe}",
            "tabella in transazione: {detalhe}",
            "Tabelle in einer Transaktion: {detalhe}",
            "tabla en transacción: {detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.transacao_abortada",
        textos: [
            "transacao abortada: {detalhe}",
            "transaction abandonnée : {detalhe}",
            "transaction aborted: {detalhe}",
            "transazione interrotta: {detalhe}",
            "Transaktion abgebrochen: {detalhe}",
            "transacción abortada: {detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.limite_excedido",
        textos: [
            "limite excedido: {detalhe}",
            "limite dépassée : {detalhe}",
            "limit exceeded: {detalhe}",
            "limite superato: {detalhe}",
            "Limit überschritten: {detalhe}",
            "límite excedido: {detalhe}",
        ],
    },
    MensagemFabrica {
        nome: "erro.erro_de_es",
        textos: [
            "erro de E/S: {detalhe}",
            "erreur d'E/S : {detalhe}",
            "I/O error: {detalhe}",
            "errore di I/O: {detalhe}",
            "E/A-Fehler: {detalhe}",
            "error de E/S: {detalhe}",
        ],
    },
    // -------------------------------------------------- textos dos portoes
    MensagemFabrica {
        nome: "erro.token_invalido",
        textos: [
            "token invalido",
            "jeton invalide",
            "invalid token",
            "token non valido",
            "ungültiges Token",
            "token inválido",
        ],
    },
    MensagemFabrica {
        nome: "erro.ip_nao_autorizado",
        textos: [
            "ip nao autorizado",
            "IP non autorisée",
            "ip not authorized",
            "IP non autorizzato",
            "IP nicht autorisiert",
            "IP no autorizada",
        ],
    },
    // As duas do aperto de mao da porta de dados. Ver `docs/CIFRA-DO-FIO.md`.
    //
    // A segunda e a UNICA coisa que um cliente velho recebe de um servidor com
    // `cifra_fio.exigir` ligado, entao ela tem de dizer o que fazer -- um
    // "acesso negado" seco mandaria procurar a permissao errada.
    MensagemFabrica {
        nome: "erro.cifra_do_fio_desligada",
        textos: [
            "este servidor nao atende a cifra do fio (cifra_fio.ligada esta em false)",
            "ce serveur ne prend pas en charge le chiffrement du lien",
            "this server does not offer wire encryption",
            "questo server non offre la cifratura del collegamento",
            "dieser Server bietet keine Leitungsverschlüsselung an",
            "este servidor no ofrece cifrado del enlace",
        ],
    },
    MensagemFabrica {
        nome: "erro.cifra_do_fio_exigida",
        textos: [
            "este servidor exige a cifra do fio: peca o aperto de mao com \
             {\"op\":\"cifrar\"} antes de qualquer outro pedido",
            "ce serveur exige le chiffrement du lien : demandez la poignée de main \
             avec {\"op\":\"cifrar\"} avant toute autre requête",
            "this server requires wire encryption: ask for the handshake with \
             {\"op\":\"cifrar\"} before any other request",
            "questo server richiede la cifratura del collegamento: chieda \
             {\"op\":\"cifrar\"} prima di qualsiasi altra richiesta",
            "dieser Server verlangt Leitungsverschlüsselung: fordern Sie den \
             Handschlag mit {\"op\":\"cifrar\"} vor jeder anderen Anfrage an",
            "este servidor exige el cifrado del enlace: pida el saludo con \
             {\"op\":\"cifrar\"} antes de cualquier otra petición",
        ],
    },
    MensagemFabrica {
        nome: "erro.ip_bloqueado",
        textos: [
            "bloqueado desde {desde} ate {ate} por {motivo} ({comando})",
            "bloqué depuis {desde} jusqu'à {ate} pour {motivo} ({comando})",
            "blocked since {desde} until {ate} for {motivo} ({comando})",
            "bloccato da {desde} fino a {ate} per {motivo} ({comando})",
            "gesperrt seit {desde} bis {ate} wegen {motivo} ({comando})",
            "bloqueado desde {desde} hasta {ate} por {motivo} ({comando})",
        ],
    },
    MensagemFabrica {
        nome: "erro.credencial_invalida",
        textos: [
            "usuario ou senha invalidos",
            "utilisateur ou mot de passe invalide",
            "invalid user or password",
            "utente o password non validi",
            "Benutzer oder Passwort ungültig",
            "usuario o contraseña inválidos",
        ],
    },
    MensagemFabrica {
        nome: "erro.faca_login",
        textos: [
            "faca login antes: {\"op\":\"login\",\"usuario\":...,\"senha\":...}",
            "connectez-vous d'abord : {\"op\":\"login\",\"usuario\":...,\"senha\":...}",
            "log in first: {\"op\":\"login\",\"usuario\":...,\"senha\":...}",
            "eseguire prima il login: {\"op\":\"login\",\"usuario\":...,\"senha\":...}",
            "zuerst anmelden: {\"op\":\"login\",\"usuario\":...,\"senha\":...}",
            "inicie sesión primero: {\"op\":\"login\",\"usuario\":...,\"senha\":...}",
        ],
    },
    MensagemFabrica {
        nome: "erro.replica_nao_autorizada",
        textos: [
            "este ip nao esta em replicacao.replicas_autorizadas",
            "cette ip n'est pas dans replicacao.replicas_autorizadas",
            "this ip is not in replicacao.replicas_autorizadas",
            "questo ip non e in replicacao.replicas_autorizadas",
            "diese IP steht nicht in replicacao.replicas_autorizadas",
            "esta ip no esta en replicacao.replicas_autorizadas",
        ],
    },
    MensagemFabrica {
        nome: "erro.somente_leitura",
        textos: [
            "servidor em modo somente leitura",
            "serveur en lecture seule",
            "server in read-only mode",
            "server in sola lettura",
            "Server im Nur-Lese-Modus",
            "servidor en modo de solo lectura",
        ],
    },
    MensagemFabrica {
        nome: "erro.sem_direito",
        textos: [
            "{login} nao tem permissao de {atividade} em {alvo}",
            "{login} n'a pas la permission {atividade} sur {alvo}",
            "{login} has no {atividade} permission on {alvo}",
            "{login} non ha il permesso {atividade} su {alvo}",
            "{login} hat keine Berechtigung {atividade} für {alvo}",
            "{login} no tiene permiso de {atividade} en {alvo}",
        ],
    },
    MensagemFabrica {
        nome: "erro.comando_proibido",
        textos: [
            "operacao {op} esta proibida neste servidor",
            "l'opération {op} est interdite sur ce serveur",
            "operation {op} is forbidden on this server",
            "l'operazione {op} è vietata su questo server",
            "Operation {op} ist auf diesem Server verboten",
            "la operación {op} está prohibida en este servidor",
        ],
    },
    MensagemFabrica {
        nome: "erro.base_proibida",
        textos: [
            "a base {base} esta proibida neste servidor",
            "la base {base} est interdite sur ce serveur",
            "database {base} is forbidden on this server",
            "il database {base} è vietato su questo server",
            "Datenbank {base} ist auf diesem Server verboten",
            "la base {base} está prohibida en este servidor",
        ],
    },
    MensagemFabrica {
        nome: "erro.nome_hostil",
        textos: [
            "{rotulo} {valor} nao e um nome",
            "{rotulo} {valor} n'est pas un nom",
            "{rotulo} {valor} is not a name",
            "{rotulo} {valor} non è un nome",
            "{rotulo} {valor} ist kein Name",
            "{rotulo} {valor} no es un nombre",
        ],
    },
    MensagemFabrica {
        nome: "erro.grave_bloqueado",
        textos: [
            "{recado}; o IP foi bloqueado",
            "{recado} ; l'adresse IP a été bloquée",
            "{recado}; the IP address has been blocked",
            "{recado}; l'indirizzo IP è stato bloccato",
            "{recado}; die IP-Adresse wurde gesperrt",
            "{recado}; la IP fue bloqueada",
        ],
    },
    MensagemFabrica {
        nome: "erro.grave_tentativa",
        textos: [
            "{recado}; tentativa {n} de {m}",
            "{recado} ; tentative {n} sur {m}",
            "{recado}; attempt {n} of {m}",
            "{recado}; tentativo {n} di {m}",
            "{recado}; Versuch {n} von {m}",
            "{recado}; intento {n} de {m}",
        ],
    },
    MensagemFabrica {
        nome: "erro.operacao_desconhecida",
        textos: [
            "operacao desconhecida: {op}",
            "opération inconnue : {op}",
            "unknown operation: {op}",
            "operazione sconosciuta: {op}",
            "unbekannte Operation: {op}",
            "operación desconocida: {op}",
        ],
    },
];

/// O texto de fabrica (Portugues) de um TextName conhecido.
pub fn fabrica_de(nome: &str) -> Option<&'static str> {
    FABRICA.iter().find(|m| m.nome == nome).map(|m| m.textos[0])
}

/// Decompoe um erro em (TextName da moldura, parte variavel).
///
/// A regra que os testes travam: `moldura de fabrica + detalhe == Display`,
/// byte a byte, para toda variante. E o que garante que linha ausente na
/// tabela devolve exatamente o texto de sempre.
pub fn decompor(e: &PhxError) -> (&'static str, String) {
    match e {
        PhxError::Corrompido(m) => ("erro.corrompido", m.clone()),
        PhxError::BadMagic {
            arquivo,
            esperado,
            encontrado,
        } => (
            "erro.assinatura_invalida",
            format!(
                "{arquivo}: esperado {:?}, encontrado {:?}",
                String::from_utf8_lossy(esperado.as_slice()),
                String::from_utf8_lossy(encontrado.as_slice())
            ),
        ),
        PhxError::VersaoNaoSuportada { .. } => ("erro.versao_nao_suportada", e.to_string()),
        PhxError::Esquema(m) => ("erro.esquema_invalido", m.clone()),
        PhxError::Tipo(m) => ("erro.tipo_invalido", m.clone()),
        PhxError::NaoEncontrado(m) => ("erro.nao_encontrado", m.clone()),
        PhxError::Duplicado(m) => ("erro.duplicado", m.clone()),
        PhxError::Conflito(m) => ("erro.conflito", m.clone()),
        PhxError::Autorizacao(m) => ("erro.acesso_negado", m.clone()),
        PhxError::EmCarga(m) => ("erro.em_carga", m.clone()),
        PhxError::EmTransacao(m) => ("erro.em_transacao", m.clone()),
        PhxError::TransacaoAbortada(m) => ("erro.transacao_abortada", m.clone()),
        PhxError::LimiteExcedido(m) => ("erro.limite_excedido", m.clone()),
        PhxError::Cancelado(m) => ("erro.cancelado", m.clone()),
        PhxError::SpareEmEspera(m) => ("erro.spare_em_espera", m.clone()),
        PhxError::Redireciona(m) => ("erro.redireciona", m.clone()),
        PhxError::Sinal { estado, mensagem } => (
            "erro.sinal",
            format!("{mensagem} (SIGNAL SQLSTATE {estado})"),
        ),
        PhxError::Io(m) => ("erro.erro_de_es", m.to_string()),
    }
}

/// O cache da tabela, atras de uma trava propria (nunca a de dados).
struct Cache {
    /// TextName -> texto por idioma, na ordem de [`IDIOMAS`]. Vazio = celula
    /// vazia, que cai um degrau na resolucao.
    linhas: HashMap<String, [String; 6]>,
    /// O `mtime` do `.reg` no instante da carga.
    mtime: Option<SystemTime>,
    /// Quando o `mtime` foi conferido pela ultima vez.
    conferido: Option<Instant>,
}

/// O resolvedor de mensagens do servidor.
pub struct Mensagens {
    /// Indice do idioma configurado em [`IDIOMAS`]. Ausente/vazio = 0.
    idioma: usize,
    /// O `.reg` da tabela, para o `stat` barato que decide a recarga.
    caminho_reg: PathBuf,
    cache: Mutex<Cache>,
}

impl Mensagens {
    /// `idioma_cfg` vem do `config.json`, ja validado la (desconhecido vira
    /// aviso no arranque e cai em Portugues -- aqui so se resolve o indice).
    pub fn nova(idioma_cfg: &str, base: &Path) -> Mensagens {
        let idioma = IDIOMAS
            .iter()
            .position(|i| *i == idioma_cfg.trim())
            .unwrap_or(0);
        Mensagens {
            idioma,
            caminho_reg: base.join(DATABASE).join(format!("{TABELA}.reg")),
            cache: Mutex::new(Cache {
                linhas: HashMap::new(),
                mtime: None,
                conferido: None,
            }),
        }
    }

    /// O nome do idioma em uso, para a tela dizer a verdade.
    pub fn idioma(&self) -> &'static str {
        IDIOMAS[self.idioma]
    }

    /// A tabela mudou desde a ultima carga? Conferido no maximo a cada
    /// [`INTERVALO_DE_CONFERENCIA`] -- e so quem tem a Instancia consegue
    /// recarregar, entao a pergunta e separada da acao.
    pub fn precisa_recarregar(&self) -> bool {
        let Ok(mut cache) = self.cache.lock() else {
            return false;
        };
        if let Some(quando) = cache.conferido {
            if quando.elapsed() < INTERVALO_DE_CONFERENCIA {
                return false;
            }
        }
        let agora = std::fs::metadata(&self.caminho_reg)
            .ok()
            .and_then(|m| m.modified().ok());
        if agora == cache.mtime {
            cache.conferido = Some(Instant::now());
            return false;
        }
        true
    }

    /// Entrega o conteudo relido da tabela. Quem le e o servidor, DENTRO da
    /// trava de dados -- e o `mtime` e anotado aqui, no mesmo instante, para
    /// nao anotar como lida uma escrita que a leitura nao viu.
    pub fn carregar(&self, linhas: HashMap<String, [String; 6]>) {
        let mtime = std::fs::metadata(&self.caminho_reg)
            .ok()
            .and_then(|m| m.modified().ok());
        if let Ok(mut cache) = self.cache.lock() {
            cache.linhas = linhas;
            cache.mtime = mtime;
            cache.conferido = Some(Instant::now());
        }
    }

    /// Forca a proxima resolucao a reler a tabela -- a semeadura chama.
    pub fn invalidar(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.mtime = None;
            cache.conferido = None;
        }
    }

    /// A moldura da tabela, se houver: celula do idioma, senao Portugues.
    fn resolver(&self, nome: &str) -> Option<String> {
        let cache = self.cache.lock().ok()?;
        let linha = cache.linhas.get(nome)?;
        let escolhida = &linha[self.idioma];
        if !escolhida.is_empty() {
            return Some(escolhida.clone());
        }
        // Celula vazia cai para o portugues DA TABELA; portugues vazio cai
        // para a fabrica, que e o mesmo texto que a semeadura gravou.
        let portugues = &linha[0];
        if !portugues.is_empty() {
            return Some(portugues.clone());
        }
        None
    }

    /// O texto de uma mensagem, com os parametros no lugar dos `{marcadores}`.
    ///
    /// Sem tabela, e a fabrica -- byte a byte o texto de sempre. Nome que nao
    /// existe na fabrica volta como esta: e defeito de programacao, e sumir
    /// com o erro seria pior que um texto estranho na resposta.
    pub fn texto(&self, nome: &str, parametros: &[(&str, &str)]) -> String {
        let mut moldura = self
            .resolver(nome)
            .unwrap_or_else(|| fabrica_de(nome).unwrap_or(nome).to_string());
        for (chave, valor) in parametros {
            moldura = moldura.replace(&format!("{{{chave}}}"), valor);
        }
        moldura
    }

    /// O texto humano de um erro, pela moldura da variante.
    pub fn texto_do_erro(&self, e: &PhxError) -> String {
        let (nome, detalhe) = decompor(e);
        self.texto(nome, &[("detalhe", &detalhe)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sem_tabela() -> Mensagens {
        Mensagens::nova("", &std::env::temp_dir().join("phxsql-msg-inexistente"))
    }

    /// **O teste que garante o comportamento velho.** Sem tabela, o texto de
    /// erro e byte a byte o `Display` de sempre, para TODA variante.
    #[test]
    fn sem_tabela_o_texto_e_o_de_fabrica_byte_a_byte() {
        let m = sem_tabela();
        let todos = [
            PhxError::Corrompido("crc nao confere".into()),
            PhxError::BadMagic {
                arquivo: "x.reg".into(),
                esperado: b"PHXSQL01",
                encontrado: *b"YYYYYYYY",
            },
            PhxError::VersaoNaoSuportada {
                arquivo: "x.reg".into(),
                encontrada: 9,
                suportada: 2,
            },
            PhxError::Esquema("informe \"tabela\"".into()),
            PhxError::Tipo("nao e numero".into()),
            PhxError::NaoEncontrado("rowid 7".into()),
            PhxError::Duplicado("porNome".into()),
            PhxError::Conflito("versao 3, esperada 2".into()),
            PhxError::Autorizacao("token invalido".into()),
            PhxError::EmCarga("clientes reservada".into()),
            PhxError::EmTransacao("clientes na transacao 3".into()),
            PhxError::TransacaoAbortada("houve erro de transacao".into()),
            PhxError::LimiteExcedido("Str(10)".into()),
            PhxError::Io(std::io::Error::other("disco cheio")),
        ];
        for e in &todos {
            assert_eq!(
                m.texto_do_erro(e),
                e.to_string(),
                "a moldura de fabrica divergiu do Display em {:?}",
                e.nome()
            );
        }
    }

    /// A resolucao em tres degraus: idioma -> Portugues -> fabrica.
    #[test]
    fn resolve_pelo_idioma_e_cai_degrau_a_degrau() {
        let m = Mensagens::nova("Ingles", &std::env::temp_dir());
        let mut linhas = HashMap::new();
        // Traduzida: sai o ingles.
        linhas.insert(
            "erro.em_carga".to_string(),
            [
                "tabela em carga: {detalhe}".to_string(),
                String::new(),
                "table busy: {detalhe}".to_string(),
                String::new(),
                String::new(),
                String::new(),
            ],
        );
        // Celula do idioma vazia: sai o portugues DA TABELA (editado).
        linhas.insert(
            "erro.token_invalido".to_string(),
            [
                "chave de acesso errada".to_string(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ],
        );
        m.carregar(linhas);

        assert_eq!(
            m.texto_do_erro(&PhxError::EmCarga("clientes".into())),
            "table busy: clientes"
        );
        assert_eq!(
            m.texto("erro.token_invalido", &[]),
            "chave de acesso errada"
        );
        // Linha ausente: fabrica, byte a byte.
        assert_eq!(
            m.texto_do_erro(&PhxError::Duplicado("porNome".into())),
            "chave duplicada: porNome"
        );
    }

    /// **Prova real do degrau 2** (o defeito reposto que este teste pega):
    /// se o fallback de celula vazia devolvesse a celula como esta, o cliente
    /// receberia texto VAZIO -- pior que sem traducao nenhuma.
    #[test]
    fn celula_vazia_nunca_vira_texto_vazio() {
        let m = Mensagens::nova("Alemao", &std::env::temp_dir());
        let mut linhas = HashMap::new();
        linhas.insert(
            "erro.somente_leitura".to_string(),
            [
                "servidor em modo somente leitura".to_string(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ],
        );
        // Linha semeada com TODAS as celulas vazias (alguem apagou o
        // portugues pela tela): cai para a fabrica, nunca para "".
        linhas.insert(
            "erro.token_invalido".to_string(),
            std::array::from_fn(|_| String::new()),
        );
        m.carregar(linhas);

        assert_eq!(
            m.texto("erro.somente_leitura", &[]),
            "servidor em modo somente leitura"
        );
        assert_eq!(m.texto("erro.token_invalido", &[]), "token invalido");
    }

    #[test]
    fn idioma_desconhecido_cai_em_portugues() {
        let m = Mensagens::nova("Klingon", &std::env::temp_dir());
        assert_eq!(m.idioma(), "Portugues");
        let m = Mensagens::nova("", &std::env::temp_dir());
        assert_eq!(m.idioma(), "Portugues");
        let m = Mensagens::nova("Espanhol", &std::env::temp_dir());
        assert_eq!(m.idioma(), "Espanhol");
    }

    #[test]
    fn parametros_entram_nos_marcadores() {
        let m = sem_tabela();
        assert_eq!(
            m.texto(
                "erro.sem_direito",
                &[("login", "maria"), ("atividade", "excluir"), ("alvo", "Z")]
            ),
            "maria nao tem permissao de excluir em Z"
        );
        // O texto do faca_login tem chaves literais do exemplo de pedido --
        // sem parametro nenhum, nada e trocado e o exemplo sobrevive.
        assert!(m
            .texto("erro.faca_login", &[])
            .contains("{\"op\":\"login\""));
    }

    /// A fabrica e a lista canonica: nome unico, prefixo estavel, portugues
    /// sempre preenchido -- e nenhuma celula com espaco disfarcado de vazio.
    #[test]
    fn a_fabrica_e_bem_formada() {
        let mut nomes: Vec<&str> = FABRICA.iter().map(|m| m.nome).collect();
        let quantos = nomes.len();
        nomes.sort_unstable();
        nomes.dedup();
        assert_eq!(nomes.len(), quantos, "ha TextName repetido na fabrica");
        for m in FABRICA {
            assert!(m.nome.starts_with("erro."), "{} sem prefixo erro.", m.nome);
            assert!(!m.textos[0].is_empty(), "{} sem portugues", m.nome);
            for t in &m.textos {
                assert_eq!(t.trim(), *t, "{} tem texto com espaco nas pontas", m.nome);
            }
        }
    }

    /// Toda variante decompoe para um TextName que existe na fabrica.
    #[test]
    fn toda_variante_tem_moldura_na_fabrica() {
        let todos = [
            PhxError::Corrompido(String::new()),
            PhxError::Esquema(String::new()),
            PhxError::Tipo(String::new()),
            PhxError::NaoEncontrado(String::new()),
            PhxError::Duplicado(String::new()),
            PhxError::Conflito(String::new()),
            PhxError::Autorizacao(String::new()),
            PhxError::EmCarga(String::new()),
            PhxError::LimiteExcedido(String::new()),
            PhxError::Cancelado(String::new()),
            PhxError::SpareEmEspera(String::new()),
            PhxError::Redireciona(String::new()),
            PhxError::Sinal {
                estado: String::new(),
                mensagem: String::new(),
            },
            PhxError::Io(std::io::Error::other("x")),
        ];
        for e in &todos {
            let (nome, _) = decompor(e);
            assert!(fabrica_de(nome).is_some(), "{nome} nao esta na fabrica");
        }
    }
}

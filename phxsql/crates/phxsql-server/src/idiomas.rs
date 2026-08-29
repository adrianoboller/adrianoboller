//! Os textos da TELA em seis idiomas, na mesma tabela das mensagens.
//!
//! # Onde isto mora
//!
//! Numa tabela comum do motor, [`DATABASE`]`.`[`TABELA`] -- a mesma que guarda
//! as mensagens do protocolo. Ser tabela comum e a decisao central: a grade do
//! Centro de Controle ja edita tabela, a permissao ja protege quem pode mexer,
//! o diario ja conta quem mudou o que. Nenhum arquivo de formato novo.
//!
//! Os `TextName` daqui comecam todos com `tela.`; os do protocolo comecam com
//! `erro.`. Os dois conjuntos convivem na mesma tabela sem se pisar, e cada um
//! semeia o seu -- por isso semear um nunca apaga o outro.
//!
//! # A resolucao, em tres degraus
//!
//! 1. a celula do idioma pedido;
//! 2. vazia? cai para a coluna `Portugues`;
//! 3. linha ausente (ou tabela ausente)? cai para o texto de FABRICA, que e o
//!    que esta escrito neste arquivo.
//!
//! O degrau 3 e o que faz **sem tabela nada mudar**: a tela em portugues e
//! exatamente a tela de sempre. E o degrau 2 e o que impede o pior defeito
//! possivel aqui -- uma celula em branco virar um botao sem rotulo. Ha teste
//! para os dois.
//!
//! # Por que a fabrica esta so aqui
//!
//! A pagina tambem sabe desenhar o formulario em portugues -- e o HTML dela.
//! Se ela carregasse tambem as SEIS traducoes, existiriam duas verdades, e a
//! segunda e sempre a que envelhece. Entao a pagina pede `/idiomas` e recebe o
//! texto ja resolvido: quem resolve e este arquivo, sozinho.
//!
//! O laco entre os dois lados e travado por teste: todo `data-txt` do
//! `index.html` tem de existir aqui, e todo texto daqui tem de aparecer la.

use std::collections::{HashMap, HashSet};

use phxsql_core::error::{PhxError, Result};
use phxsql_core::json::Json;
use phxsql_core::schema::{Column, IndexColumn, IndexDef, Schema};
use phxsql_core::types::ColumnType;
use phxsql_core::value::Value;
use phxsql_store::catalogo::Instancia;
use phxsql_store::table::Visao;

use crate::valores::json_para_linha;

/// As seis colunas de idioma, na ordem das colunas da tabela.
///
/// Os nomes sao exatamente os nomes das colunas. `Portugues` e o indice 0 de
/// proposito: e o degrau intermediario da resolucao.
pub const IDIOMAS: [&str; 6] = [
    "Portugues",
    "Frances",
    "Ingles",
    "Italiano",
    "Alemao",
    "Espanhol",
];

/// Um database comum: aparece na arvore, abre na grade, obedece a permissao.
pub const DATABASE: &str = "phxsys";
pub const TABELA: &str = "mensagens";

/// O prefixo dos `TextName` que sao texto de TELA.
///
/// E o que separa o meu conjunto do conjunto do protocolo dentro da mesma
/// tabela -- e o que a rota publica usa para nao servir mais do que precisa.
pub const PREFIXO_DA_TELA: &str = "tela.";

/// Quantos idiomas: escrito uma vez, para o resto derivar.
pub const QUANTOS: usize = IDIOMAS.len();

/// Um texto de fabrica: o nome estavel e o texto em cada idioma.
///
/// `textos[0]` e o portugues e nunca e vazio -- e o texto que a tela sempre
/// mostrou. Celula vazia nao e semeada e cai para o portugues na resolucao:
/// melhor nenhuma traducao do que uma inventada.
pub struct TextoDeFabrica {
    pub nome: &'static str,
    pub textos: [&'static str; QUANTOS],
}

/// Todo texto que a tela de entrada mostra, nos seis idiomas.
///
/// Ordem das colunas: Portugues, Frances, Ingles, Italiano, Alemao, Espanhol.
pub const FABRICA_TELA: &[TextoDeFabrica] = &[
    // ------------------------------------------------------- a moldura
    TextoDeFabrica {
        nome: "tela.assinatura",
        textos: [
            "Centro de Controle",
            "Centre de Contrôle",
            "Control Center",
            "Centro di Controllo",
            "Kontrollzentrum",
            "Centro de Control",
        ],
    },
    // ------------------------------------------------------- os campos
    TextoDeFabrica {
        nome: "tela.servidor",
        textos: [
            "Servidor", "Serveur", "Server", "Server", "Server", "Servidor",
        ],
    },
    TextoDeFabrica {
        nome: "tela.servidor_dica",
        textos: [
            "IP ou DNS",
            "IP ou DNS",
            "IP or DNS",
            "IP o DNS",
            "IP oder DNS",
            "IP o DNS",
        ],
    },
    TextoDeFabrica {
        nome: "tela.porta",
        textos: ["Porta", "Port", "Port", "Porta", "Port", "Puerto"],
    },
    TextoDeFabrica {
        nome: "tela.usuario",
        textos: [
            "Usuário",
            "Utilisateur",
            "User",
            "Utente",
            "Benutzer",
            "Usuario",
        ],
    },
    TextoDeFabrica {
        nome: "tela.senha",
        textos: [
            "Senha",
            "Mot de passe",
            "Password",
            "Password",
            "Kennwort",
            "Contraseña",
        ],
    },
    TextoDeFabrica {
        nome: "tela.token",
        textos: [
            "Token do servidor",
            "Jeton du serveur",
            "Server token",
            "Token del server",
            "Server-Token",
            "Token del servidor",
        ],
    },
    TextoDeFabrica {
        nome: "tela.chave",
        textos: [
            "Chave privada",
            "Clé privée",
            "Private key",
            "Chiave privata",
            "Privater Schlüssel",
            "Clave privada",
        ],
    },
    TextoDeFabrica {
        nome: "tela.facultativa",
        textos: [
            "facultativa",
            "facultative",
            "optional",
            "facoltativa",
            "optional",
            "facultativa",
        ],
    },
    TextoDeFabrica {
        nome: "tela.chave_dica",
        textos: [
            "Ed25519, 64 hexadecimais",
            "Ed25519, 64 hexadécimaux",
            "Ed25519, 64 hex digits",
            "Ed25519, 64 esadecimali",
            "Ed25519, 64 Hexadezimalstellen",
            "Ed25519, 64 hexadecimales",
        ],
    },
    TextoDeFabrica {
        nome: "tela.database",
        textos: [
            "Database", "Database", "Database", "Database", "Datenbank", "Database",
        ],
    },
    TextoDeFabrica {
        nome: "tela.opcional",
        textos: [
            "opcional",
            "optionnel",
            "optional",
            "opzionale",
            "optional",
            "opcional",
        ],
    },
    TextoDeFabrica {
        nome: "tela.database_dica",
        textos: [
            "abre já neste banco",
            "ouvre directement cette base",
            "opens straight into this database",
            "apre subito in questo database",
            "öffnet direkt diese Datenbank",
            "abre ya en esta base",
        ],
    },
    TextoDeFabrica {
        nome: "tela.entrar",
        textos: [
            "Entrar", "Entrer", "Sign in", "Entra", "Anmelden", "Entrar",
        ],
    },
    // ---------------------------------------------------- o que o login diz
    TextoDeFabrica {
        nome: "tela.conferindo",
        textos: [
            "Conferindo…",
            "Vérification…",
            "Checking…",
            "Verifica…",
            "Prüfung…",
            "Comprobando…",
        ],
    },
    TextoDeFabrica {
        nome: "tela.derivando",
        textos: [
            "Derivando a prova…",
            "Calcul de la preuve…",
            "Deriving the proof…",
            "Derivazione della prova…",
            "Nachweis wird abgeleitet…",
            "Derivando la prueba…",
        ],
    },
    TextoDeFabrica {
        nome: "tela.assinando",
        textos: [
            "Assinando o desafio…",
            "Signature du défi…",
            "Signing the challenge…",
            "Firma della sfida…",
            "Challenge wird signiert…",
            "Firmando el desafío…",
        ],
    },
    TextoDeFabrica {
        nome: "tela.falhou",
        textos: [
            "não consegui entrar",
            "connexion impossible",
            "could not sign in",
            "accesso non riuscito",
            "Anmeldung fehlgeschlagen",
            "no fue posible entrar",
        ],
    },
    // ------------------------------------------------------- o idioma
    TextoDeFabrica {
        nome: "tela.idioma",
        textos: [
            "Idioma", "Langue", "Language", "Lingua", "Sprache", "Idioma",
        ],
    },
    TextoDeFabrica {
        nome: "tela.idioma_dica",
        textos: [
            "A escolha vale nesta sessão e fica guardada neste navegador.",
            "Le choix vaut pour cette session et reste dans ce navigateur.",
            "The choice applies to this session and is kept in this browser.",
            "La scelta vale per questa sessione e resta in questo browser.",
            "Die Wahl gilt für diese Sitzung und bleibt in diesem Browser.",
            "La elección vale para esta sesión y queda en este navegador.",
        ],
    },
    // -------------------------------------------------- o histórico
    TextoDeFabrica {
        nome: "tela.conexoes",
        textos: [
            "Conexões salvas",
            "Connexions enregistrées",
            "Saved connections",
            "Connessioni salvate",
            "Gespeicherte Verbindungen",
            "Conexiones guardadas",
        ],
    },
    TextoDeFabrica {
        nome: "tela.conexoes_vazio",
        textos: [
            "Nenhuma conexão guardada ainda. Preencha os campos acima e clique em guardar.",
            "Aucune connexion enregistrée. Remplissez les champs ci-dessus et enregistrez.",
            "No saved connections yet. Fill in the fields above and save.",
            "Nessuna connessione salvata. Compila i campi sopra e salva.",
            "Noch keine Verbindung gespeichert. Felder oben ausfüllen und speichern.",
            "Ninguna conexión guardada. Rellene los campos de arriba y guarde.",
        ],
    },
    TextoDeFabrica {
        nome: "tela.conexoes_aviso",
        textos: [
            "Ficam neste navegador, nunca no servidor. A senha e o token NÃO são guardados.",
            "Restent dans ce navigateur, jamais sur le serveur. Le mot de passe et le jeton ne sont PAS enregistrés.",
            "Kept in this browser, never on the server. The password and token are NOT stored.",
            "Restano in questo browser, mai sul server. La password e il token NON vengono salvati.",
            "Bleiben in diesem Browser, nie auf dem Server. Kennwort und Token werden NICHT gespeichert.",
            "Quedan en este navegador, nunca en el servidor. La contraseña y el token NO se guardan.",
        ],
    },
    TextoDeFabrica {
        nome: "tela.apelido",
        textos: [
            "Apelido", "Nom", "Nickname", "Nome", "Name", "Apodo",
        ],
    },
    TextoDeFabrica {
        nome: "tela.endereco",
        textos: [
            "Endereço", "Adresse", "Address", "Indirizzo", "Adresse", "Dirección",
        ],
    },
    TextoDeFabrica {
        nome: "tela.ultimo_uso",
        textos: [
            "Último uso",
            "Dernier usage",
            "Last used",
            "Ultimo uso",
            "Zuletzt benutzt",
            "Último uso",
        ],
    },
    TextoDeFabrica {
        nome: "tela.guardar_conexao",
        textos: [
            "Guardar esta conexão",
            "Enregistrer cette connexion",
            "Save this connection",
            "Salva questa connessione",
            "Diese Verbindung speichern",
            "Guardar esta conexión",
        ],
    },
    TextoDeFabrica {
        nome: "tela.renomear",
        textos: [
            "Renomear", "Renommer", "Rename", "Rinomina", "Umbenennen", "Renombrar",
        ],
    },
    TextoDeFabrica {
        nome: "tela.remover",
        textos: [
            "Remover", "Supprimer", "Remove", "Rimuovi", "Entfernen", "Quitar",
        ],
    },
    TextoDeFabrica {
        nome: "tela.pergunta_apelido",
        textos: [
            "Que nome dar a esta conexão? (por exemplo: base da farmácia)",
            "Quel nom donner à cette connexion ? (par exemple : base de la pharmacie)",
            "What name should this connection have? (for example: pharmacy database)",
            "Che nome dare a questa connessione? (per esempio: base della farmacia)",
            "Welchen Namen soll diese Verbindung haben? (zum Beispiel: Apotheken-Datenbank)",
            "¿Qué nombre dar a esta conexión? (por ejemplo: base de la farmacia)",
        ],
    },
    TextoDeFabrica {
        nome: "tela.confirmar_remover",
        textos: [
            "Remover esta conexão da lista deste navegador?",
            "Retirer cette connexion de la liste de ce navigateur ?",
            "Remove this connection from this browser's list?",
            "Rimuovere questa connessione dall'elenco di questo browser?",
            "Diese Verbindung aus der Liste dieses Browsers entfernen?",
            "¿Quitar esta conexión de la lista de este navegador?",
        ],
    },
    TextoDeFabrica {
        nome: "tela.nunca_usada",
        textos: [
            "nunca", "jamais", "never", "mai", "nie", "nunca",
        ],
    },
];

/// A posicao de um idioma pelo nome da coluna. Desconhecido = portugues.
///
/// Cair no portugues em vez de recusar e a mesma escolha do degrau 2: idioma
/// escrito errado no navegador de alguem mostra a tela em portugues, e nao uma
/// tela em branco.
pub fn indice_do_idioma(nome: &str) -> usize {
    IDIOMAS.iter().position(|i| *i == nome).unwrap_or(0)
}

/// O texto de fabrica de um `TextName`, se ele for da tela.
pub fn fabrica(nome: &str) -> Option<&'static TextoDeFabrica> {
    FABRICA_TELA.iter().find(|f| f.nome == nome)
}

/// Este `TextName` e texto de TELA?
pub fn e_da_tela(nome: &str) -> bool {
    nome.starts_with(PREFIXO_DA_TELA)
}

/// Os tres degraus, para UM texto.
///
/// `gravadas` e a linha da tabela, quando existe. O portugues de fabrica
/// nunca e vazio, entao esta funcao **nunca devolve texto vazio** -- e e essa
/// a garantia que o teste `nenhum_degrau_devolve_texto_vazio` tranca: uma
/// celula em branco na tabela viraria um botao sem rotulo na tela.
pub fn resolver_um(
    gravadas: Option<&[String; QUANTOS]>,
    fab: &TextoDeFabrica,
    idioma: usize,
) -> String {
    if let Some(linha) = gravadas {
        // Degrau 1: a celula do idioma pedido.
        if !linha[idioma].trim().is_empty() {
            return linha[idioma].clone();
        }
        // Degrau 2: o portugues gravado.
        if !linha[0].trim().is_empty() {
            return linha[0].clone();
        }
    }
    // Degrau 3: a fabrica -- e nela o idioma vazio tambem cai no portugues.
    if !fab.textos[idioma].is_empty() {
        return fab.textos[idioma].to_string();
    }
    fab.textos[0].to_string()
}

/// Os textos da tela inteira, ja resolvidos, prontos para a pagina.
pub fn resolver_a_tela(
    gravadas: &HashMap<String, [String; QUANTOS]>,
    idioma: usize,
) -> Vec<(&'static str, String)> {
    FABRICA_TELA
        .iter()
        .map(|f| (f.nome, resolver_um(gravadas.get(f.nome), f, idioma)))
        .collect()
}

// =====================================================================
// A tabela
// =====================================================================

/// Le a tabela inteira. Tabela ausente = mapa vazio, que na resolucao
/// significa "textos de fabrica" -- o comportamento de sempre.
///
/// Nao devolve `Result` de proposito: quem chama e a rota publica e o
/// cache, e para os dois "nao ha tabela" e uma resposta, nao um erro.
pub fn ler_gravadas(dados: &Instancia) -> HashMap<String, [String; QUANTOS]> {
    let mut mapa = HashMap::new();
    let Ok(db) = dados.abrir_database(DATABASE) else {
        return mapa;
    };
    let Ok(mut t) = db.abrir_qualificada(TABELA) else {
        return mapa;
    };
    let Some(col_nome) = coluna(t.esquema(), "TextName") else {
        return mapa;
    };
    let cols: Vec<Option<usize>> = IDIOMAS.iter().map(|n| coluna(t.esquema(), n)).collect();
    let total = t.registros();
    let Ok((rowids, _)) = t.pagina_por_posicao(0, total, Visao::Ativas) else {
        return mapa;
    };
    for rowid in rowids {
        let Ok(Some(linha)) = t.ler(rowid) else {
            continue;
        };
        let Some(nome) = linha.get(col_nome).and_then(Value::como_str) else {
            continue;
        };
        let nome = nome.trim().to_string();
        if nome.is_empty() {
            continue;
        }
        let mut textos: [String; QUANTOS] = Default::default();
        for (i, pos) in cols.iter().enumerate() {
            if let Some(p) = pos {
                if let Some(s) = linha.get(*p).and_then(Value::como_str) {
                    textos[i] = s.trim().to_string();
                }
            }
        }
        mapa.insert(nome, textos);
    }
    mapa
}

fn coluna(e: &Schema, nome: &str) -> Option<usize> {
    e.colunas().iter().position(|c| c.nome == nome)
}

/// Cria `phxsys` e `phxsys.mensagens` se faltarem. Devolve (criou db, criou
/// tabela).
///
/// O esquema e o mesmo das mensagens do protocolo: `id` e `TextName` sao os
/// fixos da programacao, as seis colunas de idioma sao texto comum -- e a
/// grade do Centro de Controle ja sabe editar texto comum.
pub fn garantir_tabela(dados: &Instancia) -> Result<(bool, bool)> {
    let mut criou_db = false;
    let db = match dados.abrir_database(DATABASE) {
        Ok(db) => db,
        Err(_) => {
            criou_db = true;
            dados.criar_database(DATABASE)?
        }
    };
    let mut criou_tabela = false;
    if !db.existe_tabela(None, TABELA)? {
        let mut colunas = vec![
            Column::new("id", ColumnType::Uuid).obrigatoria(),
            Column::new("TextName", ColumnType::Str(80)).obrigatoria(),
        ];
        for idioma in IDIOMAS {
            colunas.push(Column::new(idioma, ColumnType::Str(250)));
        }
        let indices = vec![
            IndexDef::new("porId", vec![IndexColumn::asc(0)]).primaria(),
            IndexDef::new("porTextName", vec![IndexColumn::asc(1)]).unico(),
        ];
        db.criar_tabela(None, Schema::new(TABELA, colunas, indices)?)?;
        criou_tabela = true;
    }
    Ok((criou_db, criou_tabela))
}

/// O que uma carga fez. E o que a tela mostra depois de clicar.
#[derive(Default)]
pub struct Relatorio {
    pub criou_database: bool,
    pub criou_tabela: bool,
    pub incluidas: u64,
    pub alteradas: u64,
    pub intocadas: u64,
}

impl Relatorio {
    pub fn para_json(&self) -> Json {
        Json::objeto(vec![
            ("ok", Json::Bool(true)),
            ("database", Json::texto_de(DATABASE)),
            ("tabela", Json::texto_de(TABELA)),
            ("criou_database", Json::Bool(self.criou_database)),
            ("criou_tabela", Json::Bool(self.criou_tabela)),
            ("incluidas", Json::de_u64(self.incluidas)),
            ("alteradas", Json::de_u64(self.alteradas)),
            ("intocadas", Json::de_u64(self.intocadas)),
        ])
    }
}

/// O que a carga padrao sobrescreve.
///
/// A diferenca entre os dois nao e detalhe: `Nenhum` e a carga que SEMEIA e
/// nunca desfaz traducao de ninguem; os outros dois voltam texto de fabrica
/// por cima de trabalho que alguem fez. E por isso que a tela pergunta antes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sobrescrever {
    /// Semear: linha que existe fica exatamente como esta.
    Nenhum,
    /// Um idioma so: a coluna daquele idioma volta para a fabrica, as outras
    /// cinco ficam como estao.
    So(usize),
    /// Os seis idiomas voltam para a fabrica.
    Tudo,
}

/// Semeia (e opcionalmente devolve a fabrica) os textos DA TELA.
///
/// Nunca toca em `TextName` que nao seja da tela: as mensagens do protocolo
/// estao na mesma tabela e tem a propria carga. Uma carga que varresse a
/// tabela inteira apagaria o trabalho do outro conjunto.
pub fn carga(dados: &Instancia, modo: Sobrescrever) -> Result<Relatorio> {
    let mut r = Relatorio::default();
    let (criou_db, criou_tabela) = garantir_tabela(dados)?;
    r.criou_database = criou_db;
    r.criou_tabela = criou_tabela;

    let db = dados.abrir_database(DATABASE)?;
    let mut t = db.abrir_qualificada(TABELA)?;
    let Some(col_nome) = coluna(t.esquema(), "TextName") else {
        return Err(PhxError::Esquema(format!(
            "a tabela {DATABASE}.{TABELA} existe mas nao tem a coluna TextName"
        )));
    };
    let cols: Vec<Option<usize>> = IDIOMAS.iter().map(|n| coluna(t.esquema(), n)).collect();

    // Quem ja esta la, e em que rowid. A visao e TODAS de proposito: um
    // TextName marcado como excluido ainda ocupa o indice unico, e inserir
    // por cima daria chave duplicada.
    let mut onde: HashMap<String, u64> = HashMap::new();
    let total = t.registros();
    if let Ok((rowids, _)) = t.pagina_por_posicao(0, total, Visao::Todas) {
        for rowid in rowids {
            if let Ok(Some(linha)) = t.ler(rowid) {
                if let Some(nome) = linha.get(col_nome).and_then(Value::como_str) {
                    onde.insert(nome.trim().to_string(), rowid);
                }
            }
        }
    }

    for f in FABRICA_TELA {
        match onde.get(f.nome) {
            None => {
                t.inserir(&linha_de_fabrica(f, t.esquema())?)?;
                r.incluidas += 1;
            }
            Some(&rowid) if modo != Sobrescrever::Nenhum => {
                let Ok(Some(mut linha)) = t.ler(rowid) else {
                    continue;
                };
                let mut mexeu = false;
                for (i, pos) in cols.iter().enumerate() {
                    let Some(p) = pos else { continue };
                    if modo == Sobrescrever::So(i) || modo == Sobrescrever::Tudo {
                        let novo = valor_do_texto(f.textos[i]);
                        if linha[*p] != novo {
                            linha[*p] = novo;
                            mexeu = true;
                        }
                    }
                }
                if mexeu {
                    t.atualizar(rowid, &linha)?;
                    r.alteradas += 1;
                } else {
                    r.intocadas += 1;
                }
            }
            Some(_) => r.intocadas += 1,
        }
    }
    if r.incluidas > 0 || r.alteradas > 0 {
        t.sincronizar()?;
    }
    Ok(r)
}

/// A linha entra pelo MESMO caminho do `inserir` da rede (`json_para_linha`):
/// e ele que completa as colunas de sistema. Um segundo caminho de montar
/// linha seria o que diverge um dia.
fn linha_de_fabrica(f: &TextoDeFabrica, esquema: &Schema) -> Result<Vec<Value>> {
    let mut objeto = vec![
        (
            "id".to_string(),
            Json::texto_de(phxsql_core::uuid::Uuid::v7().to_string()),
        ),
        ("TextName".to_string(), Json::texto_de(f.nome)),
    ];
    for (i, idioma) in IDIOMAS.iter().enumerate() {
        // Celula vazia fica NULL: e o degrau que cai para o portugues, e e o
        // que a tela mostra como "sem traducao".
        if !f.textos[i].is_empty() {
            objeto.push((idioma.to_string(), Json::texto_de(f.textos[i])));
        }
    }
    json_para_linha(&Json::Objeto(objeto), esquema)
}

fn valor_do_texto(s: &str) -> Value {
    if s.is_empty() {
        Value::Null
    } else {
        Value::Str(s.to_string())
    }
}

// =====================================================================
// O backup
// =====================================================================

/// A versao do arquivo de backup. Sobe quando o formato mudar, para o
/// importar saber recusar o que nao entende em vez de gravar lixo.
pub const VERSAO_DO_BACKUP: u64 = 1;

/// A tabela inteira, em JSON, para o operador guardar FORA do banco.
///
/// Leva **todos** os `TextName`, e nao so os da tela: as mensagens do
/// protocolo moram na mesma tabela, e um backup que deixasse metade para tras
/// nao seria backup. Por isso o importar tambem aceita nome que nao conhece.
pub fn exportar(dados: &Instancia) -> Result<Json> {
    let gravadas = ler_gravadas(dados);
    let mut nomes: Vec<&String> = gravadas.keys().collect();
    nomes.sort(); // ordem estavel: dois backups iguais dao arquivos iguais
    let linhas: Vec<Json> = nomes
        .iter()
        .map(|nome| {
            let textos = &gravadas[*nome];
            let mut campos = vec![("TextName".to_string(), Json::texto_de(nome.as_str()))];
            for (i, idioma) in IDIOMAS.iter().enumerate() {
                if !textos[i].is_empty() {
                    campos.push((idioma.to_string(), Json::texto_de(&textos[i])));
                }
            }
            Json::Objeto(campos)
        })
        .collect();
    Ok(Json::objeto(vec![
        ("ok", Json::Bool(true)),
        ("versao", Json::de_u64(VERSAO_DO_BACKUP)),
        ("database", Json::texto_de(DATABASE)),
        ("tabela", Json::texto_de(TABELA)),
        (
            "idiomas",
            Json::Lista(IDIOMAS.iter().map(|i| Json::texto_de(*i)).collect()),
        ),
        ("linhas", Json::Lista(linhas)),
    ]))
}

/// Devolve o backup para a tabela.
///
/// Grava por `TextName`: o que existe e atualizado, o que falta e incluido.
/// So mexe nas seis colunas de idioma -- um backup adulterado nao consegue
/// inventar coluna nem escrever em `id`.
///
/// Aceita `TextName` que nao esta na fabrica da tela de proposito: e assim que
/// as mensagens do protocolo voltam do mesmo arquivo.
pub fn importar(dados: &Instancia, backup: &Json) -> Result<Relatorio> {
    let versao = backup.inteiro_ou("versao", 0).max(0) as u64;
    if versao == 0 || versao > VERSAO_DO_BACKUP {
        return Err(PhxError::Esquema(format!(
            "backup de versao {versao}: este servidor le ate a {VERSAO_DO_BACKUP}"
        )));
    }
    let Some(Json::Lista(linhas)) = backup.campo("linhas") else {
        return Err(PhxError::Esquema(
            "o backup nao tem a lista \"linhas\"".into(),
        ));
    };

    let mut r = Relatorio::default();
    let (criou_db, criou_tabela) = garantir_tabela(dados)?;
    r.criou_database = criou_db;
    r.criou_tabela = criou_tabela;

    let db = dados.abrir_database(DATABASE)?;
    let mut t = db.abrir_qualificada(TABELA)?;
    let Some(col_nome) = coluna(t.esquema(), "TextName") else {
        return Err(PhxError::Esquema(format!(
            "a tabela {DATABASE}.{TABELA} existe mas nao tem a coluna TextName"
        )));
    };
    let cols: Vec<Option<usize>> = IDIOMAS.iter().map(|n| coluna(t.esquema(), n)).collect();

    let mut onde: HashMap<String, u64> = HashMap::new();
    let total = t.registros();
    if let Ok((rowids, _)) = t.pagina_por_posicao(0, total, Visao::Todas) {
        for rowid in rowids {
            if let Ok(Some(linha)) = t.ler(rowid) {
                if let Some(nome) = linha.get(col_nome).and_then(Value::como_str) {
                    onde.insert(nome.trim().to_string(), rowid);
                }
            }
        }
    }

    let mut vistos = HashSet::new();
    for item in linhas {
        let nome = item.texto_ou("TextName", "").trim().to_string();
        if nome.is_empty() || !vistos.insert(nome.clone()) {
            continue;
        }
        match onde.get(&nome) {
            Some(&rowid) => {
                let Ok(Some(mut linha)) = t.ler(rowid) else {
                    continue;
                };
                let mut mexeu = false;
                for (i, pos) in cols.iter().enumerate() {
                    let Some(p) = pos else { continue };
                    let novo = valor_do_texto(item.texto_ou(IDIOMAS[i], ""));
                    if linha[*p] != novo {
                        linha[*p] = novo;
                        mexeu = true;
                    }
                }
                if mexeu {
                    t.atualizar(rowid, &linha)?;
                    r.alteradas += 1;
                } else {
                    r.intocadas += 1;
                }
            }
            None => {
                let mut objeto = vec![
                    (
                        "id".to_string(),
                        Json::texto_de(phxsql_core::uuid::Uuid::v7().to_string()),
                    ),
                    ("TextName".to_string(), Json::texto_de(&nome)),
                ];
                for idioma in IDIOMAS {
                    let texto = item.texto_ou(idioma, "");
                    if !texto.is_empty() {
                        objeto.push((idioma.to_string(), Json::texto_de(texto)));
                    }
                }
                let linha = json_para_linha(&Json::Objeto(objeto), t.esquema())?;
                t.inserir(&linha)?;
                r.incluidas += 1;
            }
        }
    }
    if r.incluidas > 0 || r.alteradas > 0 {
        t.sincronizar()?;
    }
    Ok(r)
}

/// O estado da tabela, para a tela de administracao mostrar.
pub fn estado(dados: &Instancia, idioma: usize) -> Json {
    let gravadas = ler_gravadas(dados);
    let da_tela = FABRICA_TELA
        .iter()
        .filter(|f| gravadas.contains_key(f.nome))
        .count() as u64;
    // Quantas celulas daquele idioma ja tem traducao propria: e o numero que
    // diz se vale a pena mostrar a bandeira ou se a tela sai em portugues.
    let traduzidas = FABRICA_TELA
        .iter()
        .filter(|f| {
            gravadas
                .get(f.nome)
                .is_some_and(|l| !l[idioma].trim().is_empty())
        })
        .count() as u64;
    Json::objeto(vec![
        ("ok", Json::Bool(true)),
        ("database", Json::texto_de(DATABASE)),
        ("tabela", Json::texto_de(TABELA)),
        ("idioma", Json::texto_de(IDIOMAS[idioma])),
        (
            "idiomas",
            Json::Lista(IDIOMAS.iter().map(|i| Json::texto_de(*i)).collect()),
        ),
        ("linhas_na_tabela", Json::de_u64(gravadas.len() as u64)),
        ("textos_de_tela", Json::de_u64(FABRICA_TELA.len() as u64)),
        ("textos_de_tela_semeados", Json::de_u64(da_tela)),
        ("traduzidos_no_idioma", Json::de_u64(traduzidas)),
    ])
}

/// Os textos ja resolvidos, no formato que a pagina consome.
pub fn textos_para_a_pagina(dados: &Instancia, idioma: usize) -> Json {
    let gravadas = ler_gravadas(dados);
    Json::objeto(vec![
        ("ok", Json::Bool(true)),
        ("idioma", Json::texto_de(IDIOMAS[idioma])),
        (
            "idiomas",
            Json::Lista(IDIOMAS.iter().map(|i| Json::texto_de(*i)).collect()),
        ),
        (
            "textos",
            Json::Objeto(
                resolver_a_tela(&gravadas, idioma)
                    .into_iter()
                    .map(|(n, t)| (n.to_string(), Json::texto_de(&t)))
                    .collect(),
            ),
        ),
    ])
}

/// Os mesmos textos, so que sem consultar a tabela.
///
/// Serve o caso em que nem da para pegar a trava dos dados. A tela ainda tem
/// de abrir: uma trava envenenada nao pode virar um formulario sem rotulo.
pub fn textos_para_a_pagina_sem_tabela(idioma: usize) -> Json {
    let vazio = HashMap::new();
    Json::objeto(vec![
        ("ok", Json::Bool(true)),
        ("idioma", Json::texto_de(IDIOMAS[idioma])),
        (
            "idiomas",
            Json::Lista(IDIOMAS.iter().map(|i| Json::texto_de(*i)).collect()),
        ),
        (
            "textos",
            Json::Objeto(
                resolver_a_tela(&vazio, idioma)
                    .into_iter()
                    .map(|(n, t)| (n.to_string(), Json::texto_de(&t)))
                    .collect(),
            ),
        ),
    ])
}

#[cfg(test)]
mod testes {
    use super::*;

    fn linha(v: [&str; QUANTOS]) -> [String; QUANTOS] {
        v.map(|s| s.to_string())
    }

    #[test]
    fn a_fabrica_e_bem_formada() {
        let mut vistos = HashSet::new();
        for f in FABRICA_TELA {
            assert!(
                e_da_tela(f.nome),
                "{} nao comeca com {PREFIXO_DA_TELA}",
                f.nome
            );
            assert!(vistos.insert(f.nome), "{} aparece duas vezes", f.nome);
            assert!(
                !f.textos[0].is_empty(),
                "{} sem portugues -- o degrau 2 depende dele",
                f.nome
            );
        }
    }

    /// O defeito que este teste tranca: uma celula em branco virando um botao
    /// sem rotulo. Reponha-o devolvendo `linha[idioma]` sem conferir se esta
    /// vazio e este teste falha.
    #[test]
    fn nenhum_degrau_devolve_texto_vazio() {
        for f in FABRICA_TELA {
            for i in 0..QUANTOS {
                // Sem tabela.
                assert!(!resolver_um(None, f, i).is_empty(), "{} sem tabela", f.nome);
                // Com a linha inteira em branco: o pior caso real, que e
                // alguem apagar as celulas pela grade.
                let vazia = linha([""; QUANTOS]);
                assert!(
                    !resolver_um(Some(&vazia), f, i).is_empty(),
                    "{} com a linha em branco",
                    f.nome
                );
                // Com so o idioma pedido em branco: cai no portugues gravado.
                // O portugues nao entra aqui porque ele E o degrau de baixo --
                // apagado, o que resta e a fabrica, e a linha acima ja prova.
                if i != 0 {
                    let mut so_o_idioma = linha(["gravado"; QUANTOS]);
                    so_o_idioma[i] = String::new();
                    assert_eq!(
                        resolver_um(Some(&so_o_idioma), f, i),
                        "gravado",
                        "{} devia cair no portugues gravado",
                        f.nome
                    );
                }
            }
        }
    }

    #[test]
    fn resolve_degrau_a_degrau() {
        let f = fabrica("tela.entrar").expect("tela.entrar existe na fabrica");
        let frances = indice_do_idioma("Frances");

        // Degrau 1: a celula do idioma manda.
        let mut l = linha([""; QUANTOS]);
        l[0] = "Entrar gravado".into();
        l[frances] = "Entrer gravado".into();
        assert_eq!(resolver_um(Some(&l), f, frances), "Entrer gravado");

        // Degrau 2: celula vazia cai no portugues GRAVADO, e nao na fabrica --
        // quem traduziu o portugues quer o dele.
        let mut l2 = linha([""; QUANTOS]);
        l2[0] = "Entrar gravado".into();
        assert_eq!(resolver_um(Some(&l2), f, frances), "Entrar gravado");

        // Degrau 3: sem linha nenhuma, a fabrica.
        assert_eq!(resolver_um(None, f, frances), "Entrer");
    }

    #[test]
    fn idioma_desconhecido_cai_em_portugues() {
        assert_eq!(indice_do_idioma("Klingon"), 0);
        assert_eq!(indice_do_idioma(""), 0);
        assert_eq!(indice_do_idioma("Portugues"), 0);
        assert_eq!(indice_do_idioma("Alemao"), 4);
    }

    #[test]
    fn a_tela_inteira_resolve_sem_tabela() {
        let vazio = HashMap::new();
        for (i, idioma) in IDIOMAS.iter().enumerate() {
            let textos = resolver_a_tela(&vazio, i);
            assert_eq!(textos.len(), FABRICA_TELA.len());
            for (nome, texto) in textos {
                assert!(!texto.is_empty(), "{nome} vazio no idioma {idioma}");
            }
        }
    }

    /// O laco entre a fabrica e a pagina. Sem ele, `data-txt` escrito errado
    /// no HTML fica em portugues para sempre e ninguem percebe -- que e a
    /// mesma armadilha do catalogo contra o despachar.
    #[test]
    fn todo_data_txt_da_pagina_existe_na_fabrica() {
        let pagina = include_str!("../ui/index.html");
        let mut usados = HashSet::new();
        for pedaco in pagina.split("data-txt=\"").skip(1) {
            let Some(nome) = pedaco.split('"').next() else {
                continue;
            };
            assert!(
                fabrica(nome).is_some(),
                "a pagina usa data-txt={nome:?}, que nao existe na FABRICA_TELA"
            );
            usados.insert(nome.to_string());
        }
        assert!(
            !usados.is_empty(),
            "nenhum data-txt na pagina: o laco se soltou"
        );
    }
}

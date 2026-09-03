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

// As tres constantes moram no `mensagens`, e este modulo as REUSA em vez de
// repetir. Os dois conjuntos de texto (`erro.` do protocolo e `tela.` da
// interface) dividem a MESMA tabela, entao duas listas de idioma seriam duas
// verdades sobre a mesma coisa: quem mudasse uma so deixaria a outra errada
// em silencio, e o esquema da tabela sairia com colunas que um dos lados nao
// enxerga.
pub use crate::mensagens::{DATABASE, IDIOMAS, TABELA};

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

/// Uma linha da fabrica, na ordem das colunas.
///
/// Existe para que acrescentar um texto custe UMA LINHA. Enquanto a lista
/// tinha trinta itens a forma longa cabia; com duzentos ela cobraria mil e
/// duzentas linhas de cerimonia, e o preco de obedecer a regra do idioma
/// passaria a ser o argumento para nao obedece-la.
macro_rules! texto {
    ($nome:literal, $pt:literal, $fr:literal, $en:literal, $it:literal, $de:literal, $es:literal) => {
        TextoDeFabrica {
            nome: $nome,
            textos: [$pt, $fr, $en, $it, $de, $es],
        }
    };
}

/// Todo texto que a tela mostra, nos seis idiomas.
///
/// Ordem das colunas: Portugues, Frances, Ingles, Italiano, Alemao, Espanhol.
pub const FABRICA_TELA: &[TextoDeFabrica] = &[
    // ------------------------------------------------------- a moldura
    texto!("tela.assinatura", "Centro de Controle", "Centre de Contrôle", "Control Center", "Centro di Controllo", "Kontrollzentrum", "Centro de Control"),
    // ------------------------------------------------------- os campos
    texto!("tela.servidor", "Servidor", "Serveur", "Server", "Server", "Server", "Servidor"),
    texto!("tela.servidor_dica", "IP ou DNS", "IP ou DNS", "IP or DNS", "IP o DNS", "IP oder DNS", "IP o DNS"),
    texto!("tela.porta", "Porta", "Port", "Port", "Porta", "Port", "Puerto"),
    texto!("tela.usuario", "Usuário", "Utilisateur", "User", "Utente", "Benutzer", "Usuario"),
    texto!("tela.senha", "Senha", "Mot de passe", "Password", "Password", "Kennwort", "Contraseña"),
    texto!("tela.token", "Token do servidor", "Jeton du serveur", "Server token", "Token del server", "Server-Token", "Token del servidor"),
    texto!("tela.chave", "Chave privada", "Clé privée", "Private key", "Chiave privata", "Privater Schlüssel", "Clave privada"),
    texto!("tela.facultativa", "facultativa", "facultative", "optional", "facoltativa", "optional", "facultativa"),
    texto!("tela.chave_dica", "Ed25519, 64 hexadecimais", "Ed25519, 64 hexadécimaux", "Ed25519, 64 hex digits", "Ed25519, 64 esadecimali", "Ed25519, 64 Hexadezimalstellen", "Ed25519, 64 hexadecimales"),
    texto!("tela.database", "Database", "Database", "Database", "Database", "Datenbank", "Database"),
    texto!("tela.opcional", "opcional", "optionnel", "optional", "opzionale", "optional", "opcional"),
    texto!("tela.database_dica", "abre já neste banco", "ouvre directement cette base", "opens straight into this database", "apre subito in questo database", "öffnet direkt diese Datenbank", "abre ya en esta base"),
    texto!("tela.entrar", "Entrar", "Entrer", "Sign in", "Entra", "Anmelden", "Entrar"),
    // ---------------------------------------------------- o que o login diz
    texto!("tela.conferindo", "Conferindo…", "Vérification…", "Checking…", "Verifica…", "Prüfung…", "Comprobando…"),
    texto!("tela.derivando", "Derivando a prova…", "Calcul de la preuve…", "Deriving the proof…", "Derivazione della prova…", "Nachweis wird abgeleitet…", "Derivando la prueba…"),
    texto!("tela.assinando", "Assinando o desafio…", "Signature du défi…", "Signing the challenge…", "Firma della sfida…", "Challenge wird signiert…", "Firmando el desafío…"),
    texto!("tela.falhou", "não consegui entrar", "connexion impossible", "could not sign in", "accesso non riuscito", "Anmeldung fehlgeschlagen", "no fue posible entrar"),
    // ------------------------------------------------------- o idioma
    texto!("tela.idioma_da_interface", "Idioma da interface", "Langue de l'interface", "Interface language", "Lingua dell'interfaccia", "Sprache der Oberfläche", "Idioma de la interfaz"),
    texto!("tela.idioma_dica", "A escolha vale nesta sessão e fica guardada neste navegador.", "Le choix vaut pour cette session et reste dans ce navigateur.", "The choice applies to this session and is kept in this browser.", "La scelta vale per questa sessione e resta in questo browser.", "Die Wahl gilt für diese Sitzung und bleibt in diesem Browser.", "La elección vale para esta sesión y queda en este navegador."),
    // -------------------------------------------------- o histórico
    texto!("tela.conexoes", "Conexões salvas", "Connexions enregistrées", "Saved connections", "Connessioni salvate", "Gespeicherte Verbindungen", "Conexiones guardadas"),
    texto!("tela.conexoes_vazio", "Nenhuma conexão guardada ainda. Preencha os campos acima e clique em guardar.", "Aucune connexion enregistrée. Remplissez les champs ci-dessus et enregistrez.", "No saved connections yet. Fill in the fields above and save.", "Nessuna connessione salvata. Compila i campi sopra e salva.", "Noch keine Verbindung gespeichert. Felder oben ausfüllen und speichern.", "Ninguna conexión guardada. Rellene los campos de arriba y guarde."),
    texto!("tela.conexoes_aviso", "Ficam neste navegador, nunca no servidor. A senha e o token NÃO são guardados.", "Restent dans ce navigateur, jamais sur le serveur. Le mot de passe et le jeton ne sont PAS enregistrés.", "Kept in this browser, never on the server. The password and token are NOT stored.", "Restano in questo browser, mai sul server. La password e il token NON vengono salvati.", "Bleiben in diesem Browser, nie auf dem Server. Kennwort und Token werden NICHT gespeichert.", "Quedan en este navegador, nunca en el servidor. La contraseña y el token NO se guardan."),
    texto!("tela.guardar_conexao", "Guardar esta conexão", "Enregistrer cette connexion", "Save this connection", "Salva questa connessione", "Diese Verbindung speichern", "Guardar esta conexión"),
    texto!("tela.renomear", "Renomear", "Renommer", "Rename", "Rinomina", "Umbenennen", "Renombrar"),
    texto!("tela.remover", "Remover", "Supprimer", "Remove", "Rimuovi", "Entfernen", "Quitar"),
    texto!("tela.pergunta_apelido", "Que nome dar a esta conexão? (por exemplo: base da farmácia)", "Quel nom donner à cette connexion ? (par exemple : base de la pharmacie)", "What name should this connection have? (for example: pharmacy database)", "Che nome dare a questa connessione? (per esempio: base della farmacia)", "Welchen Namen soll diese Verbindung haben? (zum Beispiel: Apotheken-Datenbank)", "¿Qué nombre dar a esta conexión? (por ejemplo: base de la farmacia)"),
    texto!("tela.confirmar_remover", "Remover esta conexão da lista deste navegador?", "Retirer cette connexion de la liste de ce navigateur ?", "Remove this connection from this browser's list?", "Rimuovere questa connessione dall'elenco di questo browser?", "Diese Verbindung aus der Liste dieses Browsers entfernen?", "¿Quitar esta conexión de la lista de este navegador?"),
    texto!("tela.nunca_usada", "nunca", "jamais", "never", "mai", "nie", "nunca"),

    // ================================================ o cromo, depois de entrar
    // A moldura que nunca sai da tela: barra do alto, barra de menu, barra de
    // ferramentas e o painel lateral. E o que a pessoa mais ve, entao e o que
    // entrou primeiro.
    texto!("tela.demo", "modo demonstração", "mode démonstration", "demo mode", "modalità dimostrativa", "Demo-Modus", "modo demostración"),
    texto!("tela.tema_para_claro", "Mudar para o tema claro", "Passer au thème clair", "Switch to the light theme", "Passa al tema chiaro", "Zum hellen Design wechseln", "Cambiar al tema claro"),
    texto!("tela.tema_para_escuro", "Mudar para o tema escuro", "Passer au thème sombre", "Switch to the dark theme", "Passa al tema scuro", "Zum dunklen Design wechseln", "Cambiar al tema oscuro"),
    texto!("tela.tema_dica", "Alternar tema claro e escuro", "Basculer entre thème clair et sombre", "Switch between light and dark theme", "Alterna tema chiaro e scuro", "Zwischen hellem und dunklem Design wechseln", "Alternar entre tema claro y oscuro"),
    texto!("tela.sair", "Sair", "Quitter", "Sign out", "Esci", "Abmelden", "Salir"),
    texto!("tela.menu_principal", "Menu principal", "Menu principal", "Main menu", "Menu principale", "Hauptmenü", "Menú principal"),
    texto!("tela.barra_ferramentas", "Barra de ferramentas", "Barre d'outils", "Toolbar", "Barra degli strumenti", "Werkzeugleiste", "Barra de herramientas"),
    texto!("tela.navegacao", "Navegação", "Navigation", "Navigation", "Navigazione", "Navigation", "Navegación"),
    texto!("tela.bancos_e_tabelas", "Bancos e tabelas", "Bases et tables", "Databases and tables", "Basi e tabelle", "Datenbanken und Tabellen", "Bases y tablas"),
    texto!("tela.lateral_nota", "Recolhido, pinado e largura ficam", "Réduit, épinglé et largeur restent", "Collapsed, pinned and width stay", "Ridotto, fissato e larghezza restano", "Eingeklappt, angeheftet und Breite bleiben", "Plegado, fijado y ancho quedan"),
    texto!("tela.neste_navegador", "neste navegador", "dans ce navigateur", "in this browser", "in questo browser", "in diesem Browser", "en este navegador"),
    texto!("tela.lateral_nota_fim", "— não no servidor.", "— pas sur le serveur.", "— not on the server.", "— non sul server.", "— nicht auf dem Server.", "— no en el servidor."),
    texto!("tela.largura_lateral", "Largura do painel lateral", "Largeur du panneau latéral", "Side panel width", "Larghezza del pannello laterale", "Breite der Seitenleiste", "Ancho del panel lateral"),

    // ------------------------------------------------------- as cinco abas
    texto!("tela.aba_estrutura", "Estrutura", "Structure", "Structure", "Struttura", "Struktur", "Estructura"),
    texto!("tela.aba_conteudo", "Conteúdo", "Contenu", "Content", "Contenuto", "Inhalt", "Contenido"),
    texto!("tela.aba_indices", "Índices", "Index", "Indexes", "Indici", "Indizes", "Índices"),
    texto!("tela.aba_diario", "Diário", "Journal", "Journal", "Diario", "Journal", "Diario"),
    texto!("tela.aba_integridade", "Integridade", "Intégrité", "Integrity", "Integrità", "Integrität", "Integridad"),

    // ------------------------------------------------------------- a arvore
    texto!("tela.painel", "Painel", "Tableau de bord", "Dashboard", "Pannello", "Übersicht", "Panel"),
    // A diferenca entre declarar e impor, na folha do esquema. Duas
    // tabelas com a mesma chave apareceriam identicas sem isto.
    texto!("tela.col_ordem", "nº", "n°", "no.", "n.", "Nr.", "n.º"),
    texto!("tela.bt_restaurar", "restaurar", "restaurer", "restore", "ripristina", "wiederherstellen", "restaurar"),
    texto!("tela.bt_editar", "editar", "modifier", "edit", "modifica", "bearbeiten", "editar"),
    // Cabecalhos da SysTables -- a primeira tela da padronizacao «toda tabela
    // e PhxGrid». Entram pela fabrica porque titulo de coluna e ROTULO, e
    // rotulo se traduz; o valor da celula e dado, e dado nunca.
    texto!("tela.col_tabela", "tabela", "table", "table", "tabella", "Tabelle", "tabla"),
    texto!("tela.col_schema", "schema", "schéma", "schema", "schema", "Schema", "esquema"),
    texto!("tela.col_registros", "registros", "enregistrements", "records", "record", "Datensätze", "registros"),
    texto!("tela.col_slots", "slots", "emplacements", "slots", "slot", "Slots", "ranuras"),
    texto!("tela.col_colunas", "colunas", "colonnes", "columns", "colonne", "Spalten", "columnas"),
    texto!("tela.col_indices", "índices", "index", "indexes", "indici", "Indizes", "índices"),
    texto!("tela.col_chave_primaria", "chave primária", "clé primaire", "primary key", "chiave primaria", "Primärschlüssel", "clave primaria"),
    texto!("tela.col_fks", "FKs", "CLÉ", "FKs", "FK", "FKs", "FKs"),
    texto!("tela.col_bytes_linha", "bytes/linha", "octets/ligne", "bytes/row", "byte/riga", "Bytes/Zeile", "bytes/fila"),
    texto!("tela.col_particao", "partição", "partition", "partition", "partizione", "Partition", "partición"),
    texto!("tela.col_volumes", "volumes", "volumes", "volumes", "volumi", "Volumes", "volúmenes"),
    texto!("tela.fk_conferida", "conferida", "vérifiée", "checked", "verificata", "geprüft", "verificada"),
    texto!("tela.fk_imposta", "imposta", "imposée", "enforced", "imposta", "erzwungen", "impuesta"),
    texto!("tela.fk_so_declarada", "só declarada", "seulement déclarée", "declared only", "solo dichiarata", "nur deklariert", "solo declarada"),
    texto!("tela.fk_nota_conferida", "**Conferidas na gravação.** Chave declarada **nasce** conferida, e a coluna diz por chave: a imposta recusa filha sem mãe, mãe que ainda tem filha, e alteração que quebre a ligação.", "**Vérifiées à l'écriture.** Une clé déclarée **naît** vérifiée, et la colonne répond clé par clé : l'imposée refuse l'enfant sans parent, le parent qui a des enfants, et ce qui romprait le lien.", "**Enforced on write.** A declared key is **born** enforced, and the column answers per key: an enforced one rejects a child without a parent, a parent that still has children, and anything that would break the link.", "**Verificate in scrittura.** Una chiave dichiarata **nasce** verificata, e la colonna risponde per chiave: l'imposta rifiuta il figlio senza madre, la madre con figli, e ciò che romperebbe il legame.", "**Beim Schreiben geprüft.** Ein deklarierter Schlüssel wird **geprüft** geboren, und die Spalte antwortet je Schlüssel: Der erzwungene weist Kind ohne Mutter, Mutter mit Kindern und alles Verbindungsbrechende ab.", "**Verificadas al grabar.** Una clave declarada **nace** verificada, y la columna responde por clave: la impuesta rechaza la hija sin madre, la madre con hijas, y lo que rompería el vínculo."),
    texto!("tela.fk_declarada_ok", "chave {nome} declarada em {tabela} — e já conferida na gravação", "clé {nome} déclarée dans {tabela} — et déjà vérifiée à l'écriture", "key {nome} declared on {tabela} — and already enforced on write", "chiave {nome} dichiarata in {tabela} — e già verificata in scrittura", "Schlüssel {nome} in {tabela} deklariert — und beim Schreiben bereits geprüft", "clave {nome} declarada en {tabela} — y ya verificada al grabar"),
    texto!("tela.bancos_de_dados", "Bancos de dados", "Bases de données", "Databases", "Basi di dati", "Datenbanken", "Bases de datos"),
    texto!("tela.novo_database", "Novo database", "Nouvelle base", "New database", "Nuovo database", "Neue Datenbank", "Nueva base"),
    texto!("tela.adicionar_database", "Adicionar um database", "Ajouter une base", "Add a database", "Aggiungi un database", "Eine Datenbank hinzufügen", "Añadir una base"),
    texto!("tela.sem_tabelas", "sem tabelas", "aucune table", "no tables", "nessuna tabella", "keine Tabellen", "sin tablas"),
    texto!("tela.administracao", "Administração", "Administration", "Administration", "Amministrazione", "Verwaltung", "Administración"),
    texto!("tela.usuarios", "Usuários", "Utilisateurs", "Users", "Utenti", "Benutzer", "Usuarios"),
    texto!("tela.acessos", "Acessos", "Accès", "Access log", "Accessi", "Zugriffe", "Accesos"),
    texto!("tela.bloqueios", "Bloqueios", "Blocages", "Blocks", "Blocchi", "Sperren", "Bloqueos"),
    texto!("tela.idiomas", "Idiomas", "Langues", "Languages", "Lingue", "Sprachen", "Idiomas"),

    // ------------------------------------------------- o que toda tela repete
    texto!("tela.carregando", "carregando…", "chargement…", "loading…", "caricamento…", "wird geladen…", "cargando…"),
    texto!("tela.voltar", "← Voltar", "← Retour", "← Back", "← Indietro", "← Zurück", "← Volver"),
    texto!("tela.cancelar", "Cancelar", "Annuler", "Cancel", "Annulla", "Abbrechen", "Cancelar"),
    texto!("tela.fechar", "Fechar", "Fermer", "Close", "Chiudi", "Schließen", "Cerrar"),
    texto!("tela.salvar", "Salvar", "Enregistrer", "Save", "Salva", "Speichern", "Guardar"),
    texto!("tela.incluir", "Incluir", "Ajouter", "Add", "Inserisci", "Hinzufügen", "Añadir"),

    // ------------------------------------- acrescentar coluna (sprint 25 / #127)
    texto!("tela.acrescentar", "Acrescentar", "Ajouter", "Add", "Aggiungi", "Hinzufügen", "Añadir"),
    texto!("tela.acrescentar_coluna", "✚ Acrescentar coluna…", "✚ Ajouter une colonne…", "✚ Add column…", "✚ Aggiungi colonna…", "✚ Spalte hinzufügen…", "✚ Añadir columna…"),
    texto!("tela.acrescentar_coluna_em", "Acrescentar coluna em", "Ajouter une colonne à", "Add column to", "Aggiungi colonna a", "Spalte hinzufügen zu", "Añadir columna a"),
    texto!("tela.registros", "registro(s)", "enregistrement(s)", "record(s)", "record", "Datensätze", "registro(s)"),
    texto!("tela.col_nome", "nome da coluna", "nom de la colonne", "column name", "nome della colonna", "Spaltenname", "nombre de la columna"),
    texto!("tela.col_tipo", "tipo", "type", "type", "tipo", "Typ", "tipo"),
    texto!("tela.col_caption", "rótulo de tela", "libellé d'écran", "screen label", "etichetta a schermo", "Bildschirmbezeichnung", "rótulo de pantalla"),
    texto!("tela.col_padrao", "valor das linhas que já existem", "valeur des lignes existantes", "value for the rows that already exist", "valore delle righe esistenti", "Wert für die vorhandenen Zeilen", "valor de las filas que ya existen"),
    texto!("tela.col_padrao_vazio", "vazio = nulo", "vide = nul", "empty = null", "vuoto = nullo", "leer = null", "vacío = nulo"),
    // Estrutura de tabela externa (DbLink): os cabecalhos e os pinos da grade.
    // O `col_padrao` que ja existia e da tela de acrescentar coluna ("valor
    // das linhas que ja existem") e quer dizer outra coisa -- por isso aqui e
    // `col_valor_padrao`, e nao um reaproveitamento que mentiria na traducao.
    texto!("tela.col_nulo", "nulo", "nul", "null", "nullo", "Null", "nulo"),
    texto!("tela.col_valor_padrao", "padrão", "défaut", "default", "predefinito", "Standard", "predeterminado"),
    texto!("tela.col_extra", "extra", "extra", "extra", "extra", "Extra", "extra"),
    texto!("tela.col_comentario", "comentário", "commentaire", "comment", "commento", "Kommentar", "comentario"),
    texto!("tela.col_cardinalidade", "cardinalidade", "cardinalité", "cardinality", "cardinalità", "Kardinalität", "cardinalidad"),
    texto!("tela.pino_nulo_ok", "nulo ok", "nul ok", "null ok", "nullo ok", "Null ok", "nulo ok"),
    texto!("tela.pino_obrigatorio", "obrigatório", "obligatoire", "required", "obbligatorio", "Pflicht", "obligatorio"),
    texto!("tela.pino_unico", "único", "unique", "unique", "unico", "eindeutig", "único"),
    texto!("tela.pino_duplicado_ok", "duplicado ok", "doublon ok", "duplicates ok", "duplicato ok", "Duplikate ok", "duplicado ok"),
    texto!("tela.col_obrigatoria", "obrigatória (não aceita nulo)", "obligatoire (n'accepte pas nul)", "required (no nulls)", "obbligatoria (non accetta nullo)", "pflicht (kein null)", "obligatoria (no acepta nulo)"),
    texto!("tela.slots_reescritos", "slot(s) reescrito(s)", "emplacement(s) réécrit(s)", "slot(s) rewritten", "slot riscritti", "neu geschriebene Slots", "slot(s) reescrito(s)"),
    // O preco e a parte que ninguem pode descobrir depois de clicar.
    texto!("tela.alter_preco", "Acrescentar coluna reescreve o arquivo de dados inteiro, linha por linha, na mesma ordem. O rowid de cada linha NÃO muda — e por isso os índices não precisam ser refeitos. Medido: 0,55 µs por linha (10 milhões de linhas em 5,5 s).", "", "Adding a column rewrites the whole data file, row by row, in the same order. Each row keeps its rowid — which is why the indexes are not rebuilt. Measured: 0.55 µs per row (10 million rows in 5.5 s).", "", "", ""),
    texto!("tela.alter_obrigatoria", "Coluna obrigatória numa tabela que já tem linha exige um valor padrão: sem ele, o motor teria de inventar um dado para linhas que ninguém digitou — e ele recusa em vez de inventar.", "", "A required column on a table that already has rows needs a default: without one the engine would have to invent data for rows nobody typed — and it refuses instead of inventing.", "", "", ""),
    texto!("tela.alter_pode", "Acrescentar coluna funciona nesta tabela, mesmo com dado dentro: o rowid de cada linha não muda, e por isso os índices não são refeitos.", "", "Adding a column works on this table even with data in it: each row keeps its rowid, so the indexes are not rebuilt.", "", "", ""),
    texto!("tela.alter_nao_pode", "Trocar o tipo ou a largura de uma coluna que já existe continua não existindo — o caminho é duplicar a tabela e reimportar.", "", "Changing the type or the width of an existing column still does not exist — the way through is to duplicate the table and reimport.", "", "", ""),

    texto!("tela.sim", "sim", "oui", "yes", "sì", "ja", "sí"),
    texto!("tela.nao", "não", "non", "no", "no", "nein", "no"),
    texto!("tela.ligado", "ligado", "activé", "on", "attivo", "ein", "activado"),
    texto!("tela.desligado", "desligado", "désactivé", "off", "spento", "aus", "desactivado"),

    // ------------------------------------------------- os titulos da barra de menu
    texto!("tela.menu_arquivo", "Arquivo", "Fichier", "File", "File", "Datei", "Archivo"),
    texto!("tela.menu_banco", "Banco", "Base", "Database", "Base", "Datenbank", "Base"),
    texto!("tela.menu_tabelas", "Tabelas", "Tables", "Tables", "Tabelle", "Tabellen", "Tablas"),
    texto!("tela.menu_memoria", "Memória", "Mémoire", "Memory", "Memoria", "Speicher", "Memoria"),
    texto!("tela.menu_ferramentas", "Ferramentas", "Outils", "Tools", "Strumenti", "Werkzeuge", "Herramientas"),
    texto!("tela.menu_configuracoes", "Configurações", "Configuration", "Settings", "Impostazioni", "Einstellungen", "Configuración"),
    texto!("tela.menu_ver", "Ver", "Affichage", "View", "Vista", "Ansicht", "Ver"),
    texto!("tela.menu_ajuda", "Ajuda", "Aide", "Help", "Aiuto", "Hilfe", "Ayuda"),

    // ------------------------------------------------------ os itens do menu
    texto!("tela.mi_novo_database", "Novo database…", "Nouvelle base…", "New database…", "Nuovo database…", "Neue Datenbank…", "Nueva base…"),
    texto!("tela.mi_view_database", "View Database", "Voir la base", "View Database", "Vedi database", "Datenbank ansehen", "Ver la base"),
    texto!("tela.mi_backup_agora", "Backup agora…", "Sauvegarde immédiate…", "Back up now…", "Backup adesso…", "Jetzt sichern…", "Copia ahora…"),
    texto!("tela.mi_conferir_backup", "Conferir um backup…", "Vérifier une sauvegarde…", "Check a backup…", "Verifica un backup…", "Ein Backup prüfen…", "Comprobar una copia…"),
    texto!("tela.mi_restaurar_backup", "Restaurar um backup…", "Restaurer une sauvegarde…", "Restore a backup…", "Ripristina un backup…", "Ein Backup zurückspielen…", "Restaurar una copia…"),
    texto!("tela.mi_gerir_banco", "Gerir este banco", "Gérer cette base", "Manage this database", "Gestisci questa base", "Diese Datenbank verwalten", "Gestionar esta base"),
    texto!("tela.mi_pivot", "Tabela dinâmica…", "Tableau croisé…", "Pivot table…", "Tabella pivot…", "Pivot-Tabelle…", "Tabla dinámica…"),
    texto!("tela.mi_juncao", "Junção de tabelas…", "Jointure de tables…", "Table join…", "Join di tabelle…", "Tabellen-Join…", "Unión de tablas…"),
    texto!("tela.mi_uniao", "União de tabelas…", "Union de tables…", "Table union…", "Unione di tabelle…", "Tabellen-Union…", "Unión (UNION) de tablas…"),
    texto!("tela.mi_sequencias", "Sequências", "Séquences", "Sequences", "Sequenze", "Sequenzen", "Secuencias"),
    texto!("tela.mi_diagrama_er", "Diagrama ER…", "Diagramme ER…", "ER diagram…", "Diagramma ER…", "ER-Diagramm…", "Diagrama ER…"),
    texto!("tela.mi_lgpd", "Dado pessoal (LGPD)…", "Données personnelles (RGPD)…", "Personal data (GDPR)…", "Dati personali (GDPR)…", "Personenbezogene Daten (DSGVO)…", "Datos personales (RGPD)…"),
    texto!("tela.mi_copiar_colar", "Copiar / colar tabela…", "Copier / coller une table…", "Copy / paste table…", "Copia / incolla tabella…", "Tabelle kopieren / einfügen…", "Copiar / pegar tabla…"),
    texto!("tela.mi_backup_restauracao", "Backup e restauração", "Sauvegarde et restauration", "Backup and restore", "Backup e ripristino", "Sicherung und Wiederherstellung", "Copia y restauración"),
    texto!("tela.mi_arquivos_bloqueados", "Arquivos bloqueados", "Fichiers bloqués", "Locked files", "File bloccati", "Gesperrte Dateien", "Archivos bloqueados"),
    texto!("tela.mi_transacoes", "Transações", "Transactions", "Transactions", "Transazioni", "Transaktionen", "Transacciones"),
    texto!("tela.mi_gerir_tabelas", "Gerir as tabelas", "Gérer les tables", "Manage tables", "Gestisci le tabelle", "Tabellen verwalten", "Gestionar las tablas"),
    texto!("tela.mi_nova_tabela", "Nova tabela…", "Nouvelle table…", "New table…", "Nuova tabella…", "Neue Tabelle…", "Nueva tabla…"),
    texto!("tela.mi_estrutura_tabela", "Estrutura da tabela", "Structure de la table", "Table structure", "Struttura della tabella", "Tabellenstruktur", "Estructura de la tabla"),
    texto!("tela.mi_editar_conteudo", "Editar conteúdo", "Modifier le contenu", "Edit content", "Modifica contenuto", "Inhalt bearbeiten", "Editar contenido"),
    texto!("tela.mi_particoes", "Partições da tabela", "Partitions de la table", "Table partitions", "Partizioni della tabella", "Tabellenpartitionen", "Particiones de la tabla"),
    texto!("tela.mi_config_diretivas", "Configurações e diretivas", "Réglages et directives", "Settings and directives", "Impostazioni e direttive", "Einstellungen und Direktiven", "Ajustes y directivas"),
    texto!("tela.mi_lixeira", "Lixeira da tabela", "Corbeille de la table", "Table recycle bin", "Cestino della tabella", "Papierkorb der Tabelle", "Papelera de la tabla"),
    texto!("tela.mi_motivos", "Motivos das exclusões", "Motifs des suppressions", "Reasons for deletions", "Motivi delle eliminazioni", "Gründe der Löschungen", "Motivos de las eliminaciones"),
    texto!("tela.mi_duplicar_tabela", "Duplicar tabela…", "Dupliquer la table…", "Duplicate table…", "Duplica tabella…", "Tabelle duplizieren…", "Duplicar tabla…"),
    texto!("tela.mi_verificar", "Verificar", "Vérifier", "Verify", "Verifica", "Prüfen", "Verificar"),
    texto!("tela.mi_checksum", "Soma de verificação", "Somme de contrôle", "Checksum", "Somma di controllo", "Prüfsumme", "Suma de verificación"),
    texto!("tela.mi_exportar", "Exportar…", "Exporter…", "Export…", "Esporta…", "Exportieren…", "Exportar…"),
    texto!("tela.mi_importar", "Importar carga…", "Importer un lot…", "Import a load…", "Importa un carico…", "Ladung importieren…", "Importar carga…"),
    texto!("tela.mi_reparar_indice", "Reparar índice…", "Réparer l'index…", "Repair index…", "Ripara l'indice…", "Index reparieren…", "Reparar índice…"),
    texto!("tela.mi_reparar_espelho", "Reparar tabela pelo espelho…", "Réparer la table par le miroir…", "Repair table from the mirror…", "Ripara la tabella dallo specchio…", "Tabelle aus dem Spiegel reparieren…", "Reparar tabla por el espejo…"),
    texto!("tela.mi_excluir_tabela", "Excluir tabela…", "Supprimer la table…", "Delete table…", "Elimina tabella…", "Tabelle löschen…", "Eliminar tabla…"),
    texto!("tela.mi_carregar_ram", "Carregar esta tabela na RAM", "Charger cette table en RAM", "Load this table into RAM", "Carica questa tabella in RAM", "Diese Tabelle in den RAM laden", "Cargar esta tabla en RAM"),
    texto!("tela.mi_residentes", "Tabelas residentes", "Tables résidentes", "Resident tables", "Tabelle residenti", "Residente Tabellen", "Tablas residentes"),
    texto!("tela.mi_liberar", "Liberar esta tabela", "Libérer cette table", "Release this table", "Libera questa tabella", "Diese Tabelle freigeben", "Liberar esta tabla"),
    texto!("tela.mi_jobs", "Jobs de execução…", "Tâches planifiées…", "Scheduled jobs…", "Job di esecuzione…", "Ausführungs-Jobs…", "Trabajos de ejecución…"),
    texto!("tela.mi_sessoes_agora", "Sessões agora", "Sessions en cours", "Sessions right now", "Sessioni adesso", "Sitzungen jetzt", "Sesiones ahora"),
    texto!("tela.mi_estatisticas", "Estatísticas de uso", "Statistiques d'usage", "Usage statistics", "Statistiche d'uso", "Nutzungsstatistik", "Estadísticas de uso"),
    texto!("tela.mi_estatisticas_p", "Estatísticas de uso…", "Statistiques d'usage…", "Usage statistics…", "Statistiche d'uso…", "Nutzungsstatistik…", "Estadísticas de uso…"),
    texto!("tela.mi_de_onde_vem", "De onde vêm", "D'où viennent-ils", "Where they come from", "Da dove vengono", "Woher sie kommen", "De dónde vienen"),
    texto!("tela.mi_configuracao", "Configuração", "Configuration", "Configuration", "Configurazione", "Konfiguration", "Configuración"),
    texto!("tela.mi_quem_sou", "Quem sou eu", "Qui suis-je", "Who am I", "Chi sono", "Wer bin ich", "Quién soy"),
    texto!("tela.mi_gestao_transacoes", "Gestão de transações", "Gestion des transactions", "Transaction management", "Gestione delle transazioni", "Transaktionsverwaltung", "Gestión de transacciones"),
    // ------------------------------- a tela de gestao de transacoes
    texto!("tela.tx_sub", "o que existe, quem segura o quê, e o que não existe", "ce qui existe, qui retient quoi, et ce qui n'existe pas", "what exists, who holds what, and what does not exist", "cosa esiste, chi trattiene cosa, e cosa non esiste", "was es gibt, wer was hält, und was es nicht gibt", "qué existe, quién retiene qué, y qué no existe"),
    texto!("tela.tx_abertas", "transações abertas", "transactions ouvertes", "open transactions", "transazioni aperte", "offene Transaktionen", "transacciones abiertas"),
    texto!("tela.tx_agora", "neste servidor, agora", "sur ce serveur, maintenant", "on this server, right now", "su questo server, adesso", "auf diesem Server, jetzt", "en este servidor, ahora"),
    texto!("tela.tx_esta_sessao", "esta sessão", "cette session", "this session", "questa sessione", "diese Sitzung", "esta sesión"),
    texto!("tela.tx_pela_web", "a web não abre transação", "le web n'ouvre pas de transaction", "the web does not open transactions", "il web non apre transazioni", "das Web öffnet keine Transaktion", "la web no abre transacción"),
    texto!("tela.tx_lock_padrao", "modo de trava padrão", "mode de verrou par défaut", "default lock mode", "modalità di blocco predefinita", "Standard-Sperrmodus", "modo de bloqueo por omisión"),
    texto!("tela.tx_lock_padrao_u", "intenção na tabela, exclusiva na linha", "intention sur la table, exclusive sur la ligne", "intent on the table, exclusive on the row", "intenzione sulla tabella, esclusiva sulla riga", "Absicht auf der Tabelle, exklusiv auf der Zeile", "intención en la tabla, exclusiva en la fila"),
    texto!("tela.tx_escopo_padrao", "modo de escopo padrão", "mode de portée par défaut", "default scope mode", "modalità di ambito predefinita", "Standard-Geltungsbereich", "modo de alcance por omisión"),
    texto!("tela.tx_escopo_padrao_u", "STRICT se pede", "STRICT se demande", "STRICT is asked for", "STRICT si chiede", "STRICT wird angefordert", "STRICT se pide"),
    texto!("tela.tx_abertas_titulo", "Transações abertas", "Transactions ouvertes", "Open transactions", "Transazioni aperte", "Offene Transaktionen", "Transacciones abiertas"),
    texto!("tela.tx_vazia", "Nenhuma transação aberta agora.", "Aucune transaction ouverte pour le moment.", "No open transactions right now.", "Nessuna transazione aperta al momento.", "Zurzeit keine offene Transaktion.", "Ninguna transacción abierta ahora."),
    texto!("tela.tx_estados_titulo", "A máquina de estados", "La machine à états", "The state machine", "La macchina a stati", "Der Zustandsautomat", "La máquina de estados"),
    texto!("tela.tx_abertura_titulo", "A abertura declarada", "L'ouverture déclarée", "The declared opening", "L'apertura dichiarata", "Die deklarierte Eröffnung", "La apertura declarada"),
    texto!("tela.tx_falta_titulo", "O que NÃO existe, e o motivo", "Ce qui n'existe PAS, et pourquoi", "What does NOT exist, and why", "Cosa NON esiste, e perché", "Was es NICHT gibt, und warum", "Lo que NO existe, y por qué"),
    texto!("tela.tx_c_id", "id", "id", "id", "id", "ID", "id"),
    texto!("tela.tx_c_usuario", "usuário", "utilisateur", "user", "utente", "Benutzer", "usuario"),
    texto!("tela.tx_c_estado", "estado", "état", "state", "stato", "Zustand", "estado"),
    texto!("tela.tx_c_idade", "idade (ms)", "âge (ms)", "age (ms)", "età (ms)", "Alter (ms)", "edad (ms)"),
    texto!("tela.tx_c_linhas", "linhas empilhadas", "lignes empilées", "stacked rows", "righe accumulate", "gestapelte Zeilen", "filas apiladas"),
    texto!("tela.tx_c_lock", "trava", "verrou", "lock", "blocco", "Sperre", "bloqueo"),
    texto!("tela.tx_c_espera", "esperando", "en attente", "waiting on", "in attesa", "wartet auf", "esperando"),
    texto!("tela.tx_c_declaradas", "declaradas", "déclarées", "declared", "dichiarate", "deklariert", "declaradas"),
    texto!("tela.tx_c_efetivas", "efetivas", "effectives", "effective", "effettive", "effektiv", "efectivas"),
    texto!("tela.tx_c_travas", "travas", "verrous", "locks", "blocchi", "Sperren", "bloqueos"),
    texto!("tela.tx_isolamento_a", "**O nível de isolamento, sem enfeite:** escrita serializável por tabela, leitura confirmada e não bloqueante, sem leitura repetível.", "**Le niveau d'isolation, sans fioriture :** écriture sérialisable par table, lecture validée et non bloquante, sans lecture répétable.", "**The isolation level, plainly:** serializable writes per table, committed and non-blocking reads, no repeatable read.", "**Il livello di isolamento, senza fronzoli:** scrittura serializzabile per tabella, lettura confermata e non bloccante, senza lettura ripetibile.", "**Die Isolationsstufe, schmucklos:** pro Tabelle serialisierbares Schreiben, bestätigtes und nicht blockierendes Lesen, ohne Repeatable Read.", "**El nivel de aislamiento, sin adornos:** escritura serializable por tabla, lectura confirmada y no bloqueante, sin lectura repetible."),
    texto!("tela.tx_isolamento_b", "**Não é ANSI SERIALIZABLE** e não se chama assim: não há leitura repetível. A transação **vê** as próprias escritas.", "**Ce n'est pas ANSI SERIALIZABLE** et cela ne s'appelle pas ainsi : pas de lecture répétable. La transaction **voit** ses propres écritures.", "**It is not ANSI SERIALIZABLE** and is not called that: there is no repeatable read. A transaction **does** see its own writes.", "**Non è ANSI SERIALIZABLE** e non si chiama così: non c'è lettura ripetibile. La transazione **vede** le proprie scritture.", "**Es ist nicht ANSI SERIALIZABLE** und heißt auch nicht so: kein Repeatable Read. Die Transaktion **sieht** ihre eigenen Schreibvorgänge.", "**No es ANSI SERIALIZABLE** y no se llama así: no hay lectura repetible. La transacción **ve** sus propias escrituras."),
    texto!("tela.tx_nada_no_disco_a", "**Nada vai a disco antes do COMMIT.** A transação empilha o conjunto de escrita em RAM; desfazer é jogar a lista fora.", "**Rien ne va sur le disque avant le COMMIT.** La transaction empile l'ensemble d'écriture en RAM ; annuler, c'est jeter la liste.", "**Nothing reaches disk before COMMIT.** The transaction stacks the write set in RAM; undoing is throwing the list away.", "**Nulla va su disco prima del COMMIT.** La transazione accumula l'insieme di scrittura in RAM; annullare è buttare via l'elenco.", "**Vor dem COMMIT gelangt nichts auf die Platte.** Die Transaktion stapelt die Schreibmenge im RAM; rückgängig heißt, die Liste wegzuwerfen.", "**Nada va al disco antes del COMMIT.** La transacción apila el conjunto de escritura en RAM; deshacer es tirar la lista."),
    texto!("tela.tx_nada_no_disco_b", "Nenhum slot queimado, nenhum rowid consumido -- e é por isso que quem lê nunca vê dado não confirmado nem espera por quem escreve.", "Aucun slot brûlé, aucun rowid consommé -- et c'est pourquoi celui qui lit ne voit jamais de donnée non validée ni n'attend celui qui écrit.", "No slot burned, no rowid consumed -- and that is why a reader never sees uncommitted data nor waits for a writer.", "Nessuno slot bruciato, nessun rowid consumato -- ed è per questo che chi legge non vede mai dati non confermati né aspetta chi scrive.", "Kein verbrannter Slot, keine verbrauchte rowid -- und deshalb sieht ein Leser nie unbestätigte Daten und wartet nie auf einen Schreiber.", "Ningún slot quemado, ningún rowid consumido -- y por eso quien lee nunca ve dato no confirmado ni espera a quien escribe."),
    texto!("tela.tx_estados_a", "`IDLE`, `ACTIVE`, `COMMITTING`, `COMMITTED` -- ou `ROLLING_BACK` e `ROLLED_BACK`.", "`IDLE`, `ACTIVE`, `COMMITTING`, `COMMITTED` -- ou `ROLLING_BACK` et `ROLLED_BACK`.", "`IDLE`, `ACTIVE`, `COMMITTING`, `COMMITTED` -- or `ROLLING_BACK` and `ROLLED_BACK`.", "`IDLE`, `ACTIVE`, `COMMITTING`, `COMMITTED` -- oppure `ROLLING_BACK` e `ROLLED_BACK`.", "`IDLE`, `ACTIVE`, `COMMITTING`, `COMMITTED` -- oder `ROLLING_BACK` und `ROLLED_BACK`.", "`IDLE`, `ACTIVE`, `COMMITTING`, `COMMITTED` -- o `ROLLING_BACK` y `ROLLED_BACK`."),
    texto!("tela.tx_estados_b", "Depois de um erro de TRANSAÇÃO ela vai para `ABORT_ONLY`: ali o COMMIT **recusa** em vez de confirmar trabalho meio inválido, e só o ROLLBACK passa.", "Après une erreur de TRANSACTION elle passe en `ABORT_ONLY` : là le COMMIT **refuse** au lieu de valider un travail à moitié invalide, et seul le ROLLBACK passe.", "After a TRANSACTION error it goes to `ABORT_ONLY`: there COMMIT **refuses** instead of confirming half-invalid work, and only ROLLBACK gets through.", "Dopo un errore di TRANSAZIONE passa a `ABORT_ONLY`: lì il COMMIT **rifiuta** invece di confermare lavoro mezzo non valido, e passa solo il ROLLBACK.", "Nach einem TRANSAKTIONS-Fehler geht sie in `ABORT_ONLY`: dort **verweigert** das COMMIT, statt halb ungültige Arbeit zu bestätigen, und nur ROLLBACK kommt durch.", "Tras un error de TRANSACCIÓN pasa a `ABORT_ONLY`: allí el COMMIT **rechaza** en vez de confirmar trabajo medio inválido, y solo el ROLLBACK pasa."),
    texto!("tela.tx_classes_a", "Erro de **instrução** -- chave duplicada, tipo errado -- cancela a instrução, e a transação segue `ACTIVE`.", "Une erreur d'**instruction** -- clé dupliquée, type erroné -- annule l'instruction, et la transaction reste `ACTIVE`.", "A **statement** error -- duplicate key, wrong type -- cancels the statement, and the transaction stays `ACTIVE`.", "Un errore di **istruzione** -- chiave duplicata, tipo errato -- annulla l'istruzione, e la transazione resta `ACTIVE`.", "Ein **Anweisungs**fehler -- doppelter Schlüssel, falscher Typ -- bricht die Anweisung ab, und die Transaktion bleibt `ACTIVE`.", "Un error de **instrucción** -- clave duplicada, tipo erróneo -- cancela la instrucción, y la transacción sigue `ACTIVE`."),
    texto!("tela.tx_classes_b", "Erro de **transação** leva a `ABORT_ONLY`, e a queda da conexão desfaz sozinha. O erro diz qual é pelo código: `4005` e `6002`.", "Une erreur de **transaction** mène à `ABORT_ONLY`, et la chute de la connexion annule d'elle-même. L'erreur dit laquelle par le code : `4005` et `6002`.", "A **transaction** error leads to `ABORT_ONLY`, and a dropped connection undoes it by itself. The error says which one by code: `4005` and `6002`.", "Un errore di **transazione** porta a `ABORT_ONLY`, e la caduta della connessione annulla da sola. L'errore dice quale per codice: `4005` e `6002`.", "Ein **Transaktions**fehler führt zu `ABORT_ONLY`, und ein Verbindungsabbruch macht sie von selbst rückgängig. Der Fehler nennt ihn per Code: `4005` und `6002`.", "Un error de **transacción** lleva a `ABORT_ONLY`, y la caída de la conexión deshace sola. El error dice cuál es por el código: `4005` y `6002`."),
    texto!("tela.tx_abertura_a", "Parâmetros **nomeados**, e as cláusulas **não têm ordem**: quem escreve TIMEOUT antes de SCOPE não está errado.", "Paramètres **nommés**, et les clauses **n'ont pas d'ordre** : écrire TIMEOUT avant SCOPE n'est pas une erreur.", "**Named** parameters, and the clauses **have no order**: writing TIMEOUT before SCOPE is not wrong.", "Parametri **nominati**, e le clausole **non hanno ordine**: scrivere TIMEOUT prima di SCOPE non è un errore.", "**Benannte** Parameter, und die Klauseln **haben keine Reihenfolge**: TIMEOUT vor SCOPE zu schreiben ist nicht falsch.", "Parámetros **nombrados**, y las cláusulas **no tienen orden**: escribir TIMEOUT antes de SCOPE no es un error."),
    texto!("tela.tx_abertura_b", "Declarar o escopo é o que paga pela trava de linha: as tabelas conhecidas na abertura são tomadas sempre na mesma ordem canônica.", "Déclarer la portée est ce qui paie le verrou de ligne : les tables connues à l'ouverture sont prises toujours dans le même ordre canonique.", "Declaring the scope is what pays for row locking: tables known at opening are always taken in the same canonical order.", "Dichiarare l'ambito è ciò che paga il blocco di riga: le tabelle note all'apertura sono prese sempre nello stesso ordine canonico.", "Den Geltungsbereich zu deklarieren bezahlt die Zeilensperre: bei der Eröffnung bekannte Tabellen werden stets in derselben kanonischen Reihenfolge genommen.", "Declarar el alcance es lo que paga el bloqueo de fila: las tablas conocidas en la apertura se toman siempre en el mismo orden canónico."),
    texto!("tela.tx_abertura_c", "**Sem SCOPE nenhum, nada muda** -- guarda nova entra pedida, e nunca imposta.", "**Sans aucun SCOPE, rien ne change** -- une garde nouvelle entre sur demande, jamais imposée.", "**With no SCOPE at all, nothing changes** -- a new guard comes in when asked for, never imposed.", "**Senza alcuno SCOPE, nulla cambia** -- una guardia nuova entra su richiesta, mai imposta.", "**Ohne jedes SCOPE ändert sich nichts** -- eine neue Sicherung kommt auf Wunsch, nie aufgezwungen.", "**Sin ningún SCOPE, nada cambia** -- una guarda nueva entra pedida, nunca impuesta."),
    texto!("tela.tx_nao_promete", "E o que ele **não** promete: a ordem canônica mata o ciclo entre TABELAS; entre LINHAS ele continua possível, e a resposta é o `LOCK TIMEOUT`.", "Et ce qu'il **ne** promet pas : l'ordre canonique tue le cycle entre TABLES ; entre LIGNES il reste possible, et la réponse est le `LOCK TIMEOUT`.", "And what it does **not** promise: canonical order kills the cycle between TABLES; between ROWS it is still possible, and the answer is the `LOCK TIMEOUT`.", "E ciò che **non** promette: l'ordine canonico uccide il ciclo tra TABELLE; tra RIGHE resta possibile, e la risposta è il `LOCK TIMEOUT`.", "Und was sie **nicht** verspricht: die kanonische Ordnung tötet den Zyklus zwischen TABELLEN; zwischen ZEILEN bleibt er möglich, und die Antwort ist das `LOCK TIMEOUT`.", "Y lo que **no** promete: el orden canónico mata el ciclo entre TABLAS; entre FILAS sigue siendo posible, y la respuesta es el `LOCK TIMEOUT`."),
    texto!("tela.tx_espera_limitada", "Espera limitada e erro nomeado, nunca uma linha de execução pendurada.", "Attente limitée et erreur nommée, jamais un fil d'exécution suspendu.", "Bounded waiting and a named error, never a hung thread.", "Attesa limitata ed errore nominato, mai un thread appeso.", "Begrenztes Warten und ein benannter Fehler, nie ein hängender Thread.", "Espera limitada y error nombrado, nunca un hilo colgado."),
    texto!("tela.tx_sem_mvcc_a", "**MVCC.** Aqui o rowid **é** o endereço. Uma segunda versão da linha pede um segundo slot, logo um segundo rowid.", "**MVCC.** Ici le rowid **est** l'adresse. Une deuxième version de la ligne demande un deuxième slot, donc un deuxième rowid.", "**MVCC.** Here the rowid **is** the address. A second version of the row asks for a second slot, hence a second rowid.", "**MVCC.** Qui il rowid **è** l'indirizzo. Una seconda versione della riga chiede un secondo slot, quindi un secondo rowid.", "**MVCC.** Hier **ist** die rowid die Adresse. Eine zweite Version der Zeile verlangt einen zweiten Slot, also eine zweite rowid.", "**MVCC.** Aquí el rowid **es** la dirección. Una segunda versión de la fila pide un segundo slot, y por tanto un segundo rowid."),
    texto!("tela.tx_sem_mvcc_b", "Isso quebra a ordem de digitação **e** a replicação, que para quando o rowid diverge. A metade que se quer dele -- leitor que não bloqueia -- já está entregue.", "Cela casse l'ordre de saisie **et** la réplication, qui s'arrête quand le rowid diverge. La moitié qu'on en veut -- un lecteur qui ne bloque pas -- est déjà livrée.", "That breaks the entry order **and** replication, which stops when the rowid diverges. The half of it that is wanted -- a reader that does not block -- is already delivered.", "Questo rompe l'ordine di inserimento **e** la replicazione, che si ferma quando il rowid diverge. La metà che se ne vuole -- un lettore che non blocca -- è già offerta.", "Das bricht die Eingabereihenfolge **und** die Replikation, die anhält, sobald die rowid abweicht. Die gewünschte Hälfte -- ein Leser, der nicht blockiert -- ist schon da.", "Eso rompe el orden de digitación **y** la replicación, que se detiene cuando el rowid diverge. La mitad que se quiere de él -- un lector que no bloquea -- ya está entregada."),
    texto!("tela.tx_sem_ddl", "**DDL dentro de transação.** Um `CREATE TABLE` no meio dela é **recusado**, e não confirma a transação aberta pelas costas -- que é o que outros motores fazem.", "**DDL dans une transaction.** Un `CREATE TABLE` au milieu est **refusé**, et ne valide pas la transaction ouverte dans le dos -- ce que font d'autres moteurs.", "**DDL inside a transaction.** A `CREATE TABLE` in the middle is **refused**, and does not commit the open transaction behind your back -- which is what other engines do.", "**DDL dentro di una transazione.** Un `CREATE TABLE` nel mezzo viene **rifiutato**, e non conferma la transazione aperta alle spalle -- come fanno altri motori.", "**DDL in einer Transaktion.** Ein `CREATE TABLE` mittendrin wird **abgelehnt** und bestätigt die offene Transaktion nicht hinter dem Rücken -- was andere Engines tun.", "**DDL dentro de una transacción.** Un `CREATE TABLE` en medio se **rechaza**, y no confirma la transacción abierta a espaldas de nadie -- que es lo que hacen otros motores."),
    texto!("tela.tx_sem_dois_bancos", "**Transação entre dois databases.** Isso é *two-phase commit*, e é outro projeto: a marca de recuperação mora dentro do diretório do database.", "**Transaction entre deux bases.** C'est du *two-phase commit*, et c'est un autre projet : la marque de récupération vit dans le répertoire de la base.", "**A transaction across two databases.** That is *two-phase commit*, and it is another project: the recovery mark lives inside the database directory.", "**Transazione tra due database.** Questo è *two-phase commit*, ed è un altro progetto: il segno di recupero vive dentro la cartella del database.", "**Transaktion über zwei Datenbanken.** Das ist *Two-Phase-Commit* und ein anderes Projekt: die Wiederherstellungsmarke liegt im Verzeichnis der Datenbank.", "**Transacción entre dos databases.** Eso es *two-phase commit*, y es otro proyecto: la marca de recuperación vive dentro del directorio del database."),
    texto!("tela.tx_docs", "O desenho inteiro, com o que foi recusado e por quê, está em `docs/TRANSACOES.md`. Os números estão em `docs/DESEMPENHO.md`.", "Le dessin entier, avec ce qui a été refusé et pourquoi, est dans `docs/TRANSACOES.md`. Les chiffres sont dans `docs/DESEMPENHO.md`.", "The whole design, with what was refused and why, is in `docs/TRANSACOES.md`. The numbers are in `docs/DESEMPENHO.md`.", "L'intero disegno, con ciò che è stato rifiutato e perché, è in `docs/TRANSACOES.md`. I numeri sono in `docs/DESEMPENHO.md`.", "Der gesamte Entwurf, mit dem Abgelehnten und dem Warum, steht in `docs/TRANSACOES.md`. Die Zahlen stehen in `docs/DESEMPENHO.md`.", "El diseño entero, con lo rechazado y por qué, está en `docs/TRANSACOES.md`. Los números están en `docs/DESEMPENHO.md`."),
    texto!("tela.mi_consulta_sql", "Consulta SQL", "Requête SQL", "SQL query", "Query SQL", "SQL-Abfrage", "Consulta SQL"),
    texto!("tela.mi_servico", "Serviço", "Service", "Service", "Servizio", "Dienst", "Servicio"),
    texto!("tela.mi_sessoes_conexoes", "Sessões e conexões", "Sessions et connexions", "Sessions and connections", "Sessioni e connessioni", "Sitzungen und Verbindungen", "Sesiones y conexiones"),
    texto!("tela.mi_replicacao", "Replicação", "Réplication", "Replication", "Replica", "Replikation", "Replicación"),
    texto!("tela.mi_telemetria", "Telemetria ao vivo…", "Télémétrie en direct…", "Live telemetry…", "Telemetria dal vivo…", "Live-Telemetrie…", "Telemetría en vivo…"),
    texto!("tela.mi_profiler", "Profiler…", "Profileur…", "Profiler…", "Profiler…", "Profiler…", "Profiler…"),
    texto!("tela.mi_reparar", "Reparar…", "Réparer…", "Repair…", "Ripara…", "Reparieren…", "Reparar…"),
    texto!("tela.mi_gerais_servidor", "Gerais do servidor", "Générales du serveur", "Server general", "Generali del server", "Server allgemein", "Generales del servidor"),
    texto!("tela.mi_do_banco", "Do banco atual", "De la base courante", "Of the current database", "Della base corrente", "Der aktuellen Datenbank", "De la base actual"),
    texto!("tela.mi_dos_usuarios", "Dos usuários", "Des utilisateurs", "Of the users", "Degli utenti", "Der Benutzer", "De los usuarios"),
    texto!("tela.mi_diretivas_acesso", "Diretivas de acesso", "Directives d'accès", "Access directives", "Direttive di accesso", "Zugriffsdirektiven", "Directivas de acceso"),
    texto!("tela.mi_diretivas_banco", "Diretivas do banco", "Directives de la base", "Database directives", "Direttive della base", "Datenbankdirektiven", "Directivas de la base"),
    texto!("tela.mi_dblink", "Definições do DbLink…", "Définitions du DbLink…", "DbLink definitions…", "Definizioni del DbLink…", "DbLink-Definitionen…", "Definiciones del DbLink…"),
    texto!("tela.mi_mensagens", "Mensagens do servidor…", "Messages du serveur…", "Server messages…", "Messaggi del server…", "Servermeldungen…", "Mensajes del servidor…"),
    texto!("tela.mi_claude", "Integração com a Claude…", "Intégration avec Claude…", "Claude integration…", "Integrazione con Claude…", "Claude-Integration…", "Integración con Claude…"),
    texto!("tela.mi_editor_menu", "Editor de menu…", "Éditeur de menu…", "Menu editor…", "Editor di menu…", "Menü-Editor…", "Editor de menú…"),
    texto!("tela.mi_atualizar", "Atualizar", "Actualiser", "Refresh", "Aggiorna", "Aktualisieren", "Actualizar"),
    texto!("tela.mi_tema", "Tema claro / escuro", "Thème clair / sombre", "Light / dark theme", "Tema chiaro / scuro", "Helles / dunkles Design", "Tema claro / oscuro"),
    // A area de trabalho multitela entrou depois da regra petrea, entao ela
    // nasce na fabrica em vez de nascer cravada em portugues.
    texto!("tela.mi_nova_aba", "Nova aba nesta região", "Nouvel onglet dans cette zone", "New tab in this region", "Nuova scheda in questa regione", "Neuer Tab in diesem Bereich", "Nueva pestaña en esta región"),
    texto!("tela.mi_fechar_aba", "Fechar esta aba", "Fermer cet onglet", "Close this tab", "Chiudi questa scheda", "Diesen Tab schließen", "Cerrar esta pestaña"),
    texto!("tela.mi_uma_regiao", "Uma região", "Une zone", "One region", "Una regione", "Ein Bereich", "Una región"),
    texto!("tela.mi_duas_regioes", "Duas regiões", "Deux zones", "Two regions", "Due regioni", "Zwei Bereiche", "Dos regiones"),
    texto!("tela.mi_tres_regioes", "Três regiões", "Trois zones", "Three regions", "Tre regioni", "Drei Bereiche", "Tres regiones"),
    texto!("tela.mi_quatro_regioes", "Quatro regiões", "Quatre zones", "Four regions", "Quattro regioni", "Vier Bereiche", "Cuatro regiones"),
    texto!("tela.mi_soltar", "Soltar esta tela numa janela", "Détacher cet écran dans une fenêtre", "Detach this screen into a window", "Stacca questa schermata in una finestra", "Diese Ansicht in ein Fenster lösen", "Soltar esta pantalla en una ventana"),
    texto!("tela.mi_alinhar", "Alinhar com as bordas dos monitores", "Aligner sur les bords des écrans", "Align with the monitor edges", "Allinea ai bordi dei monitor", "An den Monitorrändern ausrichten", "Alinear con los bordes de los monitores"),
    texto!("tela.mi_sobre_multitela", "Sobre o modo multitela…", "À propos du mode multi-écran…", "About multi-screen mode…", "Informazioni sulla modalità multischermo…", "Über den Multiscreen-Modus…", "Acerca del modo multipantalla…"),
    texto!("tela.abas_da_regiao", "Telas abertas nesta região", "Écrans ouverts dans cette zone", "Screens open in this region", "Schermate aperte in questa regione", "Geöffnete Ansichten in diesem Bereich", "Pantallas abiertas en esta región"),
    texto!("tela.cores_de_fabrica", "Voltar às cores de fábrica", "Revenir aux couleurs d'origine", "Back to factory colours", "Torna ai colori di fabbrica", "Zurück zu den Werksfarben", "Volver a los colores de fábrica"),
    texto!("tela.mi_sobre", "Sobre o PhxSql", "À propos de PhxSql", "About PhxSql", "Informazioni su PhxSql", "Über PhxSql", "Acerca de PhxSql"),
    texto!("tela.mi_creditos", "About — quem fez", "About — les auteurs", "About — who made it", "About — chi l'ha fatto", "About — wer es gemacht hat", "About — quién lo hizo"),

    // --------------------------------------------- os botoes da barra de ferramentas
    // Rotulo de barra e curto de proposito: ele fica embaixo de um icone de
    // 22px. O alemao estica ~30%, entao aqui a traducao escolhe a palavra
    // CURTA quando ha duas -- «Konfig», e nao «Konfiguration».
    texto!("tela.fer_bancos", "Bancos", "Bases", "Databases", "Basi", "Datenbanken", "Bases"),
    texto!("tela.fer_gerir_banco", "Gerir Banco", "Gérer la base", "Manage DB", "Gestisci base", "DB verwalten", "Gestionar base"),
    texto!("tela.fer_view_db", "View DB", "Voir la base", "View DB", "Vedi DB", "DB ansehen", "Ver DB"),
    texto!("tela.fer_query", "Query", "Requête", "Query", "Query", "Abfrage", "Consulta"),
    texto!("tela.fer_pivot", "Pivot", "Croisé", "Pivot", "Pivot", "Pivot", "Dinámica"),
    texto!("tela.fer_juncao", "Junção", "Jointure", "Join", "Join", "Join", "Unión"),
    texto!("tela.fer_exportar", "Exportar", "Exporter", "Export", "Esporta", "Export", "Exportar"),
    texto!("tela.fer_importar", "Importar", "Importer", "Import", "Importa", "Import", "Importar"),
    texto!("tela.fer_conexoes", "Conexões", "Connexions", "Connections", "Connessioni", "Verbindungen", "Conexiones"),
    texto!("tela.fer_telemetria", "Telemetria", "Télémétrie", "Telemetry", "Telemetria", "Telemetrie", "Telemetría"),
    texto!("tela.fer_profiler", "Profiler", "Profileur", "Profiler", "Profiler", "Profiler", "Profiler"),
    texto!("tela.fer_jobs", "Jobs", "Tâches", "Jobs", "Job", "Jobs", "Trabajos"),
    texto!("tela.fer_backup", "Backup", "Sauvegarde", "Backup", "Backup", "Sicherung", "Copia"),
    texto!("tela.fer_restaurar", "Restaurar", "Restaurer", "Restore", "Ripristina", "Rücksicherung", "Restaurar"),
    texto!("tela.fer_lixeira", "Lixeira", "Corbeille", "Recycle bin", "Cestino", "Papierkorb", "Papelera"),
    texto!("tela.fer_diagrama", "Diagrama ER", "Diagramme ER", "ER diagram", "Diagramma ER", "ER-Diagramm", "Diagrama ER"),
    texto!("tela.fer_lgpd", "LGPD", "RGPD", "GDPR", "GDPR", "DSGVO", "RGPD"),
    texto!("tela.fer_diretivas", "Diretivas", "Directives", "Directives", "Direttive", "Direktiven", "Directivas"),
    texto!("tela.fer_start_stop", "Start/Stop", "Marche/Arrêt", "Start/Stop", "Avvio/Arresto", "Start/Stopp", "Marcha/Paro"),
    texto!("tela.fer_repair", "Repair", "Réparer", "Repair", "Ripara", "Reparieren", "Reparar"),
    texto!("tela.fer_duplicar", "Duplicar", "Dupliquer", "Duplicate", "Duplica", "Duplizieren", "Duplicar"),
    // O botao da tela de Query. Estava cravado em portugues ate a Juncao mudar
    // de lugar e a barra dessa tela ser mexida -- e foi por isso que ele
    // apareceu: peca que se toca e peca que se conta.
    texto!("tela.consultar", "Consultar", "Interroger", "Query", "Interroga", "Abfragen", "Consultar"),
    texto!("tela.fer_server_mail", "Server Mail", "Messagerie", "Server Mail", "Server Mail", "Server-Mail", "Correo"),
    texto!("tela.fer_dica_telemetria", "gráficos bolha ordenados por peso, no molde do SQL Check da Idera®", "graphiques à bulles triés par poids, dans l'esprit du SQL Check d'Idera®", "bubble charts sorted by weight, in the spirit of Idera® SQL Check", "grafici a bolle ordinati per peso, nello stile di SQL Check di Idera®", "Blasendiagramme nach Gewicht sortiert, im Stil von Idera® SQL Check", "gráficos de burbujas ordenados por peso, al estilo del SQL Check de Idera®"),
    texto!("tela.fer_dica_profiler", "o que chega pela porta de dados, antes de virar dado", "ce qui arrive par le port de données, avant de devenir donnée", "what arrives at the data port, before it becomes data", "ciò che arriva dalla porta dati, prima di diventare dato", "was am Datenport ankommt, bevor es zu Daten wird", "lo que llega por el puerto de datos, antes de volverse dato"),
    texto!("tela.fer_dica_restaurar", "traz uma cópia de volta: com outro nome, ou por cima", "ramène une copie : sous un autre nom, ou par-dessus", "brings a copy back: under another name, or over the top", "riporta una copia: con un altro nome, o sopra", "holt eine Kopie zurück: unter anderem Namen oder darüber", "trae una copia de vuelta: con otro nombre, o encima"),

    // ------------------------------------------------------- a tela de idiomas
    // Onde a escolha do idioma vive DEPOIS do login -- o outro lado do
    // caminho que comeca nas bandeiras da tela de entrada.
    texto!("tela.idi_sub", "os textos da tela em seis idiomas · phxsys.mensagens, uma tabela como as outras", "les textes de l'écran en six langues · phxsys.mensagens, une table comme les autres", "the screen texts in six languages · phxsys.mensagens, a table like any other", "i testi dello schermo in sei lingue · phxsys.mensagens, una tabella come le altre", "die Bildschirmtexte in sechs Sprachen · phxsys.mensagens, eine Tabelle wie jede andere", "los textos de la pantalla en seis idiomas · phxsys.mensagens, una tabla como las demás"),
    texto!("tela.idi_escolha", "Idioma desta tela", "Langue de cet écran", "Language of this screen", "Lingua di questo schermo", "Sprache dieses Bildschirms", "Idioma de esta pantalla"),
    texto!("tela.idi_escolha_dica", "Vale na hora, sem recarregar, e é a mesma escolha das bandeiras da tela de entrada.", "S'applique aussitôt, sans recharger, et c'est le même choix que les drapeaux de l'écran d'entrée.", "Applies at once, without reloading, and it is the same choice as the flags on the sign-in screen.", "Vale subito, senza ricaricare, ed è la stessa scelta delle bandiere della schermata d'ingresso.", "Gilt sofort, ohne Neuladen, und ist dieselbe Wahl wie die Flaggen im Anmeldebildschirm.", "Vale al instante, sin recargar, y es la misma elección que las banderas de la pantalla de entrada."),
    texto!("tela.idi_linhas", "linhas na tabela", "lignes dans la table", "rows in the table", "righe nella tabella", "Zeilen in der Tabelle", "filas en la tabla"),
    texto!("tela.idi_semeados", "textos de tela semeados", "textes d'écran semés", "screen texts seeded", "testi di schermo seminati", "gesäte Bildschirmtexte", "textos de pantalla sembrados"),
    texto!("tela.idi_faltam", "faltam", "il en manque", "missing", "ne mancano", "es fehlen", "faltan"),
    texto!("tela.idi_nada_semear", "nada a semear", "rien à semer", "nothing to seed", "niente da seminare", "nichts zu säen", "nada que sembrar"),
    texto!("tela.idi_traduzidos", "com tradução própria", "avec traduction propre", "with their own translation", "con traduzione propria", "mit eigener Übersetzung", "con traducción propia"),
    texto!("tela.idi_no_idioma", "no idioma", "dans la langue", "in language", "nella lingua", "in der Sprache", "en el idioma"),
    texto!("tela.idi_cobertura", "da tela na fábrica", "de l'écran dans l'usine", "of the screen in the factory", "dello schermo nella fabbrica", "des Bildschirms in der Fabrik", "de la pantalla en la fábrica"),
    texto!("tela.idi_cobertura_u", "medido pelo conferidor, não digitado", "mesuré par le vérificateur, non saisi", "measured by the checker, not typed", "misurato dal verificatore, non digitato", "vom Prüfer gemessen, nicht getippt", "medido por el verificador, no escrito"),
    texto!("tela.idi_leg", "Os textos que a tabela não cobre caem no português de fábrica — sem tabela, a tela é a de sempre. A tradução se edita na grade, como qualquer tabela.", "Les textes que la table ne couvre pas retombent sur le portugais d'usine — sans table, l'écran est celui de toujours. La traduction s'édite dans la grille, comme toute table.", "Texts the table does not cover fall back to factory Portuguese — with no table, the screen is the usual one. Translation is edited in the grid, like any table.", "I testi non coperti dalla tabella ricadono sul portoghese di fabbrica — senza tabella, lo schermo è quello di sempre. La traduzione si modifica nella griglia, come ogni tabella.", "Texte, die die Tabelle nicht abdeckt, fallen auf das Werks-Portugiesisch zurück — ohne Tabelle ist der Bildschirm der gewohnte. Übersetzt wird im Raster, wie bei jeder Tabelle.", "Los textos que la tabla no cubre caen al portugués de fábrica — sin tabla, la pantalla es la de siempre. La traducción se edita en la rejilla, como cualquier tabla."),
    texto!("tela.idi_carga", "Carga da tabela", "Chargement de la table", "Seed the table", "Carica la tabella", "Tabelle befüllen", "Carga de la tabla"),
    texto!("tela.idi_exportar", "Exportar backup", "Exporter la sauvegarde", "Export backup", "Esporta backup", "Backup exportieren", "Exportar copia"),
    texto!("tela.idi_importar", "Importar backup", "Importer la sauvegarde", "Import backup", "Importa backup", "Backup importieren", "Importar copia"),
    texto!("tela.idi_carga_aviso", "só acrescenta o que falta: linha que já existe não é tocada, então ela pode ser repetida à vontade sem desfazer tradução nenhuma.", "n'ajoute que ce qui manque : une ligne existante n'est pas touchée, elle peut donc être répétée sans défaire aucune traduction.", "only adds what is missing: an existing row is left alone, so it can be repeated freely without undoing any translation.", "aggiunge solo ciò che manca: una riga già esistente non viene toccata, quindi può essere ripetuta senza disfare alcuna traduzione.", "fügt nur hinzu, was fehlt: eine vorhandene Zeile bleibt unberührt, sie kann also beliebig wiederholt werden, ohne eine Übersetzung zu zerstören.", "solo añade lo que falta: una fila existente no se toca, así que puede repetirse a voluntad sin deshacer ninguna traducción."),
    texto!("tela.idi_padrao", "Carga padrão", "Chargement d'usine", "Factory seed", "Carica di fabbrica", "Werksbefüllung", "Carga de fábrica"),
    texto!("tela.idi_padrao_leg", "Devolve os textos de fábrica por cima do que está gravado — é o «caso a tradução não tenha ficado boa». Escolha o alcance: um idioma só deixa os outros cinco intactos.", "Remet les textes d'usine par-dessus l'existant — c'est le « au cas où la traduction ne serait pas bonne ». Choisissez la portée : une seule langue laisse les cinq autres intactes.", "Puts the factory texts back over what is stored — the «in case the translation did not turn out well». Choose the scope: one language leaves the other five untouched.", "Rimette i testi di fabbrica sopra quanto è salvato — è il «caso in cui la traduzione non sia venuta bene». Scegli la portata: una sola lingua lascia intatte le altre cinque.", "Legt die Werkstexte über das Gespeicherte — das «falls die Übersetzung nicht gut wurde». Wählen Sie den Umfang: eine einzelne Sprache lässt die anderen fünf unberührt.", "Devuelve los textos de fábrica encima de lo guardado — es el «por si la traducción no quedó bien». Elija el alcance: un solo idioma deja los otros cinco intactos."),
    texto!("tela.idi_so", "só", "seulement", "only", "solo", "nur", "solo"),
    texto!("tela.idi_seis", "os seis idiomas", "les six langues", "all six languages", "le sei lingue", "alle sechs Sprachen", "los seis idiomas"),
    texto!("tela.mi_idiomas", "Idiomas da interface…", "Langues de l'interface…", "Interface languages…", "Lingue dell'interfaccia…", "Sprachen der Oberfläche…", "Idiomas de la interfaz…"),

    // ------------------------------------- o resto do que esta rodada trouxe
    texto!("tela.titulo_pagina", "PhxSql — Centro de Controle", "PhxSql — Centre de Contrôle", "PhxSql — Control Center", "PhxSql — Centro di Controllo", "PhxSql — Kontrollzentrum", "PhxSql — Centro de Control"),
    texto!("tela.ainda_nao_existe", "ainda não existe", "n'existe pas encore", "does not exist yet", "non esiste ancora", "gibt es noch nicht", "todavía no existe"),
    texto!("tela.painel_sub", "o servidor inteiro numa tela · os monitores da máquina renovam sozinhos", "le serveur entier sur un écran · les moniteurs de la machine se rafraîchissent seuls", "the whole server on one screen · the machine monitors refresh on their own", "l'intero server in una schermata · i monitor della macchina si aggiornano da soli", "der ganze Server auf einem Bildschirm · die Maschinenmonitore aktualisieren sich selbst", "el servidor entero en una pantalla · los monitores de la máquina se renuevan solos"),
    texto!("tela.usuarios_sub", "cadastro do config.json · a senha nunca sai daqui", "fiche du config.json · le mot de passe ne sort jamais d'ici", "roster from config.json · the password never leaves here", "anagrafica del config.json · la password non esce mai da qui", "Verzeichnis aus config.json · das Kennwort verlässt diesen Ort nie", "registro del config.json · la contraseña nunca sale de aquí"),
    texto!("tela.acessos_sub", "toda tentativa entra, inclusive as recusadas", "toute tentative est consignée, refus compris", "every attempt is logged, refusals included", "ogni tentativo entra, comprese le negazioni", "jeder Versuch wird erfasst, auch die abgelehnten", "todo intento entra, incluidos los rechazados"),
    texto!("tela.bloqueios_sub", "blacklist.json · o IP é recusado antes do token", "blacklist.json · l'IP est refusé avant le jeton", "blacklist.json · the IP is refused before the token", "blacklist.json · l'IP è respinto prima del token", "blacklist.json · die IP wird vor dem Token abgewiesen", "blacklist.json · la IP se rechaza antes del token"),
    texto!("tela.cfg_titulo", "Configurações gerais do servidor", "Réglages généraux du serveur", "General server settings", "Impostazioni generali del server", "Allgemeine Servereinstellungen", "Ajustes generales del servidor"),
    texto!("tela.cfg_sub", "o que está valendo agora · edita e grava no config.json", "ce qui est en vigueur · édite et écrit dans config.json", "what is in force now · edits and writes to config.json", "ciò che vale adesso · modifica e scrive nel config.json", "was gerade gilt · bearbeitet und schreibt in config.json", "lo que está vigente ahora · edita y graba en config.json"),
    texto!("tela.cfg_salvar", "Salvar no config.json", "Enregistrer dans config.json", "Save to config.json", "Salva nel config.json", "In config.json speichern", "Guardar en config.json"),
    texto!("tela.cfg_descartar", "Descartar as mudanças", "Abandonner les modifications", "Discard the changes", "Scarta le modifiche", "Änderungen verwerfen", "Descartar los cambios"),
    texto!("tela.cfg_salvar_leg", "grava só o que você mudou · escrita atômica, com o arquivo antigo inteiro até o fim", "n'écrit que ce que vous avez changé · écriture atomique, l'ancien fichier entier jusqu'au bout", "writes only what you changed · atomic write, with the old file whole until the end", "scrive solo ciò che hai cambiato · scrittura atomica, con il vecchio file intero fino alla fine", "schreibt nur, was Sie geändert haben · atomarer Schreibvorgang, die alte Datei bleibt bis zuletzt vollständig", "graba solo lo que usted cambió · escritura atómica, con el archivo antiguo entero hasta el final"),
    texto!("tela.cfg_idioma", "Idioma desta interface", "Langue de cette interface", "Language of this interface", "Lingua di questa interfaccia", "Sprache dieser Oberfläche", "Idioma de esta interfaz"),
    texto!("tela.cfg_idioma_dica", "Vale neste navegador e na hora. O campo «idioma» do config.json é outro: ele manda nas mensagens que o servidor devolve pelo protocolo.", "Vaut dans ce navigateur et tout de suite. Le champ « idioma » du config.json est autre chose : il régit les messages que le serveur renvoie par le protocole.", "Applies to this browser, at once. The «idioma» field of config.json is a different thing: it governs the messages the server returns over the protocol.", "Vale in questo browser e subito. Il campo «idioma» del config.json è un'altra cosa: governa i messaggi che il server restituisce dal protocollo.", "Gilt in diesem Browser und sofort. Das Feld «idioma» in config.json ist etwas anderes: es steuert die Meldungen, die der Server über das Protokoll zurückgibt.", "Vale en este navegador y al instante. El campo «idioma» del config.json es otra cosa: manda en los mensajes que el servidor devuelve por el protocolo."),
    // ------------------------------------------- o rodizio do .txt do Profiler
    texto!("tela.pf_rodizio_a_cada", "rodízio a cada", "rotation tous les", "rotates every", "rotazione ogni", "Rotation alle", "rotación cada"),
    texto!("tela.pf_rodizio_guardando", "guardando", "en gardant", "keeping", "conservando", "mit", "guardando"),
    texto!("tela.pf_rodizio_teto", "no máximo", "au maximum", "at most", "al massimo", "höchstens", "como máximo"),
    texto!("tela.pf_rodizio_em_disco", "em disco", "sur disque", "on disk", "su disco", "auf der Festplatte", "en disco"),
    texto!("tela.pf_rodizio_ja_virou", "já virou", "a déjà tourné", "already rotated", "già ruotato", "bereits rotiert", "ya rotó"),
    texto!("tela.pf_rodizio_vezes", "vez(es) — o começo da sessão está nos arquivos com sufixo .1 e seguintes", "fois — le début de la session est dans les fichiers suffixés .1 et suivants", "time(s) — the start of the session is in the files suffixed .1 and up", "volta/e — l'inizio della sessione è nei file con suffisso .1 e seguenti", "Mal — der Anfang der Sitzung liegt in den Dateien mit Suffix .1 und folgenden", "vez(ces) — el inicio de la sesión está en los archivos con sufijo .1 y siguientes"),
    texto!("tela.pf_sem_rodizio", "sem rodízio: o arquivo cresce sem teto", "sans rotation : le fichier grandit sans limite", "no rotation: the file grows without a ceiling", "senza rotazione: il file cresce senza limite", "ohne Rotation: die Datei wächst ohne Obergrenze", "sin rotación: el archivo crece sin tope"),
    texto!("tela.pf_rodizio_falhou", "rodízio(s) com falha — não deu para renomear ou reabrir o arquivo", "rotation(s) en échec — impossible de renommer ou rouvrir le fichier", "failed rotation(s) — could not rename or reopen the file", "rotazione/i fallita/e — non è stato possibile rinominare o riaprire il file", "fehlgeschlagene Rotation(en) — Umbenennen oder erneutes Öffnen nicht möglich", "rotación(es) con fallo — no fue posible renombrar o reabrir el archivo"),

    // ================================================ o modo multitela
    // Esta leva pagou uma licao de desenho que esta escrita no
    // `docs/MENSAGENS.md`: **frase picada por marcacao e intraduzivel por
    // construcao**. Os `<b>` e os `<code>` que quebravam um paragrafo em treze
    // literais viraram MARCAS dentro do proprio texto -- `**assim**` e a frase
    // entre crases -- e o corte em etiquetas passou a acontecer DEPOIS da
    // traducao, no `marcado()` da pagina. Assim o tradutor move a enfase para
    // onde a lingua dele pede, e o alemao pode mandar o verbo para o fim.
    //
    // A outra regra desta leva: a unidade e a FRASE, nunca o paragrafo. A
    // celula so guarda 250 caracteres, e paragrafo alemao passa disso -- entao
    // paragrafo longo entra partido em frases inteiras, que se traduzem
    // sozinhas, e nunca em pedacos de frase, que nao se traduzem.

    // ------------------------------------------------------ a tira de abas
    texto!("tela.mt_aba_pinada", "pinada: volta na próxima abertura", "épinglée : revient à la prochaine ouverture", "pinned: comes back on the next opening", "fissata: torna alla prossima apertura", "angeheftet: kommt beim nächsten Öffnen zurück", "fijada: vuelve en la próxima apertura"),
    texto!("tela.mt_despinar_dica", "Despinar — esta tela deixa de voltar sozinha", "Désépingler — cet écran cesse de revenir tout seul", "Unpin — this screen stops coming back on its own", "Sblocca — questa schermata smette di tornare da sola", "Lösen — dieser Bildschirm kommt nicht mehr von selbst zurück", "Desfijar — esta pantalla deja de volver sola"),
    texto!("tela.mt_pinar_aba_dica", "Pinar — esta tela volta na próxima abertura, na mesma região", "Épingler — cet écran revient à la prochaine ouverture, dans la même région", "Pin — this screen comes back on the next opening, in the same region", "Fissa — questa schermata torna alla prossima apertura, nella stessa regione", "Anheften — dieser Bildschirm kommt beim nächsten Öffnen zurück, in derselben Region", "Fijar — esta pantalla vuelve en la próxima apertura, en la misma región"),
    texto!("tela.mt_fechar_tela", "Fechar esta tela", "Fermer cet écran", "Close this screen", "Chiudi questa schermata", "Diesen Bildschirm schließen", "Cerrar esta pantalla"),
    texto!("tela.mt_uma_regiao", "Uma região só", "Une seule région", "One region only", "Una sola regione", "Nur eine Region", "Una sola región"),
    texto!("tela.mt_n_regioes", "{n} regiões lado a lado", "{n} régions côte à côte", "{n} regions side by side", "{n} regioni affiancate", "{n} Regionen nebeneinander", "{n} regiones lado a lado"),
    texto!("tela.mt_nao_cabe", "não cabe: cada região precisa de {px}px", "ne tient pas : chaque région a besoin de {px}px", "does not fit: each region needs {px}px", "non ci sta: ogni regione ha bisogno di {px}px", "passt nicht: jede Region braucht {px}px", "no cabe: cada región necesita {px}px"),
    texto!("tela.mt_largura_regioes", "Largura das regiões", "Largeur des régions", "Width of the regions", "Larghezza delle regioni", "Breite der Regionen", "Ancho de las regiones"),

    // ------------------------------------------ os botoes da janela do sistema
    texto!("tela.mt_pinar_janela_dica", "Pinar — guarda x, y, largura, altura e o monitor desta janela neste navegador, e ela volta assim na próxima vez", "Épingler — enregistre x, y, largeur, hauteur et l'écran de cette fenêtre dans ce navigateur ; elle revient ainsi la prochaine fois", "Pin — stores x, y, width, height and the monitor of this window in this browser, and it comes back like this next time", "Fissa — salva x, y, larghezza, altezza e il monitor di questa finestra in questo browser, e torna così la prossima volta", "Anheften — speichert x, y, Breite, Höhe und den Monitor dieses Fensters in diesem Browser; beim nächsten Mal kommt es genauso zurück", "Fijar — guarda x, y, ancho, alto y el monitor de esta ventana en este navegador, y vuelve así la próxima vez"),
    texto!("tela.mt_pinar_aqui", "pinar aqui", "épingler ici", "pin here", "fissa qui", "hier anheften", "fijar aquí"),
    texto!("tela.mt_devolver_dica", "Devolver esta tela para a janela principal e fechar esta", "Renvoyer cet écran vers la fenêtre principale et fermer celle-ci", "Send this screen back to the main window and close this one", "Riporta questa schermata alla finestra principale e chiudi questa", "Diesen Bildschirm ins Hauptfenster zurückgeben und dieses schließen", "Devolver esta pantalla a la ventana principal y cerrar esta"),
    texto!("tela.mt_devolver", "devolver", "renvoyer", "send back", "riporta", "zurückgeben", "devolver"),
    texto!("tela.mt_nova_dica", "Abrir outra tela nesta região (a próxima escolha cai aqui)", "Ouvrir un autre écran dans cette région (le prochain choix arrive ici)", "Open another screen in this region (the next choice lands here)", "Apri un'altra schermata in questa regione (la prossima scelta finisce qui)", "Einen weiteren Bildschirm in dieser Region öffnen (die nächste Wahl landet hier)", "Abrir otra pantalla en esta región (la próxima elección cae aquí)"),
    texto!("tela.mt_soltar_dica", "Soltar esta tela numa janela flutuante DENTRO da página, arrastável pelo cabeçalho e redimensionável pelo canto", "Détacher cet écran dans une fenêtre flottante À L'INTÉRIEUR de la page, déplaçable par l'en-tête et redimensionnable par le coin", "Float this screen in a window INSIDE the page, draggable by the header and resizable by the corner", "Stacca questa schermata in una finestra mobile DENTRO la pagina, trascinabile dall'intestazione e ridimensionabile dall'angolo", "Diesen Bildschirm in ein schwebendes Fenster INNERHALB der Seite lösen, per Kopfzeile verschiebbar und per Ecke skalierbar", "Soltar esta pantalla en una ventana flotante DENTRO de la página, arrastrable por la cabecera y redimensionable por la esquina"),
    texto!("tela.mt_destacar_dica", "Destacar numa janela do sistema, fora desta página (só serve para quem tem monitor separado — o modo em regiões não depende disto)", "Détacher dans une fenêtre du système, hors de cette page (utile seulement avec un écran séparé — le mode en régions n'en dépend pas)", "Detach into a system window, outside this page (only useful with a separate monitor — the region mode does not depend on it)", "Stacca in una finestra di sistema, fuori da questa pagina (serve solo a chi ha un monitor separato — la modalità a regioni non dipende da questo)", "In ein Systemfenster außerhalb dieser Seite lösen (nur sinnvoll mit einem separaten Monitor — der Regionenmodus hängt nicht davon ab)", "Separar en una ventana del sistema, fuera de esta página (solo sirve con monitor aparte — el modo en regiones no depende de esto)"),

    // ---------------------------------- a janela solta DENTRO da propria pagina
    texto!("tela.mt_pinar_solta_dica", "Pinar — guarda x, y, largura e altura desta janela neste navegador", "Épingler — enregistre x, y, largeur et hauteur de cette fenêtre dans ce navigateur", "Pin — stores x, y, width and height of this window in this browser", "Fissa — salva x, y, larghezza e altezza di questa finestra in questo browser", "Anheften — speichert x, y, Breite und Höhe dieses Fensters in diesem Browser", "Fijar — guarda x, y, ancho y alto de esta ventana en este navegador"),
    texto!("tela.mt_acoplar_dica", "Devolver esta tela para a área em regiões", "Renvoyer cet écran vers la zone en régions", "Send this screen back to the region area", "Riporta questa schermata all'area a regioni", "Diesen Bildschirm in den Regionenbereich zurückgeben", "Devolver esta pantalla al área en regiones"),
    texto!("tela.mt_redimensionar", "Redimensionar", "Redimensionner", "Resize", "Ridimensiona", "Größe ändern", "Redimensionar"),

    // ---------------------------------------------------- os recados do modo
    texto!("tela.mt_pinada_aviso", "“{tela}” pinada: volta na próxima abertura, neste navegador", "« {tela} » épinglée : revient à la prochaine ouverture, dans ce navigateur", "“{tela}” pinned: comes back on the next opening, in this browser", "«{tela}» fissata: torna alla prossima apertura, in questo browser", "„{tela}“ angeheftet: kommt beim nächsten Öffnen zurück, in diesem Browser", "«{tela}» fijada: vuelve en la próxima apertura, en este navegador"),
    texto!("tela.mt_despinada_aviso", "“{tela}” despinada", "« {tela} » désépinglée", "“{tela}” unpinned", "«{tela}» sbloccata", "„{tela}“ gelöst", "«{tela}» desfijada"),
    texto!("tela.mt_nao_cabem_regioes", "não cabem {n} regiões: cada uma precisa de {px}px", "{n} régions ne tiennent pas : chacune a besoin de {px}px", "{n} regions do not fit: each one needs {px}px", "non ci stanno {n} regioni: ognuna ha bisogno di {px}px", "{n} Regionen passen nicht: jede braucht {px}px", "no caben {n} regiones: cada una necesita {px}px"),
    texto!("tela.mt_um_monitor_so", "esta janela está dentro de um monitor só — nada a alinhar", "cette fenêtre tient dans un seul écran — rien à aligner", "this window sits inside a single monitor — nothing to align", "questa finestra sta dentro un solo monitor — niente da allineare", "dieses Fenster liegt in einem einzigen Monitor — nichts auszurichten", "esta ventana está dentro de un solo monitor — nada que alinear"),
    texto!("tela.mt_sem_arranjo", "este navegador não expõe o arranjo de monitores; a divisão fica em partes iguais", "ce navigateur n'expose pas la disposition des écrans ; le partage reste en parts égales", "this browser does not expose the monitor layout; the split stays in equal parts", "questo browser non espone la disposizione dei monitor; la divisione resta in parti uguali", "dieser Browser gibt die Monitoranordnung nicht preis; die Teilung bleibt gleichmäßig", "este navegador no expone la disposición de monitores; el reparto queda en partes iguales"),
    texto!("tela.mt_alinhadas", "{n} regiões alinhadas com as bordas físicas dos monitores", "{n} régions alignées sur les bords physiques des écrans", "{n} regions aligned with the physical monitor edges", "{n} regioni allineate ai bordi fisici dei monitor", "{n} Regionen an den physischen Monitorkanten ausgerichtet", "{n} regiones alineadas con los bordes físicos de los monitores"),
    texto!("tela.mt_sem_endereco_destacar", "esta tela não tem endereço próprio para destacar", "cet écran n'a pas d'adresse propre à détacher", "this screen has no address of its own to detach", "questa schermata non ha un indirizzo proprio da staccare", "dieser Bildschirm hat keine eigene Adresse zum Lösen", "esta pantalla no tiene dirección propia para separar"),
    texto!("tela.mt_popup_bloqueado", "o navegador bloqueou a janela — libere o popup desta origem", "le navigateur a bloqué la fenêtre — autorisez la pop-up de cette origine", "the browser blocked the window — allow pop-ups from this origin", "il browser ha bloccato la finestra — consenti il popup di questa origine", "der Browser hat das Fenster blockiert — Pop-ups dieser Herkunft zulassen", "el navegador bloqueó la ventana — permita el popup de este origen"),
    texto!("tela.mt_foi_para_janela", "“{tela}” foi para uma janela", "« {tela} » est passée dans une fenêtre", "“{tela}” moved to a window", "«{tela}» è passata in una finestra", "„{tela}“ ist in ein Fenster gewandert", "«{tela}» pasó a una ventana"),
    texto!("tela.mt_foi_para_janela_pinada", "“{tela}” foi para uma janela na posição pinada", "« {tela} » est passée dans une fenêtre à la position épinglée", "“{tela}” moved to a window at the pinned position", "«{tela}» è passata in una finestra nella posizione fissata", "„{tela}“ ist in ein Fenster an der angehefteten Position gewandert", "«{tela}» pasó a una ventana en la posición fijada"),
    texto!("tela.mt_monitor_sumiu", "o monitor “{monitor}” não está mais aqui — a janela abre no principal", "l'écran « {monitor} » n'est plus là — la fenêtre s'ouvre sur le principal", "the monitor “{monitor}” is no longer here — the window opens on the primary one", "il monitor «{monitor}» non c'è più — la finestra si apre sul principale", "der Monitor „{monitor}“ ist nicht mehr da — das Fenster öffnet auf dem primären", "el monitor «{monitor}» ya no está — la ventana abre en el principal"),
    texto!("tela.mt_janela_pinada", "posição, tamanho e monitor guardados neste navegador", "position, taille et écran enregistrés dans ce navigateur", "position, size and monitor stored in this browser", "posizione, dimensione e monitor salvati in questo browser", "Position, Größe und Monitor in diesem Browser gespeichert", "posición, tamaño y monitor guardados en este navegador"),
    texto!("tela.mt_outra_densidade", "monitor de outra densidade ({antes}× → {agora}×) — redesenhando", "écran d'une autre densité ({antes}× → {agora}×) — redessin en cours", "monitor of another density ({antes}× → {agora}×) — redrawing", "monitor di altra densità ({antes}× → {agora}×) — ridisegno in corso", "Monitor mit anderer Dichte ({antes}× → {agora}×) — wird neu gezeichnet", "monitor de otra densidad ({antes}× → {agora}×) — redibujando"),
    texto!("tela.mt_presa", "a janela solta não cabia onde estava guardada — foi presa dentro da área visível", "la fenêtre flottante ne tenait pas où elle était enregistrée — elle a été ramenée dans la zone visible", "the floating window did not fit where it was stored — it was kept inside the visible area", "la finestra mobile non stava dov'era salvata — è stata riportata dentro l'area visibile", "das schwebende Fenster passte nicht dorthin, wo es gespeichert war — es wurde in den sichtbaren Bereich geholt", "la ventana flotante no cabía donde estaba guardada — se sujetó dentro del área visible"),
    texto!("tela.mt_sem_endereco_pinar", "esta tela não tem endereço próprio para pinar", "cet écran n'a pas d'adresse propre à épingler", "this screen has no address of its own to pin", "questa schermata non ha un indirizzo proprio da fissare", "dieser Bildschirm hat keine eigene Adresse zum Anheften", "esta pantalla no tiene dirección propia para fijar"),
    texto!("tela.mt_solta_pinada", "esta janela volta solta, nesta posição e neste tamanho, na próxima abertura", "cette fenêtre revient flottante, à cette position et à cette taille, à la prochaine ouverture", "this window comes back floating, at this position and size, on the next opening", "questa finestra torna mobile, in questa posizione e con questa dimensione, alla prossima apertura", "dieses Fenster kommt beim nächsten Öffnen schwebend zurück, an dieser Position und in dieser Größe", "esta ventana vuelve suelta, en esta posición y con este tamaño, en la próxima apertura"),
    texto!("tela.mt_solta_despinada", "esta janela deixa de voltar sozinha", "cette fenêtre cesse de revenir toute seule", "this window stops coming back on its own", "questa finestra smette di tornare da sola", "dieses Fenster kommt nicht mehr von selbst zurück", "esta ventana deja de volver sola"),

    // ------------------------- a nota que diz o que muda em qual navegador
    // Uma chave por FRASE, com o `<b>` e o `<code>` virados marca dentro do
    // texto: e por isso que a ordem das palavras e livre em cada idioma.
    texto!("tela.mt_nota", "**Multitela.** Abas vivas e regiões lado a lado funcionam em **qualquer navegador** — é layout.", "**Multi-écran.** Les onglets vivants et les régions côte à côte fonctionnent dans **n'importe quel navigateur** — c'est de la mise en page.", "**Multi-screen.** Live tabs and side-by-side regions work in **any browser** — it is layout.", "**Multischermo.** Le schede vive e le regioni affiancate funzionano in **qualsiasi browser** — è impaginazione.", "**Mehrbildschirm.** Lebende Registerkarten und nebeneinanderliegende Regionen funktionieren in **jedem Browser** — das ist Layout.", "**Multipantalla.** Las pestañas vivas y las regiones lado a lado funcionan en **cualquier navegador** — es maquetación."),
    texto!("tela.mt_nota_janela", "Destacar em janela também, com `window.open`. O que depende do navegador é abrir a janela **já no monitor certo**:", "Détacher en fenêtre aussi, avec `window.open`. Ce qui dépend du navigateur, c'est d'ouvrir la fenêtre **directement sur le bon écran** :", "Detaching into a window works too, with `window.open`. What depends on the browser is opening the window **already on the right monitor**:", "Anche staccare in finestra funziona, con `window.open`. Ciò che dipende dal browser è aprire la finestra **già sul monitor giusto**:", "Das Lösen in ein Fenster geht ebenfalls, mit `window.open`. Vom Browser hängt ab, das Fenster **gleich auf dem richtigen Monitor** zu öffnen:", "Separar en ventana también, con `window.open`. Lo que depende del navegador es abrir la ventana **ya en el monitor correcto**:"),
    texto!("tela.mt_nota_com_api", "este navegador tem a **Window Management API**, então a posição pinada volta no monitor em que você a deixou.", "ce navigateur a la **Window Management API**, donc la position épinglée revient sur l'écran où vous l'avez laissée.", "this browser has the **Window Management API**, so the pinned position comes back on the monitor where you left it.", "questo browser ha la **Window Management API**, quindi la posizione fissata torna sul monitor dove l'hai lasciata.", "dieser Browser hat die **Window Management API**, daher kehrt die angeheftete Position auf den Monitor zurück, auf dem Sie sie gelassen haben.", "este navegador tiene la **Window Management API**, así que la posición fijada vuelve al monitor donde la dejó."),
    texto!("tela.mt_nota_sem_api", "este navegador **não tem** a Window Management API (Firefox e Safari não a têm). A janela abre onde o navegador quiser e você a arrasta; a posição volta, o monitor não é escolhido.", "ce navigateur **n'a pas** la Window Management API (Firefox et Safari ne l'ont pas). La fenêtre s'ouvre où le navigateur veut et vous la déplacez ; la position revient, l'écran n'est pas choisi.", "this browser **does not have** the Window Management API (Firefox and Safari do not). The window opens wherever the browser likes and you drag it; the position comes back, the monitor is not chosen.", "questo browser **non ha** la Window Management API (Firefox e Safari non ce l'hanno). La finestra si apre dove vuole il browser e tu la trascini; la posizione torna, il monitor non viene scelto.", "dieser Browser **hat** die Window Management API **nicht** (Firefox und Safari haben sie nicht). Das Fenster öffnet, wo der Browser will, und Sie ziehen es hin; die Position kehrt zurück, der Monitor nicht.", "este navegador **no tiene** la Window Management API (Firefox y Safari no la tienen). La ventana abre donde el navegador quiera y usted la arrastra; la posición vuelve, el monitor no se elige."),
    texto!("tela.mt_nota_docking", "Arrastar uma janela do sistema de volta para a barra de abas **não é possível** em navegador nenhum — o navegador não vê esse arrasto. Use **⤺ devolver** na janela destacada.", "Faire glisser une fenêtre du système jusqu'à la barre d'onglets **n'est possible** dans aucun navigateur — il ne voit pas ce glissement. Utilisez **⤺ renvoyer** dans la fenêtre détachée.", "Dragging a system window back onto the tab strip **is not possible** in any browser — the browser does not see that drag. Use **⤺ send back** in the detached window.", "Trascinare una finestra di sistema di nuovo sulla barra delle schede **non è possibile** in nessun browser — il browser non vede quel trascinamento. Usa **⤺ riporta** nella finestra staccata.", "Ein Systemfenster zurück auf die Registerleiste zu ziehen, **ist in keinem Browser möglich** — der Browser sieht dieses Ziehen nicht. Nutzen Sie **⤺ zurückgeben** im gelösten Fenster.", "Arrastrar una ventana del sistema de vuelta a la barra de pestañas **no es posible** en ningún navegador — el navegador no ve ese arrastre. Use **⤺ devolver** en la ventana separada."),

    // -------------------------------------------- a tela de ajuda do modo
    texto!("tela.mt_titulo", "Multitela", "Multi-écran", "Multi-screen", "Multischermo", "Mehrbildschirm", "Multipantalla"),
    texto!("tela.mt_subtitulo", "abas vivas, regiões lado a lado e janelas destacadas", "onglets vivants, régions côte à côte et fenêtres détachées", "live tabs, side-by-side regions and detached windows", "schede vive, regioni affiancate e finestre staccate", "lebende Registerkarten, nebeneinanderliegende Regionen und gelöste Fenster", "pestañas vivas, regiones lado a lado y ventanas separadas"),
    texto!("tela.mt_regioes_abertas", "regiões abertas", "régions ouvertes", "open regions", "regioni aperte", "offene Regionen", "regiones abiertas"),
    texto!("tela.mt_cabem", "cabem {n}", "il en tient {n}", "{n} fit", "ne stanno {n}", "{n} passen", "caben {n}"),
    texto!("tela.mt_abas_vivas", "abas vivas", "onglets vivants", "live tabs", "schede vive", "lebende Registerkarten", "pestañas vivas"),
    texto!("tela.mt_pinadas", "{n} pinada(s)", "{n} épinglée(s)", "{n} pinned", "{n} fissata/e", "{n} angeheftet", "{n} fijada(s)"),
    texto!("tela.mt_largura_util", "largura útil", "largeur utile", "usable width", "larghezza utile", "nutzbare Breite", "ancho útil"),
    texto!("tela.mt_pixels_css", "pixels CSS", "pixels CSS", "CSS pixels", "pixel CSS", "CSS-Pixel", "píxeles CSS"),
    texto!("tela.mt_densidade", "densidade desta janela", "densité de cette fenêtre", "density of this window", "densità di questa finestra", "Dichte dieses Fensters", "densidad de esta ventana"),
    texto!("tela.mt_os_monitores", "Os monitores", "Les écrans", "The monitors", "I monitor", "Die Monitore", "Los monitores"),
    texto!("tela.mt_col_monitor", "monitor", "écran", "monitor", "monitor", "Monitor", "monitor"),
    texto!("tela.mt_col_tamanho", "tamanho", "taille", "size", "dimensione", "Größe", "tamaño"),
    texto!("tela.mt_col_canto", "canto", "coin", "corner", "angolo", "Ecke", "esquina"),
    texto!("tela.mt_principal", "principal", "principal", "primary", "principale", "primär", "principal"),
    texto!("tela.mt_emendas", "{n} emenda(s) física(s) dentro desta janela, a {onde} da borda esquerda da área de trabalho.", "{n} jointure(s) physique(s) dans cette fenêtre, à {onde} du bord gauche du bureau.", "{n} physical seam(s) inside this window, at {onde} from the left edge of the desktop.", "{n} giuntura/e fisica/che dentro questa finestra, a {onde} dal bordo sinistro della scrivania.", "{n} physische Naht/Nähte in diesem Fenster, {onde} vom linken Rand des Desktops.", "{n} junta(s) física(s) dentro de esta ventana, a {onde} del borde izquierdo del escritorio."),
    texto!("tela.mt_emendas_alinhar", "**Alinhar** põe uma calha em cada uma, para nenhuma região ficar partida ao meio.", "**Aligner** place une gouttière sur chacune, pour qu'aucune région ne soit coupée en deux.", "**Align** puts a gutter on each one, so no region is cut in half.", "**Allinea** mette una canalina su ciascuna, perché nessuna regione resti tagliata a metà.", "**Ausrichten** setzt an jede eine Rinne, damit keine Region in der Mitte zerschnitten wird.", "**Alinear** pone un canal en cada una, para que ninguna región quede partida por la mitad."),
    texto!("tela.mt_um_monitor_inteiro", "Esta janela está inteira dentro de um monitor só.", "Cette fenêtre tient entièrement dans un seul écran.", "This window sits entirely inside a single monitor.", "Questa finestra sta interamente dentro un solo monitor.", "Dieses Fenster liegt vollständig in einem einzigen Monitor.", "Esta ventana está entera dentro de un solo monitor."),
    texto!("tela.mt_alinhar_bt", "Alinhar as regiões com os monitores", "Aligner les régions sur les écrans", "Align the regions with the monitors", "Allinea le regioni ai monitor", "Regionen an den Monitoren ausrichten", "Alinear las regiones con los monitores"),
    texto!("tela.mt_sem_monitores", "**Este navegador não expõe os monitores.**", "**Ce navigateur n'expose pas les écrans.**", "**This browser does not expose the monitors.**", "**Questo browser non espone i monitor.**", "**Dieser Browser gibt die Monitore nicht preis.**", "**Este navegador no expone los monitores.**"),
    texto!("tela.mt_sem_monitores2", "A `Window Management API` (`getScreenDetails`) existe no Chrome e no Edge, em contexto seguro — e `127.0.0.1` é contexto seguro.", "La `Window Management API` (`getScreenDetails`) existe dans Chrome et Edge, en contexte sécurisé — et `127.0.0.1` est un contexte sécurisé.", "The `Window Management API` (`getScreenDetails`) exists in Chrome and Edge, in a secure context — and `127.0.0.1` is a secure context.", "La `Window Management API` (`getScreenDetails`) esiste in Chrome ed Edge, in contesto sicuro — e `127.0.0.1` è contesto sicuro.", "Die `Window Management API` (`getScreenDetails`) gibt es in Chrome und Edge, in sicherem Kontext — und `127.0.0.1` ist sicherer Kontext.", "La `Window Management API` (`getScreenDetails`) existe en Chrome y Edge, en contexto seguro — y `127.0.0.1` es contexto seguro."),
    texto!("tela.mt_sem_monitores3", "No Firefox e no Safari ela não existe: as regiões dividem em partes iguais, e a janela destacada abre onde o navegador quiser.", "Dans Firefox et Safari elle n'existe pas : les régions se partagent en parts égales, et la fenêtre détachée s'ouvre où le navigateur veut.", "In Firefox and Safari it does not exist: the regions split into equal parts, and the detached window opens wherever the browser likes.", "In Firefox e Safari non esiste: le regioni si dividono in parti uguali, e la finestra staccata si apre dove vuole il browser.", "In Firefox und Safari gibt es sie nicht: die Regionen teilen sich gleichmäßig, und das gelöste Fenster öffnet, wo der Browser will.", "En Firefox y Safari no existe: las regiones se dividen en partes iguales, y la ventana separada abre donde el navegador quiera."),
    texto!("tela.mt_nao_faz", "O que este modo NÃO faz", "Ce que ce mode NE fait PAS", "What this mode does NOT do", "Ciò che questa modalità NON fa", "Was dieser Modus NICHT tut", "Lo que este modo NO hace"),
    texto!("tela.mt_nao_faz_docking", "**Arrastar uma janela do sistema de volta para a barra de abas.**", "**Faire glisser une fenêtre du système jusqu'à la barre d'onglets.**", "**Dragging a system window back onto the tab strip.**", "**Trascinare una finestra di sistema di nuovo sulla barra delle schede.**", "**Ein Systemfenster zurück auf die Registerleiste ziehen.**", "**Arrastrar una ventana del sistema de vuelta a la barra de pestañas.**"),
    texto!("tela.mt_nao_faz_docking2", "O navegador não recebe evento nenhum quando uma janela passa por cima de outra — o docking por arrasto do WINDEV(R) e do Visual Studio(R) não é implementável aqui.", "Le navigateur ne reçoit aucun événement quand une fenêtre passe au-dessus d'une autre — l'ancrage par glisser de WINDEV(R) et de Visual Studio(R) n'est pas implémentable ici.", "The browser receives no event at all when one window passes over another — the drag docking of WINDEV(R) and Visual Studio(R) cannot be implemented here.", "Il browser non riceve alcun evento quando una finestra passa sopra un'altra — il docking a trascinamento di WINDEV(R) e Visual Studio(R) non è implementabile qui.", "Der Browser erhält kein Ereignis, wenn ein Fenster über ein anderes zieht — das Drag-Docking von WINDEV(R) und Visual Studio(R) ist hier nicht umsetzbar.", "El navegador no recibe ningún evento cuando una ventana pasa por encima de otra — el acoplamiento por arrastre de WINDEV(R) y Visual Studio(R) no es implementable aquí."),
    texto!("tela.mt_use_devolver", "Use **⤺ devolver**, na janela destacada.", "Utilisez **⤺ renvoyer**, dans la fenêtre détachée.", "Use **⤺ send back**, in the detached window.", "Usa **⤺ riporta**, nella finestra staccata.", "Nutzen Sie **⤺ zurückgeben**, im gelösten Fenster.", "Use **⤺ devolver**, en la ventana separada."),
    texto!("tela.mt_nao_faz_reabrir", "**Reabrir sozinho as janelas destacadas.**", "**Rouvrir toutes seules les fenêtres détachées.**", "**Reopening the detached windows on its own.**", "**Riaprire da sole le finestre staccate.**", "**Die gelösten Fenster von selbst wieder öffnen.**", "**Reabrir solas las ventanas separadas.**"),
    texto!("tela.mt_nao_faz_reabrir2", "`window.open` sem clique é bloqueio de popup em todo navegador. O arranjo fica guardado; volta com um clique.", "`window.open` sans clic, c'est un blocage de pop-up dans tous les navigateurs. La disposition reste enregistrée ; elle revient d'un clic.", "`window.open` without a click is a pop-up block in every browser. The arrangement is stored; it comes back with one click.", "`window.open` senza clic è blocco popup in ogni browser. La disposizione resta salvata; torna con un clic.", "`window.open` ohne Klick ist in jedem Browser eine Pop-up-Blockade. Die Anordnung bleibt gespeichert; sie kommt mit einem Klick zurück.", "`window.open` sin clic es bloqueo de popup en todo navegador. La disposición queda guardada; vuelve con un clic."),
    texto!("tela.mt_nao_faz_sessao", "**Guardar a sessão no disco do navegador.**", "**Enregistrer la session sur le disque du navigateur.**", "**Storing the session on the browser's disk.**", "**Salvare la sessione sul disco del browser.**", "**Die Sitzung auf der Browser-Festplatte speichern.**", "**Guardar la sesión en el disco del navegador.**"),
    texto!("tela.mt_nao_faz_sessao2", "A ficha de sessão viaja pelo `BroadcastChannel`, em memória. Se a janela principal fechar, a destacada pede login — e isso é de propósito.", "La fiche de session voyage par `BroadcastChannel`, en mémoire. Si la fenêtre principale se ferme, la détachée redemande la connexion — et c'est voulu.", "The session record travels over `BroadcastChannel`, in memory. If the main window closes, the detached one asks for sign-in — and that is on purpose.", "La scheda di sessione viaggia su `BroadcastChannel`, in memoria. Se la finestra principale si chiude, quella staccata chiede l'accesso — ed è di proposito.", "Der Sitzungsdatensatz reist über `BroadcastChannel`, im Speicher. Schließt das Hauptfenster, verlangt das gelöste eine Anmeldung — und das ist Absicht.", "La ficha de sesión viaja por `BroadcastChannel`, en memoria. Si la ventana principal se cierra, la separada pide inicio de sesión — y eso es a propósito."),
    texto!("tela.mt_rodape", "Regiões, larguras e abas pinadas ficam **neste navegador** — não no servidor. Desenho completo em `docs/MULTITELA.md`.", "Régions, largeurs et onglets épinglés restent **dans ce navigateur** — pas sur le serveur. Conception complète dans `docs/MULTITELA.md`.", "Regions, widths and pinned tabs stay **in this browser** — not on the server. Full design in `docs/MULTITELA.md`.", "Regioni, larghezze e schede fissate restano **in questo browser** — non sul server. Progetto completo in `docs/MULTITELA.md`.", "Regionen, Breiten und angeheftete Registerkarten bleiben **in diesem Browser** — nicht auf dem Server. Vollständiger Entwurf in `docs/MULTITELA.md`.", "Regiones, anchos y pestañas fijadas quedan **en este navegador** — no en el servidor. Diseño completo en `docs/MULTITELA.md`."),

    // ============================================ o diagrama ER (`diagrama-er.js`)
    texto!("tela.er_fora", "→ {tabela} (fora)", "→ {tabela} (hors)", "→ {tabela} (outside)", "→ {tabela} (fuori)", "→ {tabela} (außerhalb)", "→ {tabela} (fuera)"),
    texto!("tela.er_diagrama", "Diagrama de entidades e relacionamentos", "Diagramme entités-associations", "Entity-relationship diagram", "Diagramma entità-relazioni", "Entity-Relationship-Diagramm", "Diagrama entidad-relación"),

    // ================================================= a grade (`grid/phx-grid.js`)
    // O rodapé, o seletor de colunas e a paginação.
    texto!("tela.gr_itens_por_pagina", "itens por página", "éléments par page", "items per page", "elementi per pagina", "Einträge pro Seite", "elementos por página"),
    texto!("tela.gr_exportar_dica", "baixa o que está na tela: estas colunas, este filtro, esta ordem", "télécharge ce qui est à l'écran : ces colonnes, ce filtre, cet ordre", "downloads what is on screen: these columns, this filter, this order", "scarica ciò che è a schermo: queste colonne, questo filtro, questo ordine", "lädt herunter, was am Bildschirm steht: diese Spalten, dieser Filter, diese Sortierung", "descarga lo que está en la pantalla: estas columnas, este filtro, este orden"),
    texto!("tela.gr_exportar_vista", "⤓ Exportar a vista", "⤓ Exporter la vue", "⤓ Export the view", "⤓ Esporta la vista", "⤓ Ansicht exportieren", "⤓ Exportar la vista"),
    texto!("tela.gr_pagina_de", "Página {p} de {tp} ({n} registros)", "Page {p} sur {tp} ({n} enregistrements)", "Page {p} of {tp} ({n} records)", "Pagina {p} di {tp} ({n} record)", "Seite {p} von {tp} ({n} Datensätze)", "Página {p} de {tp} ({n} registros)"),
    texto!("tela.gr_ir_para", "ir para", "aller à", "go to", "vai a", "gehe zu", "ir a"),
    texto!("tela.gr_colunas_conta", "Colunas: {n} ▾", "Colonnes : {n} ▾", "Columns: {n} ▾", "Colonne: {n} ▾", "Spalten: {n} ▾", "Columnas: {n} ▾"),
    texto!("tela.gr_congelada", "congelada — clique para soltar", "figée — cliquez pour libérer", "frozen — click to release", "bloccata — clicca per liberare", "fixiert — zum Lösen klicken", "congelada — haga clic para soltar"),
    texto!("tela.gr_congelar", "congelar à esquerda", "figer à gauche", "freeze to the left", "blocca a sinistra", "links fixieren", "congelar a la izquierda"),

    // A caixa de grupos.
    texto!("tela.gr_crescente", "crescente — clique para inverter", "croissant — cliquez pour inverser", "ascending — click to reverse", "crescente — clicca per invertire", "aufsteigend — zum Umkehren klicken", "ascendente — haga clic para invertir"),
    texto!("tela.gr_decrescente", "decrescente — clique para inverter", "décroissant — cliquez pour inverser", "descending — click to reverse", "decrescente — clicca per invertire", "absteigend — zum Umkehren klicken", "descendente — haga clic para invertir"),
    texto!("tela.gr_desagrupar", "desagrupar", "dégrouper", "ungroup", "separa", "Gruppierung aufheben", "desagrupar"),
    texto!("tela.gr_expandir_tudo", "expandir tudo", "tout déplier", "expand all", "espandi tutto", "alle aufklappen", "expandir todo"),
    texto!("tela.gr_recolher_tudo", "recolher tudo", "tout replier", "collapse all", "comprimi tutto", "alle einklappen", "contraer todo"),
    texto!("tela.gr_total_grupo_dica", "mostra o total embaixo de cada grupo", "affiche le total sous chaque groupe", "shows the total under each group", "mostra il totale sotto ogni gruppo", "zeigt die Summe unter jeder Gruppe", "muestra el total debajo de cada grupo"),
    texto!("tela.gr_total_por_grupo", "total por grupo", "total par groupe", "total per group", "totale per gruppo", "Summe je Gruppe", "total por grupo"),
    texto!("tela.gr_total_geral", "total geral", "total général", "grand total", "totale generale", "Gesamtsumme", "total general"),

    // A barra de filtros ativos.
    texto!("tela.gr_filtros_ativos", "Filtros Ativos ({n})", "Filtres actifs ({n})", "Active Filters ({n})", "Filtri attivi ({n})", "Aktive Filter ({n})", "Filtros activos ({n})"),
    texto!("tela.gr_remover", "remover", "supprimer", "remove", "rimuovi", "entfernen", "quitar"),
    texto!("tela.gr_limpar_todos", "Limpar Todos", "Tout effacer", "Clear All", "Cancella tutti", "Alle löschen", "Borrar todos"),

    // O painel de filtro de uma coluna.
    texto!("tela.gr_ordenar_az", "Classificar de A a Z", "Trier de A à Z", "Sort A to Z", "Ordina dalla A alla Z", "Von A bis Z sortieren", "Ordenar de A a Z"),
    texto!("tela.gr_ordenar_za", "Classificar de Z a A", "Trier de Z à A", "Sort Z to A", "Ordina dalla Z alla A", "Von Z bis A sortieren", "Ordenar de Z a A"),
    texto!("tela.gr_limpar_filtro", "Limpar Filtro", "Effacer le filtre", "Clear Filter", "Cancella filtro", "Filter löschen", "Borrar filtro"),
    texto!("tela.gr_sem_distintos", "fonte remota sem suporte a valores distintos", "source distante sans prise en charge des valeurs distinctes", "remote source with no support for distinct values", "sorgente remota senza supporto ai valori distinti", "entfernte Quelle ohne Unterstützung für eindeutige Werte", "fuente remota sin soporte a valores distintos"),
    texto!("tela.gr_pesquisar", "Pesquisar", "Rechercher", "Search", "Cerca", "Suchen", "Buscar"),
    texto!("tela.gr_selecionar_tudo", "(Selecionar Tudo)", "(Tout sélectionner)", "(Select All)", "(Seleziona tutto)", "(Alle auswählen)", "(Seleccionar todo)"),
    texto!("tela.gr_exibir_sem_valor", "Exibir itens sem valor", "Afficher les éléments sans valeur", "Show items with no value", "Mostra elementi senza valore", "Einträge ohne Wert anzeigen", "Mostrar elementos sin valor"),
    texto!("tela.gr_filtros_numero", "Filtros de Número", "Filtres numériques", "Number Filters", "Filtri numerici", "Zahlenfilter", "Filtros de número"),
    texto!("tela.gr_e", "E", "ET", "AND", "E", "UND", "Y"),
    texto!("tela.gr_ou", "OU", "OU", "OR", "O", "ODER", "O"),
    texto!("tela.gr_mostrando", "mostrando {vis} de {total} — refine a pesquisa", "affichage de {vis} sur {total} — affinez la recherche", "showing {vis} of {total} — refine the search", "mostrando {vis} di {total} — affina la ricerca", "{vis} von {total} angezeigt — Suche eingrenzen", "mostrando {vis} de {total} — afine la búsqueda"),

    // O cabeçalho de coluna e a linha de filtro rápido.
    texto!("tela.gr_alternar_agregador", "alternar agregador", "changer d'agrégat", "switch aggregate", "cambia aggregatore", "Aggregat wechseln", "cambiar agregador"),
    texto!("tela.gr_filtrar", "filtrar", "filtrer", "filter", "filtra", "filtern", "filtrar"),
    texto!("tela.gr_valor", "valor", "valeur", "value", "valore", "Wert", "valor"),
    texto!("tela.gr_selecionar", "Selecionar", "Sélectionner", "Select", "Seleziona", "Auswählen", "Seleccionar"),

    // Os operadores do filtro de numero e o rodape da busca. Achados
    // EXERCITANDO: com a grade em espanhol, o painel do filtro ainda trazia
    // «é maior que» em portugues -- eles estao escritos com `\uXXXX` dentro
    // de um array, e nenhuma das duas vias do conferidor ve isso.
    texto!("tela.gr_op_maior", "é maior que", "est supérieur à", "is greater than", "è maggiore di", "ist größer als", "es mayor que"),
    texto!("tela.gr_op_maior_ig", "é maior ou igual a", "est supérieur ou égal à", "is greater than or equal to", "è maggiore o uguale a", "ist größer oder gleich", "es mayor o igual que"),
    texto!("tela.gr_op_menor", "é menor que", "est inférieur à", "is less than", "è minore di", "ist kleiner als", "es menor que"),
    texto!("tela.gr_op_menor_ig", "é menor ou igual a", "est inférieur ou égal à", "is less than or equal to", "è minore o uguale a", "ist kleiner oder gleich", "es menor o igual que"),
    texto!("tela.gr_op_igual", "é igual a", "est égal à", "is equal to", "è uguale a", "ist gleich", "es igual a"),
    texto!("tela.gr_op_diferente", "é diferente de", "est différent de", "is not equal to", "è diverso da", "ist ungleich", "es distinto de"),
    texto!("tela.gr_arraste", "Arraste uma coluna para cá para agrupar", "Faites glisser une colonne ici pour grouper", "Drag a column here to group by it", "Trascina qui una colonna per raggruppare", "Ziehen Sie eine Spalte hierher, um zu gruppieren", "Arrastre una columna aquí para agrupar"),
    texto!("tela.gr_buscar_curto", "Buscar…", "Chercher…", "Search…", "Cerca…", "Suchen…", "Buscar…"),
    texto!("tela.gr_busca_tudo", "Buscar em tudo… (vários termos = E)", "Chercher partout… (plusieurs termes = ET)", "Search everything… (several terms = AND)", "Cerca ovunque… (più termini = E)", "Überall suchen… (mehrere Begriffe = UND)", "Buscar en todo… (varios términos = Y)"),
    texto!("tela.gr_resultados", "{n} resultado(s)", "{n} résultat(s)", "{n} result(s)", "{n} risultato/i", "{n} Ergebnis(se)", "{n} resultado(s)"),
    texto!("tela.gr_linhas_em_grupos", "{linhas} linhas em {niveis} nível(is) de grupo", "{linhas} lignes sur {niveis} niveau(x) de groupe", "{linhas} rows in {niveis} group level(s)", "{linhas} righe in {niveis} livello/i di gruppo", "{linhas} Zeilen in {niveis} Gruppenebene(n)", "{linhas} filas en {niveis} nivel(es) de grupo"),
    texto!("tela.gr_mostrando_de_ate", "Mostrando {de}–{ate} de {total}", "Affichage de {de}–{ate} sur {total}", "Showing {de}–{ate} of {total}", "Mostrando {de}–{ate} di {total}", "{de}–{ate} von {total} angezeigt", "Mostrando {de}–{ate} de {total}"),

    // ================================================== a telemetria (`telemetria.js`)
    // Os quatro níveis, e os três sinais de cada um: a palavra da cor, o
    // rótulo e o traço da borda. Vão pelo PAR (`rot:`/`txt:`) porque `NIVEIS`
    // é lido no arranque, antes de existir texto traduzido.
    texto!("tela.tl_nivel_normal", "normal", "normal", "normal", "normale", "normal", "normal"),
    texto!("tela.tl_nivel_alto", "uso alto", "usage élevé", "high use", "uso elevato", "hohe Last", "uso alto"),
    texto!("tela.tl_nivel_stress", "stress", "stress", "stress", "stress", "Stress", "estrés"),
    texto!("tela.tl_nivel_encerrando", "encerrando", "arrêt en cours", "stopping", "in chiusura", "wird beendet", "finalizando"),
    texto!("tela.tl_cor_azul", "azul", "bleu", "blue", "blu", "blau", "azul"),
    texto!("tela.tl_cor_amarelo", "amarelo", "jaune", "yellow", "giallo", "gelb", "amarillo"),
    texto!("tela.tl_cor_vermelho", "vermelho", "rouge", "red", "rosso", "rot", "rojo"),
    texto!("tela.tl_cor_rosa", "rosa", "rose", "pink", "rosa", "rosa", "rosa"),
    texto!("tela.tl_borda_cheia", "borda cheia", "bordure pleine", "solid border", "bordo pieno", "durchgezogener Rand", "borde continuo"),
    texto!("tela.tl_borda_tracejada", "borda tracejada", "bordure tiretée", "dashed border", "bordo tratteggiato", "gestrichelter Rand", "borde discontinuo"),
    texto!("tela.tl_borda_pontilhada", "borda pontilhada", "bordure pointillée", "dotted border", "bordo punteggiato", "gepunkteter Rand", "borde punteado"),
    texto!("tela.tl_borda_longa", "borda de traço longo", "bordure à longs tirets", "long-dash border", "bordo a trattini lunghi", "Rand mit langen Strichen", "borde de trazo largo"),

    // A barra do alto e o estado da coleta.
    texto!("tela.tl_pausar", "Pausar", "Mettre en pause", "Pause", "Pausa", "Anhalten", "Pausar"),
    texto!("tela.tl_agora", "Atualizar agora", "Actualiser maintenant", "Refresh now", "Aggiorna adesso", "Jetzt aktualisieren", "Actualizar ahora"),
    texto!("tela.tl_desligar", "Desligar coleta", "Arrêter la collecte", "Turn collection off", "Disattiva la raccolta", "Erfassung ausschalten", "Apagar la recolección"),
    texto!("tela.tl_ligar", "Ligar coleta", "Démarrer la collecte", "Turn collection on", "Attiva la raccolta", "Erfassung einschalten", "Encender la recolección"),
    texto!("tela.tl_coletando", "coletando", "collecte en cours", "collecting", "raccolta in corso", "erfasst", "recolectando"),
    texto!("tela.tl_coleta_off", "coleta desligada", "collecte arrêtée", "collection off", "raccolta disattivata", "Erfassung aus", "recolección apagada"),
    texto!("tela.tl_pausado", "pausado por você", "mis en pause par vous", "paused by you", "in pausa da parte tua", "von Ihnen angehalten", "pausado por usted"),
    texto!("tela.tl_ultima_amostra", "última amostra **{v}**", "dernier échantillon **{v}**", "last sample **{v}**", "ultimo campione **{v}**", "letzte Messung **{v}**", "última muestra **{v}**"),
    texto!("tela.tl_atraso", "atraso da amostra **{v}**", "retard de l'échantillon **{v}**", "sample delay **{v}**", "ritardo del campione **{v}**", "Verzögerung der Messung **{v}**", "retraso de la muestra **{v}**"),
    texto!("tela.tl_ida_e_volta", "ida e volta **{v}**", "aller-retour **{v}**", "round trip **{v}**", "andata e ritorno **{v}**", "Hin und zurück **{v}**", "ida y vuelta **{v}**"),
    texto!("tela.tl_periodo", "período **{v}**", "période **{v}**", "period **{v}**", "periodo **{v}**", "Intervall **{v}**", "período **{v}**"),
    texto!("tela.tl_em_stress", "servidor em stress · {por_que}", "serveur en stress · {por_que}", "server under stress · {por_que}", "server sotto stress · {por_que}", "Server unter Stress · {por_que}", "servidor en estrés · {por_que}"),
    texto!("tela.tl_sem_resposta", "sem resposta do servidor", "aucune réponse du serveur", "no answer from the server", "nessuna risposta dal server", "keine Antwort vom Server", "sin respuesta del servidor"),
    texto!("tela.tl_na_tela_de", "o que está na tela é de **{quando}**, há **{idade}**", "ce qui est à l'écran date de **{quando}**, il y a **{idade}**", "what is on screen is from **{quando}**, **{idade}** ago", "quello che è a schermo è di **{quando}**, **{idade}** fa", "was am Bildschirm steht, ist von **{quando}**, vor **{idade}**", "lo que está en la pantalla es de **{quando}**, hace **{idade}**"),
    texto!("tela.tl_nunca_respondeu", "nunca houve resposta nesta sessão", "aucune réponse dans cette session", "there has never been an answer in this session", "nessuna risposta in questa sessione", "in dieser Sitzung gab es nie eine Antwort", "nunca hubo respuesta en esta sesión"),
    texto!("tela.tl_tentativas", "{n} tentativa(s) sem resposta", "{n} tentative(s) sans réponse", "{n} attempt(s) with no answer", "{n} tentativo/i senza risposta", "{n} Versuch(e) ohne Antwort", "{n} intento(s) sin respuesta"),

    // As faixas de séries.
    texto!("tela.tl_pico", "pico {v}", "pic {v}", "peak {v}", "picco {v}", "Spitze {v}", "pico {v}"),
    texto!("tela.tl_fx_esperas", "Esperas — atividades por estado", "Attentes — activités par état", "Waits — activities by state", "Attese — attività per stato", "Wartezeiten — Aktivitäten nach Zustand", "Esperas — actividades por estado"),
    texto!("tela.tl_fx_esperas_v", "{n} na fila · a mais antiga há {ha}", "{n} dans la file · la plus ancienne il y a {ha}", "{n} in the queue · the oldest {ha} ago", "{n} in coda · la più vecchia da {ha}", "{n} in der Warteschlange · die älteste seit {ha}", "{n} en la cola · la más antigua hace {ha}"),
    texto!("tela.tl_fx_esperas_v0", "ninguém na fila · {ms} ms/s de espera", "personne dans la file · {ms} ms/s d'attente", "nobody in the queue · {ms} ms/s of wait", "nessuno in coda · {ms} ms/s di attesa", "niemand in der Warteschlange · {ms} ms/s Wartezeit", "nadie en la cola · {ms} ms/s de espera"),
    texto!("tela.tl_s_executando", "executando", "en exécution", "running", "in esecuzione", "läuft", "ejecutando"),
    texto!("tela.tl_s_esperando", "esperando", "en attente", "waiting", "in attesa", "wartet", "esperando"),
    texto!("tela.tl_s_encerrando", "encerrando", "arrêt en cours", "stopping", "in chiusura", "wird beendet", "finalizando"),
    texto!("tela.tl_s_ociosas", "ociosas", "inactives", "idle", "inattive", "untätig", "ociosas"),
    texto!("tela.tl_fx_disco", "Leitura e escrita físicas (deste processo)", "Lectures et écritures physiques (de ce processus)", "Physical reads and writes (of this process)", "Letture e scritture fisiche (di questo processo)", "Physische Lese- und Schreibvorgänge (dieses Prozesses)", "Lecturas y escrituras físicas (de este proceso)"),
    texto!("tela.tl_fx_disco_v", "{ler}/s ler · {gravar}/s gravar", "{ler}/s en lecture · {gravar}/s en écriture", "{ler}/s read · {gravar}/s write", "{ler}/s in lettura · {gravar}/s in scrittura", "{ler}/s lesen · {gravar}/s schreiben", "{ler}/s leer · {gravar}/s grabar"),
    texto!("tela.tl_s_lidos", "lidos", "lus", "read", "letti", "gelesen", "leídos"),
    texto!("tela.tl_s_gravados", "gravados", "écrits", "written", "scritti", "geschrieben", "grabados"),
    texto!("tela.tl_fx_cpu_v", "processo {p}% · máquina {m}%", "processus {p}% · machine {m}%", "process {p}% · machine {m}%", "processo {p}% · macchina {m}%", "Prozess {p}% · Maschine {m}%", "proceso {p}% · máquina {m}%"),
    texto!("tela.tl_s_processo", "processo", "processus", "process", "processo", "Prozess", "proceso"),
    texto!("tela.tl_s_maquina", "máquina", "machine", "machine", "macchina", "Maschine", "máquina"),
    texto!("tela.tl_fx_vazao", "Vazão — operações por segundo", "Débit — opérations par seconde", "Throughput — operations per second", "Portata — operazioni al secondo", "Durchsatz — Operationen pro Sekunde", "Caudal — operaciones por segundo"),
    texto!("tela.tl_fx_vazao_v", "{l} leitura/s · {e} escrita/s", "{l} lecture/s · {e} écriture/s", "{l} read/s · {e} write/s", "{l} lettura/s · {e} scrittura/s", "{l} Lesen/s · {e} Schreiben/s", "{l} lectura/s · {e} escritura/s"),
    texto!("tela.tl_s_leitura", "leitura", "lecture", "read", "lettura", "Lesen", "lectura"),
    texto!("tela.tl_s_escrita", "escrita", "écriture", "write", "scrittura", "Schreiben", "escritura"),
    texto!("tela.tl_s_erro", "erro", "erreur", "error", "errore", "Fehler", "error"),
    texto!("tela.tl_fx_cache", "Cache de páginas do .ndx", "Cache de pages du .ndx", ".ndx page cache", "Cache di pagine del .ndx", "Seiten-Cache der .ndx", "Caché de páginas del .ndx"),
    texto!("tela.tl_fx_cache_v", "{p}% de acerto · teto {teto} páginas", "{p}% de réussite · plafond {teto} pages", "{p}% hit rate · cap {teto} pages", "{p}% di successo · tetto {teto} pagine", "{p}% Treffer · Obergrenze {teto} Seiten", "{p}% de acierto · tope {teto} páginas"),
    texto!("tela.tl_fx_cache_v0", "sem toque de página ainda · teto {teto} páginas", "aucune page touchée pour l'instant · plafond {teto} pages", "no page touched yet · cap {teto} pages", "nessuna pagina toccata finora · tetto {teto} pagine", "noch keine Seite berührt · Obergrenze {teto} Seiten", "ninguna página tocada aún · tope {teto} páginas"),
    texto!("tela.tl_s_acertos", "acertos", "réussites", "hits", "successi", "Treffer", "aciertos"),
    texto!("tela.tl_s_faltas", "faltas", "échecs", "misses", "mancati", "Fehlgriffe", "fallos"),

    // O painel de bolhas: trilha, busca, legenda e escala.
    texto!("tela.tl_trilha", "onde você está", "où vous êtes", "where you are", "dove ti trovi", "wo Sie sind", "dónde está usted"),
    texto!("tela.tl_atividades", "Atividades", "Activités", "Activities", "Attività", "Aktivitäten", "Actividades"),
    texto!("tela.tl_por_estacao", "Por estação", "Par poste", "By workstation", "Per postazione", "Nach Arbeitsplatz", "Por estación"),
    texto!("tela.tl_busca_dica", "procurar conexão, IP, usuário ou operação…", "chercher connexion, IP, utilisateur ou opération…", "search connection, IP, user or operation…", "cerca connessione, IP, utente o operazione…", "Verbindung, IP, Benutzer oder Operation suchen…", "buscar conexión, IP, usuario u operación…"),
    texto!("tela.tl_busca_al", "procurar entre as atividades", "chercher parmi les activités", "search among the activities", "cerca tra le attività", "unter den Aktivitäten suchen", "buscar entre las actividades"),
    texto!("tela.tl_ocultar_legenda", "ocultar legenda", "masquer la légende", "hide legend", "nascondi la legenda", "Legende ausblenden", "ocultar leyenda"),
    texto!("tela.tl_mostrar_legenda", "mostrar legenda", "afficher la légende", "show legend", "mostra la legenda", "Legende einblenden", "mostrar leyenda"),
    texto!("tela.tl_bolhas_al", "atividades vivas, uma bolha por atividade", "activités vivantes, une bulle par activité", "live activities, one bubble per activity", "attività vive, una bolla per attività", "lebende Aktivitäten, eine Blase je Aktivität", "actividades vivas, una burbuja por actividad"),
    texto!("tela.tl_peso", "peso", "poids", "weight", "peso", "Gewicht", "peso"),
    texto!("tela.tl_escala_al", "escala: a área da bolha segue o peso", "échelle : l'aire de la bulle suit le poids", "scale: the bubble area follows the weight", "scala: l'area della bolla segue il peso", "Skala: die Fläche der Blase folgt dem Gewicht", "escala: el área de la burbuja sigue el peso"),
    texto!("tela.tl_escala_area", "a **área** segue o peso — milissegundos de servidor que a atividade já gastou.", "l'**aire** suit le poids — millisecondes de serveur déjà dépensées par l'activité.", "the **area** follows the weight — server milliseconds the activity has already spent.", "l'**area** segue il peso — millisecondi di server già spesi dall'attività.", "die **Fläche** folgt dem Gewicht — Server-Millisekunden, die die Aktivität schon verbraucht hat.", "el **área** sigue el peso — milisegundos de servidor que la actividad ya gastó."),
    texto!("tela.tl_escala_piso", "Escala reduzida; as mais leves têm piso, para o rótulo caber.", "Échelle réduite ; les plus légères ont un plancher, pour que l'étiquette tienne.", "Reduced scale; the lightest ones have a floor, so the label fits.", "Scala ridotta; le più leggere hanno un minimo, perché l'etichetta ci stia.", "Verkleinerte Skala; die leichtesten haben einen Mindestwert, damit die Beschriftung passt.", "Escala reducida; las más ligeras tienen un mínimo, para que quepa la etiqueta."),
    texto!("tela.tl_escala_max", "escala do peso: a área segue o peso; a mais pesada tem {maior}", "échelle du poids : l'aire suit le poids ; la plus lourde a {maior}", "weight scale: the area follows the weight; the heaviest has {maior}", "scala del peso: l'area segue il peso; la più pesante ha {maior}", "Gewichtsskala: die Fläche folgt dem Gewicht; die schwerste hat {maior}", "escala del peso: el área sigue el peso; la más pesada tiene {maior}"),
    texto!("tela.tl_escala_min", " e a mais leve, {menor}", " et la plus légère, {menor}", " and the lightest, {menor}", " e la più leggera, {menor}", " und die leichteste, {menor}", " y la más ligera, {menor}"),
    texto!("tela.tl_amostra_dica", "amostra da bolha {nivel}, contraste {razao} para 1", "échantillon de la bulle {nivel}, contraste {razao} pour 1", "sample of the {nivel} bubble, contrast {razao} to 1", "campione della bolla {nivel}, contrasto {razao} a 1", "Probe der Blase {nivel}, Kontrast {razao} zu 1", "muestra de la burbuja {nivel}, contraste {razao} a 1"),
    texto!("tela.tl_sem_ip", "sem IP", "sans IP", "no IP", "senza IP", "ohne IP", "sin IP"),
    texto!("tela.tl_conex", "{n} conex.", "{n} conn.", "{n} conn.", "{n} conn.", "{n} Verb.", "{n} conex."),
    texto!("tela.tl_ociosa", "ociosa", "inactive", "idle", "inattiva", "untätig", "ociosa"),
    texto!("tela.tl_quem_estacao", "estação {ip} · {quantas} conexão(ões), {executando} executando · peso {peso}", "poste {ip} · {quantas} connexion(s), {executando} en exécution · poids {peso}", "workstation {ip} · {quantas} connection(s), {executando} running · weight {peso}", "postazione {ip} · {quantas} connessione/i, {executando} in esecuzione · peso {peso}", "Arbeitsplatz {ip} · {quantas} Verbindung(en), {executando} laufend · Gewicht {peso}", "estación {ip} · {quantas} conexión(es), {executando} ejecutando · peso {peso}"),
    texto!("tela.tl_quem_atividade", "{id}{eu} · {nivel} · {sub} · peso {peso}", "{id}{eu} · {nivel} · {sub} · poids {peso}", "{id}{eu} · {nivel} · {sub} · weight {peso}", "{id}{eu} · {nivel} · {sub} · peso {peso}", "{id}{eu} · {nivel} · {sub} · Gewicht {peso}", "{id}{eu} · {nivel} · {sub} · peso {peso}"),
    texto!("tela.tl_sua_tela_par", " (a sua própria tela)", " (votre propre écran)", " (your own screen)", " (la tua schermata)", " (Ihr eigener Bildschirm)", " (su propia pantalla)"),
    texto!("tela.tl_clique_estacao", ", clique para ver as conexões desta estação", ", cliquez pour voir les connexions de ce poste", ", click to see this workstation's connections", ", clicca per vedere le connessioni di questa postazione", ", zum Anzeigen der Verbindungen dieses Arbeitsplatzes klicken", ", haga clic para ver las conexiones de esta estación"),
    texto!("tela.tl_clique_descritivo", ", clique para o descritivo completo", ", cliquez pour le descriptif complet", ", click for the full description", ", clicca per la descrizione completa", ", zum Anzeigen der vollen Beschreibung klicken", ", haga clic para la descripción completa"),
    texto!("tela.tl_nada_no_filtro", "nenhuma atividade bate com o filtro", "aucune activité ne correspond au filtre", "no activity matches the filter", "nessuna attività corrisponde al filtro", "keine Aktivität passt zum Filter", "ninguna actividad coincide con el filtro"),
    texto!("tela.tl_nada_viva", "nenhuma atividade viva neste instante", "aucune activité vivante en ce moment", "no live activity right now", "nessuna attività viva in questo momento", "gerade keine lebende Aktivität", "ninguna actividad viva en este instante"),

    // O resumo do painel.
    texto!("tela.tl_r_vivas", "{vivas} viva(s) · {exec} executando", "{vivas} vivante(s) · {exec} en exécution", "{vivas} live · {exec} running", "{vivas} viva/e · {exec} in esecuzione", "{vivas} lebend · {exec} laufend", "{vivas} viva(s) · {exec} ejecutando"),
    texto!("tela.tl_r_estacoes", "{n} estação(ões)", "{n} poste(s)", "{n} workstation(s)", "{n} postazione/i", "{n} Arbeitsplatz/-plätze", "{n} estación(es)"),
    texto!("tela.tl_r_mais_pesada", "a mais pesada aqui: {quem} ({peso})", "la plus lourde ici : {quem} ({peso})", "the heaviest here: {quem} ({peso})", "la più pesante qui: {quem} ({peso})", "die schwerste hier: {quem} ({peso})", "la más pesada aquí: {quem} ({peso})"),
    texto!("tela.tl_r_fora_filtro", "{n} fora do filtro", "{n} hors du filtre", "{n} outside the filter", "{n} fuori dal filtro", "{n} außerhalb des Filters", "{n} fuera del filtro"),
    texto!("tela.tl_r_fora_desenho", "{n} mais leve(s) fora do desenho", "{n} plus légère(s) hors du dessin", "{n} lighter one(s) outside the drawing", "{n} più leggera/e fuori dal disegno", "{n} leichtere außerhalb der Zeichnung", "{n} más ligera(s) fuera del dibujo"),
    texto!("tela.tl_r_encerramentos", "{n} encerramento(s) desde que subiu", "{n} arrêt(s) depuis le démarrage", "{n} termination(s) since startup", "{n} chiusura/e dall'avvio", "{n} Beendigung(en) seit dem Start", "{n} finalización(es) desde el arranque"),

    // As quatro faixas da legenda, escritas com os limiares do servidor.
    texto!("tela.tl_fxn_normal", "abaixo dos limiares, ou sem operação", "sous les seuils, ou sans opération", "below the thresholds, or with no operation", "sotto le soglie, o senza operazione", "unter den Schwellen oder ohne Operation", "por debajo de los umbrales, o sin operación"),
    texto!("tela.tl_fxn_alto", "operação acima de {t}, ou parada na fila da trava", "opération au-delà de {t}, ou arrêtée dans la file du verrou", "operation above {t}, or stalled in the lock queue", "operazione oltre {t}, o ferma nella coda del lock", "Operation über {t} oder in der Sperren-Warteschlange", "operación por encima de {t}, o detenida en la cola del bloqueo"),
    texto!("tela.tl_fxn_alto0", "operação longa, ou parada na fila da trava", "opération longue, ou arrêtée dans la file du verrou", "long operation, or stalled in the lock queue", "operazione lunga, o ferma nella coda del lock", "lange Operation oder in der Sperren-Warteschlange", "operación larga, o detenida en la cola del bloqueo"),
    texto!("tela.tl_fxn_stress", "trabalhando há mais de {t}, ou segurando a trava com fila", "au travail depuis plus de {t}, ou tenant le verrou avec une file", "working for more than {t}, or holding the lock with a queue", "al lavoro da più di {t}, o con il lock in mano e la coda", "arbeitet seit mehr als {t} oder hält die Sperre mit Warteschlange", "trabajando desde hace más de {t}, o sujetando el bloqueo con cola"),
    texto!("tela.tl_fxn_stress0", "trabalhando demais, ou segurando a trava com fila", "trop de travail, ou tenant le verrou avec une file", "working too long, or holding the lock with a queue", "al lavoro da troppo, o con il lock in mano e la coda", "arbeitet zu lange oder hält die Sperre mit Warteschlange", "trabajando demasiado, o sujetando el bloqueo con cola"),
    texto!("tela.tl_fxn_encerrando", "marcada, esperando o ponto seguro", "marquée, en attente du point sûr", "marked, waiting for the safe point", "contrassegnata, in attesa del punto sicuro", "markiert, wartet auf den sicheren Punkt", "marcada, esperando el punto seguro"),

    // O cartão do descritivo.
    texto!("tela.tl_cartao_vazio", "nenhuma atividade aqui — quando houver, clique numa bolha para ver o descritivo completo", "aucune activité ici — quand il y en aura, cliquez sur une bulle pour le descriptif complet", "no activity here — when there is one, click a bubble for the full description", "nessuna attività qui — quando ce ne sarà, clicca su una bolla per la descrizione completa", "keine Aktivität hier — wenn es eine gibt, klicken Sie auf eine Blase für die volle Beschreibung", "ninguna actividad aquí — cuando haya, haga clic en una burbuja para la descripción completa"),
    texto!("tela.tl_clique_bolha", "clique numa bolha para ver o descritivo completo", "cliquez sur une bulle pour voir le descriptif complet", "click a bubble to see the full description", "clicca su una bolla per la descrizione completa", "auf eine Blase klicken für die volle Beschreibung", "haga clic en una burbuja para ver la descripción completa"),
    texto!("tela.tl_sua_tela", "esta é a sua tela", "c'est votre écran", "this is your screen", "questa è la tua schermata", "das ist Ihr Bildschirm", "esta es su pantalla"),
    texto!("tela.tl_mais_pesada", "a mais pesada agora", "la plus lourde maintenant", "the heaviest right now", "la più pesante adesso", "die schwerste gerade jetzt", "la más pesada ahora"),
    texto!("tela.tl_sim", "sim", "oui", "yes", "sì", "ja", "sí"),
    texto!("tela.tl_nao", "não", "non", "no", "no", "nein", "no"),
    texto!("tela.tl_c_estado", "estado", "état", "state", "stato", "Zustand", "estado"),
    texto!("tela.tl_c_nivel", "nível", "niveau", "level", "livello", "Stufe", "nivel"),
    texto!("tela.tl_c_operacao", "operação em curso", "opération en cours", "operation under way", "operazione in corso", "laufende Operation", "operación en curso"),
    texto!("tela.tl_c_sem_op", "nenhuma em curso", "aucune en cours", "none under way", "nessuna in corso", "keine im Gange", "ninguna en curso"),
    texto!("tela.tl_c_alvo", "alvo", "cible", "target", "obiettivo", "Ziel", "objetivo"),
    texto!("tela.tl_c_fase", "fase", "phase", "phase", "fase", "Phase", "fase"),
    texto!("tela.tl_c_usuario", "usuário", "utilisateur", "user", "utente", "Benutzer", "usuario"),
    texto!("tela.tl_c_origem", "origem", "origine", "origin", "origine", "Herkunft", "origen"),
    texto!("tela.tl_c_estacao", "estação (IP)", "poste (IP)", "workstation (IP)", "postazione (IP)", "Arbeitsplatz (IP)", "estación (IP)"),
    texto!("tela.tl_c_conexao", "conexão", "connexion", "connection", "connessione", "Verbindung", "conexión"),
    texto!("tela.tl_c_numero", "nº {n}", "nº {n}", "no. {n}", "n. {n}", "Nr. {n}", "n.º {n}"),
    texto!("tela.tl_c_desde", "conectada desde", "connectée depuis", "connected since", "connessa da", "verbunden seit", "conectada desde"),
    texto!("tela.tl_c_aberta_ha", "aberta há", "ouverte depuis", "open for", "aperta da", "offen seit", "abierta hace"),
    texto!("tela.tl_c_op_inicio", "operação iniciada", "opération démarrée", "operation started", "operazione iniziata", "Operation gestartet", "operación iniciada"),
    texto!("tela.tl_c_op_dura", "operação dura há", "opération dure depuis", "operation running for", "operazione dura da", "Operation läuft seit", "operación dura hace"),
    texto!("tela.tl_c_trabalhando", "desse tempo, trabalhando", "de ce temps, au travail", "of that time, working", "di quel tempo, al lavoro", "davon arbeitend", "de ese tiempo, trabajando"),
    texto!("tela.tl_c_na_fila", "desse tempo, na fila da trava", "de ce temps, dans la file du verrou", "of that time, in the lock queue", "di quel tempo, nella coda del lock", "davon in der Sperren-Warteschlange", "de ese tiempo, en la cola del bloqueo"),
    texto!("tela.tl_c_peso", "peso (servidor gasto)", "poids (serveur dépensé)", "weight (server spent)", "peso (server speso)", "Gewicht (verbrauchter Server)", "peso (servidor gastado)"),
    texto!("tela.tl_c_pedidos", "pedidos já feitos", "requêtes déjà faites", "requests made so far", "richieste già fatte", "bisher gestellte Anfragen", "peticiones ya hechas"),
    texto!("tela.tl_c_passos", "unidades percorridas", "unités parcourues", "units gone through", "unità percorse", "durchlaufene Einheiten", "unidades recorridas"),
    texto!("tela.tl_c_trava", "trava de dados", "verrou de données", "data lock", "lock dei dati", "Datensperre", "bloqueo de datos"),
    texto!("tela.tl_c_trava_sim", "na mão desta atividade", "aux mains de cette activité", "held by this activity", "in mano a questa attività", "in der Hand dieser Aktivität", "en manos de esta actividad"),
    texto!("tela.tl_c_esperando", "esperando", "en attente de", "waiting for", "in attesa di", "wartet auf", "esperando"),
    texto!("tela.tl_c_tem_ponto", "tem ponto de cancelamento", "a un point d'annulation", "has a cancellation point", "ha un punto di annullamento", "hat einen Abbruchpunkt", "tiene punto de cancelación"),
    texto!("tela.tl_c_cancelavel", "cancelável neste instante", "annulable à cet instant", "cancellable right now", "annullabile in questo istante", "gerade jetzt abbrechbar", "cancelable en este instante"),
    texto!("tela.tl_c_marcada", "marcada para encerrar", "marquée pour arrêt", "marked to stop", "contrassegnata per la chiusura", "zum Beenden markiert", "marcada para finalizar"),
    texto!("tela.tl_c_ja_encerrada", "já encerrada", "déjà arrêtée", "already stopped", "già chiusa", "schon beendet", "ya finalizada"),
    texto!("tela.tl_c_vezes", "{n} vez(es)", "{n} fois", "{n} time(s)", "{n} volta/e", "{n} Mal", "{n} vez/veces"),

    // Os botões e a nota do cartão.
    texto!("tela.tl_encerrar", "Encerrar a operação", "Arrêter l'opération", "Stop the operation", "Chiudi l'operazione", "Operation beenden", "Finalizar la operación"),
    texto!("tela.tl_encerrar_off_clique", "clique na bolha para poder encerrá-la", "cliquez sur la bulle pour pouvoir l'arrêter", "click the bubble to be able to stop it", "clicca sulla bolla per poterla chiudere", "auf die Blase klicken, um sie beenden zu können", "haga clic en la burbuja para poder finalizarla"),
    texto!("tela.tl_encerrar_off_ja", "já está encerrando", "elle s'arrête déjà", "it is already stopping", "sta già chiudendo", "sie wird bereits beendet", "ya está finalizando"),
    texto!("tela.tl_encerrar_off_sem_ponto", "esta operação não tem ponto de cancelamento: vai terminar", "cette opération n'a pas de point d'annulation : elle ira jusqu'au bout", "this operation has no cancellation point: it will finish", "questa operazione non ha punto di annullamento: arriverà in fondo", "diese Operation hat keinen Abbruchpunkt: sie läuft zu Ende", "esta operación no tiene punto de cancelación: va a terminar"),
    texto!("tela.tl_encerrar_off_sem_op", "não há operação em curso", "aucune opération en cours", "there is no operation under way", "non c'è nessuna operazione in corso", "es läuft keine Operation", "no hay operación en curso"),
    texto!("tela.tl_derrubar", "Derrubar a conexão", "Couper la connexion", "Drop the connection", "Chiudi la connessione", "Verbindung trennen", "Cortar la conexión"),
    texto!("tela.tl_derrubar_pergunta", "Derrubar a conexão {n}? O soquete fecha e o cliente perde a resposta.", "Couper la connexion {n} ? La socket se ferme et le client perd la réponse.", "Drop connection {n}? The socket closes and the client loses the answer.", "Chiudere la connessione {n}? Il socket si chiude e il client perde la risposta.", "Verbindung {n} trennen? Der Socket schließt und der Client verliert die Antwort.", "¿Cortar la conexión {n}? El socket se cierra y el cliente pierde la respuesta."),
    texto!("tela.tl_conexao_encerrada", "conexão encerrada", "connexion fermée", "connection closed", "connessione chiusa", "Verbindung geschlossen", "conexión cerrada"),
    texto!("tela.tl_ver_estacao", "Ver as {n} desta estação", "Voir les {n} de ce poste", "See the {n} of this workstation", "Vedi le {n} di questa postazione", "Die {n} dieses Arbeitsplatzes ansehen", "Ver las {n} de esta estación"),
    texto!("tela.tl_nota_auto", "esta é a atividade mais pesada agora, mostrada sem ninguém ter pedido. **Clique na bolha dela** para poder encerrá-la — derrubar o trabalho de outra pessoa exige escolha explícita.", "c'est l'activité la plus lourde en ce moment, montrée sans que personne l'ait demandée. **Cliquez sur sa bulle** pour pouvoir l'arrêter — couper le travail d'autrui exige un choix explicite.", "this is the heaviest activity right now, shown without anyone asking. **Click its bubble** to be able to stop it — dropping someone else's work requires an explicit choice.", "questa è l'attività più pesante adesso, mostrata senza che nessuno l'abbia chiesta. **Clicca sulla sua bolla** per poterla chiudere — buttare giù il lavoro di un altro richiede una scelta esplicita.", "das ist gerade die schwerste Aktivität, gezeigt ohne dass jemand danach gefragt hat. **Klicken Sie auf ihre Blase**, um sie beenden zu können — fremde Arbeit abzubrechen verlangt eine ausdrückliche Wahl.", "esta es la actividad más pesada ahora, mostrada sin que nadie la pidiera. **Haga clic en su burbuja** para poder finalizarla — cortar el trabajo de otra persona exige una elección explícita."),
    texto!("tela.tl_nota_encerrando", "encerrando… a operação aborta no próximo ponto seguro.", "arrêt en cours… l'opération s'interrompt au prochain point sûr.", "stopping… the operation aborts at the next safe point.", "in chiusura… l'operazione si interrompe al prossimo punto sicuro.", "wird beendet… die Operation bricht am nächsten sicheren Punkt ab.", "finalizando… la operación aborta en el próximo punto seguro."),
    texto!("tela.tl_nota_sem_op", "sem operação em curso. Derrubar a conexão fecha o soquete — é o «kill» de sempre.", "aucune opération en cours. Couper la connexion ferme la socket — c'est le « kill » habituel.", "no operation under way. Dropping the connection closes the socket — it is the usual «kill».", "nessuna operazione in corso. Chiudere la connessione chiude il socket — è il solito «kill».", "keine Operation im Gange. Die Verbindung zu trennen schließt den Socket — das übliche «kill».", "sin operación en curso. Cortar la conexión cierra el socket — es el «kill» de siempre."),
    texto!("tela.tl_nota_cancelavel", "cancelável agora: a marca é lida entre duas unidades de trabalho, e o que já foi gravado fica gravado.", "annulable maintenant : la marque est lue entre deux unités de travail, et ce qui est déjà écrit reste écrit.", "cancellable now: the mark is read between two units of work, and what was written stays written.", "annullabile adesso: il contrassegno è letto tra due unità di lavoro, e ciò che è già scritto resta scritto.", "jetzt abbrechbar: die Markierung wird zwischen zwei Arbeitseinheiten gelesen, und was geschrieben ist, bleibt geschrieben.", "cancelable ahora: la marca se lee entre dos unidades de trabajo, y lo que ya se grabó queda grabado."),
    texto!("tela.tl_nota_tem_ponto", "esta operação tem ponto de cancelamento, mas não está nele neste instante — tipicamente porque espera a trava de dados. A marca vale para o primeiro ponto seguro que vier.", "cette opération a un point d'annulation, mais elle n'y est pas à cet instant — typiquement parce qu'elle attend le verrou. La marque vaudra au premier point sûr venu.", "this operation has a cancellation point but is not at it right now — typically because it awaits the data lock. The mark applies at the first safe point that comes.", "questa operazione ha un punto di annullamento, ma non ci si trova adesso — di solito perché attende il lock. Il contrassegno vale al primo punto sicuro che arriva.", "diese Operation hat einen Abbruchpunkt, ist aber gerade nicht dort — meist weil sie auf die Datensperre wartet. Die Markierung gilt am ersten sicheren Punkt.", "esta operación tiene punto de cancelación, pero no está en él ahora — típicamente porque espera el bloqueo. La marca vale en el primer punto seguro que llegue."),
    texto!("tela.tl_nota_sem_ponto", "não cancelável: a operação não tem ponto de cancelamento e vai terminar. Abandonar uma gravação no meio deixaria o arquivo mentindo.", "non annulable : l'opération n'a pas de point d'annulation et ira jusqu'au bout. Abandonner une écriture en cours laisserait le fichier menteur.", "not cancellable: the operation has no cancellation point and will finish. Abandoning a write halfway would leave the file lying.", "non annullabile: l'operazione non ha punto di annullamento e arriverà in fondo. Abbandonare una scrittura a metà lascerebbe il file bugiardo.", "nicht abbrechbar: die Operation hat keinen Abbruchpunkt und läuft zu Ende. Ein Schreibvorgang mittendrin abgebrochen würde die Datei lügen lassen.", "no cancelable: la operación no tiene punto de cancelación y va a terminar. Abandonar una grabación a medias dejaría el archivo mintiendo."),

    // A tabela de threads.
    texto!("tela.tl_gestor_threads", "Gestor de threads", "Gestionnaire de threads", "Thread manager", "Gestore dei thread", "Thread-Verwaltung", "Gestor de hilos"),
    texto!("tela.tl_th_vivas", "{vivas} viva(s) de {total} registrada(s)", "{vivas} vivante(s) sur {total} enregistrée(s)", "{vivas} live of {total} registered", "{vivas} viva/e su {total} registrate", "{vivas} lebend von {total} registrierten", "{vivas} viva(s) de {total} registradas"),
    texto!("tela.tl_th_thread", "thread", "thread", "thread", "thread", "Thread", "hilo"),
    texto!("tela.tl_th_familia", "família", "famille", "family", "famiglia", "Familie", "familia"),
    texto!("tela.tl_th_finalidade", "finalidade", "finalité", "purpose", "finalità", "Zweck", "finalidad"),
    texto!("tela.tl_th_fazendo", "fazendo agora", "en train de faire", "doing now", "sta facendo", "macht gerade", "haciendo ahora"),
    texto!("tela.tl_th_voltas", "voltas", "tours", "loops", "giri", "Runden", "vueltas"),
    texto!("tela.tl_th_viva_ha", "viva há", "vivante depuis", "alive for", "viva da", "lebt seit", "viva hace"),
    texto!("tela.tl_th_encerrada", "encerrada", "arrêtée", "stopped", "chiusa", "beendet", "finalizada"),

    // ============================================ a tela da Claude (`claude.js`)
    // O nome de cada modelo NAO entra aqui: ele e nome de produto, e esta nos
    // ISENTOS do conferidor com a razao escrita. O que entra e a explicacao de
    // custo ao lado dele, que e rotulo.
    texto!("tela.ia_modelo_capaz", "o mais capaz — o padrão", "le plus capable — celui par défaut", "the most capable — the default", "il più capace — quello predefinito", "der leistungsfähigste — die Vorgabe", "el más capaz — el predeterminado"),
    texto!("tela.ia_modelo_medio", "intermediário, custa menos", "intermédiaire, coûte moins", "in-between, costs less", "intermedio, costa meno", "mittlere Stufe, kostet weniger", "intermedio, cuesta menos"),
    texto!("tela.ia_modelo_barato", "o mais barato e o mais rápido", "le moins cher et le plus rapide", "the cheapest and the fastest", "il più economico e il più veloce", "der günstigste und schnellste", "el más barato y el más rápido"),

    // A tela de configuração.
    texto!("tela.ia_titulo", "Integração com a Claude", "Intégration avec Claude", "Claude integration", "Integrazione con Claude", "Claude-Integration", "Integración con Claude"),
    texto!("tela.ia_subtitulo", "a chave é sua e fica neste navegador · o servidor PhxSql não participa", "la clé est à vous et reste dans ce navigateur · le serveur PhxSql n'y participe pas", "the key is yours and stays in this browser · the PhxSql server takes no part", "la chiave è tua e resta in questo browser · il server PhxSql non partecipa", "der Schlüssel gehört Ihnen und bleibt in diesem Browser · der PhxSql-Server ist nicht beteiligt", "la clave es suya y queda en este navegador · el servidor PhxSql no participa"),
    texto!("tela.ia_leia", "**Leia antes de ligar.** Esta tela liga o Centro de Controle direto na API da Anthropic, **do seu navegador**. Em português claro:", "**À lire avant d'activer.** Cet écran relie le Centre de Contrôle directement à l'API d'Anthropic, **depuis votre navigateur**. En clair :", "**Read before turning it on.** This screen connects the Control Center straight to Anthropic's API, **from your browser**. In plain words:", "**Leggi prima di attivare.** Questa schermata collega il Centro di Controllo direttamente all'API di Anthropic, **dal tuo browser**. In parole chiare:", "**Vor dem Einschalten lesen.** Dieser Bildschirm verbindet das Kontrollzentrum direkt mit der API von Anthropic, **aus Ihrem Browser**. Im Klartext:", "**Lea antes de activar.** Esta pantalla conecta el Centro de Control directamente a la API de Anthropic, **desde su navegador**. En claro:"),
    texto!("tela.ia_leia_chave", "a chave fica guardada **neste navegador** (no `localStorage`), e não no servidor — quem usar o console de outra máquina precisa da própria chave;", "la clé est gardée **dans ce navigateur** (dans le `localStorage`), pas sur le serveur — qui utilise la console depuis une autre machine a besoin de sa propre clé ;", "the key is kept **in this browser** (in `localStorage`), not on the server — whoever uses the console from another machine needs their own key;", "la chiave resta **in questo browser** (nel `localStorage`), non sul server — chi usa la console da un'altra macchina ha bisogno della propria chiave;", "der Schlüssel bleibt **in diesem Browser** (im `localStorage`), nicht auf dem Server — wer die Konsole von einem anderen Rechner nutzt, braucht einen eigenen;", "la clave se guarda **en este navegador** (en `localStorage`), no en el servidor — quien use la consola desde otra máquina necesita su propia clave;"),
    texto!("tela.ia_leia_sobe", "as suas perguntas e o contexto que você mandar (o **esquema** do banco, e as linhas se você marcar) **vão para a Anthropic**, que é uma empresa de fora;", "vos questions et le contexte envoyé (le **schéma** de la base, et les lignes si vous cochez) **partent chez Anthropic**, une entreprise extérieure ;", "your questions and the context you send (the database **schema**, and the rows if you tick the box) **go to Anthropic**, an outside company;", "le tue domande e il contesto che mandi (lo **schema** del database, e le righe se spunti) **vanno ad Anthropic**, un'azienda esterna;", "Ihre Fragen und der gesendete Kontext (das **Schema** der Datenbank, und die Zeilen bei gesetztem Haken) **gehen zu Anthropic**, einer fremden Firma;", "sus preguntas y el contexto que envíe (el **esquema** de la base, y las filas si lo marca) **van a Anthropic**, una empresa de fuera;"),
    texto!("tela.ia_leia_servidor", "o servidor PhxSql **não participa**: ele nunca vê a chave, nunca faz a chamada e não guarda nada disto;", "le serveur PhxSql **n'y participe pas** : il ne voit jamais la clé, ne fait jamais l'appel et ne garde rien de tout cela ;", "the PhxSql server **takes no part**: it never sees the key, never makes the call and keeps none of this;", "il server PhxSql **non partecipa**: non vede mai la chiave, non fa mai la chiamata e non conserva nulla di questo;", "der PhxSql-Server **ist nicht beteiligt**: er sieht den Schlüssel nie, ruft nie auf und speichert nichts davon;", "el servidor PhxSql **no participa**: nunca ve la clave, nunca hace la llamada y no guarda nada de esto;"),
    texto!("tela.ia_leia_custo", "o **custo é seu**, na sua conta da Anthropic. Cada resposta mostra os tokens que consumiu.", "le **coût est pour vous**, sur votre compte Anthropic. Chaque réponse affiche les jetons consommés.", "the **cost is yours**, on your Anthropic account. Every answer shows the tokens it used.", "il **costo è tuo**, sul tuo account Anthropic. Ogni risposta mostra i token che ha consumato.", "die **Kosten sind Ihre**, auf Ihrem Anthropic-Konto. Jede Antwort zeigt die verbrauchten Token.", "el **coste es suyo**, en su cuenta de Anthropic. Cada respuesta muestra los tokens que consumió."),
    texto!("tela.ia_chave", "Chave da API", "Clé de l'API", "API key", "Chiave dell'API", "API-Schlüssel", "Clave de la API"),
    texto!("tela.ia_chave_guardada", "guardada — digite para trocar", "enregistrée — tapez pour la changer", "saved — type to replace it", "salvata — digita per cambiarla", "gespeichert — zum Ersetzen tippen", "guardada — escriba para cambiarla"),
    texto!("tela.ia_chave_fim", "Há uma chave guardada, terminada em `{fim}`.", "Une clé est enregistrée, terminée par `{fim}`.", "There is a key saved, ending in `{fim}`.", "C'è una chiave salvata, che finisce in `{fim}`.", "Es ist ein Schlüssel gespeichert, endend auf `{fim}`.", "Hay una clave guardada, terminada en `{fim}`."),
    texto!("tela.ia_sem_chave", "Ainda não há chave guardada neste navegador.", "Aucune clé enregistrée dans ce navigateur pour l'instant.", "There is no key saved in this browser yet.", "Non c'è ancora nessuna chiave salvata in questo browser.", "In diesem Browser ist noch kein Schlüssel gespeichert.", "Todavía no hay clave guardada en este navegador."),
    texto!("tela.ia_modelo", "Modelo", "Modèle", "Model", "Modello", "Modell", "Modelo"),
    texto!("tela.ia_modelo_leg", "A escolha é de **custo**: os três respondem, e o mais capaz cobra mais por token.", "Le choix est une question de **coût** : les trois répondent, et le plus capable coûte plus par jeton.", "The choice is about **cost**: all three answer, and the most capable charges more per token.", "La scelta è di **costo**: tutti e tre rispondono, e il più capace costa di più per token.", "Die Wahl ist eine **Kostenfrage**: alle drei antworten, und der leistungsfähigste kostet mehr je Token.", "La elección es de **coste**: los tres responden, y el más capaz cobra más por token."),
    texto!("tela.ia_ligada", "Ligada — mostrar os botões da Claude na tela de Query", "Activée — afficher les boutons de Claude sur l'écran Query", "On — show Claude's buttons on the Query screen", "Attiva — mostra i pulsanti di Claude nella schermata Query", "Ein — die Claude-Schaltflächen im Query-Bildschirm zeigen", "Activada — mostrar los botones de Claude en la pantalla de Query"),
    texto!("tela.ia_endereco", "Endereço da API:", "Adresse de l'API :", "API address:", "Indirizzo dell'API:", "Adresse der API:", "Dirección de la API:"),
    texto!("tela.ia_oficial", "oficial", "officielle", "official", "ufficiale", "offiziell", "oficial"),
    texto!("tela.ia_nao_oficial", "NÃO é o oficial", "CE N'EST PAS l'officielle", "NOT the official one", "NON è quello ufficiale", "NICHT die offizielle", "NO es la oficial"),
    texto!("tela.ia_testar", "Testar a chave", "Tester la clé", "Test the key", "Prova la chiave", "Schlüssel testen", "Probar la clave"),
    texto!("tela.ia_remover", "Remover a chave deste navegador", "Retirer la clé de ce navigateur", "Remove the key from this browser", "Rimuovi la chiave da questo browser", "Schlüssel aus diesem Browser entfernen", "Quitar la clave de este navegador"),
    texto!("tela.ia_nota_docs", "O desenho e o porquê estão em `docs/CLAUDE-IA.md`.", "La conception et le pourquoi sont dans `docs/CLAUDE-IA.md`.", "The design and the why are in `docs/CLAUDE-IA.md`.", "Il disegno e il perché stanno in `docs/CLAUDE-IA.md`.", "Entwurf und Begründung stehen in `docs/CLAUDE-IA.md`.", "El diseño y el porqué están en `docs/CLAUDE-IA.md`."),
    texto!("tela.ia_nota_tls", "Em uma frase: a API é HTTPS obrigatório, a `std` do Rust não tem TLS, e a casa não acrescenta dependência — então a chamada sai do navegador, e o servidor fica de fora do caminho inteiro.", "En une phrase : l'API impose HTTPS, la `std` de Rust n'a pas de TLS, et la maison n'ajoute pas de dépendance — l'appel part donc du navigateur, et le serveur reste hors du chemin entier.", "In one sentence: the API requires HTTPS, Rust's `std` has no TLS, and this house adds no dependency — so the call leaves from the browser, and the server stays out of the whole path.", "In una frase: l'API impone HTTPS, la `std` di Rust non ha TLS, e la casa non aggiunge dipendenze — così la chiamata parte dal browser, e il server resta fuori dall'intero percorso.", "In einem Satz: die API verlangt HTTPS, Rusts `std` hat kein TLS, und das Haus fügt keine Abhängigkeit hinzu — der Aufruf geht vom Browser aus, und der Server bleibt ganz aus dem Weg.", "En una frase: la API exige HTTPS, la `std` de Rust no tiene TLS, y la casa no añade dependencias — así la llamada sale del navegador, y el servidor queda fuera de todo el camino."),
    texto!("tela.ia_salva", "integração com a Claude salva neste navegador", "intégration avec Claude enregistrée dans ce navigateur", "Claude integration saved in this browser", "integrazione con Claude salvata in questo browser", "Claude-Integration in diesem Browser gespeichert", "integración con Claude guardada en este navegador"),
    texto!("tela.ia_removida", "chave removida deste navegador", "clé retirée de ce navigateur", "key removed from this browser", "chiave rimossa da questo browser", "Schlüssel aus diesem Browser entfernt", "clave quitada de este navegador"),
    texto!("tela.ia_sem_chave_testar", "Não há chave para testar.", "Aucune clé à tester.", "There is no key to test.", "Non c'è nessuna chiave da provare.", "Es gibt keinen Schlüssel zum Testen.", "No hay clave para probar."),
    texto!("tela.ia_testando", "testando…", "test en cours…", "testing…", "prova in corso…", "wird getestet…", "probando…"),
    texto!("tela.ia_chave_ok", "A chave funciona. A API respondeu «{resposta}» · {entrada} token(s) de entrada, {saida} de saída.", "La clé fonctionne. L'API a répondu « {resposta} » · {entrada} jeton(s) en entrée, {saida} en sortie.", "The key works. The API answered «{resposta}» · {entrada} input token(s), {saida} output.", "La chiave funziona. L'API ha risposto «{resposta}» · {entrada} token in ingresso, {saida} in uscita.", "Der Schlüssel funktioniert. Die API antwortete «{resposta}» · {entrada} Eingabe-Token, {saida} Ausgabe.", "La clave funciona. La API respondió «{resposta}» · {entrada} token(s) de entrada, {saida} de salida."),

    // As quatro receitas e o formulário da tela de Query.
    texto!("tela.ia_r_sql", "Texto → SQL", "Texte → SQL", "Text → SQL", "Testo → SQL", "Text → SQL", "Texto → SQL"),
    texto!("tela.ia_r_sql_pede", "Descreva em português o que você quer consultar", "Décrivez en français ce que vous voulez interroger", "Describe in English what you want to query", "Descrivi in italiano che cosa vuoi interrogare", "Beschreiben Sie auf Deutsch, was Sie abfragen wollen", "Describa en español qué quiere consultar"),
    texto!("tela.ia_r_sql_ex", "os dez últimos clientes cadastrados", "les dix derniers clients enregistrés", "the last ten customers registered", "gli ultimi dieci clienti registrati", "die zehn zuletzt eingetragenen Kunden", "los diez últimos clientes registrados"),
    texto!("tela.ia_r_explicar", "Explicar o SQL", "Expliquer le SQL", "Explain the SQL", "Spiega l'SQL", "SQL erklären", "Explicar el SQL"),
    texto!("tela.ia_r_explicar_pede", "Cole a consulta que você quer entender", "Collez la requête que vous voulez comprendre", "Paste the query you want to understand", "Incolla la query che vuoi capire", "Fügen Sie die Abfrage ein, die Sie verstehen wollen", "Pegue la consulta que quiere entender"),
    texto!("tela.ia_r_indice", "Índice / desempenho", "Index / performance", "Index / performance", "Indice / prestazioni", "Index / Leistung", "Índice / rendimiento"),
    texto!("tela.ia_r_indice_pede", "Cole a consulta que está lenta", "Collez la requête qui est lente", "Paste the query that is slow", "Incolla la query che è lenta", "Fügen Sie die langsame Abfrage ein", "Pegue la consulta que está lenta"),
    texto!("tela.ia_r_modelar", "Modelar tabelas", "Modéliser des tables", "Model tables", "Modella tabelle", "Tabellen modellieren", "Modelar tablas"),
    texto!("tela.ia_r_modelar_pede", "Descreva o negócio a modelar", "Décrivez le métier à modéliser", "Describe the business to model", "Descrivi l'attività da modellare", "Beschreiben Sie den zu modellierenden Geschäftsfall", "Describa el negocio a modelar"),
    texto!("tela.ia_r_modelar_ex", "uma loja com clientes, pedidos e itens de pedido", "une boutique avec clients, commandes et lignes de commande", "a shop with customers, orders and order items", "un negozio con clienti, ordini e righe d'ordine", "ein Laden mit Kunden, Bestellungen und Bestellpositionen", "una tienda con clientes, pedidos y líneas de pedido"),
    texto!("tela.ia_perguntar_bt", "✦ Perguntar à Claude", "✦ Demander à Claude", "✦ Ask Claude", "✦ Chiedi a Claude", "✦ Claude fragen", "✦ Preguntar a Claude"),
    texto!("tela.ia_perguntar_leg", "o SQL gerado **não executa sozinho** — ele cai no editor abaixo, e quem aperta Executar é você", "le SQL généré **ne s'exécute pas tout seul** — il arrive dans l'éditeur ci-dessous, et c'est vous qui appuyez sur Exécuter", "the generated SQL **does not run by itself** — it lands in the editor below, and you are the one who presses Run", "l'SQL generato **non si esegue da solo** — finisce nell'editor qui sotto, e sei tu a premere Esegui", "das erzeugte SQL **läuft nicht von selbst** — es landet im Editor unten, und Sie drücken auf Ausführen", "el SQL generado **no se ejecuta solo** — cae en el editor de abajo, y quien pulsa Ejecutar es usted"),
    texto!("tela.ia_modelo_em_uso", "modelo: `{m}`", "modèle : `{m}`", "model: `{m}`", "modello: `{m}`", "Modell: `{m}`", "modelo: `{m}`"),
    texto!("tela.ia_db_contexto", "Database do contexto", "Base du contexte", "Context database", "Database del contesto", "Datenbank des Kontexts", "Base del contexto"),
    texto!("tela.ia_mandar_esquema", "Mandar o **esquema** deste banco (nomes de tabela, de coluna, tipos, chaves) — é o que faz a resposta acertar", "Envoyer le **schéma** de cette base (noms de table, de colonne, types, clés) — c'est ce qui fait juste la réponse", "Send this database's **schema** (table names, column names, types, keys) — it is what makes the answer right", "Mandare lo **schema** di questo database (nomi di tabella, di colonna, tipi, chiavi) — è ciò che fa azzeccare la risposta", "Das **Schema** dieser Datenbank senden (Tabellen- und Spaltennamen, Typen, Schlüssel) — davon hängt ab, ob die Antwort stimmt", "Enviar el **esquema** de esta base (nombres de tabla, de columna, tipos, claves) — es lo que hace acertar la respuesta"),
    texto!("tela.ia_mandar_linhas", "Mandar também **linhas de exemplo** de cada tabela", "Envoyer aussi des **lignes d'exemple** de chaque table", "Also send **sample rows** from each table", "Mandare anche **righe di esempio** di ogni tabella", "Auch **Beispielzeilen** jeder Tabelle senden", "Enviar también **filas de ejemplo** de cada tabla"),
    texto!("tela.ia_quantas", "quantas por tabela:", "combien par table :", "how many per table:", "quante per tabella:", "wie viele je Tabelle:", "cuántas por tabla:"),
    texto!("tela.ia_ver_envio", "Ver o que vai subir", "Voir ce qui va partir", "See what will be sent", "Vedi che cosa parte", "Sehen, was hochgeht", "Ver lo que va a subir"),
    texto!("tela.ia_perguntar", "Perguntar", "Demander", "Ask", "Chiedi", "Fragen", "Preguntar"),
    texto!("tela.ia_editor", "Editor — o SQL cai aqui, e quem executa é você", "Éditeur — le SQL arrive ici, et c'est vous qui l'exécutez", "Editor — the SQL lands here, and you are the one who runs it", "Editor — l'SQL finisce qui, e sei tu a eseguirlo", "Editor — das SQL landet hier, und Sie führen es aus", "Editor — el SQL cae aquí, y quien lo ejecuta es usted"),
    texto!("tela.ia_executar", "Executar", "Exécuter", "Run", "Esegui", "Ausführen", "Ejecutar"),
    texto!("tela.ia_sem_clique", "nada roda sem este clique", "rien ne tourne sans ce clic", "nothing runs without this click", "niente parte senza questo clic", "ohne diesen Klick läuft nichts", "nada corre sin este clic"),
    texto!("tela.ia_dado_sai", "**O dado sai desta máquina.** Marcada, esta caixa manda linhas de verdade das tabelas para a Anthropic.", "**La donnée quitte cette machine.** Cochée, cette case envoie de vraies lignes des tables chez Anthropic.", "**The data leaves this machine.** Ticked, this box sends real rows from the tables to Anthropic.", "**Il dato esce da questa macchina.** Spuntata, questa casella manda righe vere delle tabelle ad Anthropic.", "**Die Daten verlassen diesen Rechner.** Angehakt sendet dieses Feld echte Zeilen der Tabellen an Anthropic.", "**El dato sale de esta máquina.** Marcada, esta casilla manda filas reales de las tablas a Anthropic."),
    texto!("tela.ia_dado_sai2", "Coluna marcada como **dado pessoal** no esquema vai redigida (`{redigido}`), mas o resto vai como está. Marque só se você pode fazer isso com este banco.", "Une colonne marquée **donnée personnelle** part caviardée (`{redigido}`), mais le reste part tel quel. Ne cochez que si vous avez le droit de le faire avec cette base.", "A column marked **personal data** goes redacted (`{redigido}`), but the rest goes as it is. Tick only if you may do this with this database.", "Una colonna marcata **dato personale** parte oscurata (`{redigido}`), ma il resto parte com'è. Spunta solo se puoi farlo con questo database.", "Eine als **personenbezogen** markierte Spalte geht geschwärzt (`{redigido}`), der Rest wie er ist. Nur ankreuzen, wenn Sie das mit dieser Datenbank dürfen.", "Una columna marcada como **dato personal** va redactada (`{redigido}`), pero el resto va tal cual. Marque solo si puede hacer esto con esta base."),
    texto!("tela.ia_montando", "montando o contexto…", "assemblage du contexte…", "building the context…", "costruzione del contesto…", "Kontext wird zusammengestellt…", "montando el contexto…"),
    texto!("tela.ia_sem_esquema", "Não deu para ler o esquema: {erro}", "Impossible de lire le schéma : {erro}", "Could not read the schema: {erro}", "Non è stato possibile leggere lo schema: {erro}", "Das Schema ließ sich nicht lesen: {erro}", "No se pudo leer el esquema: {erro}"),
    texto!("tela.ia_ainda_vazio", "(ainda vazio)", "(encore vide)", "(still empty)", "(ancora vuoto)", "(noch leer)", "(aún vacío)"),
    texto!("tela.ia_sem_chave_curto", "(sem chave)", "(sans clé)", "(no key)", "(senza chiave)", "(ohne Schlüssel)", "(sin clave)"),
    texto!("tela.ia_sem_banco", "Você pediu para mandar o esquema, mas nenhum database está escolhido — então **nenhum esquema vai subir** e a resposta vai chutar os nomes das colunas. Escreva o database no campo acima.", "Vous avez demandé d'envoyer le schéma, mais aucune base n'est choisie — donc **aucun schéma ne partira** et la réponse devinera les noms des colonnes. Écrivez la base dans le champ ci-dessus.", "You asked to send the schema, but no database is chosen — so **no schema will be sent** and the answer will guess the column names. Write the database in the field above.", "Hai chiesto di mandare lo schema, ma nessun database è scelto — quindi **nessuno schema parte** e la risposta tirerà a indovinare i nomi delle colonne. Scrivi il database nel campo sopra.", "Sie wollten das Schema senden, aber keine Datenbank ist gewählt — also **geht kein Schema hoch** und die Antwort rät die Spaltennamen. Tragen Sie die Datenbank oben ein.", "Pidió mandar el esquema, pero ninguna base está elegida — así que **no subirá ningún esquema** y la respuesta adivinará los nombres de las columnas. Escriba la base en el campo de arriba."),
    texto!("tela.ia_vai_subir", "**O que vai subir para a Anthropic** — {tabelas} tabela(s) de esquema, {linhas} linha(s) de exemplo, {redigidas} valor(es) redigido(s) por serem dado pessoal.", "**Ce qui va partir chez Anthropic** — {tabelas} table(s) de schéma, {linhas} ligne(s) d'exemple, {redigidas} valeur(s) caviardée(s) car donnée personnelle.", "**What will be sent to Anthropic** — {tabelas} schema table(s), {linhas} sample row(s), {redigidas} value(s) redacted for being personal data.", "**Che cosa parte per Anthropic** — {tabelas} tabella/e di schema, {linhas} riga/he di esempio, {redigidas} valore/i oscurato/i perché dato personale.", "**Was zu Anthropic hochgeht** — {tabelas} Schema-Tabelle(n), {linhas} Beispielzeile(n), {redigidas} Wert(e) geschwärzt, weil personenbezogen.", "**Lo que va a subir a Anthropic** — {tabelas} tabla(s) de esquema, {linhas} fila(s) de ejemplo, {redigidas} valor(es) redactado(s) por ser dato personal."),
    texto!("tela.ia_escreva", "Escreva a pergunta primeiro.", "Écrivez d'abord la question.", "Write the question first.", "Scrivi prima la domanda.", "Schreiben Sie zuerst die Frage.", "Escriba primero la pregunta."),
    texto!("tela.ia_resposta", "Resposta", "Réponse", "Answer", "Risposta", "Antwort", "Respuesta"),
    texto!("tela.ia_respondeu", "O que a Claude respondeu", "Ce que Claude a répondu", "What Claude answered", "Che cosa ha risposto Claude", "Was Claude geantwortet hat", "Lo que respondió Claude"),
    texto!("tela.ia_tokens", "entrada {entrada} · saída {saida} token(s)", "entrée {entrada} · sortie {saida} jeton(s)", "input {entrada} · output {saida} token(s)", "ingresso {entrada} · uscita {saida} token", "Eingabe {entrada} · Ausgabe {saida} Token", "entrada {entrada} · salida {saida} token(s)"),
    texto!("tela.ia_tokens_fim", "entrada **{entrada}** · saída **{saida}** token(s) — o custo é da sua conta", "entrée **{entrada}** · sortie **{saida}** jeton(s) — le coût est sur votre compte", "input **{entrada}** · output **{saida}** token(s) — the cost is on your account", "ingresso **{entrada}** · uscita **{saida}** token — il costo è sul tuo account", "Eingabe **{entrada}** · Ausgabe **{saida}** Token — die Kosten gehen auf Ihr Konto", "entrada **{entrada}** · salida **{saida}** token(s) — el coste es de su cuenta"),

    // A revisão do plano, antes de qualquer escrita.
    texto!("tela.ia_revisao", "Revisão do plano — nada foi criado ainda", "Revue du plan — rien n'a encore été créé", "Plan review — nothing has been created yet", "Revisione del piano — non è ancora stato creato nulla", "Prüfung des Plans — noch wurde nichts angelegt", "Revisión del plan — todavía no se ha creado nada"),
    texto!("tela.ia_propos", "**A Claude propôs; quem cria é você.** Confira item por item e desmarque o que não quiser. Ao confirmar, o PhxSql vai criar", "**Claude a proposé ; c'est vous qui créez.** Vérifiez point par point et décochez ce que vous ne voulez pas. À la confirmation, PhxSql créera", "**Claude proposed; you are the one who creates.** Check item by item and untick what you do not want. On confirming, PhxSql will create", "**Claude ha proposto; sei tu a creare.** Controlla voce per voce e togli la spunta a ciò che non vuoi. Confermando, PhxSql creerà", "**Claude hat vorgeschlagen; anlegen tun Sie.** Prüfen Sie Punkt für Punkt und haken Sie ab, was Sie nicht wollen. Beim Bestätigen legt PhxSql an", "**Claude propuso; quien crea es usted.** Revise punto por punto y desmarque lo que no quiera. Al confirmar, PhxSql creará"),
    texto!("tela.ia_n_tabelas", "{n} tabela(s)", "{n} table(s)", "{n} table(s)", "{n} tabella/e", "{n} Tabelle(n)", "{n} tabla(s)"),
    texto!("tela.ia_n_fks", "{n} relacionamento(s)", "{n} association(s)", "{n} relationship(s)", "{n} relazione/i", "{n} Beziehung(en)", "{n} relación(es)"),
    texto!("tela.ia_e", "e", "et", "and", "e", "und", "y"),
    texto!("tela.ia_no_banco", "no banco `{db}`.", "dans la base `{db}`.", "in database `{db}`.", "nel database `{db}`.", "in der Datenbank `{db}`.", "en la base `{db}`."),
    texto!("tela.ia_travadas", "{n} tabela(s) do plano **não podem ser criadas** e ficaram travadas abaixo, com o motivo. Nada é sobrescrito em silêncio.", "{n} table(s) du plan **ne peuvent pas être créées** et sont bloquées ci-dessous, avec la raison. Rien n'est écrasé en silence.", "{n} table(s) of the plan **cannot be created** and are locked below, with the reason. Nothing is overwritten in silence.", "{n} tabella/e del piano **non possono essere create** e restano bloccate qui sotto, col motivo. Nulla è sovrascritto in silenzio.", "{n} Tabelle(n) des Plans **können nicht angelegt werden** und sind unten gesperrt, mit Begründung. Nichts wird stillschweigend überschrieben.", "{n} tabla(s) del plan **no pueden crearse** y quedaron bloqueadas abajo, con el motivo. Nada se sobrescribe en silencio."),
    texto!("tela.ia_n_colunas", "{n} coluna(s)", "{n} colonne(s)", "{n} column(s)", "{n} colonna/e", "{n} Spalte(n)", "{n} columna(s)"),
    texto!("tela.ia_n_indices", "{n} índice(s)", "{n} index", "{n} index(es)", "{n} indice/i", "{n} Index/Indizes", "{n} índice(s)"),
    texto!("tela.ia_col_coluna", "coluna", "colonne", "column", "colonna", "Spalte", "columna"),
    texto!("tela.ia_col_tipo", "tipo", "type", "type", "tipo", "Typ", "tipo"),
    texto!("tela.ia_col_obrig", "obrig.", "oblig.", "req.", "obbl.", "Pflicht", "oblig."),
    texto!("tela.ia_col_pessoal", "dado pessoal", "donnée personnelle", "personal data", "dato personale", "personenbezogen", "dato personal"),
    texto!("tela.ia_sim", "sim", "oui", "yes", "sì", "ja", "sí"),
    texto!("tela.ia_nao", "não", "non", "no", "no", "nein", "no"),
    texto!("tela.ia_indice_de", "índice `{nome}` ({colunas})", "index `{nome}` ({colunas})", "index `{nome}` ({colunas})", "indice `{nome}` ({colunas})", "Index `{nome}` ({colunas})", "índice `{nome}` ({colunas})"),
    texto!("tela.ia_unico", "único", "unique", "unique", "unico", "eindeutig", "único"),
    texto!("tela.ia_primario", "primário", "primaire", "primary", "primario", "primär", "primario"),
    texto!("tela.ia_relacionamentos", "Relacionamentos", "Associations", "Relationships", "Relazioni", "Beziehungen", "Relaciones"),
    texto!("tela.ia_fk_declarada", "**A chave estrangeira do PhxSql nasce CONFERIDA.** Declarar já é impor: a gravação recusa filha sem mãe e mãe que ainda tem filha, e o `ao_alterar` cascateia. Para só o desenho, mande `\"verificar\": false`.", "**La clé étrangère de PhxSql naît VÉRIFIÉE.** Déclarer, c'est imposer : l'écriture refuse l'enfant sans parent et le parent qui a des enfants, et `ao_alterar` cascade. Pour le seul dessin, envoyez `\"verificar\": false`.", "**PhxSql's foreign key is born ENFORCED.** Declaring is enforcing: writes reject a child without a parent and a parent that still has children, and `ao_alterar` cascades. For the drawing only, send `\"verificar\": false`.", "**La chiave esterna di PhxSql nasce VERIFICATA.** Dichiarare è imporre: la scrittura rifiuta il figlio senza madre e la madre che ha figli, e `ao_alterar` fa cascata. Per il solo disegno, inviare `\"verificar\": false`.", "**Der Fremdschlüssel von PhxSql wird GEPRÜFT geboren.** Deklarieren heißt erzwingen: Das Schreiben weist Kind ohne Mutter und Mutter mit Kindern ab, und `ao_alterar` kaskadiert. Nur fürs Bild: `\"verificar\": false`.", "**La clave foránea de PhxSql nace VERIFICADA.** Declarar es imponer: la grabación rechaza la hija sin madre y la madre que aún tiene hijas, y `ao_alterar` hace cascada. Para sólo el dibujo, envíe `\"verificar\": false`."),
    texto!("tela.ia_fk_declarada2", "O que ela **não** cobre é o que chega pela replicação: ali outro servidor já julgou, e este aplica.", "Ce qu'elle **ne** couvre **pas**, c'est ce qui arrive par la réplication : là, un autre serveur a déjà jugé, et celui-ci applique.", "What it does **not** cover is what arrives through replication: there another server has already judged, and this one applies.", "Ciò che **non** copre è quanto arriva dalla replica: lì un altro server ha già giudicato, e questo applica.", "Was er **nicht** abdeckt, ist das, was über die Replikation kommt: dort hat ein anderer Server bereits entschieden, und dieser wendet an.", "Lo que **no** cubre es lo que llega por la replicación: allí otro servidor ya juzgó, y éste aplica."),
    texto!("tela.ia_ao_excluir", "ao excluir: {acao}", "à la suppression : {acao}", "on delete: {acao}", "all'eliminazione: {acao}", "beim Löschen: {acao}", "al eliminar: {acao}"),
    texto!("tela.ia_criar", "Criar o que está marcado", "Créer ce qui est coché", "Create what is ticked", "Crea ciò che è spuntato", "Anlegen, was angehakt ist", "Crear lo que está marcado"),
    texto!("tela.ia_so_este_clique", "só este clique escreve no banco", "seul ce clic écrit dans la base", "only this click writes to the database", "solo questo clic scrive nel database", "nur dieser Klick schreibt in die Datenbank", "solo este clic escribe en la base"),

    // A criação e o desfazer.
    texto!("tela.ia_nada_marcado", "Nada marcado.", "Rien de coché.", "Nothing ticked.", "Niente spuntato.", "Nichts angehakt.", "Nada marcado."),
    texto!("tela.ia_criando", "criando…", "création…", "creating…", "creazione…", "wird angelegt…", "creando…"),
    texto!("tela.ia_feito_tabela", "tabela **{nome}** criada com {n} coluna(s)", "table **{nome}** créée avec {n} colonne(s)", "table **{nome}** created with {n} column(s)", "tabella **{nome}** creata con {n} colonna/e", "Tabelle **{nome}** mit {n} Spalte(n) angelegt", "tabla **{nome}** creada con {n} columna(s)"),
    texto!("tela.ia_falha_tabela", "tabela **{nome}**: {erro}", "table **{nome}** : {erro}", "table **{nome}**: {erro}", "tabella **{nome}**: {erro}", "Tabelle **{nome}**: {erro}", "tabla **{nome}**: {erro}"),
    texto!("tela.ia_feito_fk", "relacionamento **{nome}**: {de} → {para} declarado", "association **{nome}** : {de} → {para} déclarée", "relationship **{nome}**: {de} → {para} declared", "relazione **{nome}**: {de} → {para} dichiarata", "Beziehung **{nome}**: {de} → {para} deklariert", "relación **{nome}**: {de} → {para} declarada"),
    texto!("tela.ia_falha_fk", "relacionamento **{nome}**: {erro}", "association **{nome}** : {erro}", "relationship **{nome}**: {erro}", "relazione **{nome}**: {erro}", "Beziehung **{nome}**: {erro}", "relación **{nome}**: {erro}"),
    texto!("tela.ia_nasceu", "O que nasceu", "Ce qui est né", "What was born", "Che cosa è nato", "Was entstanden ist", "Lo que nació"),
    texto!("tela.ia_n_criados", "{bons} de {total} item(ns) criados.", "{bons} sur {total} élément(s) créés.", "{bons} of {total} item(s) created.", "{bons} di {total} elemento/i creati.", "{bons} von {total} Element(en) angelegt.", "{bons} de {total} elemento(s) creados."),
    texto!("tela.ia_dicionario", "Dicionário de dados", "Dictionnaire de données", "Data dictionary", "Dizionario dei dati", "Datenwörterbuch", "Diccionario de datos"),
    texto!("tela.ia_er_cheia", "Diagrama ER em tela cheia", "Diagramme E-A en plein écran", "ER diagram full screen", "Diagramma ER a schermo intero", "ER-Diagramm im Vollbild", "Diagrama ER a pantalla completa"),
    texto!("tela.ia_desfazer", "Desfazer esta rodada", "Annuler cette série", "Undo this round", "Annulla questo giro", "Diese Runde rückgängig machen", "Deshacer esta ronda"),
    texto!("tela.ia_modelo_agora", "O modelo agora", "Le modèle maintenant", "The model now", "Il modello adesso", "Das Modell jetzt", "El modelo ahora"),
    texto!("tela.ia_desenhando", "desenhando…", "dessin en cours…", "drawing…", "disegno in corso…", "wird gezeichnet…", "dibujando…"),
    texto!("tela.ia_resumo_er", "{tabelas} tabela(s) · {ligacoes} relacionamento(s) · {sem} sem ligação", "{tabelas} table(s) · {ligacoes} association(s) · {sem} sans lien", "{tabelas} table(s) · {ligacoes} relationship(s) · {sem} with no link", "{tabelas} tabella/e · {ligacoes} relazione/i · {sem} senza legame", "{tabelas} Tabelle(n) · {ligacoes} Beziehung(en) · {sem} ohne Verbindung", "{tabelas} tabla(s) · {ligacoes} relación(es) · {sem} sin enlace"),
    texto!("tela.ia_nada_desfazer", "Nada desta rodada para desfazer.", "Rien à annuler dans cette série.", "Nothing from this round to undo.", "Niente da annullare in questo giro.", "Aus dieser Runde gibt es nichts rückgängig zu machen.", "Nada de esta ronda para deshacer."),
    texto!("tela.ia_conferindo", "conferindo…", "vérification…", "checking…", "verifica in corso…", "wird geprüft…", "comprobando…"),
    texto!("tela.ia_ja_ha_dado", "**Atenção: já há dado gravado.**", "**Attention : des données sont déjà écrites.**", "**Careful: there is data written already.**", "**Attenzione: ci sono già dati scritti.**", "**Achtung: es sind bereits Daten geschrieben.**", "**Atención: ya hay datos grabados.**"),
    texto!("tela.ia_tem_linhas", "`{tabela}` tem {n} linha(s)", "`{tabela}` a {n} ligne(s)", "`{tabela}` has {n} row(s)", "`{tabela}` ha {n} riga/he", "`{tabela}` hat {n} Zeile(n)", "`{tabela}` tiene {n} fila(s)"),
    texto!("tela.ia_desfazer_apaga", "Desfazer apaga a tabela e o dado junto, e não há volta. Clique em **Desfazer esta rodada** outra vez para confirmar.", "Annuler efface la table et les données avec, sans retour. Cliquez encore sur **Annuler cette série** pour confirmer.", "Undoing deletes the table and the data with it, with no way back. Click **Undo this round** again to confirm.", "Annullare cancella la tabella e i dati con essa, senza ritorno. Clicca di nuovo su **Annulla questo giro** per confermare.", "Rückgängig löscht die Tabelle samt Daten, ohne Weg zurück. Klicken Sie erneut auf **Diese Runde rückgängig machen**.", "Deshacer borra la tabla y el dato con ella, y no hay vuelta. Haga clic en **Deshacer esta ronda** otra vez para confirmar."),
    texto!("tela.ia_fk_removido", "relacionamento {nome} removido", "association {nome} retirée", "relationship {nome} removed", "relazione {nome} rimossa", "Beziehung {nome} entfernt", "relación {nome} quitada"),
    texto!("tela.ia_tabela_removida", "tabela {nome} removida", "table {nome} retirée", "table {nome} removed", "tabella {nome} rimossa", "Tabelle {nome} entfernt", "tabla {nome} quitada"),

    // O editor de SQL da tela.
    texto!("tela.ia_editor_vazio", "O editor está vazio.", "L'éditeur est vide.", "The editor is empty.", "L'editor è vuoto.", "Der Editor ist leer.", "El editor está vacío."),
    texto!("tela.ia_executando", "executando…", "exécution…", "running…", "esecuzione…", "wird ausgeführt…", "ejecutando…"),
    texto!("tela.ia_res_op", "operação `{op}` · {n} linha(s)", "opération `{op}` · {n} ligne(s)", "operation `{op}` · {n} row(s)", "operazione `{op}` · {n} riga/he", "Operation `{op}` · {n} Zeile(n)", "operación `{op}` · {n} fila(s)"),
    texto!("tela.ia_res_contagem", "contagem {n}", "comptage {n}", "count {n}", "conteggio {n}", "Anzahl {n}", "recuento {n}"),
    texto!("tela.ia_sem_linhas", "sem linhas", "aucune ligne", "no rows", "nessuna riga", "keine Zeilen", "sin filas"),

    // O que a API recusa, dito com o que fazer a seguir.
    texto!("tela.ia_e_disse", "A API disse: «{msg}»", "L'API a dit : « {msg} »", "The API said: «{msg}»", "L'API ha detto: «{msg}»", "Die API sagte: «{msg}»", "La API dijo: «{msg}»"),
    texto!("tela.ia_e_bytes", "({n} bytes de resposta)", "({n} octets de réponse)", "({n} bytes of answer)", "({n} byte di risposta)", "({n} Bytes Antwort)", "({n} bytes de respuesta)"),
    texto!("tela.ia_e_401", "A chave não foi aceita (401). Confira se ela está inteira e ainda válida em Configurações → Integração com a Claude.", "La clé a été refusée (401). Vérifiez qu'elle est entière et encore valide dans Configuration → Intégration avec Claude.", "The key was not accepted (401). Check that it is whole and still valid in Settings → Claude integration.", "La chiave non è stata accettata (401). Controlla che sia intera e ancora valida in Impostazioni → Integrazione con Claude.", "Der Schlüssel wurde nicht akzeptiert (401). Prüfen Sie in Einstellungen → Claude-Integration, ob er vollständig und gültig ist.", "La clave no fue aceptada (401). Compruebe que esté entera y aún válida en Configuración → Integración con Claude."),
    texto!("tela.ia_e_402", "Cobrança pendente na conta da Anthropic (402). Acerte o pagamento no Console dela e tente de novo.", "Paiement en attente sur le compte Anthropic (402). Réglez-le dans sa Console et réessayez.", "Payment pending on the Anthropic account (402). Settle it in their Console and try again.", "Pagamento in sospeso sull'account Anthropic (402). Sistemalo nella loro Console e riprova.", "Offene Zahlung im Anthropic-Konto (402). Begleichen Sie sie in deren Konsole und versuchen Sie es erneut.", "Cobro pendiente en la cuenta de Anthropic (402). Regularice el pago en su Consola e inténtelo de nuevo."),
    texto!("tela.ia_e_403", "Esta chave não tem permissão para este recurso (403).", "Cette clé n'a pas le droit d'accéder à cette ressource (403).", "This key has no permission for this resource (403).", "Questa chiave non ha il permesso per questa risorsa (403).", "Dieser Schlüssel hat keine Berechtigung für diese Ressource (403).", "Esta clave no tiene permiso para este recurso (403)."),
    texto!("tela.ia_e_429", "Limite de uso atingido (429). Espere um pouco e peça de novo — ou escolha um modelo mais barato em Configurações.", "Limite d'usage atteinte (429). Attendez un peu et redemandez — ou choisissez un modèle moins cher dans Configuration.", "Usage limit reached (429). Wait a little and ask again — or pick a cheaper model in Settings.", "Limite d'uso raggiunto (429). Aspetta un poco e richiedi — o scegli un modello più economico in Impostazioni.", "Nutzungsgrenze erreicht (429). Warten Sie kurz und fragen Sie erneut — oder wählen Sie in den Einstellungen ein günstigeres Modell.", "Límite de uso alcanzado (429). Espere un poco y pida de nuevo — o elija un modelo más barato en Configuración."),
    texto!("tela.ia_e_400", "A API recusou o pedido (400).", "L'API a refusé la requête (400).", "The API refused the request (400).", "L'API ha rifiutato la richiesta (400).", "Die API hat die Anfrage abgelehnt (400).", "La API rechazó la petición (400)."),
    texto!("tela.ia_e_500", "A API está sobrecarregada ou fora do ar ({codigo}). Não é a sua chave nem o seu pedido: tente de novo em alguns segundos.", "L'API est surchargée ou hors service ({codigo}). Ce n'est ni votre clé ni votre requête : réessayez dans quelques secondes.", "The API is overloaded or down ({codigo}). It is neither your key nor your request: try again in a few seconds.", "L'API è sovraccarica o fuori servizio ({codigo}). Non è la tua chiave né la tua richiesta: riprova tra qualche secondo.", "Die API ist überlastet oder außer Betrieb ({codigo}). Es liegt weder am Schlüssel noch an der Anfrage: gleich erneut versuchen.", "La API está sobrecargada o fuera de servicio ({codigo}). No es su clave ni su petición: inténtelo de nuevo en unos segundos."),
    texto!("tela.ia_e_outro", "A API respondeu {codigo}.", "L'API a répondu {codigo}.", "The API answered {codigo}.", "L'API ha risposto {codigo}.", "Die API antwortete {codigo}.", "La API respondió {codigo}."),
    texto!("tela.ia_e_sem_chave", "Sem chave configurada.", "Aucune clé configurée.", "No key configured.", "Nessuna chiave configurata.", "Kein Schlüssel eingerichtet.", "Sin clave configurada."),
    texto!("tela.ia_e_rede", "Não deu para alcançar a API da Anthropic. Confira a conexão desta máquina com a internet e o endereço configurado ({alvo}). Detalhe do navegador: {detalhe}", "Impossible de joindre l'API d'Anthropic. Vérifiez la connexion de cette machine à Internet et l'adresse configurée ({alvo}). Détail du navigateur : {detalhe}", "Could not reach Anthropic's API. Check this machine's connection to the internet and the configured address ({alvo}). Browser detail: {detalhe}", "Non è stato possibile raggiungere l'API di Anthropic. Controlla la connessione a internet di questa macchina e l'indirizzo configurato ({alvo}). Dettaglio del browser: {detalhe}", "Die API von Anthropic war nicht erreichbar. Prüfen Sie die Internetverbindung dieses Rechners und die eingestellte Adresse ({alvo}). Browser-Detail: {detalhe}", "No se pudo alcanzar la API de Anthropic. Compruebe la conexión a internet de esta máquina y la dirección configurada ({alvo}). Detalle del navegador: {detalhe}"),
    texto!("tela.ia_e_meio", "erro no meio da resposta", "erreur au milieu de la réponse", "error in the middle of the answer", "errore a metà della risposta", "Fehler mitten in der Antwort", "error en medio de la respuesta"),
    texto!("tela.ia_e_interrompida", "A resposta foi interrompida pela API: {detalhe}", "La réponse a été interrompue par l'API : {detalhe}", "The answer was interrupted by the API: {detalhe}", "La risposta è stata interrotta dall'API: {detalhe}", "Die Antwort wurde von der API unterbrochen: {detalhe}", "La respuesta fue interrumpida por la API: {detalhe}"),

    // O que a conferência do plano recusa, antes da primeira escrita.
    texto!("tela.ia_p_str", "Str sem tamanho vira Str(60)", "Str sans taille devient Str(60)", "Str with no size becomes Str(60)", "Str senza dimensione diventa Str(60)", "Str ohne Größe wird zu Str(60)", "Str sin tamaño se vuelve Str(60)"),
    texto!("tela.ia_p_decimal", "Decimal sem parâmetro vira Decimal(15,2)", "Decimal sans paramètre devient Decimal(15,2)", "Decimal with no parameter becomes Decimal(15,2)", "Decimal senza parametro diventa Decimal(15,2)", "Decimal ohne Parameter wird zu Decimal(15,2)", "Decimal sin parámetro se vuelve Decimal(15,2)"),
    texto!("tela.ia_p_tipo", "tipo \"{tipo}\" não existe no PhxSql", "le type \"{tipo}\" n'existe pas dans PhxSql", "type \"{tipo}\" does not exist in PhxSql", "il tipo \"{tipo}\" non esiste in PhxSql", "Typ \"{tipo}\" gibt es in PhxSql nicht", "el tipo \"{tipo}\" no existe en PhxSql"),
    texto!("tela.ia_p_formato", "A resposta não veio no formato de plano que esta tela sabe conferir ({n} bytes de texto). Peça de novo, ou use um modelo mais capaz em Configurações.", "La réponse n'est pas au format de plan que cet écran sait vérifier ({n} octets de texte). Redemandez, ou prenez un modèle plus capable dans Configuration.", "The answer did not come in the plan format this screen can check ({n} bytes of text). Ask again, or use a more capable model in Settings.", "La risposta non è nel formato di piano che questa schermata sa verificare ({n} byte di testo). Richiedi, o usa un modello più capace in Impostazioni.", "Die Antwort kam nicht im Planformat, das dieser Bildschirm prüfen kann ({n} Bytes Text). Fragen Sie erneut oder nehmen Sie in den Einstellungen ein stärkeres Modell.", "La respuesta no vino en el formato de plan que esta pantalla sabe comprobar ({n} bytes de texto). Pida de nuevo, o use un modelo más capaz en Configuración."),
    texto!("tela.ia_p_sem_tabelas", "O plano veio sem a lista de tabelas.", "Le plan est arrivé sans la liste des tables.", "The plan came with no list of tables.", "Il piano è arrivato senza l'elenco delle tabelle.", "Der Plan kam ohne die Tabellenliste.", "El plan llegó sin la lista de tablas."),
    texto!("tela.ia_p_sem_nome", "tabela sem nome", "table sans nom", "table with no name", "tabella senza nome", "Tabelle ohne Namen", "tabla sin nombre"),
    texto!("tela.ia_p_sem_coluna", "tabela sem coluna nenhuma", "table sans aucune colonne", "table with no column at all", "tabella senza nessuna colonna", "Tabelle ganz ohne Spalte", "tabla sin ninguna columna"),
    texto!("tela.ia_p_ja_existe", "a tabela \"{nome}\" JÁ EXISTE neste banco — e o PhxSql não tem ALTER de coluna, então ela não pode ser alterada aqui. Nada é sobrescrito: crie com outro nome, ou duplique e recrie.", "la table \"{nome}\" EXISTE DÉJÀ dans cette base — et PhxSql n'a pas d'ALTER de colonne, elle ne peut donc pas être modifiée ici. Rien n'est écrasé : créez sous un autre nom, ou dupliquez et recréez.", "table \"{nome}\" ALREADY EXISTS in this database — and PhxSql has no column ALTER, so it cannot be changed here. Nothing is overwritten: create under another name, or copy and recreate.", "la tabella \"{nome}\" ESISTE GIÀ in questo database — e PhxSql non ha ALTER di colonna, quindi non può essere modificata qui. Nulla è sovrascritto: creala con un altro nome, o duplica e ricrea.", "die Tabelle \"{nome}\" GIBT ES SCHON in dieser Datenbank — und PhxSql hat kein Spalten-ALTER, sie kann hier also nicht geändert werden. Nichts wird überschrieben: unter anderem Namen anlegen.", "la tabla \"{nome}\" YA EXISTE en esta base — y PhxSql no tiene ALTER de columna, así que no puede alterarse aquí. Nada se sobrescribe: cree con otro nombre, o duplique y recree."),
    texto!("tela.ia_p_repetida", "o plano traz \"{nome}\" duas vezes", "le plan contient \"{nome}\" deux fois", "the plan brings \"{nome}\" twice", "il piano porta \"{nome}\" due volte", "der Plan bringt \"{nome}\" zweimal", "el plan trae \"{nome}\" dos veces"),
    texto!("tela.ia_p_col_sem_nome", "coluna sem nome", "colonne sans nom", "column with no name", "colonna senza nome", "Spalte ohne Namen", "columna sin nombre"),
    texto!("tela.ia_p_col_repetida", "a coluna \"{nome}\" aparece duas vezes", "la colonne \"{nome}\" apparaît deux fois", "column \"{nome}\" appears twice", "la colonna \"{nome}\" compare due volte", "die Spalte \"{nome}\" kommt zweimal vor", "la columna \"{nome}\" aparece dos veces"),
    texto!("tela.ia_p_col", "coluna \"{nome}\": {motivo}", "colonne \"{nome}\" : {motivo}", "column \"{nome}\": {motivo}", "colonna \"{nome}\": {motivo}", "Spalte \"{nome}\": {motivo}", "columna \"{nome}\": {motivo}"),
    texto!("tela.ia_p_sequence", "mais de uma coluna Sequence — o PhxSql aceita uma só", "plus d'une colonne Sequence — PhxSql n'en accepte qu'une", "more than one Sequence column — PhxSql accepts only one", "più di una colonna Sequence — PhxSql ne accetta una sola", "mehr als eine Sequence-Spalte — PhxSql akzeptiert nur eine", "más de una columna Sequence — PhxSql acepta una sola"),
    texto!("tela.ia_p_indice", "o índice \"{indice}\" cita a coluna \"{coluna}\", que a tabela não tem", "l'index \"{indice}\" cite la colonne \"{coluna}\", que la table n'a pas", "index \"{indice}\" names column \"{coluna}\", which the table does not have", "l'indice \"{indice}\" cita la colonna \"{coluna}\", che la tabella non ha", "der Index \"{indice}\" nennt die Spalte \"{coluna}\", die die Tabelle nicht hat", "el índice \"{indice}\" cita la columna \"{coluna}\", que la tabla no tiene"),
    texto!("tela.ia_p_sem_primario", "sem índice primário — a tabela funciona, mas nada garante a unicidade da chave", "sans index primaire — la table marche, mais rien ne garantit l'unicité de la clé", "no primary index — the table works, but nothing guarantees the key's uniqueness", "senza indice primario — la tabella funziona, ma nulla garantisce l'unicità della chiave", "ohne Primärindex — die Tabelle läuft, aber nichts sichert die Eindeutigkeit des Schlüssels", "sin índice primario — la tabla funciona, pero nada garantiza la unicidad de la clave"),
    texto!("tela.ia_p_fk_sem_destino", "relacionamento sem tabela de destino", "association sans table de destination", "relationship with no target table", "relazione senza tabella di destinazione", "Beziehung ohne Zieltabelle", "relación sin tabla de destino"),
    texto!("tela.ia_p_fk_destino", "a tabela de destino \"{nome}\" não existe nem no plano nem neste banco", "la table de destination \"{nome}\" n'existe ni dans le plan ni dans cette base", "target table \"{nome}\" exists neither in the plan nor in this database", "la tabella di destinazione \"{nome}\" non esiste né nel piano né in questo database", "die Zieltabelle \"{nome}\" gibt es weder im Plan noch in dieser Datenbank", "la tabla de destino \"{nome}\" no existe ni en el plan ni en esta base"),
    texto!("tela.ia_p_fk_sem_coluna", "relacionamento sem coluna", "association sans colonne", "relationship with no column", "relazione senza colonna", "Beziehung ohne Spalte", "relación sin columna"),
    texto!("tela.ia_p_fk_coluna", "a coluna \"{coluna}\" não existe em \"{tabela}\"", "la colonne \"{coluna}\" n'existe pas dans \"{tabela}\"", "column \"{coluna}\" does not exist in \"{tabela}\"", "la colonna \"{coluna}\" non esiste in \"{tabela}\"", "die Spalte \"{coluna}\" gibt es in \"{tabela}\" nicht", "la columna \"{coluna}\" no existe en \"{tabela}\""),
    // ============================================= o webservice REST
    // O explorador da especificacao serve estes textos da MESMA `/idiomas`
    // que a interface web: uma segunda tabela de rotulos para a pagina da API
    // envelheceria sozinha, e o tradutor nao saberia que ela existe.
    texto!("tela.api_operacoes", "Operações", "Opérations", "Operations", "Operazioni", "Operationen", "Operaciones"),
    texto!("tela.api_seguranca", "Como autenticar", "Comment s'authentifier", "How to authenticate", "Come autenticarsi", "Wie man sich anmeldet", "Cómo autenticarse"),
    texto!("tela.api_buscar", "Procurar operação", "Chercher une opération", "Search operations", "Cerca operazione", "Operation suchen", "Buscar operación"),
    texto!("tela.api_nada_achado", "Nenhuma operação com esse nome.", "Aucune opération de ce nom.", "No operation by that name.", "Nessuna operazione con quel nome.", "Keine Operation mit diesem Namen.", "Ninguna operación con ese nombre."),
    texto!("tela.api_baixar", "Baixar a especificação", "Télécharger la spécification", "Download the specification", "Scarica la specifica", "Spezifikation herunterladen", "Descargar la especificación"),
    texto!("tela.api_campo", "campo", "champ", "field", "campo", "Feld", "campo"),
    texto!("tela.api_tipo", "tipo", "type", "type", "tipo", "Typ", "tipo"),
    texto!("tela.api_para_que", "para que serve", "à quoi ça sert", "what it is for", "a cosa serve", "wofür es dient", "para qué sirve"),
    texto!("tela.api_obrigatorio", "obrigatório", "obligatoire", "required", "obbligatorio", "erforderlich", "obligatorio"),
    texto!("tela.api_sem_campo", "Esta operação não pede campo nenhum.", "Cette opération ne demande aucun champ.", "This operation takes no fields.", "Questa operazione non chiede alcun campo.", "Diese Operation verlangt kein Feld.", "Esta operación no pide ningún campo."),
    texto!("tela.api_exemplo", "Exemplo pronto", "Exemple prêt", "Ready-made example", "Esempio pronto", "Fertiges Beispiel", "Ejemplo listo"),
    texto!("tela.api_grava", "grava", "écrit", "writes", "scrive", "schreibt", "escribe"),
    texto!("tela.api_so_le", "só lê", "lecture seule", "read only", "solo lettura", "nur Lesen", "solo lee"),
    texto!("tela.api_permissao", "permissão", "permission", "permission", "permesso", "Berechtigung", "permiso"),
    texto!("tela.api_sem_permissao", "basta estar autenticado", "il suffit d'être authentifié", "just being signed in is enough", "basta essere autenticati", "angemeldet sein genügt", "basta estar autenticado"),
    texto!("tela.api_apelidos", "também atende por", "répond aussi à", "also answers to", "risponde anche a", "hört auch auf", "también responde a"),
    texto!("tela.api_aviso_claro", "Em HTTP em claro o token viaja em texto puro em todo pedido, e quem escuta o fio fica com ele. Não há TLS aqui: ponha um proxy que termine TLS na frente, ou um túnel.", "En HTTP en clair le jeton voyage en texte brut à chaque requête, et qui écoute le fil le récupère. Il n'y a pas de TLS ici : placez devant un proxy qui termine le TLS, ou un tunnel.", "Over cleartext HTTP the token travels in plain text on every request, and whoever listens on the wire keeps it. There is no TLS here: put a TLS-terminating proxy in front, or a tunnel.", "In HTTP in chiaro il token viaggia in testo semplice a ogni richiesta, e chi ascolta il filo se lo prende. Qui non c'è TLS: metti davanti un proxy che termini il TLS, o un tunnel.", "Über unverschlüsseltes HTTP reist das Token bei jeder Anfrage im Klartext, und wer die Leitung abhört, behält es. Hier gibt es kein TLS: Setzen Sie einen TLS-terminierenden Proxy davor oder einen Tunnel.", "En HTTP en claro el token viaja en texto plano en cada petición, y quien escucha el hilo se queda con él. Aquí no hay TLS: ponga delante un proxy que termine TLS, o un túnel."),

    // A secao da tela de configuracao. O rotulo e a explicacao de cada campo
    // saem daqui, e nao do dicionario cravado do `index.html`: campo novo com
    // rotulo em portugues seria a promessa do multi-idioma quebrada no lugar
    // mais visivel, que e a tela onde se escolhe o idioma.
    texto!("tela.cfg_rest", "Webservice REST", "Service web REST", "REST web service", "Servizio web REST", "REST-Webdienst", "Servicio web REST"),
    texto!("tela.cfg_rest_ligado", "publicar o webservice REST", "publier le service web REST", "publish the REST web service", "pubblicare il servizio web REST", "REST-Webdienst veröffentlichen", "publicar el servicio web REST"),
    texto!("tela.cfg_rest_ligado_diz", "abre a porta do REST; desligado é como o servidor sempre foi", "ouvre le port REST ; désactivé, c'est le serveur tel qu'il a toujours été", "opens the REST port; off is the server as it has always been", "apre la porta REST; spento è il server come è sempre stato", "öffnet den REST-Port; aus ist der Server wie immer", "abre el puerto REST; apagado es el servidor como siempre fue"),
    texto!("tela.cfg_rest_bind", "endereço e porta do REST", "adresse et port du REST", "REST address and port", "indirizzo e porta del REST", "REST-Adresse und Port", "dirección y puerto del REST"),
    texto!("tela.cfg_rest_bind_diz", "de fábrica 127.0.0.1:6000 — só esta máquina", "par défaut 127.0.0.1:6000 — cette machine seulement", "factory default 127.0.0.1:6000 — this machine only", "di fabbrica 127.0.0.1:6000 — solo questa macchina", "werkseitig 127.0.0.1:6000 — nur diese Maschine", "de fábrica 127.0.0.1:6000 — solo esta máquina"),
    texto!("tela.cfg_rest_nome", "nome deste webservice", "nom de ce service web", "name of this web service", "nome di questo servizio web", "Name dieses Webdienstes", "nombre de este servicio web"),
    texto!("tela.cfg_rest_nome_diz", "aparece no título da especificação e no explorador; nome não é portão", "apparaît dans le titre de la spécification et dans l'explorateur ; un nom n'est pas une barrière", "shows in the specification title and in the explorer; a name is not a gate", "appare nel titolo della specifica e nell'esploratore; un nome non è un cancello", "erscheint im Titel der Spezifikation und im Explorer; ein Name ist kein Tor", "aparece en el título de la especificación y en el explorador; un nombre no es una puerta"),
    texto!("tela.cfg_rest_database", "banco que este webservice atende", "base servie par ce service web", "database this web service serves", "database servito da questo servizio web", "Datenbank, die dieser Webdienst bedient", "base que atiende este servicio web"),
    texto!("tela.cfg_rest_database_diz", "vazio atende todos; preenchido, pedido sem banco recebe este e outro banco responde como inexistente", "vide, il les sert tous ; rempli, une requête sans base reçoit celle-ci et une autre base répond comme inexistante", "empty serves them all; filled in, a request with no database gets this one and any other answers as nonexistent", "vuoto li serve tutti; compilato, una richiesta senza database riceve questo e un altro risponde come inesistente", "leer bedient alle; ausgefüllt erhält eine Anfrage ohne Datenbank diese, und eine andere antwortet als nicht vorhanden", "vacío atiende todos; rellenado, una petición sin base recibe esta y otra base responde como inexistente"),
    texto!("tela.cfg_rest_tabelas", "tabelas que este webservice expõe", "tables exposées par ce service web", "tables this web service exposes", "tabelle esposte da questo servizio web", "Tabellen, die dieser Webdienst freigibt", "tablas que expone este servicio web"),
    texto!("tela.cfg_rest_tabelas_diz", "uma por linha; vazia expõe todas. A lista só ESTREITA: tabela de fora não existe para o REST, e estar nela não dá direito nenhum — quem não pode continua não podendo.", "une par ligne ; vide, elle les expose toutes. La liste ne fait que RESTREINDRE : une table absente n'existe pas pour le REST, et y figurer ne donne aucun droit — qui ne peut pas continue de ne pas pouvoir.", "one per line; empty exposes them all. The list only NARROWS: a table not on it does not exist for the REST, and being on it grants no right — whoever cannot, still cannot.", "una per riga; vuota le espone tutte. L'elenco solo RESTRINGE: una tabella fuori non esiste per il REST, ed esserci non dà alcun diritto — chi non può continua a non potere.", "eine pro Zeile; leer gibt alle frei. Die Liste ENGT nur EIN: eine Tabelle außerhalb existiert für den REST nicht, und darin zu stehen gibt kein Recht — wer nicht darf, darf weiterhin nicht.", "una por línea; vacía expone todas. La lista solo ESTRECHA: una tabla fuera no existe para el REST, y estar en ella no da ningún derecho — quien no puede sigue sin poder."),
    texto!("tela.cfg_rest_swagger", "publicar o explorador da especificação", "publier l'explorateur de la spécification", "publish the specification explorer", "pubblicare l'esploratore della specifica", "Spezifikations-Explorer veröffentlichen", "publicar el explorador de la especificación"),
    texto!("tela.cfg_rest_swagger_diz", "porta separada, de fábrica 7000: quem sobe numa placa quer o REST sem o visualizador", "port séparé, 7000 par défaut : qui déploie sur une carte veut le REST sans le visualiseur", "separate port, 7000 by default: whoever runs on a board wants the REST without the viewer", "porta separata, di fabbrica 7000: chi installa su una scheda vuole il REST senza il visualizzatore", "eigener Port, werkseitig 7000: wer auf einer Platine läuft, will den REST ohne Betrachter", "puerto separado, de fábrica 7000: quien lo sube en una placa quiere el REST sin el visor"),
    texto!("tela.cfg_rest_swagger_bind", "endereço e porta do explorador", "adresse et port de l'explorateur", "explorer address and port", "indirizzo e porta dell'esploratore", "Adresse und Port des Explorers", "dirección y puerto del explorador"),
    texto!("tela.cfg_rest_swagger_bind_diz", "de fábrica 127.0.0.1:7000 — só esta máquina", "par défaut 127.0.0.1:7000 — cette machine seulement", "factory default 127.0.0.1:7000 — this machine only", "di fabbrica 127.0.0.1:7000 — solo questa macchina", "werkseitig 127.0.0.1:7000 — nur diese Maschine", "de fábrica 127.0.0.1:7000 — solo esta máquina"),
    texto!("tela.cfg_rest_token_sim", "esta porta tem token próprio, e o token do protocolo não a abre", "ce port a son propre jeton, et le jeton du protocole ne l'ouvre pas", "this port has its own token, and the protocol token does not open it", "questa porta ha un token proprio, e il token del protocollo non la apre", "dieser Port hat ein eigenes Token, und das Protokoll-Token öffnet ihn nicht", "este puerto tiene token propio, y el token del protocolo no lo abre"),
    texto!("tela.cfg_rest_token_nao", "sem token próprio: vale o token do protocolo", "sans jeton propre : le jeton du protocole s'applique", "no token of its own: the protocol token applies", "senza token proprio: vale il token del protocollo", "kein eigenes Token: es gilt das Protokoll-Token", "sin token propio: vale el token del protocolo"),
    texto!("tela.cfg_rest_token_diz", "**O token não se edita aqui**, pelo mesmo motivo do token do protocolo: campo com credencial se edita no `config.json`, para que uma sessão tomada não abra a porta.", "**Le jeton ne s'édite pas ici**, pour la même raison que celui du protocole : un champ qui porte un identifiant s'édite dans `config.json`, pour qu'une session volée n'ouvre pas la porte.", "**The token is not edited here**, for the same reason as the protocol token: a field carrying a credential is edited in `config.json`, so a stolen session cannot open the door.", "**Il token non si modifica qui**, per lo stesso motivo di quello del protocollo: un campo con una credenziale si modifica nel `config.json`, perché una sessione rubata non apra la porta.", "**Das Token wird hier nicht bearbeitet**, wie das Protokoll-Token: ein Feld mit Zugangsdaten wird in der `config.json` bearbeitet, damit eine gestohlene Sitzung die Tür nicht öffnet.", "**El token no se edita aquí**, por el mismo motivo que el del protocolo: un campo con credencial se edita en el `config.json`, para que una sesión robada no abra la puerta."),
    texto!("tela.cfg_rest_nota", "As duas portas nascem **desligadas** e continuam assim numa atualização: servidor que já roda hoje não passa a expor porta nenhuma sem alguém pedir.", "Les deux ports naissent **désactivés** et le restent lors d'une mise à jour : un serveur qui tourne déjà n'expose aucun port sans qu'on le demande.", "Both ports are born **off** and stay off across an upgrade: a server already running does not start exposing a port without someone asking.", "Le due porte nascono **spente** e restano tali dopo un aggiornamento: un server già in funzione non espone porte senza che qualcuno lo chieda.", "Beide Ports sind **aus** und bleiben es über ein Update hinweg: ein schon laufender Server legt keinen Port offen, ohne dass jemand darum bittet.", "Los dos puertos nacen **apagados** y siguen así tras una actualización: un servidor que ya funciona no expone ningún puerto sin que alguien lo pida."),
    texto!("tela.cfg_rest_nota2", "E o `Bearer` sobre HTTP em claro entrega o token a quem escuta o fio — a saída honesta é um proxy com TLS na frente, ou um túnel.", "Et le `Bearer` sur HTTP en clair livre le jeton à qui écoute le fil — la sortie honnête est un proxy TLS devant, ou un tunnel.", "And `Bearer` over cleartext HTTP hands the token to whoever listens on the wire — the honest way out is a TLS proxy in front, or a tunnel.", "E il `Bearer` su HTTP in chiaro consegna il token a chi ascolta il filo — l'uscita onesta è un proxy TLS davanti, o un tunnel.", "Und `Bearer` über unverschlüsseltes HTTP übergibt das Token an jeden, der die Leitung abhört — der ehrliche Ausweg ist ein TLS-Proxy davor oder ein Tunnel.", "Y el `Bearer` sobre HTTP en claro entrega el token a quien escucha el hilo — la salida honesta es un proxy con TLS delante, o un túnel."),

    // ------------------------------------------- colunas de grade: PhxGrid
    // A padronizacao "toda tabela e PhxGrid" moveu o cabecalho de `{t:"..."}`
    // para `titulo:txt(...)`, e sao estas dezenove telas que faziam a tabela
    // a mao. Palavra curta que se repete em mais de uma grade usa a MESMA
    // chave -- "usuario", "operacao" e "tabela" significam a mesma coisa em
    // toda tela, e duas chaves para o mesmo texto so envelheceriam torto.
    texto!("tela.col_volume", "volume", "volume", "volume", "volume", "Volume", "volumen"),
    texto!("tela.col_arquivo", "arquivo", "fichier", "file", "file", "Datei", "archivo"),
    texto!("tela.col_periodo_abriu", "período que o abriu", "période d'ouverture", "period it opened in", "periodo di apertura", "Zeitraum der Eröffnung", "período de apertura"),
    texto!("tela.col_existe", "existe", "existe", "exists", "esiste", "vorhanden", "existe"),
    texto!("tela.col_do_rowid", "do rowid", "du rowid", "from rowid", "dal rowid", "ab rowid", "desde rowid"),
    texto!("tela.col_ate_rowid", "até o rowid", "jusqu'au rowid", "to rowid", "al rowid", "bis rowid", "hasta rowid"),
    texto!("tela.col_slots_usados", "slots usados", "emplacements utilisés", "slots used", "slot usati", "belegte Slots", "ranuras usadas"),
    texto!("tela.col_coluna", "coluna", "colonne", "column", "colonna", "Spalte", "columna"),
    texto!("tela.col_proximo_numero", "próximo número", "prochain numéro", "next number", "numero successivo", "nächste Nummer", "próximo número"),
    texto!("tela.col_indice", "índice", "index", "index", "indice", "Index", "índice"),
    texto!("tela.col_unico", "único", "unique", "unique", "unico", "eindeutig", "único"),
    texto!("tela.col_primaria", "primária", "primaire", "primary", "primaria", "primär", "primaria"),
    texto!("tela.col_composta", "composta", "composée", "composite", "composita", "zusammengesetzt", "compuesta"),
    texto!("tela.col_nome_curto", "nome", "nom", "name", "nome", "Name", "nombre"),
    texto!("tela.col_aponta_para", "aponta para", "pointe vers", "points to", "punta a", "verweist auf", "apunta a"),
    texto!("tela.col_ao_excluir", "ao excluir", "à la suppression", "on delete", "in eliminazione", "beim Löschen", "al eliminar"),
    texto!("tela.col_ao_alterar", "ao alterar", "à la modification", "on update", "in modifica", "beim Ändern", "al modificar"),
    texto!("tela.col_hora", "hora", "heure", "time", "ora", "Uhrzeit", "hora"),
    texto!("tela.col_alvo", "alvo", "cible", "target", "bersaglio", "Ziel", "objetivo"),
    texto!("tela.col_pedido_como_chegou", "pedido, como chegou", "requête, telle qu'elle est arrivée", "request, as it arrived", "richiesta, come è arrivata", "Anfrage, wie sie ankam", "petición, tal como llegó"),
    texto!("tela.pf_em_curso", "em curso", "en cours", "in flight", "in corso", "läuft", "en curso"),
    texto!("tela.pf_erro", "erro", "erreur", "error", "errore", "Fehler", "error"),
    texto!("tela.col_primeiro_rowid", "primeiro rowid", "premier rowid", "first rowid", "primo rowid", "erste rowid", "primer rowid"),
    texto!("tela.col_periodo", "período", "période", "period", "periodo", "Zeitraum", "período"),
    texto!("tela.col_login", "login", "identifiant", "login", "login", "Login", "usuario"),
    texto!("tela.col_nivel", "nível", "niveau", "level", "livello", "Ebene", "nivel"),
    texto!("tela.col_pode_aqui", "pode aqui", "peut ici", "can here", "può qui", "darf hier", "puede aquí"),
    texto!("tela.col_como_resolveu", "como resolveu", "comment résolu", "how it resolved", "come risolto", "wie aufgelöst", "cómo resolvió"),
    texto!("tela.col_supervisor", "supervisor", "superviseur", "supervisor", "supervisore", "Supervisor", "supervisor"),
    texto!("tela.col_ativo", "ativo", "actif", "active", "attivo", "aktiv", "activo"),
    texto!("tela.col_chave", "chave", "clé", "key", "chiave", "Schlüssel", "clave"),
    texto!("tela.col_email", "e-mail", "e-mail", "email", "e-mail", "E-Mail", "correo"),
    texto!("tela.col_caption_grade", "caption", "légende", "caption", "didascalia", "Beschriftung", "título"),
    texto!("tela.col_tam", "tam.", "taille", "size", "dim.", "Größe", "tam."),
    texto!("tela.col_obrig", "obrig.", "obl.", "req.", "obbl.", "Pfl.", "obl."),
    texto!("tela.col_mascara", "máscara", "masque", "mask", "maschera", "Maske", "máscara"),
    texto!("tela.col_id", "id", "id", "id", "id", "id", "id"),
    texto!("tela.col_descricao", "descrição", "description", "description", "descrizione", "Beschreibung", "descripción"),
    texto!("tela.col_linha_carga", "linha da carga", "ligne du chargement", "load line", "riga del caricamento", "Zeile der Ladung", "línea de la carga"),
    texto!("tela.col_por_que", "por quê", "pourquoi", "why", "perché", "warum", "por qué"),
    texto!("tela.col_valor", "valor", "valeur", "value", "valore", "Wert", "valor"),
    texto!("tela.col_origem", "origem", "origine", "origin", "origine", "Herkunft", "origen"),
    texto!("tela.col_usuario", "usuário", "utilisateur", "user", "utente", "Benutzer", "usuario"),
    texto!("tela.col_aberta_ha", "aberta há", "ouverte depuis", "open for", "aperta da", "offen seit", "abierta hace"),
    texto!("tela.col_estado", "estado", "état", "state", "stato", "Zustand", "estado"),
    texto!("tela.col_operacao", "operação", "opération", "operation", "operazione", "Vorgang", "operación"),
    texto!("tela.col_objeto", "objeto", "objet", "object", "oggetto", "Objekt", "objeto"),
    texto!("tela.col_ha", "há", "depuis", "for", "da", "seit", "hace"),
    texto!("tela.col_pedidos", "pedidos", "requêtes", "requests", "richieste", "Anfragen", "solicitudes"),
    texto!("tela.col_expira_em", "expira em", "expire dans", "expires in", "scade tra", "läuft ab in", "expira en"),
    texto!("tela.col_vezes", "vezes", "fois", "times", "volte", "Mal", "veces"),
    texto!("tela.col_recusadas", "recusadas", "refusées", "refused", "rifiutate", "abgelehnt", "rechazadas"),
    texto!("tela.col_ms_medio", "ms médio", "ms moyen", "avg ms", "ms medio", "ø ms", "ms medio"),
    texto!("tela.col_pior", "pior", "pire", "worst", "peggiore", "schlechteste", "peor"),
    texto!("tela.col_tempo_total", "tempo total", "temps total", "total time", "tempo totale", "Gesamtzeit", "tiempo total"),
    texto!("tela.col_quando", "quando", "quand", "when", "quando", "wann", "cuándo"),
    texto!("tela.col_ms", "ms", "ms", "ms", "ms", "ms", "ms"),
    texto!("tela.col_codigo", "código", "code", "code", "codice", "Code", "código"),
    texto!("tela.col_exemplo", "exemplo", "exemple", "example", "esempio", "Beispiel", "ejemplo"),
    // -------------------------------------------- Sessões e Estatísticas de uso
    // As duas telas que o console nao tinha: quem esta falando agora (o SHOW
    // PROCESSLIST) e o que o log ja sabia (por tabela, por usuario, a cauda
    // da latencia). Baixa a catraca do conferidor -- ver conferidor.rs
    // TETO_ROTULOS_E_CRASE.
    texto!("tela.se_titulo", "Sessões", "Sessions", "Sessions", "Sessioni", "Sitzungen", "Sesiones"),
    texto!("tela.se_sub", "quem está falando com o servidor agora · atualiza a cada 3 s", "qui parle avec le serveur en ce moment · actualise toutes les 3 s", "who is talking to the server right now · refreshes every 3 s", "chi sta parlando con il server ora · si aggiorna ogni 3 s", "wer gerade mit dem Server spricht · aktualisiert alle 3 s", "quién está hablando con el servidor ahora · se actualiza cada 3 s"),
    texto!("tela.se_fic_porta", "na porta de dados", "sur le port de données", "on the data port", "sulla porta dati", "am Datenport", "en el puerto de datos"),
    texto!("tela.se_fic_web", "sessões web", "sessions web", "web sessions", "sessioni web", "Web-Sitzungen", "sesiones web"),
    texto!("tela.se_fic_demorada", "a mais demorada", "la plus lente", "the slowest", "la più lenta", "die langsamste", "la más lenta"),
    texto!("tela.se_mais_de_5s", "há mais de 5 s", "depuis plus de 5 s", "for more than 5 s", "da più di 5 s", "seit mehr als 5 s", "hace más de 5 s"),
    texto!("tela.se_navegador_h3", "Sessões do navegador", "Sessions du navigateur", "Browser sessions", "Sessioni del browser", "Browser-Sitzungen", "Sesiones del navegador"),
    // Str(250) na coluna do idioma -- o paragrafo original (344 bytes em
    // portugues) nao cabia inteiro numa linha so, entao ele entra em tres
    // chaves curtas e o chamador junta com marcado() + esc().
    texto!("tela.se_aviso_b", "**Encerrar fecha o soquete.**", "**Arrêter ferme la socket.**", "**Stop closes the socket.**", "**Chiudi chiude il socket.**", "**Beenden schließt den Socket.**", "**Finalizar cierra el socket.**"),
    texto!("tela.se_aviso_p1", "É imediato para a conexão que está esperando pedido — o caso comum da conexão esquecida aberta. Uma operação que **já entrou** na trava de dados termina assim mesmo:", "C'est immédiat pour la connexion qui attend une requête — le cas courant de la connexion oubliée ouverte. Une opération qui **est déjà entrée** dans le verrou de données se termine quand même :", "It's immediate for the connection waiting on a request — the common case of a forgotten open connection. An operation that **has already entered** the data lock still finishes:", "È immediato per la connessione che sta aspettando una richiesta — il caso comune della connessione dimenticata aperta. Un'operazione che **è già entrata** nel lock dei dati termina comunque:", "Für die Verbindung, die auf eine Anfrage wartet, wirkt es sofort — der übliche Fall der vergessenen offenen Verbindung. Eine Operation, die **die Datensperre bereits hält**, wird trotzdem beendet:", "Es inmediato para la conexión que está esperando una petición — el caso común de la conexión olvidada abierta. Una operación que **ya entró** en el bloqueo de datos termina de todos modos:"),
    texto!("tela.se_aviso_p2", "não há como abandonar uma varredura no meio sem arriscar deixar a tabela aberta pela metade. O que muda é que o resultado não vai para lugar nenhum.", "impossible d'abandonner un parcours à mi-chemin sans risquer de laisser la table ouverte à moitié. Ce qui change, c'est que le résultat ne va nulle part.", "there is no way to abandon a scan partway through without risking leaving the table half-open. What changes is that the result goes nowhere.", "non c'è modo di abbandonare una scansione a metà senza rischiare di lasciare la tabella aperta a metà. Quel che cambia è che il risultato non va da nessuna parte.", "es gibt keine Möglichkeit, einen Scan mittendrin abzubrechen, ohne die Tabelle halb offen zu lassen. Was sich ändert, ist, dass das Ergebnis nirgendwohin geht.", "no hay forma de abandonar un recorrido a medias sin arriesgarse a dejar la tabla medio abierta. Lo que cambia es que el resultado no va a ninguna parte."),
    texto!("tela.se_encerrar", "Encerrar", "Arrêter", "Stop", "Chiudi", "Beenden", "Finalizar"),
    texto!("tela.se_confirma_kill", "Encerrar a conexão {id}?\n\nO cliente do outro lado perde a conexão.", "Arrêter la connexion {id} ?\n\nLe client de l'autre côté perd la connexion.", "Stop connection {id}?\n\nThe client on the other side loses the connection.", "Chiudere la connessione {id}?\n\nIl client dall'altra parte perde la connessione.", "Verbindung {id} beenden?\n\nDer Client auf der anderen Seite verliert die Verbindung.", "¿Finalizar la conexión {id}?\n\nEl cliente del otro lado pierde la conexión."),
    texto!("tela.se_confirma_killweb", "Encerrar a sessão web {id}…?\n\nO próximo clique de quem estava usando cai no login.", "Arrêter la session web {id}… ?\n\nLe prochain clic de la personne qui l'utilisait retombera sur la connexion.", "Stop web session {id}…?\n\nThe next click from whoever was using it will drop them to the login screen.", "Chiudere la sessione web {id}…?\n\nIl prossimo clic di chi la stava usando torna al login.", "Web-Sitzung {id} beenden…?\n\nDer nächste Klick der Person, die sie benutzt hat, landet beim Anmeldebildschirm.", "¿Finalizar la sesión web {id}…?\n\nEl próximo clic de quien la estaba usando cae en el inicio de sesión."),
    texto!("tela.se_av_kill", "conexão {id} ({estava}) — {aviso}", "connexion {id} ({estava}) — {aviso}", "connection {id} ({estava}) — {aviso}", "connessione {id} ({estava}) — {aviso}", "Verbindung {id} ({estava}) — {aviso}", "conexión {id} ({estava}) — {aviso}"),
    texto!("tela.se_av_killweb", "sessão {id} encerrada — {aviso}", "session {id} arrêtée — {aviso}", "session {id} stopped — {aviso}", "sessione {id} chiusa — {aviso}", "Sitzung {id} beendet — {aviso}", "sesión {id} finalizada — {aviso}"),
    texto!("tela.st_sub", "desde {desde} · {acessos} acessos", "depuis {desde} · {acessos} accès", "since {desde} · {acessos} requests", "da {desde} · {acessos} accessi", "seit {desde} · {acessos} Zugriffe", "desde {desde} · {acessos} accesos"),
    texto!("tela.st_periodo", "Período", "Période", "Period", "Periodo", "Zeitraum", "Período"),
    texto!("tela.st_todos_log", "tudo o que há no log", "tout ce qu'il y a dans le journal", "everything in the log", "tutto quello che c'è nel log", "alles, was im Log steht", "todo lo que hay en el registro"),
    texto!("tela.st_ultima_hora", "última hora", "dernière heure", "last hour", "ultima ora", "letzte Stunde", "última hora"),
    texto!("tela.st_24h", "últimas 24 horas", "dernières 24 heures", "last 24 hours", "ultime 24 ore", "letzte 24 Stunden", "últimas 24 horas"),
    texto!("tela.st_7d", "últimos 7 dias", "derniers 7 jours", "last 7 days", "ultimi 7 giorni", "letzte 7 Tage", "últimos 7 días"),
    texto!("tela.st_sem_acessos", "sem acessos no período", "aucun accès sur la période", "no requests in this period", "nessun accesso nel periodo", "keine Zugriffe im Zeitraum", "sin accesos en el período"),
    texto!("tela.st_aria_dist", "Distribuição do tempo de resposta", "Répartition du temps de réponse", "Response time distribution", "Distribuzione del tempo di risposta", "Verteilung der Antwortzeit", "Distribución del tiempo de respuesta"),
    texto!("tela.st_ms_ou_mais", "ms ou mais", "ms ou plus", "ms or more", "ms o più", "ms oder mehr", "ms o más"),
    texto!("tela.st_fic_mediana", "mediana", "médiane", "median", "mediana", "Median", "mediana"),
    texto!("tela.st_fic_mediana_un", "ms · metade responde abaixo disso", "ms · la moitié répond en dessous", "ms · half of responses fall below this", "ms · metà risponde sotto questo valore", "ms · die Hälfte antwortet darunter", "ms · la mitad responde por debajo de esto"),
    texto!("tela.st_fic_p95_un", "ms · é este que responde «está rápido?»", "ms · c'est lui qui répond « c'est rapide ? »", "ms · this is the one that answers «is it fast?»", "ms · è questo che risponde a «è veloce?»", "ms · das ist die Antwort auf «ist es schnell?»", "ms · es este el que responde «¿es rápido?»"),
    texto!("tela.st_falhou", "falhou", "échec", "failed", "fallita", "fehlgeschlagen", "fallo"),
    texto!("tela.st_carta_tempo_resposta", "Tempo de resposta", "Temps de réponse", "Response time", "Tempo di risposta", "Antwortzeit", "Tiempo de respuesta"),
    texto!("tela.st_carta_tempo_resposta_leg", "cada faixa dobra a anterior · vermelho é acima de um segundo", "chaque tranche double la précédente · le rouge est au-delà d'une seconde", "each band doubles the previous one · red is above one second", "ogni fascia raddoppia la precedente · il rosso è oltre un secondo", "jede Stufe verdoppelt die vorherige · Rot ist über einer Sekunde", "cada franja duplica la anterior · el rojo es por encima de un segundo"),
    texto!("tela.st_carta_mais_demoradas", "As mais demoradas", "Les plus lentes", "The slowest ones", "Le più lente", "Die langsamsten", "Las más lentas"),
    texto!("tela.st_carta_mais_demoradas_leg", "o registro de consulta lenta, sem precisar ligar nada", "le journal des requêtes lentes, sans rien avoir à activer", "the slow-query log, with nothing to turn on", "il registro delle query lente, senza dover attivare nulla", "das Protokoll der langsamen Abfragen, ohne dass etwas eingeschaltet werden muss", "el registro de consultas lentas, sin necesidad de activar nada"),
    texto!("tela.st_carta_por_tabela", "Por tabela", "Par table", "By table", "Per tabella", "Nach Tabelle", "Por tabla"),
    texto!("tela.st_carta_por_tabela_leg", "qual tabela custa caro — o log não sabia dizer isso antes", "quelle table coûte cher — le journal ne le disait pas avant", "which table costs the most — the log couldn't say that before", "quale tabella costa cara — prima il log non lo sapeva dire", "welche Tabelle teuer ist — das konnte das Log vorher nicht sagen", "qué tabla cuesta cara — el registro antes no sabía decir eso"),
    texto!("tela.st_carta_por_operacao", "Por operação", "Par opération", "By operation", "Per operazione", "Nach Vorgang", "Por operación"),
    texto!("tela.st_carta_por_operacao_leg", "o que mais se pede", "ce qui est le plus demandé", "what gets asked for the most", "cosa viene richiesto di più", "was am häufigsten angefragt wird", "lo que más se pide"),
    texto!("tela.st_carta_por_usuario", "Por usuário", "Par utilisateur", "By user", "Per utente", "Nach Benutzer", "Por usuario"),
    texto!("tela.st_carta_por_usuario_leg", "quem mais usa", "qui utilise le plus", "who uses it the most", "chi usa di più", "wer es am meisten benutzt", "quién más lo usa"),
    texto!("tela.st_carta_por_erro", "Por erro", "Par erreur", "By error", "Per errore", "Nach Fehler", "Por error"),
    texto!("tela.st_carta_por_erro_leg", "agrupado pelo código, e não pelo texto da mensagem", "regroupé par code, et non par le texte du message", "grouped by code, not by the message text", "raggruppato per codice, non per il testo del messaggio", "gruppiert nach Code, nicht nach dem Meldungstext", "agrupado por código, no por el texto del mensaje"),
    texto!("tela.col_apelido", "apelido", "alias", "alias", "alias", "Alias", "alias"),
    texto!("tela.col_motor", "motor", "moteur", "engine", "motore", "Engine", "motor"),
    texto!("tela.col_endereco", "endereço", "adresse", "address", "indirizzo", "Adresse", "dirección"),
    texto!("tela.col_base", "base", "base", "database", "base", "Datenbank", "base"),
    texto!("tela.col_senha", "senha", "mot de passe", "password", "password", "Kennwort", "contraseña"),
    texto!("tela.col_escrita", "escrita", "écriture", "write", "scrittura", "Schreiben", "escritura"),
    texto!("tela.col_teto", "teto", "plafond", "cap", "tetto", "Obergrenze", "tope"),
    texto!("tela.col_agenda", "agenda", "planning", "schedule", "pianificazione", "Zeitplan", "agenda"),
    texto!("tela.col_roda_como", "roda como", "s'exécute en tant que", "runs as", "eseguito come", "läuft als", "se ejecuta como"),
    texto!("tela.col_ultima_corrida", "última corrida", "dernière exécution", "last run", "ultima esecuzione", "letzter Lauf", "última ejecución"),
    texto!("tela.col_proxima", "próxima", "prochaine", "next", "prossima", "nächste", "próxima"),
    texto!("tela.col_como", "como", "comment", "how", "come", "wie", "cómo"),
    texto!("tela.col_resultado", "resultado", "résultat", "result", "risultato", "Ergebnis", "resultado"),
    texto!("tela.col_detalhe", "detalhe", "détail", "detail", "dettaglio", "Detail", "detalle"),
    texto!("tela.col_job", "job", "job", "job", "job", "Job", "job"),
    texto!("tela.col_grau", "grau", "degré", "grade", "grado", "Grad", "grado"),
    texto!("tela.col_rotulo", "rótulo", "libellé", "label", "etichetta", "Bezeichnung", "etiqueta"),
    texto!("tela.col_trilha", "trilha", "piste", "trail", "traccia", "Spur", "rastro"),
    texto!("tela.col_database", "database", "base de données", "database", "database", "Datenbank", "base de datos"),
    texto!("tela.col_tabelas_pl", "tabelas", "tables", "tables", "tabelle", "Tabellen", "tablas"),
    texto!("tela.col_linhas", "linhas", "lignes", "rows", "righe", "Zeilen", "filas"),
    texto!("tela.col_bytes", "bytes", "bytes", "bytes", "bytes", "bytes", "bytes"),
    texto!("tela.col_carregada_em", "carregada em", "chargée le", "loaded at", "caricata il", "geladen am", "cargada el"),
    texto!("tela.col_ip", "IP", "IP", "IP", "IP", "IP", "IP"),
    texto!("tela.col_acessos", "acessos", "accès", "hits", "accessi", "Zugriffe", "accesos"),
    texto!("tela.col_recusados", "recusados", "refusés", "refused", "rifiutati", "abgelehnt", "rechazados"),
    texto!("tela.col_ultimo", "último", "dernier", "last", "ultimo", "letzter", "último"),
    texto!("tela.col_campo", "campo", "champ", "field", "campo", "Feld", "campo"),
    texto!("tela.col_o_que_faz", "o que faz", "ce que ça fait", "what it does", "cosa fa", "was es tut", "qué hace"),
    texto!("tela.col_ligacao", "ligação", "connexion", "connection", "connessione", "Verbindung", "conexión"),
    texto!("tela.col_desde", "desde", "depuis", "since", "da", "seit", "desde"),
    // -------------------------------------------- grades: toda table e PhxGrid
    // As chaves abaixo nasceram convertendo os 16 ultimos `tabela()` a mao de
    // `index.html` para PhxGrid (pedido do dono: "todas as table sao phxgrid
    // com agrupamento dinamico"). Cabecalho de coluna e ROTULO -- rotulo se
    // traduz -- e por isso cada `{t:"..."}` cravado virou `titulo:txt(...)`.
    texto!("tela.col_ordem_indice", "ordem", "ordre", "order", "ordine", "Reihenfolge", "orden"),
    texto!("tela.col_versao", "versão", "version", "version", "versione", "Version", "versión"),
    texto!("tela.col_situacao", "situação", "situation", "status", "situazione", "Status", "situación"),
    texto!("tela.col_chaves", "chaves", "clés", "keys", "chiavi", "Schlüssel", "claves"),
    texto!("tela.pino_em_dia", "em dia", "à jour", "up to date", "in pari", "aktuell", "al día"),
    texto!("tela.pino_fora_sincronia", "fora de sincronia", "désynchronisé", "out of sync", "fuori sincronia", "nicht synchron", "desincronizado"),
    texto!("tela.col_telefone", "telefone", "téléphone", "phone", "telefono", "Telefon", "teléfono"),
    texto!("tela.col_poder_por_base", "poder por base", "pouvoir par base", "power per database", "potere per base", "Rechte pro Datenbank", "poder por base"),
    texto!("tela.col_ate", "até", "jusqu'à", "until", "fino a", "bis", "hasta"),
    texto!("tela.col_motivo", "motivo", "motif", "reason", "motivo", "Grund", "motivo"),
    texto!("tela.col_comando", "comando", "commande", "command", "comando", "Befehl", "comando"),
    texto!("tela.col_tentativas", "tentativas", "tentatives", "attempts", "tentativi", "Versuche", "intentos"),
    texto!("tela.col_firewall", "firewall", "pare-feu", "firewall", "firewall", "Firewall", "cortafuegos"),
    texto!("tela.pino_aplicado", "aplicado", "appliqué", "applied", "applicato", "angewendet", "aplicado"),
    texto!("tela.pino_so_servidor", "só no servidor", "seulement sur le serveur", "server only", "solo sul server", "nur auf dem Server", "solo en el servidor"),
    texto!("tela.col_no_disco", "no disco", "sur le disque", "on disk", "su disco", "auf der Platte", "en disco"),
    texto!("tela.col_arquivos", "arquivos", "fichiers", "files", "file", "Dateien", "archivos"),
    texto!("tela.col_conteudo", "o que tem dentro", "ce qu'il y a dedans", "what's inside", "cosa c'è dentro", "was drin ist", "qué hay dentro"),
    texto!("tela.pino_ilegivel", "ilegível", "illisible", "unreadable", "illeggibile", "unlesbar", "ilegible"),
    texto!("tela.pino_zip", "zip", "zip", "zip", "zip", "zip", "zip"),
    texto!("tela.pino_pasta", "pasta", "dossier", "folder", "cartella", "Ordner", "carpeta"),
    texto!("tela.pino_deduzido", "deduzido", "déduit", "deduced", "dedotto", "abgeleitet", "deducido"),
    texto!("tela.col_anexos", "anexos", "pièces jointes", "attachments", "allegati", "Anhänge", "adjuntos"),
    texto!("tela.col_o_que", "o quê", "quoi", "what", "cosa", "was", "qué"),
    texto!("tela.col_registro", "registro", "enregistrement", "record", "record", "Datensatz", "registro"),
    texto!("tela.col_linha", "linha", "ligne", "row", "riga", "Zeile", "fila"),
    texto!("tela.col_antes", "antes", "avant", "before", "prima", "vorher", "antes"),
    texto!("tela.col_depois", "depois", "après", "after", "dopo", "nachher", "después"),
    texto!("tela.col_de_onde", "de onde", "d'où", "from where", "da dove", "von wo", "desde dónde"),
    texto!("tela.col_eventos", "eventos", "événements", "events", "eventi", "Ereignisse", "eventos"),
    texto!("tela.col_databases_pl", "databases", "bases de données", "databases", "database", "Datenbanken", "bases de datos"),
    texto!("tela.col_a_cada", "a cada", "toutes les", "every", "ogni", "alle", "cada"),
    texto!("tela.col_eventos_la", "eventos lá", "événements là-bas", "events there", "eventi là", "Ereignisse dort", "eventos allá"),
    texto!("tela.col_eventos_aqui", "eventos aqui", "événements ici", "events here", "eventi qui", "Ereignisse hier", "eventos aquí"),
    texto!("tela.col_onde_mora", "onde mora", "où ça vit", "where it lives", "dove vive", "wo es lebt", "dónde vive"),
    texto!("tela.col_quem", "quem", "qui", "who", "chi", "wer", "quién"),
    texto!("tela.pino_anexo_nao_carregado", "anexo · não carregado", "pièce jointe · non chargée", "attachment · not loaded", "allegato · non caricato", "Anhang · nicht geladen", "adjunto · no cargado"),
    texto!("tela.pino_redigido", "redigido", "expurgé", "redacted", "redatto", "geschwärzt", "redactado"),
    texto!("tela.pino_vazio", "(vazio)", "(vide)", "(empty)", "(vuoto)", "(leer)", "(vacío)"),
    texto!("tela.pino_alterou", "alterou", "a modifié", "changed", "ha modificato", "hat geändert", "modificó"),
    texto!("tela.pino_leu", "leu", "a lu", "read", "ha letto", "hat gelesen", "leyó"),
    texto!("tela.pino_atras_de", "atrás {n}", "en retard {n}", "behind {n}", "indietro {n}", "zurück {n}", "atrás {n}"),
    texto!("tela.pino_a_frente_de", "à frente {n}", "en avance {n}", "ahead {n}", "avanti {n}", "voraus {n}", "adelante {n}"),
    texto!("tela.pino_recusada", "recusada: {motivo}", "refusée : {motivo}", "refused: {motivo}", "rifiutata: {motivo}", "abgelehnt: {motivo}", "rechazada: {motivo}"),
    texto!("tela.pino_nao_chegou", "ainda não chegou aqui", "pas encore arrivé ici", "hasn't arrived here yet", "non ancora arrivato qui", "noch nicht hier angekommen", "todavía no llegó aquí"),

    // -------------------------------------------------- o Painel (dashboard)
    // Pedido 165: o `${carta(...)}` e o `${ficha(...)}` escondiam rotulo
    // dentro de dado, e `sem_interpolacao` apagava tudo de um golpe -- entao
    // nenhuma das cartas do Painel (o widget "a maquina" e os sete cartoes de
    // metrica) nem passava pelo conferidor. Lote coerente: e UMA tela, o
    // `vPainel()`/`maquinaHtml()` de `index.html`.
    texto!("tela.pa_maquina", "A máquina", "La machine", "The machine", "La macchina", "Die Maschine", "La máquina"),
    texto!("tela.pa_indisponivel_msg", "monitor de sistema indisponível em **{sistema}**", "moniteur système indisponible sur **{sistema}**", "system monitor unavailable on **{sistema}**", "monitor di sistema non disponibile su **{sistema}**", "Systemmonitor auf **{sistema}** nicht verfügbar", "monitor de sistema no disponible en **{sistema}**"),
    texto!("tela.pa_indisponivel_corpo", "CPU, memória e rede saem do /proc, que só existe no Linux. O espaço em disco continua valendo — ele vem do `df`.", "CPU, mémoire et réseau viennent de /proc, qui n'existe que sous Linux. L'espace disque reste valable — il vient de `df`.", "CPU, memory and network come from /proc, which only exists on Linux. Disk space still works — it comes from `df`.", "CPU, memoria e rete provengono da /proc, che esiste solo su Linux. Lo spazio su disco resta valido — viene da `df`.", "CPU, Speicher und Netzwerk stammen aus /proc, das es nur unter Linux gibt. Der Festplattenspeicher bleibt gültig — er kommt von `df`.", "CPU, memoria y red vienen de /proc, que solo existe en Linux. El espacio en disco sigue funcionando — viene de `df`."),
    texto!("tela.pa_espaco_disco", "Espaço em disco", "Espace disque", "Disk space", "Spazio su disco", "Festplattenspeicher", "Espacio en disco"),
    texto!("tela.pa_espaco_disco_leg", "cada caminho que este servidor usa", "chaque chemin que ce serveur utilise", "every path this server uses", "ogni percorso usato da questo server", "jeder Pfad, den dieser Server nutzt", "cada ruta que este servidor usa"),
    texto!("tela.pa_primeira_leitura", "primeira leitura · a taxa aparece na próxima, porque taxa precisa de dois instantes", "première lecture · le taux apparaît à la prochaine, car un taux a besoin de deux instants", "first reading · the rate appears on the next one, because a rate needs two instants", "prima lettura · il tasso appare alla prossima, perché un tasso richiede due istanti", "erste Messung · die Rate erscheint bei der nächsten, weil eine Rate zwei Zeitpunkte braucht", "primera lectura · la tasa aparece en la próxima, porque una tasa necesita dos instantes"),
    texto!("tela.pa_medido_ha", "medido nos últimos {s} s", "mesuré sur les {s} dernières s", "measured over the last {s} s", "misurato negli ultimi {s} s", "gemessen in den letzten {s} s", "medido en los últimos {s} s"),
    texto!("tela.pa_placas_rede", "Placas de rede", "Cartes réseau", "Network cards", "Schede di rete", "Netzwerkkarten", "Tarjetas de red"),
    texto!("tela.pa_aguardando", "aguardando o segundo instante", "en attente du second instant", "waiting for the second instant", "in attesa del secondo istante", "wartet auf den zweiten Zeitpunkt", "esperando el segundo instante"),
    texto!("tela.pa_trafego_leg", "tráfego por interface", "trafic par interface", "traffic by interface", "traffico per interfaccia", "Datenverkehr pro Schnittstelle", "tráfico por interfaz"),
    texto!("tela.pa_trafego_titulo", "Tráfego por placa de rede", "Trafic par carte réseau", "Traffic by network card", "Traffico per scheda di rete", "Datenverkehr pro Netzwerkkarte", "Tráfico por tarjeta de red"),
    texto!("tela.pa_discos_titulo", "Discos", "Disques", "Disks", "Dischi", "Festplatten", "Discos"),
    texto!("tela.pa_disco_leg", "leitura e escrita por disco físico", "lecture et écriture par disque physique", "reads and writes by physical disk", "lettura e scrittura per disco fisico", "Lese- und Schreibvorgänge pro physischer Festplatte", "lectura y escritura por disco físico"),
    texto!("tela.pa_disco_titulo_barras", "Leitura e escrita por disco", "Lecture et écriture par disque", "Reads and writes by disk", "Lettura e scrittura per disco", "Lese- und Schreibvorgänge pro Festplatte", "Lectura y escritura por disco"),
    // Compartilhada por `barras()`, `anel()` e `barrasCheias()` -- os tres
    // desenhistas de grafico do painel, usados em bem mais telas que so o
    // Painel. Prefixo `grafico_`, e nao `pa_`, porque o texto nao e do
    // Painel: e do desenhista.
    texto!("tela.grafico_sem_dados", "sem dados ainda", "pas encore de données", "no data yet", "nessun dato ancora", "noch keine Daten", "sin datos todavía"),
    texto!("tela.pa_ops_hora_titulo", "Operações por hora", "Opérations par heure", "Operations per hour", "Operazioni per ora", "Vorgänge pro Stunde", "Operaciones por hora"),
    texto!("tela.pa_ops_hora_leg", "últimas 24 horas · as barras vermelhas são as recusadas", "dernières 24 heures · les barres rouges sont les refusées", "last 24 hours · the red bars are the refused ones", "ultime 24 ore · le barre rosse sono quelle rifiutate", "letzte 24 Stunden · die roten Balken sind die abgelehnten", "últimas 24 horas · las barras rojas son las rechazadas"),
    texto!("tela.pa_ops_pedidas_titulo", "Operações mais pedidas", "Opérations les plus demandées", "Most requested operations", "Operazioni più richieste", "Meistgefragte Vorgänge", "Operaciones más pedidas"),
    texto!("tela.pa_ops_pedidas_leg", "verde é o que passou, vermelho o que foi recusado", "vert c'est ce qui est passé, rouge ce qui a été refusé", "green is what went through, red is what was refused", "verde è ciò che è passato, rosso ciò che è stato rifiutato", "grün ist, was durchging, rot, was abgelehnt wurde", "verde es lo que pasó, rojo lo que fue rechazado"),
    texto!("tela.pa_usuarios_nivel_titulo", "Usuários por nível", "Utilisateurs par niveau", "Users by level", "Utenti per livello", "Benutzer nach Rolle", "Usuarios por nivel"),
    texto!("tela.pa_usuarios_nivel_leg", "quem pode o quê, do config.json", "qui peut quoi, depuis le config.json", "who can do what, from config.json", "chi può fare cosa, dal config.json", "wer was darf, aus der config.json", "quién puede qué, desde config.json"),
    texto!("tela.pa_maiores_tabelas_titulo", "Maiores tabelas", "Plus grandes tables", "Largest tables", "Tabelle più grandi", "Größte Tabellen", "Tablas más grandes"),
    texto!("tela.pa_maiores_tabelas_leg", "por registro · o tamanho é o do .reg", "par enregistrement · la taille est celle du .reg", "by record · the size is that of the .reg", "per record · la dimensione è quella del .reg", "nach Datensatz · die Größe ist die der .reg", "por registro · el tamaño es el del .reg"),
    texto!("tela.pa_de_onde_titulo", "De onde vêm", "D'où ça vient", "Where from", "Da dove arrivano", "Woher sie kommen", "De dónde vienen"),
    texto!("tela.pa_de_onde_leg", "por IP · vermelho é o que foi recusado", "par IP · rouge c'est ce qui a été refusé", "by IP · red is what was refused", "per IP · rosso è ciò che è stato rifiutato", "nach IP · rot ist, was abgelehnt wurde", "por IP · rojo es lo que fue rechazado"),
    texto!("tela.pa_acessos_ip_titulo", "Acessos por IP", "Accès par IP", "Access by IP", "Accessi per IP", "Zugriffe nach IP", "Accesos por IP"),
    texto!("tela.pa_bancos_titulo", "Bancos", "Bases", "Databases", "Basi", "Datenbanken", "Bases"),
    texto!("tela.pa_bancos_leg", "tabelas por banco de dados", "tables par base de données", "tables by database", "tabelle per database", "Tabellen pro Datenbank", "tablas por base de datos"),
    texto!("tela.pa_tabelas_banco_titulo", "Tabelas por banco", "Tables par base", "Tables by database", "Tabelle per base", "Tabellen pro Datenbank", "Tablas por base"),
    texto!("tela.pa_quem_mais_titulo", "Quem mais usou", "Qui a le plus utilisé", "Who used it most", "Chi ha usato di più", "Wer am meisten genutzt hat", "Quién más lo usó"),
    texto!("tela.pa_quem_mais_leg", "por login, no log inteiro", "par identifiant, sur tout le journal", "by login, across the whole log", "per login, sull'intero log", "nach Login, im gesamten Protokoll", "por login, en todo el registro"),
    texto!("tela.pa_acessos_usuario_titulo", "Acessos por usuário", "Accès par utilisateur", "Access by user", "Accessi per utente", "Zugriffe nach Benutzer", "Accesos por usuario"),
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
    let placar = crate::conferidor::Placar::medir();
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
        // O placar do conferidor. A tela nao DIGITA quanto da interface ja
        // passa pela fabrica: ela pergunta a quem conta. Numero digitado a
        // mao envelhece calado -- e este envelheceria a cada tela nova.
        ("na_fabrica", Json::de_u64(placar.cobertos as u64)),
        ("fora_da_fabrica", Json::de_u64(placar.fora as u64)),
        ("cobertura", Json::de_u64(placar.por_cento() as u64)),
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

    /// Etiqueta CRUA num texto de fabrica nunca vira etiqueta na tela.
    ///
    /// Os dois caminhos escapam antes de escrever: o `data-txt` do
    /// `aplicarIdioma` grava por `textContent`, e o `marcado()` chama `esc()`
    /// antes de marcar. Um `<b>` gravado na celula apareceria escrito, com
    /// sinal de menor e tudo -- e apareceria so no idioma de quem digitou.
    ///
    /// Escapar e a decisao certa e nao se muda: o texto vem de
    /// `phxsys.mensagens`, que um administrador edita pela grade, e celula
    /// editavel e entrada de usuario. Aceitar `<b>` cru seria aceitar
    /// `<script>` junto. A enfase e MARCA -- `**assim**`, ou a palavra entre
    /// crases -- e o corte em etiqueta acontece depois da traducao.
    ///
    /// **Prova real, com o defeito reposto:** troque um `**qualquer
    /// navegador**` de volta por `<b>qualquer navegador</b>` na
    /// `FABRICA_TELA` e este teste reprova, nomeando a chave.
    #[test]
    fn nenhum_texto_da_fabrica_traz_etiqueta_crua() {
        for f in FABRICA_TELA {
            for (i, t) in f.textos.iter().enumerate() {
                assert!(
                    !t.contains('<') && !t.contains('>'),
                    "{} em {}: etiqueta crua no texto de fabrica -- a enfase e \
                     marca (**assim** ou entre crases), porque a pagina escapa \
                     antes de escrever: {t:?}",
                    f.nome,
                    IDIOMAS[i]
                );
            }
        }
    }

    /// Marca aberta e nao fechada aparece na tela como asterisco ou crase.
    ///
    /// E o erro de digitacao mais provavel de uma traducao -- quem escreve em
    /// alemao mexe na frase inteira -- e o mais silencioso, porque so aparece
    /// naquele idioma. Contar e barato; ler seis colunas a olho, nao.
    #[test]
    fn as_marcas_de_enfase_fecham() {
        for f in FABRICA_TELA {
            for (i, t) in f.textos.iter().enumerate() {
                assert_eq!(
                    t.matches("**").count() % 2,
                    0,
                    "{} em {}: ** aberto e nao fechado: {t:?}",
                    f.nome,
                    IDIOMAS[i]
                );
                assert_eq!(
                    t.matches('`').count() % 2,
                    0,
                    "{} em {}: crase aberta e nao fechada: {t:?}",
                    f.nome,
                    IDIOMAS[i]
                );
            }
        }
    }

    /// Marcador `{assim}` que existe numa lingua e some noutra vira buraco na
    /// tela: o numero, o nome do monitor ou o rotulo da aba nao aparecem.
    ///
    /// A conferencia e contra o PORTUGUES, que e o degrau 2 e a origem de toda
    /// traducao. Reordenar os marcadores e livre -- e para isso que eles sao
    /// posicionais por nome; perder um, nao.
    #[test]
    fn todo_idioma_tem_os_mesmos_marcadores_do_portugues() {
        for f in FABRICA_TELA {
            let base = marcadores(f.textos[0]);
            for (i, t) in f.textos.iter().enumerate().skip(1) {
                if t.is_empty() {
                    continue; // celula vazia cai no portugues, e ele ja tem
                }
                assert_eq!(
                    marcadores(t),
                    base,
                    "{} em {}: os marcadores nao batem com os do portugues",
                    f.nome,
                    IDIOMAS[i]
                );
            }
        }
    }

    /// Os `{nome}` de um texto, em ordem alfabetica e sem repetir.
    fn marcadores(texto: &str) -> Vec<String> {
        let mut achados: Vec<String> = Vec::new();
        let mut resto = texto;
        while let Some(i) = resto.find('{') {
            resto = &resto[i + 1..];
            let Some(f) = resto.find('}') else { break };
            let nome = &resto[..f];
            if !nome.is_empty()
                && nome.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !achados.iter().any(|a| a == nome)
            {
                achados.push(nome.to_string());
            }
            resto = &resto[f + 1..];
        }
        achados.sort();
        achados
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
            // O esquema da tabela e `TextName Str(80)` e idioma `Str(250)`.
            // Texto que nao cabe na coluna nao e erro de digitacao: e uma
            // linha que a carga recusa, e a tela fica sem aquele rotulo.
            assert!(f.nome.len() <= 80, "{} passa dos 80 do TextName", f.nome);
            for (i, t) in f.textos.iter().enumerate() {
                assert!(
                    t.chars().count() <= 250 && t.len() <= 250,
                    "{} em {} passa dos 250 da coluna: {} caracteres, {} bytes",
                    f.nome,
                    IDIOMAS[i],
                    t.chars().count(),
                    t.len()
                );
            }
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
        let usados = chaves_usadas_na_pagina();
        assert!(
            usados.len() > 100,
            "so {} chaves na pagina: o laco se soltou",
            usados.len()
        );
        for nome in &usados {
            assert!(
                fabrica(nome).is_some(),
                "a pagina pede o texto {nome:?}, que nao existe na FABRICA_TELA -- \
                 chave errada nao quebra a tela, ela fica em portugues para sempre"
            );
        }
    }

    /// O outro lado do laco: texto de fabrica que NINGUEM pede.
    ///
    /// Chave morta e pior que chave faltando -- ela aparece na tabela para o
    /// tradutor, ele traduz nos seis idiomas, e nada muda na tela. O trabalho
    /// dele foi para o lixo sem aviso.
    #[test]
    fn todo_texto_da_fabrica_e_pedido_por_alguem() {
        let usados = chaves_usadas_na_pagina();
        for f in FABRICA_TELA {
            assert!(
                usados.contains(f.nome),
                "{} esta na fabrica e nenhuma tela o pede: ou a tela esqueceu \
                 de usar, ou a chave sobrou de uma troca",
                f.nome
            );
        }
    }

    /// Toda chave de texto que a interface pede.
    ///
    /// A busca e pelo PREFIXO, e nao pelas formas (`data-txt=`, `txt:`,
    /// `txt(`, o terceiro item de uma tupla): formas mudam quando alguem
    /// refatora, e um laco que so conhece as de hoje passaria a aprovar tudo
    /// em silencio no dia da mudanca. Nada mais no fonte comeca com `tela.`.
    fn chaves_usadas_na_pagina() -> HashSet<String> {
        let mut usados = HashSet::new();
        for (_, fonte) in crate::conferidor::FONTES {
            for pedaco in fonte.split(&format!("\"{PREFIXO_DA_TELA}")).skip(1) {
                if let Some(resto) = pedaco.split('"').next() {
                    let nome = format!("{PREFIXO_DA_TELA}{resto}");
                    if resto
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                    {
                        usados.insert(nome);
                    }
                }
            }
        }
        usados
    }
}

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
    texto!("tela.nada_aqui", "nada aqui", "rien ici", "nothing here", "niente qui", "nichts hier", "nada aquí"),
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
    texto!("tela.fer_config", "Config", "Config", "Config", "Config", "Konfig", "Config"),
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

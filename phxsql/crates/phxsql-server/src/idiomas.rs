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

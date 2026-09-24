//! Os textos da tela do PhxZip, nos seis idiomas da fabrica do PhxSql.
//!
//! O MOTOR e o da fabrica (`phxsql_server::idiomas`): o mesmo tipo
//! `TextoDeFabrica`, a mesma resolucao `resolver_um` (idioma pedido -> vazio
//! cai no portugues), os mesmos seis `IDIOMAS` na mesma ordem de colunas. O
//! que mora aqui e so a LISTA do PhxZip, com o prefixo `zip.` -- e ela mora
//! aqui, e nao na `FABRICA_TELA`, porque la cada chave e conferida contra a
//! pagina do Centro de Controle, e uma chave que so o PhxZip pede seria
//! «chave morta» para aquela guarda.
//!
//! O PhxZip nao tem banco, entao a resolucao para no degrau 3 (a fabrica):
//! nao ha a tabela `phxsys.mensagens` para alguem sobrescrever um texto.
//!
//! Marcadores entre chaves (`{n}`, `{bytes}`) sao DADO que a tela encaixa ja
//! formatado no idioma; ha teste que exige os mesmos marcadores nos seis.

use phxsql_core::json::Json;
use phxsql_server::idiomas::{indice_do_idioma, resolver_um, TextoDeFabrica, IDIOMAS};

/// Uma linha, na ordem das colunas da fabrica: Portugues, Frances, Ingles,
/// Italiano, Alemao, Espanhol.
macro_rules! texto {
    ($nome:literal, $pt:literal, $fr:literal, $en:literal, $it:literal, $de:literal, $es:literal) => {
        TextoDeFabrica {
            nome: $nome,
            textos: [$pt, $fr, $en, $it, $de, $es],
        }
    };
}

/// Todo texto que a tela do PhxZip mostra.
pub const FABRICA_ZIP: &[TextoDeFabrica] = &[
    texto!(
        "zip.sub",
        "7z · LZMA2 · AES-256 — escrito em Rust, sem dependências",
        "7z · LZMA2 · AES-256 — écrit en Rust, sans dépendances",
        "7z · LZMA2 · AES-256 — written in Rust, zero dependencies",
        "7z · LZMA2 · AES-256 — scritto in Rust, senza dipendenze",
        "7z · LZMA2 · AES-256 — in Rust geschrieben, ohne Abhängigkeiten",
        "7z · LZMA2 · AES-256 — escrito en Rust, sin dependencias"
    ),
    texto!(
        "zip.sair",
        "Sair",
        "Déconnexion",
        "Sign out",
        "Esci",
        "Abmelden",
        "Salir"
    ),
    texto!(
        "zip.login_titulo",
        "Entrar",
        "Connexion",
        "Sign in",
        "Accedi",
        "Anmelden",
        "Entrar"
    ),
    texto!(
        "zip.usuario",
        "Usuário",
        "Utilisateur",
        "User",
        "Utente",
        "Benutzer",
        "Usuario"
    ),
    texto!(
        "zip.senha",
        "Senha",
        "Mot de passe",
        "Password",
        "Password",
        "Passwort",
        "Contraseña"
    ),
    texto!(
        "zip.entrar",
        "Entrar",
        "Se connecter",
        "Sign in",
        "Accedi",
        "Anmelden",
        "Entrar"
    ),
    texto!(
        "zip.logado_como",
        "logado: {usuario}",
        "connecté : {usuario}",
        "signed in: {usuario}",
        "connesso: {usuario}",
        "angemeldet: {usuario}",
        "conectado: {usuario}"
    ),
    texto!(
        "zip.compactar_titulo",
        "Compactar",
        "Compresser",
        "Compress",
        "Comprimi",
        "Komprimieren",
        "Comprimir"
    ),
    texto!(
        "zip.soltar_titulo",
        "Arraste arquivos ou pastas para cá",
        "Glissez des fichiers ou des dossiers ici",
        "Drag files or folders here",
        "Trascina qui file o cartelle",
        "Dateien oder Ordner hierher ziehen",
        "Arrastre archivos o carpetas aquí"
    ),
    texto!(
        "zip.soltar_ou",
        "ou clique para escolher",
        "ou cliquez pour choisir",
        "or click to choose",
        "oppure fai clic per scegliere",
        "oder klicken zum Auswählen",
        "o haga clic para elegir"
    ),
    texto!(
        "zip.soltar_7z",
        "Arraste o .7z ou .phz para cá",
        "Glissez le .7z ou .phz ici",
        "Drag the .7z or .phz here",
        "Trascina qui il .7z o .phz",
        "Die .7z- oder .phz-Datei hierher ziehen",
        "Arrastre el .7z o .phz aquí"
    ),
    texto!(
        "zip.col_nome",
        "Nome",
        "Nom",
        "Name",
        "Nome",
        "Name",
        "Nombre"
    ),
    texto!(
        "zip.col_tamanho",
        "Tamanho",
        "Taille",
        "Size",
        "Dimensione",
        "Größe",
        "Tamaño"
    ),
    texto!(
        "zip.col_metodo",
        "Método",
        "Méthode",
        "Method",
        "Metodo",
        "Methode",
        "Método"
    ),
    texto!(
        "zip.col_data",
        "Data",
        "Date",
        "Date",
        "Data",
        "Datum",
        "Fecha"
    ),
    texto!(
        "zip.limpar",
        "Limpar",
        "Vider",
        "Clear",
        "Svuota",
        "Leeren",
        "Limpiar"
    ),
    texto!(
        "zip.remover",
        "remover",
        "retirer",
        "remove",
        "rimuovi",
        "entfernen",
        "quitar"
    ),
    texto!(
        "zip.nivel",
        "Nível",
        "Niveau",
        "Level",
        "Livello",
        "Stufe",
        "Nivel"
    ),
    texto!(
        "zip.nivel_copy",
        "0 — sem compressão",
        "0 — sans compression",
        "0 — no compression",
        "0 — senza compressione",
        "0 — ohne Kompression",
        "0 — sin compresión"
    ),
    texto!(
        "zip.nivel_max",
        "9 — máximo",
        "9 — maximum",
        "9 — maximum",
        "9 — massimo",
        "9 — maximal",
        "9 — máximo"
    ),
    texto!(
        "zip.senha_arquivo",
        "Senha do arquivo (opcional)",
        "Mot de passe de l'archive (facultatif)",
        "Archive password (optional)",
        "Password dell'archivio (facoltativa)",
        "Archivpasswort (optional)",
        "Contraseña del archivo (opcional)"
    ),
    texto!(
        "zip.nome_saida",
        "Nome do arquivo",
        "Nom de l'archive",
        "Archive name",
        "Nome dell'archivio",
        "Archivname",
        "Nombre del archivo"
    ),
    texto!(
        "zip.nomes_visiveis",
        "nomes visíveis sem senha",
        "noms visibles sans mot de passe",
        "names visible without password",
        "nomi visibili senza password",
        "Namen ohne Passwort sichtbar",
        "nombres visibles sin contraseña"
    ),
    texto!(
        "zip.compactar",
        "Compactar e baixar",
        "Compresser et télécharger",
        "Compress and download",
        "Comprimi e scarica",
        "Komprimieren und herunterladen",
        "Comprimir y descargar"
    ),
    texto!(
        "zip.abrir_titulo",
        "Abrir .7z",
        "Ouvrir un .7z",
        "Open .7z",
        "Apri .7z",
        ".7z öffnen",
        "Abrir .7z"
    ),
    texto!(
        "zip.abrir",
        "Abrir",
        "Ouvrir",
        "Open",
        "Apri",
        "Öffnen",
        "Abrir"
    ),
    texto!(
        "zip.testar",
        "Testar integridade",
        "Tester l'intégrité",
        "Test integrity",
        "Verifica integrità",
        "Integrität prüfen",
        "Probar integridad"
    ),
    texto!(
        "zip.baixar",
        "baixar",
        "télécharger",
        "download",
        "scarica",
        "herunterladen",
        "descargar"
    ),
    texto!(
        "zip.filtrar",
        "filtrar por nome…",
        "filtrer par nom…",
        "filter by name…",
        "filtra per nome…",
        "nach Name filtern…",
        "filtrar por nombre…"
    ),
    texto!(
        "zip.arquivo_1",
        "1 arquivo",
        "1 fichier",
        "1 file",
        "1 file",
        "1 Datei",
        "1 archivo"
    ),
    texto!(
        "zip.arquivos_n",
        "{n} arquivos",
        "{n} fichiers",
        "{n} files",
        "{n} file",
        "{n} Dateien",
        "{n} archivos"
    ),
    texto!(
        "zip.total_fila",
        "{arquivos} · {bytes} bytes",
        "{arquivos} · {bytes} octets",
        "{arquivos} · {bytes} bytes",
        "{arquivos} · {bytes} byte",
        "{arquivos} · {bytes} Bytes",
        "{arquivos} · {bytes} bytes"
    ),
    texto!(
        "zip.feito_compactar",
        "{arquivos} → {antes} bytes viraram {depois} bytes ({pct}) em {ms} ms",
        "{arquivos} → {antes} octets sont devenus {depois} octets ({pct}) en {ms} ms",
        "{arquivos} → {antes} bytes became {depois} bytes ({pct}) in {ms} ms",
        "{arquivos} → {antes} byte sono diventati {depois} byte ({pct}) in {ms} ms",
        "{arquivos} → aus {antes} Bytes wurden {depois} Bytes ({pct}) in {ms} ms",
        "{arquivos} → {antes} bytes pasaron a {depois} bytes ({pct}) en {ms} ms"
    ),
    texto!(
        "zip.feito_listar",
        "{n} entradas",
        "{n} entrées",
        "{n} entries",
        "{n} voci",
        "{n} Einträge",
        "{n} entradas"
    ),
    texto!(
        "zip.feito_listar_cifrado",
        "{n} entradas · os nomes estavam cifrados",
        "{n} entrées · les noms étaient chiffrés",
        "{n} entries · the names were encrypted",
        "{n} voci · i nomi erano cifrati",
        "{n} Einträge · die Namen waren verschlüsselt",
        "{n} entradas · los nombres estaban cifrados"
    ),
    texto!(
        "zip.feito_testar",
        "Tudo certo: {n} entradas, todos os CRC conferidos em {ms} ms",
        "Tout est bon : {n} entrées, tous les CRC vérifiés en {ms} ms",
        "All good: {n} entries, every CRC checked in {ms} ms",
        "Tutto a posto: {n} voci, tutti i CRC verificati in {ms} ms",
        "Alles in Ordnung: {n} Einträge, alle CRC in {ms} ms geprüft",
        "Todo correcto: {n} entradas, todos los CRC verificados en {ms} ms"
    ),
    texto!(
        "zip.total_arquivo",
        "{arquivos} · {bytes} bytes descompactados · {no7z} no .7z ({pct})",
        "{arquivos} · {bytes} octets décompressés · {no7z} dans le .7z ({pct})",
        "{arquivos} · {bytes} bytes uncompressed · {no7z} in the .7z ({pct})",
        "{arquivos} · {bytes} byte decompressi · {no7z} nel .7z ({pct})",
        "{arquivos} · {bytes} Bytes entpackt · {no7z} in der .7z ({pct})",
        "{arquivos} · {bytes} bytes descomprimidos · {no7z} en el .7z ({pct})"
    ),
    texto!(
        "zip.escolha_antes",
        "Escolha o arquivo primeiro.",
        "Choisissez d'abord le fichier.",
        "Choose the file first.",
        "Scegli prima il file.",
        "Wählen Sie zuerst die Datei.",
        "Elija primero el archivo."
    ),
    texto!(
        "zip.senha_necessaria",
        "Arquivo cifrado: informe a senha.",
        "Archive chiffrée : saisissez le mot de passe.",
        "Encrypted archive: enter the password.",
        "Archivio cifrato: inserisci la password.",
        "Verschlüsseltes Archiv: Passwort eingeben.",
        "Archivo cifrado: introduzca la contraseña."
    ),
    texto!(
        "zip.senha_errada",
        "Senha errada ou arquivo corrompido.",
        "Mot de passe erroné ou archive corrompue.",
        "Wrong password or corrupted archive.",
        "Password errata o archivio danneggiato.",
        "Falsches Passwort oder beschädigtes Archiv.",
        "Contraseña incorrecta o archivo dañado."
    ),
    texto!(
        "zip.recusado",
        "Método {metodo} recusado: o PhxZip lê só Copy, LZMA, LZMA2 e 7zAES.",
        "Méthode {metodo} refusée : PhxZip ne lit que Copy, LZMA, LZMA2 et 7zAES.",
        "Method {metodo} refused: PhxZip reads only Copy, LZMA, LZMA2 and 7zAES.",
        "Metodo {metodo} rifiutato: PhxZip legge solo Copy, LZMA, LZMA2 e 7zAES.",
        "Methode {metodo} abgelehnt: PhxZip liest nur Copy, LZMA, LZMA2 und 7zAES.",
        "Método {metodo} rechazado: PhxZip solo lee Copy, LZMA, LZMA2 y 7zAES."
    ),
    texto!(
        "zip.corrompido",
        "Arquivo corrompido.",
        "Archive corrompue.",
        "Corrupted archive.",
        "Archivio danneggiato.",
        "Beschädigtes Archiv.",
        "Archivo dañado."
    ),
    texto!(
        "zip.teto",
        "Acima do teto desta porta.",
        "Au-dessus de la limite de ce port.",
        "Above this port's limit.",
        "Oltre il limite di questa porta.",
        "Über dem Limit dieses Ports.",
        "Por encima del límite de este puerto."
    ),
    texto!(
        "zip.caminho",
        "Nome de entrada inseguro (sairia da pasta).",
        "Nom d'entrée dangereux (il sortirait du dossier).",
        "Unsafe entry name (would leave the folder).",
        "Nome di voce non sicuro (uscirebbe dalla cartella).",
        "Unsicherer Eintragsname (würde den Ordner verlassen).",
        "Nombre de entrada inseguro (saldría de la carpeta)."
    ),
    texto!(
        "zip.login",
        "Entre com usuário e senha.",
        "Connectez-vous avec utilisateur et mot de passe.",
        "Sign in with user and password.",
        "Accedi con utente e password.",
        "Mit Benutzer und Passwort anmelden.",
        "Entre con usuario y contraseña."
    ),
    texto!(
        "zip.login_errado",
        "Usuário ou senha errados.",
        "Utilisateur ou mot de passe erroné.",
        "Wrong user or password.",
        "Utente o password errati.",
        "Falscher Benutzer oder falsches Passwort.",
        "Usuario o contraseña incorrectos."
    ),
    texto!(
        "zip.uso",
        "Pedido inválido.",
        "Requête invalide.",
        "Invalid request.",
        "Richiesta non valida.",
        "Ungültige Anfrage.",
        "Solicitud no válida."
    ),
    texto!(
        "zip.origem",
        "Pedido recusado.",
        "Requête refusée.",
        "Request refused.",
        "Richiesta rifiutata.",
        "Anfrage abgelehnt.",
        "Solicitud rechazada."
    ),
    texto!(
        "zip.rota",
        "Rota inexistente.",
        "Route inexistante.",
        "No such route.",
        "Percorso inesistente.",
        "Unbekannte Route.",
        "Ruta inexistente."
    ),
    texto!(
        "zip.interno",
        "Erro interno do servidor.",
        "Erreur interne du serveur.",
        "Internal server error.",
        "Errore interno del server.",
        "Interner Serverfehler.",
        "Error interno del servidor."
    ),
    texto!(
        "zip.rede",
        "Sem resposta do servidor.",
        "Pas de réponse du serveur.",
        "No answer from the server.",
        "Nessuna risposta dal server.",
        "Keine Antwort vom Server.",
        "Sin respuesta del servidor."
    ),
    texto!(
        "zip.confirmar_senha",
        "Repita a senha",
        "Répétez le mot de passe",
        "Repeat the password",
        "Ripeti la password",
        "Passwort wiederholen",
        "Repita la contraseña"
    ),
    texto!(
        "zip.mostrar_senha",
        "Mostrar a senha",
        "Afficher le mot de passe",
        "Show password",
        "Mostra la password",
        "Passwort anzeigen",
        "Mostrar la contraseña"
    ),
    texto!(
        "zip.ocultar_senha",
        "Ocultar a senha",
        "Masquer le mot de passe",
        "Hide password",
        "Nascondi la password",
        "Passwort verbergen",
        "Ocultar la contraseña"
    ),
    texto!(
        "zip.senha_difere",
        "As duas senhas não conferem. Uma senha digitada errado tranca o arquivo para sempre — nem o 7-Zip o abre.",
        "Les deux mots de passe ne correspondent pas. Un mot de passe mal saisi verrouille l’archive pour toujours — même 7-Zip ne l’ouvre pas.",
        "The two passwords do not match. A mistyped password locks the archive forever — not even 7-Zip can open it.",
        "Le due password non coincidono. Una password digitata male blocca l’archivio per sempre — nemmeno 7-Zip lo apre.",
        "Die beiden Passwörter stimmen nicht überein. Ein vertipptes Passwort sperrt das Archiv für immer — nicht einmal 7-Zip öffnet es.",
        "Las dos contraseñas no coinciden. Una contraseña mal escrita bloquea el archivo para siempre — ni 7-Zip lo abre."
    ),
    texto!(
        "zip.teto_antes",
        "Nada foi enviado: {bytes} bytes passam do teto desta porta ({teto} bytes). Divida em partes menores.",
        "Rien n’a été envoyé : {bytes} octets dépassent la limite de ce port ({teto} octets). Divisez en parties plus petites.",
        "Nothing was sent: {bytes} bytes exceed this port's limit ({teto} bytes). Split it into smaller parts.",
        "Nulla è stato inviato: {bytes} byte superano il limite di questa porta ({teto} byte). Dividi in parti più piccole.",
        "Nichts wurde gesendet: {bytes} Bytes überschreiten das Limit dieses Ports ({teto} Bytes). In kleinere Teile aufteilen.",
        "No se envió nada: {bytes} bytes superan el límite de este puerto ({teto} bytes). Divídalo en partes más pequeñas."
    ),
    texto!(
        "zip.teto_fila",
        "A fila passa do teto desta porta ({teto} bytes): o servidor recusaria. Remova arquivos antes de compactar.",
        "La file dépasse la limite de ce port ({teto} octets) : le serveur refuserait. Retirez des fichiers avant de compresser.",
        "The queue exceeds this port's limit ({teto} bytes): the server would refuse it. Remove files before compressing.",
        "La coda supera il limite di questa porta ({teto} byte): il server la rifiuterebbe. Rimuovi file prima di comprimere.",
        "Die Warteschlange überschreitet das Limit dieses Ports ({teto} Bytes): der Server würde ablehnen. Dateien vor dem Komprimieren entfernen.",
        "La cola supera el límite de este puerto ({teto} bytes): el servidor la rechazaría. Quite archivos antes de comprimir."
    ),
    texto!(
        "zip.conferido",
        "conferido: {n} entradas abertas e CRC certo em {ms} ms",
        "vérifié : {n} entrées ouvertes, CRC correct en {ms} ms",
        "verified: {n} entries opened, CRC correct in {ms} ms",
        "verificato: {n} voci aperte, CRC corretto in {ms} ms",
        "geprüft: {n} Einträge geöffnet, CRC korrekt in {ms} ms",
        "verificado: {n} entradas abiertas, CRC correcto en {ms} ms"
    ),
    texto!(
        "zip.conferir_falhou",
        "a conferência FALHOU e o arquivo não foi entregue: {erro}",
        "la vérification a ÉCHOUÉ et l’archive n’a pas été livrée : {erro}",
        "verification FAILED and the archive was not delivered: {erro}",
        "la verifica è FALLITA e l’archivio non è stato consegnato: {erro}",
        "die Prüfung ist FEHLGESCHLAGEN, das Archiv wurde nicht ausgeliefert: {erro}",
        "la verificación FALLÓ y el archivo no se entregó: {erro}"
    ),
    texto!(
        "zip.nao_conferido_teto",
        "não conferido: o arquivo gerado passa do teto desta porta",
        "non vérifié : l’archive produite dépasse la limite de ce port",
        "not verified: the generated archive exceeds this port's limit",
        "non verificato: l’archivio generato supera il limite di questa porta",
        "nicht geprüft: das erzeugte Archiv überschreitet das Limit dieses Ports",
        "no verificado: el archivo generado supera el límite de este puerto"
    ),
    texto!(
        "zip.dica_0",
        "só guarda, sem comprimir — o mais rápido",
        "stocke seulement, sans compresser — le plus rapide",
        "store only, no compression — the fastest",
        "solo archiviazione, senza compressione — il più veloce",
        "nur speichern, ohne Kompression — am schnellsten",
        "solo guarda, sin comprimir — el más rápido"
    ),
    texto!(
        "zip.dica_1",
        "rápido, arquivo maior",
        "rapide, archive plus grande",
        "fast, larger archive",
        "veloce, archivio più grande",
        "schnell, größeres Archiv",
        "rápido, archivo más grande"
    ),
    texto!(
        "zip.dica_3",
        "rápido, com um pouco mais de compressão",
        "rapide, avec un peu plus de compression",
        "fast, with a bit more compression",
        "veloce, con un po’ più di compressione",
        "schnell, mit etwas mehr Kompression",
        "rápido, con algo más de compresión"
    ),
    texto!(
        "zip.dica_5",
        "equilíbrio entre tamanho e tempo — o padrão",
        "équilibre entre taille et temps — par défaut",
        "balance of size and time — the default",
        "equilibrio tra dimensione e tempo — predefinito",
        "Gleichgewicht zwischen Größe und Zeit — Standard",
        "equilibrio entre tamaño y tiempo — el predeterminado"
    ),
    texto!(
        "zip.dica_7",
        "arquivo menor, mais lento",
        "archive plus petite, plus lent",
        "smaller archive, slower",
        "archivio più piccolo, più lento",
        "kleineres Archiv, langsamer",
        "archivo más pequeño, más lento"
    ),
    texto!(
        "zip.dica_9",
        "o menor arquivo, o mais lento",
        "l’archive la plus petite, le plus lent",
        "the smallest archive, the slowest",
        "l’archivio più piccolo, il più lento",
        "das kleinste Archiv, am langsamsten",
        "el archivo más pequeño, el más lento"
    ),
    texto!(
        "zip.enviando",
        "Enviando… {pct}",
        "Envoi… {pct}",
        "Uploading… {pct}",
        "Invio… {pct}",
        "Senden… {pct}",
        "Enviando… {pct}"
    ),
    texto!(
        "zip.processando",
        "Processando no servidor…",
        "Traitement sur le serveur…",
        "Processing on the server…",
        "Elaborazione sul server…",
        "Verarbeitung auf dem Server…",
        "Procesando en el servidor…"
    ),
    texto!(
        "zip.recebendo",
        "Recebendo… {pct}",
        "Réception… {pct}",
        "Downloading… {pct}",
        "Ricezione… {pct}",
        "Empfangen… {pct}",
        "Recibiendo… {pct}"
    ),
    texto!(
        "zip.cancelar",
        "Cancelar",
        "Annuler",
        "Cancel",
        "Annulla",
        "Abbrechen",
        "Cancelar"
    ),
    texto!(
        "zip.cancelado",
        "Cancelado. Nada foi baixado nem gravado.",
        "Annulé. Rien n’a été téléchargé ni enregistré.",
        "Cancelled. Nothing was downloaded or saved.",
        "Annullato. Nulla è stato scaricato né salvato.",
        "Abgebrochen. Nichts wurde heruntergeladen oder gespeichert.",
        "Cancelado. No se descargó ni guardó nada."
    ),
    texto!(
        "zip.extrair_tudo",
        "Extrair tudo",
        "Tout extraire",
        "Extract all",
        "Estrai tutto",
        "Alles extrahieren",
        "Extraer todo"
    ),
    texto!(
        "zip.feito_extrair_pasta",
        "{arquivos} extraídos para a pasta «{pasta}» ({bytes} bytes), com as subpastas.",
        "{arquivos} extraits dans le dossier « {pasta} » ({bytes} octets), avec les sous-dossiers.",
        "{arquivos} extracted to the folder “{pasta}” ({bytes} bytes), with subfolders.",
        "{arquivos} estratti nella cartella «{pasta}» ({bytes} byte), con le sottocartelle.",
        "{arquivos} in den Ordner „{pasta}“ extrahiert ({bytes} Bytes), mit Unterordnern.",
        "{arquivos} extraídos en la carpeta «{pasta}» ({bytes} bytes), con las subcarpetas."
    ),
    texto!(
        "zip.feito_extrair_um_a_um",
        "{arquivos} baixados um a um ({bytes} bytes). Este navegador não grava pastas, então as subpastas não foram recriadas.",
        "{arquivos} téléchargés un par un ({bytes} octets). Ce navigateur n’écrit pas de dossiers : les sous-dossiers n’ont pas été recréés.",
        "{arquivos} downloaded one by one ({bytes} bytes). This browser cannot write folders, so subfolders were not recreated.",
        "{arquivos} scaricati uno per uno ({bytes} byte). Questo browser non scrive cartelle: le sottocartelle non sono state ricreate.",
        "{arquivos} einzeln heruntergeladen ({bytes} Bytes). Dieser Browser schreibt keine Ordner, Unterordner wurden nicht nachgebildet.",
        "{arquivos} descargados uno a uno ({bytes} bytes). Este navegador no escribe carpetas, así que no se recrearon las subcarpetas."
    ),
    texto!(
        "zip.pulados",
        "{n} entradas ficaram de fora por segurança (ligação simbólica ou caminho inseguro): {nomes}",
        "{n} entrées exclues par sécurité (lien symbolique ou chemin non sûr) : {nomes}",
        "{n} entries left out for safety (symbolic link or unsafe path): {nomes}",
        "{n} voci escluse per sicurezza (collegamento simbolico o percorso non sicuro): {nomes}",
        "{n} Einträge aus Sicherheitsgründen ausgelassen (symbolischer Link oder unsicherer Pfad): {nomes}",
        "{n} entradas quedaron fuera por seguridad (enlace simbólico o ruta insegura): {nomes}"
    ),
    texto!(
        "zip.expandir",
        "Abrir a pasta",
        "Ouvrir le dossier",
        "Expand folder",
        "Espandi la cartella",
        "Ordner aufklappen",
        "Abrir la carpeta"
    ),
    texto!(
        "zip.recolher",
        "Fechar a pasta",
        "Fermer le dossier",
        "Collapse folder",
        "Comprimi la cartella",
        "Ordner zuklappen",
        "Cerrar la carpeta"
    ),
    texto!(
        "zip.tema_claro",
        "Mudar para o tema claro",
        "Passer au thème clair",
        "Switch to light theme",
        "Passa al tema chiaro",
        "Zum hellen Design wechseln",
        "Cambiar al tema claro"
    ),
    texto!(
        "zip.tema_escuro",
        "Mudar para o tema escuro",
        "Passer au thème sombre",
        "Switch to dark theme",
        "Passa al tema scuro",
        "Zum dunklen Design wechseln",
        "Cambiar al tema oscuro"
    ),
];

/// Os textos resolvidos no idioma pedido (nome da coluna da fabrica:
/// `Portugues`, `Ingles`...; desconhecido cai no portugues, como na fabrica).
pub fn para_a_pagina(idioma: &str) -> Json {
    let i = indice_do_idioma(idioma);
    Json::objeto(vec![
        ("ok", Json::Bool(true)),
        ("idioma", Json::texto_de(IDIOMAS[i])),
        (
            "idiomas",
            Json::Lista(IDIOMAS.iter().map(|x| Json::texto_de(*x)).collect()),
        ),
        (
            "textos",
            Json::Objeto(
                FABRICA_ZIP
                    .iter()
                    .map(|f| (f.nome.to_string(), Json::texto_de(resolver_um(None, f, i))))
                    .collect(),
            ),
        ),
    ])
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::BTreeSet;

    const PAGINA: &str = include_str!("../ui/index.html");

    fn chaves_da_pagina() -> BTreeSet<String> {
        let mut v = BTreeSet::new();
        let b = PAGINA.as_bytes();
        let mut i = 0;
        while let Some(p) = PAGINA[i..].find("zip.") {
            let ini = i + p;
            let mut fim = ini + 4;
            while fim < b.len()
                && (b[fim].is_ascii_lowercase() || b[fim].is_ascii_digit() || b[fim] == b'_')
            {
                fim += 1;
            }
            // `zip.` seguido de letra e chave; `.zip.` de nome de arquivo nao.
            let antes = if ini > 0 { b[ini - 1] } else { b' ' };
            if fim > ini + 4 && (antes == b'\'' || antes == b'"') {
                v.insert(PAGINA[ini..fim].to_string());
            }
            i = fim;
        }
        v
    }

    fn marcadores(t: &str) -> BTreeSet<String> {
        let mut v = BTreeSet::new();
        let mut resto = t;
        while let Some(a) = resto.find('{') {
            let Some(f) = resto[a..].find('}') else { break };
            v.insert(resto[a + 1..a + f].to_string());
            resto = &resto[a + f + 1..];
        }
        v
    }

    #[test]
    fn toda_chave_que_a_tela_pede_existe_na_fabrica() {
        let faltam: Vec<String> = chaves_da_pagina()
            .into_iter()
            .filter(|c| !FABRICA_ZIP.iter().any(|f| f.nome == c))
            .collect();
        assert!(
            faltam.is_empty(),
            "a tela pede e a fabrica nao tem: {faltam:?}"
        );
    }

    /// Chave morta e pior que chave faltando: o tradutor a traduz nos seis
    /// idiomas e nada muda na tela.
    #[test]
    fn todo_texto_da_fabrica_e_pedido_pela_tela() {
        let pedidas = chaves_da_pagina();
        let mortas: Vec<&str> = FABRICA_ZIP
            .iter()
            .map(|f| f.nome)
            .filter(|n| !pedidas.contains(*n))
            .collect();
        assert!(mortas.is_empty(), "ninguem pede: {mortas:?}");
    }

    #[test]
    fn os_seis_idiomas_tem_os_mesmos_marcadores_e_nenhum_vazio() {
        for f in FABRICA_ZIP {
            let pt = marcadores(f.textos[0]);
            for (i, t) in f.textos.iter().enumerate() {
                assert!(!t.trim().is_empty(), "{} vazio em {}", f.nome, IDIOMAS[i]);
                assert_eq!(
                    marcadores(t),
                    pt,
                    "{} em {}: marcadores diferentes do portugues",
                    f.nome,
                    IDIOMAS[i]
                );
            }
        }
    }

    #[test]
    fn a_resolucao_e_a_da_fabrica_e_idioma_desconhecido_cai_no_portugues() {
        let j = para_a_pagina("Alemao");
        assert_eq!(
            j.campo("textos")
                .unwrap()
                .campo("zip.abrir")
                .unwrap()
                .texto(),
            Some("Öffnen")
        );
        let j = para_a_pagina("Klingon");
        assert_eq!(j.campo("idioma").unwrap().texto(), Some("Portugues"));
        assert_eq!(
            j.campo("textos")
                .unwrap()
                .campo("zip.abrir")
                .unwrap()
                .texto(),
            Some("Abrir")
        );
    }
}

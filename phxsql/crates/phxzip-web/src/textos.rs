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

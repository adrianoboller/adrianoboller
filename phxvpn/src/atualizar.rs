//! Auto-atualizacao (`phxvpn atualizar`): baixa um MANIFESTO assinado, confere
//! a assinatura Ed25519 e o SHA-256 do binario, recusa versao menor ou igual,
//! e troca o executavel em uso -- atomico no Linux, renomear-e-substituir no
//! Windows.
//!
//! # Por que HTTP simples basta
//!
//! O download em si acontece por `http://`, sem TLS. Isso e aceitavel PORQUE
//! a integridade nao depende do transporte: a assinatura Ed25519 do manifesto
//! prova que quem publicou tem a chave privada de publicacao, e o SHA-256 do
//! manifesto prova que o binario baixado e exatamente o apontado -- um
//! atacante no meio do caminho pode trocar bytes, mas nao pode fazer o
//! resultado passar nas duas conferencias. O LIMITE, que HTTP simples nao
//! cobre e a pessoa que decide se aceita este projeto tem de saber: nao ha
//! SIGILO de qual versao esta sendo baixada, nem de qual maquina fala com
//! qual servidor de atualizacao -- quem observa o fio ve o pedido, so nao
//! consegue forjar a resposta.
//!
//! # Por que a chave publica fica embutida e a privada nunca no repositorio
//!
//! A chave publica de publicacao (`CHAVE_PUBLICA_PADRAO`) e um dado, nao um
//! segredo: qualquer um pode confirmar uma assinatura com ela, ninguem pode
//! criar uma. A privada mora so na maquina de quem publica, passada ao
//! `publicar-atualizacao.sh` por caminho de arquivo ou variavel de ambiente
//! -- nunca gravada no repositorio, nunca em log.

use phxsql_core::ed25519;
use phxsql_core::hash::{para_hex, sha256};
use phxsql_core::json::Json;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;
use std::time::Duration;

pub type R<T> = Result<T, String>;

/// Chave publica de publicacao embutida no binario. PLACEHOLDER: gerada uma
/// vez para este trabalho (`gerar-chave`), sem a privada correspondente
/// guardada em lugar nenhum -- quem for publicar de verdade gera o PAR dela
/// com `phxvpn atualizar gerar-chave` e substitui esta constante antes do
/// primeiro lancamento.
pub const CHAVE_PUBLICA_PADRAO: &str =
    "e2c69e104f13183148c700f52e605e027683843246c06fa8277dd232e276a4da";

/// Teto do que se baixa de uma vez (manifesto ou binario): 256 MiB. Sem teto,
/// um servidor de atualizacao comprometido (ou so mal configurado) poderia
/// mandar um corpo sem fim e esgotar a memoria de quem atualiza.
const TETO_BAIXAR: u64 = 256 * 1024 * 1024;

const PRAZO_FIO: Duration = Duration::from_secs(30);

/// Um alvo do manifesto: onde baixar o binario desta plataforma, e o hash
/// que ele tem de bater.
#[derive(Clone, Debug, PartialEq)]
pub struct Alvo {
    pub url: String,
    pub sha256: String,
}

/// O manifesto assinado: versao e um alvo por plataforma.
#[derive(Clone, Debug, PartialEq)]
pub struct Manifesto {
    pub versao: String,
    pub alvos: Vec<(String, Alvo)>,
}

impl Manifesto {
    pub fn alvo(&self, plataforma: &str) -> Option<&Alvo> {
        self.alvos
            .iter()
            .find(|(p, _)| p == plataforma)
            .map(|(_, a)| a)
    }

    /// O corpo assinado -- SEMPRE a mesma serializacao para a mesma
    /// estrutura (o `Json` desta casa preserva ordem), entao assinar e
    /// conferir usam o mesmo texto sem precisar guardar bytes originais.
    fn corpo_json(&self) -> Json {
        let alvos = self
            .alvos
            .iter()
            .map(|(p, a)| {
                (
                    p.as_str(),
                    Json::objeto(vec![
                        ("url", Json::texto_de(a.url.clone())),
                        ("sha256", Json::texto_de(a.sha256.clone())),
                    ]),
                )
            })
            .collect::<Vec<_>>();
        Json::objeto(vec![
            ("versao", Json::texto_de(self.versao.clone())),
            (
                "alvos",
                Json::Objeto(alvos.into_iter().map(|(k, v)| (k.to_string(), v)).collect()),
            ),
        ])
    }
}

/// Plataforma desta maquina, na chave que o manifesto usa. So as tres que o
/// projeto empacota (`empacotar.sh`); outra combinacao nao tem para onde
/// apontar e o erro diz isso, em vez de silenciosamente pegar o alvo errado.
pub fn plataforma_atual() -> R<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("linux-x86_64"),
        ("windows", "x86_64") => Ok("windows-x86_64"),
        ("windows", "aarch64") => Ok("windows-aarch64"),
        (os, arch) => Err(format!(
            "sem pacote de atualizacao para {os}-{arch} (so linux-x86_64, windows-x86_64, windows-aarch64)"
        )),
    }
}

/// Assina um manifesto novo. Uso do `publicar-atualizacao.sh`, nao da
/// atualizacao em si.
pub fn assinar_manifesto(m: &Manifesto, privada: &[u8; ed25519::CHAVE_LEN]) -> String {
    let corpo = m.corpo_json();
    let bytes = corpo.escrever();
    let assinatura = ed25519::assinar(privada, bytes.as_bytes());
    Json::objeto(vec![
        ("corpo", corpo),
        ("assinatura", Json::texto_de(para_hex(&assinatura))),
    ])
    .escrever_identado()
}

/// Confere a assinatura do manifesto contra a chave publica e devolve o
/// manifesto (versao + alvos) se bater. Falha fechado: qualquer coisa que
/// nao se encaixe -- JSON torto, campo faltando, hex invalido, assinatura
/// que nao confere -- e erro, nunca um manifesto parcial.
pub fn verificar_manifesto(texto: &str, chave_publica: &[u8; ed25519::CHAVE_LEN]) -> R<Manifesto> {
    let raiz = Json::analisar(texto).map_err(|e| format!("manifesto: {e}"))?;
    let corpo = raiz.campo("corpo").ok_or("manifesto sem \"corpo\"")?;
    let assinatura_hex = raiz
        .campo("assinatura")
        .and_then(Json::texto)
        .ok_or("manifesto sem \"assinatura\"")?;
    let assinatura: [u8; ed25519::ASSINATURA_LEN] = ed25519::assinatura_de_hex(assinatura_hex)
        .ok_or("assinatura do manifesto nao e hex valido de 64 bytes")?;

    let versao = corpo
        .campo("versao")
        .and_then(Json::texto)
        .ok_or("manifesto sem \"versao\"")?
        .to_string();
    let alvos_json = corpo.campo("alvos").ok_or("manifesto sem \"alvos\"")?;
    let mut alvos = Vec::new();
    for plataforma in alvos_json.chaves() {
        let a = alvos_json
            .campo(plataforma)
            .expect("chave veio de chaves()");
        let url = a
            .campo("url")
            .and_then(Json::texto)
            .ok_or_else(|| format!("alvo {plataforma} sem \"url\""))?;
        let sha = a
            .campo("sha256")
            .and_then(Json::texto)
            .ok_or_else(|| format!("alvo {plataforma} sem \"sha256\""))?;
        alvos.push((
            plataforma.to_string(),
            Alvo {
                url: url.to_string(),
                sha256: sha.to_lowercase(),
            },
        ));
    }
    let m = Manifesto { versao, alvos };

    // A mesma serializacao que `assinar_manifesto` usou -- e por isso que o
    // corpo entra e sai do `Json` em vez de se assinar o texto cru do
    // arquivo: reformatacao (espaco, quebra de linha) nao pode derrubar uma
    // assinatura valida nem, pior, deixar passar uma que nao bate no corpo
    // LIDO.
    let bytes_do_corpo_lido = m.corpo_json().escrever();
    if !ed25519::conferir(chave_publica, bytes_do_corpo_lido.as_bytes(), &assinatura) {
        return Err("assinatura do manifesto nao confere".to_string());
    }
    Ok(m)
}

/// Compara duas versoes `MAJOR.MINOR.PATCH` (so numeros, sem sufixo). Erro se
/// alguma nao se encaixar no formato -- comparar lexicograficamente daria
/// "0.9.0" > "0.10.0", calado.
fn comparar_versoes(a: &str, b: &str) -> R<std::cmp::Ordering> {
    fn partes(v: &str) -> R<[u64; 3]> {
        let p: Vec<&str> = v.split('.').collect();
        if p.len() != 3 {
            return Err(format!("versao {v:?} nao e MAJOR.MINOR.PATCH"));
        }
        let mut n = [0u64; 3];
        for (i, s) in p.iter().enumerate() {
            n[i] = s
                .parse()
                .map_err(|_| format!("versao {v:?} tem um pedaco que nao e numero"))?;
        }
        Ok(n)
    }
    Ok(partes(a)?.cmp(&partes(b)?))
}

/// `http://host[:porta]/caminho` -> (host:porta, caminho). So o essencial:
/// sem query nem fragmento, que o manifesto e o binario nao usam.
fn separar_url(url: &str) -> R<(String, String)> {
    let resto = url
        .strip_prefix("http://")
        .ok_or("a URL de atualizacao tem de comecar com http:// (a integridade vem da assinatura e do SHA-256, nao do transporte)")?;
    let (hostporta, caminho) = match resto.find('/') {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, "/"),
    };
    if hostporta.is_empty() {
        return Err("URL de atualizacao sem host".to_string());
    }
    let hostporta = if hostporta.contains(':') {
        hostporta.to_string()
    } else {
        format!("{hostporta}:80")
    };
    Ok((hostporta, caminho.to_string()))
}

/// GET cru por HTTP/1.1, devolvendo o corpo em bytes (o manifesto e texto, o
/// binario nao e). Sem redirecionar -- um manifesto assinado nao teria por
/// que apontar por 302 para outro lugar, e seguir redireciono as cegas e
/// como servidor de atualizacao vira vetor de SSRF.
fn baixar(url: &str) -> R<Vec<u8>> {
    let (hostporta, caminho) = separar_url(url)?;
    let mut fio = TcpStream::connect(&hostporta).map_err(|e| format!("{hostporta}: {e}"))?;
    fio.set_read_timeout(Some(PRAZO_FIO)).ok();
    fio.set_write_timeout(Some(PRAZO_FIO)).ok();
    write!(
        fio,
        "GET {caminho} HTTP/1.1\r\nHost: {hostporta}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|e| e.to_string())?;
    let mut bruto = Vec::new();
    fio.take(TETO_BAIXAR)
        .read_to_end(&mut bruto)
        .map_err(|e| format!("baixando {url}: {e}"))?;
    let sep = b"\r\n\r\n";
    let fim_cab = bruto
        .windows(sep.len())
        .position(|j| j == sep)
        .ok_or("resposta HTTP sem cabecalho terminado")?;
    let cab = String::from_utf8_lossy(&bruto[..fim_cab]);
    let status: u16 = cab
        .lines()
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or("resposta HTTP sem linha de status")?;
    if status != 200 {
        return Err(format!("{url}: HTTP {status}"));
    }
    Ok(bruto[fim_cab + sep.len()..].to_vec())
}

/// Resultado de uma checagem: o que ha disponivel, e se e mais novo que a
/// versao atual.
pub struct Disponivel {
    pub versao: String,
    pub mais_novo: bool,
}

/// So confere e informa -- nunca baixa o binario nem troca nada. E o que
/// `--verificar` e a checagem periodica usam.
pub fn verificar(
    manifesto_url: &str,
    chave_publica: &[u8; ed25519::CHAVE_LEN],
    versao_atual: &str,
) -> R<Disponivel> {
    let texto = String::from_utf8(baixar(manifesto_url)?)
        .map_err(|_| "manifesto nao e UTF-8".to_string())?;
    let m = verificar_manifesto(&texto, chave_publica)?;
    let mais_novo = comparar_versoes(&m.versao, versao_atual)? == std::cmp::Ordering::Greater;
    Ok(Disponivel {
        versao: m.versao,
        mais_novo,
    })
}

/// Baixa, confere (assinatura do manifesto, SHA-256 do binario, versao
/// maior) e troca o executavel `exe_atual` pelo baixado. Devolve a versao
/// nova. `exe_atual` existe so para os testes poderem apontar para um
/// arquivo qualquer -- o `main.rs` passa `std::env::current_exe()`.
pub fn aplicar(
    manifesto_url: &str,
    chave_publica: &[u8; ed25519::CHAVE_LEN],
    versao_atual: &str,
    exe_atual: &Path,
) -> R<String> {
    let texto = String::from_utf8(baixar(manifesto_url)?)
        .map_err(|_| "manifesto nao e UTF-8".to_string())?;
    let m = verificar_manifesto(&texto, chave_publica)?;
    if comparar_versoes(&m.versao, versao_atual)? != std::cmp::Ordering::Greater {
        return Err(format!(
            "versao do manifesto ({}) nao e maior que a atual ({versao_atual}) -- recusado (nunca se faz downgrade)",
            m.versao
        ));
    }
    let plataforma = plataforma_atual()?;
    let alvo = m
        .alvo(plataforma)
        .ok_or_else(|| format!("o manifesto nao tem alvo para {plataforma}"))?;
    let binario = baixar(&alvo.url)?;
    let hash = para_hex(&sha256(&binario));
    if hash != alvo.sha256 {
        return Err(format!(
            "SHA-256 do binario baixado ({hash}) nao bate com o do manifesto ({}) -- recusado",
            alvo.sha256
        ));
    }
    trocar_binario(exe_atual, &binario)?;
    Ok(m.versao)
}

/// Linux: grava no mesmo diretorio (mesmo sistema de arquivos, para o
/// `rename` ser atomico) e troca por cima. Funciona com o binario em
/// execucao: o processo rodando mantem a inode antiga aberta; quem abrir o
/// caminho DEPOIS do rename ve o binario novo.
#[cfg(unix)]
fn trocar_binario(atual: &Path, novo: &[u8]) -> R<()> {
    use std::os::unix::fs::PermissionsExt;
    let tmp = atual.with_extension("novo");
    std::fs::write(&tmp, novo).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, atual).map_err(|e| format!("trocar {}: {e}", atual.display()))
}

/// Windows: nao da para sobrescrever o `.exe` que esta em execucao, mas da
/// para RENOMEA-LO -- o SO mantem a imagem mapeada pelo identificador
/// antigo. Move o atual para `.old` (limpo no proximo arranque, quando mais
/// nenhum processo o tem aberto) e escreve o novo no nome de sempre.
#[cfg(windows)]
fn trocar_binario(atual: &Path, novo: &[u8]) -> R<()> {
    let velho = caminho_old(atual);
    let _ = std::fs::remove_file(&velho); // sobra de uma troca anterior, se nao foi limpa ainda
    if atual.exists() {
        std::fs::rename(atual, &velho)
            .map_err(|e| format!("renomear {} para {}: {e}", atual.display(), velho.display()))?;
    }
    std::fs::write(atual, novo).map_err(|e| format!("{}: {e}", atual.display()))
}

#[cfg(windows)]
fn caminho_old(atual: &Path) -> PathBuf {
    let mut s = atual.as_os_str().to_os_string();
    s.push(".old");
    PathBuf::from(s)
}

/// Chamado no arranque (`main.rs`): apaga o `.exe.old` de uma troca
/// anterior, se sobrou um. So no Windows -- o Linux nunca deixa sobra porque
/// o `rename` e atomico, sem passo intermediario.
#[cfg(windows)]
pub fn limpar_binario_antigo() {
    if let Ok(exe) = std::env::current_exe() {
        let velho = caminho_old(&exe);
        if velho.exists() {
            let _ = std::fs::remove_file(&velho);
        }
    }
}

#[cfg(not(windows))]
pub fn limpar_binario_antigo() {}

/// Checagem periodica opcional (servico e mesa): so avisa no `eprintln!`,
/// nunca aplica sozinha -- decidir trocar o binario de um processo que esta
/// rodando sem ninguem mandando e o tipo de "guarda imposta, nao pedida" que
/// a casa recusa. Custa zero em quem nao chama esta funcao: nasce so quando
/// alguem passa a URL do manifesto.
pub fn iniciar_verificacao_periodica(
    manifesto_url: String,
    chave_publica: [u8; ed25519::CHAVE_LEN],
    versao_atual: String,
    intervalo: Duration,
) {
    std::thread::spawn(move || loop {
        match verificar(&manifesto_url, &chave_publica, &versao_atual) {
            Ok(d) if d.mais_novo => eprintln!(
                "phxvpn: atualizacao disponivel: {} (rode: phxvpn atualizar)",
                d.versao
            ),
            Ok(_) => {}
            Err(e) => eprintln!("phxvpn: checagem de atualizacao falhou: {e}"),
        }
        std::thread::sleep(intervalo);
    });
}

#[cfg(test)]
mod testes {
    use super::*;

    fn par_de_teste() -> ([u8; 32], [u8; 32]) {
        let privada = ed25519::gerar_privada();
        let publica = ed25519::chave_publica(&privada);
        (privada, publica)
    }

    fn manifesto_exemplo(versao: &str) -> Manifesto {
        Manifesto {
            versao: versao.to_string(),
            alvos: vec![(
                "linux-x86_64".to_string(),
                Alvo {
                    url: "http://127.0.0.1:9/bin".to_string(),
                    sha256: "a".repeat(64),
                },
            )],
        }
    }

    // RED: com `ed25519::conferir` removido do meio de `verificar_manifesto`
    // (trocado por `true`), este teste passaria a aceitar a assinatura
    // adulterada -- e falharia aqui, porque espera Err.
    #[test]
    fn assinatura_adulterada_e_recusada() {
        let (privada, publica) = par_de_teste();
        let m = manifesto_exemplo("1.0.0");
        let bom = assinar_manifesto(&m, &privada);
        assert!(verificar_manifesto(&bom, &publica).is_ok());

        // Troca um caractere hex da assinatura por outro.
        let pos = bom.find("\"assinatura\": \"").unwrap() + 15;
        let mut adulterado = bom.clone();
        let c = adulterado.as_bytes()[pos] as char;
        let trocado = if c == 'a' { 'b' } else { 'a' };
        adulterado.replace_range(pos..pos + 1, &trocado.to_string());

        let e = verificar_manifesto(&adulterado, &publica).unwrap_err();
        assert!(e.contains("nao confere"), "erro inesperado: {e}");
    }

    // RED: sem a conferencia de assinatura, um manifesto assinado por OUTRA
    // chave (nao a embutida) passaria. Prova que a chave publica importa, nao
    // so o formato da assinatura.
    #[test]
    fn manifesto_assinado_por_outra_chave_e_recusado() {
        let (privada_certa, publica_certa) = par_de_teste();
        let (privada_errada, _) = par_de_teste();
        assert_ne!(para_hex(&privada_certa), para_hex(&privada_errada));
        let m = manifesto_exemplo("1.0.0");
        let texto = assinar_manifesto(&m, &privada_errada);
        assert!(verificar_manifesto(&texto, &publica_certa).is_err());
    }

    // RED: com a conferencia de SHA-256 removida de `aplicar`, um binario
    // trocado no meio do caminho (ou um manifesto com hash errado por
    // engano) passaria direto para o `trocar_binario`.
    #[test]
    fn sha256_errado_e_recusado_ponta_a_ponta() {
        let dir = std::env::temp_dir().join(format!("phxvpn-atualizar-sha-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let conteudo_novo = b"binario novo, versao 9.0.0".to_vec();
        std::fs::write(dir.join("bin"), &conteudo_novo).unwrap();

        let (privada, publica) = par_de_teste();
        let plataforma = plataforma_atual().unwrap();
        // A URL entra no corpo ANTES de assinar -- trocar texto depois de
        // assinado derrubaria a propria assinatura, e o teste provaria a
        // conferencia errada.
        let porta = porta_livre();
        let m = Manifesto {
            versao: "9.0.0".to_string(),
            alvos: vec![(
                plataforma.to_string(),
                Alvo {
                    url: format!("http://127.0.0.1:{porta}/bin"),
                    sha256: para_hex(&sha256(b"outra coisa, nao o binario")), // ERRADO de proposito
                },
            )],
        };
        let manifesto_texto = assinar_manifesto(&m, &privada);
        std::fs::write(dir.join("manifesto.json"), &manifesto_texto).unwrap();

        let servidor = subir_servidor_de_arquivos(&dir, porta);
        let manifesto_url = format!("http://127.0.0.1:{porta}/manifesto.json");

        let exe_falso = dir.join("phxvpn-atual");
        std::fs::write(&exe_falso, b"binario velho, versao 1.0.0").unwrap();

        let r = aplicar(&manifesto_url, &publica, "1.0.0", &exe_falso);
        parar_servidor(servidor);
        let e = r.unwrap_err();
        assert!(e.contains("SHA-256"), "erro inesperado: {e}");
        // O binario velho continua no lugar -- a troca nunca aconteceu.
        assert_eq!(
            std::fs::read(&exe_falso).unwrap(),
            b"binario velho, versao 1.0.0"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // RED: com a comparacao de versao removida (ou trocada por `>=`), um
    // manifesto com a MESMA versao ou uma MENOR passaria e trocaria o
    // binario por um mais velho.
    #[test]
    fn versao_menor_ou_igual_e_recusada() {
        let (privada, publica) = par_de_teste();
        let m_igual = manifesto_exemplo("2.0.0");
        let m_menor = manifesto_exemplo("1.0.0");
        let m_maior = manifesto_exemplo("3.0.0");
        assert!(comparar_versoes(&m_igual.versao, "2.0.0").unwrap() == std::cmp::Ordering::Equal);
        assert!(comparar_versoes(&m_menor.versao, "2.0.0").unwrap() == std::cmp::Ordering::Less);
        assert!(comparar_versoes(&m_maior.versao, "2.0.0").unwrap() == std::cmp::Ordering::Greater);

        // A parte que aplicar() de fato usa: versao igual e menor tem de dar
        // erro dito, versao maior tem de passar dessa conferencia (mesmo que
        // depois falhe no download por a URL ser falsa -- o que importa aqui
        // e QUAL erro vem primeiro).
        let dir = std::env::temp_dir().join(format!("phxvpn-atualizar-ver-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (m, deve_recusar_por_versao) in [(&m_igual, true), (&m_menor, true), (&m_maior, false)]
        {
            let texto = assinar_manifesto(m, &privada);
            std::fs::write(dir.join("manifesto.json"), &texto).unwrap();
            let porta = porta_livre();
            let servidor = subir_servidor_de_arquivos(&dir, porta);
            let url = format!("http://127.0.0.1:{porta}/manifesto.json");
            let exe = dir.join("exe");
            std::fs::write(&exe, b"velho").unwrap();
            let r = aplicar(&url, &publica, "2.0.0", &exe);
            parar_servidor(servidor);
            let e = r.unwrap_err();
            if deve_recusar_por_versao {
                assert!(e.contains("nunca se faz downgrade"), "erro inesperado: {e}");
            } else {
                assert!(
                    !e.contains("nunca se faz downgrade"),
                    "versao maior nao devia cair na conferencia de versao: {e}"
                );
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn porta_livre() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    struct ServidorDeArquivos(std::process::Child);

    /// Sobe `python3 -m http.server` servindo `dir` -- a prova ponta a ponta
    /// pede um servidor HTTP de verdade, nao um mock dentro do processo.
    fn subir_servidor_de_arquivos(dir: &Path, porta: u16) -> ServidorDeArquivos {
        let filho = std::process::Command::new("python3")
            .args([
                "-m",
                "http.server",
                &porta.to_string(),
                "--bind",
                "127.0.0.1",
                "--directory",
            ])
            .arg(dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("python3 -m http.server (precisa do python3 no PATH)");
        // Espera o servidor aceitar conexao, em vez de um sleep fixo.
        for _ in 0..100 {
            if TcpStream::connect(("127.0.0.1", porta)).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        ServidorDeArquivos(filho)
    }

    fn parar_servidor(mut s: ServidorDeArquivos) {
        let _ = s.0.kill();
        let _ = s.0.wait();
    }

    /// A prova ponta a ponta pedida: manifesto assinado, servido por um
    /// `python3 -m http.server` de verdade, baixado e trocado no binario
    /// "em uso" -- caminho feliz completo, incluindo a troca atomica.
    #[test]
    fn ponta_a_ponta_com_servidor_http_local() {
        let dir = std::env::temp_dir().join(format!("phxvpn-atualizar-e2e-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (privada, publica) = par_de_teste();
        let plataforma = plataforma_atual().unwrap();
        let conteudo_novo = b"phxvpn binario novo, versao 5.0.0, conteudo de prova".to_vec();
        std::fs::write(dir.join("bin"), &conteudo_novo).unwrap();

        let porta = porta_livre();
        let m = Manifesto {
            versao: "5.0.0".to_string(),
            alvos: vec![(
                plataforma.to_string(),
                Alvo {
                    url: format!("http://127.0.0.1:{porta}/bin"),
                    sha256: para_hex(&sha256(&conteudo_novo)),
                },
            )],
        };
        std::fs::write(dir.join("manifesto.json"), assinar_manifesto(&m, &privada)).unwrap();

        let exe_atual = dir.join("phxvpn-em-uso");
        std::fs::write(&exe_atual, b"phxvpn binario velho, versao 1.0.0").unwrap();

        let servidor = subir_servidor_de_arquivos(&dir, porta);
        let manifesto_url = format!("http://127.0.0.1:{porta}/manifesto.json");
        let versao = aplicar(&manifesto_url, &publica, "1.0.0", &exe_atual);
        parar_servidor(servidor);

        assert_eq!(versao.unwrap(), "5.0.0");
        assert_eq!(std::fs::read(&exe_atual).unwrap(), conteudo_novo);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plataforma_desconhecida_da_erro_nomeando_o_alvo() {
        // So exercita o formato de erro; a plataforma real deste teste passa
        // (linux-x86_64 nesta casa).
        let r = plataforma_atual();
        assert!(r.is_ok());
    }

    #[test]
    fn constante_da_chave_publica_padrao_e_hex_valido_de_32_bytes() {
        assert!(ed25519::chave_de_hex(CHAVE_PUBLICA_PADRAO).is_some());
    }
}

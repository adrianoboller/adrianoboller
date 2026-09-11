//! Prototipo rodavel do correio nativo do PhxSql com criptografia fim-a-fim
//! e relacao de confianca -- modelo pedido pelo dono em 11/09/2026.
//!
//! O QUE ESTE EXEMPLO PROVA (nao e o formato em disco final; e a prova do modelo):
//!   1. Cada usuario tem uma identidade X25519. A chave PRIVADA fica CIFRADA sob
//!      a senha do usuario (PBKDF2 -> ChaCha20-Poly1305). A senha nunca aparece.
//!   2. Um so manda para o outro se houver relacao de CONFIANCA aceita -- vale
//!      para a mesma empresa e para empresas diferentes (o gate e o mesmo).
//!   3. A mensagem e cifrada com a chave derivada do segredo ECDH entre os dois
//!      (segredo(priv_A, pub_B) == segredo(priv_B, pub_A)). Assim CADA UM decifra
//!      com a PROPRIA senha (que abre a sua privada) + a chave publica do outro.
//!   4. O que fica gravado e ciphertext -- o texto claro nao esta la.
//!
//! So `std` + a cripto ja escrita e conferida contra vetor em phxsql-core.
//! Rodar: cargo run -q --example correio-e2e -p phxsql-core

use phxsql_core::{cifra, hkdf, x25519, CHAVE_LEN, NONCE_LEN, TAG_LEN};

const ITER: u32 = 200_000; // PBKDF2 para senha->chave
const INFO: &[u8] = b"phxsql-correio-v1"; // rotulo do HKDF

fn rnd16() -> [u8; 16] {
    let mut b = [0u8; 16];
    cifra::sortear(&mut b);
    b
}
fn rnd_nonce() -> [u8; NONCE_LEN] {
    let mut b = [0u8; NONCE_LEN];
    cifra::sortear(&mut b);
    b
}

/// Uma conta de correio. A privada NAO mora aqui em claro: mora cifrada.
#[derive(Clone)]
struct Usuario {
    endereco: String,         // adrianoboller@empresa.phxsql.com.br
    empresa: String,          // empresa.phxsql.com.br
    publica: [u8; CHAVE_LEN], // identidade X25519, publica
    sal: [u8; 16],            // sal do PBKDF2 (publico)
    nonce_priv: [u8; NONCE_LEN],
    priv_cifrada: Vec<u8>, // a privada X25519, selada sob a senha
    tag_priv: [u8; TAG_LEN],
}

fn criar_usuario(endereco: &str, empresa: &str, senha: &str) -> Usuario {
    let privada = x25519::gerar_privada();
    let publica = x25519::chave_publica(&privada);
    let sal = rnd16();
    let ksenha = cifra::chave_de_senha(senha, &sal, ITER); // PBKDF2
    let nonce_priv = rnd_nonce();
    // a privada e selada com a chave da senha; o endereco entra como AAD
    let (priv_cifrada, tag_priv) =
        cifra::selar(&ksenha, &nonce_priv, endereco.as_bytes(), &privada);
    Usuario {
        endereco: endereco.into(),
        empresa: empresa.into(),
        publica,
        sal,
        nonce_priv,
        priv_cifrada,
        tag_priv,
    }
}

/// Abre a privada do usuario com a senha dele. Senha errada -> Err (a etiqueta
/// do AEAD nao confere). E o unico jeito de obter a privada.
fn abrir_privada(u: &Usuario, senha: &str) -> Result<[u8; CHAVE_LEN], String> {
    let ksenha = cifra::chave_de_senha(senha, &u.sal, ITER);
    let claro = cifra::abrir(
        &ksenha,
        &u.nonce_priv,
        u.endereco.as_bytes(),
        &u.priv_cifrada,
        &u.tag_priv,
    )
    .map_err(|_| "senha incorreta: nao abre a chave privada".to_string())?;
    let mut p = [0u8; CHAVE_LEN];
    p.copy_from_slice(&claro);
    Ok(p)
}

struct Mensagem {
    de: String,
    para: String,
    sal_kdf: [u8; 16], // sal do HKDF, por mensagem (publico)
    nonce: [u8; NONCE_LEN],
    ct: Vec<u8>, // corpo CIFRADO
    tag: [u8; TAG_LEN],
}

/// O "server mail": guarda contas, relacoes de confianca e mensagens cifradas.
struct ServerMail {
    dominio_base: String, // phxsql.com.br
    usuarios: Vec<Usuario>,
    confiancas: Vec<(String, String, bool)>, // (de, para, aceita)
    mensagens: Vec<Mensagem>,
}

impl ServerMail {
    fn novo(dominio_base: &str) -> Self {
        ServerMail {
            dominio_base: dominio_base.into(),
            usuarios: vec![],
            confiancas: vec![],
            mensagens: vec![],
        }
    }
    fn achar(&self, endereco: &str) -> Option<&Usuario> {
        self.usuarios.iter().find(|u| u.endereco == endereco)
    }
    fn criar_conta(&mut self, local: &str, empresa: &str, senha: &str) -> String {
        let endereco = format!("{local}@{empresa}");
        self.usuarios.push(criar_usuario(&endereco, empresa, senha));
        println!("   conta criada: {endereco}  (identidade X25519, privada selada sob a senha)");
        endereco
    }
    fn solicitar_confianca(&mut self, de: &str, para: &str) {
        self.confiancas.push((de.into(), para.into(), false));
        let cruz = if self.mesma_empresa(de, para) {
            "mesma empresa"
        } else {
            "EMPRESAS DIFERENTES"
        };
        println!("   solicitacao de confianca: {de} -> {para}  [{cruz}]  (pendente de aceite)");
    }
    fn aceitar_confianca(&mut self, de: &str, para: &str) {
        let mut achou = false;
        for c in &mut self.confiancas {
            if c.0 == de && c.1 == para {
                c.2 = true;
                achou = true;
            }
        }
        if achou {
            println!("   {para} ACEITOU a relacao de confianca de {de}");
        }
    }
    fn confia(&self, a: &str, b: &str) -> bool {
        self.confiancas
            .iter()
            .any(|c| c.2 && ((c.0 == a && c.1 == b) || (c.0 == b && c.1 == a)))
    }
    fn mesma_empresa(&self, a: &str, b: &str) -> bool {
        match (self.achar(a), self.achar(b)) {
            (Some(x), Some(y)) => x.empresa == y.empresa,
            _ => false,
        }
    }

    /// Envia. RECUSA se nao houver confianca aceita. Cifra fim-a-fim com o
    /// segredo ECDH entre remetente e destinatario.
    fn enviar(
        &mut self,
        de: &str,
        senha_de: &str,
        para: &str,
        corpo: &str,
    ) -> Result<usize, String> {
        if !self.confia(de, para) {
            return Err(format!(
                "RECUSADO: nao ha relacao de confianca aceita entre {de} e {para}"
            ));
        }
        let ud = self.achar(de).ok_or("remetente inexistente")?.clone();
        let up = self.achar(para).ok_or("destinatario inexistente")?.clone();
        let priv_de = abrir_privada(&ud, senha_de)?; // senha do remetente
        let segredo = x25519::segredo(&priv_de, &up.publica).map_err(|e| e.to_string())?;
        let sal_kdf = rnd16();
        let mut kmsg = [0u8; CHAVE_LEN];
        hkdf::derivar(&sal_kdf, &segredo, INFO, &mut kmsg).map_err(|e| e.to_string())?;
        let nonce = rnd_nonce();
        let aad = format!("{de}|{para}");
        let (ct, tag) = cifra::selar(&kmsg, &nonce, aad.as_bytes(), corpo.as_bytes());
        self.mensagens.push(Mensagem {
            de: de.into(),
            para: para.into(),
            sal_kdf,
            nonce,
            ct,
            tag,
        });
        Ok(self.mensagens.len() - 1)
    }

    /// Le a mensagem idx como `quem`, usando a senha DELE. Deriva o mesmo segredo
    /// ECDH (priv de quem + pub do remetente). Chave/senha erradas -> Err.
    fn ler(&self, idx: usize, quem: &str, senha: &str) -> Result<String, String> {
        let m = self.mensagens.get(idx).ok_or("mensagem inexistente")?;
        let uq = self.achar(quem).ok_or("leitor inexistente")?;
        let outro = if m.de == quem { &m.para } else { &m.de };
        let uo = self.achar(outro).ok_or("contraparte inexistente")?;
        let priv_q = abrir_privada(uq, senha)?; // senha de quem le
        let segredo = x25519::segredo(&priv_q, &uo.publica).map_err(|e| e.to_string())?;
        let mut kmsg = [0u8; CHAVE_LEN];
        hkdf::derivar(&m.sal_kdf, &segredo, INFO, &mut kmsg).map_err(|e| e.to_string())?;
        let aad = format!("{}|{}", m.de, m.para);
        let claro = cifra::abrir(&kmsg, &m.nonce, aad.as_bytes(), &m.ct, &m.tag)
            .map_err(|_| "nao decifra: chave/senha erradas ou dado adulterado".to_string())?;
        String::from_utf8(claro).map_err(|_| "utf8 invalido".to_string())
    }
}

fn main() {
    let mut res: Vec<(bool, String)> = vec![];
    let mut ok = |c: bool, n: &str| res.push((c, n.to_string()));

    println!("===== TENTATIVA: server mail PhxSql + e-mail fim-a-fim + confianca =====\n");

    // --- criar o server mail e as contas de UMA empresa ---
    println!("1) Criando o server mail e as contas de empresa.phxsql.com.br:");
    let mut srv = ServerMail::novo("phxsql.com.br");
    let a = srv.criar_conta(
        "adrianoboller",
        "empresa.phxsql.com.br",
        "senha-do-adriano-9f",
    );
    let b = srv.criar_conta("joanaprado", "empresa.phxsql.com.br", "senha-da-joana-7k");
    // uma terceira pessoa, para provar que quem nao e o par NAO le
    let c = srv.criar_conta("marcoslima", "empresa.phxsql.com.br", "senha-do-marcos-3z");
    // e uma pessoa de OUTRA empresa, para o caso inter-empresa
    let d = srv.criar_conta("carlos", "outra.phxsql.com.br", "senha-do-carlos-1w");
    ok(srv.usuarios.len() == 4, "server mail criou 4 contas");

    let corpo = "Bom dia, Joana. Segue o fechamento de setembro para sua aprovacao. -- Adriano";

    // --- 2) tentar enviar SEM confianca aceita: tem de RECUSAR ---
    println!("\n2) adrianoboller tenta enviar para joanaprado SEM confianca aceita:");
    match srv.enviar(&a, "senha-do-adriano-9f", &b, corpo) {
        Ok(_) => {
            println!("   (enviou -- ERRADO)");
            ok(false, "envio sem confianca e recusado");
        }
        Err(e) => {
            println!("   {e}");
            ok(e.contains("RECUSADO"), "envio sem confianca e recusado");
        }
    }

    // --- 3) estabelecer a relacao de confianca (solicita + aceita) ---
    println!("\n3) Relacao de confianca entre adrianoboller e joanaprado:");
    srv.solicitar_confianca(&a, &b);
    srv.aceitar_confianca(&a, &b);

    // --- 4) enviar COM confianca: tem de suceder ---
    println!("\n4) adrianoboller envia para joanaprado (com confianca):");
    let idx = match srv.enviar(&a, "senha-do-adriano-9f", &b, corpo) {
        Ok(i) => {
            println!(
                "   enviado. gravado como mensagem #{i}, CIFRADO ({} bytes de ciphertext).",
                srv.mensagens[i].ct.len()
            );
            ok(true, "envio com confianca sucede");
            i
        }
        Err(e) => {
            println!("   FALHA inesperada: {e}");
            ok(false, "envio com confianca sucede");
            usize::MAX
        }
    };

    if idx != usize::MAX {
        let m = &srv.mensagens[idx];
        // --- 5) o gravado e cifrado: o corpo em claro NAO esta nos bytes ---
        let claro_bytes = corpo.as_bytes();
        let vazou =
            m.ct.windows(claro_bytes.len().min(16))
                .any(|w| w == &claro_bytes[..claro_bytes.len().min(16)]);
        println!("\n5) O que foi gravado (primeiros 32 bytes do ciphertext):");
        let amostra: String = m.ct.iter().take(32).map(|x| format!("{x:02x}")).collect();
        println!("   {amostra}...");
        ok(!vazou, "o corpo em claro NAO aparece no dado gravado");

        // --- 6) joanaprado le com a senha DELA: tem de sair o texto certo ---
        println!("\n6) joanaprado le com a propria senha:");
        match srv.ler(idx, &b, "senha-da-joana-7k") {
            Ok(t) => {
                println!("   \"{t}\"");
                ok(
                    t == corpo,
                    "destinataria decifra o texto certo com a propria senha",
                );
            }
            Err(e) => {
                println!("   FALHA: {e}");
                ok(
                    false,
                    "destinataria decifra o texto certo com a propria senha",
                );
            }
        }

        // --- 7) senha ERRADA nao decifra ---
        println!("\n7) joanaprado tenta ler com a senha ERRADA:");
        match srv.ler(idx, &b, "senha-errada") {
            Ok(_) => {
                println!("   (decifrou -- ERRADO)");
                ok(false, "senha errada NAO decifra");
            }
            Err(e) => {
                println!("   {e}");
                ok(true, "senha errada NAO decifra");
            }
        }

        // --- 8) um terceiro (marcoslima), mesmo com a propria senha, NAO le ---
        println!("\n8) marcoslima (nao e o par) tenta ler com a propria senha:");
        match srv.ler(idx, &c, "senha-do-marcos-3z") {
            Ok(_) => {
                println!("   (leu -- ERRADO)");
                ok(false, "terceiro fora do par NAO decifra");
            }
            Err(e) => {
                println!("   {e}");
                ok(true, "terceiro fora do par NAO decifra");
            }
        }
    }

    // --- 9) inter-empresa: mesmo gate de confianca ---
    println!("\n9) adrianoboller (empresa) tenta enviar para carlos (outra empresa):");
    match srv.enviar(&a, "senha-do-adriano-9f", &d, "Ola, Carlos.") {
        Ok(_) => {
            println!("   (enviou sem confianca -- ERRADO)");
            ok(false, "inter-empresa exige confianca");
        }
        Err(e) => {
            println!("   {e}");
            ok(e.contains("RECUSADO"), "inter-empresa exige confianca");
        }
    }
    println!("   -> solicitando e aceitando a confianca inter-empresa...");
    srv.solicitar_confianca(&a, &d);
    srv.aceitar_confianca(&a, &d);
    match srv.enviar(
        &a,
        "senha-do-adriano-9f",
        &d,
        "Ola, Carlos. Teste inter-empresa.",
    ) {
        Ok(i) => {
            let lido = srv.ler(i, &d, "senha-do-carlos-1w");
            println!("   enviado; carlos leu: {:?}", lido);
            ok(
                lido.as_deref() == Ok("Ola, Carlos. Teste inter-empresa."),
                "inter-empresa entrega apos confianca",
            );
        }
        Err(e) => {
            println!("   FALHA: {e}");
            ok(false, "inter-empresa entrega apos confianca");
        }
    }

    // ---- relatorio ----
    println!("\n===== RESULTADO =====");
    let mut falhas = 0;
    for (c, n) in &res {
        if !c {
            falhas += 1;
        }
        println!("  {}  {}", if *c { " ok  " } else { "FALHA" }, n);
    }
    println!(
        "\n{} checagens, {} ok, {} falha(s).",
        res.len(),
        res.len() - falhas,
        falhas
    );
    println!(
        "{}",
        if falhas == 0 {
            "PROVA VERDE"
        } else {
            "PROVA VERMELHA"
        }
    );
    if falhas != 0 {
        std::process::exit(1);
    }
    let _ = srv.dominio_base;
}

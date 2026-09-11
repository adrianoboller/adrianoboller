//! Prototipo rodavel do correio nativo do PhxSql: criptografia fim-a-fim,
//! relacao de confianca, anti-spam por consentimento e prioridade autenticada.
//! Modelo pedido pelo dono em 11/09/2026.
//!
//! O QUE ESTE EXEMPLO PROVA (nao e o formato em disco final; e a prova do modelo):
//!   1. Cada usuario tem identidade X25519; a PRIVADA fica CIFRADA sob a senha
//!      (PBKDF2 -> ChaCha20-Poly1305). Nao abre o e-mail sem a senha.
//!   2. Nao ha contato sem aprovacao: so entrega entre quem tem CONFIANCA aceita
//!      -- mesma empresa ou empresas diferentes (o gate e o mesmo).
//!   3. Anti-spam vale TAMBEM no canal de pedido: solicitacao repetida do mesmo
//!      remetente e recusada, remetente bloqueado nao pede, ha teto de pendentes.
//!   4. A mensagem e cifrada com a chave do segredo ECDH entre os dois; cada um
//!      decifra com a PROPRIA senha + a chave publica do outro. O gravado e
//!      ciphertext.
//!   5. Prioridade (alta/media/baixa) e tipo (alerta/aviso/normal) viajam
//!      AUTENTICADOS (no AAD): rebaixar um alerta quebra a etiqueta.
//!
//! So `std` + a cripto ja conferida contra vetor em phxsql-core.
//! Rodar: cargo run -q --example correio-e2e -p phxsql-core

use phxsql_core::{cifra, hkdf, x25519, CHAVE_LEN, NONCE_LEN, TAG_LEN};

const ITER: u32 = 200_000; // PBKDF2 para senha->chave
const INFO: &[u8] = b"phxsql-correio-v1"; // rotulo do HKDF
const TETO_PENDENTES: usize = 50; // teto de solicitacoes pendentes por conta

#[derive(Clone, Copy, PartialEq, Debug)]
enum Prioridade {
    Alta,
    Media,
    Baixa,
}
impl Prioridade {
    fn rotulo(self) -> &'static str {
        match self {
            Prioridade::Alta => "alta",
            Prioridade::Media => "media",
            Prioridade::Baixa => "baixa",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tipo {
    Alerta, // 🚨
    Aviso,  // 🔔
    Normal,
}
impl Tipo {
    fn rotulo(self) -> &'static str {
        match self {
            Tipo::Alerta => "alerta",
            Tipo::Aviso => "aviso",
            Tipo::Normal => "normal",
        }
    }
    fn icone(self) -> &'static str {
        match self {
            Tipo::Alerta => "🚨",
            Tipo::Aviso => "🔔",
            Tipo::Normal => "  ",
        }
    }
}

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
    endereco: String,
    empresa: String,
    publica: [u8; CHAVE_LEN],
    sal: [u8; 16],
    nonce_priv: [u8; NONCE_LEN],
    priv_cifrada: Vec<u8>,
    tag_priv: [u8; TAG_LEN],
}

fn criar_usuario(endereco: &str, empresa: &str, senha: &str) -> Usuario {
    let privada = x25519::gerar_privada();
    let publica = x25519::chave_publica(&privada);
    let sal = rnd16();
    let ksenha = cifra::chave_de_senha(senha, &sal, ITER);
    let nonce_priv = rnd_nonce();
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

/// Abre a privada com a senha. Senha errada -> Err (etiqueta do AEAD).
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
    prioridade: Prioridade,
    tipo: Tipo,
    sal_kdf: [u8; 16],
    nonce: [u8; NONCE_LEN],
    ct: Vec<u8>,
    tag: [u8; TAG_LEN],
}

/// O que a etiqueta autentica alem do texto: quem, para quem, prioridade e tipo.
/// Assim ninguem rebaixa um alerta nem troca o remetente sem quebrar o AEAD.
fn aad(de: &str, para: &str, pri: Prioridade, tipo: Tipo) -> String {
    format!("{de}|{para}|{}|{}", pri.rotulo(), tipo.rotulo())
}

struct ServerMail {
    dominio_base: String,
    usuarios: Vec<Usuario>,
    confiancas: Vec<(String, String, bool)>, // (de, para, aceita)
    bloqueios: Vec<(String, String)>,        // (dono, bloqueado): dono nao recebe de bloqueado
    mensagens: Vec<Mensagem>,
}

impl ServerMail {
    fn novo(dominio_base: &str) -> Self {
        ServerMail {
            dominio_base: dominio_base.into(),
            usuarios: vec![],
            confiancas: vec![],
            bloqueios: vec![],
            mensagens: vec![],
        }
    }
    fn achar(&self, endereco: &str) -> Option<&Usuario> {
        self.usuarios.iter().find(|u| u.endereco == endereco)
    }
    fn criar_conta(&mut self, local: &str, empresa: &str, senha: &str) -> String {
        let endereco = format!("{local}@{empresa}");
        self.usuarios.push(criar_usuario(&endereco, empresa, senha));
        println!("   conta criada: {endereco}");
        endereco
    }
    fn confia(&self, a: &str, b: &str) -> bool {
        self.confiancas
            .iter()
            .any(|c| c.2 && ((c.0 == a && c.1 == b) || (c.0 == b && c.1 == a)))
    }
    fn bloqueado(&self, dono: &str, quem: &str) -> bool {
        self.bloqueios.iter().any(|x| x.0 == dono && x.1 == quem)
    }
    fn pendentes_para(&self, para: &str) -> usize {
        self.confiancas
            .iter()
            .filter(|c| c.1 == para && !c.2)
            .count()
    }
    fn mesma_empresa(&self, a: &str, b: &str) -> bool {
        matches!((self.achar(a), self.achar(b)), (Some(x), Some(y)) if x.empresa == y.empresa)
    }

    /// Pedir confianca. Aqui mora o anti-spam do CANAL DE PEDIDO: bloqueado nao
    /// pede, nao se repete pedido pendente, e ha teto de pendentes por conta.
    fn solicitar_confianca(&mut self, de: &str, para: &str) -> Result<(), String> {
        if self.bloqueado(para, de) {
            return Err(format!("RECUSADO: {para} bloqueou {de}"));
        }
        if self.confia(de, para) {
            return Err("ja ha confianca aceita".into());
        }
        if self
            .confiancas
            .iter()
            .any(|c| c.0 == de && c.1 == para && !c.2)
        {
            return Err(format!("RECUSADO: ja ha uma solicitacao pendente de {de}"));
        }
        if self.pendentes_para(para) >= TETO_PENDENTES {
            return Err(format!(
                "RECUSADO: {para} atingiu o teto de solicitacoes pendentes"
            ));
        }
        self.confiancas.push((de.into(), para.into(), false));
        let cruz = if self.mesma_empresa(de, para) {
            "mesma empresa"
        } else {
            "EMPRESAS DIFERENTES"
        };
        println!("   solicitacao de confianca: {de} -> {para}  [{cruz}]  (pendente)");
        Ok(())
    }
    fn aceitar_confianca(&mut self, de: &str, para: &str) {
        for c in &mut self.confiancas {
            if c.0 == de && c.1 == para {
                c.2 = true;
            }
        }
        println!("   {para} ACEITOU a confianca de {de}");
    }
    fn bloquear(&mut self, dono: &str, quem: &str) {
        self.bloqueios.push((dono.into(), quem.into()));
        self.confiancas
            .retain(|c| !(c.0 == quem && c.1 == dono && !c.2));
        println!("   {dono} BLOQUEOU {quem}");
    }

    fn enviar(
        &mut self,
        de: &str,
        senha_de: &str,
        para: &str,
        corpo: &str,
        pri: Prioridade,
        tipo: Tipo,
    ) -> Result<usize, String> {
        if !self.confia(de, para) {
            return Err(format!(
                "RECUSADO: nao ha relacao de confianca aceita entre {de} e {para}"
            ));
        }
        let ud = self.achar(de).ok_or("remetente inexistente")?.clone();
        let up = self.achar(para).ok_or("destinatario inexistente")?.clone();
        let priv_de = abrir_privada(&ud, senha_de)?;
        let segredo = x25519::segredo(&priv_de, &up.publica).map_err(|e| e.to_string())?;
        let sal_kdf = rnd16();
        let mut kmsg = [0u8; CHAVE_LEN];
        hkdf::derivar(&sal_kdf, &segredo, INFO, &mut kmsg).map_err(|e| e.to_string())?;
        let nonce = rnd_nonce();
        let a = aad(de, para, pri, tipo);
        let (ct, tag) = cifra::selar(&kmsg, &nonce, a.as_bytes(), corpo.as_bytes());
        self.mensagens.push(Mensagem {
            de: de.into(),
            para: para.into(),
            prioridade: pri,
            tipo,
            sal_kdf,
            nonce,
            ct,
            tag,
        });
        Ok(self.mensagens.len() - 1)
    }

    fn ler(
        &self,
        idx: usize,
        quem: &str,
        senha: &str,
    ) -> Result<(String, Prioridade, Tipo), String> {
        let m = self.mensagens.get(idx).ok_or("mensagem inexistente")?;
        let uq = self.achar(quem).ok_or("leitor inexistente")?;
        let outro = if m.de == quem { &m.para } else { &m.de };
        let uo = self.achar(outro).ok_or("contraparte inexistente")?;
        let priv_q = abrir_privada(uq, senha)?;
        let segredo = x25519::segredo(&priv_q, &uo.publica).map_err(|e| e.to_string())?;
        let mut kmsg = [0u8; CHAVE_LEN];
        hkdf::derivar(&m.sal_kdf, &segredo, INFO, &mut kmsg).map_err(|e| e.to_string())?;
        // o AAD e reconstruido dos campos GRAVADOS: se alguem adulterou a
        // prioridade em disco, o AAD muda e a etiqueta nao confere.
        let a = aad(&m.de, &m.para, m.prioridade, m.tipo);
        let claro = cifra::abrir(&kmsg, &m.nonce, a.as_bytes(), &m.ct, &m.tag).map_err(|_| {
            "nao decifra: chave/senha erradas ou dado/prioridade adulterados".to_string()
        })?;
        let texto = String::from_utf8(claro).map_err(|_| "utf8 invalido".to_string())?;
        Ok((texto, m.prioridade, m.tipo))
    }
}

fn main() {
    let mut res: Vec<(bool, String)> = vec![];
    let mut ok = |c: bool, n: &str| res.push((c, n.to_string()));

    println!("===== correio PhxSql: confianca, anti-spam e prioridade autenticada =====\n");

    println!("1) server mail + contas de empresa.phxsql.com.br:");
    let mut srv = ServerMail::novo("phxsql.com.br");
    let a = srv.criar_conta(
        "adrianoboller",
        "empresa.phxsql.com.br",
        "senha-do-adriano-9f",
    );
    let b = srv.criar_conta("joanaprado", "empresa.phxsql.com.br", "senha-da-joana-7k");
    let c = srv.criar_conta("marcoslima", "empresa.phxsql.com.br", "senha-do-marcos-3z");
    let d = srv.criar_conta("carlos", "outra.phxsql.com.br", "senha-do-carlos-1w");
    let spammer = srv.criar_conta("spammer", "golpe.phxsql.com.br", "x");
    ok(srv.usuarios.len() == 5, "server mail criou as contas");

    let corpo = "Bom dia, Joana. Segue o fechamento de setembro. -- Adriano";

    println!("\n2) enviar SEM confianca (adriano -> joana): tem de RECUSAR");
    let r = srv.enviar(
        &a,
        "senha-do-adriano-9f",
        &b,
        corpo,
        Prioridade::Media,
        Tipo::Normal,
    );
    match &r {
        Err(e) => {
            println!("   {e}");
            ok(
                e.contains("RECUSADO"),
                "sem confianca: contato recusado (anti-spam)",
            );
        }
        Ok(_) => ok(false, "sem confianca: contato recusado (anti-spam)"),
    }

    println!("\n3) confianca adriano <-> joana:");
    srv.solicitar_confianca(&a, &b).unwrap();
    srv.aceitar_confianca(&a, &b);

    println!("\n4) enviar COM confianca, prioridade MEDIA:");
    let idx = srv
        .enviar(
            &a,
            "senha-do-adriano-9f",
            &b,
            corpo,
            Prioridade::Media,
            Tipo::Normal,
        )
        .expect("devia enviar");
    println!(
        "   gravado #{idx}, CIFRADO ({} bytes).",
        srv.mensagens[idx].ct.len()
    );
    ok(true, "com confianca: envio sucede");

    // gravado e ciphertext?
    let claro_bytes = corpo.as_bytes();
    let n = claro_bytes.len().min(16);
    let vazou = srv.mensagens[idx]
        .ct
        .windows(n)
        .any(|w| w == &claro_bytes[..n]);
    ok(!vazou, "o corpo em claro NAO aparece no dado gravado");

    println!("\n5) joana le com a PROPRIA senha:");
    match srv.ler(idx, &b, "senha-da-joana-7k") {
        Ok((t, _, _)) => {
            println!("   \"{t}\"");
            ok(t == corpo, "destinataria decifra com a propria senha");
        }
        Err(e) => {
            println!("   FALHA: {e}");
            ok(false, "destinataria decifra com a propria senha");
        }
    }

    println!("\n6) NAO abre o e-mail sem a senha (senha errada):");
    match srv.ler(idx, &b, "errada") {
        Err(e) => {
            println!("   {e}");
            ok(true, "sem a senha certa NAO abre");
        }
        Ok(_) => ok(false, "sem a senha certa NAO abre"),
    }

    println!("\n7) terceiro (marcos) fora do par nao le:");
    ok(
        srv.ler(idx, &c, "senha-do-marcos-3z").is_err(),
        "terceiro fora do par nao decifra",
    );

    // --- prioridade autenticada ---
    println!("\n8) alerta 🚨 prioridade ALTA (adriano -> joana):");
    let ia = srv
        .enviar(
            &a,
            "senha-do-adriano-9f",
            &b,
            "Fechamento da folha vence hoje.",
            Prioridade::Alta,
            Tipo::Alerta,
        )
        .expect("devia enviar alerta");
    let (t, pri, tp) = srv
        .ler(ia, &b, "senha-da-joana-7k")
        .expect("devia ler alerta");
    println!("   {} [{}] \"{}\"", tp.icone(), pri.rotulo(), t);
    ok(
        pri == Prioridade::Alta && tp == Tipo::Alerta,
        "alerta chega com prioridade e tipo intactos",
    );

    println!("\n9) adulterar a prioridade gravada (rebaixar o alerta) quebra a leitura:");
    srv.mensagens[ia].prioridade = Prioridade::Baixa; // ataque: rebaixar
    match srv.ler(ia, &b, "senha-da-joana-7k") {
        Err(e) => {
            println!("   {e}");
            ok(
                true,
                "rebaixar a prioridade quebra a etiqueta (nao decifra)",
            );
        }
        Ok(_) => ok(
            false,
            "rebaixar a prioridade quebra a etiqueta (nao decifra)",
        ),
    }
    srv.mensagens[ia].prioridade = Prioridade::Alta; // desfaz

    // --- anti-spam no canal de PEDIDO ---
    println!("\n10) spammer pede confianca a joana:");
    println!(
        "   1o pedido: {:?}",
        srv.solicitar_confianca(&spammer, &b)
            .map(|_| "aceito para fila")
    );
    println!("\n11) spammer REPETE o pedido: tem de RECUSAR");
    match srv.solicitar_confianca(&spammer, &b) {
        Err(e) => {
            println!("   {e}");
            ok(
                e.contains("RECUSADO"),
                "pedido repetido recusado (anti-spam do canal)",
            );
        }
        Ok(_) => ok(false, "pedido repetido recusado (anti-spam do canal)"),
    }
    println!("\n12) joana BLOQUEIA o spammer; novo pedido tem de RECUSAR:");
    srv.bloquear(&b, &spammer);
    match srv.solicitar_confianca(&spammer, &b) {
        Err(e) => {
            println!("   {e}");
            ok(
                e.contains("RECUSADO"),
                "remetente bloqueado nao consegue pedir",
            );
        }
        Ok(_) => ok(false, "remetente bloqueado nao consegue pedir"),
    }

    // --- inter-empresa: mesmo gate ---
    println!("\n13) inter-empresa (adriano -> carlos): recusa sem confianca, entrega apos aceite");
    let sem = srv.enviar(
        &a,
        "senha-do-adriano-9f",
        &d,
        "Ola, Carlos.",
        Prioridade::Baixa,
        Tipo::Aviso,
    );
    let gate = sem.is_err();
    srv.solicitar_confianca(&a, &d).unwrap();
    srv.aceitar_confianca(&a, &d);
    let com = srv.enviar(
        &a,
        "senha-do-adriano-9f",
        &d,
        "Ola, Carlos. Inter-empresa.",
        Prioridade::Baixa,
        Tipo::Aviso,
    );
    let entregou = matches!(com, Ok(i) if srv.ler(i, &d, "senha-do-carlos-1w").map(|(t, _, _)| t).as_deref() == Ok("Ola, Carlos. Inter-empresa."));
    ok(gate && entregou, "inter-empresa: mesmo gate de confianca");

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
    let _ = srv.dominio_base;
    if falhas != 0 {
        std::process::exit(1);
    }
}

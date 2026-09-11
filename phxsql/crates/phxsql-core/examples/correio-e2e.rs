//! Prototipo rodavel do correio nativo do PhxSql. Modelo pedido pelo dono
//! (11/09/2026), provado ponta a ponta so com `std` + a cripto ja conferida
//! contra vetor em phxsql-core (X25519, HKDF, ChaCha20-Poly1305, PBKDF2).
//!
//! O QUE ESTE EXEMPLO PROVA (e o MODELO, nao o formato em disco final):
//!   1. Identidade X25519; a PRIVADA fica cifrada sob a senha -> nao abre sem a senha.
//!   2. Sem CONFIANCA aceita nao ha contato (anti-spam), inclusive no canal de
//!      PEDIDO (repetido recusado, bloqueado nao pede, teto de pendentes).
//!   3. Mensagem cifrada pelo segredo ECDH entre os dois: cada um decifra com a
//!      PROPRIA senha + a chave publica do outro. Prioridade/tipo AUTENTICADOS.
//!   4. ANEXOS ficam num armazem A PARTE; a mensagem guarda so a referencia
//!      (o registro nao carrega o binario -> nao fica lento).
//!   5. ALTO SEGREDO: uma 2a camada de cifra sobre texto e anexos, com uma frase
//!      passada por telefone/SMS -> precisa da senha da conta E da frase.
//!   6. Moderacao: banir (alto prejuizo), quarentena (baixo prejuizo) e multa
//!      (link de pagamento) sao solicitacoes com MOTIVO obrigatorio.
//!   7. Porta configuravel; padrao 8000.
//!
//! Rodar: cargo run -q --example correio-e2e -p phxsql-core

use phxsql_core::{cifra, hkdf, x25519, CHAVE_LEN, NONCE_LEN, TAG_LEN};

const ITER: u32 = 200_000;
const INFO: &[u8] = b"phxsql-correio-v1";
const TETO_PENDENTES: usize = 50;
const PORTA_PADRAO: u16 = 8000;

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
    Alerta,
    Aviso,
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

#[derive(Clone, Copy, PartialEq, Debug)]
enum TipoSolicitacao {
    Banir,      // uso indevido de ALTO prejuizo
    Quarentena, // uso indevido de BAIXO prejuizo
    Multa,      // link de pagamento de multa por mau uso
}
impl TipoSolicitacao {
    fn prejuizo(self) -> &'static str {
        match self {
            TipoSolicitacao::Banir => "alto prejuizo",
            TipoSolicitacao::Quarentena => "baixo prejuizo",
            TipoSolicitacao::Multa => "multa por mau uso",
        }
    }
    fn rotulo(self) -> &'static str {
        match self {
            TipoSolicitacao::Banir => "banir",
            TipoSolicitacao::Quarentena => "quarentena",
            TipoSolicitacao::Multa => "multa",
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

// -------- camada extra "alto segredo": chave da FRASE passada por telefone/SMS --------
fn selar_extra(frase: &str, sal: &[u8; 16], claro: &[u8]) -> Vec<u8> {
    let k = cifra::chave_de_senha(frase, sal, ITER);
    let nonce = rnd_nonce();
    let (ct, tag) = cifra::selar(&k, &nonce, b"alto-segredo", claro);
    let mut out = Vec::with_capacity(NONCE_LEN + TAG_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&tag);
    out.extend_from_slice(&ct);
    out
}
fn abrir_extra(frase: &str, sal: &[u8; 16], blob: &[u8]) -> Result<Vec<u8>, String> {
    if blob.len() < NONCE_LEN + TAG_LEN {
        return Err("blob de alto segredo malformado".into());
    }
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&blob[..NONCE_LEN]);
    let mut tag = [0u8; TAG_LEN];
    tag.copy_from_slice(&blob[NONCE_LEN..NONCE_LEN + TAG_LEN]);
    let ct = &blob[NONCE_LEN + TAG_LEN..];
    let k = cifra::chave_de_senha(frase, sal, ITER);
    cifra::abrir(&k, &nonce, b"alto-segredo", ct, &tag)
        .map_err(|_| "frase de alto segredo errada ou ausente".to_string())
}

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

/// Um bloco cifrado (E2E) guardado num arquivo A PARTE dos registros de mensagem.
struct Anexo {
    id: u64,
    nome: String,
    sal_kdf: [u8; 16],
    nonce: [u8; NONCE_LEN],
    ct: Vec<u8>,
    tag: [u8; TAG_LEN],
}

struct Mensagem {
    de: String,
    para: String,
    prioridade: Prioridade,
    tipo: Tipo,
    alto_segredo: bool,
    sal_extra: [u8; 16],
    sal_kdf: [u8; 16],
    nonce: [u8; NONCE_LEN],
    ct: Vec<u8>,
    tag: [u8; TAG_LEN],
    anexos: Vec<u64>, // SO referencias -- o binario mora no armazem a parte
}

fn aad(de: &str, para: &str, pri: Prioridade, tipo: Tipo) -> String {
    format!("{de}|{para}|{}|{}", pri.rotulo(), tipo.rotulo())
}

/// deriva uma chave de mensagem/anexo do segredo ECDH + um sal proprio
fn chave_derivada(segredo: &[u8; CHAVE_LEN], sal: &[u8; 16]) -> Result<[u8; CHAVE_LEN], String> {
    let mut k = [0u8; CHAVE_LEN];
    hkdf::derivar(sal, segredo, INFO, &mut k).map_err(|e| e.to_string())?;
    Ok(k)
}

struct Envio<'a> {
    de: &'a str,
    senha: &'a str,
    para: &'a str,
    corpo: &'a str,
    pri: Prioridade,
    tipo: Tipo,
    anexos: Vec<(&'a str, &'a [u8])>, // (nome, bytes)
    alto_segredo: Option<&'a str>,    // Some(frase passada por telefone/SMS)
}

struct Solicitacao {
    tipo: TipoSolicitacao,
    de: String,
    alvo: String,
    motivo: String,
    link_pagamento: Option<String>,
}

struct ServerMail {
    dominio_base: String,
    porta: u16,
    usuarios: Vec<Usuario>,
    confiancas: Vec<(String, String, bool)>,
    bloqueios: Vec<(String, String)>,
    mensagens: Vec<Mensagem>,
    anexos: Vec<Anexo>, // arquivo A PARTE
    solicitacoes: Vec<Solicitacao>,
    proximo_anexo: u64,
}

impl ServerMail {
    fn novo(dominio_base: &str) -> Self {
        ServerMail {
            dominio_base: dominio_base.into(),
            porta: PORTA_PADRAO,
            usuarios: vec![],
            confiancas: vec![],
            bloqueios: vec![],
            mensagens: vec![],
            anexos: vec![],
            solicitacoes: vec![],
            proximo_anexo: 1,
        }
    }
    fn configurar_porta(&mut self, p: u16) {
        self.porta = p;
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

    /// Solicitacao de moderacao. MOTIVO e obrigatorio (como o .reason do excluir);
    /// multa exige link de pagamento.
    fn solicitar_moderacao(
        &mut self,
        tipo: TipoSolicitacao,
        de: &str,
        alvo: &str,
        motivo: &str,
        link: Option<&str>,
    ) -> Result<usize, String> {
        if motivo.trim().is_empty() {
            return Err(format!(
                "RECUSADO: solicitacao de {} exige motivo",
                tipo.rotulo()
            ));
        }
        if tipo == TipoSolicitacao::Multa && link.is_none() {
            return Err("RECUSADO: multa exige link de pagamento".into());
        }
        self.solicitacoes.push(Solicitacao {
            tipo,
            de: de.into(),
            alvo: alvo.into(),
            motivo: motivo.into(),
            link_pagamento: link.map(|s| s.into()),
        });
        println!(
            "   solicitacao [{}] {} -> {alvo}  [{}]  motivo: \"{motivo}\"{}",
            tipo.rotulo(),
            de,
            tipo.prejuizo(),
            link.map(|l| format!("  link: {l}")).unwrap_or_default()
        );
        Ok(self.solicitacoes.len() - 1)
    }

    fn enviar(&mut self, e: Envio) -> Result<usize, String> {
        if !self.confia(e.de, e.para) {
            return Err(format!(
                "RECUSADO: nao ha relacao de confianca aceita entre {} e {}",
                e.de, e.para
            ));
        }
        let ud = self.achar(e.de).ok_or("remetente inexistente")?.clone();
        let up = self
            .achar(e.para)
            .ok_or("destinatario inexistente")?
            .clone();
        let priv_de = abrir_privada(&ud, e.senha)?;
        let segredo = x25519::segredo(&priv_de, &up.publica).map_err(|x| x.to_string())?;

        let sal_extra = rnd16();
        // texto: camada extra (se alto segredo) DENTRO da camada E2E
        let payload = match e.alto_segredo {
            Some(frase) => selar_extra(frase, &sal_extra, e.corpo.as_bytes()),
            None => e.corpo.as_bytes().to_vec(),
        };
        let sal_kdf = rnd16();
        let kmsg = chave_derivada(&segredo, &sal_kdf)?;
        let nonce = rnd_nonce();
        let a = aad(e.de, e.para, e.pri, e.tipo);
        let (ct, tag) = cifra::selar(&kmsg, &nonce, a.as_bytes(), &payload);

        // anexos: cada um cifrado e guardado no armazem A PARTE
        let mut refs = vec![];
        for (nome, bytes) in &e.anexos {
            let interno = match e.alto_segredo {
                Some(frase) => selar_extra(frase, &sal_extra, bytes),
                None => bytes.to_vec(),
            };
            let sal_a = rnd16();
            let ka = chave_derivada(&segredo, &sal_a)?;
            let nonce_a = rnd_nonce();
            let (cta, taga) = cifra::selar(&ka, &nonce_a, nome.as_bytes(), &interno);
            let id = self.proximo_anexo;
            self.proximo_anexo += 1;
            self.anexos.push(Anexo {
                id,
                nome: (*nome).into(),
                sal_kdf: sal_a,
                nonce: nonce_a,
                ct: cta,
                tag: taga,
            });
            refs.push(id);
        }

        self.mensagens.push(Mensagem {
            de: e.de.into(),
            para: e.para.into(),
            prioridade: e.pri,
            tipo: e.tipo,
            alto_segredo: e.alto_segredo.is_some(),
            sal_extra,
            sal_kdf,
            nonce,
            ct,
            tag,
            anexos: refs,
        });
        Ok(self.mensagens.len() - 1)
    }

    fn segredo_de(&self, quem: &str, senha: &str, outro: &str) -> Result<[u8; CHAVE_LEN], String> {
        let uq = self.achar(quem).ok_or("leitor inexistente")?;
        let uo = self.achar(outro).ok_or("contraparte inexistente")?;
        let priv_q = abrir_privada(uq, senha)?;
        x25519::segredo(&priv_q, &uo.publica).map_err(|e| e.to_string())
    }

    fn ler(
        &self,
        idx: usize,
        quem: &str,
        senha: &str,
        frase_extra: Option<&str>,
    ) -> Result<(String, Prioridade, Tipo), String> {
        let m = self.mensagens.get(idx).ok_or("mensagem inexistente")?;
        let outro = if m.de == quem { &m.para } else { &m.de };
        let segredo = self.segredo_de(quem, senha, outro)?;
        let kmsg = chave_derivada(&segredo, &m.sal_kdf)?;
        let a = aad(&m.de, &m.para, m.prioridade, m.tipo);
        let interno = cifra::abrir(&kmsg, &m.nonce, a.as_bytes(), &m.ct, &m.tag)
            .map_err(|_| "nao decifra: chave/senha erradas ou prioridade adulterada".to_string())?;
        let corpo = if m.alto_segredo {
            let frase = frase_extra
                .ok_or("mensagem de ALTO SEGREDO: precisa da frase passada por telefone/SMS")?;
            abrir_extra(frase, &m.sal_extra, &interno)?
        } else {
            interno
        };
        let texto = String::from_utf8(corpo).map_err(|_| "utf8 invalido".to_string())?;
        Ok((texto, m.prioridade, m.tipo))
    }

    fn ler_anexo(
        &self,
        msg_idx: usize,
        anexo_id: u64,
        quem: &str,
        senha: &str,
        frase_extra: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let m = self.mensagens.get(msg_idx).ok_or("mensagem inexistente")?;
        let outro = if m.de == quem { &m.para } else { &m.de };
        let segredo = self.segredo_de(quem, senha, outro)?;
        let anexo = self
            .anexos
            .iter()
            .find(|x| x.id == anexo_id)
            .ok_or("anexo inexistente")?;
        let ka = chave_derivada(&segredo, &anexo.sal_kdf)?;
        let interno = cifra::abrir(
            &ka,
            &anexo.nonce,
            anexo.nome.as_bytes(),
            &anexo.ct,
            &anexo.tag,
        )
        .map_err(|_| "anexo nao decifra".to_string())?;
        if m.alto_segredo {
            let frase = frase_extra.ok_or("anexo de ALTO SEGREDO: precisa da frase")?;
            abrir_extra(frase, &m.sal_extra, &interno)
        } else {
            Ok(interno)
        }
    }
}

fn main() {
    let mut res: Vec<(bool, String)> = vec![];
    let mut ok = |c: bool, n: &str| res.push((c, n.to_string()));

    println!(
        "===== correio PhxSql: E2E, confianca, anexos a parte, alto segredo, moderacao =====\n"
    );

    println!("1) server mail (porta padrao) + contas:");
    let mut srv = ServerMail::novo("phxsql.com.br");
    ok(srv.porta == 8000, "porta padrao e 8000");
    let a = srv.criar_conta(
        "adrianoboller",
        "empresa.phxsql.com.br",
        "senha-do-adriano-9f",
    );
    let b = srv.criar_conta("joanaprado", "empresa.phxsql.com.br", "senha-da-joana-7k");

    println!("\n2) confianca adriano <-> joana:");
    srv.solicitar_confianca(&a, &b).unwrap();
    srv.aceitar_confianca(&a, &b);

    println!("\n3) enviar com ANEXO (o binario vai para o armazem a parte):");
    let anexo_bytes = vec![0x50u8; 4096]; // 4 KB de "binario"
    let idx = srv
        .enviar(Envio {
            de: &a,
            senha: "senha-do-adriano-9f",
            para: &b,
            corpo: "Segue a planilha em anexo.",
            pri: Prioridade::Media,
            tipo: Tipo::Normal,
            anexos: vec![("fechamento.xlsx", &anexo_bytes)],
            alto_segredo: None,
        })
        .expect("devia enviar");
    let m = &srv.mensagens[idx];
    println!(
        "   mensagem #{idx}: {} bytes de texto cifrado, {} anexo(s) por REFERENCIA {:?}",
        m.ct.len(),
        m.anexos.len(),
        m.anexos
    );
    println!(
        "   armazem de anexos (a parte): {} bloco(s), o maior com {} bytes",
        srv.anexos.len(),
        srv.anexos.iter().map(|x| x.ct.len()).max().unwrap_or(0)
    );
    // o registro da mensagem NAO carrega os 4 KB do anexo:
    ok(
        m.ct.len() < 200 && m.anexos == vec![1],
        "registro da mensagem so tem referencia, nao o binario",
    );
    ok(
        srv.anexos.len() == 1 && srv.anexos[0].ct.len() >= 4096,
        "o binario mora no armazem a parte",
    );
    let ax = &srv.anexos[0];
    let vazou_ax = ax.ct.windows(8).any(|w| w == [0x50u8; 8]);
    ok(
        !vazou_ax,
        "o anexo esta cifrado no armazem (nao e o binario cru)",
    );

    println!("\n4) joana le o texto e baixa o anexo com a propria senha:");
    let (t, _, _) = srv
        .ler(idx, &b, "senha-da-joana-7k", None)
        .expect("le texto");
    let ab = srv
        .ler_anexo(idx, 1, &b, "senha-da-joana-7k", None)
        .expect("baixa anexo");
    println!("   texto: \"{t}\"  | anexo: {} bytes recuperados", ab.len());
    ok(t == "Segue a planilha em anexo.", "texto decifra certo");
    ok(ab == anexo_bytes, "anexo decifra byte a byte");

    println!("\n5) ALTO SEGREDO 🤐 (frase passada por telefone/SMS): texto + anexo:");
    let seg_bytes = vec![0x9au8; 2048];
    let frase = "girassol-42-telefonei";
    let ix = srv
        .enviar(Envio {
            de: &a,
            senha: "senha-do-adriano-9f",
            para: &b,
            corpo: "Contrato confidencial: valor final 1,2 mi.",
            pri: Prioridade::Alta,
            tipo: Tipo::Alerta,
            anexos: vec![("contrato.pdf", &seg_bytes)],
            alto_segredo: Some(frase),
        })
        .expect("envia alto segredo");

    println!("   joana com a senha da conta, MAS sem a frase:");
    match srv.ler(ix, &b, "senha-da-joana-7k", None) {
        Err(e) => {
            println!("   {e}");
            ok(
                true,
                "alto segredo: sem a frase nao le, mesmo sendo a destinataria",
            );
        }
        Ok(_) => ok(
            false,
            "alto segredo: sem a frase nao le, mesmo sendo a destinataria",
        ),
    }
    println!("   joana com a senha da conta E a frase (recebida por telefone):");
    match srv.ler(ix, &b, "senha-da-joana-7k", Some(frase)) {
        Ok((t2, _, _)) => {
            println!("   \"{t2}\"");
            let ab2 = srv.ler_anexo(ix, 2, &b, "senha-da-joana-7k", Some(frase));
            ok(
                t2.contains("Contrato") && ab2.as_deref() == Ok(seg_bytes.as_slice()),
                "alto segredo: com a frase, le texto e anexo",
            );
        }
        Err(e) => {
            println!("   FALHA: {e}");
            ok(false, "alto segredo: com a frase, le texto e anexo");
        }
    }
    println!("   frase ERRADA nao le:");
    ok(
        srv.ler(ix, &b, "senha-da-joana-7k", Some("frase-errada"))
            .is_err(),
        "alto segredo: frase errada nao le",
    );

    println!("\n6) solicitacoes de moderacao (motivo obrigatorio):");
    let mau = srv.criar_conta("golpista", "golpe.phxsql.com.br", "x");
    match srv.solicitar_moderacao(TipoSolicitacao::Banir, &b, &mau, "  ", None) {
        Err(e) => {
            println!("   (sem motivo) {e}");
            ok(e.contains("motivo"), "solicitacao sem motivo e recusada");
        }
        Ok(_) => ok(false, "solicitacao sem motivo e recusada"),
    }
    let s1 = srv.solicitar_moderacao(
        TipoSolicitacao::Banir,
        &b,
        &mau,
        "enviou 3 golpes de phishing com prejuizo financeiro",
        None,
    );
    let s2 = srv.solicitar_moderacao(
        TipoSolicitacao::Quarentena,
        &b,
        &mau,
        "flood de mensagens repetidas, sem prejuizo direto",
        None,
    );
    let s3 = srv.solicitar_moderacao(
        TipoSolicitacao::Multa,
        &b,
        &mau,
        "uso indevido reincidente apos aviso",
        Some("https://pagar.phxsql.com.br/multa/abc123"),
    );
    ok(
        s1.is_ok() && s2.is_ok() && s3.is_ok(),
        "banir/quarentena/multa registram com motivo",
    );
    let s4 = srv.solicitar_moderacao(TipoSolicitacao::Multa, &b, &mau, "reincidencia", None);
    ok(s4.is_err(), "multa sem link de pagamento e recusada");
    ok(
        srv.solicitacoes.iter().all(|s| !s.motivo.trim().is_empty()),
        "toda solicitacao gravada tem motivo",
    );

    println!("\n6b) anti-spam do canal de PEDIDO (repetido/bloqueado):");
    let spammer = srv.criar_conta("spammer", "golpe.phxsql.com.br", "z");
    srv.solicitar_confianca(&spammer, &b).unwrap();
    ok(
        srv.solicitar_confianca(&spammer, &b).is_err(),
        "pedido de confianca repetido e recusado",
    );
    srv.bloquear(&b, &spammer);
    ok(
        srv.solicitar_confianca(&spammer, &b).is_err(),
        "remetente bloqueado nao consegue nem pedir",
    );

    println!("\n6c) resumo das solicitacoes (le tipo/de/alvo/link):");
    for s in &srv.solicitacoes {
        println!(
            "   [{}] {} -> {} {}",
            s.tipo.rotulo(),
            s.de,
            s.alvo,
            s.link_pagamento
                .as_deref()
                .map(|l| format!("(link: {l})"))
                .unwrap_or_default()
        );
    }
    ok(
        srv.solicitacoes
            .iter()
            .any(|s| s.tipo == TipoSolicitacao::Multa && s.link_pagamento.is_some()),
        "a multa registrada carrega o link de pagamento",
    );

    println!("\n7) porta configuravel:");
    srv.configurar_porta(9443);
    println!("   porta agora: {}", srv.porta);
    ok(
        srv.porta == 9443,
        "porta pode ser configurada diferente de 8000",
    );

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
    let _ = (
        &srv.dominio_base,
        Tipo::Aviso.icone(),
        Prioridade::Baixa.rotulo(),
    );
    if falhas != 0 {
        std::process::exit(1);
    }
}

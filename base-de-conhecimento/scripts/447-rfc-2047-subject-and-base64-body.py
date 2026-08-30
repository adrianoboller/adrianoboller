# RFC 2047 subject and base64 body
# 28/08 15:03

p='crates/phxsql-server/src/email.rs'
s=open(p).read()
a='''//! # Injecao de cabecalho'''
b='''//! # Acento no cabecalho, e o corpo em base64
//!
//! Cabecalho de e-mail e ASCII por definicao (RFC 5322). Um assunto com `ç`
//! passou cru na primeira versao -- um rele moderno costuma aceitar, um rele
//! rigoroso embaralha ou recusa. Agora o assunto sai em palavra codificada da
//! RFC 2047 quando tem acento, e passa direto quando nao tem (assunto legivel
//! no arquivo de log do rele vale mais do que uniformidade).
//!
//! O corpo vai em base64, com `Content-Transfer-Encoding: base64`. UTF-8 cru
//! seria 8 bits declarado como 7, e um rele sem `8BITMIME` teria licenca para
//! cortar o oitavo bit -- o acento chegaria trocado. Base64 e feio de ler no
//! log e chega igual em qualquer rele.
//!
//! # Injecao de cabecalho'''
assert a in s; s=s.replace(a,b,1)

a='''    m.push_str(&format!("Subject: {}\\r\\n", uma_linha_so(assunto)?));'''
b='''    m.push_str(&format!(
        "Subject: {}\\r\\n",
        palavra_codificada(uma_linha_so(assunto)?)
    ));'''
assert a in s; s=s.replace(a,b,1)

a='''    m.push_str("MIME-Version: 1.0\\r\\n");
    m.push_str("Content-Type: text/plain; charset=utf-8\\r\\n");
    m.push_str("X-Mailer: PhxSql\\r\\n");
    m.push_str("\\r\\n");
    for linha in corpo.split('\\n') {
        let linha = linha.trim_end_matches('\\r');
        // "Dot stuffing": uma linha que comeca com ponto encerraria o DATA. O
        // ponto dobrado e como o RFC manda escapar, e o outro lado desfaz.
        if linha.starts_with('.') {
            m.push('.');
        }
        m.push_str(linha);
        m.push_str("\\r\\n");
    }
    Ok(m)
}'''
b='''    m.push_str("MIME-Version: 1.0\\r\\n");
    m.push_str("Content-Type: text/plain; charset=utf-8\\r\\n");
    m.push_str("Content-Transfer-Encoding: base64\\r\\n");
    m.push_str("X-Mailer: PhxSql\\r\\n");
    m.push_str("\\r\\n");
    // Base64 nao produz ponto no comeco de linha nem CR/LF, entao o
    // "dot stuffing" que o DATA exigiria nao tem o que escapar.
    for pedaco in quebrar(&base64::codificar(corpo.as_bytes()), 76) {
        m.push_str(&pedaco);
        m.push_str("\\r\\n");
    }
    Ok(m)
}

/// Um cabecalho com acento, na palavra codificada da RFC 2047.
///
/// So quando precisa: assunto em ASCII passa inteiro, e continua legivel no
/// log do rele e em qualquer cliente antigo.
fn palavra_codificada(texto: &str) -> String {
    if texto.is_ascii() {
        return texto.to_string();
    }
    format!("=?UTF-8?B?{}?=", base64::codificar(texto.as_bytes()))
}

/// Quebra em linhas de no maximo `n`. O RFC 2045 para em 76 colunas.
fn quebrar(texto: &str, n: usize) -> Vec<String> {
    if texto.is_empty() {
        return vec![String::new()];
    }
    texto
        .as_bytes()
        .chunks(n)
        .map(|c| String::from_utf8_lossy(c).to_string())
        .collect()
}'''
assert a in s; s=s.replace(a,b,1)

# testes
a='''    #[test]
    fn o_cabecalho_sai_completo() {
        let m = mensagem(&cfg(), "disco apertado", "linha 1\\nlinha 2").unwrap();
        assert!(m.contains("From: phx@exemplo.com\\r\\n"));
        assert!(m.contains("To: a@exemplo.com, b@exemplo.com\\r\\n"));
        assert!(m.contains("Subject: disco apertado\\r\\n"));
        assert!(m.contains("\\r\\n\\r\\nlinha 1\\r\\nlinha 2\\r\\n"));
    }'''
b='''    #[test]
    fn o_cabecalho_sai_completo() {
        let m = mensagem(&cfg(), "disco apertado", "linha 1\\nlinha 2").unwrap();
        assert!(m.contains("From: phx@exemplo.com\\r\\n"));
        assert!(m.contains("To: a@exemplo.com, b@exemplo.com\\r\\n"));
        // Assunto sem acento passa inteiro: legivel no log do rele.
        assert!(m.contains("Subject: disco apertado\\r\\n"));
        let corpo = m.split("\\r\\n\\r\\n").nth(1).unwrap();
        assert_eq!(
            phxsql_core::base64::decodificar_texto(corpo.trim()).unwrap(),
            "linha 1\\nlinha 2"
        );
    }

    /// Cabecalho e ASCII por definicao. Um assunto com acento passou cru na
    /// primeira versao, e um rele rigoroso o embaralharia.
    #[test]
    fn assunto_com_acento_vira_palavra_codificada() {
        let m = mensagem(&cfg(), "espaço em disco", "corpo").unwrap();
        let linha = m
            .lines()
            .find(|l| l.starts_with("Subject:"))
            .unwrap()
            .to_string();
        assert!(linha.is_ascii(), "cabecalho com byte alto: {linha:?}");
        assert!(linha.starts_with("Subject: =?UTF-8?B?"), "{linha}");
        let dentro = linha
            .trim_start_matches("Subject: =?UTF-8?B?")
            .trim_end_matches("?=");
        assert_eq!(
            phxsql_core::base64::decodificar_texto(dentro).unwrap(),
            "espaço em disco"
        );
    }

    /// UTF-8 cru seria 8 bits declarado como 7: um rele sem 8BITMIME teria
    /// licenca para cortar o oitavo bit, e o acento chegaria trocado.
    #[test]
    fn o_corpo_atravessa_rele_de_sete_bits() {
        let m = mensagem(&cfg(), "x", "acentuação e ç no corpo").unwrap();
        assert!(m.contains("Content-Transfer-Encoding: base64\\r\\n"));
        assert!(m.is_ascii(), "a mensagem inteira tem de ser ASCII");
        let corpo = m.split("\\r\\n\\r\\n").nth(1).unwrap().replace("\\r\\n", "");
        assert_eq!(
            phxsql_core::base64::decodificar_texto(&corpo).unwrap(),
            "acentuação e ç no corpo"
        );
    }

    /// O RFC 2045 para em 76 colunas.
    #[test]
    fn a_linha_de_base64_nao_passa_de_setenta_e_seis() {
        let m = mensagem(&cfg(), "x", &"a".repeat(5_000)).unwrap();
        for l in m.lines() {
            assert!(l.len() <= 78, "linha de {} bytes: {l:?}", l.len());
        }
    }'''
assert a in s; s=s.replace(a,b,1)

# o teste do ponto dobrado nao vale mais: base64 nao produz ponto inicial
a='''    #[test]
    fn linha_que_comeca_com_ponto_e_dobrada() {
        // Sem dobrar, esta linha encerraria o DATA e o resto do corpo viraria
        // comando SMTP.
        let m = mensagem(&cfg(), "x", ".\\nfim").unwrap();
        assert!(m.contains("\\r\\n..\\r\\nfim\\r\\n"), "{m}");
    }'''
b='''    /// O corpo em base64 nunca produz linha comecando com ponto, entao o
    /// "dot stuffing" do DATA nao tem o que escapar -- e a linha que
    /// encerraria a mensagem cedo nao existe.
    #[test]
    fn nenhuma_linha_do_corpo_comeca_com_ponto() {
        let m = mensagem(&cfg(), "x", ".\\n..\\n.fim").unwrap();
        let corpo = m.split("\\r\\n\\r\\n").nth(1).unwrap();
        for l in corpo.lines() {
            assert!(!l.starts_with('.'), "linha com ponto no inicio: {l:?}");
        }
        assert_eq!(
            phxsql_core::base64::decodificar_texto(&corpo.replace("\\r\\n", "")).unwrap(),
            ".\\n..\\n.fim"
        );
    }'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)
print('ok')

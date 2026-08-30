# Fix Base64 padding validation
# 27/08 19:21

p='crates/phxsql-core/src/base64.rs'
s=open(p).read()
s=s.replace('''    // Depois do primeiro '=' so pode vir '='.
    if limpo[sem_pad.len()..].iter().any(|c| *c != b'=') {
        return Err(PhxError::Tipo("base64 com dado depois do padding".into()));
    }
    if sem_pad.len() % 4 == 1 {
        return Err(PhxError::Tipo("base64 com comprimento invalido".into()));
    }''','''    // Depois do primeiro '=' so pode vir '='.
    if limpo[sem_pad.len()..].iter().any(|c| *c != b'=') {
        return Err(PhxError::Tipo("base64 com dado depois do padding".into()));
    }
    // Quantos bytes de dado sobram no ultimo grupo decide o padding possivel.
    let padding_esperado = match sem_pad.len() % 4 {
        0 => 0,
        2 => 2,
        3 => 1,
        _ => return Err(PhxError::Tipo("base64 com comprimento invalido".into())),
    };
    // Entrada sem padding e aceita; com padding, ele tem de estar certo.
    let padding = limpo.len() - sem_pad.len();
    if padding != 0 && (padding != padding_esperado || limpo.len() % 4 != 0) {
        return Err(PhxError::Tipo("base64 com padding invalido".into()));
    }''')
s=s.replace('''        for ruim in ["Z", "Zm9vYmFy!", "Zg==Zg==", "Zm 9v Ym Fy ="] {''','''        for ruim in [
            "Z",              // grupo de 1 byte nao existe
            "Zm9vYmFy!",      // caractere fora do alfabeto
            "Zg==Zg==",       // dado depois do padding
            "Zm 9v Ym Fy =",  // padding num grupo que ja esta completo
            "Zm9=",           // padding a mais para o que sobra
            "Zg=",            // padding de menos
        ] {''')
s=s.replace('''    #[test]
    fn ignora_espaco_e_quebra_de_linha() {''','''    #[test]
    fn aceita_sem_padding_mas_exige_padding_correto() {
        // Sem padding e aceito (aparece muito em API e em URL).
        assert_eq!(decodificar_texto("Zm9vYmE").unwrap(), "fooba");
        assert_eq!(decodificar_texto("Zg").unwrap(), "f");
        // Com padding, ele tem de estar certo.
        assert_eq!(decodificar_texto("Zm9vYmE=").unwrap(), "fooba");
        assert!(decodificar("Zm9vYmE==").is_err());
    }

    #[test]
    fn ignora_espaco_e_quebra_de_linha() {''')
open(p,'w').write(s)

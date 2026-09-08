//! Strings como o WLanguage as trata (Help 1512006 e as funcoes de string 3024xxx).
//! Posicoes sao 1-based e contam CARACTERES, nao bytes.

fn sem_acento(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'È' | 'É' | 'Ê' | 'Ë' => 'E',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
        'ç' => 'c',
        'Ç' => 'C',
        'ñ' => 'n',
        'Ñ' => 'N',
        'ý' | 'ÿ' => 'y',
        'Ý' => 'Y',
        _ => c,
    }
}

/// `=` entre strings (1512006): ESTRITO. "Dupond" = "DUPOND" e falso.
pub fn igual(a: &str, b: &str) -> bool {
    a == b
}

fn chave_flexivel(s: &str) -> String {
    s.trim()
        .chars()
        .map(sem_acento)
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// `~=` (1512006): ignora caixa, espacos das pontas e acento.
/// "Dupond" ~= "DUPOND", " Dupond" ~= "DUPOND", "C'est l'été" ~= "C'est l'ete".
pub fn igual_flexivel(a: &str, b: &str) -> bool {
    chave_flexivel(a) == chave_flexivel(b)
}

fn chave_muito_flexivel(s: &str) -> String {
    s.chars()
        .map(sem_acento)
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// `~~` (1512006): ignora todos os espacos, a pontuacao e a caixa. "Ent. S.A.R.L" ~~ "Ent SARL".
pub fn igual_muito_flexivel(a: &str, b: &str) -> bool {
    chave_muito_flexivel(a) == chave_muito_flexivel(b)
}

/// `[=` (1512006): comeca com, estrito.
pub fn comeca_com(a: &str, prefixo: &str) -> bool {
    a.starts_with(prefixo)
}

/// NoSpace (3024034): tira os espacos das duas pontas; com `dentro`, tambem os de dentro.
pub fn no_space(s: &str, dentro: bool) -> String {
    if dentro {
        s.chars().filter(|c| *c != ' ').collect()
    } else {
        s.trim_matches(' ').to_string()
    }
}

/// Length (3024031): numero de caracteres, espacos incluidos.
pub fn length(s: &str) -> usize {
    s.chars().count()
}

/// Middle(<s>, <inicio 1-based>[, <tamanho>]) (3024023): Middle("Eponine", 2, 3) = "pon";
/// fora da string devolve "".
pub fn middle(s: &str, inicio: usize, tamanho: Option<usize>) -> String {
    if inicio == 0 {
        return String::new();
    }
    let it = s.chars().skip(inicio - 1);
    match tamanho {
        Some(n) => it.take(n).collect(),
        None => it.collect(),
    }
}

/// Left(<s>[, <n>]) (3024001): os n primeiros caracteres; SEM n, a string sem os espacos
/// finais -- Left("AZELMA ") = "AZELMA".
pub fn left(s: &str, n: Option<usize>) -> String {
    match n {
        Some(n) => s.chars().take(n).collect(),
        None => s.trim_end_matches(' ').to_string(),
    }
}

/// Right(<s>, <n>): os n ultimos caracteres. A forma sem n nao esta no vetor lido do
/// Help; por isso aqui n e obrigatorio -- limite dito, nao adivinhado.
pub fn right(s: &str, n: usize) -> String {
    let total = length(s);
    s.chars().skip(total.saturating_sub(n)).collect()
}

/// RepeatString (3024036): RepeatString("Boby", 2) = "BobyBoby".
pub fn repeat_string(s: &str, n: usize) -> String {
    s.repeat(n)
}

/// Replace (3024022): todas as ocorrencias.
pub fn replace(s: &str, de: &str, para: &str) -> String {
    if de.is_empty() {
        s.to_string()
    } else {
        s.replace(de, para)
    }
}

/// Upper (3024039): maiusculas do conjunto latino, e o acento CAI: Upper("élan") = "ELAN".
pub fn upper(s: &str) -> String {
    s.chars()
        .map(sem_acento)
        .flat_map(|c| c.to_uppercase())
        .collect()
}

/// Truncate(<s>, <n>[, apagar]) (1000020476): guarda n caracteres, ou apaga n do fim.
pub fn truncate(s: &str, n: usize, apagar: bool) -> String {
    let total = length(s);
    let guardar = if apagar { total.saturating_sub(n) } else { n };
    s.chars().take(guardar).collect()
}

/// Val (3024037): o numero no comeco da string; "3plus2" = 3, "7,5" = 7, "ABC" = 0,
/// "1D2" = 100 (D e expoente), "2.5e-2" = 0.025.
pub fn val(s: &str) -> f64 {
    let t = s.trim_start();
    let b = t.as_bytes();
    let mut i = 0;
    let mut num = String::new();
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        num.push(b[i] as char);
        i += 1;
    }
    let mut digitos = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        num.push(b[i] as char);
        i += 1;
        digitos += 1;
    }
    if i < b.len() && b[i] == b'.' {
        num.push('.');
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            num.push(b[i] as char);
            i += 1;
            digitos += 1;
        }
    }
    if digitos == 0 {
        return 0.0;
    }
    if i < b.len() && matches!(b[i], b'e' | b'E' | b'd' | b'D') {
        let mut exp = String::from("e");
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'-' || b[j] == b'+') {
            exp.push(b[j] as char);
            j += 1;
        }
        let ini = j;
        while j < b.len() && b[j].is_ascii_digit() {
            exp.push(b[j] as char);
            j += 1;
        }
        if j > ini {
            num.push_str(&exp);
        }
    }
    num.parse().unwrap_or(0.0)
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn comparacao_do_help_1512006() {
        assert!(!igual("Dupond", "DUPOND"));
        assert!(igual_flexivel("Dupond", "DUPOND"));
        assert!(igual_flexivel(" Dupond", "DUPOND"));
        assert!(igual_flexivel(" Dupond", "Dupond"));
        assert!(igual_flexivel("C'est l'été", "C'est l'ete"));
        assert!(igual_muito_flexivel("Ent. S.A.R.L", "Ent SARL"));
        assert!(
            !igual_flexivel("Ent. S.A.R.L", "Ent SARL"),
            "~= nao ignora pontuacao; so ~~"
        );
        assert!(comeca_com("Blumenau", "Blu") && !comeca_com("Blumenau", "blu"));
    }
    #[test]
    fn funcoes_de_string_com_os_exemplos_do_help() {
        assert_eq!(no_space(" 3.75 Euros ", false), "3.75 Euros");
        assert_eq!(no_space("Abra ca da bra", true), "Abracadabra");
        assert_eq!(middle("Eponine", 2, Some(3)), "pon");
        assert_eq!(middle("Eponine", 2, None), "ponine");
        assert_eq!(middle("Eponine", 50, None), "");
        assert_eq!(middle("Eponine", 2, Some(50)), "ponine");
        assert_eq!(middle("Antananarivo - Madagascar", 10, Some(7)), "ivo - M");
        assert_eq!(left("The cuckoo", Some(6)), "The cu");
        assert_eq!(left("ABC", Some(50)), "ABC");
        assert_eq!(left("AZELMA ", None), "AZELMA");
        assert_eq!(right("Blumenau", 3), "nau");
        assert_eq!(repeat_string("x", 4), "xxxx");
        assert_eq!(repeat_string("Boby", 2), "BobyBoby");
        assert_eq!(repeat_string("", 2), "");
        // O Help 3024022 imprime "Abrococobro!" para este exemplo -- e um erro de digitacao
        // da pagina: trocar so o 'a' nao transforma o 'd' em 'c'. O resultado correto e este.
        assert_eq!(replace("Abracadabra!", "a", "o"), "Abrocodobro!");
        assert_eq!(upper("abcd"), "ABCD");
        assert_eq!(upper("élan"), "ELAN");
        assert_eq!(upper("this!"), "THIS!");
        assert_eq!(truncate(" Turlututu", 9, false), " Turlutut");
        assert_eq!(truncate(" Turlututu", 4, true), " Turlu");
        assert_eq!(length("Maria da Silva"), 14);
        assert_eq!(length("São"), 3, "caracteres, nao bytes");
    }
    #[test]
    fn val_do_help_3024037() {
        assert_eq!(val("143"), 143.0);
        assert_eq!(val("1.67"), 1.67);
        assert_eq!(val("ABC"), 0.0);
        assert_eq!(val("3plus2"), 3.0);
        assert_eq!(val("7,5"), 7.0);
        assert_eq!(val("1D2"), 100.0);
        assert_eq!(val("2.5e-2"), 0.025);
    }
    #[test]
    fn validacpf_do_estoque_com_as_pecas_daqui() {
        // ValidaCPF (estoque-codigo.pdf p.2) usa NoSpace, Replace, Length, RepeatString, Val, sCPF[[i]]
        let s = no_space(
            &replace(&replace("529.982.247-25", ".", ""), "-", ""),
            false,
        );
        assert_eq!(length(&s), 11);
        assert_ne!(s, repeat_string(&middle(&s, 1, Some(1)), 11));
        assert_eq!(val(&middle(&s, 10, Some(1))), 2.0);
    }
}

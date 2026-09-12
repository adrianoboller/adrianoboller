//! Cadastro do correio: CPF/CNPJ validos e nenhum campo vazio.
//!
//! Regra do dono (12/09/2026):
//!   - Empresa nao cadastra sem CPF E CNPJ validos, e sem NENHUM campo vazio.
//!   - Nenhum usuario sem CPF valido.
//!
//! "Valido" e o digito verificador conferido (mod-11), nao so preenchido --
//! escrito a mao (zero-deps, so a std) e provado contra vetor, como o resto das
//! conferencias desta casa (CRC, SHA, HMAC). A recusa acontece na DECLARACAO do
//! cadastro (cedo), do mesmo modo que "chave nasce conferida".
//!
//! Rodar:  cargo run --example correio-documentos -p phxsql-core

use phxsql_core::cifra;
use phxsql_core::hash::hmac_sha256;

fn so_digitos(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

/// 111.111.111-11 e afins passam no mod-11 mas nao valem: recusa antes.
fn todos_iguais(d: &str) -> bool {
    !d.is_empty() && d.as_bytes().windows(2).all(|w| w[0] == w[1])
}

/// Digito verificador mod-11 padrao (CPF e CNPJ): resto < 2 -> 0, senao 11-resto.
fn digito_mod11(digitos: &[u8], pesos: &[u8]) -> u8 {
    let soma: u32 = digitos
        .iter()
        .zip(pesos)
        .map(|(d, p)| u32::from(*d) * u32::from(*p))
        .sum();
    let resto = (soma % 11) as u8;
    if resto < 2 {
        0
    } else {
        11 - resto
    }
}

fn cpf_valido(s: &str) -> bool {
    let d = so_digitos(s);
    if d.len() != 11 || todos_iguais(&d) {
        return false;
    }
    let n: Vec<u8> = d.bytes().map(|b| b - b'0').collect();
    let d1 = digito_mod11(&n[0..9], &[10, 9, 8, 7, 6, 5, 4, 3, 2]);
    let d2 = digito_mod11(&n[0..10], &[11, 10, 9, 8, 7, 6, 5, 4, 3, 2]);
    n[9] == d1 && n[10] == d2
}

fn cnpj_valido(s: &str) -> bool {
    let d = so_digitos(s);
    if d.len() != 14 || todos_iguais(&d) {
        return false;
    }
    let n: Vec<u8> = d.bytes().map(|b| b - b'0').collect();
    let d1 = digito_mod11(&n[0..12], &[5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2]);
    let d2 = digito_mod11(&n[0..13], &[6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2]);
    n[12] == d1 && n[13] == d2
}

/// Nenhum campo vazio: espaco em branco tambem conta como vazio.
fn validar_campos(campos: &[(&str, &str)]) -> Result<(), String> {
    for (nome, valor) in campos {
        if valor.trim().is_empty() {
            return Err(format!("campo vazio: {nome}"));
        }
    }
    Ok(())
}

/// Cadastro de empresa: todos os campos presentes, CNPJ e CPF do responsavel validos.
fn validar_empresa(
    campos: &[(&str, &str)],
    cnpj: &str,
    cpf_responsavel: &str,
) -> Result<(), String> {
    validar_campos(campos)?;
    if !cnpj_valido(cnpj) {
        return Err(format!("CNPJ invalido: {cnpj}"));
    }
    if !cpf_valido(cpf_responsavel) {
        return Err(format!("CPF do responsavel invalido: {cpf_responsavel}"));
    }
    Ok(())
}

/// Cadastro de usuario: todos os campos presentes e CPF valido.
fn validar_usuario(campos: &[(&str, &str)], cpf: &str) -> Result<(), String> {
    validar_campos(campos)?;
    if !cpf_valido(cpf) {
        return Err(format!("CPF invalido: {cpf}"));
    }
    Ok(())
}

// ---- chave unica + cifra em repouso (itens 1, 4 e 5 do dono, 12/09) ----
// Em producao estas chaves vem do servidor (secret), nunca do codigo; aqui sao
// fixas so para a prova.
const CHAVE_INDICE: &[u8] = b"indice-cego-do-servermail-teste";
const CHAVE_CIFRA: [u8; 32] = *b"cifra-em-repouso-do-servermail!!";

/// Indice CEGO do documento: HMAC-SHA256 dos digitos. Determinista (mesmo doc ->
/// mesmo indice, e assim acha a duplicata) e de MAO UNICA (nao volta ao
/// documento). E ele que vira a CHAVE UNICA de cpf/cnpj, sem guardar o numero.
fn indice_cego(digitos: &str) -> [u8; 32] {
    hmac_sha256(CHAVE_INDICE, digitos.as_bytes())
}

/// O documento guardado: so o indice cego e o texto CIFRADO. O numero em claro
/// nao mora aqui.
struct DocGuardado {
    indice: [u8; 32],
    nonce: [u8; 12],
    ct: Vec<u8>,
    tag: [u8; 16],
}

fn guardar_doc(digitos: &str) -> DocGuardado {
    let mut nonce = [0u8; 12];
    cifra::sortear(&mut nonce);
    let (ct, tag) = cifra::selar(&CHAVE_CIFRA, &nonce, b"doc", digitos.as_bytes());
    DocGuardado {
        indice: indice_cego(digitos),
        nonce,
        ct,
        tag,
    }
}

fn ler_doc(g: &DocGuardado) -> String {
    let claro = cifra::abrir(&CHAVE_CIFRA, &g.nonce, b"doc", &g.ct, &g.tag)
        .expect("decifra o doc guardado");
    String::from_utf8(claro).expect("doc e ascii")
}

/// Cadastro em memoria que impoe as chaves unicas: nome de empresa, cnpj e cpf.
#[derive(Default)]
struct Cadastro {
    nomes: Vec<String>,
    idx_cnpj: Vec<[u8; 32]>,
    idx_cpf: Vec<[u8; 32]>,
}

impl Cadastro {
    fn empresa(&mut self, nome: &str, cnpj: &str, cpf_resp: &str) -> Result<DocGuardado, String> {
        validar_empresa(
            &[
                ("nome", nome),
                ("cnpj", cnpj),
                ("cpf_responsavel", cpf_resp),
            ],
            cnpj,
            cpf_resp,
        )?;
        if self.nomes.iter().any(|n| n.as_str() == nome) {
            return Err(format!("nome de empresa duplicado: {nome}"));
        }
        let g = guardar_doc(&so_digitos(cnpj));
        if self.idx_cnpj.contains(&g.indice) {
            return Err("CNPJ ja cadastrado (chave unica)".to_string());
        }
        self.nomes.push(nome.to_string());
        self.idx_cnpj.push(g.indice);
        Ok(g)
    }

    fn usuario(&mut self, endereco: &str, cpf: &str) -> Result<(), String> {
        validar_usuario(&[("endereco", endereco), ("cpf", cpf)], cpf)?;
        let idx = indice_cego(&so_digitos(cpf));
        if self.idx_cpf.contains(&idx) {
            return Err("CPF ja cadastrado (chave unica)".to_string());
        }
        self.idx_cpf.push(idx);
        Ok(())
    }
}

fn main() {
    let mut ok = 0u32;
    let mut falhas = 0u32;
    let mut cheque = |cond: bool, nome: &str| {
        if cond {
            ok += 1;
            println!("    ok    {nome}");
        } else {
            falhas += 1;
            println!("   FALHA  {nome}");
        }
    };

    println!("===== CPF/CNPJ do correio: cadastro so com documento valido =====");

    // --- CPF: vetores validos (digito conferido a mao) ---
    cheque(
        cpf_valido("529.982.247-25"),
        "CPF valido 529.982.247-25 (com pontuacao)",
    );
    cheque(
        cpf_valido("52998224725"),
        "CPF valido 52998224725 (so digitos)",
    );
    cheque(cpf_valido("111.444.777-35"), "CPF valido 111.444.777-35");

    // --- CPF: invalidos (a prova pega o erro) ---
    cheque(
        !cpf_valido("529.982.247-24"),
        "CPF com digito trocado RECUSA",
    );
    cheque(
        !cpf_valido("111.111.111-11"),
        "CPF todos iguais RECUSA (passa no mod-11)",
    );
    cheque(!cpf_valido("529.982.247-2"), "CPF curto RECUSA");
    cheque(!cpf_valido(""), "CPF vazio RECUSA");
    cheque(!cpf_valido("abc.def.ghi-jk"), "CPF sem digito RECUSA");

    // --- CNPJ: vetor valido ---
    cheque(
        cnpj_valido("11.222.333/0001-81"),
        "CNPJ valido 11.222.333/0001-81 (pontuado)",
    );
    cheque(
        cnpj_valido("11222333000181"),
        "CNPJ valido 11222333000181 (so digitos)",
    );

    // --- CNPJ: invalidos ---
    cheque(
        !cnpj_valido("11.222.333/0001-80"),
        "CNPJ com digito trocado RECUSA",
    );
    cheque(
        !cnpj_valido("00.000.000/0000-00"),
        "CNPJ todos iguais RECUSA",
    );
    cheque(!cnpj_valido("11222333000"), "CNPJ curto RECUSA");
    cheque(!cnpj_valido(""), "CNPJ vazio RECUSA");

    // --- Cadastro de empresa: gate completo ---
    let empresa_ok = [
        ("nome", "Prado & Filhos Ltda"),
        ("cnpj", "11.222.333/0001-81"),
        ("cpf_responsavel", "529.982.247-25"),
        ("cidade", "Blumenau"),
        ("uf", "SC"),
    ];
    cheque(
        validar_empresa(&empresa_ok, "11.222.333/0001-81", "529.982.247-25").is_ok(),
        "empresa com tudo preenchido e docs validos ACEITA",
    );

    let empresa_campo_vazio = [
        ("nome", "Prado & Filhos Ltda"),
        ("cnpj", "11.222.333/0001-81"),
        ("cpf_responsavel", "529.982.247-25"),
        ("cidade", ""), // vazio
        ("uf", "SC"),
    ];
    cheque(
        validar_empresa(&empresa_campo_vazio, "11.222.333/0001-81", "529.982.247-25")
            == Err("campo vazio: cidade".to_string()),
        "empresa com UM campo vazio RECUSA (e diz qual)",
    );

    let empresa_cnpj_ruim = [
        ("nome", "Fantasma ME"),
        ("cnpj", "11.222.333/0001-80"),
        ("cpf_responsavel", "529.982.247-25"),
        ("cidade", "Joinville"),
        ("uf", "SC"),
    ];
    cheque(
        validar_empresa(&empresa_cnpj_ruim, "11.222.333/0001-80", "529.982.247-25").is_err(),
        "empresa com CNPJ invalido RECUSA",
    );

    cheque(
        validar_empresa(&empresa_ok, "11.222.333/0001-81", "529.982.247-24").is_err(),
        "empresa com CPF do responsavel invalido RECUSA",
    );

    // --- Cadastro de usuario: gate ---
    let usuario_ok = [
        ("endereco", "adrianoboller@empresa.phxmail.com.br"),
        ("cpf", "111.444.777-35"),
    ];
    cheque(
        validar_usuario(&usuario_ok, "111.444.777-35").is_ok(),
        "usuario com CPF valido e campos cheios ACEITA",
    );
    cheque(
        validar_usuario(&usuario_ok, "").is_err(),
        "usuario sem CPF RECUSA",
    );
    cheque(
        validar_usuario(&usuario_ok, "111.444.777-30").is_err(),
        "usuario com CPF invalido RECUSA",
    );
    let usuario_sem_endereco = [("endereco", "   "), ("cpf", "111.444.777-35")];
    cheque(
        validar_usuario(&usuario_sem_endereco, "111.444.777-35").is_err(),
        "usuario com endereco em branco RECUSA",
    );

    // --- chave unica (cpf/cnpj/nome) + cifra em repouso ---
    println!("\n----- chave unica + cifra em repouso (itens 1/4/5) -----");
    let g = guardar_doc(&so_digitos("11.222.333/0001-81"));
    cheque(
        ler_doc(&g) == "11222333000181",
        "CNPJ guardado DECIFRA de volta (round-trip)",
    );
    cheque(
        g.ct.as_slice() != "11222333000181".as_bytes(),
        "CNPJ NAO fica em claro no disco (so ciphertext)",
    );
    cheque(
        indice_cego("11222333000181") == indice_cego("11222333000181"),
        "indice cego e determinista (mesmo doc -> acha a duplicata)",
    );
    cheque(
        indice_cego("11222333000181") != indice_cego("11444777000161"),
        "indice cego difere por documento",
    );

    let mut cad = Cadastro::default();
    cheque(
        cad.empresa(
            "Prado & Filhos Ltda",
            "11.222.333/0001-81",
            "529.982.247-25",
        )
        .is_ok(),
        "1a empresa cadastra",
    );
    cheque(
        cad.empresa(
            "Prado & Filhos Ltda",
            "11.444.777/0001-61",
            "529.982.247-25",
        )
        .is_err(),
        "empresa com NOME duplicado RECUSA",
    );
    cheque(
        cad.empresa("Outra Ltda", "11.222.333/0001-81", "529.982.247-25")
            .is_err(),
        "empresa com CNPJ repetido RECUSA (chave unica cega)",
    );
    cheque(
        cad.empresa("Outra Ltda", "11.444.777/0001-61", "529.982.247-25")
            .is_ok(),
        "empresa nova (nome e CNPJ novos) cadastra",
    );
    cheque(
        cad.usuario("a@empresa.phxmail.com.br", "111.444.777-35")
            .is_ok(),
        "1o usuario cadastra",
    );
    cheque(
        cad.usuario("b@empresa.phxmail.com.br", "111.444.777-35")
            .is_err(),
        "usuario com CPF repetido RECUSA (chave unica)",
    );
    cheque(
        cad.usuario("b@empresa.phxmail.com.br", "529.982.247-25")
            .is_ok(),
        "usuario novo (CPF novo) cadastra",
    );

    println!("\n===== RESULTADO =====");
    println!("{} checagens, {ok} ok, {falhas} falha(s).", ok + falhas);
    if falhas == 0 {
        println!("PROVA VERDE");
    } else {
        println!("PROVA VERMELHA");
        std::process::exit(1);
    }
}

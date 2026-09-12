//! Cadastro do correio: CPF/CNPJ validos, unicos, ocultos e imutaveis.
//!
//! Regras do dono (12/09/2026):
//!   - Empresa nao cadastra sem CPF E CNPJ validos, e sem NENHUM campo vazio.
//!   - Nenhum usuario sem CPF valido.
//!   - CPF e CNPJ sao CHAVE UNICA; nome de empresa nao duplica.
//!   - Campo de 25 caracteres. Uma vez cadastrado: NAO se altera e NAO se ve em
//!     tela nenhuma (write-only, oculto pos-cadastro) -- por isso "nao fica
//!     listado na base legivel". Tem "digite novamente" (dupla digitacao) para
//!     nao existir cadastro errado, ja que ninguem mais ve o numero.
//!   - NAO precisa criptografar: a protecao e a ocultacao + a imutabilidade.
//!
//! "Valido" e o digito verificador conferido (mod-11), nao so preenchido --
//! escrito a mao (zero-deps, so a std), como o CRC/SHA desta casa. A recusa e na
//! DECLARACAO do cadastro (cedo), do mesmo modo que "chave nasce conferida".
//!
//! Rodar:  cargo run --example correio-documentos -p phxsql-core

/// Largura do campo de documento (CPF/CNPJ), com folga para pontuacao.
const LARGURA_DOC: usize = 25;

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

fn validar_usuario(campos: &[(&str, &str)], cpf: &str) -> Result<(), String> {
    validar_campos(campos)?;
    if !cpf_valido(cpf) {
        return Err(format!("CPF invalido: {cpf}"));
    }
    Ok(())
}

fn confere_largura(doc: &str) -> Result<(), String> {
    if doc.chars().count() > LARGURA_DOC {
        return Err(format!("documento passa de {LARGURA_DOC} caracteres"));
    }
    Ok(())
}

/// "Digite novamente": o documento e a confirmacao tem de bater (pelos digitos).
/// Como o numero fica oculto pos-cadastro, so aqui da para conferir o que se
/// digitou.
fn confere_digitado(doc: &str, confirmacao: &str) -> Result<(), String> {
    if so_digitos(doc) != so_digitos(confirmacao) {
        return Err("digite novamente nao confere: cadastro recusado".to_string());
    }
    Ok(())
}

/// O que uma tela recebe do documento: SEMPRE oculto. Nunca o numero -- e por
/// isso que "nao fica listado na base legivel".
fn doc_na_tela() -> &'static str {
    "••••••••• (oculto)"
}

/// Cadastro em memoria que impoe: nome de empresa unico, cpf/cnpj unicos e
/// IMUTAVEIS. Os documentos so vivem como digitos para a chave unica; nao ha
/// getter que devolva o numero a uma tela.
#[derive(Default)]
struct Cadastro {
    nomes: Vec<String>,
    cnpjs: Vec<String>,
    cpfs: Vec<String>,
}

impl Cadastro {
    fn empresa(
        &mut self,
        nome: &str,
        cnpj: &str,
        cnpj_confirma: &str,
        cpf_resp: &str,
    ) -> Result<(), String> {
        validar_empresa(
            &[
                ("nome", nome),
                ("cnpj", cnpj),
                ("cpf_responsavel", cpf_resp),
            ],
            cnpj,
            cpf_resp,
        )?;
        confere_largura(cnpj)?;
        confere_digitado(cnpj, cnpj_confirma)?;
        if self.nomes.iter().any(|n| n.as_str() == nome) {
            return Err(format!("nome de empresa duplicado: {nome}"));
        }
        let d = so_digitos(cnpj);
        if self.cnpjs.contains(&d) {
            return Err("CNPJ ja cadastrado (chave unica)".to_string());
        }
        self.nomes.push(nome.to_string());
        self.cnpjs.push(d);
        Ok(())
    }

    fn usuario(&mut self, endereco: &str, cpf: &str, cpf_confirma: &str) -> Result<(), String> {
        validar_usuario(&[("endereco", endereco), ("cpf", cpf)], cpf)?;
        confere_largura(cpf)?;
        confere_digitado(cpf, cpf_confirma)?;
        let d = so_digitos(cpf);
        if self.cpfs.contains(&d) {
            return Err("CPF ja cadastrado (chave unica)".to_string());
        }
        self.cpfs.push(d);
        Ok(())
    }

    /// Imutavel: nao ha caminho para alterar o documento depois do cadastro.
    fn alterar_documento(&mut self, _velho: &str, _novo: &str) -> Result<(), String> {
        Err("documento nao se altera apos o cadastro (imutavel)".to_string())
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

    // --- Cadastro de empresa: campos e docs ---
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
        ("cidade", ""),
        ("uf", "SC"),
    ];
    cheque(
        validar_empresa(&empresa_campo_vazio, "11.222.333/0001-81", "529.982.247-25")
            == Err("campo vazio: cidade".to_string()),
        "empresa com UM campo vazio RECUSA (e diz qual)",
    );

    // --- Usuario: gate ---
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

    // --- chave unica + oculto + imutavel + digite novamente (itens novos) ---
    println!("\n----- chave unica · 25 car · oculto · imutavel · digite novamente -----");
    let mut cad = Cadastro::default();
    cheque(
        cad.empresa(
            "Prado & Filhos Ltda",
            "11.222.333/0001-81",
            "11.222.333/0001-81",
            "529.982.247-25",
        )
        .is_ok(),
        "1a empresa cadastra (CNPJ digitado 2x, confere)",
    );
    cheque(
        cad.empresa(
            "Outra ME",
            "11.222.333/0001-81",
            "11.222.333/0001-82",
            "529.982.247-25",
        ) == Err("digite novamente nao confere: cadastro recusado".to_string()),
        "CNPJ com 2a digitacao DIFERENTE recusa (digite novamente)",
    );
    cheque(
        cad.empresa(
            "Prado & Filhos Ltda",
            "11.444.777/0001-61",
            "11.444.777/0001-61",
            "529.982.247-25",
        )
        .is_err(),
        "empresa com NOME duplicado RECUSA",
    );
    cheque(
        cad.empresa(
            "Outra Ltda",
            "11.222.333/0001-81",
            "11.222.333/0001-81",
            "529.982.247-25",
        )
        .is_err(),
        "empresa com CNPJ repetido RECUSA (chave unica)",
    );
    cheque(
        cad.empresa(
            "Outra Ltda",
            "11.444.777/0001-61",
            "11.444.777/0001-61",
            "529.982.247-25",
        )
        .is_ok(),
        "empresa nova (nome e CNPJ novos) cadastra",
    );
    cheque(
        cad.usuario(
            "a@empresa.phxmail.com.br",
            "111.444.777-35",
            "111.444.777-35",
        )
        .is_ok(),
        "1o usuario cadastra (CPF digitado 2x)",
    );
    cheque(
        cad.usuario(
            "b@empresa.phxmail.com.br",
            "111.444.777-35",
            "111.444.777-30",
        )
        .is_err(),
        "CPF com 2a digitacao diferente RECUSA",
    );
    cheque(
        cad.usuario(
            "b@empresa.phxmail.com.br",
            "111.444.777-35",
            "111.444.777-35",
        )
        .is_err(),
        "usuario com CPF repetido RECUSA (chave unica)",
    );
    cheque(
        cad.alterar_documento("111.444.777-35", "529.982.247-25")
            .is_err(),
        "documento NAO se altera apos cadastro (imutavel)",
    );
    cheque(
        !doc_na_tela().chars().any(|c| c.is_ascii_digit()),
        "na tela o documento aparece OCULTO (nenhum digito)",
    );
    cheque(
        confere_largura(&"9".repeat(26)).is_err(),
        "documento acima de 25 caracteres RECUSA",
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

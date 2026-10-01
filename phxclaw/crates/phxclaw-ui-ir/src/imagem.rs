//! Print de tela -> SQL, a parte deterministica. Quem le a imagem (OCR e modelo de visao)
//! fica na borda, no agente; aqui chegam so textos, e daqui sai SQL que o `schema` ja sabe
//! analisar.
//!
//! Duas regras seguram a borda:
//! - o que o modelo disser so vale se estiver ESCRITO na tela (linha do OCR): modelo nao
//!   inventa campo, porque o que ele inventa nao passa por aqui;
//! - nome de coluna, tipo e obrigatorio saem de regra fixa, nao do modelo. O `*` do
//!   obrigatorio vem do OCR, que le o que esta pintado.

/// Rotulo confirmado na tela.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rotulo {
    pub texto: String,
    pub obrigatorio: bool,
    /// O modelo o apontou como lista de selecao.
    pub lista: bool,
}

/// Comparacao tolerante a OCR: sem acento, sem caixa, so letras e digitos.
pub fn normalizar(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            let c = match c {
                'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' => 'a',
                'é' | 'ê' | 'è' | 'É' | 'Ê' => 'e',
                'í' | 'Í' => 'i',
                'ó' | 'ô' | 'õ' | 'Ó' | 'Ô' | 'Õ' => 'o',
                'ú' | 'ü' | 'Ú' => 'u',
                'ç' | 'Ç' => 'c',
                c => c,
            };
            c.is_ascii_alphanumeric().then(|| c.to_ascii_lowercase())
        })
        .collect()
}

/// Fica so o que o modelo disse E esta numa linha do OCR; o obrigatorio vem da linha lida.
/// As listas de selecao entram tambem como candidatas: o modelo as acha melhor numa
/// pergunta propria (medido: a "Situação" so apareceu assim).
pub fn confirmar(campos: &[String], listas: &[String], ocr: &[String]) -> Vec<Rotulo> {
    let lidas: Vec<(String, &String)> = ocr.iter().map(|l| (normalizar(l), l)).collect();
    let nl: Vec<String> = listas.iter().map(|l| normalizar(l)).collect();
    let mut v: Vec<Rotulo> = vec![];
    for c in campos.iter().chain(listas) {
        let n = normalizar(c);
        if n.is_empty() || texto_de_exemplo(&n) || v.iter().any(|r| normalizar(&r.texto) == n) {
            continue;
        }
        if let Some((_, linha)) = lidas.iter().find(|(l, _)| *l == n) {
            v.push(Rotulo {
                texto: linha.replace('*', "").trim().to_string(),
                obrigatorio: linha.contains('*') || c.contains('*'),
                lista: nl.contains(&n),
            });
        }
    }
    v
}

/// Texto de exemplo dentro do campo ("Selecione produto", "dd/mm/aaaa") esta escrito na
/// tela e passa pelo OCR, mas nao e rotulo (medido: virava a coluna selecione_produto).
pub(crate) fn texto_de_exemplo(normalizado: &str) -> bool {
    // "R$", "%", "4": simbolo ou numero solto nao e rotulo (medido: "R$" virava coluna "r")
    normalizado
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .count()
        < 2
        || [
            "selecione",
            "digite",
            "informe",
            "escolha",
            // "ddmm" e nao "ddmmaaaa": medido em 01/10, o OCR leu "dd/mmyaaaa"
            "ddmm",
            "pesquisar",
        ]
        .iter()
        .any(|p| normalizado.starts_with(p))
}

/// Tira dos campos do formulario o que e coluna da grade de itens: coluna da grade
/// pertence a tabela filha (medido: o modelo as listava nas duas perguntas).
pub fn sem_itens(campos: &[Rotulo], itens: &[Rotulo]) -> Vec<Rotulo> {
    campos
        .iter()
        .filter(|c| {
            !itens
                .iter()
                .any(|i| normalizar(&i.texto) == normalizar(&c.texto))
        })
        .cloned()
        .collect()
}

/// "Razão social" -> "razao_social"; "Código" vira a chave.
pub fn coluna(rotulo: &str) -> String {
    let mut s = String::new();
    for p in rotulo.split_whitespace() {
        let n = normalizar(p);
        if n.is_empty()
            || matches!(n.as_str(), "de" | "da" | "do" | "das" | "dos" | "e") && !s.is_empty()
        {
            continue;
        }
        if !s.is_empty() {
            s.push('_');
        }
        s.push_str(&n);
    }
    if s.is_empty() { "campo".into() } else { s }
}

fn tem(c: &str, palavras: &[&str]) -> bool {
    c.split('_').any(|p| palavras.contains(&p))
}

/// Tipo SQL pela palavra do rotulo -- a mesma familia de regra do `schema` na volta.
pub fn tipo(rotulo: &Rotulo) -> String {
    let c = coluna(&rotulo.texto);
    let t = if c == "criado_em" || c == "alterado_em" || (tem(&c, &["data"]) && tem(&c, &["hora"]))
    {
        "timestamp"
    } else if tem(
        &c,
        &[
            "data",
            "dt",
            "nascimento",
            "emissao",
            "vencimento",
            "validade",
        ],
    ) {
        "date"
    } else if tem(
        &c,
        &[
            "valor", "preco", "total", "frete", "desconto", "salario", "custo", "subtotal",
        ],
    ) {
        "numeric(12,2)"
    } else if tem(&c, &["quantidade", "qtd", "qtde", "peso"]) {
        "numeric(12,3)"
    } else if tem(&c, &["observacao", "observacoes", "obs", "historico"]) {
        "text"
    } else if tem(&c, &["ativo", "inativo", "bloqueado"]) {
        "boolean"
    } else if tem(&c, &["cpf"]) {
        "varchar(14)"
    } else if tem(&c, &["cnpj"]) {
        "varchar(18)"
    } else if tem(&c, &["telefone", "celular", "fone"]) {
        "varchar(20)"
    } else if tem(&c, &["cep"]) {
        "varchar(9)"
    } else if rotulo.lista {
        "varchar(40)"
    } else {
        "varchar(120)"
    };
    t.into()
}

fn colunas_sql(rotulos: &[Rotulo]) -> Vec<String> {
    let mut v = vec![];
    for r in rotulos {
        let c = coluna(&r.texto);
        if c == "codigo" || c == "id" {
            continue; // a chave ja entra como id
        }
        let mut l = format!("  {c} {}", tipo(r));
        if c == "criado_em" || c == "alterado_em" {
            l.push_str(" DEFAULT now()");
        } else if r.obrigatorio {
            l.push_str(" NOT NULL");
        }
        v.push(l);
    }
    v
}

/// SQL da tela: a tabela dos campos e, se havia grade de itens, a filha com a chave para
/// a mae (o `schema` monta dai o mestre/detalhe e o total).
pub fn sql(tabela: &str, campos: &[Rotulo], itens: &[Rotulo]) -> String {
    let t = coluna(tabela);
    let mut cols = vec!["  id serial PRIMARY KEY".to_string()];
    cols.extend(colunas_sql(campos));
    let mut s = format!("CREATE TABLE {t} (\n{}\n);\n", cols.join(",\n"));
    if !itens.is_empty() {
        let mut ci = vec![
            "  id serial PRIMARY KEY".to_string(),
            format!("  {t}_id integer NOT NULL REFERENCES {t}(id)"),
        ];
        ci.extend(colunas_sql(itens));
        s.push_str(&format!(
            "CREATE TABLE {t}_item (\n{}\n);\n",
            ci.join(",\n")
        ));
    }
    s
}

/// Extrai a lista de textos da resposta do modelo: array JSON (com ou sem cerca de
/// codigo), ou objeto com uma lista dentro, ou objeto de pares (fica o VALOR, que e o
/// texto da tela; a chave costuma ser a traducao que o modelo inventou).
pub fn lista_da_resposta(r: &str) -> Vec<String> {
    let ini = r.find(['[', '{']);
    let Some(ini) = ini else { return vec![] };
    let fim = r.rfind([']', '}']).map(|f| f + 1).unwrap_or(r.len());
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&r[ini..fim.max(ini)]) else {
        // JSON que nao fechou (medido: o modelo degenerou repetindo "R$" ate o teto de
        // tokens): ficam as cadeias entre aspas, e a confirmacao pelo OCR filtra o resto
        return r[ini..]
            .split('"')
            .skip(1)
            .step_by(2)
            .map(str::to_owned)
            .collect();
    };
    fn textos(v: &serde_json::Value) -> Vec<String> {
        match v {
            serde_json::Value::Array(a) => a
                .iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect(),
            serde_json::Value::Object(o) => {
                if let Some(a) = o.values().find(|x| x.is_array()) {
                    textos(a)
                } else {
                    o.values()
                        .filter_map(|x| x.as_str().map(str::to_owned))
                        .collect()
                }
            }
            _ => vec![],
        }
    }
    textos(&v)
}

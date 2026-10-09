//! O MOTOR da fabrica de idiomas: a lista de idiomas, a resolucao por chave e
//! a queda para o portugues.
//!
//! # Por que mora no core
//!
//! Ate a 0.20 o motor morava no `phxsql-server`, junto das tabelas de texto
//! dele (`idiomas.rs`, a tela; `mensagens.rs`, o protocolo). O PhxZipCmd e o
//! PhxZipWeb (pedido 454, fatia Z2) precisam da MESMA resolucao sem depender
//! do servidor -- e copiar os degraus para la seria a decisao escrita duas
//! vezes, que e o que a petrea «funcao e comando vem do mesmo motor» proibe.
//! Entao sobe so o que e decisao: a ordem dos idiomas, os degraus da queda, o
//! preenchimento dos `{marcadores}` e o laco que confere chave pedida contra
//! chave existente. As TABELAS de texto ficam com quem as mostra.
//!
//! # Os degraus
//!
//! 1. a celula do idioma pedido;
//! 2. vazia (ou so espaco)? a celula do portugues;
//! 3. sem linha gravada? o texto de FABRICA, pelos mesmos dois degraus.
//!
//! Idioma desconhecido e o indice 0, o portugues: idioma escrito errado mostra
//! portugues, e nunca uma tela -- ou um terminal -- sem texto.

use std::collections::{BTreeSet, HashSet};

/// As seis colunas de idioma, na ordem das colunas de `phxsys.mensagens`.
///
/// Os nomes sao os nomes das colunas e o valor aceito no `"idioma"` do
/// `config.json`. `Portugues` e o indice 0 de proposito: e o degrau 2.
pub const IDIOMAS: [&str; 6] = [
    "Portugues",
    "Frances",
    "Ingles",
    "Italiano",
    "Alemao",
    "Espanhol",
];

/// Quantos idiomas: escrito uma vez, para o resto derivar.
pub const QUANTOS: usize = IDIOMAS.len();

/// O codigo ISO 639-1 de cada idioma, na ordem de [`IDIOMAS`]. E o que o
/// `LANG` do sistema traz (`de_DE.UTF-8`), e o que um humano digita.
pub const CODIGOS: [&str; QUANTOS] = ["pt", "fr", "en", "it", "de", "es"];

/// Um texto de fabrica: o nome estavel (a CHAVE) e o texto em cada idioma.
///
/// `textos[0]` e o portugues e nunca e vazio -- [`defeitos_da_tabela`]
/// reprova. Celula vazia cai no portugues: melhor nenhuma traducao do que uma
/// inventada.
pub struct TextoDeFabrica {
    pub nome: &'static str,
    pub textos: [&'static str; QUANTOS],
}

/// Uma linha de fabrica em uma linha de fonte, na ordem das colunas:
/// Portugues, Frances, Ingles, Italiano, Alemao, Espanhol.
///
/// Existe para que acrescentar um texto custe UMA LINHA. Com duzentos textos a
/// forma longa cobraria mil e duzentas linhas de cerimonia, e o preco de
/// obedecer a regra do idioma viraria o argumento para nao obedece-la.
#[macro_export]
macro_rules! texto {
    ($nome:literal, $pt:literal, $fr:literal, $en:literal, $it:literal, $de:literal, $es:literal) => {
        $crate::idiomas::TextoDeFabrica {
            nome: $nome,
            textos: [$pt, $fr, $en, $it, $de, $es],
        }
    };
}

/// A posicao de um idioma pelo nome da coluna. Desconhecido = portugues.
pub fn indice_do_idioma(nome: &str) -> usize {
    let nome = nome.trim();
    IDIOMAS.iter().position(|i| *i == nome).unwrap_or(0)
}

/// O idioma de um valor de ambiente, se ele for reconhecido: o nome da coluna
/// (`Ingles`, sem diferenca de caixa) ou o codigo ISO, com ou sem regiao e
/// codificacao (`en`, `en_US.UTF-8`, `pt-BR`).
///
/// `None` e «nao reconheci» -- quem chama decide se isso cai no portugues ou
/// na proxima variavel. `C` e `POSIX` nao sao idioma, sao a falta de um.
pub fn idioma_do_valor(valor: &str) -> Option<usize> {
    let v = valor.trim();
    if v.is_empty() {
        return None;
    }
    if let Some(i) = IDIOMAS.iter().position(|n| n.eq_ignore_ascii_case(v)) {
        return Some(i);
    }
    let codigo = v.split(['_', '-', '.', '@']).next().unwrap_or("");
    CODIGOS.iter().position(|c| c.eq_ignore_ascii_case(codigo))
}

/// O idioma pelas variaveis de ambiente, na ordem de preferencia de quem
/// chama: a PRIMEIRA definida e nao vazia decide, e a que ela traz e
/// desconhecida cai no portugues -- e nao na proxima. Quem escreveu
/// `PHXZIP_IDIOMA=klingon` pediu algo, e o `LANG` da maquina nao e resposta
/// ao pedido dele; o portugues e a resposta de sempre a idioma desconhecido.
///
/// Recebe os VALORES, e nao le o ambiente: assim o teste nao depende de quem
/// roda a suite.
pub fn idioma_do_ambiente(valores: &[Option<&str>]) -> usize {
    for v in valores.iter().flatten() {
        if !v.trim().is_empty() {
            return idioma_do_valor(v).unwrap_or(0);
        }
    }
    0
}

/// Degraus 1 e 2 sobre uma linha: a celula do idioma, senao a do portugues.
/// `None` quando as duas estao vazias -- quem chama desce para a fabrica.
///
/// Espaco conta como vazio: uma celula com um espaco na grade viraria um
/// botao sem rotulo, que e exatamente o defeito que o degrau 2 existe para
/// impedir.
pub fn da_linha<S: AsRef<str>>(linha: &[S; QUANTOS], idioma: usize) -> Option<&str> {
    let pedido = linha.get(idioma).map(AsRef::as_ref).unwrap_or("");
    if !pedido.trim().is_empty() {
        return Some(pedido);
    }
    let portugues = linha[0].as_ref();
    if !portugues.trim().is_empty() {
        return Some(portugues);
    }
    None
}

/// Os tres degraus para UM texto: a linha gravada (quando ha), depois a
/// fabrica. Nunca devolve vazio: na falta de tudo, a propria chave -- que e
/// defeito de programacao, e sumir com o texto seria pior que mostra-la.
pub fn resolver(
    gravadas: Option<&[String; QUANTOS]>,
    fab: &TextoDeFabrica,
    idioma: usize,
) -> String {
    gravadas
        .and_then(|l| da_linha(l, idioma))
        .or_else(|| da_linha(&fab.textos, idioma))
        .unwrap_or(fab.nome)
        .to_string()
}

/// O texto de fabrica de uma chave.
pub fn achar<'a>(tabela: &'a [TextoDeFabrica], nome: &str) -> Option<&'a TextoDeFabrica> {
    tabela.iter().find(|f| f.nome == nome)
}

/// Poe os parametros nos `{marcadores}`, numa passada so.
///
/// Uma passada, e nao um `replace` por parametro: o valor entra como DADO e
/// nunca e relido. Com `replace` em serie, um nome de arquivo chamado
/// `{destino}` seria trocado pelo parametro seguinte -- o dado reescrevendo a
/// frase. Marcador sem parametro fica como esta, escrito.
pub fn preencher(moldura: &str, parametros: &[(&str, &str)]) -> String {
    let mut saida = String::with_capacity(moldura.len());
    let mut resto = moldura;
    while let Some(i) = resto.find('{') {
        saida.push_str(&resto[..i]);
        let depois = &resto[i + 1..];
        let valor = depois.find('}').and_then(|f| {
            let chave = &depois[..f];
            parametros
                .iter()
                .find(|(c, _)| *c == chave)
                .map(|(_, v)| (*v, f))
        });
        match valor {
            Some((v, f)) => {
                saida.push_str(v);
                resto = &depois[f + 1..];
            }
            None => {
                saida.push('{');
                resto = depois;
            }
        }
    }
    saida.push_str(resto);
    saida
}

/// Quem fala por uma tabela de fabrica num idioma, sem tabela gravada: o
/// caso do programa que nao tem banco (o PhxZipCmd, o PhxZipWeb).
#[derive(Clone, Copy)]
pub struct Fala {
    tabela: &'static [TextoDeFabrica],
    idioma: usize,
}

impl Fala {
    /// Indice fora da lista e idioma desconhecido: cai no portugues.
    pub fn nova(tabela: &'static [TextoDeFabrica], idioma: usize) -> Fala {
        Fala {
            tabela,
            idioma: if idioma < QUANTOS { idioma } else { 0 },
        }
    }

    pub fn idioma(&self) -> usize {
        self.idioma
    }

    /// O texto de uma chave, com os parametros no lugar. Chave que nao existe
    /// volta como esta -- e o laco de [`conferir_laco`] e quem impede isso de
    /// chegar a um usuario.
    pub fn texto(&self, nome: &str, parametros: &[(&str, &str)]) -> String {
        let moldura = match achar(self.tabela, nome) {
            Some(f) => resolver(None, f, self.idioma),
            None => nome.to_string(),
        };
        preencher(&moldura, parametros)
    }
}

// =====================================================================
// O laco: chave pedida contra chave existente
// =====================================================================

/// As chaves que um fonte pede: todo literal `"<prefixo>nome"` com o nome em
/// `[a-z0-9_]`.
///
/// A busca e pelo PREFIXO entre aspas, e nao pelas formas de chamada
/// (`t("...")`, `data-txt=`): formas mudam quando alguem refatora, e um laco
/// que so conhece as de hoje passaria a aprovar tudo calado no dia da mudanca.
/// Por isso a tabela mora em arquivo SEPARADO do fonte varrido -- senao toda
/// chave da tabela se contaria como pedida por ela mesma.
pub fn chaves_no_fonte(fonte: &str, prefixo: &str) -> HashSet<String> {
    let mut usadas = HashSet::new();
    for pedaco in fonte.split(&format!("\"{prefixo}")).skip(1) {
        if let Some(resto) = pedaco.split('"').next() {
            if !resto.is_empty()
                && resto
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                usadas.insert(format!("{prefixo}{resto}"));
            }
        }
    }
    usadas
}

/// O resultado do laco, nos dois sentidos.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Laco {
    /// Pedida pelo fonte e ausente da tabela: o texto sai como a chave crua.
    pub faltando: BTreeSet<String>,
    /// Na tabela e ninguem pede. Chave morta e pior que chave faltando: o
    /// tradutor a ve, traduz nos seis idiomas, e nada muda na tela.
    pub mortas: BTreeSet<&'static str>,
}

impl Laco {
    pub fn fechado(&self) -> bool {
        self.faltando.is_empty() && self.mortas.is_empty()
    }
}

/// Confere o laco entre o que o fonte pede e o que a tabela tem.
pub fn conferir_laco(tabela: &'static [TextoDeFabrica], pedidas: &HashSet<String>) -> Laco {
    Laco {
        faltando: pedidas
            .iter()
            .filter(|p| achar(tabela, p).is_none())
            .cloned()
            .collect(),
        mortas: tabela
            .iter()
            .map(|f| f.nome)
            .filter(|n| !pedidas.contains(*n))
            .collect(),
    }
}

/// Os `{nome}` de um texto, em ordem alfabetica e sem repetir.
pub fn marcadores(texto: &str) -> Vec<String> {
    let mut achados: Vec<String> = Vec::new();
    let mut resto = texto;
    while let Some(i) = resto.find('{') {
        resto = &resto[i + 1..];
        let Some(f) = resto.find('}') else { break };
        let nome = &resto[..f];
        if !nome.is_empty()
            && nome.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !achados.iter().any(|a| a == nome)
        {
            achados.push(nome.to_string());
        }
        resto = &resto[f + 1..];
    }
    achados.sort();
    achados
}

/// O que uma tabela de fabrica tem de errado: prefixo, chave repetida,
/// portugues vazio, e traducao com marcador a mais ou a menos que o portugues
/// (traducao que perde `{arquivo}` diz «ja existe» sem dizer o que).
pub fn defeitos_da_tabela(tabela: &[TextoDeFabrica], prefixo: &str) -> Vec<String> {
    let mut defeitos = Vec::new();
    let mut vistos = HashSet::new();
    for f in tabela {
        if !f.nome.starts_with(prefixo) {
            defeitos.push(format!("{} nao comeca com {prefixo}", f.nome));
        }
        if !vistos.insert(f.nome) {
            defeitos.push(format!("{} aparece duas vezes", f.nome));
        }
        if f.textos[0].trim().is_empty() {
            defeitos.push(format!(
                "{} sem portugues -- o degrau 2 depende dele",
                f.nome
            ));
        }
        let base = marcadores(f.textos[0]);
        for (i, t) in f.textos.iter().enumerate().skip(1) {
            if !t.trim().is_empty() && marcadores(t) != base {
                defeitos.push(format!(
                    "{} em {}: os marcadores nao batem com os do portugues",
                    f.nome, IDIOMAS[i]
                ));
            }
        }
    }
    defeitos
}

/// Quantas celulas vazias cada idioma tem -- o numero que diz quanto da
/// tabela ainda fala portugues por queda, e nao por traducao.
pub fn vazias_por_idioma(tabela: &[TextoDeFabrica]) -> [usize; QUANTOS] {
    let mut n = [0; QUANTOS];
    for f in tabela {
        for (i, t) in f.textos.iter().enumerate() {
            if t.trim().is_empty() {
                n[i] += 1;
            }
        }
    }
    n
}

#[cfg(test)]
mod testes {
    use super::*;

    const TABELA: &[TextoDeFabrica] = &[
        texto!(
            "t.ola",
            "ola {nome}",
            "salut {nome}",
            "hi {nome}",
            "",
            "hallo {nome}",
            "  "
        ),
        texto!("t.sim", "sim", "oui", "yes", "si", "ja", "si"),
    ];

    /// **RED da Z2:** celula vazia cai no portugues -- inclusive a de so
    /// espaco. Prova real: devolver `linha[idioma]` sem conferir o vazio em
    /// `da_linha` faz o italiano e o espanhol sairem em branco, e este cai.
    #[test]
    fn celula_vazia_cai_no_portugues() {
        let f = Fala::nova(TABELA, indice_do_idioma("Italiano"));
        assert_eq!(f.texto("t.ola", &[("nome", "Ana")]), "ola Ana");
        let f = Fala::nova(TABELA, indice_do_idioma("Espanhol"));
        assert_eq!(f.texto("t.ola", &[("nome", "Ana")]), "ola Ana");
        let f = Fala::nova(TABELA, indice_do_idioma("Alemao"));
        assert_eq!(f.texto("t.ola", &[("nome", "Ana")]), "hallo Ana");
        // A linha gravada desce para o portugues GRAVADO antes da fabrica.
        let mut l: [String; QUANTOS] = Default::default();
        l[0] = "ola gravado".into();
        assert_eq!(resolver(Some(&l), &TABELA[0], 3), "ola gravado");
        // Linha inteira em branco: a fabrica, no idioma pedido.
        let vazia: [String; QUANTOS] = Default::default();
        assert_eq!(resolver(Some(&vazia), &TABELA[0], 2), "hi {nome}");
    }

    /// **RED da Z2:** idioma desconhecido cai no portugues, pelos tres
    /// caminhos -- nome de coluna, valor de ambiente e indice fora da lista.
    #[test]
    fn idioma_desconhecido_cai_no_portugues() {
        assert_eq!(indice_do_idioma("Klingon"), 0);
        assert_eq!(indice_do_idioma(""), 0);
        assert_eq!(indice_do_idioma(" Alemao "), 4);
        assert_eq!(Fala::nova(TABELA, 99).texto("t.sim", &[]), "sim");
        assert_eq!(
            idioma_do_ambiente(&[Some("klingon"), Some("en_US.UTF-8")]),
            0
        );
        assert_eq!(idioma_do_ambiente(&[None, Some("C")]), 0);
        assert_eq!(idioma_do_ambiente(&[None, None]), 0);
        // O irmao: o reconhecido decide, e o vazio passa a vez.
        assert_eq!(idioma_do_ambiente(&[Some(""), Some("de_DE.UTF-8")]), 4);
        assert_eq!(idioma_do_ambiente(&[Some("ingles"), Some("de")]), 2);
        assert_eq!(idioma_do_ambiente(&[Some("es-AR")]), 5);
        assert_eq!(idioma_do_valor("pt_BR.UTF-8"), Some(0));
        assert_eq!(idioma_do_valor("POSIX"), None);
    }

    /// O valor entra como dado: um nome `{b}` nao e trocado pelo parametro b.
    /// Prova real: com `replace` em serie, sai `x=B y=B`.
    #[test]
    fn o_parametro_nao_e_relido() {
        assert_eq!(
            preencher("x={a} y={b}", &[("a", "{b}"), ("b", "B")]),
            "x={b} y=B"
        );
        assert_eq!(preencher("{falta} e {", &[]), "{falta} e {");
        assert_eq!(preencher("{a}{a}", &[("a", "1")]), "11");
    }

    /// **RED da Z2:** o laco acusa nos dois sentidos -- a chave pedida que nao
    /// existe e a chave morta.
    #[test]
    fn o_laco_acusa_faltando_e_morta() {
        let fonte = r#"f.texto("t.ola", &[]); f.texto("t.nao_existe", &[]);"#;
        let laco = conferir_laco(TABELA, &chaves_no_fonte(fonte, "t."));
        assert_eq!(laco.faltando.iter().collect::<Vec<_>>(), ["t.nao_existe"]);
        assert_eq!(laco.mortas.iter().collect::<Vec<_>>(), [&"t.sim"]);
        assert!(!laco.fechado());
        // O irmao: o laco certo fecha -- senao o conferidor recusaria tudo.
        let fonte = r#"("t.ola") ("t.sim") "t.Maiuscula" "t.""#;
        assert!(conferir_laco(TABELA, &chaves_no_fonte(fonte, "t.")).fechado());
    }

    #[test]
    fn a_tabela_de_teste_e_conferida() {
        assert!(defeitos_da_tabela(TABELA, "t.").is_empty());
        const RUIM: &[TextoDeFabrica] = &[
            texto!("t.a", "", "", "", "", "", ""),
            texto!("t.a", "{x}", "{y}", "", "", "", ""),
            texto!("u.b", "b", "", "", "", "", ""),
        ];
        assert_eq!(defeitos_da_tabela(RUIM, "t.").len(), 4);
        assert_eq!(vazias_por_idioma(TABELA), [0, 0, 0, 1, 0, 1]);
    }
}

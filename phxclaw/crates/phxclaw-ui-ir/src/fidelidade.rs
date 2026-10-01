//! Prova de fidelidade da conversao de tela, em ida e volta e deterministica: SQL ->
//! `design_erp_ui` -> PNG (Chromium) -> `screenshot_to_erp_ui` -> UI-IR, comparado com o
//! UI-IR de origem. Aqui fica a parte pura: o gabarito e a comparacao. Quem renderiza e le a
//! imagem e o agente (`phxclaw-agent/src/fidelidade_ui.rs`).
//!
//! O que se mede e ESTRUTURA, nao aparencia (LPIPS e CLIP morreram na triagem de 01/10: a
//! tela gerada difere da original de proposito): campo achado, perdido e inventado; rotulo
//! exato; tipo; obrigatorio; ordem (tau de Kendall); grupo (concordancia por pares, o
//! indice de Rand); e posicao (distancia do canto do rotulo, em fracao da imagem).
//!
//! A chave fica FORA da conta: o SQL convertido sempre cria `id`, entao conta-la daria um
//! acerto por tela que o OCR nao precisou ler.

use crate::imagem::normalizar;
use crate::ir::*;
use serde::Serialize;

/// Uma tela do gabarito: SQL de origem e a tabela cuja tela de edicao se fotografa.
pub struct Gabarito {
    pub nome: &'static str,
    /// cadastro | lookup | mestre_detalhe | secoes | obrigatorios
    pub padrao: &'static str,
    pub tabela: &'static str,
    pub sql: &'static str,
}

/// 20 telas sinteticas. Variam o que a leitura tem de acertar: cadastro simples, campo
/// obrigatorio, lista fixa, chave estrangeira (lookup), muitas secoes (a tela "com abas"
/// do ERP vira secoes aqui: o UI-IR nao tem aba), mestre-detalhe com e sem total.
pub const GABARITO: &[Gabarito] = &[
    Gabarito {
        nome: "cliente",
        padrao: "cadastro",
        tabela: "cliente",
        sql: "CREATE TABLE cliente (id SERIAL PRIMARY KEY, razao_social VARCHAR(120) NOT NULL, \
cnpj VARCHAR(18) NOT NULL, email VARCHAR(120), telefone VARCHAR(20), ativo BOOLEAN NOT NULL DEFAULT true);",
    },
    Gabarito {
        nome: "produto",
        padrao: "cadastro",
        tabela: "produto",
        sql: "CREATE TABLE produto (id SERIAL PRIMARY KEY, descricao VARCHAR(200) NOT NULL, \
preco NUMERIC(12,2) NOT NULL, peso_kg NUMERIC(10,3), ativo BOOLEAN);",
    },
    Gabarito {
        nome: "categoria",
        padrao: "cadastro",
        tabela: "categoria",
        sql: "CREATE TABLE categoria (id SERIAL PRIMARY KEY, nome VARCHAR(60) NOT NULL, \
descricao TEXT);",
    },
    Gabarito {
        nome: "veiculo",
        padrao: "cadastro",
        tabela: "veiculo",
        sql: "CREATE TABLE veiculo (id SERIAL PRIMARY KEY, placa VARCHAR(8) NOT NULL, \
modelo VARCHAR(60) NOT NULL, ano INTEGER, cor VARCHAR(30), km_atual INTEGER);",
    },
    Gabarito {
        nome: "conta_bancaria",
        padrao: "cadastro",
        tabela: "conta",
        sql: "CREATE TABLE conta (id SERIAL PRIMARY KEY, banco VARCHAR(60) NOT NULL, \
agencia VARCHAR(10) NOT NULL, numero VARCHAR(20) NOT NULL, saldo_inicial NUMERIC(12,2), ativo BOOLEAN);",
    },
    Gabarito {
        nome: "fornecedor",
        padrao: "obrigatorios",
        tabela: "fornecedor",
        sql: "CREATE TABLE fornecedor (id SERIAL PRIMARY KEY, nome VARCHAR(120) NOT NULL, \
cnpj VARCHAR(18) NOT NULL, cidade VARCHAR(60) NOT NULL, \
uf VARCHAR(2) NOT NULL CHECK (uf IN ('SC','PR','RS','SP')), telefone VARCHAR(20) NOT NULL, email VARCHAR(120));",
    },
    Gabarito {
        nome: "funcionario",
        padrao: "obrigatorios",
        tabela: "funcionario",
        sql: "CREATE TABLE funcionario (id SERIAL PRIMARY KEY, nome VARCHAR(120) NOT NULL, \
cpf VARCHAR(14) NOT NULL, dt_nascimento DATE NOT NULL, \
cargo VARCHAR(20) NOT NULL CHECK (cargo IN ('vendedor','gerente','caixa')), salario NUMERIC(12,2) NOT NULL, email VARCHAR(120));",
    },
    Gabarito {
        nome: "transportadora",
        padrao: "obrigatorios",
        tabela: "transportadora",
        sql: "CREATE TABLE transportadora (id SERIAL PRIMARY KEY, razao_social VARCHAR(120) NOT NULL, \
cnpj VARCHAR(18) NOT NULL, telefone VARCHAR(20), \
modal VARCHAR(12) NOT NULL CHECK (modal IN ('rodoviario','aereo','maritimo')));",
    },
    Gabarito {
        nome: "paciente",
        padrao: "secoes",
        tabela: "paciente",
        sql: "CREATE TABLE paciente (id SERIAL PRIMARY KEY, nome VARCHAR(120) NOT NULL, \
cpf VARCHAR(14) NOT NULL, dt_nascimento DATE, telefone VARCHAR(20), email VARCHAR(120), \
valor_consulta NUMERIC(12,2), observacao TEXT, criado_em TIMESTAMP NOT NULL DEFAULT now());",
    },
    Gabarito {
        nome: "imovel",
        padrao: "secoes",
        tabela: "imovel",
        sql: "CREATE TABLE imovel (id SERIAL PRIMARY KEY, endereco VARCHAR(160) NOT NULL, \
cep VARCHAR(9), cidade VARCHAR(60) NOT NULL, area NUMERIC(10,2), valor_aluguel NUMERIC(12,2) NOT NULL, \
situacao VARCHAR(12) NOT NULL CHECK (situacao IN ('livre','alugado','reforma')), observacao TEXT);",
    },
    Gabarito {
        nome: "contrato",
        padrao: "secoes",
        tabela: "contrato",
        sql: "CREATE TABLE contrato (id SERIAL PRIMARY KEY, numero VARCHAR(20) NOT NULL, \
dt_inicio DATE NOT NULL, dt_fim DATE, valor_mensal NUMERIC(12,2) NOT NULL, observacao TEXT, \
criado_em TIMESTAMP NOT NULL DEFAULT now(), alterado_em TIMESTAMP NOT NULL DEFAULT now());",
    },
    Gabarito {
        nome: "cidade_cliente",
        padrao: "lookup",
        tabela: "cliente",
        sql: "CREATE TABLE cidade (id SERIAL PRIMARY KEY, nome VARCHAR(60) NOT NULL); \
CREATE TABLE cliente (id SERIAL PRIMARY KEY, nome VARCHAR(120) NOT NULL, \
cidade_id INTEGER NOT NULL REFERENCES cidade(id), telefone VARCHAR(20), email VARCHAR(120));",
    },
    Gabarito {
        nome: "produto_categoria",
        padrao: "lookup",
        tabela: "produto",
        sql: "CREATE TABLE categoria (id SERIAL PRIMARY KEY, nome VARCHAR(60) NOT NULL); \
CREATE TABLE produto (id SERIAL PRIMARY KEY, descricao VARCHAR(200) NOT NULL, \
categoria_id INTEGER NOT NULL REFERENCES categoria(id), preco NUMERIC(12,2) NOT NULL, estoque INTEGER);",
    },
    Gabarito {
        nome: "pedido",
        padrao: "mestre_detalhe",
        tabela: "pedido",
        sql: "CREATE TABLE cliente (id SERIAL PRIMARY KEY, razao_social VARCHAR(120) NOT NULL); \
CREATE TABLE produto (id SERIAL PRIMARY KEY, descricao VARCHAR(200) NOT NULL); \
CREATE TABLE pedido (id SERIAL PRIMARY KEY, cliente_id INTEGER NOT NULL REFERENCES cliente(id), \
dt_emissao DATE NOT NULL, situacao VARCHAR(12) NOT NULL CHECK (situacao IN ('aberto','faturado')), \
vl_frete NUMERIC(12,2), obs TEXT); \
CREATE TABLE pedido_item (id SERIAL PRIMARY KEY, pedido_id INTEGER NOT NULL REFERENCES pedido(id), \
produto_id INTEGER NOT NULL REFERENCES produto(id), quantidade NUMERIC(12,3) NOT NULL, \
preco_unitario NUMERIC(12,2) NOT NULL, vl_total NUMERIC(12,2) NOT NULL);",
    },
    Gabarito {
        nome: "nota_fiscal",
        padrao: "mestre_detalhe",
        tabela: "nota",
        sql: "CREATE TABLE nota (id SERIAL PRIMARY KEY, numero VARCHAR(20) NOT NULL, \
dt_emissao DATE NOT NULL, cnpj VARCHAR(18) NOT NULL); \
CREATE TABLE nota_item (id SERIAL PRIMARY KEY, nota_id INTEGER NOT NULL REFERENCES nota(id), \
descricao VARCHAR(120) NOT NULL, quantidade NUMERIC(12,3) NOT NULL, valor_total NUMERIC(12,2) NOT NULL);",
    },
    Gabarito {
        nome: "orcamento",
        padrao: "mestre_detalhe",
        tabela: "orcamento",
        sql: "CREATE TABLE orcamento (id SERIAL PRIMARY KEY, cliente VARCHAR(120) NOT NULL, \
dt_validade DATE, observacao TEXT); \
CREATE TABLE orcamento_item (id SERIAL PRIMARY KEY, orcamento_id INTEGER NOT NULL REFERENCES orcamento(id), \
descricao VARCHAR(120) NOT NULL, quantidade NUMERIC(12,3) NOT NULL, preco_unitario NUMERIC(12,2), \
valor_total NUMERIC(12,2) NOT NULL);",
    },
    Gabarito {
        nome: "ordem_servico",
        padrao: "mestre_detalhe",
        tabela: "ordem",
        sql: "CREATE TABLE ordem (id SERIAL PRIMARY KEY, cliente VARCHAR(120) NOT NULL, \
equipamento VARCHAR(80) NOT NULL, dt_entrada DATE NOT NULL, \
situacao VARCHAR(12) NOT NULL CHECK (situacao IN ('aberta','pronta','entregue'))); \
CREATE TABLE ordem_item (id SERIAL PRIMARY KEY, ordem_id INTEGER NOT NULL REFERENCES ordem(id), \
servico VARCHAR(120) NOT NULL, horas NUMERIC(6,2), valor_total NUMERIC(12,2) NOT NULL);",
    },
    Gabarito {
        nome: "compra",
        padrao: "mestre_detalhe",
        tabela: "compra",
        sql: "CREATE TABLE compra (id SERIAL PRIMARY KEY, fornecedor VARCHAR(120) NOT NULL, \
dt_compra DATE NOT NULL, vl_frete NUMERIC(12,2)); \
CREATE TABLE compra_item (id SERIAL PRIMARY KEY, compra_id INTEGER NOT NULL REFERENCES compra(id), \
descricao VARCHAR(120) NOT NULL, quantidade NUMERIC(12,3) NOT NULL, vl_total NUMERIC(12,2) NOT NULL);",
    },
    Gabarito {
        nome: "requisicao",
        padrao: "mestre_detalhe",
        tabela: "requisicao",
        sql: "CREATE TABLE requisicao (id SERIAL PRIMARY KEY, setor VARCHAR(60) NOT NULL, \
dt_pedido DATE NOT NULL, responsavel VARCHAR(80)); \
CREATE TABLE requisicao_item (id SERIAL PRIMARY KEY, requisicao_id INTEGER NOT NULL REFERENCES requisicao(id), \
material VARCHAR(120) NOT NULL, quantidade NUMERIC(12,3) NOT NULL);",
    },
    Gabarito {
        nome: "inventario",
        padrao: "mestre_detalhe",
        tabela: "inventario",
        sql: "CREATE TABLE inventario (id SERIAL PRIMARY KEY, deposito VARCHAR(60) NOT NULL, \
dt_contagem DATE NOT NULL, observacao TEXT); \
CREATE TABLE inventario_item (id SERIAL PRIMARY KEY, inventario_id INTEGER NOT NULL REFERENCES inventario(id), \
produto VARCHAR(120) NOT NULL, quantidade NUMERIC(12,3) NOT NULL, valor_total NUMERIC(12,2));",
    },
];

/// A tela de edicao (cadastro ou documento) da tabela: e ela que se fotografa.
pub fn tela_de(app: &App, tabela: &str) -> Option<String> {
    app.screens.iter().find_map(|s| match s {
        Screen::Form { id, entity, .. } if entity == tabela => Some(id.clone()),
        Screen::MasterDetail { id, master, .. } if master == tabela => Some(id.clone()),
        _ => None,
    })
}

/// Campo como a tela o mostra: rotulo, tipo, obrigatorio, grupo e a ordem.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CampoDaTela {
    pub entidade: String,
    pub campo: String,
    pub rotulo: String,
    pub tipo: String,
    pub obrigatorio: bool,
    /// Titulo da secao, ou "grade:" + titulo da grade.
    pub grupo: String,
}

fn tipo(w: &Widget) -> &'static str {
    match w {
        Widget::Text => "text",
        Widget::TextArea => "text_area",
        Widget::Integer => "integer",
        Widget::Decimal { .. } => "decimal",
        Widget::Money => "money",
        Widget::Date => "date",
        Widget::DateTime => "date_time",
        Widget::Checkbox => "checkbox",
        Widget::Select { .. } => "select",
        Widget::Lookup { .. } => "lookup",
        Widget::Email => "email",
        Widget::Phone => "phone",
    }
}

/// Os campos da tela na ordem em que ela os desenha (a do DOM: secao a secao, depois a
/// grade), com o layout aplicado -- e o MESMO `com_layout` que os renderizadores usam.
/// A chave primaria fica de fora (ver o topo do modulo).
pub fn campos_da_tela(app: &App, tela: &str) -> Vec<CampoDaTela> {
    let app = app.com_layout();
    let Some(s) = app.screens.iter().find(|s| s.id() == tela) else {
        return vec![];
    };
    let titulo_grade = |l: &ScreenLayout, ent: &str| {
        l.items
            .iter()
            .find(|i| i.entity == ent)
            .and_then(|i| {
                l.groups
                    .iter()
                    .find(|g| g.id == i.group && g.kind == "grid")
            })
            .map(|g| g.title.clone())
            .filter(|t| !t.is_empty())
    };
    let mut v = vec![];
    let mut empurra = |ent: &str, nome: &str, grupo: String| {
        let Some(e) = app.entities.iter().find(|e| e.name == ent) else {
            return;
        };
        if e.primary_key.iter().any(|k| k == nome) {
            return;
        }
        if let Some(f) = e.fields.iter().find(|f| f.name == nome) {
            v.push(CampoDaTela {
                entidade: ent.into(),
                campo: nome.into(),
                rotulo: f.label.clone(),
                tipo: tipo(&f.widget).into(),
                obrigatorio: f.required,
                grupo,
            });
        }
    };
    let (ent, secoes) = match s {
        Screen::Form {
            entity, sections, ..
        } => (entity, sections),
        Screen::MasterDetail { master, header, .. } => (master, header),
        Screen::List { .. } | Screen::Painel { .. } => return vec![],
    };
    for sec in secoes {
        for f in &sec.fields {
            empurra(ent, f, sec.title.clone());
        }
    }
    if let Screen::MasterDetail {
        detail,
        detail_columns,
        ..
    } = s
    {
        let titulo = app
            .layouts
            .iter()
            .find(|l| l.screen == tela)
            .and_then(|l| titulo_grade(l, detail))
            .or_else(|| {
                app.entities
                    .iter()
                    .find(|e| &e.name == detail)
                    .map(|e| e.label_plural.clone())
            })
            .unwrap_or_default();
        for c in detail_columns {
            empurra(detail, c, format!("grade:{titulo}"));
        }
    }
    v
}

/// Tau de Kendall (tau-a) entre duas ordens dos mesmos itens: 1 = mesma ordem, -1 =
/// invertida. `None` com menos de dois itens, que nao tem ordem para errar.
pub fn kendall_tau(a: &[usize], b: &[usize]) -> Option<f64> {
    let n = a.len().min(b.len());
    if n < 2 {
        return None;
    }
    let (mut conc, mut disc) = (0i64, 0i64);
    for i in 0..n {
        for j in i + 1..n {
            let s = (a[i] as i64 - a[j] as i64).signum() * (b[i] as i64 - b[j] as i64).signum();
            match s {
                1 => conc += 1,
                -1 => disc += 1,
                _ => {}
            }
        }
    }
    Some((conc - disc) as f64 / (n * (n - 1) / 2) as f64)
}

/// Medida de UMA tela.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Medida {
    pub tela: String,
    pub padrao: String,
    pub origem: usize,
    pub achados: usize,
    pub perdidos: Vec<String>,
    pub inventados: Vec<String>,
    pub rotulo_exato: usize,
    pub tipo_igual: usize,
    pub obrigatorio_igual: usize,
    pub kendall_tau: Option<f64>,
    /// Pares de campos achados em que "mesmo grupo na origem" == "mesmo grupo lido".
    pub grupo_rand: Option<f64>,
    pub titulo_do_grupo_igual: usize,
    /// Distancia do canto superior esquerdo do rotulo, em fracao da imagem (0..~1,4).
    pub distancia_media: Option<f64>,
    pub distancia_max: Option<f64>,
    pub com_posicao: usize,
}

impl Medida {
    pub fn revocacao(&self) -> f64 {
        if self.origem == 0 {
            return 1.0;
        }
        self.achados as f64 / self.origem as f64
    }
    pub fn precisao(&self) -> f64 {
        let lidos = self.achados + self.inventados.len();
        if lidos == 0 {
            return 0.0;
        }
        self.achados as f64 / lidos as f64
    }
    fn fracao(&self, n: usize) -> Option<f64> {
        (self.achados > 0).then(|| n as f64 / self.achados as f64)
    }
    pub fn rotulo_exato_fracao(&self) -> Option<f64> {
        self.fracao(self.rotulo_exato)
    }
    pub fn tipo_fracao(&self) -> Option<f64> {
        self.fracao(self.tipo_igual)
    }
    pub fn obrigatorio_fracao(&self) -> Option<f64> {
        self.fracao(self.obrigatorio_igual)
    }
    pub fn titulo_do_grupo_fracao(&self) -> Option<f64> {
        self.fracao(self.titulo_do_grupo_igual)
    }
}

/// Compara a tela de origem com a convertida. `caixas_origem` sao as caixas dos rotulos
/// medidas no navegador (entidade, campo, caixa); as da convertida saem do layout dela.
pub fn comparar(
    origem: &App,
    tela_origem: &str,
    padrao: &str,
    caixas_origem: &[(String, String, Caixa)],
    convertido: &App,
    tabela: &str,
) -> Medida {
    let o = campos_da_tela(origem, tela_origem);
    let tela_conv = tela_de(convertido, &crate::imagem::coluna(tabela));
    let c = tela_conv
        .as_deref()
        .map(|t| campos_da_tela(convertido, t))
        .unwrap_or_default();
    // pares (indice na origem, indice na convertida) pelo rotulo, como o OCR compara
    let mut pares: Vec<(usize, usize)> = vec![];
    for (i, a) in o.iter().enumerate() {
        let n = normalizar(&a.rotulo);
        if let Some(j) = c
            .iter()
            .enumerate()
            .position(|(j, b)| normalizar(&b.rotulo) == n && !pares.iter().any(|p| p.1 == j))
        {
            pares.push((i, j));
        }
    }
    let perdidos = o
        .iter()
        .enumerate()
        .filter(|(i, _)| !pares.iter().any(|p| p.0 == *i))
        .map(|(_, a)| a.rotulo.clone())
        .collect();
    let inventados = c
        .iter()
        .enumerate()
        .filter(|(j, _)| !pares.iter().any(|p| p.1 == *j))
        .map(|(_, b)| b.rotulo.clone())
        .collect();
    let conta = |f: &dyn Fn(&CampoDaTela, &CampoDaTela) -> bool| {
        pares.iter().filter(|(i, j)| f(&o[*i], &c[*j])).count()
    };
    let rotulo_exato = conta(&|a, b| a.rotulo == b.rotulo);
    let tipo_igual = conta(&|a, b| a.tipo == b.tipo);
    let obrigatorio_igual = conta(&|a, b| a.obrigatorio == b.obrigatorio);
    let titulo_do_grupo_igual = conta(&|a, b| normalizar(&a.grupo) == normalizar(&b.grupo));
    let ordem_o: Vec<usize> = pares.iter().map(|p| p.0).collect();
    let ordem_c: Vec<usize> = pares.iter().map(|p| p.1).collect();
    let kendall = kendall_tau(&ordem_o, &ordem_c);
    let grupo_rand = (pares.len() >= 2).then(|| {
        let (mut ok, mut tot) = (0usize, 0usize);
        for x in 0..pares.len() {
            for y in x + 1..pares.len() {
                let (a1, b1) = (&o[pares[x].0], &c[pares[x].1]);
                let (a2, b2) = (&o[pares[y].0], &c[pares[y].1]);
                tot += 1;
                if (a1.grupo == a2.grupo) == (b1.grupo == b2.grupo) {
                    ok += 1;
                }
            }
        }
        ok as f64 / tot as f64
    });
    let caixa_conv = |ent: &str, campo: &str| {
        convertido
            .layouts
            .iter()
            .flat_map(|l| &l.items)
            .find(|i| i.entity == ent && i.field == campo)
            .map(|i| i.bbox)
    };
    let dist: Vec<f64> = pares
        .iter()
        .filter_map(|(i, j)| {
            let a = caixas_origem
                .iter()
                .find(|(e, f, _)| *e == o[*i].entidade && *f == o[*i].campo)?
                .2;
            let b = caixa_conv(&c[*j].entidade, &c[*j].campo)?;
            Some((((a.x - b.x) as f64).powi(2) + ((a.y - b.y) as f64).powi(2)).sqrt())
        })
        .collect();
    let r4 = |v: f64| (v * 10000.0).round() / 10000.0;
    Medida {
        tela: tela_origem.into(),
        padrao: padrao.into(),
        origem: o.len(),
        achados: pares.len(),
        perdidos,
        inventados,
        rotulo_exato,
        tipo_igual,
        obrigatorio_igual,
        kendall_tau: kendall.map(r4),
        grupo_rand: grupo_rand.map(r4),
        titulo_do_grupo_igual,
        distancia_media: (!dist.is_empty())
            .then(|| r4(dist.iter().sum::<f64>() / dist.len() as f64)),
        distancia_max: dist.iter().copied().reduce(f64::max).map(r4),
        com_posicao: dist.len(),
    }
}

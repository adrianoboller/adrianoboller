//! Pedidos 765 e 767, fatia P1: a CAMADA UNICA DE PROTECAO.
//!
//! Desenho: `docs/propostas/protecao-765-desenho.md` §4.1. Decisoes do dono
//! que mandam aqui, acima do texto do desenho:
//!
//! * proteger e o padrao de fabrica, e a promessa ao cliente e «protege e
//!   bloqueia» -- excecao explicita a «guarda nova entra pedida»;
//! * sem a senha de execucao, o comando da lista de perigo NAO executa, em
//!   modo nenhum;
//! * a senha de execucao e uma SEGUNDA senha, nao a de login (P14), e uma vez
//!   informada ela libera a SESSAO ate a sessao terminar -- logout,
//!   inatividade, queda da conexao ou o `trancar_execucao` (o `sudo -k`).
//!
//! # Classifica a OP, nao o texto
//!
//! E a licao do portao de permissao: o campo que se le e o furo. `DROP VIEW`
//! vira `excluir_visao` antes de chegar a camada, entao a rede, a op `sql`,
//! o job e o corpo de uma rotina chegam com o MESMO nome -- e um texto que o
//! tradutor nao entende nao executa nada para a camada precisar ver.
//!
//! # Os pontos que chamam, e por que nao sao um so
//!
//! A decisao e UMA ([`Veredito`], daqui). Quem pergunta:
//!
//! 1. `executar_e_contar_escrita_local` -- por onde passam os tres irmaos
//!    (`despachar`, `executar_derivado`, job). E o ponto do desenho.
//! 2. `sql_de_cadastro` -- o `CREATE/ALTER/DROP USER` pela op `sql` chamava
//!    o `executar` direto, sem passar pelo ponto 1. Era a porta dos fundos
//!    desta camada, achada procurando quem chama `executar` sem os irmaos.
//! 3. o PLANO de escrita (F8, `plano_largo`): o `DELETE`/`UPDATE` por faixa
//!    chega a camada como mil `excluir` de UMA linha cada; o tamanho so existe
//!    onde a lista de rowids fecha. O mesmo vale para a cascata do
//!    `ao_alterar`, nos dois irmaos que a planejam (solto e empilhado).
//!
//! # A replica fica isenta, e por construcao
//!
//! A replica aplica o que veio da origem por `Table::aplicar_evento`, por
//! dentro, e o `aplicar` empurrado pela rede so carrega evento de linha. O
//! comando ja passou por esta camada NA ORIGEM; exigir a senha de novo na
//! replica pararia a replicacao sem proteger nada. Por isso nenhuma op de
//! replicacao esta na lista, e ha teste que trava isso.
//!
//! # Custo para quem nao e perigoso
//!
//! O portao e o `match` de [`classe`]: op fora da lista volta `Livre` sem
//! alocar nada. So as de reescrita medem a tabela, e so elas pagam a trava.

use phxsql_core::error::PhxError;
use phxsql_core::json::Json;

/// Por que o comando esta na lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Categoria {
    /// DROP de tabela, visao, sequencia, chave estrangeira ou usuario, e o
    /// indice de texto que some num `redeclarar_indices_texto`.
    DestruirEstrutura,
    /// O que apaga de vez em massa: o TRUNCATE daqui (`esvaziar_lixeira`,
    /// `expurgar_trilha`) e o `DELETE` largo da F8.
    ApagarEmMassa,
    /// O `UPDATE` largo da F8 e a cascata larga do `ao_alterar`.
    AlterarEmMassa,
    /// ALTER e indice que reescrevem uma tabela grande.
    ReescreverTabelaGrande,
    /// O cadastro: GRANT/REVOKE e ALTER USER (`usuario_alterar`) e o CREATE
    /// USER de administrador.
    Acesso,
    /// A escrita que BAIXA a propria guarda: uma linha de `phxsys.protecao`
    /// que sai de `proteger` (P13). Nunca consulta a tabela -- a guarda que
    /// se deixasse desligar pela linha que ela mesma protege nao guardaria
    /// nada.
    APropriaGuarda,
}

impl Categoria {
    pub fn nome(self) -> &'static str {
        match self {
            Categoria::DestruirEstrutura => "destruir_estrutura",
            Categoria::ApagarEmMassa => "apagar_em_massa",
            Categoria::AlterarEmMassa => "alterar_em_massa",
            Categoria::ReescreverTabelaGrande => "reescrever_tabela_grande",
            Categoria::Acesso => "acesso",
            Categoria::APropriaGuarda => "a_propria_guarda",
        }
    }
}

/// O que a senha de execucao teria de cobrir: a op, onde, e quanto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Escopo {
    pub op: String,
    pub database: String,
    pub tabela: String,
    pub categoria: Categoria,
    /// Linhas do plano e vivas na tabela, quando a medida e um plano.
    pub linhas: Option<(u64, u64)>,
}

impl Escopo {
    /// O detalhe da recusa. Sao DADOS -- op, alvo, categoria, numeros --, e a
    /// frase que explica vem da moldura traduzida (`erro.senha_de_execucao_exigida`).
    pub fn descrever(&self) -> String {
        let mut s = self.op.clone();
        match (self.database.is_empty(), self.tabela.is_empty()) {
            (false, false) => s.push_str(&format!(" em {}.{}", self.database, self.tabela)),
            (false, true) => s.push_str(&format!(" em {}", self.database)),
            (true, false) => s.push_str(&format!(" em {}", self.tabela)),
            (true, true) => {}
        }
        s.push_str(&format!(" ({}", self.categoria.nome()));
        if let Some((linhas, vivas)) = self.linhas {
            s.push_str(&format!(", {linhas} de {vivas} linhas"));
        }
        s.push(')');
        s
    }
}

/// O veredito da camada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Veredito {
    /// Nao e da lista, ou a sessao esta liberada pela senha de execucao.
    Livre,
    /// E da lista: so executa com a sessao liberada.
    ExigeSenhaDeExecucao { escopo: Escopo },
    /// E da lista e a sessao NAO esta liberada.
    Recusado { escopo: Escopo },
}

impl Veredito {
    /// A pergunta da precisao do dono: «a sessao esta liberada?». Liberada,
    /// o perigoso passa; nao liberada, recusa. A P14 e quem LIBERA; aqui so
    /// se le.
    pub fn para_a_sessao(self, liberada: bool) -> Veredito {
        match self {
            Veredito::ExigeSenhaDeExecucao { .. } if liberada => Veredito::Livre,
            Veredito::ExigeSenhaDeExecucao { escopo } => Veredito::Recusado { escopo },
            outro => outro,
        }
    }

    /// O erro que sai, ou `Ok`. `ExigeSenhaDeExecucao` que chegasse aqui sem
    /// ter sido resolvido contra a sessao tambem recusa: na duvida, nao
    /// executa.
    pub fn em_resultado(self) -> Result<(), PhxError> {
        match self {
            Veredito::Livre => Ok(()),
            Veredito::ExigeSenhaDeExecucao { escopo } | Veredito::Recusado { escopo } => {
                Err(PhxError::SenhaDeExecucaoExigida(escopo.descrever()))
            }
        }
    }
}

/// A classe de uma op, sem medir nada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classe {
    Livre,
    Perigosa(Categoria),
    /// So e perigosa se a tabela for grande -- ou, no indice de texto, se a
    /// declaracao nova tirar um indice que existe. Quem chama mede.
    SeGrande,
}

/// A LISTA DE PERIGO, num lugar so.
///
/// Os nomes sao os REAIS do catalogo. Do que a ordem pede e nao tem op aqui:
/// DROP de base (nao existe op que apague um database), DROP INDEX e CREATE
/// INDEX de B+tree (o indice nasce com a tabela; o que se cria e se tira
/// depois e o de texto, `redeclarar_indices_texto`), TRUNCATE (o que apaga
/// em massa de vez e o `esvaziar_lixeira`) e GRANT/REVOKE (o direito muda
/// pelo `usuario_alterar`).
pub fn classe(op: &str, pedido: &Json) -> Classe {
    match op {
        "excluir_tabela" | "excluir_visao" | "excluir_sequencia" | "excluir_fk"
        | "usuario_excluir" => Classe::Perigosa(Categoria::DestruirEstrutura),
        "esvaziar_lixeira" | "expurgar_trilha" => Classe::Perigosa(Categoria::ApagarEmMassa),
        "usuario_alterar" => Classe::Perigosa(Categoria::Acesso),
        "usuario_criar" if crate::usuarios::pedido_faz_administrador(pedido) => {
            Classe::Perigosa(Categoria::Acesso)
        }
        "acrescentar_coluna" | "criptografar" | "descriptografar" | "redeclarar_indices_texto" => {
            Classe::SeGrande
        }
        // Sem `tabela` e varredura que so relata; sem `confirmar` e a vista
        // previa. Nenhuma das duas reescreve nada.
        "migrar_esquema"
            if !pedido.texto_ou("tabela", "").trim().is_empty()
                && !pedido.texto_ou("confirmar", "").trim().is_empty() =>
        {
            Classe::SeGrande
        }
        _ => Classe::Livre,
    }
}

fn escopo_do_pedido(op: &str, pedido: &Json, categoria: Categoria) -> Escopo {
    // O usuario e o alvo do cadastro, e vai no lugar da tabela.
    let tabela = match op {
        "usuario_criar" | "usuario_alterar" | "usuario_excluir" => {
            pedido.texto_ou("login", pedido.texto_ou("usuario", ""))
        }
        "excluir_visao" | "excluir_sequencia" | "excluir_fk" => {
            match pedido.texto_ou("tabela", "").trim() {
                "" => pedido.texto_ou("nome", ""),
                t => t,
            }
        }
        _ => pedido.texto_ou("tabela", ""),
    };
    Escopo {
        op: op.to_string(),
        database: pedido.texto_ou("database", "").trim().to_string(),
        tabela: tabela.trim().to_string(),
        categoria,
        linhas: None,
    }
}

/// O veredito de uma op [`Classe::Perigosa`].
pub fn da_op_perigosa(op: &str, pedido: &Json, categoria: Categoria) -> Veredito {
    Veredito::ExigeSenhaDeExecucao {
        escopo: escopo_do_pedido(op, pedido, categoria),
    }
}

/// O veredito de uma op [`Classe::SeGrande`], com a medida na mao: os slots
/// da tabela (o que a reescrita paga -- o `.reg` nunca reaproveita slot) e se
/// a declaracao nova tira um indice de texto que existe.
///
/// O piso e o MESMO da F8 ([`crate::plano_largo::PISO_DE_LINHAS`]): duas
/// ideias de «grande» divergiriam no primeiro ajuste.
pub fn da_medida(op: &str, pedido: &Json, slots: u64, tira_indice: bool) -> Veredito {
    if tira_indice {
        return da_op_perigosa(op, pedido, Categoria::DestruirEstrutura);
    }
    if slots >= crate::plano_largo::PISO_DE_LINHAS {
        let mut escopo = escopo_do_pedido(op, pedido, Categoria::ReescreverTabelaGrande);
        escopo.linhas = Some((slots, slots));
        return Veredito::ExigeSenhaDeExecucao { escopo };
    }
    Veredito::Livre
}

/// O veredito de um plano que a F8 ja achou largo. `rotulo` e o da F8:
/// `excluir_por_faixa`, `atualizar_por_faixa` ou `cascata`.
pub fn do_plano(rotulo: &str, database: &str, tabela: &str, linhas: u64, vivas: u64) -> Veredito {
    let categoria = if rotulo == "excluir_por_faixa" {
        Categoria::ApagarEmMassa
    } else {
        Categoria::AlterarEmMassa
    };
    Veredito::ExigeSenhaDeExecucao {
        escopo: Escopo {
            op: rotulo.to_string(),
            database: database.to_string(),
            tabela: tabela.to_string(),
            categoria,
            linhas: Some((linhas, vivas)),
        },
    }
}

// ------------------------------------------------ a tabela `phxsys.protecao` (P12)

/// Onde mora a tabela de configuracao da camada: o MESMO database de sistema
/// do `phxsys.mensagens`, que quem administra ja abre na grade.
pub const DATABASE: &str = "phxsys";
pub const TABELA: &str = "protecao";

/// Quantas linhas a camada le da tabela. A fabrica tem 17; o teto existe para
/// a leitura nunca virar varredura sem fim dentro da decisao. O que passar
/// dele fica no padrao de fabrica, que e `proteger` -- o lado estrito.
pub const TETO_DE_LINHAS: u64 = 1_000;

/// O modo de uma linha de `phxsys.protecao`, em ordem: o maior protege mais.
///
/// # O que cada um muda, e o que NAO muda (decisao desta fatia)
///
/// Ordem do dono no 767: a tabela «habilita ou nao o monitoramento» de cada
/// comando. Precisao do dono, do mesmo dia: «sem a senha de execucao esses
/// comandos nao sao executados, em qualquer modo». As duas so cabem juntas
/// assim:
///
/// * `proteger` (fabrica): exige a segunda senha; o executado vai a trilha.
/// * `observar`: o MONITORAMENTO continua ligado (trilha); a senha so deixa
///   de ser exigida se o dono do servidor ligar
///   `protecao.modo_dispensa_a_senha` (fabrica `false`) -- e e essa a
///   pergunta de produto que sobe ao dono.
/// * `desligado`: monitoramento desligado -- o executado nao vai a trilha. A
///   senha segue a mesma regra do `observar`.
///
/// O BLOQUEADO vira ocorrencia em qualquer modo: recusa escondida seria
/// pior que recusa nenhuma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Modo {
    Desligado,
    Observar,
    Proteger,
}

impl Modo {
    /// O texto da celula. Vazio, nulo ou torto vale `proteger`: um erro de
    /// digitacao nao pode ser o que desliga a guarda. E a MESMA funcao para
    /// quem le a tabela e para quem confere a escrita nela (P13) -- se as
    /// duas lessem «Observar » de jeitos diferentes, a diferenca seria a
    /// porta.
    pub fn de_texto(t: Option<&str>) -> Modo {
        match t.map(|t| t.trim().to_ascii_lowercase()).as_deref() {
            Some("proteger") => Modo::Proteger,
            Some("observar") => Modo::Observar,
            Some("desligado") => Modo::Desligado,
            _ => Modo::Proteger,
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Modo::Desligado => "desligado",
            Modo::Observar => "observar",
            Modo::Proteger => "proteger",
        }
    }

    /// O executado vai a trilha?
    pub fn monitora(self) -> bool {
        self != Modo::Desligado
    }
}

/// A linha de fabrica: a op (ou o rotulo do plano da F8) e a categoria. E
/// a lista de perigo inteira, a mesma de [`classe`] e de [`do_plano`] -- o
/// teste `a_fabrica_e_a_lista_de_perigo` reprova a que divergir.
pub const FABRICA: &[(&str, Categoria)] = &[
    ("excluir_tabela", Categoria::DestruirEstrutura),
    ("excluir_visao", Categoria::DestruirEstrutura),
    ("excluir_sequencia", Categoria::DestruirEstrutura),
    ("excluir_fk", Categoria::DestruirEstrutura),
    ("usuario_excluir", Categoria::DestruirEstrutura),
    ("esvaziar_lixeira", Categoria::ApagarEmMassa),
    ("expurgar_trilha", Categoria::ApagarEmMassa),
    ("excluir_por_faixa", Categoria::ApagarEmMassa),
    ("atualizar_por_faixa", Categoria::AlterarEmMassa),
    ("cascata", Categoria::AlterarEmMassa),
    ("acrescentar_coluna", Categoria::ReescreverTabelaGrande),
    ("criptografar", Categoria::ReescreverTabelaGrande),
    ("descriptografar", Categoria::ReescreverTabelaGrande),
    (
        "redeclarar_indices_texto",
        Categoria::ReescreverTabelaGrande,
    ),
    ("migrar_esquema", Categoria::ReescreverTabelaGrande),
    ("usuario_alterar", Categoria::Acesso),
    ("usuario_criar", Categoria::Acesso),
];

/// Os modos lidos da tabela. Op sem linha vale `proteger` -- a tabela que
/// sumiu, a linha apagada e a que passou do teto ficam todas no lado
/// estrito, e por isso apagar nunca BAIXA a guarda.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Modos(Vec<(String, Modo)>);

impl Modos {
    /// O LEITOR UNICO da tabela, sobre a resposta do `varrer` -- o mesmo
    /// motor da grade. A coluna se acha sem caixa: a tabela recriada a mao
    /// com `Modo` nao pode virar «sem coluna, tudo desligado». Duas linhas
    /// da mesma op valem a mais estrita.
    pub fn de_varredura(resposta: &Json) -> Modos {
        let mut v: Vec<(String, Modo)> = Vec::new();
        for linha in resposta
            .campo("linhas")
            .and_then(Json::lista)
            .unwrap_or(&[])
        {
            let Some(op) = campo_sem_caixa(linha, "op").and_then(Json::texto) else {
                continue;
            };
            let op = op.trim().to_string();
            let modo = Modo::de_texto(campo_sem_caixa(linha, "modo").and_then(Json::texto));
            match v.iter_mut().find(|(o, _)| *o == op) {
                Some((_, m)) => *m = (*m).max(modo),
                None => v.push((op, modo)),
            }
        }
        Modos(v)
    }

    pub fn de(&self, op: &str) -> Modo {
        self.0
            .iter()
            .find(|(o, _)| o == op)
            .map(|(_, m)| *m)
            .unwrap_or(Modo::Proteger)
    }
}

fn campo_sem_caixa<'a>(j: &'a Json, nome: &str) -> Option<&'a Json> {
    match j {
        Json::Objeto(pares) => pares
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(nome))
            .map(|(_, v)| v),
        _ => None,
    }
}

/// O que uma escrita faz com a propria guarda.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toque {
    /// So sobe ou mantem: toda linha que ela deixa vale `proteger`, ou ela
    /// tira linhas (que voltam a fabrica, `proteger`). Nunca pede senha.
    Sobe,
    /// Pode baixar -- ou nao se sabe. Pede a sessao liberada.
    PodeBaixar,
}

/// A escrita toca `phxsys.protecao`? `None` quando nao -- e o caso de todo
/// pedido, decidido por UMA leitura de campo (`database`) e, so quando ela
/// nao e o sistema, mais uma (`destino_database`, a do `copiar_tabela`).
///
/// # Por que «na duvida, pode baixar»
///
/// So tres escritas se deixam ler: `inserir` (o `valores` e o `atualizar`
/// do upsert), `atualizar` (o `valores`) e `excluir` (que so tira linha).
/// Todo o resto que alcanca a tabela -- o lote, a carga, o restaurar da
/// lixeira, renomear/copiar uma tabela PARA ela, restaurar o backup do
/// database de sistema -- pode trazer uma linha baixa sem que a camada
/// consiga ver, e por isso pede a senha. Subir a protecao por esses caminhos
/// tambem pede; subir pelos tres de cima, nunca.
pub fn toque_na_guarda(op: &str, pedido: &Json) -> Option<Toque> {
    let e_sistema = |campo: &str| {
        pedido
            .texto_ou(campo, "")
            .trim()
            .eq_ignore_ascii_case(DATABASE)
    };
    if !e_sistema("database") && !e_sistema("destino_database") {
        return None;
    }
    // O `aplicar` fica de fora como em toda a camada (P5): ele so passa num
    // no que recebe replicacao -- a origem e o isolado o recusam pelo papel
    // (4001), medido em `o_aplicar_na_origem_ja_recusa_pelo_papel` --, e o
    // evento que a replica aplica ja passou pela guarda NA ORIGEM.
    if !crate::servidor::OPS_ESCRITA.contains(&op) {
        return None;
    }
    // Sem caixa: num sistema de arquivos que nao distingue, `PROTECAO` abre
    // o mesmo `.reg`.
    let nomeia = |campo: &str| {
        pedido
            .texto_ou(campo, "")
            .trim()
            .eq_ignore_ascii_case(TABELA)
    };
    if !nomeia("tabela") && !nomeia("destino") && op != "restaurar_backup" {
        return None;
    }
    // `valores` e `linha` sao os dois nomes que o `inserir` e o `atualizar`
    // aceitam (o primeiro ganha); olhar os DOIS fecha a porta de mandar um
    // em proteger e o outro em desligado.
    let linha = || deixa_proteger(pedido.campo("valores")) && deixa_proteger(pedido.campo("linha"));
    let sobe = match op {
        "excluir" => true,
        "inserir" => linha() && deixa_proteger(pedido.campo("atualizar")),
        "atualizar" => linha(),
        _ => false,
    };
    Some(if sobe { Toque::Sobe } else { Toque::PodeBaixar })
}

/// O mapa de valores deixa a coluna `modo` em `proteger`? Ausente e nulo
/// valem `proteger` (o leitor le assim); texto passa pelo MESMO
/// [`Modo::de_texto`]; qualquer outra coisa (numero, lista) e duvida.
fn deixa_proteger(valores: Option<&Json>) -> bool {
    let Some(valores) = valores else {
        return true;
    };
    let Json::Objeto(pares) = valores else {
        return false;
    };
    pares
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case("modo"))
        .all(|(_, v)| match v {
            Json::Nulo => true,
            Json::Texto(t) => Modo::de_texto(Some(t)) == Modo::Proteger,
            _ => false,
        })
}

/// O escopo de uma escrita na propria guarda, para a recusa e a trilha.
pub fn escopo_da_guarda(op: &str) -> Escopo {
    Escopo {
        op: op.to_string(),
        database: DATABASE.to_string(),
        tabela: TABELA.to_string(),
        categoria: Categoria::APropriaGuarda,
        linhas: None,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn p(t: &str) -> Json {
        Json::analisar(t).unwrap()
    }

    /// A lista inteira, op por op, contra o nome REAL no catalogo: uma op
    /// renomeada no catalogo e esquecida aqui sairia da lista calada.
    #[test]
    fn toda_op_da_lista_existe_no_catalogo() {
        for op in [
            "excluir_tabela",
            "excluir_visao",
            "excluir_sequencia",
            "excluir_fk",
            "usuario_excluir",
            "esvaziar_lixeira",
            "expurgar_trilha",
            "usuario_alterar",
            "usuario_criar",
            "acrescentar_coluna",
            "criptografar",
            "descriptografar",
            "redeclarar_indices_texto",
            "migrar_esquema",
        ] {
            assert!(
                crate::catalogo::por_nome(op).is_some(),
                "{op} nao esta no catalogo"
            );
        }
    }

    #[test]
    fn a_classe_de_cada_op() {
        let vazio = p("{}");
        for op in [
            "excluir_tabela",
            "excluir_visao",
            "usuario_excluir",
            "excluir_fk",
        ] {
            assert_eq!(
                classe(op, &vazio),
                Classe::Perigosa(Categoria::DestruirEstrutura),
                "{op}"
            );
        }
        assert_eq!(
            classe("esvaziar_lixeira", &vazio),
            Classe::Perigosa(Categoria::ApagarEmMassa)
        );
        assert_eq!(
            classe("usuario_alterar", &vazio),
            Classe::Perigosa(Categoria::Acesso)
        );
        assert_eq!(classe("acrescentar_coluna", &vazio), Classe::SeGrande);
        // O comportamento velho: o laco quente nao e da lista.
        for op in [
            "inserir",
            "atualizar",
            "excluir",
            "ler",
            "varrer",
            "sql",
            "criar_tabela",
        ] {
            assert_eq!(classe(op, &vazio), Classe::Livre, "{op}");
        }
    }

    /// A replica fica isenta: nenhuma op de replicacao e da lista, e o
    /// `aplicar` de uma exclusao continua livre.
    #[test]
    fn a_replica_fica_isenta() {
        let evento = p(r#"{"database":"loja","tabela":"c","eventos":[{"operacao":"exclusao"}]}"#);
        for op in [
            "aplicar",
            "replicar",
            "posicao",
            "retrato_da_replica",
            "cluster_pulso",
            "replicar_aguardar",
        ] {
            assert_eq!(classe(op, &evento), Classe::Livre, "{op}");
        }
    }

    #[test]
    fn criar_usuario_so_e_perigoso_quando_cria_administrador() {
        assert_eq!(
            classe("usuario_criar", &p(r#"{"login":"a","nivel":"leitor"}"#)),
            Classe::Livre
        );
        assert_eq!(
            classe("usuario_criar", &p(r#"{"login":"a"}"#)),
            Classe::Livre
        );
        for admin in [
            r#"{"login":"a","supervisor":true}"#,
            r#"{"login":"a","nivel":"admin"}"#,
            r#"{"login":"a","bases":{"loja":{"administrar":true}}}"#,
            r#"{"login":"a","bases":{"loja":{"tabelas":{"c":{"administrar":true}}}}}"#,
            r#"{"login":"a","nivel":"torto"}"#,
        ] {
            assert_eq!(
                classe("usuario_criar", &p(admin)),
                Classe::Perigosa(Categoria::Acesso),
                "{admin}"
            );
        }
    }

    #[test]
    fn migrar_esquema_so_e_perigoso_quando_reescreve() {
        assert_eq!(
            classe("migrar_esquema", &p(r#"{"database":"d"}"#)),
            Classe::Livre
        );
        assert_eq!(
            classe("migrar_esquema", &p(r#"{"tabela":"t"}"#)),
            Classe::Livre
        );
        assert_eq!(
            classe("migrar_esquema", &p(r#"{"tabela":"t","confirmar":"t"}"#)),
            Classe::SeGrande
        );
    }

    /// O piso e o da F8, dos dois lados da fronteira.
    #[test]
    fn a_tabela_grande_comeca_no_piso_da_f8() {
        let ped = p(r#"{"database":"d","tabela":"t"}"#);
        let piso = crate::plano_largo::PISO_DE_LINHAS;
        assert_eq!(
            da_medida("acrescentar_coluna", &ped, piso - 1, false),
            Veredito::Livre
        );
        assert!(matches!(
            da_medida("acrescentar_coluna", &ped, piso, false),
            Veredito::ExigeSenhaDeExecucao { .. }
        ));
        // Tirar indice e DROP, qualquer que seja o tamanho.
        assert!(matches!(
            da_medida("redeclarar_indices_texto", &ped, 1, true),
            Veredito::ExigeSenhaDeExecucao { escopo } if escopo.categoria == Categoria::DestruirEstrutura
        ));
    }

    /// A sessao liberada passa; a nao liberada recusa, com o codigo novo.
    #[test]
    fn o_veredito_pergunta_se_a_sessao_esta_liberada() {
        let v = da_op_perigosa(
            "excluir_tabela",
            &p(r#"{"database":"d","tabela":"t"}"#),
            Categoria::DestruirEstrutura,
        );
        assert_eq!(v.clone().para_a_sessao(true), Veredito::Livre);
        let e = v.para_a_sessao(false).em_resultado().unwrap_err();
        assert_eq!(e.codigo(), 4009);
        assert!(
            e.to_string()
                .contains("excluir_tabela em d.t (destruir_estrutura)"),
            "{e}"
        );
        assert!(e.to_string().contains("senha de execucao"), "{e}");
        assert!(Veredito::Livre.para_a_sessao(false).em_resultado().is_ok());
    }

    #[test]
    fn o_plano_diz_quantas_linhas() {
        let e = do_plano("excluir_por_faixa", "d", "t", 2_000, 2_000)
            .para_a_sessao(false)
            .em_resultado()
            .unwrap_err();
        assert!(
            e.to_string()
                .contains("(apagar_em_massa, 2000 de 2000 linhas)"),
            "{e}"
        );
    }

    /// A fabrica e a lista de perigo: toda op da fabrica e da lista (ou e
    /// rotulo de plano da F8), e toda op da lista tem linha de fabrica. Uma
    /// op nova na lista sem linha ficaria sem como baixar; uma linha sem op
    /// seria configuracao que nada le.
    #[test]
    fn a_fabrica_e_a_lista_de_perigo() {
        let planos = ["excluir_por_faixa", "atualizar_por_faixa", "cascata"];
        let largo = p(r#"{"tabela":"t","confirmar":"t","nivel":"admin"}"#);
        for (op, cat) in FABRICA {
            if planos.contains(op) {
                let Veredito::ExigeSenhaDeExecucao { escopo } = do_plano(op, "d", "t", 1, 1) else {
                    panic!("{op}: o plano nao exige a senha");
                };
                assert_eq!(escopo.categoria, *cat, "{op}");
                continue;
            }
            match classe(op, &largo) {
                Classe::Perigosa(c) => assert_eq!(c, *cat, "{op}"),
                Classe::SeGrande => {
                    assert_eq!(*cat, Categoria::ReescreverTabelaGrande, "{op}")
                }
                Classe::Livre => panic!("{op} esta na fabrica e fora da lista"),
            }
        }
        // O inverso: as ops do teste do catalogo, todas com linha.
        for op in [
            "excluir_tabela",
            "excluir_visao",
            "excluir_sequencia",
            "excluir_fk",
            "usuario_excluir",
            "esvaziar_lixeira",
            "expurgar_trilha",
            "usuario_alterar",
            "usuario_criar",
            "acrescentar_coluna",
            "criptografar",
            "descriptografar",
            "redeclarar_indices_texto",
            "migrar_esquema",
        ] {
            assert!(FABRICA.iter().any(|(o, _)| *o == op), "{op} sem linha");
        }
    }

    /// Torto, vazio e nulo valem proteger; a caixa e o espaco nao mudam o
    /// modo; duas linhas da mesma op valem a mais estrita; op sem linha
    /// vale proteger.
    #[test]
    fn o_leitor_da_tabela_fica_no_lado_estrito() {
        assert_eq!(Modo::de_texto(Some(" Observar ")), Modo::Observar);
        assert_eq!(Modo::de_texto(Some("DESLIGADO")), Modo::Desligado);
        for torto in ["", "nada", "observa", "0"] {
            assert_eq!(Modo::de_texto(Some(torto)), Modo::Proteger, "{torto}");
        }
        assert_eq!(Modo::de_texto(None), Modo::Proteger);
        let m = Modos::de_varredura(&p(
            r#"{"linhas":[{"op":"a","modo":"desligado"},{"OP":"b","MODO":"observar"},
                          {"op":"c","modo":"desligado"},{"op":"c","modo":"proteger"},
                          {"op":"d"},{"modo":"desligado"}]}"#,
        ));
        assert_eq!(m.de("a"), Modo::Desligado);
        assert_eq!(m.de("b"), Modo::Observar);
        assert_eq!(m.de("c"), Modo::Proteger);
        assert_eq!(m.de("d"), Modo::Proteger);
        assert_eq!(m.de("sem-linha"), Modo::Proteger);
        assert_eq!(Modos::de_varredura(&p("{}")).de("a"), Modo::Proteger);
    }

    /// A classificacao da escrita na guarda, pelos dois lados: o pedido
    /// comum nao e tocado (o comportamento velho), e o que nao se le pede
    /// a senha.
    #[test]
    fn o_toque_na_guarda() {
        let t = |op: &str, j: &str| toque_na_guarda(op, &p(j));
        let sys = r#""database":"phxsys","tabela":"protecao""#;
        // Fora da guarda: nada.
        assert_eq!(
            t("inserir", r#"{"database":"b","tabela":"protecao"}"#),
            None
        );
        assert_eq!(
            t("atualizar", r#"{"database":"phxsys","tabela":"mensagens"}"#),
            None
        );
        assert_eq!(t("varrer", &format!("{{{sys}}}")), None);
        // Sobe.
        assert_eq!(
            t("excluir", &format!("{{{sys},\"rowid\":1}}")),
            Some(Toque::Sobe)
        );
        for v in [
            r#"{"modo":"proteger"}"#,
            r#"{"op":"x"}"#,
            r#"{"modo":null}"#,
            r#"{"MODO":"torto"}"#,
        ] {
            assert_eq!(
                t("atualizar", &format!("{{{sys},\"valores\":{v}}}")),
                Some(Toque::Sobe),
                "{v}"
            );
        }
        // Baixa, ou nao se sabe.
        for (op, resto) in [
            ("atualizar", r#""valores":{"modo":"observar"}"#),
            ("atualizar", r#""linha":{"Modo":" desligado"}"#),
            (
                "atualizar",
                r#""valores":{"modo":"proteger"},"linha":{"modo":"observar"}"#,
            ),
            ("atualizar", r#""valores":{"modo":1}"#),
            ("atualizar", r#""valores":"x""#),
            (
                "inserir",
                r#""valores":{"modo":"proteger"},"atualizar":{"modo":"observar"}"#,
            ),
            ("inserir_lote", r#""linhas":[]"#),
            ("restaurar", r#""rowid":1"#),
            ("excluir_tabela", r#""confirmar":"protecao""#),
        ] {
            assert_eq!(
                t(op, &format!("{{{sys},{resto}}}")),
                Some(Toque::PodeBaixar),
                "{op} {resto}"
            );
        }
        assert_eq!(
            t(
                "atualizar",
                r#"{"database":" PhxSys ","tabela":"PROTECAO","valores":{"modo":"observar"}}"#
            ),
            Some(Toque::PodeBaixar)
        );
        assert_eq!(
            t(
                "copiar_tabela",
                r#"{"database":"b","tabela":"protecao","destino_database":"phxsys"}"#
            ),
            Some(Toque::PodeBaixar)
        );
        assert_eq!(
            t(
                "renomear_tabela",
                r#"{"database":"phxsys","tabela":"x","destino":"protecao"}"#
            ),
            Some(Toque::PodeBaixar)
        );
        assert_eq!(
            t("restaurar_backup", r#"{"database":"phxsys","origem":"/b"}"#),
            Some(Toque::PodeBaixar)
        );
        // O `aplicar` nao e da guarda: o papel ja o recusa fora da replica.
        assert_eq!(
            t(
                "aplicar",
                r#"{"database":"phxsys","tabela":"protecao","eventos":[]}"#
            ),
            None
        );
    }
}

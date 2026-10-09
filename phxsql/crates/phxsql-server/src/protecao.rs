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
}

impl Categoria {
    pub fn nome(self) -> &'static str {
        match self {
            Categoria::DestruirEstrutura => "destruir_estrutura",
            Categoria::ApagarEmMassa => "apagar_em_massa",
            Categoria::AlterarEmMassa => "alterar_em_massa",
            Categoria::ReescreverTabelaGrande => "reescrever_tabela_grande",
            Categoria::Acesso => "acesso",
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
}

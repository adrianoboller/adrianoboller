//! As rotas HTTP do `phxclaw servir`, numa tabela que a CLI le (`phxclaw api rotas` e
//! `phxclaw api METODO ROTA`).
//!
//! Por que uma tabela e nao a lista do proprio `Router`: o axum nao enumera as rotas que
//! montou. A tabela e a segunda descricao delas, e por isso o teste
//! `toda_rota_do_fonte_esta_na_tabela` le o fonte dos routers (`api::servidor`, os
//! `rotas()` que ele junta e o `gatilhos::router` que o `servir` junta) e cruza com esta
//! lista nos dois sentidos: rota nova sem linha aqui reprova, linha sem rota tambem.
//!
//! A CLI nao reimplementa rota nenhuma: `phxclaw api` faz o pedido HTTP ao servidor que
//! esta de pe, com o token local, e quem responde e o MESMO handler, atras do MESMO portao
//! do RBAC. `equivalente` diz o comando local que chama as mesmas funcoes sem servidor,
//! quando existe; e informacao, nao segunda porta.

/// Como a rota se alcanca pela CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forma {
    /// Pedido e resposta HTTP: `phxclaw api METODO ROTA`.
    Http,
    /// Pagina ou arquivo da tela; `phxclaw api GET` busca o conteudo, mas o uso e o
    /// navegador.
    Tela,
    /// WebSocket: pedido e resposta nao descrevem a rota, e a CLI nao abre o fio.
    WebSocket(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct Rota {
    pub metodo: &'static str,
    /// O molde do axum: `{id}` e um segmento, `{*resto}` o resto do caminho.
    pub caminho: &'static str,
    pub resumo: &'static str,
    /// Consulta (`?k=v`) e corpo, como o handler os le; vazio quando nao le nenhum.
    pub parametros: &'static str,
    /// Comando local que chama as mesmas funcoes, quando existe.
    pub equivalente: Option<&'static str>,
    pub forma: Forma,
}

const fn r(
    metodo: &'static str,
    caminho: &'static str,
    resumo: &'static str,
    parametros: &'static str,
    equivalente: Option<&'static str>,
) -> Rota {
    Rota {
        metodo,
        caminho,
        resumo,
        parametros,
        equivalente,
        forma: Forma::Http,
    }
}

const fn tela(caminho: &'static str, resumo: &'static str) -> Rota {
    Rota {
        metodo: "GET",
        caminho,
        resumo,
        parametros: "",
        equivalente: None,
        forma: Forma::Tela,
    }
}

pub const ROTAS: &[Rota] = &[
    // --- api::router ---
    r("GET", "/health", "Saude do servidor", "", None),
    r(
        "GET",
        "/metrics",
        "Exposicao Prometheus (so com api.metricas)",
        "",
        None,
    ),
    r(
        "POST",
        "/v1/tasks",
        "Cria uma tarefa (roda em segundo plano no servidor)",
        "corpo: {objective, model?, plan_only?, images?, webhook?, verificar?, ...}",
        None,
    ),
    r("GET", "/v1/tasks", "Lista as tarefas", "", None),
    r(
        "GET",
        "/v1/tasks/{id}",
        "Uma tarefa inteira, com os passos",
        "",
        None,
    ),
    r(
        "POST",
        "/v1/tasks/{id}/plan",
        "Edita o plano de uma tarefa esperando aprovacao",
        "corpo: {steps: [texto]}",
        None,
    ),
    r(
        "POST",
        "/v1/tasks/{id}/approve",
        "Aprova o plano",
        "corpo: {}",
        None,
    ),
    r(
        "POST",
        "/v1/tasks/{id}/cancel",
        "Cancela a tarefa",
        "corpo: {}",
        None,
    ),
    r(
        "POST",
        "/v1/tasks/{id}/answer",
        "Responde a pergunta do agente",
        "corpo: {answer}",
        None,
    ),
    r(
        "GET",
        "/v1/tasks/{id}/artifacts/{*path}",
        "Bytes de um artefato da tarefa",
        "",
        None,
    ),
    r(
        "POST",
        "/v1/schedules",
        "Agenda uma tarefa ou fluxo",
        "corpo: {name, objective | fluxo, cron | every_seconds}",
        Some("agenda adicionar"),
    ),
    r(
        "GET",
        "/v1/schedules",
        "A agenda",
        "",
        Some("agenda listar"),
    ),
    tela("/sites/{id}/{*path}", "Site publicado por publish_site"),
    // --- canvas::rotas ---
    tela("/canvas/{id}/{nome}", "Pagina hospedeira de um canvas"),
    tela("/canvas/{id}/{nome}/widget", "O widget de um canvas"),
    // --- config::rotas ---
    r(
        "GET",
        "/v1/config",
        "Valor efetivo e origem de cada chave",
        "",
        Some("config mostrar --json"),
    ),
    r(
        "PUT",
        "/v1/config",
        "Grava chaves do config.json (If-Match: revisao)",
        "corpo: {valores: {chave: valor}, escopo?}",
        Some("config definir"),
    ),
    r(
        "GET",
        "/v1/config/perfis",
        "Perfis do config.json",
        "",
        Some("config perfil listar"),
    ),
    r(
        "PUT",
        "/v1/config/perfis",
        "Cria, usa ou desliga um perfil",
        "corpo: {acao, nome?}",
        Some("config perfil"),
    ),
    // --- sincronizar::rotas ---
    r(
        "GET",
        "/v1/config/sincronizar",
        "Retrato do config.json para sincronizar pela ponte",
        "",
        Some("config sincronizar receber"),
    ),
    r(
        "PUT",
        "/v1/config/sincronizar",
        "Recebe o config.json da outra ponta",
        "corpo: o retrato",
        Some("config sincronizar enviar"),
    ),
    // --- pwa::rotas ---
    tela("/", "A tela (PWA)"),
    tela("/{a}", "index.html, manifest.webmanifest e sw.js da tela"),
    tela("/assets/{*resto}", "Arquivos da tela"),
    r(
        "GET",
        "/ui/politica",
        "Politica da tela (bloquear_inspecao); publica, antes do login",
        "",
        None,
    ),
    // --- tunel::rotas ---
    r(
        "POST",
        "/v1/tunel/terminal",
        "Uma linha de comando no terminal do projeto",
        "corpo: {comando}",
        None,
    ),
    r(
        "POST",
        "/v1/tunel/lsp",
        "Consulta ao servidor de linguagem do projeto",
        "corpo: o pedido LSP",
        None,
    ),
    // --- ide::rotas ---
    Rota {
        metodo: "GET",
        caminho: "/v1/ide/terminal",
        resumo: "Terminal do Helix no navegador",
        parametros: "",
        equivalente: None,
        forma: Forma::WebSocket(
            "fio de terminal interativo; no terminal local, rode o editor direto",
        ),
    },
    r(
        "GET",
        "/v1/ide/simbolos",
        "Simbolos de um arquivo do projeto",
        "consulta: arquivo, prazo?",
        None,
    ),
    r(
        "GET",
        "/v1/ide/arquivo",
        "Conteudo de um arquivo do projeto",
        "consulta: caminho",
        None,
    ),
    r(
        "GET",
        "/v1/ide/dobras",
        "Texto e regioes dobraveis de um arquivo do projeto (LSP, chaves ou indentacao)",
        "consulta: arquivo, prazo?",
        None,
    ),
    r(
        "POST",
        "/v1/ide/compartilhar",
        "Compartilha o terminal do IDE aberto (convite de leitura, expira, teto de convidados)",
        "corpo: {expira_em_s?, max_convidados?, escrita?}",
        None,
    ),
    r(
        "POST",
        "/v1/ide/compartilhar/revogar",
        "Revoga um convidado ou o compartilhamento inteiro",
        "corpo: {convidado?}",
        None,
    ),
    Rota {
        metodo: "GET",
        caminho: "/v1/ide/compartilhado",
        resumo: "Terminal compartilhado, lado do convidado",
        parametros: "",
        equivalente: None,
        forma: Forma::WebSocket("fio do convidado: o convite vai na primeira mensagem"),
    },
    r(
        "POST",
        "/v1/ide/completar",
        "Completacao de codigo pelo modelo",
        "corpo: {antes, depois?, arquivo?, linguagem?}",
        None,
    ),
    r(
        "GET",
        "/v1/ide/testes",
        "Arvore de testes do projeto",
        "consulta: caminho?, linguagem?",
        Some("testes listar"),
    ),
    r(
        "POST",
        "/v1/ide/testes/rodar",
        "Roda um no da arvore de testes",
        "corpo: {no, caminho?, linguagem?}",
        Some("testes rodar"),
    ),
    r(
        "GET",
        "/v1/plugins/catalogo",
        "Catalogo da loja de pacotes",
        "consulta: busca?",
        Some("plugins catalogo"),
    ),
    r(
        "POST",
        "/v1/plugins/instalar",
        "Instala um pacote assinado da loja",
        "corpo: {nome}",
        Some("plugins instalar"),
    ),
    // --- mcp::rotas ---
    r(
        "POST",
        "/mcp",
        "MCP por streamable HTTP (JSON-RPC)",
        "corpo: a mensagem JSON-RPC; cabecalho mcp-session-id",
        Some("mcp-serve"),
    ),
    r("GET", "/mcp", "405: o servidor nao inicia fluxo", "", None),
    r("DELETE", "/mcp", "Fecha a sessao MCP (204)", "", None),
    // --- fluxos_tela::rotas ---
    r("GET", "/v1/fluxos", "Os fluxos da pasta", "", None),
    r(
        "GET",
        "/v1/fluxos/arquivo",
        "Le um fluxo",
        "consulta: nome",
        None,
    ),
    r(
        "PUT",
        "/v1/fluxos/arquivo",
        "Grava um fluxo",
        "consulta: nome; corpo: {texto}",
        None,
    ),
    r(
        "POST",
        "/v1/fluxos/validar",
        "Valida um fluxo sem gravar",
        "corpo: {texto}",
        None,
    ),
    r(
        "POST",
        "/v1/fluxos/rodar",
        "Roda um fluxo (com --ate)",
        "corpo: {nome, ate?, orcamento?}",
        None,
    ),
    r(
        "POST",
        "/v1/fluxos/assistente",
        "Monta um fluxo pela descricao e grava rascunho",
        "corpo: {descricao, tentativas?}",
        None,
    ),
    // --- insights::rotas ---
    r(
        "GET",
        "/v1/insights",
        "Insights: estado, p50/p95, custo, falhas, por fluxo e dia",
        "consulta: periodo (24h|7d|30d|tudo), fluxo",
        None,
    ),
    // --- voz_rest::rotas ---
    r(
        "POST",
        "/v1/voz/transcrever",
        "Transcreve (STT) o audio do microfone da Conversa",
        "corpo: audio cru (audio/webm;codecs=opus ou audio/wav)",
        None,
    ),
    // --- hardware::rotas ---
    r(
        "GET",
        "/v1/hardware",
        "Monitor de hardware: CPU, carga, memoria, swap, disco, uptime, temperatura, saude",
        "consulta: nenhum",
        Some("hardware"),
    ),
    // --- gatilhos::router (juntado pelo `servir`) ---
    r(
        "POST",
        "/v1/triggers/{nome}",
        "Dispara o gatilho de webhook de um fluxo",
        "corpo: o evento; credencial do gatilho",
        None,
    ),
    tela(
        "/v1/triggers/{nome}",
        "Formulario do gatilho, quando o fluxo declara um",
    ),
    r(
        "POST",
        "/v1/flows/{tarefa}/resume",
        "Retoma um fluxo parado numa espera de webhook",
        "consulta: a credencial da espera; corpo: a resposta",
        Some("fluxo responder"),
    ),
];

/// A rota da tabela que atende `metodo caminho` (caminho sem a consulta).
pub fn achar(metodo: &str, caminho: &str) -> Option<&'static Rota> {
    let caminho = caminho.split('?').next().unwrap_or(caminho);
    ROTAS
        .iter()
        .filter(|r| r.metodo.eq_ignore_ascii_case(metodo))
        .find(|r| casa(r.caminho, caminho))
}

/// O caminho casa com o molde: `{x}` e um segmento nao vazio, `{*x}` um ou mais.
pub fn casa(molde: &str, caminho: &str) -> bool {
    let m: Vec<&str> = molde.trim_start_matches('/').split('/').collect();
    let c: Vec<&str> = caminho.trim_start_matches('/').split('/').collect();
    for (i, pm) in m.iter().enumerate() {
        if pm.starts_with("{*") {
            return c.len() > i && c[i..].iter().all(|s| !s.is_empty());
        }
        let Some(pc) = c.get(i) else { return false };
        if pm.starts_with('{') {
            if pc.is_empty() {
                return false;
            }
        } else if pm != pc {
            return false;
        }
    }
    m.len() == c.len()
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::Path;

    fn fonte(arq: &str) -> String {
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(arq))
            .unwrap()
    }

    /// O corpo da funcao `assinatura` (do `{` ate o `}` que fecha), no texto `t`.
    fn corpo_de<'a>(t: &'a str, assinatura: &str) -> &'a str {
        let i = t
            .find(assinatura)
            .unwrap_or_else(|| panic!("sem {assinatura}"));
        let ab = i + t[i..].find('{').unwrap();
        let mut nivel = 0;
        for (j, ch) in t[ab..].char_indices() {
            match ch {
                '{' => nivel += 1,
                '}' => {
                    nivel -= 1;
                    if nivel == 0 {
                        return &t[ab..ab + j];
                    }
                }
                _ => {}
            }
        }
        panic!("corpo de {assinatura} sem fim")
    }

    /// O texto entre o `(` em `ini` e o `)` que o fecha.
    fn parenteses(t: &str, ini: usize) -> &str {
        let mut nivel = 0;
        for (j, ch) in t[ini..].char_indices() {
            match ch {
                '(' => nivel += 1,
                ')' => {
                    nivel -= 1;
                    if nivel == 0 {
                        return &t[ini + 1..ini + j];
                    }
                }
                _ => {}
            }
        }
        panic!("parentese sem fim")
    }

    /// `(METODO, caminho)` de cada `.route(...)` de um corpo, e os modulos que ele junta.
    fn rotas_de(arq: &str, assinatura: &str, saida: &mut BTreeSet<(String, String)>) {
        let t = fonte(arq);
        let corpo = corpo_de(&t, assinatura);
        let mut resto = corpo;
        while let Some(i) = resto.find("route(") {
            let args = parenteses(resto, i + "route".len());
            let (caminho, metodos) = args.split_once(',').expect("route sem metodo");
            let caminho = caminho.trim();
            let caminho = if let Some(l) = caminho.strip_prefix('"') {
                l.trim_end_matches('"').to_string()
            } else if let Some(f) = caminho.strip_prefix("&format!(\"") {
                f.trim_end_matches(")").trim_end_matches('"').to_string()
            } else {
                // Constante do proprio arquivo: `const ROTA_X: &str = "...";`.
                let c = caminho.trim();
                let decl = format!("const {c}: &str = \"");
                let i = t.find(&decl).unwrap_or_else(|| panic!("{arq}: const {c}"));
                let v = &t[i + decl.len()..];
                v[..v.find('"').unwrap()].to_string()
            };
            // Os metodos de primeiro nivel: `get(`, `.post(`... fora de parenteses internos.
            let mut nivel = 0;
            let mut palavra = String::new();
            for ch in metodos.chars() {
                match ch {
                    '(' => {
                        if nivel == 0
                            && let m @ ("get" | "post" | "put" | "delete" | "patch" | "any") =
                                palavra.trim_start_matches('.').trim()
                        {
                            saida.insert((m.to_ascii_uppercase(), caminho.clone()));
                        }
                        nivel += 1;
                        palavra.clear();
                    }
                    ')' => {
                        nivel -= 1;
                        palavra.clear();
                    }
                    c if nivel == 0 && (c.is_alphanumeric() || c == '_') => palavra.push(c),
                    _ if nivel == 0 => palavra.clear(),
                    _ => {}
                }
            }
            resto = &resto[i + "route(".len() + args.len()..];
        }
        let mut r = corpo;
        while let Some(i) = r.find(".merge(crate::") {
            let depois = &r[i + ".merge(crate::".len()..];
            let modulo = &depois[..depois.find("::").unwrap()];
            rotas_de(&format!("{modulo}.rs"), "pub fn rotas", saida);
            r = depois;
        }
    }

    fn do_fonte() -> BTreeSet<(String, String)> {
        let mut s = BTreeSet::new();
        rotas_de("api.rs", "pub fn servidor(state: ApiState", &mut s);
        // O `servir` junta o router dos gatilhos ao da API (`main.rs`).
        rotas_de("gatilhos.rs", "pub fn router(api: ApiState", &mut s);
        s
    }

    #[test]
    fn toda_rota_do_fonte_esta_na_tabela() {
        let fonte = do_fonte();
        assert!(fonte.len() > 30, "leitura dos routers falhou: {fonte:?}");
        let tabela: BTreeSet<(String, String)> = ROTAS
            .iter()
            .map(|r| (r.metodo.to_string(), r.caminho.to_string()))
            .collect();
        assert_eq!(tabela.len(), ROTAS.len(), "linha repetida na tabela");
        let sem_linha: Vec<_> = fonte.difference(&tabela).collect();
        assert!(
            sem_linha.is_empty(),
            "rota no fonte sem linha em rotas::ROTAS (sem forma na CLI): {sem_linha:?}"
        );
        let sem_rota: Vec<_> = tabela.difference(&fonte).collect();
        assert!(
            sem_rota.is_empty(),
            "linha em rotas::ROTAS sem rota no fonte: {sem_rota:?}"
        );
    }

    #[test]
    fn o_molde_casa_so_o_que_o_axum_casaria() {
        assert!(casa("/v1/tasks/{id}", "/v1/tasks/abc"));
        assert!(!casa("/v1/tasks/{id}", "/v1/tasks/"));
        assert!(!casa("/v1/tasks/{id}", "/v1/tasks/a/b"));
        assert!(casa(
            "/v1/tasks/{id}/artifacts/{*path}",
            "/v1/tasks/a/artifacts/x/y.png"
        ));
        assert!(!casa(
            "/v1/tasks/{id}/artifacts/{*path}",
            "/v1/tasks/a/artifacts"
        ));
        assert_eq!(achar("get", "/v1/tasks?x=1").unwrap().caminho, "/v1/tasks");
        assert_eq!(achar("POST", "/v1/tasks/9/cancel").unwrap().metodo, "POST");
        assert!(achar("PATCH", "/v1/tasks").is_none());
    }
}

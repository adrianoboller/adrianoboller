//! Adaptador Bootstrap 5.3: o mesmo UI-IR, a mesma arvore do motor de marcacao (`html`),
//! com as classes de COMPONENTE do Bootstrap -- `btn`, `form-control`, `form-select`,
//! `form-label`, `input-group`, `card`, `table`, `nav-link`.
//!
//! O que este adaptador NAO faz, de proposito:
//! - **Nao decide coluna.** Nada de `container`/`row`/`col-*`: a grade sai do motor
//!   responsivo (`responsivo::css`), igual a do adaptador «phoenix». Se o Bootstrap
//!   decidisse, o mesmo IR teria duas grades, e a prova de fidelidade mediria o framework.
//! - **Nao usa a aparencia padrao.** Os tokens da casa (`html::tokens_css`) vao para as
//!   variaveis `--bs-*` e para as variaveis de componente (`--bs-btn-*`, `--bs-card-*`,
//!   `--bs-table-*`): a identidade e a nossa, o Bootstrap empresta o componente.
//! - **Nao carrega o JavaScript do Bootstrap.** O IR de hoje nao tem Dialog nem Tabs, e o
//!   JS dele manipula DOM que o nosso estado controla (a linha de item, a mascara). Quando
//!   entrar dialogo, entra pelo `<dialog>` nativo, nao pelo `bootstrap.bundle`.
//! - **Nao vai a CDN.** A folha e um arquivo LOCAL, com a versao no caminho
//!   (`CSS_PADRAO`); endereco com esquema (`https:`, `//`) e recusado: a tela gerada tem de
//!   abrir sem rede, e CDN e dependencia que muda sem commit.

use crate::html::{Adaptador, esc, pagina, tokens_css};
use crate::ir::App;
use crate::responsivo;

/// A versao fixada do Bootstrap que o HTML gerado pede.
pub const VERSAO: &str = "5.3.3";

/// Caminho padrao da folha, relativo ao `index.html` gerado: a versao vai no nome da pasta,
/// para que trocar de versao seja trocar de arquivo, nunca sobrescrever um em silencio.
pub const CSS_PADRAO: &str = "vendor/bootstrap-5.3.3/bootstrap.min.css";

/// Variante de acao da casa -> variante de botao do Bootstrap, sempre de CONTORNO
/// (`btn-outline-*`): o preenchimento so no hover, a mesma regra do «phoenix».
fn variante(acao: &str) -> &'static str {
    match acao {
        "include" => "success",
        "alter" => "warning",
        "delete" => "danger",
        _ => "primary",
    }
}

fn botao(acao: &str, _mini: bool) -> String {
    // sem `btn-sm` no mini: o botao pequeno do Bootstrap tem 31 px, abaixo do alvo de toque
    format!("btn btn-outline-{}", variante(acao))
}

const BOOTSTRAP: Adaptador = Adaptador {
    rotulo: "form-label",
    texto: "form-control",
    selecao: "form-select",
    marca: "form-check-input",
    grupo: "input-group",
    prefixo: "input-group-text",
    secao: "card card-body",
    legenda: "card-title",
    tabela: "table",
    obrig: "obrig",
    so_leitor: "visually-hidden",
    vazio: "vazio",
    menu: "nav-link",
    cartao: "card card-body",
    botao,
};

/// Recusa o que nao e arquivo local: esquema, `//`, e o que fecharia o atributo.
pub fn conferir_caminho(css: &str) -> Result<(), String> {
    let c = css.trim();
    let esquema = c.split_once(':').is_some_and(|(e, _)| {
        !e.is_empty()
            && e.chars()
                .all(|x| x.is_ascii_alphanumeric() || "+-.".contains(x))
    });
    if c.is_empty() || c.starts_with("//") || esquema || c.contains(['"', '<', '>', '\n']) {
        return Err(format!(
            "folha do Bootstrap «{css}» recusada: use um caminho de arquivo LOCAL (ex.: {CSS_PADRAO}), nunca CDN"
        ));
    }
    Ok(())
}

/// Tokens da casa -> variaveis do Bootstrap. Vem DEPOIS da folha dele, e `:root` ganha do
/// `:root,[data-bs-theme=light]` dele pela ordem.
const MAPA: &str = r#"
:root{--bs-body-bg:var(--fundo);--bs-body-color:var(--texto);--bs-emphasis-color:var(--texto);--bs-border-color:var(--borda);
--bs-secondary-color:var(--fraco);--bs-secondary-bg:var(--painel);--bs-tertiary-bg:var(--painel);
--bs-primary:var(--consulta);--bs-success:var(--inclui);--bs-warning:var(--altera);--bs-danger:var(--exclui);
--bs-link-color:var(--consulta);--bs-focus-ring-color:var(--foco);--bs-body-font-size:var(--fonte);
--bs-body-font-family:system-ui,-apple-system,"Segoe UI",sans-serif;--bs-border-radius:6px}
body{background:var(--fundo);color:var(--texto);font-size:var(--fonte)}
.card{--bs-card-bg:var(--painel);--bs-card-color:var(--texto);--bs-card-border-color:var(--borda);margin-block-end:14px}
fieldset.card{display:block}
.card-body{--bs-card-spacer-y:12px;--bs-card-spacer-x:14px}
fieldset.card>legend{float:none;inline-size:auto;font-size:var(--fonte-rotulo);color:var(--fraco);padding-inline:6px;margin:0}
.form-label{font-size:var(--fonte-rotulo);color:var(--fraco);margin-block-end:0}.obrig{color:var(--exclui)}
.form-control,.form-select,.input-group-text{background-color:var(--fundo);color:var(--texto);border-color:var(--borda);min-block-size:var(--alvo)}
.form-check-input{inline-size:var(--marca);block-size:var(--marca);margin:0}
.input-group.phx-junto{gap:0}
.btn{min-block-size:var(--alvo);min-inline-size:var(--alvo);display:inline-flex;align-items:center;justify-content:center}
.btn-outline-success{--bs-btn-color:var(--inclui);--bs-btn-border-color:var(--inclui);--bs-btn-hover-bg:var(--inclui);--bs-btn-hover-border-color:var(--inclui);--bs-btn-hover-color:var(--fundo);--bs-btn-active-bg:var(--inclui);--bs-btn-active-color:var(--fundo)}
.btn-outline-warning{--bs-btn-color:var(--altera);--bs-btn-border-color:var(--altera);--bs-btn-hover-bg:var(--altera);--bs-btn-hover-border-color:var(--altera);--bs-btn-hover-color:var(--fundo);--bs-btn-active-bg:var(--altera);--bs-btn-active-color:var(--fundo)}
.btn-outline-danger{--bs-btn-color:var(--exclui);--bs-btn-border-color:var(--exclui);--bs-btn-hover-bg:var(--exclui);--bs-btn-hover-border-color:var(--exclui);--bs-btn-hover-color:var(--fundo);--bs-btn-active-bg:var(--exclui);--bs-btn-active-color:var(--fundo)}
.btn-outline-primary{--bs-btn-color:var(--consulta);--bs-btn-border-color:var(--consulta);--bs-btn-hover-bg:var(--consulta);--bs-btn-hover-border-color:var(--consulta);--bs-btn-hover-color:var(--fundo);--bs-btn-active-bg:var(--consulta);--bs-btn-active-color:var(--fundo)}
.table{--bs-table-bg:transparent;--bs-table-color:var(--texto);--bs-table-border-color:var(--borda);margin:0}
.table th{font-size:12px;color:var(--fraco);white-space:nowrap}.table td input,.table td select{min-inline-size:110px}
.phx-tabela{border:1px solid var(--borda);border-radius:8px;margin-block:8px}.phx-tabela td[data-rotulo]::before{font-size:var(--fonte-rotulo);color:var(--fraco)}
.vazio{color:var(--fraco);text-align:center}
nav{background:var(--painel);padding:16px}nav .marca{font-weight:700;color:var(--ouro);font-size:18px;margin-bottom:8px}
nav h2{font-size:11px;letter-spacing:.12em;text-transform:uppercase;color:var(--fraco);margin:12px 0 6px}
.nav-link{display:flex;align-items:center;min-block-size:var(--alvo);min-inline-size:var(--alvo);padding:4px 10px;color:var(--texto);border-radius:6px}.nav-link[aria-current]{background:var(--borda)}
.rodape{margin-top:16px;font-size:11px;color:var(--fraco)}main{padding:0}.tela{padding:var(--respiro)}h1{font-size:var(--fonte-titulo)}
.acoes{margin-block:10px}.totais{margin-top:10px;column-gap:24px}.total{display:flex;flex-direction:column;align-items:flex-end}
.total span{font-size:12px;color:var(--fraco)}.total output{font-size:18px;font-weight:700}
.phx-cartao{border-radius:var(--phx-raio,12px);gap:8px}.phx-cartao h2{font-size:15px;margin:0}.phx-cartao p{margin:0}
.phx-sub{font-size:var(--fonte-rotulo);color:var(--fraco)}
.phx-sigla{display:grid;place-items:center;inline-size:40px;block-size:40px;border-radius:10px;background:var(--borda);font-weight:700}
.phx-caps{list-style:none;margin:0;padding:0}.phx-caps li{font-size:12px;border:1px solid var(--borda);border-radius:6px;padding:2px 6px}
.phx-cartao summary{min-block-size:var(--alvo);display:flex;align-items:center;cursor:pointer;color:var(--phx-acento,var(--consulta))}
"#;

/// O HTML Bootstrap do app, pedindo a folha em `css` (caminho local, conferido), na versao
/// fixada desta casa.
pub fn render(app: &App, css: &str) -> Result<String, String> {
    render_com_versao(app, css, VERSAO)
}

/// O mesmo, com a versao que o projeto fixou (o `adapterVersion` do PHX JSON). A versao vai
/// para o atributo e para o rodape: quem abre a tela sabe qual folha ela pediu.
pub fn render_com_versao(app: &App, css: &str, versao: &str) -> Result<String, String> {
    conferir_caminho(css)?;
    if versao.is_empty() || !versao.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err(format!(
            "versao do Bootstrap «{versao}» recusada (so digitos e ponto)"
        ));
    }
    let cabeca = format!(
        "<link rel=\"stylesheet\" href=\"{}\" data-phx-bootstrap=\"{versao}\"><style>{}{}{MAPA}</style>",
        esc(css.trim()),
        tokens_css(app),
        responsivo::css(app)
    );
    Ok(pagina(
        app,
        &BOOTSTRAP,
        &cabeca,
        &format!("Protótipo Bootstrap {versao} gerado do UI-IR"),
    ))
}

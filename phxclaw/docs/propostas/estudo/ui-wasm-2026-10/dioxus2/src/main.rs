use dioxus::prelude::*;
#[path = "../../comum.rs"] mod comum; use comum::*;
fn main() { dioxus::launch(App); }
#[component]
fn App() -> Element {
    let linhas = use_hook(|| serde_json::from_str::<Vec<Linha>>(DADOS).unwrap());
    let mut filtro = use_signal(String::new);
    let mut nome = use_signal(String::new);
    let mut msg = use_signal(String::new);
    let f = filtro.read().clone();
    rsx! {
        h1 { {t("titulo")} }
        input { aria_label: t("filtro"), value: "{filtro}", oninput: move |e| filtro.set(e.value()) }
        table { thead { tr { th { "ID" } th { {t("nome")} } th { {t("status")} } } }
            tbody { for x in linhas.iter().filter(|x| x.nome.contains(&f)) { tr { key: "{x.id}", td { "{x.id}" } td { "{x.nome}" } td { "{x.status}" } } } } }
        form { onsubmit: move |e| { e.prevent_default(); if nome.read().trim().is_empty() { msg.set(t("obrigatorio").into()) } else { msg.set(t("salvo").into()) } },
            label { {t("nome")} input { value: "{nome}", oninput: move |e| nome.set(e.value()) } }
            label { {t("email")} input { r#type: "email" } }
            label { {t("fone")} input {} }
            label { {t("cidade")} input {} }
            label { {t("uf")} input {} }
            label { {t("doc")} input {} }
            button { r#type: "submit", {t("salvar")} }
            p { role: "status", "{msg}" }
        }
    }
}

use leptos::prelude::*;
#[path = "../comum2.rs"] mod comum; use comum::*;
#[component]
fn App() -> impl IntoView {
    let linhas = StoredValue::new((1..=50u32).map(|i| Linha{id:i,nome:format!("Cliente {i:03}"),status:["ativo","inativo","pendente"][(i%3) as usize].into()}).collect::<Vec<_>>());
    let (filtro, set_filtro) = signal(String::new());
    let (nome, set_nome) = signal(String::new());
    let (msg, set_msg) = signal(String::new());
    view! {
        <h1>{t("titulo")}</h1>
        <input aria-label=t("filtro") prop:value=filtro on:input=move |e| set_filtro.set(event_target_value(&e)) />
        <table><thead><tr><th>"ID"</th><th>{t("nome")}</th><th>{t("status")}</th></tr></thead>
        <tbody>{move || { let f = filtro.get(); linhas.with_value(|l| l.iter().filter(|x| x.nome.contains(&f)).map(|x| view!{<tr><td>{x.id}</td><td>{x.nome.clone()}</td><td>{x.status.clone()}</td></tr>}).collect_view()) }}</tbody></table>
        <form on:submit=move |ev| { ev.prevent_default(); if nome.get().trim().is_empty() { set_msg.set(t("obrigatorio").into()) } else { set_msg.set(t("salvo").into()) } }>
            <label>{t("nome")}<input prop:value=nome on:input=move |e| set_nome.set(event_target_value(&e)) /></label>
            <label>{t("email")}<input type="email" /></label>
            <label>{t("fone")}<input /></label>
            <label>{t("cidade")}<input /></label>
            <label>{t("uf")}<input /></label>
            <label>{t("doc")}<input /></label>
            <button type="submit">{t("salvar")}</button>
            <p role="status">{msg}</p>
        </form>
    }
}
fn main() { leptos::mount::mount_to_body(App) }

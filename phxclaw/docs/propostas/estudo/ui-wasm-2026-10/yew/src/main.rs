use yew::prelude::*;
use web_sys::HtmlInputElement;
#[path = "../../comum.rs"] mod comum; use comum::*;
#[function_component]
fn App() -> Html {
    let linhas = use_memo((), |_| serde_json::from_str::<Vec<Linha>>(DADOS).unwrap());
    let filtro = use_state(String::new);
    let nome = use_state(String::new);
    let msg = use_state(String::new);
    let on_filtro = { let s = filtro.clone(); Callback::from(move |e: InputEvent| { let i: HtmlInputElement = e.target_unchecked_into(); s.set(i.value()); }) };
    let on_nome = { let s = nome.clone(); Callback::from(move |e: InputEvent| { let i: HtmlInputElement = e.target_unchecked_into(); s.set(i.value()); }) };
    let on_submit = { let n = nome.clone(); let m = msg.clone(); Callback::from(move |e: SubmitEvent| { e.prevent_default(); if n.trim().is_empty() { m.set(t("obrigatorio").into()) } else { m.set(t("salvo").into()) } }) };
    html! { <>
        <h1>{t("titulo")}</h1>
        <input aria-label={t("filtro")} value={(*filtro).clone()} oninput={on_filtro} />
        <table><thead><tr><th>{"ID"}</th><th>{t("nome")}</th><th>{t("status")}</th></tr></thead>
        <tbody>{ for linhas.iter().filter(|x| x.nome.contains(&*filtro)).map(|x| html!{<tr key={x.id}><td>{x.id}</td><td>{x.nome.clone()}</td><td>{x.status.clone()}</td></tr>}) }</tbody></table>
        <form onsubmit={on_submit}>
            <label>{t("nome")}<input value={(*nome).clone()} oninput={on_nome} /></label>
            <label>{t("email")}<input type="email" /></label>
            <label>{t("fone")}<input /></label>
            <label>{t("cidade")}<input /></label>
            <label>{t("uf")}<input /></label>
            <label>{t("doc")}<input /></label>
            <button type="submit">{t("salvar")}</button>
            <p role="status">{(*msg).clone()}</p>
        </form>
    </> }
}
fn main() { yew::Renderer::<App>::new().render(); }

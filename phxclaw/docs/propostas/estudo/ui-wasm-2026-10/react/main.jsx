import { useState, useMemo } from 'react';
import { createRoot } from 'react-dom/client';
import DADOS from '../dados.json';
const T = {titulo:"Clientes",filtro:"Filtrar",nome:"Nome",status:"Situacao",email:"E-mail",fone:"Telefone",cidade:"Cidade",uf:"UF",doc:"Documento",salvar:"Salvar",obrigatorio:"Nome e obrigatorio",salvo:"Salvo"};
const t = k => T[k] ?? "?";
function App() {
  const linhas = useMemo(() => DADOS, []);
  const [filtro, setFiltro] = useState(""); const [nome, setNome] = useState(""); const [msg, setMsg] = useState("");
  return <>
    <h1>{t("titulo")}</h1>
    <input aria-label={t("filtro")} value={filtro} onInput={e => setFiltro(e.target.value)} onChange={()=>{}} />
    <table><thead><tr><th>ID</th><th>{t("nome")}</th><th>{t("status")}</th></tr></thead>
    <tbody>{linhas.filter(x => x.nome.includes(filtro)).map(x => <tr key={x.id}><td>{x.id}</td><td>{x.nome}</td><td>{x.status}</td></tr>)}</tbody></table>
    <form onSubmit={e => { e.preventDefault(); setMsg(nome.trim() ? t("salvo") : t("obrigatorio")); }}>
      <label>{t("nome")}<input value={nome} onChange={e => setNome(e.target.value)} /></label>
      <label>{t("email")}<input type="email" /></label><label>{t("fone")}<input /></label>
      <label>{t("cidade")}<input /></label><label>{t("uf")}<input /></label><label>{t("doc")}<input /></label>
      <button type="submit">{t("salvar")}</button><p role="status">{msg}</p>
    </form></>;
}
createRoot(document.body.appendChild(document.createElement('div'))).render(<App />);

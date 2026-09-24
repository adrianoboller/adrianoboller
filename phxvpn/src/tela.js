"use strict";
const $ = (id) => document.getElementById(id);
let sessao = null;
try { sessao = JSON.parse(sessionStorage.getItem("phxvpn") || "null"); } catch (_) {}

async function api(metodo, rota, corpo) {
  const cab = { "Content-Type": "application/json" };
  if (sessao) cab.Authorization = "Bearer " + sessao.token;
  const r = await fetch(rota, { method: metodo, headers: cab, body: corpo ? JSON.stringify(corpo) : undefined });
  const j = await r.json().catch(() => ({ erro: "resposta ilegível do painel" }));
  if (r.status === 401 && sessao && rota !== "/api/login") { sair(); }
  if (!r.ok) throw new Error(j.erro || ("erro " + r.status));
  return j;
}
function msg(id, texto, ok) { const m = $(id); m.textContent = texto || ""; m.className = "msg " + (ok ? "ok" : "erro"); }
function dados(form) { return Object.fromEntries(new FormData(form).entries()); }
function mostrar(tela) { for (const t of ["tela-instalar","tela-login","tela-redes","tela-admin","tela-mfa"]) $(t).hidden = t !== tela; $("barra").hidden = !sessao; }
function baixar(nome, texto) {
  const a = document.createElement("a");
  a.href = URL.createObjectURL(new Blob([texto], { type: "application/x-openvpn-profile" }));
  a.download = nome; document.body.appendChild(a); a.click(); a.remove();
}
function sair() { if (sessao) api("POST", "/api/sair").catch(() => {}); sessao = null; try { sessionStorage.removeItem("phxvpn"); } catch (_) {} iniciar(); }

async function iniciar() {
  let e;
  try { e = await api("GET", "/api/estado"); } catch (x) { document.body.textContent = "Painel indisponível: " + x.message; return; }
  $("empresa-nome").textContent = e.empresa ? e.empresa.nome : "";
  if (!e.instalado) return mostrar("tela-instalar");
  if (!sessao) return mostrar("tela-login");
  $("quem").textContent = sessao.login + (sessao.admin ? " · admin" : "");
  $("b-admin").hidden = !sessao.admin;
  $("selo-cofre").hidden = e.destrancado;
  $("cartao-cofre").hidden = e.destrancado || !sessao.admin;
  mostrar("tela-redes");
  carregarRedes();
}

async function carregarRedes() {
  const caixa = $("lista-redes"); caixa.textContent = "";
  let redes;
  try { redes = await api("GET", "/api/redes"); } catch (x) { return msg("m-redes", x.message); }
  if (!redes.length) { const v = document.createElement("div"); v.className = "vazio"; v.textContent = "Nenhuma rede ainda. Crie uma ou entre numa existente."; caixa.appendChild(v); return; }
  for (const r of redes) {
    const bloco = document.createElement("div"); bloco.className = "rede";
    const topo = document.createElement("div"); topo.className = "topo";
    const nome = document.createElement("span"); nome.className = "nome"; nome.textContent = r.nome;
    const info = document.createElement("span"); info.className = "info";
    info.textContent = `${r.subrede} · porta ${r.porta} · ${r.membros} membro(s) · servidor ${r.servidor}` + (r.meu_ip ? ` · meu IP ${r.meu_ip}` : "") + (r.finalidade ? ` · ${r.finalidade}` : "") + (r.exige_mfa ? " · exige autenticador" : "");
    const acoes = document.createElement("span"); acoes.className = "acoes";
    if (sessao.admin || r.dono === sessao.login) {
      const b = document.createElement("button"); b.className = "altera";
      b.textContent = r.exige_mfa ? "Dispensar autenticador" : "Exigir autenticador";
      b.onclick = async () => {
        const frase = r.exige_mfa ? `Dispensar o autenticador na rede «${r.nome}»?` : `Exigir usuário, senha e código do autenticador para conectar na rede «${r.nome}»? Os membros precisam baixar o perfil de novo.`;
        if (!confirm(frase)) return;
        // Quem tem autenticador prova o código para mudar a exigência.
        let codigo = "";
        if (sessao.mfa) { codigo = prompt("Código do autenticador") || ""; if (!codigo) return; }
        try { const x = await api("POST", "/api/redes/mfa", { rede_id: r.id, exige: !r.exige_mfa, codigo }); msg("m-redes", x.aviso, true); carregarRedes(); } catch (x) { msg("m-redes", x.message); }
      };
      acoes.appendChild(b);
    }
    if (r.meu_ip) {
      const b = document.createElement("button"); b.className = "exclui"; b.textContent = "Sair da rede";
      b.onclick = async () => { if (!confirm(`Sair da rede «${r.nome}»?`)) return; try { await api("POST", "/api/redes/sair", { rede_id: r.id }); carregarRedes(); } catch (x) { msg("m-redes", x.message); } };
      acoes.appendChild(b);
    }
    topo.append(nome, info, acoes); bloco.appendChild(topo);
    try {
      const membros = await api("POST", "/api/redes/membros", { rede_id: r.id });
      for (const m of membros) {
        const l = document.createElement("div"); l.className = "membro";
        const p = document.createElement("span"); p.className = "ponto" + (m.online ? " on" : ""); p.title = m.online ? "conectado" : "desconectado";
        const n = document.createElement("span"); n.textContent = m.login;
        const ip = document.createElement("span"); ip.className = "ip"; ip.textContent = m.ip;
        l.append(p, n, ip); bloco.appendChild(l);
      }
    } catch (_) {}
    caixa.appendChild(bloco);
  }
}

let modo = "criar";
// O protocolo e do SERVIDOR (so faz sentido ao criar); o proxy vale nos dois
// -- ele so muda o perfil .ovpn de quem baixa, nunca a rede em si.
function ajustarProxyPeloProtocolo() {
  if (modo !== "criar") { $("c-proxy").hidden = false; return; }
  const esconde = $("c-protocolo").value !== "tcp";
  $("c-proxy").hidden = esconde;
  // Campo escondido nao pode mandar um proxy que o UDP recusaria.
  if (esconde) $("c-proxy").querySelector("input").value = "";
}
$("c-protocolo").onchange = ajustarProxyPeloProtocolo;
function abrirDialogo(m) {
  modo = m; $("f-rede").reset(); msg("m-rede", "");
  $("d-titulo").textContent = m === "criar" ? "Criar rede" : "Entrar na rede";
  $("d-ok").textContent = m === "criar" ? "Criar" : "Entrar";
  $("d-ok").className = m === "criar" ? "inclui" : "";
  $("c-finalidade").hidden = m !== "criar";
  $("c-transporte").hidden = m !== "criar";
  ajustarProxyPeloProtocolo();
  $("d-rede").showModal();
}
$("b-criar").onclick = () => abrirDialogo("criar");
$("b-entrar").onclick = () => abrirDialogo("entrar");
$("d-cancelar").onclick = () => $("d-rede").close();
$("f-rede").onsubmit = async (ev) => {
  ev.preventDefault();
  try {
    const r = await api("POST", modo === "criar" ? "/api/redes" : "/api/redes/entrar", dados(ev.target));
    baixar(r.arquivo, r.perfil); $("d-rede").close(); carregarRedes();
  } catch (x) { msg("m-rede", x.message); }
};
$("b-atualizar").onclick = iniciar;
$("b-logout").onclick = sair;
$("b-voltar").onclick = iniciar;
$("b-admin").onclick = async () => { mostrar("tela-admin"); carregarAdmin(); };
$("b-mfa").onclick = () => { mostrar("tela-mfa"); carregarMfa(); };
$("b-mfa-voltar").onclick = iniciar;
let mfaAtivo = false;
async function carregarMfa() {
  $("mfa-novo").hidden = true; $("mfa-qr").replaceChildren(); $("mfa-segredo").textContent = ""; $("f-mfa").reset(); msg("m-mfa", "");
  try { mfaAtivo = (await api("GET", "/api/mfa")).ativo; } catch (x) { return msg("m-mfa", x.message); }
  $("mfa-estado").textContent = mfaAtivo ? "Ativo: o login e as redes que o exigem pedem o código." : "Não cadastrado.";
  $("b-mfa-iniciar").hidden = mfaAtivo;
  $("b-mfa-ok").textContent = mfaAtivo ? "Desativar" : "Confirmar";
  $("b-mfa-ok").className = mfaAtivo ? "exclui" : "";
  $("b-mfa-ok").hidden = !mfaAtivo;
  $("mfa-rotulo").textContent = mfaAtivo ? "Código atual (para desativar)" : "Código que o aplicativo mostra";
}
$("b-mfa-iniciar").onclick = async () => {
  try {
    const r = await api("POST", "/api/mfa/iniciar");
    // Analisado como XML e posto no DOM: imagem por data: a CSP do painel
    // (default-src 'self') recusa, e innerHTML nao entra nesta tela.
    const svg = new DOMParser().parseFromString(r.qr_svg, "image/svg+xml").documentElement;
    svg.setAttribute("width", "220"); svg.setAttribute("height", "220");
    $("mfa-qr").replaceChildren(document.importNode(svg, true));
    $("mfa-segredo").textContent = r.segredo.replace(/(.{4})/g, "$1 ").trim();
    $("mfa-novo").hidden = false; $("b-mfa-ok").hidden = false; $("b-mfa-iniciar").hidden = true;
  } catch (x) { msg("m-mfa", x.message); }
};
$("f-mfa").onsubmit = async (ev) => {
  ev.preventDefault();
  try {
    await api("POST", mfaAtivo ? "/api/mfa/desativar" : "/api/mfa/confirmar", dados(ev.target));
    const feito = mfaAtivo ? "autenticador desativado" : "autenticador ativo";
    await carregarMfa(); msg("m-mfa", feito, true);
  } catch (x) { msg("m-mfa", x.message); }
};

async function carregarAdmin() {
  const tu = $("t-usuarios"), ts = $("t-servidores"); tu.textContent = ""; ts.textContent = "";
  const linha = (corpo, valores) => { const tr = document.createElement("tr"); for (const v of valores) { const td = document.createElement("td"); td.textContent = v; tr.appendChild(td); } corpo.appendChild(tr); };
  try {
    for (const u of await api("GET", "/api/usuarios")) {
      linha(tu, [u.login, u.email, u.admin ? "sim" : "não", u.mfa ? "cadastrado" : "—"]);
      if (u.mfa) {
        const b = document.createElement("button"); b.className = "exclui"; b.textContent = "Zerar";
        b.onclick = async () => { if (!confirm(`Zerar o autenticador de «${u.login}»? Ele cadastra de novo no próximo login.`)) return; try { await api("POST", "/api/usuarios/mfa-zerar", { login: u.login }); carregarAdmin(); } catch (x) { msg("m-usuario", x.message); } };
        tu.lastChild.lastChild.append(" ", b);
      }
    }
  } catch (x) { msg("m-usuario", x.message); }
  try { for (const s of await api("GET", "/api/servidores")) linha(ts, [s.nome, s.ip, s.dns, String(s.redes)]); } catch (x) { msg("m-servidor", x.message); }
}
$("f-usuario").onsubmit = async (ev) => { ev.preventDefault(); try { await api("POST", "/api/usuarios", dados(ev.target)); ev.target.reset(); msg("m-usuario", "usuário incluído", true); carregarAdmin(); } catch (x) { msg("m-usuario", x.message); } };
$("f-servidor").onsubmit = async (ev) => { ev.preventDefault(); try { await api("POST", "/api/servidores", dados(ev.target)); ev.target.reset(); msg("m-servidor", "servidor incluído", true); carregarAdmin(); } catch (x) { msg("m-servidor", x.message); } };
$("f-cofre").onsubmit = async (ev) => { ev.preventDefault(); try { await api("POST", "/api/destrancar", dados(ev.target)); iniciar(); } catch (x) { msg("m-cofre", x.message); } };
$("f-login").onsubmit = async (ev) => {
  ev.preventDefault();
  try { sessao = await api("POST", "/api/login", dados(ev.target)); try { sessionStorage.setItem("phxvpn", JSON.stringify(sessao)); } catch (_) {} iniciar(); }
  catch (x) { msg("m-login", x.message); }
};
$("f-instalar").onsubmit = async (ev) => {
  ev.preventDefault(); msg("m-instalar", "instalando… (gera a autoridade certificadora e deriva a chave mestre)", true);
  try { await api("POST", "/api/instalar", dados(ev.target)); msg("m-instalar", "instalado", true); iniciar(); }
  catch (x) { msg("m-instalar", x.message); }
};
iniciar();

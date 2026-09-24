"use strict";
// A ficha da sessao vem no fragmento da URL (#f=...): o fragmento nao vai ao
// servidor nem vaza no Referer. Lida uma vez, some da barra de endereco.
const ficha = new URLSearchParams(location.hash.slice(1)).get("f") || "";
history.replaceState(null, "", location.pathname);

const $ = (id) => document.getElementById(id);
const el = (tag, cls, texto) => { const e = document.createElement(tag); if (cls) e.className = cls; if (texto !== undefined) e.textContent = texto; return e; };

async function api(metodo, rota, corpo) {
  const r = await fetch(rota, {
    method: metodo,
    headers: { "Content-Type": "application/json", "Authorization": "Bearer " + ficha },
    body: corpo ? JSON.stringify(corpo) : undefined,
  });
  const j = await r.json().catch(() => ({ erro: "resposta ilegivel" }));
  if (!r.ok) throw new Error(j.erro || ("erro " + r.status));
  return j;
}

function aviso(texto, ok) { const r = $("rodape"); r.textContent = texto || ""; r.className = "rodape " + (ok ? "ok" : "erro"); }

async function copiar(texto, rotulo) {
  try { await navigator.clipboard.writeText(texto); aviso(rotulo + " copiado", true); }
  catch (_) { aviso("nao consegui copiar: selecione e copie a mao"); }
}

let abertos = new Set();   // redes com os membros expandidos

function desenhar(redes) {
  const lista = $("lista"); lista.textContent = "";
  if (!redes.length) {
    lista.appendChild(el("div", "vazio", "Nenhuma rede ainda. Crie uma, ou entre numa com o codigo de convite."));
    return;
  }
  for (const r of redes) {
    const bloco = el("div", "rede");
    const topo = el("div", "topo");
    const lamp = el("span", "lampada" + (r.ligada ? " on" : "")); lamp.title = r.ligada ? "ligada" : "desligada";
    const nome = el("span", "nome", r.rede); nome.style.cursor = "pointer";
    nome.onclick = () => { abertos.has(r.rede) ? abertos.delete(r.rede) : abertos.add(r.rede); atualizar(); };
    const acoes = el("span", "acoes");
    const conv = el("button", "", "Convidar"); conv.onclick = () => abrirConvidar(r.rede);
    const liga = el("button", r.ligada ? "exclui" : "inclui", r.ligada ? "Desligar" : "Ligar");
    liga.onclick = () => r.ligada ? desligar(r.rede, liga) : abrirLigar(r);
    acoes.append(conv, liga);
    const online = r.membros.filter((m) => m.online).length;
    const info = el("span", "info", `meu IP ${r.ip} · ${r.membros.length} membro(s)` + (r.ligada ? `, ${online} conectado(s)` : "") + ` · modo ${r.modo}`);
    topo.append(lamp, nome, acoes, info);
    bloco.appendChild(topo);
    if (abertos.has(r.rede) || r.ligada) {
      for (const m of r.membros) {
        const linha = el("div", "membro");
        const p = el("span", "ponto" + (m.online ? " on" : "")); p.title = m.online ? "conectado" : "sem sessao";
        const ip = el("span", "ip", m.ip); ip.title = "clique para copiar"; ip.onclick = () => copiar(m.ip, "IP " + m.ip);
        const chave = el("span", "", m.chave); chave.style.color = "var(--fraco)"; chave.style.fontSize = "12px";
        const cam = el("span", "caminho", m.caminho === "-" ? m.sessao : `${m.caminho} · ${m.sessao}`);
        linha.append(p, ip, chave, cam);
        bloco.appendChild(linha);
      }
      if (!r.membros.length) bloco.appendChild(el("div", "membro", "sem membros ainda — use Convidar"));
    }
    lista.appendChild(bloco);
  }
}

async function atualizar() {
  try { desenhar(await api("GET", "/api/redes")); }
  catch (e) { aviso(e.message); }
}

function dialogo(id) {
  const d = $(id); const f = d.querySelector("form");
  f.reset(); f.querySelector(".msg").textContent = "";
  d.querySelectorAll("[data-fechar]").forEach((b) => b.onclick = () => d.close());
  d.showModal();
  return f;
}

async function enviar(form, rota, corpo, depois) {
  const botoes = form.querySelectorAll("button"); botoes.forEach((b) => b.disabled = true);
  try { const r = await api("POST", rota, corpo); depois(r); }
  catch (e) { form.querySelector(".msg").textContent = e.message; }
  finally { botoes.forEach((b) => b.disabled = false); }
}

$("b-criar").onclick = () => {
  const f = dialogo("d-criar");
  f.onsubmit = (ev) => { ev.preventDefault(); const d = Object.fromEntries(new FormData(f));
    enviar(f, "/api/criar", d, (r) => { $("d-criar").close(); aviso(r.ok, true); abertos.add(d.rede); atualizar(); }); };
};

$("b-entrar").onclick = () => {
  const f = dialogo("d-entrar");
  f.onsubmit = (ev) => { ev.preventDefault();
    enviar(f, "/api/entrar", Object.fromEntries(new FormData(f)), (r) => { $("d-entrar").close(); aviso(r.ok, true); atualizar(); }); };
};

function abrirConvidar(rede) {
  const f = dialogo("d-convidar");
  $("d-convidar").querySelector("[data-rede]").textContent = rede;
  const campo = f.querySelector("[name=codigo]").closest(".campo");
  campo.hidden = true; $("b-copiar").hidden = true; $("b-gerar").hidden = false;
  f.onsubmit = (ev) => { ev.preventDefault(); const d = Object.fromEntries(new FormData(f)); d.rede = rede;
    enviar(f, "/api/convidar", d, (r) => {
      f.querySelector("[name=codigo]").value = r.ok; campo.hidden = false;
      $("b-copiar").hidden = false; $("b-gerar").hidden = true;
      $("b-copiar").onclick = () => copiar(r.ok, "Convite");
    }); };
}

function abrirLigar(r) {
  const rede = r.rede;
  const f = dialogo("d-ligar");
  $("d-ligar").querySelector("[data-rede]").textContent = rede;
  // Usuario e senha do servidor intermediario so quando a rede passa por ele.
  const usa = r.modo !== "direto";
  $("repasse-conta").hidden = !usa;
  f.querySelector("[name=repasse_usuario]").value = r.repasse_usuario || "";
  f.onsubmit = (ev) => { ev.preventDefault(); const d = Object.fromEntries(new FormData(f)); d.rede = rede;
    if (!usa) { delete d.repasse_usuario; delete d.repasse_senha; }
    enviar(f, "/api/ligar", d, (x) => { $("d-ligar").close(); aviso(x.ok, true); atualizar(); }); };
}

async function desligar(rede, botao) {
  botao.disabled = true;
  try { aviso((await api("POST", "/api/desligar", { rede })).ok, true); } catch (e) { aviso(e.message); }
  atualizar();
}

(async () => {
  if (!ficha) { aviso("abra esta janela pelo phxvpn (o endereco traz a ficha da sessao)"); return; }
  try { const c = (await api("GET", "/api/chave")).ok; const s = $("minha-chave"); s.textContent = c.slice(0, 12) + "…"; s.onclick = () => copiar(c, "Chave"); }
  catch (e) { aviso(e.message); }
  atualizar();
  setInterval(atualizar, 2000);
})();

// Exercita a tela do PhxZip no Chromium, contra o servidor FALSO.
//
//     node testes-web/phxzip/exercitar.mjs              # tudo
//     node testes-web/phxzip/exercitar.mjs --so espiar  # os casos cujo nome contem o pedaco
//
// Interface so se prova EXERCITANDO: o CSS global morde, o `find` que devia ser
// `filter` quebra calado, a chave que ninguem pede fica morta -- e nada disso
// aparece lendo o codigo. Cada caso confere o EFEITO (o que chegou ao disco, o
// que a tela mostra, o que o servidor viu), e nao o estado de uma variavel.
//
// O `playwright` entra pelo caminho ABSOLUTO, como no `docs/dossie/olhar.mjs`:
// nao ha node_modules no repositorio, e pelo nome o Node nao o acha.
//
// Sobe o servidor falso numa porta so dele e o derruba pelo PID do FILHO que
// criou -- nunca por padrao de nome: `pgrep -f` casa a linha de comando do
// proprio shell que o chama (medido nesta frente: o `kill` derrubou o shell).
import { chromium } from "/opt/node22/lib/node_modules/playwright/index.mjs";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync, readdirSync } from "node:fs";
import { join, dirname, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, "../..");
const arg = nome => { const i = process.argv.indexOf(nome); return i > 0 ? process.argv[i + 1] : null; };
// `--ui <dir>`: exercita outra copia da tela -- a da prova das guardas, com um
// defeito reposto. A tela versionada nunca e tocada.
const UI = arg("--ui") ? resolve(arg("--ui")) : join(RAIZ, "crates/phxzip-web/ui");
const SEM_CAPTURAS = process.argv.includes("--sem-capturas");
const CAPTURAS = join(AQUI, "capturas");
const PORTA = Number(arg("--porta")) || 7799;
const PORTA_SEM_DRENO = PORTA - 2;
const BASE = `http://127.0.0.1:${PORTA}`;
const TETO_ENVIO = 16 << 20;
const TMP = mkdtempSync(join(tmpdir(), "phxzip-exercicio-"));
const LOG = join(TMP, "servidor.log");
// `--so a,b,c`: roda so os casos cujo nome contem um dos pedacos.
const SO = arg("--so") ? arg("--so").split(",") : null;

const SENHA = "s3nh@-Blumenau-2026";
const SENHA_ERRADA = "errada-123";
const SENHAS_USADAS = [SENHA, SENHA_ERRADA, "phx-config", "certa", "errada-7zip"];

mkdirSync(CAPTURAS, { recursive: true });
// As capturas sao DESTA corrida: a anterior sai antes, senao um nome que mudou
// deixa uma tela velha ao lado das novas, e ninguem sabe qual e de quando.
if (!SEM_CAPTURAS && !SO) for (const f of readdirSync(CAPTURAS)) if (f.endsWith(".png")) rmSync(join(CAPTURAS, f));

// ----------------------------------------------------------------- o registro
const resultados = [];
const achados = [];
async function caso(nome, corpo) {
  if (SO && !SO.some(p => nome.includes(p))) return;
  const t0 = Date.now();
  try {
    const nota = await corpo();
    resultados.push({ nome, ok: true, nota: nota || "", ms: Date.now() - t0 });
    console.log(`  ok    ${nome}${nota ? "  — " + nota : ""}`);
  } catch (e) {
    resultados.push({ nome, ok: false, nota: String(e.message || e).split("\n")[0], ms: Date.now() - t0 });
    console.log(`  FALHA ${nome}  — ${String(e.message || e).split("\n")[0]}`);
  }
}
function exigir(cond, msg) { if (!cond) throw new Error(msg); }
function achado(s) { achados.push(s); console.log(`  achado: ${s}`); }

// ----------------------------------------------------------------- o servidor
function subir(porta, extra = []) {
  return new Promise((ok, falha) => {
    const p = spawn("python3", [join(AQUI, "servidor_falso.py"), "--porta", String(porta),
      "--log", LOG, "--teto-envio", String(TETO_ENVIO), "--ui", UI, ...extra], { stdio: ["ignore", "pipe", "pipe"] });
    let err = "";
    p.stderr.on("data", d => { err += d; });
    p.stdout.on("data", d => { if (String(d).includes("PhxZip falso em")) ok(p); });
    p.on("exit", c => falha(new Error(`servidor falso saiu (${c}): ${err.slice(-400)}`)));
    setTimeout(() => falha(new Error("servidor falso nao subiu em 8 s")), 8000);
  });
}

// ----------------------------------------------------------------- os arquivos
const enc = new TextEncoder();
const RELATORIO = Buffer.from("Relatório anual — Blumenau, SC. Ação e coração.\n".repeat(200));
const DADOS = Buffer.from(JSON.stringify({ cidade: "Blumenau", uf: "SC", itens: [1, 2, 3] }, null, 2) + "\n");
const BLUMENAU = Buffer.from("Blumenau\n");
function cenario(nome) {
  const p = join(TMP, `cenario-${nome}.7z`);
  writeFileSync(p, Buffer.concat([Buffer.from([0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c]), Buffer.from(`\0CENARIO:${nome}\n`)]));
  return p;
}
function pastaDeProjeto() {
  const d = join(TMP, "projeto");
  mkdirSync(join(d, "src"), { recursive: true });
  writeFileSync(join(d, "leia-me.md"), "# Projeto\n\nUm projeto de exemplo.\n");
  writeFileSync(join(d, "src", "main.rs"), "fn main() {\n    println!(\"ola\");\n}\n");
  return d;
}

// ----------------------------------------------------------------- a pagina
async function pronta(page) {
  await page.waitForFunction(() => !document.body.classList.contains("carregando"));
}
async function ocioso(page) {
  await page.waitForFunction(() => !document.body.classList.contains("ocupado"), null, { timeout: 30000 });
}
async function capturar(page, nome, opcoes = {}) {
  if (SEM_CAPTURAS) return;
  const inteira = opcoes.inteira !== false;
  // Pagina inteira com a pagina rolada desenha a barra `sticky` no MEIO da
  // captura (achado na primeira corrida): volta ao topo antes.
  if (inteira) await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: join(CAPTURAS, nome + ".png"), fullPage: inteira });
}
async function abrirPacote(page, caminho) {
  await page.click("#abaAbrir");
  await page.setInputFiles("#inPacote", caminho);
  await ocioso(page);
  await page.waitForFunction(() =>
    document.querySelector("#listaPacote:not([hidden])") || document.querySelector("#avisoAbrir [data-erro]"));
}
/** Espera o download OU o cartao de erro -- o que vier primeiro. Esperar so o
    download faz o caso falhar por «Timeout», que nao diz nada; com o cartao,
    a falha diz o codigo que o servidor devolveu. */
async function baixarOuErro(page, acao, seletorErro) {
  const dl = page.waitForEvent("download", { timeout: 30000 });
  const erro = page.waitForSelector(seletorErro, { timeout: 30000 }).then(async el => {
    throw new Error(`em vez do download, o erro ${await el.getAttribute("data-erro")}`);
  });
  await acao();
  const r = await Promise.race([dl, erro]);
  erro.catch(() => {});
  return r;
}
async function erroNaTela(page) {
  return page.evaluate(() => { const e = document.querySelector("#avisoAbrir [data-erro]"); return e ? e.getAttribute("data-erro") : null; });
}
async function semRolagemLateral(page) {
  return page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 0.5);
}

/** Todo `.dado` da tela se desenha EXATAMENTE como esta gravado: o `innerText`
    aplica `text-transform`, o `textContent` nao. Qualquer diferenca e um CSS
    mentindo sobre o dado. */
async function dadosIntactos(page) {
  return page.evaluate(() => {
    const ruins = [];
    for (const el of document.querySelectorAll(".dado")) {
      if (!el.offsetParent && getComputedStyle(el).position !== "fixed") continue;
      const a = el.innerText.replace(/\s+/g, " ").trim();
      const b = el.textContent.replace(/\s+/g, " ").trim();
      if (a !== b) ruins.push(`${b} -> ${a}`);
    }
    return ruins;
  });
}

/** Texto solto como filho DIRETO de um container flex ou grid perde os
    espacos das bordas e ganha o `gap` no lugar: «11,0 KiB  von  16,0 MiB  ,
    die…» saiu assim, em alemao. Todo elemento flex/grid com texto proprio E
    filhos elemento e o mesmo defeito esperando uma frase com dado no meio. */
async function textoSoltoEmFlex(page) {
  return page.evaluate(() => {
    const ruins = [];
    for (const el of document.querySelectorAll("body *")) {
      const d = getComputedStyle(el).display;
      if (!/flex|grid/.test(d) || !el.children.length) continue;
      const solto = [...el.childNodes].filter(n => n.nodeType === 3 && n.textContent.trim()).map(n => n.textContent.trim());
      if (solto.length) ruins.push(`${el.id ? "#" + el.id : el.className || el.tagName}: «${solto.join(" | ").slice(0, 50)}»`);
    }
    return ruins;
  });
}

/** Contraste medido de cada botao de acao visivel e de cada titulo de aviso,
    contra o fundo EFETIVO (o primeiro ancestral que pinta). E o fundo das
    acoes em repouso: tem de ser transparente -- contorno, nunca fundo cheio. */
async function medirCores(page) {
  return page.evaluate(() => {
    const cor = s => { const m = s.match(/rgba?\(([^)]+)\)/); if (!m) return null; const p = m[1].split(/[ ,/]+/).filter(Boolean).map(Number); return [p[0], p[1], p[2], p.length > 3 ? p[3] : 1]; };
    const lin = v => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
    const lum = c => 0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2]);
    const razao = (a, b) => { const x = lum(a), y = lum(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
    function fundo(el) {
      for (let e = el; e; e = e.parentElement) {
        const c = cor(getComputedStyle(e).backgroundColor);
        if (c && c[3] > 0.5) return c;
      }
      return cor(getComputedStyle(document.body).backgroundColor);
    }
    const out = { acoes: [], avisos: [], fundoCheio: [] };
    for (const b of document.querySelectorAll(".acao")) {
      if (!b.offsetParent || b.disabled) continue;
      const cs = getComputedStyle(b);
      const bg = cor(cs.backgroundColor);
      if (bg && bg[3] > 0) out.fundoCheio.push(b.id || b.className);
      const f = fundo(b.parentElement);
      out.acoes.push({ quem: b.id || b.className, razao: +razao(cor(cs.color), f).toFixed(2) });
    }
    for (const t of document.querySelectorAll(".aviso-titulo, .aviso-faca, .dica, .zona-dica, .teto-texto, .grade th, .sub")) {
      if (!t.offsetParent) continue;
      out.avisos.push({ quem: t.className, razao: +razao(cor(getComputedStyle(t).color), fundo(t)).toFixed(2) });
    }
    return out;
  });
}

/** Pseudoidioma: cada texto da fabrica chega entre ⟦ ⟧. Depois, todo texto
    VISIVEL da tela ou esta entre ⟦ ⟧, ou e dado (`.dado`), ou e marca. O que
    sobrar foi digitado no fonte -- e texto cravado. Pega o que o laco
    estatico nao pega: frase montada pelo JS com pedaco literal. */
async function textosCravados(page) {
  return page.evaluate(() => {
    const isento = el => el.closest(".dado, .palavra, .lema, .endonimo, .invisivel, .bandeira, svg, script, style, title");
    const ruins = [];
    const vis = el => { const r = el.getBoundingClientRect(); return (r.width > 0 && r.height > 0) || el.closest("dialog[open]"); };
    for (const el of document.querySelectorAll("body *")) {
      if (isento(el) || !vis(el)) continue;
      let proprio = "";
      for (const n of el.childNodes) if (n.nodeType === 3) proprio += n.textContent;
      proprio = proprio.trim();
      if (/\p{L}/u.test(proprio) && !(proprio.startsWith("⟦") && proprio.endsWith("⟧"))) ruins.push(`<${el.tagName.toLowerCase()}> ${proprio.slice(0, 60)}`);
      for (const a of ["title", "aria-label", "placeholder"]) {
        const v = el.getAttribute(a);
        if (!v || !/\p{L}/u.test(v)) continue;
        if (/^\d{4}-\d{2}-\d{2}T/.test(v)) continue;          // a data em UTC: dado
        if (!(v.startsWith("⟦") && v.endsWith("⟧"))) ruins.push(`[${a}] ${v.slice(0, 60)}`);
      }
    }
    return ruins;
  });
}

// =====================================================================
console.log(`PhxZip — exercicio da tela (${new Date().toISOString()})`);

// ------------------------------------------------------------ 1. o laco das chaves
await caso("laco-das-chaves", async () => {
  const html = readFileSync(join(UI, "index.html"), "utf8");
  const js = readFileSync(join(UI, "phxzip.js"), "utf8");
  const dic = JSON.parse(readFileSync(join(UI, "textos.json"), "utf8")).textos;
  const pedidas = new Set([
    ...[...html.matchAll(/data-txt(?:-ph|-tt|-al)?="(zip\.[a-z0-9_]+)"/g)].map(m => m[1]),
    ...[...js.matchAll(/["'`](zip\.[a-z0-9_.]+)["'`]/g)].map(m => m[1]),
  ]);
  const temos = new Set(Object.keys(dic));
  const faltam = [...pedidas].filter(k => !temos.has(k));
  const mortas = [...temos].filter(k => !pedidas.has(k));
  exigir(!faltam.length, `chave pedida e ausente: ${faltam.join(", ")}`);
  exigir(!mortas.length, `chave morta (ninguem pede): ${mortas.join(", ")}`);
  const cols = readFileSync(join(RAIZ, "crates/phxsql-server/src/mensagens.rs"), "utf8")
    .match(/pub const IDIOMAS: \[&str; \d+\] = \[(.*?)\];/s)[1].match(/"([^"]+)"/g).map(s => s.slice(1, -1));
  for (const [k, cel] of Object.entries(dic)) {
    for (const c of cols) exigir(cel[c], `${k}: celula ${c} vazia`);
    const marc = s => [...s.matchAll(/\{(\w+)\}/g)].map(m => m[1]).sort().join(",");
    for (const c of cols) exigir(marc(cel[c]) === marc(cel.Portugues), `${k}: ${c} tem marcadores diferentes do Portugues`);
  }
  exigir(!/<form[\s>]/i.test(html), "ha <form> na pagina: Enter mandaria GET ?senha=");
  exigir(!/\sstyle="/i.test(html), "atributo style= no HTML: o CSP sem 'unsafe-inline' o recusa");
  return `${pedidas.size} chaves pedidas = ${temos.size} no dicionario, ${cols.length} idiomas cheios`;
});

// ------------------------------------------------------------ o servidor e o navegador
const servidor = await subir(PORTA);
const navegador = await chromium.launch();
const erros = [];
const vistoPeloNavegador = [];
function vigiar(page, rotulo) {
  page.on("console", m => { if (m.type() === "error") erros.push(`${rotulo} console: ${m.text()}`); });
  page.on("pageerror", e => erros.push(`${rotulo} pageerror: ${e.message}`));
  page.on("dialog", d => { erros.push(`${rotulo} DIALOGO ABERTO: ${d.message()}`); d.dismiss(); });
  page.on("request", r => vistoPeloNavegador.push(r.url() + " " + JSON.stringify(r.headers())));
}
async function novaPagina(opts = {}) {
  const ctx = await navegador.newContext({ acceptDownloads: true, viewport: { width: 1280, height: 860 }, locale: "pt-BR", ...opts });
  await ctx.addInitScript(() => {
    document.addEventListener("securitypolicyviolation", e => console.error(`CSP ${e.violatedDirective} ${e.blockedURI}`));
  });
  const page = await ctx.newPage();
  vigiar(page, opts.viewport ? `${opts.viewport.width}px` : "desktop");
  return { ctx, page };
}

const { ctx, page } = await novaPagina({ colorScheme: "dark" });
// A pagina abre na PREPARACAO, e nao dentro de um caso: com `--so`, uma cadeia
// sem o caso que a abria rodava sobre `about:blank` e falhava por Timeout --
// achado pela prova das guardas, que exige a cadeia passar sem o defeito.
await page.goto(BASE + "/");
await pronta(page);

await caso("abre-com-marca-e-fonte", async () => {
  exigir((await page.title()).startsWith("PhxZip"), "titulo sem PhxZip");
  const txt = await page.evaluate(() => document.body.innerText);
  exigir(!/\bzip\.[a-z_]+/.test(txt), "chave crua na tela (texto que o servidor nao devolveu)");
  const fonte = await page.evaluate(async () => {
    await document.fonts.ready;
    return [...document.fonts].filter(f => f.family.replace(/"/g, "") === "Exo 2" && f.status === "loaded").length;
  });
  exigir(fonte > 0, "a Exo 2 nao carregou (a pagina caiu na fonte substituta)");
  const img = await page.evaluate(() => document.querySelector(".marca img").naturalWidth);
  exigir(img > 0, "o simbolo da marca nao carregou");
  exigir(await page.evaluate(() => document.querySelectorAll("form").length === 0), "ha <form>");
  await capturar(page, "01-inicio-escuro");
  return `Exo 2 carregada da propria porta (${fonte} face)`;
});

// ------------------------------------------------------------ COMPACTAR
await caso("compactar-arquivos-e-pasta", async () => {
  await page.setInputFiles("#inArquivos", [
    { name: "relatorio.txt", mimeType: "text/plain", buffer: RELATORIO },
    { name: "dados.json", mimeType: "application/json", buffer: DADOS },
    { name: "Blumenau.txt", mimeType: "text/plain", buffer: BLUMENAU },
  ]);
  await page.setInputFiles("#inPasta", pastaDeProjeto());
  const nomes = await page.$$eval("#gradeCompactar tbody .nome", ns => ns.map(n => n.textContent));
  for (const n of ["relatorio.txt", "dados.json", "Blumenau.txt", "projeto", "projeto/leia-me.md", "projeto/src", "projeto/src/main.rs"])
    exigir(nomes.includes(n), `faltou ${n} na lista: ${nomes.join(" | ")}`);
  const ruins = await dadosIntactos(page);
  exigir(!ruins.length, `dado desenhado diferente do gravado: ${ruins.join("; ")}`);
  const soltos = await textoSoltoEmFlex(page);
  exigir(!soltos.length, `texto solto em flex/grid: ${soltos.join("; ")}`);
  await capturar(page, "02-compactar-lista");
  return `${nomes.length} itens, pastas derivadas dos caminhos`;
});

await caso("soltar-na-janela", async () => {
  await page.evaluate(() => {
    const dt = new DataTransfer();
    dt.items.add(new File(["conteudo solto\n"], "solto.txt", { type: "text/plain", lastModified: 1790000000000 }));
    dt.items.add(new File(["Blumenau\n"], "Blumenau.txt", { type: "text/plain" }));
    window.__dt = dt;
    window.dispatchEvent(new DragEvent("dragenter", { dataTransfer: dt, bubbles: true, cancelable: true }));
  });
  exigir(await page.isVisible("#soltar"), "a sobreposicao de soltar nao apareceu");
  await capturar(page, "03-soltar-sobreposicao", { inteira: false });
  await page.evaluate(() => window.dispatchEvent(new DragEvent("drop", { dataTransfer: window.__dt, bubbles: true, cancelable: true })));
  await page.waitForFunction(() => [...document.querySelectorAll("#gradeCompactar .nome")].some(n => n.textContent === "solto.txt"));
  exigir(!(await page.isVisible("#soltar")), "a sobreposicao ficou na tela depois de soltar");
  exigir(await page.isVisible("#recadoCompactar"), "o repetido (Blumenau.txt) entrou calado");
  return "solto.txt entrou; Blumenau.txt repetido foi avisado e ficou de fora";
});

await caso("phz-diz-por-que-nao-anda", async () => {
  await page.click('label[for="seg-formato-phz"]');
  const motivos = await page.$$eval("#porquesCompactar li", ls => ls.length);
  exigir(motivos >= 2, `o .phz com varios arquivos e sem senha deu ${motivos} motivo(s)`);
  exigir(await page.isDisabled("#btCompactar"), "o botao andou num .phz invalido");
  exigir(await page.isDisabled("#seg-nivel-armazenar"), "o nivel continuou escolhivel no .phz, que e sempre LZMA2");
  await capturar(page, "04-phz-regras");
  await page.click('label[for="seg-formato-7z"]');
  return `${motivos} motivos escritos ao lado do botao desligado`;
});

await caso("senhas-que-nao-conferem", async () => {
  await page.fill("#inSenha", SENHA);
  await page.fill("#inSenha2", SENHA + "x");
  exigir((await page.getAttribute("#inSenha2", "aria-invalid")) === "true", "confirmacao errada sem aria-invalid");
  exigir(await page.isDisabled("#btCompactar"), "compactaria com as senhas diferentes");
  await page.fill("#inSenha2", SENHA);
  exigir(!(await page.isDisabled("#btCompactar")), "senhas iguais e o botao continua desligado");
});

let pacoteBaixado = null;
await caso("enter-compacta-e-nao-vaza-senha", async () => {
  await page.fill("#inNomePacote", "relatorio-2026");
  const dl = await baixarOuErro(page, () => page.press("#inSenha2", "Enter"), "#resultadoCompactar [data-erro]");
  pacoteBaixado = join(TMP, dl.suggestedFilename());
  await dl.saveAs(pacoteBaixado);
  exigir(dl.suggestedFilename() === "relatorio-2026.7z", `nome do download: ${dl.suggestedFilename()}`);
  exigir(!page.url().includes("?"), `a URL ganhou query: ${page.url()}`);
  await page.waitForSelector('#resultadoCompactar [data-estado="ok"]');
  await capturar(page, "05-compactado");
  return `baixado ${dl.suggestedFilename()} (${readFileSync(pacoteBaixado).length} bytes)`;
});

await caso("limpar-leva-a-senha-junto", async () => {
  await page.click("#btLimpar");
  exigir(await page.isHidden("#opcoesCompactar"), "as opcoes ficaram com a lista vazia");
  exigir((await page.inputValue("#inSenha")) === "" && (await page.inputValue("#inSenha2")) === "",
    "a senha ficou escondida no campo e cifraria a proxima lista sem ninguem ver");
});

await caso("progresso-e-cancelar", async () => {
  const grande = Buffer.from("linha de um arquivo grande para ver a barra andar\n".repeat(250000)); // ~12 MB
  await page.setInputFiles("#inArquivos", [{ name: "grande.txt", mimeType: "text/plain", buffer: grande }]);
  const baixou = page.waitForEvent("download", { timeout: 60000 });
  await page.click("#btCompactar");
  await page.waitForSelector('#progressoCompactar:not([hidden])[data-fase="enviando"]', { timeout: 10000 });
  await page.waitForFunction(() => { const b = document.querySelector("#progressoCompactar .medidor-barra"); return b && parseFloat(b.style.width) > 5; });
  await capturar(page, "06-progresso-enviando");
  await page.waitForSelector('#progressoCompactar[data-fase="processando"]', { timeout: 20000 });
  await capturar(page, "07-progresso-processando");
  const dl = await baixou;
  exigir(dl.suggestedFilename().endsWith(".7z"), "o grande nao baixou");
  // De novo, e cancela no meio.
  await page.click("#btCompactar");
  await page.waitForSelector('#progressoCompactar:not([hidden])[data-fase="enviando"]', { timeout: 10000 });
  await page.click("#progressoCompactar .acao");
  await page.waitForSelector('#resultadoCompactar [data-erro="CANCELADO"]');
  await capturar(page, "08-cancelado");
  await page.click("#btLimpar");
  return `${(grande.length / 1048576).toFixed(1)} MiB: enviando -> processando -> download; cancelar vira CANCELADO`;
});

await caso("teto-de-envio-na-tela", async () => {
  await page.setInputFiles("#inArquivos", [{ name: "acima-do-teto.bin", mimeType: "application/octet-stream", buffer: Buffer.alloc(TETO_ENVIO + 1024, 7) }]);
  await page.waitForSelector("#tetoCompactar.estourou");
  exigir(await page.isDisabled("#btCompactar"), "compactaria acima do teto");
  await capturar(page, "09-acima-do-teto");
  await page.click("#btLimpar");
  return "teto lido do /api/estado, conferido antes de enviar";
});

// ------------------------------------------------------------ ABRIR
await caso("abrir-o-que-compactou-pede-senha", async () => {
  exigir(pacoteBaixado && existsSync(pacoteBaixado), "sem pacote do caso anterior");
  await abrirPacote(page, pacoteBaixado);
  exigir((await erroNaTela(page)) === "SENHA_AUSENTE", `esperava SENHA_AUSENTE, veio ${await erroNaTela(page)}`);
  exigir(await page.isVisible("#inSenhaPacote"), "o campo de senha nao apareceu");
  await capturar(page, "10-pede-senha");
  await page.fill("#inSenhaPacote", SENHA_ERRADA);
  await page.press("#inSenhaPacote", "Enter");
  await ocioso(page);
  exigir((await erroNaTela(page)) === "SENHA_ERRADA", `esperava SENHA_ERRADA, veio ${await erroNaTela(page)}`);
  exigir((await page.getAttribute("#inSenhaPacote", "aria-invalid")) === "true", "senha errada sem aria-invalid");
  exigir(!page.url().includes("?"), "Enter na senha do pacote poe query na URL");
  await capturar(page, "11-senha-errada");
  await page.fill("#inSenhaPacote", SENHA);
  await page.press("#inSenhaPacote", "Enter");
  await ocioso(page);
  await page.waitForSelector("#listaPacote:not([hidden])");
  const nomes = await page.$$eval("#gradePacote tbody .nome", ns => ns.map(n => n.textContent));
  for (const n of ["relatorio.txt", "dados.json", "Blumenau.txt", "projeto/src/main.rs", "solto.txt"])
    exigir(nomes.includes(n), `a volta perdeu ${n}: ${nomes.join(" | ")}`);
  await capturar(page, "12-lista");
  return `${nomes.length} entradas voltaram com os mesmos nomes`;
});

await caso("baixar-uma-entrada-byte-a-byte", async () => {
  const linha = page.locator("#gradePacote tbody tr", { has: page.locator(".nome", { hasText: /^relatorio\.txt$/ }) });
  const [dl] = await Promise.all([page.waitForEvent("download"), linha.locator('[data-acao="baixar"]').click()]);
  const p = join(TMP, "volta-" + dl.suggestedFilename());
  await dl.saveAs(p);
  exigir(readFileSync(p).equals(RELATORIO), "o relatorio.txt baixado nao e o que entrou");
  return `${dl.suggestedFilename()}: ${RELATORIO.length} bytes iguais`;
});

await caso("baixar-todas-em-tar", async () => {
  const [dl] = await Promise.all([page.waitForEvent("download"), page.click("#btBaixarTodas")]);
  const p = join(TMP, dl.suggestedFilename());
  await dl.saveAs(p);
  exigir(dl.suggestedFilename() === "relatorio-2026.tar", `nome do tar: ${dl.suggestedFilename()}`);
  const noTar = execFileSync("tar", ["-tf", p]).toString().trim().split("\n").map(s => s.replace(/\/$/, ""));
  const naTela = await page.$$eval("#gradePacote tbody .nome", ns => ns.map(n => n.textContent));
  exigir(JSON.stringify([...noTar].sort()) === JSON.stringify([...naTela].sort()), `tar ${noTar.length} × tela ${naTela.length}`);
  return `${noTar.length} entradas no tar, as mesmas da tela (conferido pelo tar do sistema)`;
});

await caso("testar-integridade", async () => {
  await page.click("#btTestar");
  await ocioso(page);
  await page.waitForSelector('#resultadoTeste [data-estado="ok"]');
  await capturar(page, "13-teste-ok");
});

await caso("espiar-json-valido-e-invalido", async () => {
  await abrirPacote(page, cenario("config"));
  exigir((await erroNaTela(page)) === "SENHA_AUSENTE", "o config cifrado nao pediu senha");
  await page.fill("#inSenhaPacote", "phx-config");
  await page.click("#btAbrirComSenha");
  await ocioso(page);
  await page.click('#gradePacote [data-acao="espiar"]');
  await ocioso(page);
  await page.waitForSelector("dialog#espiar[open]");
  exigir(await page.isVisible("#espiarSelos .json-ok"), "JSON valido sem o selo");
  await capturar(page, "14-espiar-json-valido", { inteira: false });
  await page.click("#btFecharEspiar");
  await abrirPacote(page, cenario("config-quebrado"));
  await page.fill("#inSenhaPacote", "phx-config");
  await page.click("#btAbrirComSenha");
  await ocioso(page);
  await page.click('#gradePacote [data-acao="espiar"]');
  await ocioso(page);
  await page.waitForSelector("dialog#espiar[open]");
  const selo = await page.textContent("#espiarSelos .json-erro");
  const linhaErro = await page.textContent("#espiarCorpo .linha-erro");
  exigir(linhaErro && linhaErro.includes(",,"), `a linha marcada nao e a do erro: ${linhaErro}`);
  await capturar(page, "15-espiar-json-invalido", { inteira: false });
  await page.click("#btFecharEspiar");
  return `selo «${selo.trim()}»; a linha marcada contem o «,,»`;
});

await caso("espiar-cortado-binario-e-solido", async () => {
  await abrirPacote(page, cenario("misto"));
  exigir(await page.isVisible("#resumoPacote .solido"), "bloco solido sem selo");
  const solidos = await page.$$eval("#gradePacote .rot-solido", xs => xs.length);
  exigir(solidos > 0, "nenhuma linha diz «solido»");
  await capturar(page, "16-lista-solida");
  const log = page.locator("#gradePacote tbody tr", { has: page.locator(".nome", { hasText: /^grande\.log$/ }) });
  await log.locator('[data-acao="espiar"]').click();
  await ocioso(page);
  await page.waitForSelector("dialog#espiar[open]");
  exigir(await page.isVisible("#espiarSelos .corte"), "trecho cortado sem o selo que diz");
  await page.click("#btFecharEspiar");
  const png = page.locator("#gradePacote tbody tr", { has: page.locator(".nome", { hasText: /marca\.png$/ }) });
  await png.locator('[data-acao="espiar"]').click();
  await ocioso(page);
  await page.waitForSelector("dialog#espiar[open] .hex");
  await capturar(page, "17-espiar-binario", { inteira: false });
  await page.click("#btFecharEspiar");
  return `${solidos} linhas «solido» (compactado do bloco nao se divide)`;
});

// Os estados de erro: um por cenario, cada um com FORMA (icone e titulo), nao so cor.
const ESTADOS = [
  ["corrompido", "CORROMPIDO"], ["legado", "METODO_LEGADO"], ["zipslip", "NOME_PERIGOSO"],
  ["grande", "GRANDE_DEMAIS"], ["estrutura", "ESTRUTURA"], ["desconhecido", "ERRO_QUE_A_TELA_NAO_CONHECE"],
];
for (const [cen, codigo] of ESTADOS) {
  await caso(`estado-${cen}`, async () => {
    await abrirPacote(page, cenario(cen));
    exigir((await erroNaTela(page)) === codigo, `esperava ${codigo}, veio ${await erroNaTela(page)}`);
    const card = page.locator(`#avisoAbrir [data-erro="${codigo}"]`);
    exigir(await card.locator(".ic.grande svg").count() === 1, "cartao sem icone (estado so por cor)");
    const titulo = (await card.locator(".aviso-titulo").textContent()).trim();
    if (cen === "legado") exigir(titulo.includes("BZip2") && titulo.includes("PhxZip"), `legado sem o metodo: ${titulo}`);
    if (cen === "zipslip") exigir(await card.locator(".dado.nome").textContent() === "../../etc/cron.d/phx", "zip-slip sem o nome cru");
    await capturar(page, `18-erro-${cen}`, { inteira: false });
    return titulo;
  });
}
await caso("estado-nao-e-7z", async () => {
  const p = join(TMP, "nao-sou-7z.7z");
  writeFileSync(p, "texto qualquer, sem a assinatura\n");
  await abrirPacote(page, p);
  exigir((await erroNaTela(page)) === "NAO_E_7Z", `veio ${await erroNaTela(page)}`);
});
await caso("estado-senha-errada-ou-corrompido", async () => {
  await abrirPacote(page, cenario("cifrado7zip"));
  await page.fill("#inSenhaPacote", "errada-7zip");
  await page.click("#btAbrirComSenha");
  await ocioso(page);
  exigir((await erroNaTela(page)) === "SENHA_ERRADA_OU_CORROMPIDO", `veio ${await erroNaTela(page)}`);
  await capturar(page, "18-erro-senha-ou-corrompido", { inteira: false });
});

await caso("nomes-hostis-e-invisiveis", async () => {
  await abrirPacote(page, cenario("xss"));
  const nomes = await page.$$eval("#gradePacote tbody .nome", ns => ns.map(n => n.textContent));
  exigir(nomes.includes("<img src=x onerror=alert(1)>.txt"), "o nome com marcacao nao apareceu escrito");
  exigir(await page.$$eval("#gradePacote img", xs => xs.length) === 0, "um <img> nasceu de um nome de entrada");
  const marca = await page.$$eval("#gradePacote .invisivel", xs => xs.map(x => x.textContent));
  exigir(marca.includes("U+202E"), `o U+202E nao virou marca: ${marca}`);
  const ruins = await dadosIntactos(page);
  exigir(!ruins.length, `dado desenhado diferente do gravado: ${ruins.join("; ")}`);
  await capturar(page, "19-nomes-hostis");
  return "marcacao escrita, U+202E mostrado como marca, «Blumenau» intacto";
});

// ------------------------------------------------------------ o servidor recusa o grande
await caso("servidor-413-com-e-sem-dreno", async () => {
  const semDreno = await subir(PORTA_SEM_DRENO, ["--nao-drenar"]);
  try {
    const medir = porta => page.evaluate(async ([p, tam]) => {
      const corpo = new Uint8Array(tam);
      try {
        const r = await fetch(`http://127.0.0.1:${p}/api/listar`, { method: "POST", body: corpo, headers: { "Content-Type": "application/octet-stream" } });
        return `HTTP ${r.status} ${(await r.json()).erro}`;
      } catch (e) { return `falha de rede (${e.name}: ${e.message})`; }
    }, [porta, TETO_ENVIO + (8 << 20)]);
    const com = await medir(PORTA);
    const sem = await medir(PORTA_SEM_DRENO);
    achado(`413 sem ler o corpo — COM dreno: ${com}; SEM dreno: ${sem}`);
    exigir(com.startsWith("HTTP 413"), `com dreno a tela nao recebeu o 413: ${com}`);
    return `com dreno: ${com}; sem: ${sem}`;
  } finally { semDreno.kill(); }
});

// ------------------------------------------------------------ a senha nao sai do corpo
/** As formas em que uma senha aparece fora do corpo. Crua nao basta: numa
    URL o `@` vira `%40`, e a primeira versao desta guarda PASSAVA com a senha
    na query -- a prova das guardas repos o defeito e ela nao viu. */
function formas(s) {
  return [...new Set([s, encodeURIComponent(s), new URLSearchParams({ x: s }).toString().slice(2), encodeURI(s),
    encodeURIComponent(s).toLowerCase(), Buffer.from(s).toString("base64")])];
}
const aparece = (texto, s) => formas(s).some(f => texto.includes(f) || texto.toLowerCase().includes(f.toLowerCase()));

await caso("senha-so-no-corpo", async () => {
  const log = readFileSync(LOG, "utf8");
  const noLog = SENHAS_USADAS.filter(s => aparece(log, s));
  exigir(!noLog.length, `senha no log do servidor (URL ou cabecalho): ${noLog.join(", ")}`);
  const noFio = SENHAS_USADAS.filter(s => vistoPeloNavegador.some(v => aparece(v, s)));
  exigir(!noFio.length, `senha em URL ou cabecalho de pedido: ${noFio.join(", ")}`);
  const naMemoria = await page.evaluate(() => JSON.stringify(localStorage));
  exigir(!SENHAS_USADAS.some(s => naMemoria.includes(s)), "senha no localStorage");
  return `${log.split("\n").filter(l => /^\d{3} /.test(l)).length} pedidos no log, nenhuma das ${SENHAS_USADAS.length} senhas fora do corpo`;
});

await caso("fechar-limpa-a-senha", async () => {
  await abrirPacote(page, cenario("config"));
  await page.fill("#inSenhaPacote", "phx-config");
  await page.click("#btAbrirComSenha");
  await ocioso(page);
  await page.click("#btFechar");
  exigir((await page.inputValue("#inSenhaPacote")) === "", "a senha ficou no campo depois de fechar o pacote");
});

// ------------------------------------------------------------ o CSS global, o dado e as cores
await caso("controles-no-tamanho-certo", async () => {
  await page.click("#abaCompactar");
  await page.setInputFiles("#inArquivos", [{ name: "um.txt", mimeType: "text/plain", buffer: Buffer.from("um\n") }]);
  const caixas = await page.$$eval('input[type="checkbox"]', xs => xs.map(x => { const r = x.getBoundingClientRect(); return [x.id, Math.round(r.width), Math.round(r.height)]; }));
  for (const [id, w, h] of caixas) exigir(w <= 20 && h <= 20, `caixa ${id} com ${w}x${h}`);
  return caixas.map(([id, w, h]) => `${id} ${w}x${h}`).join(", ");
});

async function coresNoTema(tema) {
  // Espera as transicoes acabarem: medido na hora do clique, o botao ainda
  // estava na cor do tema anterior (2,18:1 no papel) -- o numero era do meio
  // do caminho, nao do tema.
  await page.waitForFunction(() => document.getAnimations().length === 0);
  const r = await medirCores(page);
  exigir(!r.fundoCheio.length, `${tema}: acao com fundo cheio em repouso: ${r.fundoCheio.join(", ")}`);
  const baixas = [...r.acoes, ...r.avisos].filter(x => x.razao < 4.5);
  exigir(!baixas.length, `${tema}: contraste abaixo de 4,5:1: ${baixas.map(b => `${b.quem} ${b.razao}`).join("; ")}`);
  const min = Math.min(...[...r.acoes, ...r.avisos].map(x => x.razao));
  return `${tema}: ${r.acoes.length} acoes e ${r.avisos.length} textos, menor contraste ${min.toFixed(2)}:1`;
}

await caso("cores-escuro", async () => {
  await abrirPacote(page, cenario("legado"));
  const a = await coresNoTema("escuro (com erro na tela)");
  await abrirPacote(page, cenario("misto"));
  return a + " | " + await coresNoTema("escuro (com lista)");
});

await caso("tema-claro", async () => {
  // O estado que o rotulo diz: o erro na tela, e nao a lista que o caso
  // anterior deixou aberta (a primeira versao mediu a lista chamando-a de erro).
  await abrirPacote(page, cenario("legado"));
  await page.click("#btTema");
  exigir((await page.getAttribute("html", "data-tema")) === "claro", "o tema nao trocou");
  const nota = await coresNoTema("claro (com erro na tela)");
  await capturar(page, "20-claro-erro", { inteira: false });
  await abrirPacote(page, cenario("misto"));
  await capturar(page, "20-claro-lista");
  const nota2 = await coresNoTema("claro (com lista)");
  await page.click("#abaCompactar");
  await capturar(page, "20-claro-compactar");
  await page.click("#btTema");
  return nota + " | " + nota2;
});

// ------------------------------------------------------------ os idiomas
await caso("troca-de-idioma-repinta-tudo", async () => {
  const dic = JSON.parse(readFileSync(join(UI, "textos.json"), "utf8")).textos;
  await abrirPacote(page, cenario("legado"));
  const lista = await page.evaluate(() => fetch("/api/estado").then(r => r.json()).then(j => j.idiomas));
  const vistos = [];
  for (const col of lista) {
    await page.click("#btIdioma");
    await page.click(`#menuIdiomas li[data-idi="${col}"]`);
    await page.waitForFunction(t => document.querySelector("#abaAbrir [data-txt]").textContent === t, dic["zip.aba_abrir"][col]);
    // O cartao de erro, desenhado pelo JS, tem de mudar junto -- e o que
    // ficaria para tras se a mensagem tivesse sido guardada como texto.
    const titulo = await page.textContent('#avisoAbrir [data-erro="METODO_LEGADO"] .aviso-titulo');
    const esperado = dic["zip.erro_metodo_legado"][col].replace("{metodo}", "BZip2");
    exigir(titulo.trim() === esperado, `${col}: cartao ficou em outra lingua: «${titulo.trim()}»`);
    vistos.push(`${col}:${await page.getAttribute("html", "lang")}`);
    if (col === "Frances") await capturar(page, "21-idioma-frances");
  }
  await page.click("#btIdioma");
  await page.click('#menuIdiomas li[data-idi="Portugues"]');
  return vistos.join(" ");
});

await caso("pseudoidioma-sem-texto-cravado", async () => {
  const { ctx: c2, page: p2 } = await novaPagina();
  await p2.route("**/api/idiomas*", async rota => {
    const r = await rota.fetch();
    const j = await r.json();
    for (const k of Object.keys(j.textos)) j.textos[k] = "⟦" + j.textos[k] + "⟧";
    await rota.fulfill({ response: r, json: j });
  });
  await p2.goto(BASE + "/");
  await pronta(p2);
  const ruins = new Set();
  const somar = async onde => {
    for (const r of await textosCravados(p2)) ruins.add(`${onde}: ${r}`);
    for (const r of await textoSoltoEmFlex(p2)) ruins.add(`${onde}: texto solto em flex ${r}`);
  };
  await somar("inicio");
  await p2.setInputFiles("#inArquivos", [{ name: "a.txt", mimeType: "text/plain", buffer: Buffer.from("a\n") }]);
  await p2.click('label[for="seg-formato-phz"]');
  await somar("compactar");
  await p2.click('label[for="seg-formato-7z"]');
  const [dl] = await Promise.all([p2.waitForEvent("download"), p2.click("#btCompactar")]);
  await dl.path();
  await somar("compactado");
  await abrirPacote(p2, cenario("misto"));
  await somar("lista");
  await p2.click("#btTestar"); await ocioso(p2);
  await somar("teste");
  await p2.click('#gradePacote [data-acao="espiar"]'); await ocioso(p2);
  await p2.waitForSelector("dialog#espiar[open]");
  await somar("espiar");
  await p2.click("#btFecharEspiar");
  await abrirPacote(p2, cenario("grande"));
  await somar("erro");
  await capturar(p2, "22-pseudoidioma");
  await c2.close();
  exigir(!ruins.size, `texto fora da fabrica: ${[...ruins].slice(0, 8).join(" | ")}`);
  return "nenhum texto visivel fora de ⟦ ⟧ em sete estados";
});

// ------------------------------------------------------------ 360 px
await caso("celular-360", async () => {
  const { ctx: c3, page: p3 } = await novaPagina({ viewport: { width: 360, height: 740 } });
  await p3.goto(BASE + "/");
  await pronta(p3);
  await p3.setInputFiles("#inArquivos", [
    { name: "relatorio-anual-com-um-nome-comprido-demais-para-celular.txt", mimeType: "text/plain", buffer: RELATORIO },
    { name: "Blumenau.txt", mimeType: "text/plain", buffer: BLUMENAU },
  ]);
  const estados = [];
  const olhar = async nome => {
    exigir(await semRolagemLateral(p3), `${nome}: rolagem lateral em 360 px`);
    const ruins = await dadosIntactos(p3);
    exigir(!ruins.length, `${nome}: dado desenhado diferente do gravado: ${ruins.join("; ")}`);
    const soltos = await textoSoltoEmFlex(p3);
    exigir(!soltos.length, `${nome}: texto solto em flex/grid: ${soltos.join("; ")}`);
    await capturar(p3, `23-celular-${nome}`);
    estados.push(nome);
  };
  await olhar("compactar");
  await abrirPacote(p3, cenario("xss"));
  await olhar("lista-hostil");
  await abrirPacote(p3, cenario("misto"));
  await p3.locator('#gradePacote [data-acao="espiar"]').first().click();
  await ocioso(p3);
  await p3.waitForSelector("dialog#espiar[open]");
  exigir(await semRolagemLateral(p3), "espiar: rolagem lateral em 360 px");
  await capturar(p3, "23-celular-espiar", { inteira: false });
  await p3.click("#btFecharEspiar");
  await abrirPacote(p3, cenario("zipslip"));
  await olhar("erro-zipslip");
  // Alemao: as palavras mais compridas das seis.
  await p3.click("#btIdioma");
  await p3.click('#menuIdiomas li[data-idi="Alemao"]');
  await p3.waitForFunction(() => document.documentElement.lang === "de");
  await p3.click("#abaCompactar");
  await olhar("alemao");
  await c3.close();
  return estados.join(", ") + " + espiar, sem rolagem lateral";
});

// ------------------------------------------------------------ o fim
await caso("sem-erro-no-console-nem-csp", async () => {
  // O 413 SEM dreno e provocado de proposito: o erro de rede que ele deixa no
  // console e o achado, nao um defeito da tela.
  const deVerdade = erros.filter(e => !e.includes(`127.0.0.1:${PORTA_SEM_DRENO}`) && !/Failed to load resource.*(4\d\d|5\d\d)/.test(e));
  exigir(!deVerdade.length, deVerdade.slice(0, 5).join(" | "));
  return `${erros.length} mensagens de console, todas explicadas (4xx provocados pelos cenarios)`;
});

await ctx.close();
await navegador.close();
servidor.kill();
rmSync(TMP, { recursive: true, force: true });

const falhas = resultados.filter(r => !r.ok);
const resumo = { quando: new Date().toISOString(), casos: resultados.length, falhas: falhas.length, achados, resultados };
if (!SO && !SEM_CAPTURAS) writeFileSync(join(CAPTURAS, "resultado.json"), JSON.stringify(resumo, null, 1) + "\n");
if (arg("--resultado")) writeFileSync(arg("--resultado"), JSON.stringify(resumo));
console.log(`\n${resultados.length - falhas.length}/${resultados.length} casos passaram; capturas em testes-web/phxzip/capturas/`);
process.exit(falhas.length ? 1 : 0);

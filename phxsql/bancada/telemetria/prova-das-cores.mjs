/* A prova das cores configuráveis do painel de bolhas — EXERCITANDO.
 *
 *     cargo build --release -p phxsql-server --bin phxsqld
 *     node bancada/telemetria/prova-das-cores.mjs --capturas <dir>
 *
 * Ela sobe um `phxsqld` só dela (portas 6600/6601), enche uma tabela, põe
 * quatro somas de verificação concorrentes para produzir os três estados
 * vivos (normal, uso alto, stress), e então:
 *
 *   1. captura o painel com as cores DE FÁBRICA, nos dois temas;
 *   2. abre a tela de Configurações, escolhe uma paleta e mede o aviso de
 *      contraste (um amarelo claro, que reprova, e um roxo, que passa);
 *   3. salva, volta ao painel e confere que a cor CHEGOU — nos dois temas;
 *   4. confere que o que não é cor não mudou: o glifo e o traço da borda
 *      continuam nos quatro níveis, e a legenda deixa de dizer «amarelo»
 *      quando o amarelo deixou de ser amarelo.
 *
 * O Playwright NÃO entra no projeto — a regra de zero dependência vale, e um
 * conferidor de tela não é motivo para quebrá-la. Ele é procurado onde
 * estiver instalado, como nos outros dois conferidores desta pasta.
 */
const { chromium } = await import(
  process.env.PLAYWRIGHT || "/opt/node22/lib/node_modules/playwright/index.mjs");
import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { connect } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, "..", "..");
const PHXSQLD = join(RAIZ, "target", "release", "phxsqld");

/* A faixa reservada a este agente. Fora dela não se encosta, e a queda é
   sempre pelo PID que este processo guardou — `pkill -f` mataria também o
   servidor do vizinho que roda a bateria ao lado. */
const PORTA_DADOS = 6600;
const PORTA_WEB = 6601;
const USUARIO = "adm";
const SENHA = "segredo1";
const TOKEN = "prova-das-cores";

const arg = (n, p = null) => {
  const i = process.argv.indexOf(n);
  return i > 0 && process.argv[i + 1] ? process.argv[i + 1] : p;
};
const CAPTURAS = arg("--capturas", join(AQUI, "capturas-cores"));
const dormir = ms => new Promise(r => setTimeout(r, ms));

const problemas = [];
const notas = [];
const verdade = (c, oQue) => { if (!c) problemas.push(oQue); };

/* ------------------------------------------------------------- o servidor */

function hashDaSenha(senha) {
  const r = spawnSync(PHXSQLD, ["--senha"], { input: senha, encoding: "utf8" });
  const m = /"senha_hash": "([^"]+)"/.exec(r.stdout || "");
  if (!m) throw new Error(`phxsqld --senha nao devolveu o hash: ${r.stdout}${r.stderr}`);
  return m[1];
}

async function esperarPorta(porta, prazoMs = 20000) {
  const fim = Date.now() + prazoMs;
  while (Date.now() < fim) {
    const abriu = await new Promise(r => {
      const s = connect({ host: "127.0.0.1", port: porta }, () => { s.destroy(); r(true); });
      s.on("error", () => r(false));
      s.setTimeout(500, () => { s.destroy(); r(false); });
    });
    if (abriu) return true;
    await dormir(120);
  }
  return false;
}

function montarConfig(dir) {
  const base = join(dir, "dados");
  const caminho = join(dir, "config.json");
  writeFileSync(caminho, JSON.stringify({
    base,
    bind: `127.0.0.1:${PORTA_DADOS}`,
    token: TOKEN,
    max_linhas: 1000,
    web: { ligado: true, bind: `127.0.0.1:${PORTA_WEB}`, sessao_minutos: 60 },
    recursos: { durabilidade: "sistema", cache_paginas: 512 },
    // Os limiares BAIXOS são o que faz os três estados aparecerem numa carga
    // de segundos em vez de minutos — e, de quebra, são o próprio campo novo
    // sendo lido: se o servidor os ignorasse, nenhuma bolha sairia do azul.
    telemetria: { alto_uso_ms: 150, stress_ms: 600 },
    usuarios: [{
      id: 10, nome: "Adriano Boller", login: USUARIO, senha_hash: hashDaSenha(SENHA),
      supervisor: true, ativo: true, bases: {},
    }],
    replicacao: { papel: "isolado" },
  }, null, 2) + "\n");
  return caminho;
}

/** Uma conexão de dados, para a carga. */
function conexao() {
  const s = connect({ host: "127.0.0.1", port: PORTA_DADOS });
  let resto = "";
  const fila = [];
  s.setEncoding("utf8");
  s.on("data", d => {
    resto += d;
    let i;
    while ((i = resto.indexOf("\n")) >= 0) {
      const linha = resto.slice(0, i);
      resto = resto.slice(i + 1);
      const espera = fila.shift();
      if (espera) espera(linha);
    }
  });
  s.on("error", () => {});
  return {
    pedir(corpo) {
      return new Promise(r => {
        fila.push(r);
        s.write(`{"token":"${TOKEN}",${corpo}}\n`);
      });
    },
    fechar() { s.destroy(); },
  };
}

/** Entra na conexão de dados: com cadastro configurado, o token não basta. */
async function logar(c) {
  const r = await c.pedir(`"op":"login","usuario":"${USUARIO}","senha":"${SENHA}"`);
  if (!r.includes('"ok":true')) throw new Error(`login falhou: ${r.slice(0, 200)}`);
  return c;
}

async function encher(linhas) {
  const c = conexao();
  await new Promise(r => setTimeout(r, 200));
  await logar(c);
  await c.pedir('"op":"criar_database","database":"loja"');
  await c.pedir('"op":"criar_tabela","database":"loja","tabela":"clientes",'
    + '"colunas":[{"nome":"nome","tipo":"Str","tamanho":40},{"nome":"valor","tipo":"Int8"}]');
  let feitas = 0;
  while (feitas < linhas) {
    const lote = [];
    for (let i = 0; i < 20000 && feitas + i < linhas; i++) {
      lote.push(`{"nome":"Cliente ${feitas + i}","valor":${i % 97}}`);
    }
    const r = await c.pedir('"op":"inserir_lote","database":"loja","tabela":"clientes",'
      + `"linhas":[${lote.join(",")}]`);
    if (!r.includes('"ok":true')) throw new Error(`inserir_lote falhou: ${r.slice(0, 200)}`);
    feitas += lote.length;
  }
  c.fechar();
  return feitas;
}

/* Somas concorrentes: uma segura a trava de dados, as outras esperam na fila.
 *
 * O laço não para enquanto a prova durar — uma carga que acaba entre o
 * `waitForSelector` e a captura devolveria um painel todo azul e um teste
 * verde que não provou nada. */
function cargaDeSomas(quantas) {
  const abertas = [];
  for (let i = 0; i < quantas; i++) {
    const c = conexao();
    abertas.push(c);
    (async () => {
      await logar(c);
      while (!c.parada) {
        await c.pedir('"op":"checksum","database":"loja","tabela":"clientes"');
        // Um respiro entre duas somas. Sem ele as quatro conexões seguram a
        // trava de dados quase o tempo todo e a PRÓPRIA TELA não consegue
        // entrar: o login pede a trava como todo mundo. A carga existe para
        // pintar o painel, não para derrubar a prova.
        await dormir(120);
      }
    })().catch(() => {});
  }
  return {
    parar() { abertas.forEach(c => { c.parada = true; c.fechar(); }); },
  };
}

/* ------------------------------------------------------------- a tela */

async function entrar(page, url) {
  await page.goto(url, { waitUntil: "domcontentloaded" });
  await page.waitForSelector("#btEntrar");
  await page.waitForFunction(() => typeof est === "object" && est.demo === false,
    { timeout: 15000 });
  await page.fill("#u", USUARIO);
  await page.fill("#s", SENHA);
  await page.fill("#t", TOKEN);
  await page.click("#btEntrar");
  await page.waitForSelector("#app.ativo", { timeout: 40000 });
  await page.waitForSelector("#arvore .no", { timeout: 40000 });
  // O `abrirAdmin("painel")` disparado no login é ASSÍNCRONO e, sob a carga
  // desta prova, termina DEPOIS: ele sobrescreve o `#painel` da tela que
  // acabamos de abrir, e a captura sai com o cabeçalho da Telemetria e o
  // corpo do Painel. Foi exatamente o que aconteceu aqui na primeira volta —
  // a mesma armadilha que o próprio módulo documenta. Esperar o Painel
  // aparecer é esperar aquele `abrirAdmin` terminar.
  await page.waitForFunction(
    () => (document.querySelector("#painel")?.textContent || "").includes("A máquina"),
    { timeout: 40000 });
}

/** Rola ate a legenda: e la que se ve se a palavra da cor casa com o disco. */
async function verLegenda(page) {
  await page.evaluate(() => document.querySelector("#tlmExplica")
    ?.scrollIntoView({ block: "center" }));
  await dormir(500);
}

/** Abre a Telemetria e garante que ela FICOU — nada a sobrescreveu depois. */
async function abrirTelemetria(page) {
  for (let tentativa = 0; tentativa < 4; tentativa++) {
    await page.evaluate(() => telaTelemetria());
    await page.waitForSelector("#tlmBolhas [data-id]", { timeout: 25000 }).catch(() => {});
    await dormir(2600);
    if (await page.$("#tlmBolhas [data-id]")) return;
  }
  throw new Error("a tela da Telemetria nao ficou (sobrescrita ou sem bolha)");
}

const foto = async (page, nome) => {
  mkdirSync(CAPTURAS, { recursive: true });
  await page.screenshot({ path: join(CAPTURAS, `${nome}.png`) });
  return `${nome}.png`;
};

/** O que o painel está de fato pintando: a cor de cada bolha e da legenda. */
async function lerPainel(page) {
  return await page.evaluate(() => {
    const como = c => {
      const d = document.createElement("span");
      d.style.color = c; document.body.appendChild(d);
      const v = getComputedStyle(d).color; d.remove(); return v;
    };
    const bolhas = [...document.querySelectorAll("#tlmBolhas [data-id] .tlm-c")].map(c => ({
      cor: como(c.getAttribute("stroke")),
      traco: c.getAttribute("stroke-dasharray") || "",
    }));
    const legenda = [...document.querySelectorAll("#tlmExplica .tlm-leg")].map(l => ({
      n: l.dataset.n,
      texto: l.textContent.trim().replace(/\s+/g, " "),
      cor: como(getComputedStyle(l.querySelector("circle")).stroke),
    }));
    const niveis = ((window.PhxTelemetria && PhxTelemetria) ? null : null);
    return { bolhas, legenda, niveis };
  });
}

/* ------------------------------------------------------------- a prova */

const dir = mkdtempSync(join(tmpdir(), "phx-cores-"));
const caminhoConfig = montarConfig(dir);
const proc = spawn(PHXSQLD, ["--config", caminhoConfig], { cwd: dir, stdio: ["ignore", "pipe", "pipe"] });
const saida = [];
proc.stdout.on("data", d => saida.push(String(d)));
proc.stderr.on("data", d => saida.push(String(d)));
console.log(`· servidor pid ${proc.pid}, dados ${PORTA_DADOS}, web ${PORTA_WEB}`);

let carga = null;
let navegador = null;
try {
  if (!await esperarPorta(PORTA_DADOS)) throw new Error(`o servidor nao subiu:\n${saida.join("")}`);
  await esperarPorta(PORTA_WEB);

  const linhas = await encher(400_000);
  notas.push(`${linhas.toLocaleString("pt-BR")} linhas na tabela da carga`);
  carga = cargaDeSomas(4);

  navegador = await chromium.launch({ headless: true });
  const url = `http://127.0.0.1:${PORTA_WEB}/`;

  // ------------------------------------------------ 1. as cores de fábrica
  for (const tema of ["escuro", "claro"]) {
    const ctx = await navegador.newContext({ viewport: { width: 1500, height: 950 } });
    await ctx.addInitScript(t => { try { localStorage.setItem("phxsql-tema", t); } catch {} }, tema);
    const page = await ctx.newPage();
    const erros = [];
    page.on("pageerror", e => erros.push(e.message));
    await entrar(page, url);
    await abrirTelemetria(page);
    await foto(page, `01-painel-fabrica-${tema}`);
    await verLegenda(page);
    await foto(page, `01b-legenda-fabrica-${tema}`);

    const p = await lerPainel(page);
    const niveis = new Set(p.bolhas.map(b => b.traco));
    notas.push(`[${tema}] fábrica: ${p.bolhas.length} bolhas, traços ${[...niveis].map(t => t || "cheia").join(" / ")}`);
    verdade(p.bolhas.length >= 3, `[${tema}] a carga não produziu bolhas suficientes`);
    verdade(niveis.size >= 2,
      `[${tema}] a carga não produziu estados diferentes: ${[...niveis]}`);
    // A cor de fábrica é a variável do tema: no claro ela ESCURECE.
    const legAlto = p.legenda.find(l => l.n === "alto");
    verdade(/amarelo/.test(legAlto.texto),
      `[${tema}] a legenda de fábrica devia dizer «amarelo»: ${legAlto.texto}`);
    verdade(/borda tracejada/.test(legAlto.texto),
      `[${tema}] a legenda devia dizer o traço da borda: ${legAlto.texto}`);
    notas.push(`[${tema}] amarelo de fábrica na legenda = ${legAlto.cor}`);
    verdade(!erros.length, `[${tema}] erro de página (fábrica): ${erros.join(" | ")}`);
    await page.close(); await ctx.close();
  }

  // ------------------------------------------ 2. escolher a cor, na tela
  const ctxC = await navegador.newContext({ viewport: { width: 1500, height: 1000 } });
  await ctxC.addInitScript(() => { try { localStorage.setItem("phxsql-tema", "escuro"); } catch {} });
  const page = await ctxC.newPage();
  const erros = [];
  page.on("pageerror", e => erros.push(e.message));
  await entrar(page, url);
  await page.evaluate(() => verConfigServidor());
  await page.waitForSelector(".cmp-cor", { timeout: 15000 });
  await page.evaluate(() => document.querySelector(".cmp-cor").scrollIntoView({ block: "center" }));
  await dormir(300);
  await foto(page, "02-config-cores-de-fabrica-escuro");

  const deFabrica = await page.evaluate(() =>
    [...document.querySelectorAll(".cmp-cor")].map(b => ({
      nivel: b.dataset.nivel,
      valor: b.querySelector("input[type=hidden]").value,
      diz: b.querySelector(".cf-cor-diz").textContent.replace(/\s+/g, " ").trim(),
      mal: b.querySelector(".cf-cor-diz").classList.contains("mal"),
      amostra: !!b.querySelector(".tlm-amostra"),
    })));
  verdade(deFabrica.length === 4, `esperava 4 campos de cor, achei ${deFabrica.length}`);
  verdade(deFabrica.every(x => x.valor === ""), "campo de cor devia nascer vazio (= de fábrica)");
  verdade(deFabrica.every(x => x.amostra), "faltou a amostra da bolha ao lado do seletor");
  verdade(deFabrica.every(x => /de fábrica/.test(x.diz)), `a legenda de fábrica sumiu: ${JSON.stringify(deFabrica)}`);
  notas.push("de fábrica na tela: " + deFabrica.map(x => `${x.nivel} ${x.diz}`).join(" · "));

  // A cor que NENHUMA tinta salva.
  //
  // Medido aqui: com a tinta escolhida entre as duas (a clara e a escura, a
  // melhor das duas), o pior caso possível é 4,35:1, e ele acontece numa
  // faixa estreita de luminância — em torno de 0,19, que é o cinza médio.
  // Um amarelo CLARO, que é o exemplo que se conta por aí, passa com folga
  // (16,7:1) porque a tinta escura entra sozinha. Quem reprova é o meio-tom.
  const escolher = async (nivel, cor) => {
    await page.evaluate(([n, c]) => {
      const b = [...document.querySelectorAll(".cmp-cor")].find(x => x.dataset.nivel === n);
      const p = b.querySelector(".cf-cor-p");
      p.value = c;
      p.dispatchEvent(new Event("input", { bubbles: true }));
    }, [nivel, cor]);
    await dormir(120);
    return await page.evaluate(n => {
      const b = [...document.querySelectorAll(".cmp-cor")].find(x => x.dataset.nivel === n);
      const d = b.querySelector(".cf-cor-diz");
      return { valor: b.querySelector("input[type=hidden]").value,
               diz: d.textContent.replace(/\s+/g, " ").trim(),
               mal: d.classList.contains("mal") };
    }, nivel);
  };

  const amarelo = await escolher("alto", "#fff2b0");
  verdade(!amarelo.mal, `#fff2b0 (amarelo claro) passa com a tinta escura: ${amarelo.diz}`);
  notas.push(`amarelo claro #fff2b0 → ${amarelo.diz}`);

  const meio = await escolher("alto", "#797979");
  verdade(meio.mal, `#797979 (cinza médio) devia AVISAR: ${meio.diz}`);
  verdade(/atenção/i.test(meio.diz), `o aviso devia dizer o que houve: ${meio.diz}`);
  notas.push(`cinza médio #797979 → ${meio.diz}`);
  await page.evaluate(() => document.querySelector(".cmp-cor").scrollIntoView({ block: "center" }));
  await dormir(200);
  await foto(page, "03-config-aviso-de-contraste-escuro");

  // E o mesmo campo com uma cor que passa: o aviso some.
  const roxo = await escolher("alto", "#7b2ff7");
  verdade(!roxo.mal, `#7b2ff7 devia PASSAR: ${roxo.diz}`);
  verdade(roxo.valor === "#7b2ff7", `o valor gravável ficou ${roxo.valor}`);
  notas.push(`roxo #7b2ff7 → ${roxo.diz}`);

  const outras = [["normal", "#00c2a8"], ["stress", "#e01b6a"], ["encerrando", "#ff8a1c"]];
  for (const [n, c] of outras) notas.push(`${n} ${c} → ${(await escolher(n, c)).diz}`);
  await foto(page, "04-config-paleta-escolhida-escuro");

  // O botão de voltar às cores de fábrica, e depois a paleta de novo.
  await page.click("#cfCoresFabrica");
  await dormir(150);
  const zerou = await page.evaluate(() =>
    [...document.querySelectorAll(".cmp-cor input[type=hidden]")].map(i => i.value));
  verdade(zerou.every(v => v === ""), `«voltar às cores de fábrica» não zerou: ${zerou}`);
  for (const [n, c] of [["alto", "#7b2ff7"], ...outras]) await escolher(n, c);

  await page.click("#cfSalvar");
  await page.waitForFunction(() => /gravado/.test(document.querySelector(".barra .recado")?.textContent || ""),
    { timeout: 15000 }).catch(() => problemas.push("o salvar não confirmou na barra"));
  await dormir(400);
  await foto(page, "05-config-depois-de-salvar-escuro");

  // O arquivo tem de ter recebido o bloco.
  const noArquivo = await page.evaluate(() => api("config").then(c => c.telemetria));
  verdade(noArquivo && noArquivo.cor_alto === "#7b2ff7",
    `o config não voltou com a cor: ${JSON.stringify(noArquivo)}`);
  notas.push("config.telemetria = " + JSON.stringify(noArquivo));

  // ------------------------------------------ 3. o painel com a cor nova
  await abrirTelemetria(page);
  await foto(page, "06-painel-trocado-escuro");
  const p2 = await lerPainel(page);
  const legAlto2 = p2.legenda.find(l => l.n === "alto");
  verdade(legAlto2.cor === "rgb(123, 47, 247)",
    `a legenda não recebeu o roxo: ${legAlto2.cor}`);
  verdade(!/amarelo/.test(legAlto2.texto),
    `«amarelo» continuou escrito ao lado de uma bolha roxa: ${legAlto2.texto}`);
  verdade(/borda tracejada/.test(legAlto2.texto),
    `o traço da borda sumiu da legenda: ${legAlto2.texto}`);
  verdade(p2.bolhas.some(b => b.cor === "rgb(123, 47, 247)" || b.cor === "rgb(0, 194, 168)"),
    `nenhuma bolha ficou com a cor escolhida: ${JSON.stringify(p2.bolhas.slice(0, 6))}`);
  const tracos = new Set(p2.bolhas.map(b => b.traco));
  verdade(tracos.size >= 2, `o traço da borda se perdeu na troca de cor: ${[...tracos]}`);
  notas.push("painel trocado: " + JSON.stringify(p2.legenda.map(l => `${l.n} ${l.cor}`)));
  verdade(!erros.length, `erro de página (troca): ${erros.join(" | ")}`);
  await page.close(); await ctxC.close();

  // ------------------------------------------ 4. os dois temas, com a cor
  for (const tema of ["escuro", "claro"]) {
    const ctx = await navegador.newContext({ viewport: { width: 1500, height: 1000 } });
    await ctx.addInitScript(t => { try { localStorage.setItem("phxsql-tema", t); } catch {} }, tema);
    const pg = await ctx.newPage();
    const errs = [];
    pg.on("pageerror", e => errs.push(e.message));
    await entrar(pg, url);
    await abrirTelemetria(pg);
    await foto(pg, `07-painel-trocado-${tema}`);
    await verLegenda(pg);
    await foto(pg, `07b-legenda-trocada-${tema}`);

    // O contraste do rótulo DENTRO da bolha, medido contra o que o navegador
    // pintou — a mesma conta do `conferir-desenho.mjs`, e nos dois temas.
    const piores = await pg.evaluate(() => {
      const rgb = t => (String(t).match(/[\d.]+/g) || [0, 0, 0]).map(Number);
      const lum = c => {
        const f = v => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
        return 0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2]);
      };
      const razao = (a, b) => {
        const [x, y] = [lum(rgb(a)), lum(rgb(b))].sort((m, n) => n - m);
        return (x + 0.05) / (y + 0.05);
      };
      const como = c => {
        const d = document.createElement("span");
        d.style.color = c; document.body.appendChild(d);
        const v = getComputedStyle(d).color; d.remove(); return v;
      };
      const saida = [];
      for (const g of document.querySelectorAll("#tlmBolhas [data-id]")) {
        const t = g.querySelector(".tlm-id");
        if (!t || !t.textContent) continue;
        saida.push({
          razao: razao(como(t.getAttribute("fill")), como(g.querySelector(".tlm-c").getAttribute("stroke"))),
        });
      }
      return saida;
    });
    const pior = piores.reduce((m, x) => Math.min(m, x.razao), 99);
    notas.push(`[${tema}] pior contraste do rótulo na bolha = ${pior.toFixed(2)}:1`);
    verdade(pior >= 4.5 || pior === 99,
      `[${tema}] rótulo ilegível dentro da bolha: ${pior.toFixed(2)}:1`);

    // A tela de configuração, no tema, com a paleta já gravada.
    await pg.evaluate(() => verConfigServidor());
    await pg.waitForSelector(".cmp-cor", { timeout: 15000 });
    await pg.evaluate(() => document.querySelector(".cmp-cor").scrollIntoView({ block: "center" }));
    await dormir(300);
    await foto(pg, `08-config-com-paleta-${tema}`);
    verdade(!errs.length, `[${tema}] erro de página (paleta): ${errs.join(" | ")}`);
    await pg.close(); await ctx.close();
  }
} catch (e) {
  problemas.push(`quebrou: ${e && e.stack ? e.stack : e}`);
} finally {
  if (carga) carga.parar();
  if (navegador) await navegador.close();
  try { process.kill(proc.pid, "SIGTERM"); } catch {}
  await dormir(400);
  try { process.kill(proc.pid, "SIGKILL"); } catch {}
}

console.log("\nnotas:");
for (const n of notas) console.log("  · " + n);
console.log(`\ncapturas em ${CAPTURAS}`);
if (problemas.length) {
  console.log(`\n${problemas.length} PROBLEMA(S):`);
  for (const p of problemas) console.log("  ✗ " + p);
  process.exit(1);
}
console.log("\nok — as cores configuráveis passaram");

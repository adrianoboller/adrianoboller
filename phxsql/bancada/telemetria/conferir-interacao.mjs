/* A prova de INTERACAO: clicar na bolha MENOR com o painel em movimento,
   descer de nivel, voltar pela trilha, buscar, esconder a legenda.
   Nada disto aparece lendo o codigo. */
/* O Playwright nao entra no projeto: a regra de zero dependencia vale, e um
   conferidor de tela nao e motivo para quebra-la. Ele e ferramenta de quem
   confere, achada onde estiver instalada.

       PLAYWRIGHT=/caminho/para/playwright/index.mjs \
       node bancada/telemetria/conferir-desenho.mjs */
const { chromium } = await import(
  process.env.PLAYWRIGHT || "/opt/node22/lib/node_modules/playwright/index.mjs");
import path from "node:path";
const BASE = path.dirname(new URL(import.meta.url).pathname);
const DEST = process.env.TLM_CAPTURAS || BASE;

const b = await chromium.launch();
const p = await b.newPage({ viewport: { width: 1400, height: 1000 } });
const erros = [];
p.on("pageerror", e => erros.push("erro: " + e.message));
await p.goto("file://" + BASE + "/bancada.html");

const reprovadas = [];
const diga = (rot, ok, extra = "") => {
  if (!ok) reprovadas.push(rot);
  console.log(`${ok ? "OK  " : "FALHA"} ${rot}${extra ? " — " + extra : ""}`);
};

// ---- 1. clicar na MENOR bolha, com o painel se mexendo
await p.evaluate(() => window.montarCena(4));   // 40 atividades
await p.waitForTimeout(900);
const menor = await p.evaluate(() => {
  const gs = [...document.querySelectorAll("#tlmBolhas [data-id]")];
  const m = gs.map(g => ({ id: g.dataset.id, r: +g.querySelector(".tlm-c").getAttribute("r") }))
              .sort((a, c) => a.r - c.r)[0];
  return m;
});
const caixa = await p.locator(`#tlmBolhas [data-id="${menor.id}"]`).first().boundingBox();
await p.mouse.move(caixa.x + caixa.width / 2, caixa.y + caixa.height / 2);
await p.waitForTimeout(120);
await p.mouse.click(caixa.x + caixa.width / 2, caixa.y + caixa.height / 2);
await p.waitForTimeout(200);
const escolhida = await p.evaluate(() =>
  document.querySelector("#tlmCartao .tlm-cartao-cab b")?.textContent);
diga(`clicar na MENOR bolha (raio ${menor.r.toFixed(1)} px) com o painel vivo`,
     escolhida === menor.id, `cartao mostra ${escolhida}, esperado ${menor.id}`);

// A deriva tem de MORRER com o ponteiro dentro: duas medidas com 400 ms de
// intervalo tem de dar a mesma posicao.
await p.mouse.move(caixa.x + caixa.width / 2, caixa.y + caixa.height / 2);
const pos1 = await p.evaluate(id => {
  const c = document.querySelector(`#tlmBolhas [data-id="${id}"] .tlm-c`);
  return [+c.getAttribute("cx"), +c.getAttribute("cy")];
}, menor.id);
await p.waitForTimeout(700);
const pos2 = await p.evaluate(id => {
  const c = document.querySelector(`#tlmBolhas [data-id="${id}"] .tlm-c`);
  return [+c.getAttribute("cx"), +c.getAttribute("cy")];
}, menor.id);
diga("com o ponteiro dentro, a bolha PARA",
     Math.abs(pos1[0] - pos2[0]) < 0.2 && Math.abs(pos1[1] - pos2[1]) < 0.2,
     `${pos1} -> ${pos2}`);

// E tem de VOLTAR a derivar quando o ponteiro sai.
await p.mouse.move(10, 10);
await p.waitForTimeout(150);
const p3 = await p.evaluate(id => {
  const c = document.querySelector(`#tlmBolhas [data-id="${id}"] .tlm-c`);
  return [+c.getAttribute("cx"), +c.getAttribute("cy")];
}, menor.id);
await p.waitForTimeout(700);
const p4 = await p.evaluate(id => {
  const c = document.querySelector(`#tlmBolhas [data-id="${id}"] .tlm-c`);
  return [+c.getAttribute("cx"), +c.getAttribute("cy")];
}, menor.id);
diga("com o ponteiro fora, a bolha VOLTA a derivar",
     Math.abs(p3[0] - p4[0]) > 0.15 || Math.abs(p3[1] - p4[1]) > 0.15, `${p3} -> ${p4}`);

// ---- 2. descer de nivel: atividades -> estacoes -> conexoes de uma estacao
await p.evaluate(() => window.montarCena(3));
await p.waitForTimeout(700);
await p.screenshot({ path: path.join(DEST, "interacao-1-cartao-aberto.png"), fullPage: true });
await p.click('#tlmTrilha [data-nivel="estacoes"]');
await p.waitForTimeout(700);
const estacoes = await p.evaluate(() =>
  [...document.querySelectorAll("#tlmBolhas [data-id]")].map(g => g.dataset.id));
diga("vista por ESTACAO agrupa por IP", estacoes.every(i => i.startsWith("estacao:")),
     estacoes.join(", "));
await p.screenshot({ path: path.join(DEST, "interacao-2-por-estacao.png"), fullPage: true });

const primeira = estacoes[0];
await p.click(`#tlmBolhas [data-id="${primeira}"] .tlm-alvo`);
await p.waitForTimeout(700);
const dentro = await p.evaluate(() => ({
  trilha: document.querySelector("#tlmTrilha").textContent.replace(/\s+/g, " ").trim(),
  ids: [...document.querySelectorAll("#tlmBolhas [data-id]")].map(g => g.dataset.id),
}));
diga("descer numa estacao mostra as conexoes dela",
     dentro.ids.length > 0 && dentro.ids.every(i => !i.startsWith("estacao:")),
     `trilha «${dentro.trilha}», ${dentro.ids.length} conexao(oes)`);
await p.screenshot({ path: path.join(DEST, "interacao-3-dentro-da-estacao.png"), fullPage: true });

await p.click('#tlmTrilha [data-nivel="todas"]');
await p.waitForTimeout(600);
const voltou = await p.evaluate(() =>
  [...document.querySelectorAll("#tlmBolhas [data-id]")].length);
diga("a trilha volta para todas as atividades", voltou === 12, `${voltou} bolhas`);

// ---- 3. busca
await p.fill("#tlmBusca", "reindexar");
await p.waitForTimeout(500);
const achou = await p.evaluate(() => ({
  n: [...document.querySelectorAll("#tlmBolhas [data-id]")].length,
  resumo: document.querySelector("#tlmResumo").textContent,
}));
diga("a busca filtra", achou.n > 0 && achou.n < 12, `${achou.n} de 12 — «${achou.resumo}»`);
await p.fill("#tlmBusca", "zzz-nao-existe");
await p.waitForTimeout(400);
const nada = await p.evaluate(() => document.querySelector(".tlm-sem")?.textContent);
diga("busca sem resultado diz que nao achou", /filtro/.test(nada || ""), nada);
await p.fill("#tlmBusca", "");
await p.waitForTimeout(400);

// ---- 4. legenda
await p.click("#tlmLegenda");
await p.waitForTimeout(300);
const escondida = await p.evaluate(() =>
  document.querySelector("#tlmExplica").getBoundingClientRect().height === 0);
await p.click("#tlmLegenda");
await p.waitForTimeout(300);
const devolta = await p.evaluate(() =>
  document.querySelector("#tlmExplica").getBoundingClientRect().height > 0);
diga("ocultar/mostrar legenda", escondida && devolta);

// ---- 5. teclado
await p.evaluate(() => document.querySelector("#tlmBolhas [data-id]").focus());
await p.keyboard.press("Enter");
await p.waitForTimeout(250);
const porTeclado = await p.evaluate(() =>
  !!document.querySelector("#tlmBolhas .tlm-bolha.sel"));
diga("Enter numa bolha escolhe pelo teclado", porTeclado);

// ---- 6. prefers-reduced-motion desliga a deriva
const p2 = await b.newPage({ viewport: { width: 1400, height: 1000 } });
await p2.emulateMedia({ reducedMotion: "reduce" });
await p2.goto("file://" + BASE + "/bancada.html");
await p2.evaluate(() => window.montarCena(3));
await p2.waitForTimeout(900);
const a1 = await p2.evaluate(() =>
  [...document.querySelectorAll("#tlmBolhas .tlm-c")].map(c => +c.getAttribute("cx")));
await p2.waitForTimeout(800);
const a2 = await p2.evaluate(() =>
  [...document.querySelectorAll("#tlmBolhas .tlm-c")].map(c => +c.getAttribute("cx")));
diga("prefers-reduced-motion deixa tudo parado",
     a1.length > 0 && a1.every((v, i) => Math.abs(v - a2[i]) < 0.05));

await b.close();
console.log(erros.length ? "ERROS DE CONSOLE:\n" + erros.join("\n") : "sem erro de console");

/* O codigo de saida, que faltava.
 *
 * Ele imprimia «FALHA» e saia ZERO. Lido por gente, acusava; chamado pela
 * bateria unica, que soma exit codes, mentia verde. Conferidor que nao sabe
 * reprovar nao confere nada quando ninguem esta olhando -- e a mesma licao do
 * teste que passa por engano, um andar acima. */
if (reprovadas.length || erros.length) {
  console.log(`\n${reprovadas.length} reprovada(s), ${erros.length} erro(s) de console`);
}
process.exitCode = (reprovadas.length || erros.length) ? 1 : 0;

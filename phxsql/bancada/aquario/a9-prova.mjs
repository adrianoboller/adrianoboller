// Prova real da fatia A9 (aquario.js) no Chromium. Uso:
//   node bancada/aquario/a9-prova.mjs [captura.png]
import { chromium } from "/opt/node22/lib/node_modules/playwright/index.mjs";
import { resolve, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
const aqui = dirname(fileURLToPath(import.meta.url));
const saida = process.argv[2] || resolve(aqui, "../../target/aquario-a9.png");
const b = await chromium.launch({ args: ["--no-sandbox"] });
const p = await b.newPage({ viewport: { width: 1280, height: 720 } });
const erros = [];
p.on("pageerror", e => erros.push(String(e)));
p.on("console", m => m.type() === "error" && erros.push(m.text()));
await p.goto(pathToFileURL(resolve(aqui, "a9-pagina.html")).href);
let falhas = 0;
const ok = (c, m) => { console.log((c ? "OK    " : "FALHA ") + m); if (!c) falhas++; };

// 1) fps real com ~150 bolhas, com rotatividade (nascendo e estourando)
const fps = await p.evaluate(async () => {
  const a = PhxAquario.criar(document.getElementById("host"));
  window.a = a; a.atualizar(retrato(150, 7));
  let rodada = 0;
  const giro = setInterval(() => {
    rodada++;
    const l = retrato(150, 7).filter((_, i) => (i + rodada) % 15);
    a.atualizar(l.concat(retrato(10, 100 + rodada).map((t, i) => ({ ...t, id: "n" + rodada + "_" + i }))));
  }, 400);
  await new Promise(r => setTimeout(r, 2500));
  const q0 = a.stats.quadros, m0 = a.stats.msFrame, t0 = performance.now();
  await new Promise(r => setTimeout(r, 5000));
  const dt = (performance.now() - t0) / 1000;
  clearInterval(giro);
  const q = a.stats.quadros - q0;
  return { fps: q / dt, msPorQuadro: (a.stats.msFrame - m0) / q, nos: a.nosVivos(), sob: a.sobreposicoes() };
});
console.log("fps", fps.fps.toFixed(1), "| ms de trabalho por quadro", fps.msPorQuadro.toFixed(2), "| nos", fps.nos, "| sobreposicao", JSON.stringify(fps.sob));
ok(fps.fps >= 55, `>= 55 fps com ~150 bolhas (${fps.fps.toFixed(1)})`);
ok(fps.sob.pares === 0, "ao vivo, com colisao: zero pares sobrepostos");
await p.screenshot({ path: saida });
console.log("captura", saida);
await p.evaluate(() => window.a.parar());

// 2) RED/GREEN da colisao, no motor, sem relogio: 600 passos de 1/60 com giro
const prova = await p.evaluate(() => {
  function rodar(colisao) {
    const m = PhxAquario.criarMotor({ w: 1280, h: 720 }, { colisao, semente: 5 });
    let pior = 0, paresMax = 0;
    for (let k = 0; k < 600; k++) {
      if (k % 20 === 0) m.atualizar(retrato(150, 7).filter((_, i) => (i + k / 20) % 12));
      m.passo(1 / 60);
      if (k > 120) { const s = m.sobreposicoes(); pior = Math.max(pior, s.max); paresMax = Math.max(paresMax, s.pares); }
    }
    return { pior, paresMax };
  }
  return { com: rodar(true), sem: rodar(false) };
});
console.log("sem colisao", JSON.stringify(prova.sem), "| com colisao", JSON.stringify(prova.com));
ok(prova.sem.paresMax > 0, `RED: sem a colisao ha sobreposicao (${prova.sem.paresMax} pares, ate ${prova.sem.pior.toFixed(1)} px)`);
ok(prova.com.paresMax === 0, "GREEN: com a colisao nenhum par se sobrepoe mais de 0,5 px");

// 3) estouro e privacidade
const priv = await p.evaluate(async () => {
  document.body.innerHTML = '<div id="h2" style="width:600px;height:300px"></div>';
  const a = PhxAquario.criar(document.getElementById("h2"), { auto: false });
  a.atualizar(retrato(5, 3));
  for (let i = 0; i < 60; i++) a.passo(1 / 60);
  const antes = a.nosVivos(), html = document.getElementById("h2").innerHTML;
  a.atualizar([]); a.passo(1 / 60);
  const logo = a.nosVivos();
  for (let i = 0; i < 40; i++) a.passo(1 / 60);
  return { antes, logo, depois: a.nosVivos(), login: /joao\.silva/.test(html), ip: /10\.1\.2\./.test(html),
    vazio: document.getElementById("h2").classList.contains("aq-sem") };
});
ok(priv.antes === 5 && priv.logo === 5 && priv.depois === 0, `estouro: ${priv.antes} -> ${priv.logo} (estourando) -> ${priv.depois}`);
ok(!priv.login && !priv.ip, "login e IP nunca chegam ao DOM");
ok(priv.vazio, "lista vazia mostra o aviso");

// 4) o dado nao vira caixa alta, mesmo com o CSS global mordendo
await p.evaluate(() => {
  document.body.innerHTML = '<div id="h3" style="width:600px;height:300px"></div>';
  const a = PhxAquario.criar(document.getElementById("h3"), { auto: false });
  a.atualizar([{ id: 1, op: "select", tabela: "Blumenau", cor: "verde", ms: 20000 }]);
  for (let i = 0; i < 120; i++) a.passo(1 / 60);
});
const tt = await p.evaluate(() => { const e = document.querySelector("#h3 .aq-rot"); return { txt: e.textContent, tf: getComputedStyle(e).textTransform }; });
ok(tt.txt === "Blumenau" && tt.tf === "none", `dado intacto: "${tt.txt}" text-transform=${tt.tf}`);
ok(erros.length === 0, "sem erro de console " + erros.join("|"));
await b.close();
process.exit(falhas ? 1 : 0);

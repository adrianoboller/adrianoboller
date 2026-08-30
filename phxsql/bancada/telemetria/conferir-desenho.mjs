/* Confere o desenho por MEDIDA, e nao por olhar:
   - rotulo dentro da esfera;
   - esfera dentro da caixa;
   - alvo de clique nunca abaixo do minimo;
   - contraste WCAG do rotulo contra o CORPO da esfera, nos dois temas. */
/* O Playwright nao entra no projeto: a regra de zero dependencia vale, e um
   conferidor de tela nao e motivo para quebra-la. Ele e ferramenta de quem
   confere, achada onde estiver instalada.

       PLAYWRIGHT=/caminho/para/playwright/index.mjs \
       node bancada/telemetria/conferir-desenho.mjs */
const { chromium } = await import(
  process.env.PLAYWRIGHT || "/opt/node22/lib/node_modules/playwright/index.mjs");
import path from "node:path";
const BASE = path.dirname(new URL(import.meta.url).pathname);

const b = await chromium.launch();
const falhas = [];
const contrastes = [];

for (const [larg, alt, nome] of [[1400, 1000, "desktop"], [820, 1100, "tablet"], [390, 900, "celular"]]) {
  for (const tema of ["escuro", "claro"]) {
    const p = await b.newPage({ viewport: { width: larg, height: alt } });
    p.on("pageerror", e => falhas.push(`${nome}/${tema} erro: ${e.message}`));
    await p.goto("file://" + BASE + "/bancada.html");
    if (tema === "claro") await p.evaluate(() => { document.documentElement.dataset.tema = "claro"; });
    for (let i = 0; i < 8; i++) {
      await p.evaluate(i => window.montarCena(i), i);
      await p.waitForTimeout(680);
      const r = await p.evaluate(() => {
        const svg = document.querySelector("#tlmBolhas");
        const vb = svg.getAttribute("viewBox").split(" ").map(Number);
        const cor = n => getComputedStyle(document.documentElement).getPropertyValue(n).trim();
        const rgb = s => {
          const d = document.createElement("div");
          d.style.color = s; document.body.appendChild(d);
          const v = (getComputedStyle(d).color.match(/[\d.]+/g) || [0,0,0]).map(Number);
          d.remove(); return v.slice(0, 3);
        };
        const lum = c => {
          const f = v => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
          return 0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2]);
        };
        const razao = (a, c) => {
          const [x, y] = [lum(a), lum(c)].sort((m, n) => n - m);
          return (x + 0.05) / (y + 0.05);
        };
        const bolhas = [...svg.querySelectorAll("[data-id]")].map(g => {
          const c = g.querySelector(".tlm-c");
          const al = g.querySelector(".tlm-alvo");
          const t = g.querySelector(".tlm-id");
          const s = g.querySelector(".tlm-sub");
          const cx = +c.getAttribute("cx"), cy = +c.getAttribute("cy"), rr = +c.getAttribute("r");
          const medir = el => {
            if (!el.textContent) return null;
            const bb = el.getBBox();
            let pior = 0;
            for (const [x, y] of [[bb.x, bb.y], [bb.x + bb.width, bb.y],
                                  [bb.x, bb.y + bb.height], [bb.x + bb.width, bb.y + bb.height]]) {
              pior = Math.max(pior, Math.hypot(x - cx, y - cy));
            }
            return { texto: el.textContent, pior };
          };
          return { id: g.dataset.id, cx, cy, r: rr, alvo: +al.getAttribute("r"),
                   id1: medir(t), sub: medir(s), tinta: t.getAttribute("fill") };
        });
        // Contraste do rotulo contra o corpo de cada nivel.
        const cs = {};
        for (const [n, v] of [["azul", "--reg"], ["ambar", "--ambar"],
                              ["vermelho", "--vermelho"], ["rosa", "--acao-marcar"]]) {
          const corpo = rgb(cor(v));
          const f = c => { c /= 255; return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4); };
          const l = 0.2126*f(corpo[0]) + 0.7152*f(corpo[1]) + 0.0722*f(corpo[2]);
          const cl = 1.05 / (l + 0.05), es = (l + 0.05) / 0.0555; const tinta = es >= cl ? [11, 13, 22] : [255, 255, 255];
          cs[n] = razao(tinta, corpo);
        }
        return { vb, bolhas, contraste: cs };
      });
      const cena = `${nome}/${tema}/cena${i}`;
      const [, , vw, vh] = r.vb;
      r.bolhas.forEach(x => {
        const S = 7.5;   // alcance do feDropShadow, para a direita e para baixo
        if (x.cx - x.r < -0.6 || x.cx + x.r + S > vw + 0.6
            || x.cy - x.r < -0.6 || x.cy + x.r + S > vh + 0.6)
          falhas.push(`${cena}: ${x.id} (ou a sombra dela) sai da caixa (${x.cx.toFixed(0)},${x.cy.toFixed(0)} r${x.r.toFixed(0)} em ${vw}x${vh})`);
        if (x.alvo < 10.9) falhas.push(`${cena}: ${x.id} tem alvo de clique de ${x.alvo.toFixed(1)} px`);
        for (const t of [x.id1, x.sub]) {
          if (t && t.pior > x.r - 0.5)
            falhas.push(`${cena}: rotulo «${t.texto}» sai da bolha ${x.id} (canto a ${t.pior.toFixed(1)} de um raio ${x.r.toFixed(1)})`);
        }
      });
      if (i === 3) contrastes.push([nome, tema, r.contraste]);
    }
    await p.close();
  }
}
await b.close();
console.log(falhas.length ? "FALHAS (" + falhas.length + "):\n" + falhas.slice(0, 30).join("\n")
                          : "tudo dentro: rotulo na esfera, esfera na caixa, alvo de clique >= 11 px");
const vistos = new Set();
let contrasteRuim = false;
for (const [nome, tema, c] of contrastes) {
  if (vistos.has(tema)) continue;
  vistos.add(tema);
  const pior = Math.min(...Object.values(c));
  if (pior < 4.5) contrasteRuim = true;
  console.log(`contraste do rotulo no tema ${tema}: `
    + Object.entries(c).map(([k, v]) => `${k} ${v.toFixed(2)}`).join("  ")
    + (pior < 4.5 ? "   <<< ABAIXO DE 4,5" : ""));
}

/* O codigo de saida, que faltava.
 *
 * Este conferidor media certo e imprimia certo -- e saia ZERO sempre, inclusive
 * com «FALHAS (7)» na tela. Lido por gente, ele acusava; chamado por uma
 * bateria que soma exit codes, ele mentia verde. E a mesma licao do teste que
 * passa por engano, um andar acima: conferidor que nao sabe reprovar nao
 * confere nada quando ninguem esta olhando. */
process.exitCode = (falhas.length || contrasteRuim) ? 1 : 0;

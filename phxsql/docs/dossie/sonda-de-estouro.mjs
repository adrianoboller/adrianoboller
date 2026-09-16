// A SONDA DO ESTOURO LATERAL -- «esta pagina rola para o lado no telefone?»
//
//     node docs/dossie/sonda-de-estouro.mjs docs/status/status-do-projeto.html 390
//     node docs/dossie/sonda-de-estouro.mjs docs/dossie/status.html 390 captura.png
//
// Irma do `olhar.mjs`: aquele tira a foto, esta MEDE. Existe porque interface
// so se prova exercitando, e porque o defeito que ela acha nao aparece na foto
// -- tres pixels de rolagem lateral nao se ve numa captura de pagina inteira.
//
// ## O que ela responde, e por que a pergunta obvia nao serve
//
// A sonda ingenua -- «que elemento passa da janela?» -- entrega uma LISTA de
// inocentes: toda tabela larga dentro de um `.rolo` com `overflow-x:auto`
// passa da janela de proposito, e quem rola ali e o conteiner, nao a pagina.
// Em 16/09/2026 ela devolveu doze elementos de tabela e NENHUM era a causa.
//
// Esta aqui faz tres perguntas separadas:
//
//   1. a PAGINA rola de verdade? (`scrollTo(9999,0)` e le o `scrollX` -- e o
//      unico criterio que nao se discute)
//   2. quem passa da janela SEM ter ancestral rolante? (o estouro de caixa)
//   3. quem tem `scrollWidth` maior que o proprio `clientWidth`? (o estouro de
//      CONTEUDO, que nao mexe no retangulo do elemento e por isso escapa da
//      pergunta 2 -- foi assim que o culpado apareceu: uma trilha de grade de
//      62 px com 83 px de texto `nowrap` dentro)
//
// Sai != 0 quando a pagina rola para o lado, para caber num portao.
//
// O `playwright` entra pelo caminho ABSOLUTO, como no `olhar.mjs`: nao ha
// node_modules no repositorio, e o pacote global nao se resolve pelo nome --
// o Node cai com ERR_MODULE_NOT_FOUND e, cortado no `tail`, parece que so
// imprimiu a versao dele.
import { chromium } from "/opt/node22/lib/node_modules/playwright/index.mjs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const [, , arquivo, largura, captura] = process.argv;
if (!arquivo) {
  console.error("uso: node docs/dossie/sonda-de-estouro.mjs pagina.html [largura] [captura.png]");
  process.exit(2);
}
const larg = Number(largura || 390);

const navegador = await chromium.launch();
const pagina = await navegador.newPage({ viewport: { width: larg, height: 900 } });
await pagina.goto(pathToFileURL(resolve(arquivo)).href, { waitUntil: "load" });
await pagina.waitForTimeout(500);

const medida = await pagina.evaluate(() => {
  const raiz = document.documentElement;
  const L = raiz.clientWidth;
  const temAncestralRolante = (e) => {
    for (let p = e.parentElement; p; p = p.parentElement) {
      const s = getComputedStyle(p);
      if (s.overflowX === "auto" || s.overflowX === "scroll") return true;
    }
    return false;
  };
  const marca = (e) => e.tagName + (e.className ? "." + String(e.className).slice(0, 34) : "");
  const caixa = [...document.querySelectorAll("body *")]
    .filter((e) => !temAncestralRolante(e) && e.getBoundingClientRect().right > L + 0.5)
    .slice(0, 12)
    .map((e) => ({ quem: marca(e), direita: Math.round(e.getBoundingClientRect().right),
                   texto: (e.textContent || "").trim().slice(0, 40) }));
  const conteudo = [...document.querySelectorAll("body *")]
    .filter((e) => !temAncestralRolante(e) && getComputedStyle(e).overflowX === "visible"
                && e.clientWidth > 0 && e.scrollWidth > e.clientWidth + 1)
    .slice(0, 12)
    .map((e) => ({ quem: marca(e), cabe: e.clientWidth, precisa: e.scrollWidth,
                   texto: (e.textContent || "").trim().slice(0, 40) }));
  window.scrollTo(9999, 0);
  const rolou = window.scrollX;
  window.scrollTo(0, 0);
  return { largura: L, rolagem: raiz.scrollWidth, rolouDeVerdade: rolou, caixa, conteudo };
});

if (captura) await pagina.screenshot({ path: captura, fullPage: true });
await navegador.close();

console.log(JSON.stringify(medida, null, 1));
if (captura) console.log("captura:", captura);
if (medida.rolouDeVerdade > 0) {
  console.error(`\nESTOURO: a pagina rola ${medida.rolouDeVerdade} px para o lado `
    + `a ${larg} px. Olhe a lista «conteudo» primeiro -- trilha de grade com `
    + `medida fixa e texto \`nowrap\` e o suspeito que a lista «caixa» nao mostra.`);
  process.exit(1);
}
console.log(`\nSem rolagem lateral a ${larg} px.`);

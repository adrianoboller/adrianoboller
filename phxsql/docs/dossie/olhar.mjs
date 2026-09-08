// Uma OLHADA numa pagina local, antes de publicar: captura inteira em PNG.
//
//     node docs/dossie/olhar.mjs docs/dossie/status.html /tmp/status.png
//
// Existe porque interface so se prova exercitando -- e porque o CSS global
// morde todo componente novo sem aparecer no codigo. O `playwright` entra
// pelo caminho ABSOLUTO, como no `capturar-dossie.mjs`: nao ha node_modules
// no repositorio, e o pacote global nao se resolve pelo nome -- o Node cai
// com ERR_MODULE_NOT_FOUND e, cortado no `tail`, parece que so imprimiu a
// versao dele.
import { chromium } from "/opt/node22/lib/node_modules/playwright/index.mjs";
import { resolve } from "node:path";

const [, , arquivo, saida] = process.argv;
if (!arquivo || !saida) {
  console.error("uso: node docs/dossie/olhar.mjs pagina.html captura.png");
  process.exit(2);
}
const navegador = await chromium.launch();
const pagina = await navegador.newPage({ viewport: { width: 1180, height: 900 } });
await pagina.goto("file://" + resolve(arquivo), { waitUntil: "load" });
await pagina.waitForTimeout(600);
await pagina.screenshot({ path: saida, fullPage: true });
await navegador.close();
console.log("captura:", saida);

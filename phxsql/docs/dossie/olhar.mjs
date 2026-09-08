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
import { resolve, join } from "node:path";
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";

const [, , arquivo, saida] = process.argv;
if (!arquivo || !saida) {
  console.error("uso: node docs/dossie/olhar.mjs pagina.html captura.png");
  process.exit(2);
}

// A pagina com o charset declarado -- a publicada nao traz o dela (o embrulho
// do visualizador o poe ao publicar), e por file:// o Chromium ADIVINHA a
// codificacao: em 08/09/2026 adivinhou Latin-1 em tres de quatro paginas, sem
// erro nenhum. Mesma receita do pdf.mjs, que e o irmao.
function comCharset(caminho) {
  const html = readFileSync(caminho, "utf8");
  if (/<meta[^>]+charset/i.test(html)) {
    return { url: pathToFileURL(resolve(caminho)).href, limpar() {} };
  }
  const dir = mkdtempSync(join(tmpdir(), "phx-olhar-"));
  const copia = join(dir, "pagina.html");
  writeFileSync(copia, '<meta charset="utf-8">\n' + html);
  return { url: pathToFileURL(copia).href,
           limpar() { rmSync(dir, { recursive: true, force: true }); } };
}

const alvo = comCharset(arquivo);
const navegador = await chromium.launch();
const pagina = await navegador.newPage({ viewport: { width: 1180, height: 900 } });
await pagina.goto(alvo.url, { waitUntil: "load" });
await pagina.waitForTimeout(600);
await pagina.screenshot({ path: saida, fullPage: true });
await navegador.close();
alvo.limpar();
console.log("captura:", saida);

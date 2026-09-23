// Prova real do botao «baixar» do dossie -- pedido 327.
//
//     node docs/dossie/prova-do-botao-de-baixar.mjs            # acha o dossie sozinho
//     node docs/dossie/prova-do-botao-de-baixar.mjs pagina.html
//
// POR QUE ELA EXISTE. O botao chama `window.print()`, e o `LEIA-ME.md` desta
// pasta afirmava, desde 07/09/2026, que «a caixa de impressao e do navegador,
// entao ela abre». Isso era diagnostico PLAUSIVEL e nunca medido: o
// visualizador de artefatos hospeda a pagina num `iframe` com `sandbox`, e um
// quadro sem a palavra `allow-modals` faz o Chromium IGNORAR o `print()` --
// sem excecao, sem retorno diferente, so uma linha no console. O botao parecia
// funcionar e nao fazia nada, que e o pior modo de falhar desta casa.
//
// O QUE ELA MEDE, e sao duas coisas diferentes:
//
//   1. o MECANISMO -- `beforeprint` dispara quando a caixa abre e nao dispara
//      quando o sandbox engole a chamada. Medido em 23/09/2026 nos tres
//      embrulhos: sem sandbox 1, sem `allow-modals` 0, com `allow-modals` 1.
//      E o sinal DIRETO; um cronometro em volta do `print()` nao serve, porque
//      em modo sem cabeca ele volta na hora nos tres casos (0,6 a 1,5 ms).
//
//   2. o CONSERTO, nos dois sentidos -- com o defeito reposto (quadro sem
//      `allow-modals`) o aviso `#semCaixa` tem de APARECER; sem o defeito
//      (sem sandbox, e com `allow-modals`) tem de continuar escondido. Prova
//      que so passa num sentido e prova que passa por engano.
//
// O QUE ELA NAO MEDE, e vale dizer: quais palavras de `sandbox` o visualizador
// de artefatos concede de verdade. Isso nao se alcanca daqui. Por isso o
// conserto nao adivinha o embrulho -- ele pergunta ao proprio navegador se a
// caixa abriu, e so fala quando ela nao abriu.
//
// O `playwright` entra pelo caminho ABSOLUTO, como no `olhar.mjs`: nao ha
// `node_modules` no repositorio e o pacote global nao se resolve pelo nome.
import { chromium } from "/opt/node22/lib/node_modules/playwright/index.mjs";
import { writeFileSync, mkdtempSync, rmSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

// O nome do dossie muda a cada refacao, e so existe um por vez -- a mesma
// varredura do `dossie_da_pasta.py`, pelo mesmo motivo: padrao digitado aqui
// envelhece calado na proxima refacao.
function acharODossie() {
  const pasta = dirname(fileURLToPath(import.meta.url));
  const achados = readdirSync(pasta).filter((n) => /^dossie-phxsql-.*\.html$/.test(n));
  if (achados.length !== 1) {
    console.error(`esperava UM dossie-phxsql-*.html em ${pasta}, achei ${achados.length}`);
    process.exit(2);
  }
  return join(pasta, achados[0]);
}

const alvo = resolve(process.argv[2] || acharODossie());
const url = pathToFileURL(alvo).href;

// `allow-popups` entra nos dois embrulhos com sandbox para que a unica
// diferenca medida entre eles seja `allow-modals`. Variavel de mais numa
// medicao comparativa e como bancada que compara trabalho diferente.
const CASOS = [
  { rotulo: "sem sandbox", sandbox: null, esperaCaixa: true },
  { rotulo: "sandbox SEM allow-modals", sandbox: "allow-scripts allow-popups", esperaCaixa: false },
  { rotulo: "sandbox COM allow-modals", sandbox: "allow-scripts allow-popups allow-modals", esperaCaixa: true },
];

const dir = mkdtempSync(join(tmpdir(), "prova-baixar-"));
const nav = await chromium.launch();
let falhas = 0;

console.log(`prova do botao «baixar» -- ${alvo}\n`);
for (const caso of CASOS) {
  const hosped = join(dir, "host.html");
  writeFileSync(
    hosped,
    `<!doctype html><meta charset="utf-8"><title>embrulho</title>` +
      `<iframe ${caso.sandbox ? `sandbox="${caso.sandbox}"` : ""} src="${url}" ` +
      `style="width:1100px;height:800px;border:0"></iframe>`,
  );
  const pag = await nav.newPage();
  const consola = [];
  pag.on("console", (m) => consola.push(m.text()));
  await pag.goto(pathToFileURL(hosped).href, { waitUntil: "load" });

  const quadro = pag.frames().find((f) => f.url().startsWith("file:") && f !== pag.mainFrame());
  if (!quadro) {
    console.log(`  ${caso.rotulo}: FALHOU -- o quadro nao carregou`);
    falhas++;
    await pag.close();
    continue;
  }

  const medida = await quadro.evaluate(() => {
    const bt = document.getElementById("btBaixar");
    if (!bt) return { semBotao: true };
    let abriu = 0;
    window.addEventListener("beforeprint", () => abriu++);
    const t = performance.now();
    bt.click();
    return { beforeprint: abriu, ms: +(performance.now() - t).toFixed(2) };
  });
  if (medida.semBotao) {
    console.log(`  ${caso.rotulo}: FALHOU -- a pagina nao tem #btBaixar`);
    falhas++;
    await pag.close();
    continue;
  }

  // 250 ms e' o prazo do proprio detector da pagina; 600 da folga para ele.
  await pag.waitForTimeout(600);
  const avisoVisivel = await quadro.evaluate(() => {
    const e = document.getElementById("semCaixa");
    return !!e && !e.hidden && e.getClientRects().length > 0;
  });

  const caixaAbriu = medida.beforeprint > 0;
  const ignorado = consola.some((t) => /Ignored call to 'print\(\)'/.test(t));
  const okCaixa = caixaAbriu === caso.esperaCaixa;
  const okAviso = avisoVisivel === !caso.esperaCaixa;
  if (!okCaixa || !okAviso) falhas++;

  console.log(
    `  ${caso.rotulo.padEnd(26)} beforeprint=${medida.beforeprint} ` +
      `print() voltou em ${String(medida.ms).padStart(5)} ms  ` +
      `console acusou ignorado=${ignorado ? "sim" : "nao"}`,
  );
  console.log(
    `  ${"".padEnd(26)} caixa abriu=${String(caixaAbriu).padEnd(5)} ` +
      `(esperado ${String(caso.esperaCaixa).padEnd(5)}) ${okCaixa ? "OK" : "FALHOU"}   ` +
      `aviso visivel=${String(avisoVisivel).padEnd(5)} ` +
      `(esperado ${String(!caso.esperaCaixa).padEnd(5)}) ${okAviso ? "OK" : "FALHOU"}\n`,
  );
  await pag.close();
}

await nav.close();
rmSync(dir, { recursive: true, force: true });

if (falhas) {
  console.log(`VERMELHO: ${falhas} caso(s) fora do esperado.`);
  process.exit(1);
}
console.log("VERDE: o botao imprime quando pode, e DIZ que nao imprimiu quando nao pode.");

// Exercita a tela do PhxZip no Chromium contra o servidor REAL (`phxzipweb`),
// e nao contra o falso (pedido 454, fatias Z6 a Z8).
//
//     cargo build -p phxzip-web
//     node testes-web/phxzip/real.mjs [--bin target/debug/phxzipweb] [--saida DIR]
//
// O falso prova a tela; este prova a PORTA: o que a tela manda chega ao motor
// e volta como o contrato diz -- e o que a extracao grava e conferido NO
// DISCO, byte a byte, e nao pelo que a tela anuncia. Sobe o servidor numa
// porta so dele, com `--pasta` numa pasta temporaria `0755` e `--envio` baixo
// (o 413 se prova sem mandar 256 MiB), e o derruba pelo PID do filho.
import { chromium } from "/opt/node22/lib/node_modules/playwright/index.mjs";
import { spawn } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, chmodSync, readdirSync, statSync } from "node:fs";
import { join, dirname, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, "../..");
const arg = nome => { const i = process.argv.indexOf(nome); return i > 0 ? process.argv[i + 1] : null; };
const BIN = resolve(arg("--bin") || join(RAIZ, "target/debug/phxzipweb"));
const SAIDA = resolve(arg("--saida") || join(RAIZ, "target/phxzip-web-real"));
const PORTA = Number(arg("--porta")) || 7795;
const ENVIO = 4 << 20;
const BASE = `http://127.0.0.1:${PORTA}`;
const TMP = mkdtempSync(join(tmpdir(), "phxzip-real-"));
const PASTA = join(TMP, "extraido");
mkdirSync(PASTA);
chmodSync(PASTA, 0o755);
mkdirSync(SAIDA, { recursive: true });
const SENHA = "s3nh@-Blumenau-2026";

const resultados = [];
async function caso(nome, corpo) {
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
function exigir(c, msg) { if (!c) throw new Error(msg); }
const captura = (page, nome) => page.screenshot({ path: join(SAIDA, nome + ".png"), fullPage: true });

function subir() {
  return new Promise((ok, falha) => {
    const p = spawn(BIN, ["--porta", String(PORTA), "--pasta", PASTA, "--envio", String(ENVIO)],
      { stdio: ["ignore", "pipe", "pipe"] });
    let err = "";
    p.stderr.on("data", d => { err += d; });
    p.stdout.on("data", d => { if (String(d).includes("PhxZipWeb em")) ok(p); });
    p.on("exit", c => falha(new Error(`phxzipweb saiu (${c}): ${err.slice(-400)}`)));
    setTimeout(() => falha(new Error("phxzipweb nao subiu em 8 s")), 8000);
  });
}

// Os arquivos, com acento no conteudo e no nome: o dado chega byte a byte.
const RELATORIO = Buffer.from("Relatório anual — Blumenau, SC. Ação e coração.\n".repeat(300));
const DADOS = Buffer.from(JSON.stringify({ cidade: "Blumenau", uf: "SC", itens: [1, 2, 3] }, null, 2) + "\n");
const GRANDE = join(TMP, "grande.bin");
writeFileSync(GRANDE, Buffer.alloc(ENVIO + (1 << 20), 0x5a));

const ocioso = page => page.waitForFunction(() => !document.body.classList.contains("ocupado"), null, { timeout: 60000 });

console.log(`PhxZip — a tela contra o servidor REAL (${new Date().toISOString()})`);
const servidor = await subir();
const navegador = await chromium.launch();
const ctx = await navegador.newContext({ acceptDownloads: true, viewport: { width: 1280, height: 900 } });
const page = await ctx.newPage();
const erros = [];
page.on("pageerror", e => erros.push(String(e)));
page.on("console", m => { if (m.type() === "error") erros.push(m.text()); });
let pacote = null;

try {
  await page.goto(BASE + "/");
  await page.waitForFunction(() => !document.body.classList.contains("carregando"));

  await caso("estado-e-idioma", async () => {
    const e = await page.evaluate(() => est.estado);
    exigir(e && e.ok && e.limites.envio === 4194304, `estado: ${JSON.stringify(e && e.limites)}`);
    exigir(e.extrair_na_pasta === true, "o estado nao declarou a pasta");
    await page.click("#btIdioma");
    await page.click('#menuIdiomas li[data-idi="Alemao"]');
    await page.waitForFunction(() => document.documentElement.lang === "de");
    const rotulo = await page.textContent("#abaCompactar");
    exigir(rotulo.includes("Komprimieren"), `aba em alemao: ${rotulo}`);
    await captura(page, "01-alemao");
    await page.click("#btIdioma");
    await page.click('#menuIdiomas li[data-idi="Portugues"]');
    await page.waitForFunction(() => document.documentElement.lang === "pt-BR");
    exigir((await page.textContent("#abaCompactar")).includes("Compactar"), "voltou ao portugues?");
    // Com `--pasta`, a garantia do rodape nao pode dizer «nao grava nada».
    const garantia = await page.textContent("#garantiaDisco");
    exigir(/pasta que recebeu/.test(garantia), `rodape com --pasta: ${garantia}`);
    return `«${rotulo.trim()}» e de volta`;
  });

  await caso("compactar-com-senha", async () => {
    await page.click("#abaCompactar");
    // Por BUFFER, e nao por caminho: medido aqui, o Playwright descarta CALADO
    // o arquivo cujo caminho tem acento (`relatório.txt` some e so
    // `dados.json` chega a tela). Por buffer, o nome com acento chega inteiro.
    await page.setInputFiles("#inArquivos", [
      { name: "relatório.txt", mimeType: "text/plain", buffer: RELATORIO },
      { name: "dados.json", mimeType: "application/json", buffer: DADOS },
    ]);
    const lista = await page.$$eval("#gradeCompactar tbody .nome", ns => ns.map(n => n.textContent));
    exigir(lista.length === 2, `a lista de compactar tem ${lista.join(" | ")}`);
    await page.fill("#inNomePacote", "relatorio-real");
    await page.fill("#inSenha", SENHA);
    await page.fill("#inSenha2", SENHA);
    const dl = page.waitForEvent("download", { timeout: 60000 });
    await page.click("#btCompactar");
    const d = await dl;
    pacote = join(TMP, d.suggestedFilename());
    await d.saveAs(pacote);
    const b = readFileSync(pacote);
    exigir(b.subarray(0, 6).equals(Buffer.from([0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c])), "o download nao e 7z");
    exigir(d.suggestedFilename() === "relatorio-real.7z", `nome: ${d.suggestedFilename()}`);
    await page.waitForSelector('#resultadoCompactar [data-estado="ok"]');
    await captura(page, "02-compactado");
    return `${d.suggestedFilename()}: ${b.length} bytes de ${RELATORIO.length + DADOS.length}`;
  });

  await caso("abrir-com-senha-e-listar", async () => {
    await page.click("#abaAbrir");
    await page.setInputFiles("#inPacote", pacote);
    await ocioso(page);
    await page.waitForSelector("#avisoAbrir [data-erro]");
    const pediu = await page.getAttribute("#avisoAbrir [data-erro]", "data-erro");
    exigir(pediu === "SENHA_AUSENTE", `sem senha veio ${pediu}`);
    await page.fill("#inSenhaPacote", SENHA);
    await page.click("#btAbrirComSenha");
    await ocioso(page);
    await page.waitForSelector("#listaPacote:not([hidden])");
    const nomes = await page.$$eval("#gradePacote tbody .nome", ns => ns.map(n => n.textContent));
    exigir(nomes.includes("relatório.txt") && nomes.includes("dados.json"), `lista: ${nomes}`);
    await captura(page, "03-listado");
    return nomes.join(", ");
  });

  await caso("testar", async () => {
    await page.click("#btTestar");
    await ocioso(page);
    await page.waitForSelector('#resultadoTeste [data-estado="ok"]');
    return (await page.textContent("#resultadoTeste")).trim();
  });

  await caso("baixar-uma-entrada", async () => {
    const linha = page.locator("#gradePacote tbody tr", { hasText: "dados.json" });
    const dl = page.waitForEvent("download", { timeout: 60000 });
    await linha.locator('[data-acao="baixar"]').click();
    const d = await dl;
    const alvo = join(TMP, "baixado-" + d.suggestedFilename());
    await d.saveAs(alvo);
    exigir(readFileSync(alvo).equals(DADOS), "o baixado difere do original");
    return `${d.suggestedFilename()} = original, byte a byte`;
  });

  await caso("extrair-na-pasta-e-conferir-no-disco", async () => {
    exigir(await page.isVisible("#btExtrairNaPasta"), "o botao da pasta nao apareceu");
    await page.click("#btExtrairNaPasta");
    await ocioso(page);
    await page.waitForFunction(() => /\d/.test(document.querySelector("#resultadoTeste").textContent)
      && document.querySelectorAll('#resultadoTeste [data-estado="ok"]').length === 2);
    const no_disco = readdirSync(PASTA).sort();
    exigir(JSON.stringify(no_disco) === JSON.stringify(["dados.json", "relatório.txt"]), `no disco: ${no_disco}`);
    exigir(readFileSync(join(PASTA, "relatório.txt")).equals(RELATORIO), "relatório.txt difere");
    exigir(readFileSync(join(PASTA, "dados.json")).equals(DADOS), "dados.json difere");
    const modo = (statSync(join(PASTA, "dados.json")).mode & 0o777).toString(8);
    exigir(modo === "600", `cifrado devia nascer 0600, nasceu ${modo}`);
    await captura(page, "04-extraido-na-pasta");
    return `${no_disco.join(", ")} iguais ao original, modo ${modo}`;
  });

  await caso("extrair-de-novo-recusa-sem-sobrescrever", async () => {
    await page.click("#btExtrairNaPasta");
    await ocioso(page);
    await page.waitForSelector('#avisoAbrir [data-erro="DESTINO_INSEGURO"]');
    const texto = (await page.textContent("#avisoAbrir")).trim();
    exigir(!texto.includes("zip.erro"), `chave crua na tela: ${texto}`);
    exigir(readFileSync(join(PASTA, "dados.json")).equals(DADOS), "sobrescreveu");
    await captura(page, "05-destino-inseguro");
    return texto.slice(0, 90);
  });

  await caso("413-do-navegador-legivel", async () => {
    // A tela confere o teto antes de mandar; aqui ela e enganada de
    // proposito para o pedido grande CHEGAR ao servidor -- e o 413 dele, com
    // o dreno, tem de aparecer como cartao, e nao como «a conexao caiu».
    await page.click("#btFechar");
    await page.click("#abaCompactar");
    await page.click("#btLimpar");
    await page.evaluate(() => { est.estado.limites.envio = 1e12; });
    await page.setInputFiles("#inArquivos", [GRANDE]);
    await page.fill("#inNomePacote", "grande");
    await page.click("#btCompactar");
    await ocioso(page);
    await page.waitForSelector("#resultadoCompactar [data-erro]");
    const cod = await page.getAttribute("#resultadoCompactar [data-erro]", "data-erro");
    const texto = (await page.textContent("#resultadoCompactar")).trim();
    exigir(cod === "GRANDE_DEMAIS", `veio ${cod}: ${texto}`);
    exigir(!texto.includes("zip.erro"), `chave crua: ${texto}`);
    await captura(page, "06-413-legivel");
    return `${cod}: ${texto.slice(0, 100)}`;
  });

  await caso("sem-erro-no-console", async () => {
    // O 413 do caso de cima aparece no console como recurso que falhou -- e o
    // que o navegador diz de toda resposta 4xx, nao erro da pagina.
    const reais = erros.filter(e => !/status of 4\d\d/.test(e));
    exigir(!reais.length, reais.join(" | "));
    return `${erros.length} aviso(s) de 4xx do proprio navegador`;
  });
} finally {
  await navegador.close();
  servidor.kill();
}

const falhas = resultados.filter(r => !r.ok).length;
writeFileSync(join(SAIDA, "resultado.json"), JSON.stringify({ quando: new Date().toISOString(), resultados }, null, 1));
console.log(`\n${resultados.length - falhas}/${resultados.length} casos; capturas em ${SAIDA}`);
process.exit(falhas ? 1 : 0);

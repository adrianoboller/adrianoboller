// Abre a tela do PhxSql e a SEGURA aberta ate receber SIGTERM.
//
//     node bancada/vagas-da-tela/abrir-e-segurar.mjs http://127.0.0.1:PORTA/
//
// Existe porque o `playwright` desta casa e o do Node, nao o do Python -- e
// entra pelo caminho ABSOLUTO, como no `olhar.mjs`: nao ha node_modules no
// repositorio, e o pacote global nao se resolve pelo nome.
//
// Ele imprime PRONTO numa linha quando a pagina carregou, para quem chamou
// saber que pode comecar a amostrar. Sem isso a medida pegaria a pagina
// ainda subindo, e o numero sairia menor do que a verdade.
import { chromium } from "/opt/node22/lib/node_modules/playwright/index.mjs";

const url = process.argv[2];
if (!url) { console.error("uso: abrir-e-segurar.mjs <url>"); process.exit(2); }

const nav = await chromium.launch();
const pag = await nav.newPage();
await pag.goto(url, { waitUntil: "load", timeout: 60000 });
console.log("PRONTO");

const sair = async () => { await nav.close(); process.exit(0); };
process.on("SIGTERM", sair);
process.on("SIGINT", sair);
await new Promise(() => {});

// O selo de farol e o botao de marcar/tirar farol na janela de mesa,
// exercitados no navegador contra o `phxvpn mesa` DE VERDADE (servidor real,
// mesmo motor de `phxvpn p2p farol`). A rede foi semeada por
// `cargo run --example semear_farol` -- o mesmo atalho do teste
// `mesa::testes::farol_pela_api_dono_autoriza_membro_consente` (admitir o
// segundo membro no rol A MAO, sem montar um netns so para isto).
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';

const URL = process.env.URL, SAIDA = process.env.SAIDA;
const LARGURA = Number(process.env.LARGURA || 900), ALTURA = Number(process.env.ALTURA || 900);
const PREFIXO = process.env.PREFIXO || 'farol';

const b = await chromium.launch();
const p = await b.newPage({ viewport: { width: LARGURA, height: ALTURA } });
const erros = [];
p.on('pageerror', e => erros.push(e.message));
p.on('console', m => { if (m.type() === 'error') erros.push(m.text()); });

const r = {};
await p.goto(URL);
await p.waitForSelector('#lista .rede', { timeout: 15000 });

// --- O logo novo (pedido 6): mesma rota PNG do painel.
r.favicon = await p.$eval('link[rel=icon]', l => l.href);
const logo128 = await p.request.get(new globalThis.URL('/logo-128.png', URL).toString());
r.logo_status = logo128.status();
r.logo_bytes = (await logo128.body()).length;

// --- O selo discreto de farol na linha do membro (pedido 5/4): a rede
// "Farol" foi semeada com o membro em 10.78.30.2 ja marcado.
await p.click('.rede .nome'); // abre a lista de membros
await p.waitForSelector('.membro');
r.selo_visivel = await p.isVisible('.selo-farol');
r.texto_caminho = (await p.textContent('.membro .caminho'))?.trim();
await p.screenshot({ path: `${SAIDA}/${PREFIXO}-selo.png`, fullPage: true });

// --- O botao do DONO: "Tirar farol" (ja esta marcado) -- clique abre o
// dialogo sem endereco (nao precisa para tirar).
const linhaMembro = p.locator('.membro', { hasText: '10.78.30.2' });
await linhaMembro.getByRole('button', { name: 'Tirar farol' }).click();
await p.waitForSelector('#d-farol[open]');
r.campo_endereco_escondido_ao_tirar = await p.isHidden('#d-farol [name=endereco]');
await p.screenshot({ path: `${SAIDA}/${PREFIXO}-dialogo-tirar.png`, fullPage: true });
await p.click('#f-farol button[type=submit]');
await p.waitForFunction(() => !document.querySelector('.selo-farol'), null, { timeout: 15000 });
r.selo_sumiu_depois_de_tirar = !(await p.isVisible('.selo-farol'));
await p.screenshot({ path: `${SAIDA}/${PREFIXO}-depois-de-tirar.png`, fullPage: true });

// --- Marca de novo, agora PRECISANDO do endereco.
await linhaMembro.getByRole('button', { name: 'Marcar farol' }).click();
await p.waitForSelector('#d-farol[open]');
r.campo_endereco_visivel_ao_marcar = await p.isVisible('#d-farol [name=endereco]');
await p.fill('#d-farol [name=endereco]', '203.0.113.9:51820');
await p.click('#f-farol button[type=submit]');
await p.waitForSelector('.selo-farol', { timeout: 15000 });
r.selo_voltou = await p.isVisible('.selo-farol');
await p.screenshot({ path: `${SAIDA}/${PREFIXO}-depois-de-marcar.png`, fullPage: true });

r.erros_de_console = erros;
console.log(JSON.stringify(r, null, 1));
if (erros.length) process.exitCode = 1;
await b.close();

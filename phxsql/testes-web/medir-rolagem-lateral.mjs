#!/usr/bin/env node
/* MEDE a rolagem lateral do corpo em 360/768/1280/1920 px, tela por tela
 * (pedido 771, a metade «responsividade»). So mede e relata: nao reprova.
 *
 *     PLAYWRIGHT_BROWSERS_PATH=/opt/pw-browsers \
 *       node phxsql/testes-web/medir-rolagem-lateral.mjs [--porta 6480]
 *
 * Por que separado do caso `responsivo`: ele prova 390/820/1600/3440/5120 em
 * sete telas e REPROVA; esta medida e o retrato das quatro larguras que o 771
 * nomeia em TODA tela dos menus, para decidir o tamanho do conserto antes de
 * fazê-lo. «Rola de lado» e `documentElement.scrollWidth > innerWidth`, a
 * mesma regua do caso. */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { subir } from './servidor.mjs';
import { entrar, cenario } from './apoio.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const RAIZ = resolve(AQUI, '..');
const i = process.argv.indexOf('--porta');
const porta = i > 0 ? Number(process.argv[i + 1]) : 6480;
const LARGURAS = (process.env.PHX_LARGURAS || '360,768,1280,1920').split(',').map(Number);
/* Os mesmos itens que o `passeio` deixa de fora, pelo mesmo motivo: abrem
   algo POR CIMA da pagina e travariam os cliques seguintes. */
const FORA = new Set(['Sair', 'Soltar esta tela numa janela', 'Backup', 'Backup agora…',
  'Conferir um backup…']);

/* Passo com prazo: uma tela que pendura (dialogo, recarga) vira «sem medida»
   e o retrato segue, em vez de a medida inteira morrer calada. */
const comPrazo = (p, ms = 8000) => Promise.race([p,
  new Promise((_, nao) => setTimeout(() => nao(new Error('prazo')), ms))]);

const servidor = await subir({ phxsqld: join(RAIZ, 'target', 'release', 'phxsqld'),
  portaDados: porta, portaWeb: porta + 1, log: () => {} });
const nav = await chromium.launch();
const resultado = {};
try {
  for (const w of LARGURAS) {
    // Uma aba NOVA por largura: o passeio troca regioes e abre abas, e a
    // corrida com uma aba so para as quatro larguras pendurou na segunda.
    // Entra a 1600 (a arvore some no celular e o `entrar` espera por ela)
    // e so entao estreita.
    const ctx = await nav.newContext({ viewport: { width: 1600, height: 900 } });
    const page = await ctx.newPage();
    await entrar(page, servidor.url);
    await cenario(page, 'medeRolagem', 'clientes');
    await page.setViewportSize({ width: w, height: 900 });
    const menus = await page.evaluate(() =>
      [...document.querySelectorAll('.menubar .menu')].map(m => ({
        m: m.dataset.m, titulo: m.querySelector('.titulo').textContent.trim(),
        itens: [...m.querySelectorAll('.item')].map(b => ({
          i: b.dataset.i, rot: b.querySelector('.rot').textContent.trim() })),
      })));
    const rolam = [];
    const pulos = [];
    let medidas = 0;
    for (const menu of menus) {
      for (const item of menu.itens) {
        if (FORA.has(item.rot)) continue;
        if (await comPrazo(page.evaluate(() => !est.atual)).catch(() => false)) {
          const no = page.locator('#arvore .no.tab[data-db="medeRolagem"][data-tab="clientes"]');
          if (await no.count()) await no.first().click({ timeout: 3000 }).catch(() => {});
          await page.waitForTimeout(150);
        }
        // Em largura de celular o menu pode estar recolhido: abre pelo
        // proprio codigo da pagina, que e o que o clique faria.
        const ok = await comPrazo(page.evaluate(([m, i]) => {
          const bt = document.querySelector(`.menubar .item[data-m="${m}"][data-i="${i}"]`);
          if (!bt || bt.disabled) return false;
          bt.click();
          return true;
        }, [menu.m, item.i])).catch(() => false);
        if (!ok) { pulos.push(`${menu.titulo} › ${item.rot}`); continue; }
        await page.waitForTimeout(250);
        // Fecha dialogo proprio que tenha ficado por cima (o Esc e o que a
        // pessoa faria).
        await comPrazo(page.keyboard.press('Escape'), 3000).catch(() => {});
        if (process.env.PHX_DIZ) console.error(`  ${w} ${menu.titulo} › ${item.rot}`);
        const r = await comPrazo(page.evaluate(() => ({
          rola: document.documentElement.scrollWidth, vista: window.innerWidth })))
          .catch(() => null);
        if (!r) { pulos.push(`${menu.titulo} › ${item.rot}`); continue; }
        medidas++;
        if (r.rola > r.vista + 1) rolam.push(`${menu.titulo} › ${item.rot} (${r.rola}px)`);
      }
    }
    resultado[w] = { medidas, rolam, pulos };
    console.log(`${w}px: ${rolam.length} de ${medidas} telas rolam de lado`
      + (pulos.length ? ` (${pulos.length} sem medida)` : ''));
    for (const t of rolam) console.log(`    ${t}`);
    await ctx.close();
  }
} finally {
  await nav.close();
  await servidor.derrubar();
}

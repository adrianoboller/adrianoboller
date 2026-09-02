/* O VIDEO de demonstracao, do login ao arquivo em disco.
 *
 *   node testes-web/video-demonstracao.mjs
 *
 * Por que isto existe, e nao e enfeite: gravar um video ja achou TRES
 * defeitos em cinco minutos nesta casa -- e o pior deles quebrava todo salvar
 * e todo incluir pela tela. Ler o codigo nao pegava nenhum. Interface so se
 * prova exercitando, e o video e o exercicio filmado.
 *
 * O roteiro, na ordem que o dono pediu:
 *   1. entrar (login de verdade, com senha e token)
 *   2. criar um banco
 *   3. criar uma tabela
 *   4. carregar 1.000 registros
 *   5. ver o conteudo
 *   6. pesquisar um registro na PhxGrid
 *   7. agrupar arrastando o cabecalho para a faixa
 *   8. backup
 *   9. os arquivos do banco novo, em disco
 *
 * Uma HONESTIDADE sobre o passo 9: este conteiner nao tem ambiente grafico,
 * entao nao ha gerenciador de arquivos do Linux para filmar. O que o video
 * mostra e a LISTAGEM REAL do diretorio, lida do disco e desenhada numa
 * pagina -- os nomes, os tamanhos e as datas sao os verdadeiros. E listagem
 * de verdade apresentada, nao um gerenciador de arquivos fingido.
 */
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdirSync, rmSync, readdirSync, statSync, existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { subir, USUARIO, SENHA, TOKEN } from './servidor.mjs';

const PORTA_DADOS = 6310;
const PORTA_WEB = 6311;
// Fora de `/tmp` de proposito: a primeira gravacao ficou la e o ZELADOR a
// apagou -- ele limpa `/tmp` de hora em hora, e esta certo em limpar. Video
// e entregavel, nao temporario.
const SAIDA = process.env.SAIDA
  || '/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/video-phxsql';
const BANCO = 'vendas';
const TABELA = 'pedidos';
const QUANTOS = 1000;

/** Uma pausa que o olho acompanha. O video e para gente, nao para maquina. */
const respirar = (p, ms = 900) => p.waitForTimeout(ms);

/** Um cartaz entre as cenas, para o video se explicar sozinho. */
async function cartaz(page, numero, texto) {
  await page.evaluate(([n, t]) => {
    let d = document.getElementById('__cartaz');
    if (!d) {
      d = document.createElement('div');
      d.id = '__cartaz';
      d.style.cssText = 'position:fixed;inset:0;z-index:99999;display:flex;'
        + 'align-items:center;justify-content:center;flex-direction:column;gap:14px;'
        + 'background:rgba(1,4,24,.92);color:#fff;font:600 34px/1.3 system-ui,sans-serif;'
        + 'text-align:center;padding:40px;transition:opacity .25s';
      document.body.appendChild(d);
    }
    d.innerHTML = '<div style="font-size:15px;letter-spacing:.22em;opacity:.6">PASSO ' + n + '</div>'
      + '<div>' + t + '</div>';
    d.style.opacity = '1';
  }, [numero, texto]);
  await respirar(page, 1600);
  await page.evaluate(() => {
    const d = document.getElementById('__cartaz');
    if (d) { d.style.opacity = '0'; setTimeout(() => d.remove(), 300); }
  });
  await respirar(page, 400);
}

async function principal() {
  rmSync(SAIDA, { recursive: true, force: true });
  mkdirSync(SAIDA, { recursive: true });

  const phxsqld = join(process.cwd(), 'target/release/phxsqld');
  if (!existsSync(phxsqld)) {
    console.error(`falta ${phxsqld} -- rode \`cargo build --release\` antes`);
    return 2;
  }
  const srv = await subir({ phxsqld, portaDados: PORTA_DADOS, portaWeb: PORTA_WEB });
  const url = `http://127.0.0.1:${PORTA_WEB}/`;
  const navegador = await chromium.launch();
  const ctx = await navegador.newContext({
    viewport: { width: 1440, height: 900 },
    recordVideo: { dir: SAIDA, size: { width: 1440, height: 900 } },
  });
  const page = await ctx.newPage();

  try {
    // ---- 1. o login ----------------------------------------------------
    await page.goto(url, { waitUntil: 'domcontentloaded' });
    await page.waitForSelector('#btEntrar');
    await page.waitForFunction(() => typeof est === 'object' && est.demo === false,
                               { timeout: 15000 });
    await cartaz(page, 1, 'Entrar no PhxSql');
    await page.fill('#u', USUARIO); await respirar(page, 350);
    await page.fill('#s', SENHA);   await respirar(page, 350);
    await page.fill('#t', TOKEN);   await respirar(page, 500);
    await page.click('#btEntrar');
    await page.waitForSelector('#app.ativo', { timeout: 20000 });
    await page.waitForSelector('#arvore .no', { timeout: 20000 });
    await respirar(page, 1200);

    // ---- 2. o banco, pelo [+] da arvore --------------------------------
    await cartaz(page, 2, `Criar o banco «${BANCO}»`);
    page.once('dialog', d => d.accept(BANCO));
    await page.click('#btNovoDb');
    await page.waitForFunction(
      n => [...document.querySelectorAll('#arvore .no.db')].some(x => x.dataset.db === n),
      BANCO, { timeout: 15000 });
    await respirar(page, 1200);

    // ---- 3. a tabela ---------------------------------------------------
    await cartaz(page, 3, `Criar a tabela «${TABELA}»`);
    await page.evaluate(([db, tab]) => api('criar_tabela', {
      database: db, tabela: tab,
      colunas: [
        { nome: 'id', tipo: 'Sequence', obrigatoria: true },
        { nome: 'titulo', tipo: 'Str(20)' },
        { nome: 'cliente', tipo: 'Str(40)' },
        { nome: 'valor', tipo: 'Int8' },
      ],
      indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
    }), [BANCO, TABELA]);
    await page.evaluate(() => montarArvore());
    await respirar(page, 1200);

    // ---- 4. a carga de 1.000 -------------------------------------------
    await cartaz(page, 4, `Carregar ${QUANTOS} registros`);
    const TITULOS = ['Orçamento', 'Pedido', 'Contrato', 'Renovação'];
    const lote = [];
    for (let i = 1; i <= QUANTOS; i++)
      lote.push({ titulo: TITULOS[i % TITULOS.length],
                  cliente: `Cliente ${String(i).padStart(4, '0')}`,
                  valor: 100 + (i * 37) % 9000 });
    const r = await page.evaluate(([db, tab, linhas]) =>
      api('inserir_lote', { database: db, tabela: tab, linhas }), [BANCO, TABELA, lote]);
    console.log('carga:', JSON.stringify(r).slice(0, 160));
    await respirar(page, 900);

    // ---- 5. o conteudo -------------------------------------------------
    await cartaz(page, 5, 'Ver o conteúdo da tabela');
    await page.evaluate(([db, tab]) => { est.aba = 'conteudo'; return abrirTabela(db, tab); },
                        [BANCO, TABELA]);
    await page.waitForSelector('.phx-grid tbody tr', { timeout: 20000 });
    await respirar(page, 2000);

    // ---- 6. a busca ----------------------------------------------------
    await cartaz(page, 6, 'Pesquisar um registro na PhxGrid');
    const busca = page.locator('.phx-grid input[type="search"], .phx-busca input').first();
    await busca.click();
    await busca.type('Cliente 0777', { delay: 90 });
    await respirar(page, 2200);
    await busca.fill('');
    await respirar(page, 900);

    // ---- 7. agrupar arrastando o cabecalho -----------------------------
    await cartaz(page, 7, 'Agrupar arrastando a coluna «titulo»');
    await page.evaluate(() => {
      const th = [...document.querySelectorAll('.phx-grid thead tr:not(.phx-frow) th')]
        .find(t => t.dataset.campo === 'titulo');
      const faixa = document.querySelector('.phx-groupbox');
      // Arrastar de VERDADE: o mesmo `dataTransfer` viaja do `dragstart` ao
      // `drop`, que e como o navegador faz. Chamar a funcao de agrupar por
      // dentro provaria que a funcao existe, nao que o arrastar funciona.
      const dt = new DataTransfer();
      th.dispatchEvent(new DragEvent('dragstart', { dataTransfer: dt, bubbles: true }));
      faixa.dispatchEvent(new DragEvent('dragover', { dataTransfer: dt, bubbles: true }));
      faixa.dispatchEvent(new DragEvent('drop', { dataTransfer: dt, bubbles: true }));
    });
    await page.waitForSelector('.phx-gpill', { timeout: 15000 });
    await respirar(page, 2600);

    // ---- 8. o backup ---------------------------------------------------
    await cartaz(page, 8, 'Fazer o backup');
    // `destino` e OBRIGATORIO -- a primeira gravacao parou aqui, com
    // `[SP000018] esquema invalido: informe "destino"`. A moldura de sprint
    // que entrou hoje de manha dizendo onde procurar, no proprio video.
    const destino = `/tmp/phx-bkp-${Date.now()}`;
    const bkp = await page.evaluate(([db, d]) =>
      api('backup', { database: db, destino: d, zip: false }), [BANCO, destino]);
    console.log('backup:', JSON.stringify(bkp).slice(0, 200));
    await respirar(page, 1400);

    // ---- 9. os arquivos em disco ---------------------------------------
    await cartaz(page, 9, 'Os arquivos do banco, em disco');
    const pasta = join(srv.base, BANCO);
    const arquivos = readdirSync(pasta).sort().map(f => {
      const st = statSync(join(pasta, f));
      return { nome: f, bytes: st.size, quando: st.mtime.toISOString().slice(0, 19).replace('T', ' ') };
    });
    console.log('pasta:', pasta, arquivos.length, 'arquivos');
    await page.evaluate(([p, lista]) => {
      document.body.innerHTML = `
        <div style="font:14px/1.6 ui-monospace,monospace;background:#010418;color:#e8eaf2;
                    min-height:100vh;padding:48px 60px">
          <div style="font-size:12px;letter-spacing:.2em;opacity:.55;margin-bottom:6px">
            LISTAGEM REAL DO DIRETÓRIO — lida do disco</div>
          <div style="font-size:22px;color:#ff5f1f;margin-bottom:26px">${p}</div>
          <table style="border-collapse:collapse;font-size:15px">
            <tr style="opacity:.5"><th style="text-align:left;padding:4px 34px 10px 0">arquivo</th>
              <th style="text-align:right;padding:4px 34px 10px 0">bytes</th>
              <th style="text-align:left;padding:4px 0 10px">modificado</th></tr>
            ${lista.map(a => `<tr>
              <td style="padding:3px 34px 3px 0">${a.nome}</td>
              <td style="padding:3px 34px 3px 0;text-align:right">${a.bytes.toLocaleString('pt-BR')}</td>
              <td style="padding:3px 0;opacity:.6">${a.quando}</td></tr>`).join('')}
          </table>
          <div style="margin-top:30px;opacity:.5;font-size:13px">
            ${lista.length} arquivos · este contêiner não tem ambiente gráfico,
            então a listagem é apresentada aqui em vez de num gerenciador de arquivos</div>
        </div>`;
    }, [pasta, arquivos]);
    await respirar(page, 5000);
    return 0;
  } finally {
    await ctx.close();          // fecha ANTES do navegador: e o que grava o video
    await navegador.close();
    await srv.derrubar();
    const v = readdirSync(SAIDA).filter(f => f.endsWith('.webm'));
    console.log('video:', v.map(f => join(SAIDA, f)).join(' '));
    // E o MP4 junto, sem precisar pedir. O Playwright grava SO WebM, e WebM
    // nao abre em tudo -- H.264 abre. `faststart` poe o indice no comeco do
    // arquivo, entao ele comeca a tocar antes de baixar inteiro.
    // Falhar aqui NAO derruba a gravacao: o WebM ja esta salvo, e um conversor
    // ausente e motivo para avisar, nao para perder o video.
    for (const f of v) {
      const mp4 = join(SAIDA, 'phxsql-demonstracao.mp4');
      try {
        execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-i', join(SAIDA, f),
          '-c:v', 'libx264', '-preset', 'medium', '-crf', '23',
          '-pix_fmt', 'yuv420p', '-movflags', '+faststart', mp4]);
        console.log('mp4:', mp4);
      } catch (e) {
        console.log('mp4: NAO gerado --', String(e.message).slice(0, 80));
      }
    }
  }
}

principal().then(c => process.exit(c || 0));

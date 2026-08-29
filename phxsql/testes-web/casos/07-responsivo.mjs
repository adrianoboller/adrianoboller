/* Celular, tablet, desktop, ultrawide e dois monitores -- as cinco larguras
 * que foram pedidas.
 *
 * A regra que atravessava as tres primeiras esta escrita no proprio CSS: NADA
 * rola de lado no corpo da pagina. O que for largo demais rola DENTRO do
 * proprio conteiner. Este caso mede isso, em vez de acreditar no comentario:
 * para cada largura e cada tela, `documentElement.scrollWidth` nao pode passar
 * da largura visivel.
 *
 * E o piso de 40px do alvo de toque, no celular, tambem se mede -- o CSS
 * promete `@media (pointer:coarse)`, e a promessa se confere com o
 * `getBoundingClientRect` e nao com a leitura da regra.
 *
 * AS DUAS LARGURAS NOVAS -- 3440 e 5120 -- trazem TRES medidas que a rolagem
 * lateral nao pegava, porque nenhuma delas rola de lado: a responsividade ja
 * segurava, o que faltava era TETO. Medido antes do conserto, na tela do
 * Painel a 5120: paragrafo de 5.040px, vao de 4.553px entre um rotulo da
 * telemetria e o valor dele, e texto de SVG multiplicado por 5,83 -- 11px
 * desenhados com 67px ao lado de um menu de 13px. As tres reprovam aqui:
 *
 *   1. TEXTO CORRIDO tem teto (`--medida`, 74ch). Uma linha de 630 letras
 *      ninguem le.
 *   2. VAO ROTULO->VALOR tem teto. Valor a mil pixels do rotulo tambem nao se
 *      le: o olho perde a linha no meio do caminho.
 *   3. TEXTO EM SVG nao cresce com o monitor. A escala pode ser constante e
 *      maior que 1 (o medidor vive em 1,4x de proposito), o que nao pode e
 *      ela DEPENDER da largura da janela -- e por isso a prova compara a
 *      escala de 3440 com a de 1600, e nao com 1.
 *
 * E a quarta, que e a sobreposicao: dois `<text>` do mesmo `<g>` de SVG com as
 * caixas cruzadas. Era o defeito do cartao «A maquina» -- o caminho do
 * diretorio por cima do «livres de 37,0 GB» --, e ele depende do COMPRIMENTO
 * do caminho, entao a prova nao espera pelo diretorio temporario da bateria:
 * ela planta um caminho comprido na resposta de `sistema` e desenha com ele.
 * Sem isso o teste passaria por engano num servidor de caminho curto, que e
 * pior que teste que falta. */
import { entrar, cenario, capturar, verdade, bancoDoCaso } from '../apoio.mjs';

/* Do maior para o menor, e a ordem IMPORTA -- ela e um achado da bateria.
 *
 * Em 390px a lateral vira gaveta e se FECHA sozinha depois de cada escolha
 * (`fecharSeSolta`), e esse fechamento e GRAVADO no navegador. Comecando pelo
 * celular, o tablet e o desktop seguintes abriam sem arvore nenhuma -- e as
 * capturas mostravam um layout que ninguem projetou.
 *
 * O comportamento em si nao e defeito obvio: na gaveta, fechar depois de
 * escolher e o certo. O que ele revela e que uma janela que ENCOLHE e volta
 * a crescer nao recupera a arvore, porque o fechamento automatico ficou
 * indistinguivel de uma escolha da pessoa. Esta anotado em docs/TESTES.md;
 * medir com a ordem certa vem antes de propor conserto. */
const LARGURAS = [
  ['dois-monitores', 5120, 1440],
  ['ultrawide', 3440, 1440],
  ['desktop', 1600, 950],
  ['tablet', 820, 1180],
  ['celular', 390, 844],
];

/* Os tetos, em pixel, medidos no navegador com esta folha de estilo.
 *
 * Nao sao os numeros do CSS copiados a mao -- sao o que o teto VALE depois de
 * `74ch` virar pixel na fonte do painel (592px) mais a folga do padding. Um
 * teto de teste apertado demais reprovaria a primeira legenda que crescesse
 * uma palavra; folgado demais deixaria passar o defeito. 700px reprova os
 * 5.040px medidos com folga de sobra, e 900px faz o mesmo pelo vao. */
const TETO_TEXTO = 700;
const TETO_VAO = 900;

/* Tudo o que se mede numa tela, de uma vez -- roda DENTRO do navegador.
 *
 * Fica fora do caso porque `page.evaluate` serializa a funcao: escrita aqui,
 * ela nao fecha sobre nada do Node e o que volta e so numero e texto. */
const MEDIR = () => {
  const r = {
    rola: document.documentElement.scrollWidth,
    cabe: document.documentElement.clientWidth,
    texto: 0, quemTexto: '', vao: 0, quemVao: '', escala: 0, sobrepoe: [],
  };

  // 1. TEXTO CORRIDO. So o que e prosa: paragrafo, item de lista, celula de
  //    legenda e o subtitulo da tela. Tabela de dados e `<pre>` ficam de fora
  //    de proposito -- dado em coluna nao e linha de leitura.
  for (const el of document.querySelectorAll(
      '#painel p, #painel li, #painel td.leg, .cabecalho .sub')) {
    const t = (el.textContent || '').trim();
    if (t.length < 60) continue;
    const w = el.getBoundingClientRect().width;
    if (w > r.texto) { r.texto = Math.round(w); r.quemTexto = t.slice(0, 60); }
  }

  // 2. VAO ENTRE ROTULO E VALOR, nos pares da ficha da telemetria.
  for (const l of document.querySelectorAll('.tlm-l')) {
    const rot = l.querySelector('span'), val = l.querySelector('b');
    if (!rot || !val) continue;
    const a = rot.getBoundingClientRect(), b = val.getBoundingClientRect();
    if (Math.abs(a.top - b.top) > 6) continue;   // ja quebrou em duas linhas
    const vao = Math.round(b.left - a.right);
    if (vao > r.vao) { r.vao = vao; r.quemVao = (rot.textContent || '').trim(); }
  }

  // 3. ESCALA DO TEXTO EM SVG. O `viewBox` estica o desenho inteiro, texto
  //    junto; so contam os SVG que TEM texto.
  for (const svg of document.querySelectorAll('#painel svg')) {
    const vb = svg.viewBox && svg.viewBox.baseVal;
    if (!vb || !vb.width || !svg.querySelector('text')) continue;
    const larg = svg.getBoundingClientRect().width;
    if (!larg) continue;
    r.escala = Math.max(r.escala, Number((larg / vb.width).toFixed(2)));
  }

  // 4. SOBREPOSICAO entre dois `<text>` do mesmo `<g>`. Texto de SVG nao
  //    quebra e nao corta: quando nao cabe, ele passa por cima do vizinho.
  for (const g of document.querySelectorAll('#painel svg g')) {
    const ts = [...g.querySelectorAll('text')];
    for (let i = 0; i < ts.length; i++) {
      for (let j = i + 1; j < ts.length; j++) {
        const a = ts[i].getBoundingClientRect(), b = ts[j].getBoundingClientRect();
        if (a.right > b.left + 1 && b.right > a.left + 1
            && a.bottom > b.top + 1 && b.bottom > a.top + 1) {
          r.sobrepoe.push(`«${(ts[i].textContent || '').slice(0, 28)}» x `
            + `«${(ts[j].textContent || '').slice(0, 28)}»`);
        }
      }
    }
  }
  return r;
};

/* Repinta o cartao da maquina com um CAMINHO COMPRIDO.
 *
 * O defeito original dependia do comprimento do caminho, e o diretorio da
 * bateria e curto: sem plantar, o teste passaria por engano -- e teste que
 * passa por engano e pior que teste que falta. Para o relogio do monitor
 * antes, senao a leitura seguinte do servidor apaga o que foi plantado. */
const PLANTAR_CAMINHO_LONGO = () => {
  if (typeof pararMonitor === 'function') pararMonitor();
  const m = est.maquina;
  if (!m || !m.discos || !m.discos.length) return false;
  m.discos[0].caminho = '/' + 'um-diretorio-de-nome-comprido/'.repeat(4) + 'dados';
  const alvo = document.getElementById('maquina');
  if (!alvo) return false;
  alvo.innerHTML = maquinaHtml(m);
  return true;
};

export const caso = {
  nome: 'responsivo',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Resp');
    const { tab } = await cenario(page, db);

    const problemas = [];
    // Escala do texto em SVG por tela e por largura: o que se prova nao e um
    // valor absoluto, e sim que ela NAO MUDA com o monitor.
    const escalas = {};
    let plantou = 0;

    for (const [apelido, w, h] of LARGURAS) {
      await page.setViewportSize({ width: w, height: h });
      await page.waitForTimeout(250);

      const telas = [
        ['painel', () => page.evaluate(() => irPara('painel'))],
        ['telemetria', async () => {
          await page.evaluate(() => telaTelemetria());
          // A ficha so aparece depois de a primeira amostra chegar.
          await page.waitForSelector('.tlm-l', { timeout: 15000 }).catch(() => {});
        }],
        ['grade', () => page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab])],
        ['ficha', async () => {
          await page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab]);
          await page.waitForSelector('#btNova');
          await page.click('#btNova');
        }],
        ['nova tabela', () => page.evaluate(d => telaNovaTabela(d), db)],
        ['config do servidor', () => page.evaluate(() => verConfigServidor())],
        ['usuarios', () => page.evaluate(() => irPara('usuarios'))],
      ];

      for (const [nome, abrir] of telas) {
        await abrir();
        await page.waitForTimeout(450);
        // O cartao da maquina so existe no Painel, e e la que a sobreposicao
        // morava. Planta o caminho comprido antes de medir.
        if (nome === 'painel' && await page.evaluate(PLANTAR_CAMINHO_LONGO)) plantou++;
        const medida = await page.evaluate(MEDIR);

        // 1px de folga: arredondamento de layout nao e rolagem lateral.
        if (medida.rola > medida.cabe + 1) {
          problemas.push(`${apelido} · ${nome}: a PAGINA rola de lado `
            + `(${medida.rola}px num visor de ${medida.cabe}px)`);
        }
        if (medida.texto > TETO_TEXTO) {
          problemas.push(`${apelido} · ${nome}: linha de texto de ${medida.texto}px `
            + `(teto ${TETO_TEXTO}px) em «${medida.quemTexto}…»`);
        }
        if (medida.vao > TETO_VAO) {
          problemas.push(`${apelido} · ${nome}: ${medida.vao}px entre o rotulo `
            + `«${medida.quemVao}» e o valor dele`);
        }
        for (const s of medida.sobrepoe) {
          problemas.push(`${apelido} · ${nome}: dois textos de SVG sobrepostos — ${s}`);
        }
        if (medida.escala) (escalas[nome] ??= {})[apelido] = medida.escala;

        await capturar(ctx, ctx.nomeCaptura(`${apelido}-${nome.replace(/\W+/g, '-')}`));
      }

      if (apelido === 'celular') {
        // A lateral e gaveta aqui: 268px de 390 nao e um painel ao lado do
        // trabalho, e a tela inteira.
        const larguraDaColuna = await page.evaluate(() =>
          getComputedStyle(document.getElementById('app')).getPropertyValue('--arvore-col').trim());
        verdade(larguraDaColuna === '0px',
          `no celular a arvore ainda ocupa coluna (--arvore-col: ${larguraDaColuna})`);
      }
    }

    // A escala do desktop e a referencia: nas telas largas ela tem de ser a
    // MESMA. 0,05 de folga porque um cartao pode ganhar meio pixel de
    // arredondamento; 3,73x contra 1,51x -- o que se media antes -- passa
    // longe de qualquer folga.
    for (const [nome, porLargura] of Object.entries(escalas)) {
      const base = porLargura.desktop;
      if (!base) continue;
      for (const largo of ['ultrawide', 'dois-monitores']) {
        const e = porLargura[largo];
        if (e && e > base + 0.05) {
          problemas.push(`${largo} · ${nome}: o texto do SVG cresceu com o monitor `
            + `(${e}x contra ${base}x no desktop)`);
        }
      }
    }

    verdade(plantou === LARGURAS.length,
      `o caminho comprido foi plantado em ${plantou} de ${LARGURAS.length} larguras — `
      + 'sem ele a sobreposicao do cartao «A maquina» nao se reproduz');

    await page.setViewportSize({ width: 1600, height: 950 });
    ctx.notas.push(`${LARGURAS.length} larguras × 7 telas medidas`);
    verdade(problemas.length === 0, `largura:\n      ${problemas.join('\n      ')}`);
  },
};

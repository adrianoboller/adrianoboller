/* As duas armadilhas do CSS global, que ja custaram caro nesta casa.
 *
 *  A. `input{width:100%}` transforma um radio ou um checkbox numa bolota do
 *     tamanho da celula quando ele cai dentro de uma tabela.
 *  B. `label{text-transform:uppercase}` mostra «Blumenau» como «BLUMENAU» --
 *     que e uma MENTIRA SOBRE O DADO: quem olha nao sabe se esta gravado
 *     assim. Rotulo em maiuscula e estilo; dado em maiuscula e informacao
 *     errada.
 *
 * Nenhuma das duas aparece lendo o codigo, porque nenhuma esta escrita em
 * lugar nenhum: elas sao o que uma regra de tres palavras faz num componente
 * escrito seis meses depois. Este caso as procura MEDINDO o que o navegador
 * calculou, e nao lendo a folha de estilo. */
import { entrar, cenario, capturar, verdade, bancoDoCaso, api } from '../apoio.mjs';

/* O maior lado que um radio ou checkbox pode ter e ainda ser um radio.
   Um controle nativo mede ~13-16px; 24 da folga para o `pointer:coarse`. */
const TETO_DO_CONTROLE = 24;

/** Mede a tela que estiver aberta AGORA. Devolve o que estiver torto. */
async function medir(page, ondeEstou) {
  return await page.evaluate(onde => {
    const achados = [];
    const visivel = el => {
      const r = el.getBoundingClientRect();
      return r.width > 0 && r.height > 0;
    };

    // A: controle deformado.
    for (const el of document.querySelectorAll('input[type=radio],input[type=checkbox]')) {
      if (!visivel(el)) continue;
      const r = el.getBoundingClientRect();
      if (r.width > 24 || r.height > 24) {
        achados.push(`${onde}: ${el.type} de ${Math.round(r.width)}×${Math.round(r.height)}px`
          + ` (${el.id || el.name || el.className || 'sem id'}) — o input{width:100%} global`);
      }
    }

    // B: dado em maiuscula por estilo.
    //    Percorre o TEXTO, e nao a folha: o que interessa e o que o leitor ve.
    const dado = el => el.closest('td.dado, .rot-dado, #grade td, .celula-dado, .dado');
    const and = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    const jaVistos = new Set();
    for (let n = and.nextNode(); n; n = and.nextNode()) {
      const t = (n.nodeValue || '').trim();
      if (t.length < 2) continue;
      const el = n.parentElement;
      if (!el || !visivel(el) || jaVistos.has(el)) continue;
      jaVistos.add(el);
      const tt = getComputedStyle(el).textTransform;
      if (tt !== 'uppercase') continue;
      // Rotulo em maiuscula e estilo; o que nao pode e o DADO.
      if (dado(el)) {
        achados.push(`${onde}: dado «${t.slice(0, 40)}» pintado em MAIUSCULA por `
          + 'text-transform — quem le nao sabe se esta gravado assim');
      }
      // O texto MISTO que sai todo em caixa alta: «Blumenau» virando
      // «BLUMENAU», «porNome» virando «PORNOME».
      //
      // A regra e maiuscula E minuscula no MESMO texto de origem. Um rotulo
      // que alguem escreveu para ser lido em caixa alta («indexada», «papel»)
      // nao tem maiuscula nenhuma no HTML; um nome de cidade, de coluna ou de
      // indice tem. E a diferenca entre estilo e mentira sobre o dado -- e
      // pega os dois casos sem acusar rotulo nenhum.
      if (/[a-zà-ý]/.test(t) && /[A-ZÀ-Ý]/.test(t) && el.closest('table')
          && !el.closest('th') && !el.closest('label')) {
        achados.push(`${onde}: «${t.slice(0, 40)}» dentro de tabela sai em caixa alta`);
      }
    }

    // C: a caixa de marcar separada do proprio texto.
    //    Um `label` com um checkbox dentro e UMA coisa. Quando uma regra de
    //    formulario empilha o conteudo do label em coluna, a caixinha sobe e
    //    o texto desce -- e quem olha ve uma caixa solta e uma frase solta,
    //    sem saber que uma manda na outra. Foi o que aconteceu com «exigir
    //    motivo escrito» na tela de Nova tabela.
    //
    //    A medida e geometrica de proposito: nao pergunta o `flex-direction`
    //    (a proxima regra que separar os dois pode ser outra propriedade),
    //    pergunta se o controle e o texto DELE dividem alguma linha.
    for (const lab of document.querySelectorAll('label')) {
      const ctrl = lab.querySelector(':scope > input[type=checkbox], :scope > input[type=radio]');
      if (!ctrl || !visivel(lab) || !visivel(ctrl)) continue;
      const faixa = document.createRange();
      let achouTexto = false;
      for (const n of lab.childNodes) {
        if (n.nodeType === 3 && n.nodeValue.trim().length > 1) {
          faixa.selectNode(n); achouTexto = true; break;
        }
      }
      if (!achouTexto) continue;
      const c = ctrl.getBoundingClientRect(), t = faixa.getBoundingClientRect();
      if (t.width === 0 && t.height === 0) continue;
      if (c.bottom <= t.top + 1 || t.bottom <= c.top + 1) {
        achados.push(`${onde}: a caixa de «${(lab.textContent || '').trim().slice(0, 34)}» `
          + 'ficou em outra linha que o proprio texto — controle separado do rotulo dele');
      }
    }
    return achados;
  }, ondeEstou);
}

export const caso = {
  nome: 'css-global',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);
    const db = bancoDoCaso(ctx, 'Css');
    const { tab } = await cenario(page, db);
    // Uma tabela com marca de LGPD: sem ela a tela de Dado pessoal nao
    // desenha LINHA nenhuma, e uma varredura de tabela vazia nao prova nada
    // sobre o que a tabela faz com o texto do dado. O nome do indice e
    // `porCpf`, misto de proposito -- e ele que denuncia o `.pino` em caixa
    // alta mostrando o nome como se estivesse gravado assim.
    await api(page, 'criar_tabela', {
      database: db, tabela: 'fichas',
      colunas: [
        { nome: 'id', tipo: 'Int4', obrigatoria: true },
        { nome: 'cpf', tipo: 'Str(14)', dado_pessoal: 'pessoal' },
      ],
      indices: [
        { nome: 'porId', colunas: ['id'], unico: true, primario: true },
        { nome: 'porCpf', colunas: ['cpf'] },
      ],
    }).catch(() => {});

    const achados = [];
    const telas = [
      ['grade editavel', () => page.evaluate(([d, t]) => verConteudoEditavel(d, t), [db, tab])],
      ['aba conteudo', async () => {
        await page.evaluate(([d, t]) => abrirTabela(d, t), [db, tab]);
        await page.click('.aba[data-aba="conteudo"]');
      }],
      ['dado pessoal (LGPD)', () => page.evaluate(d => telaDadosPessoais(d), db)],
      ['nova tabela', () => page.evaluate(d => telaNovaTabela(d), db)],
      ['config do servidor', () => page.evaluate(() => verConfigServidor())],
      ['uniao de tabelas', () => page.evaluate(d => telaUniao(d), db)],
      ['juncao de tabelas', () => page.evaluate(d => telaJuncao(d), db)],
      ['importar carga', () => page.evaluate(([d, t]) => telaImportar(d, t), [db, tab])],
      ['definicoes do DbLink', () => page.evaluate(() => telaDbLinkDefinicoes())],
      ['jobs', () => page.evaluate(() => telaJobs())],
      ['sequencias', () => page.evaluate(d => verSequencias(d), db)],
      ['systables', () => page.evaluate(d => verSysTables(d), db)],
    ];

    for (const [nome, abrir] of telas) {
      await abrir();
      await page.waitForTimeout(500);
      achados.push(...await medir(page, nome));
      if (nome === 'dado pessoal (LGPD)' || nome === 'grade editavel') {
        await capturar(ctx, ctx.nomeCaptura(nome.replace(/[^a-z]+/gi, '-')));
      }
    }

    ctx.notas.push(`${telas.length} telas medidas`);
    verdade(achados.length === 0,
      `o CSS global mordeu:\n      ${[...new Set(achados)].join('\n      ')}`);
  },
};

/* A PROTECAO pela tela (pedidos 765, 766 e 767, fatia P15), contra o servidor
 * real, nos dois temas:
 *
 *   #btPtSemear  .bt-pt-aplicar  .bt-pt-perfil          Administracao > Protecao
 *   .bt-autorizar  .bt-revogar  #pcNao  #pcSim           Jobs
 *   #fu_senha_exec                                       ficha do usuario
 *
 * Cinco partes, e cada uma confere o EFEITO no servidor -- nunca o que a tela
 * disse:
 *
 *   a. a tabela `phxsys.protecao`: sem ela, o botao de semear; semeada, as 17
 *      linhas em proteger. BAIXAR uma linha com a sessao trancada abre o
 *      dialogo da segunda senha (o da `api()`, o funil unico) e a linha so
 *      muda depois da senha; SUBIR nao pede nada;
 *   b. o job: «Autorizar…» cancelado nao grava; aceito, com a sessao
 *      trancada, passa pela segunda senha e grava usos e prazo; «Revogar»
 *      tira;
 *   c. a segunda senha de OUTRO usuario, pelo administrador: antes, o
 *      usuario nao libera a sessao dele; depois, libera com a senha nova;
 *   d. os interruptores na tela de configuracao: o valor que a tela mostra e
 *      o que o servidor responde em `config` -- o `poupar_loopback` desta
 *      bateria e `false`, entao um «padrao de fabrica» no lugar do valor lido
 *      reprova;
 *   e. o perfil por usuario: a grade, e o detalhe com as horas e as
 *      combinacoes -- a soma das barras e a soma das 24 horas do servidor, e
 *      o nome da database (com maiuscula) aparece como foi gravado.
 *
 * As partes rodam TODAS mesmo quando uma reprova, e a falha junta as que
 * cairam: o RED de cinco defeitos reposto sai numa corrida so. */
import { entrar, api, verdade, igual, capturar, bancoDoCaso, cenario, clicarOuExplicar,
         CREDENCIAL, SENHA_EXECUCAO, Falha } from '../apoio.mjs';

const ESPERA = 20000;

export const caso = {
  nome: 'protecao',
  async rodar(ctx) {
    const extras = [];
    const falhas = [];
    try {
      await corpo(ctx, extras, falhas);
    } finally {
      for (const x of extras) { try { await x(); } catch { /* ja desfeito */ } }
    }
    if (falhas.length) throw new Falha(falhas.join(' || '));
  },
};

async function corpo(ctx, extras, falhas) {
  const { page } = ctx;
  const t = ctx.tema === 'claro' ? 'c' : 'e';
  const db = bancoDoCaso(ctx, 'Prot');
  const nomeJob = `prot${t}_job`;
  const login = `protusr${t}`;
  const senhaLogin = 'provaDaBateria123';
  const senhaUsr = `segunda-do-usuario-${t}-91`;
  await entrar(page, ctx.url);

  /** Pergunta ao servidor ate a condicao valer, ou ate o prazo. */
  const ate = async (cond, oQue) => {
    for (let i = 0; i < 80; i++) {
      if (await cond()) return;
      await page.waitForTimeout(250);
    }
    throw new Falha(oQue);
  };
  /** Uma parte: roda, e se cair anota o porque e limpa o dialogo que ficou. */
  const parte = async (nome, corpoDaParte) => {
    try { await corpoDaParte(); } catch (e) {
      const tela = await page.evaluate(() => ({
        aviso: document.querySelector('#aviso')?.textContent,
        recado: document.querySelector('#seRecado')?.textContent,
      })).catch(() => ({}));
      falhas.push(`[${nome}] ${String(e.message).split('\n')[0]} -- ${JSON.stringify(tela)}`);
      await page.evaluate(() => document.querySelectorAll('.sobre').forEach(x => x.remove())).catch(() => {});
    }
  };
  /** Contorno, nunca fundo cheio: o fundo do botao parado e transparente. */
  const contorno = async (sel, oQue) => {
    const fundo = await page.$eval(sel, b => getComputedStyle(b).backgroundColor);
    verdade(/rgba\(0, 0, 0, 0\)|transparent/.test(fundo), `${oQue}: fundo cheio (${fundo}) -- a acao e contorno`);
  };
  // Trancar pelo motor, e nao pelo botao: o `entrar` libera pela `api()` e o
  // «Trancar» da barra so acende quando a liberacao passa pelo dialogo. O
  // botao ja tem prova propria no caso `senha-de-execucao`.
  const trancar = () => page.evaluate(async () => {
    await api('trancar_execucao');
    marcarExecucaoLiberada(false);
  });
  const senhaNoDialogo = async () => {
    await page.waitForSelector('#dlgSenhaExecucao', { timeout: ESPERA });
    await page.fill('#seSenha', SENHA_EXECUCAO);
    await clicarOuExplicar(page, '#seSim');
    await page.waitForSelector('#dlgSenhaExecucao', { state: 'detached', timeout: ESPERA });
  };

  // ----------------------------------------------------------- o cenario
  // Com a sessao ainda liberada pelo `entrar`: criar e excluir usuario e
  // excluir tabela sao da lista de perigo.
  await cenario(page, db);
  await api(page, 'usuario_excluir', { login }).catch(() => {});
  await api(page, 'usuario_criar', { login, senha: senhaLogin, nome: 'Usuário da prova 767',
                                     bases: { [db]: { ler: true } } });
  extras.push(() => api(page, 'usuario_excluir', { login }));
  await api(page, 'job_excluir', { nome: nomeJob }).catch(() => {});
  await api(page, 'job_salvar', { job: { nome: nomeJob, descricao: 'job da prova 767', ligado: false,
    usuario: CREDENCIAL.USUARIO, cada_minutos: 60, pedido: { op: 'ping' } } });
  extras.push(() => api(page, 'job_excluir', { nome: nomeJob }));
  // A tabela de protecao e do SERVIDOR, e a bateria roda os dois temas no
  // mesmo: cada tema a apaga para ver o botao de semear.
  await api(page, 'excluir_tabela', { database: 'phxsys', tabela: 'protecao', confirmar: 'protecao' })
    .catch(() => {});
  const r0 = await api(page, 'aquario_retrato');
  if (r0.ligada === false) {
    await api(page, 'telemetria_ligar');
    extras.push(() => api(page, 'telemetria_desligar'));
  }
  // Pedidos com a telemetria ligada: e com eles que o perfil aprende.
  for (let i = 0; i < 4; i++) await api(page, 'varrer', { database: db, tabela: 'clientes' });

  const linhasDaTabela = async () =>
    (await api(page, 'varrer', { database: 'phxsys', tabela: 'protecao', max: 1000 })).linhas || [];
  const modoDe = async rowid => ((await linhasDaTabela()).find(l => l.rowid === rowid) || {}).modo;
  const abrirProtecao = async () => {
    await clicarOuExplicar(page, '#arvore [data-admin="protecao"]');
    await page.waitForFunction(() => document.querySelector('#titulo')?.textContent
      && document.querySelector('#painel .fichas'), undefined, { timeout: ESPERA });
  };

  // ------------------------------------------------- a. phxsys.protecao
  await parte('a. semear', async () => {
    await abrirProtecao();
    await page.waitForSelector('#btPtSemear', { timeout: ESPERA });
    await contorno('#btPtSemear', 'semear');
    await capturar(ctx, ctx.nomeCaptura('sem-tabela'));
    await clicarOuExplicar(page, '#btPtSemear');
    await page.waitForSelector('#gradePtModos .bt-pt-aplicar', { timeout: ESPERA });
    const linhas = await linhasDaTabela();
    igual(linhas.length, 17, 'linhas semeadas na tabela de protecao');
    verdade(linhas.every(l => l.modo === 'proteger'), 'a semeadura devia nascer toda em proteger');
    // O nome da tabela no titulo e dado: o `.secao{text-transform:uppercase}`
    // o escreveria PHXSYS.PROTECAO.
    igual(await page.$eval('#painel h3.secao code', e => e.innerText), 'phxsys.protecao',
      'o nome da tabela no titulo da secao');
    await capturar(ctx, ctx.nomeCaptura('tabela'), { inteira: true });
  });

  await parte('a. baixar pede a segunda senha, subir nao', async () => {
    const alvo = (await linhasDaTabela()).find(l => l.op === 'expurgar_trilha');
    verdade(alvo, 'a linha de expurgar_trilha nao existe');
    const sel = `#gradePtModos .pt-modo[data-rowid="${alvo.rowid}"]`;
    const bt = `#gradePtModos .bt-pt-aplicar[data-rowid="${alvo.rowid}"]`;
    await trancar();
    await abrirProtecao();
    await page.waitForSelector(bt, { timeout: ESPERA });
    await contorno(bt, 'aplicar');
    // A caixa do modo e o botao na MESMA linha: o `select{width:100%}` global
    // empurraria o botao para baixo da caixa.
    const [cx, bx] = await Promise.all([page.$eval(sel, e => e.getBoundingClientRect()),
                                        page.$eval(bt, e => e.getBoundingClientRect())].map(p => p));
    verdade(Math.abs(cx.top - bx.top) < 12 && bx.left > cx.left,
      `o «Aplicar» nao esta ao lado da caixa do modo (caixa ${JSON.stringify(cx)}, botao ${JSON.stringify(bx)})`);
    await page.selectOption(sel, 'observar');
    await clicarOuExplicar(page, bt);
    await page.waitForSelector('#dlgSenhaExecucao', { timeout: ESPERA });
    igual(await modoDe(alvo.rowid), 'proteger', 'a linha baixou ANTES da segunda senha');
    await capturar(ctx, ctx.nomeCaptura('baixar-pede-senha'));
    await senhaNoDialogo();
    await ate(async () => (await modoDe(alvo.rowid)) === 'observar', 'liberada a sessao, a linha nao baixou para observar');

    await trancar();
    await abrirProtecao();
    await page.waitForSelector(bt, { timeout: ESPERA });
    igual(await page.$eval(sel, e => e.value), 'observar', 'a caixa devia abrir no modo gravado');
    await page.selectOption(sel, 'proteger');
    await clicarOuExplicar(page, bt);
    await ate(async () => (await modoDe(alvo.rowid)) === 'proteger', 'subir para proteger nao gravou');
    igual(await page.$('#dlgSenhaExecucao'), null, 'subir a protecao pediu a segunda senha');
  });

  // ------------------------------------------------------------- b. o job
  const jobNoServidor = async () => ((await api(page, 'jobs', {})).jobs || []).find(j => j.nome === nomeJob);
  await parte('b. autorizar e revogar o job', async () => {
    await trancar();
    await page.evaluate(() => telaJobs());
    const bt = `#gradeJobs .bt-autorizar[data-n="${nomeJob}"]`;
    await page.waitForSelector(bt, { timeout: ESPERA });
    // Cancelar nao grava nada.
    await clicarOuExplicar(page, bt);
    await page.waitForSelector('#dlgCampos', { timeout: ESPERA });
    await clicarOuExplicar(page, '#pcNao');
    await page.waitForSelector('#dlgCampos', { state: 'detached', timeout: ESPERA });
    verdade(!(await jobNoServidor()).autorizacao, 'cancelar o dialogo autorizou o job');
    // Aceitar, com a sessao trancada: a segunda senha no dialogo de sempre.
    await clicarOuExplicar(page, bt);
    await page.waitForSelector('#dlgCampos', { timeout: ESPERA });
    await page.fill('#jaUsos', '3');
    await page.fill('#jaDias', '10');
    await capturar(ctx, ctx.nomeCaptura('autorizar-job'));
    await clicarOuExplicar(page, '#pcSim');
    await senhaNoDialogo();
    await ate(async () => !!(await jobNoServidor())?.autorizacao, 'o job nao ficou autorizado');
    const a = (await jobNoServidor()).autorizacao;
    igual(a.usos, 3, 'as corridas autorizadas');
    const dias = (a.ate_ms - Date.now()) / 86400000;
    verdade(dias > 9.9 && dias <= 10, `o prazo devia ser de 10 dias, e e de ${dias.toFixed(2)}`);
    igual(a.por, CREDENCIAL.USUARIO, 'quem autorizou');
    const revogar = `#gradeJobs .bt-revogar[data-n="${nomeJob}"]`;
    await page.waitForSelector(revogar, { timeout: ESPERA });
    await capturar(ctx, ctx.nomeCaptura('job-autorizado'), { inteira: true });
    await clicarOuExplicar(page, revogar);
    await ate(async () => !(await jobNoServidor())?.autorizacao, '«Revogar» nao tirou a autorizacao');
  });

  // --------------------------------------- c. a segunda senha de outro usuario
  await parte('c. a segunda senha de outro usuario', async () => {
    const outra = await page.context().newPage();
    try {
      await entrar(outra, ctx.url, { usuario: login, senha: senhaLogin, token: CREDENCIAL.TOKEN }, { liberar: false });
      const libera = () => outra.evaluate(s => api('liberar_execucao', { senha: s }, true)
        .then(() => true, () => false), senhaUsr);
      verdade(!(await libera()), 'o usuario liberou a sessao ANTES de ter segunda senha');
      await trancar();
      await page.evaluate(l => abrirFichaUsuario(l), login);
      await page.waitForSelector('#fu_senha_exec', { timeout: ESPERA });
      await contorno('#fu_senha_exec', 'definir a senha de execucao');
      await clicarOuExplicar(page, '#fu_senha_exec');
      await page.waitForSelector('#dlgCampos', { timeout: ESPERA });
      await page.fill('#fsNova', senhaUsr);
      await page.fill('#fsConfirma', senhaUsr);
      await clicarOuExplicar(page, '#pcSim');
      await senhaNoDialogo();
      await ate(libera, 'definida pelo administrador, a segunda senha nao libera a sessao do usuario');
      const sobrou = await page.evaluate(s => document.documentElement.outerHTML.includes(s)
        || JSON.stringify(est).includes(s), senhaUsr);
      verdade(!sobrou, 'a segunda senha do usuario sobrou na pagina');
    } finally {
      await outra.close().catch(() => {});
    }
  });

  // ------------------------------------------- d. os interruptores na configuracao
  await parte('d. os interruptores na configuracao', async () => {
    await page.evaluate(() => verConfigServidor());
    await page.waitForFunction(() => [...document.querySelectorAll('#painel td code')]
      .some(c => c.textContent === 'protecao.bloquear_por_codigo'), undefined, { timeout: ESPERA });
    const cfg = await api(page, 'config');
    const pt = cfg.protecao || {}, seg = cfg.seguranca || {};
    const simNao = v => page.evaluate(x => simNao(x), v);
    const esperado = {
      'protecao.bloquear_por_codigo': await simNao(pt.bloquear_por_codigo),
      'protecao.primeiro_cadastro_pelo_administrador': await simNao(pt.primeiro_cadastro_pelo_administrador),
      'protecao.prazo_comando_ms': String(pt.prazo_comando_ms),
      'protecao.prazo_comando_modo': String(pt.prazo_comando_modo),
      'protecao.modo_dispensa_a_senha': await simNao(pt.modo_dispensa_a_senha),
      'seguranca.poupar_loopback': await simNao(seg.poupar_loopback),
    };
    igual(seg.poupar_loopback, false, 'a premissa: esta bateria sobe com poupar_loopback false');
    const naTela = await page.evaluate(campos => Object.fromEntries(campos.map(c => {
      const tr = [...document.querySelectorAll('#painel tr')].find(x => x.querySelector('td code')?.textContent === c);
      const tds = tr ? tr.querySelectorAll('td') : [];
      return [c, tr ? { valor: tds[1].textContent.trim(), motivo: tds[2].textContent.trim(),
                        caixa: getComputedStyle(tds[2]).textTransform } : null];
    })), Object.keys(esperado));
    for (const [campo, valor] of Object.entries(esperado)) {
      const l = naTela[campo];
      verdade(l, `a configuracao nao mostra ${campo}`);
      igual(l.valor, valor, `o valor de ${campo} na tela`);
      verdade(l.motivo.length > 30, `${campo} sem o motivo`);
      verdade(l.caixa !== 'uppercase', `o motivo de ${campo} em caixa alta`);
    }
    await page.locator('#painel td code', { hasText: 'seguranca.poupar_loopback' }).scrollIntoViewIfNeeded();
    await capturar(ctx, ctx.nomeCaptura('interruptores'));
  });

  // --------------------------------------------------------- e. o perfil
  await parte('e. o perfil por usuario', async () => {
    // O retrato de ANTES de a tela perguntar: cada pedido -- inclusive o
    // `perfis` da propria tela -- entra no perfil, entao a tela fica entre
    // este e o de depois.
    const meu = ((await api(page, 'perfis')).perfis || []).find(p => p.usuario === CREDENCIAL.USUARIO);
    await abrirProtecao();
    const bt = `#gradePtPerfis .bt-pt-perfil[data-u="${CREDENCIAL.USUARIO}"]`;
    await page.waitForSelector(bt, { timeout: ESPERA });
    verdade(meu && meu.n > 0, 'o servidor nao tem o perfil do administrador da bateria');
    await clicarOuExplicar(page, bt);
    await page.waitForSelector('#ptPerfilDetalhe .barrash', { timeout: ESPERA });
    await page.waitForSelector('#gradePtCombinacoes .phx-grid tbody tr', { timeout: ESPERA });
    // As barras somam as 24 horas que o servidor tem -- a conversao de fuso
    // gira a ordem, e nunca perde nem inventa pedido. O perfil continua
    // aprendendo enquanto a tela pergunta, entao a soma da tela fica entre a
    // de antes e a de agora.
    const soma = await page.$$eval('#ptPerfilDetalhe .barrash .bh-val',
      vs => vs.reduce((s, v) => s + Number(v.textContent.replace(/\D/g, '') || 0), 0));
    const depois = ((await api(page, 'perfis')).perfis || []).find(p => p.usuario === CREDENCIAL.USUARIO);
    const somaAntes = meu.horas.reduce((s, h) => s + h, 0);
    const somaDepois = depois.horas.reduce((s, h) => s + h, 0);
    verdade(soma >= somaAntes && soma <= somaDepois,
      `as barras somam ${soma}, e o servidor tinha entre ${somaAntes} e ${somaDepois}`);
    igual(await page.$$eval('#ptPerfilDetalhe .barrash .bh-nome', n => n.length), 24, 'as 24 horas');
    // A database tem maiuscula no fim (`batProtC`): o dado aparece como foi
    // gravado, e nao como o CSS global de rotulo o escreveria.
    // A combinacao conferida e a desta base -- salvo quando o perfil ja
    // chegou ao teto (na bateria inteira o administrador passa de 256
    // combinacoes e a nova vai ao coringa, medido). Ai a conferida e a
    // primeira com maiuscula no nome que o SERVIDOR tem: a pergunta e a
    // mesma, o dado aparece como foi gravado.
    const combs = depois.combinacoes || [];
    const cheio = combs.length >= ((await api(page, 'perfis')).teto_de_combinacoes || 256);
    const alvo = combs.find(c => c[1] === db)
      || (cheio ? combs.find(c => /[A-Z]/.test(c[1] + c[2])) || combs[0] : null);
    verdade(alvo, `o SERVIDOR nao tem a combinacao com ${db} e o perfil nao esta cheio (${combs.length})`);
    const nome = /[A-Z]/.test(alvo[1]) || !/[A-Z]/.test(alvo[2]) ? alvo[1] : alvo[2];
    const visto = await page.$$eval('#gradePtCombinacoes .phx-grid tbody td .crua', (cs, d) =>
      cs.filter(c => c.textContent === d).map(c => c.innerText), nome);
    verdade(visto.length > 0, `a combinacao com ${nome} nao apareceu na tela`);
    verdade(visto.every(x => x === nome), `o nome apareceu como ${JSON.stringify(visto)} e nao ${nome}`);
    igual(await page.$eval('#ptPerfilDetalhe h3 .crua', e => e.innerText), CREDENCIAL.USUARIO,
      'o login no titulo do perfil');
    await capturar(ctx, ctx.nomeCaptura('perfil'), { inteira: true });
  });
}

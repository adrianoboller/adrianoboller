/* Gap 230: a aba de Usuários chamando `usuario_criar`/`usuario_alterar`/
 * `usuario_excluir` PELA TELA, com poder por base e por tabela -- o
 * mestre-detalhe que `docs/USUARIOS.md` descreve (usuário -> bases ->
 * tabelas). As três operações já existiam no protocolo desde o pedido 221;
 * até esta rodada a aba só lia.
 *
 * Prova nos dois sentidos, pelo SERVIDOR de verdade (nunca pelo que a tela
 * mostrou): cria um usuário com uma base e uma tabela dentro dela, confere
 * pela API que gravou exatamente aquilo -- nega por omissão incluída; edita
 * (desliga uma atividade da base, tira a regra da tabela), confere de novo;
 * exclui, confere que sumiu.
 *
 * E as duas armadilhas do CSS global que já morderam esta tela antes, na
 * mesma régua da "Nova tabela" e do "Blumelau" da aba Conteúdo:
 *
 *  1. `input{width:100%}` -- o checkbox de atividade, dentro da célula da
 *     tabela de bases/tabelas, tem de nascer pequeno (15×15), não a bolota
 *     do tamanho da célula;
 *  2. `label{text-transform:uppercase}` -- o nome do banco é DADO, nunca
 *     rótulo, e "Blumenau" (de propósito no nome do banco deste caso) tem de
 *     aparecer como foi digitado na célula da tabela, nunca "BLUMENAU". */
import {
  entrar, api, capturar, verdade, igual, bancoDoCaso, abrirPeloMenu,
} from '../apoio.mjs';

export const caso = {
  nome: 'usuarios',
  async rodar(ctx) {
    const { page } = ctx;
    await entrar(page, ctx.url);

    const db = bancoDoCaso(ctx, 'UsuBlumenau');
    // Um segundo banco, so para provar ADICIONAR e REMOVER base -- sem ele o
    // botao `.fu-rm-base` nunca receberia clique de verdade nesta bateria.
    const dbTemp = bancoDoCaso(ctx, 'UsuTemp');
    const tab = 'clientes';
    // Um login por tema, como em `27-direito-por-coluna`: os dois temas
    // batem no MESMO servidor, e um login repetido reprovaria a segunda
    // corrida com "ja ha um usuario com este login".
    const login = `fichausu${ctx.tema === 'claro' ? 'c' : 'e'}`;
    const senha = 'provaDaBateria123';

    await api(page, 'criar_database', { database: db }).catch(() => {});
    await api(page, 'criar_database', { database: dbTemp }).catch(() => {});
    await api(page, 'criar_tabela', {
      database: db, tabela: tab,
      colunas: [{ nome: 'id', tipo: 'Int4', obrigatoria: true }],
      indices: [{ nome: 'porId', colunas: ['id'], unico: true, primario: true }],
    }).catch(() => {});
    // Idempotente: uma corrida que reaproveita servidor (segundo tema, ou
    // uma corrida anterior desta bateria) não pode reprovar por causa de um
    // usuário que já existia.
    await api(page, 'usuario_excluir', { login }).catch(() => {});

    const lerUsuario = async () => {
      const r = await api(page, 'usuarios');
      const lista = r.usuarios || r;
      return lista.find(x => x.login === login);
    };

    // ------------------------------------------------------------ a lista
    await abrirPeloMenu(page, 'tela.usuarios');
    await page.waitForSelector('#btNovoUsuario', { timeout: 15000 });
    await page.waitForSelector('#gradeAdmUsuarios .phx-grid', { timeout: 15000 });
    await capturar(ctx, ctx.nomeCaptura('lista'));

    // -------------------------------------------------------- criar (novo)
    await page.click('#btNovoUsuario');
    await page.waitForSelector('#fichaUsuario');
    igual(await page.inputValue('#fu_login'), '',
      'a ficha de NOVO usuário não nasceu com o login vazio');
    igual((await page.textContent('#titulo')).trim(), 'Novo usuário',
      'o título da ficha de criação não é o de "novo usuário"');

    // Prova real de "Voltar": desiste sem salvar, e nada foi criado.
    await page.click('#fu_voltar');
    await page.waitForSelector('#gradeAdmUsuarios .phx-grid', { timeout: 15000 });
    verdade(!(await lerUsuario()), 'clicar em "Voltar" sem salvar criou o usuário mesmo assim');
    await page.click('#btNovoUsuario');
    await page.waitForSelector('#fichaUsuario');

    await page.fill('#fu_nome', 'Usuária de Prova');
    await page.fill('#fu_login', login);
    await page.fill('#fu_email', 'prova@exemplo.com');
    await page.fill('#fu_telefone', '+55 47 90000-0000');
    await page.fill('#fu_senha', senha);
    await page.fill('#fu_senha2', senha);

    // --------------------------------------------------- poder por BASE
    await page.selectOption('#fu_addBaseSel', db);
    await page.click('#fu_addBaseBtn');
    await page.waitForSelector('#fu_bases .fu-base-row');
    await capturar(ctx, ctx.nomeCaptura('form-com-base'));

    // A ARMADILHA DO CHECKBOX (achado do `css-global`, replicado aqui): sem
    // `td input[type=checkbox]{width:15px;height:15px}`, o checkbox de
    // atividade vira a bolota do tamanho da célula.
    const caixaCheck = await page.locator('#fu_bases .fu-perm[data-bi="0"][data-at="ler"]').boundingBox();
    verdade(!!caixaCheck && caixaCheck.width <= 20 && caixaCheck.height <= 20,
      `o checkbox de atividade nasceu ${caixaCheck ? `${Math.round(caixaCheck.width)}x${Math.round(caixaCheck.height)}` : 'invisível'}`
      + ' -- o CSS global (input{width:100%}) mordeu o componente novo');

    // A ARMADILHA DO DADO EM CAIXA ALTA: o nome do banco é DADO, e "Blumenau"
    // é a marca registrada desta casa para esse defeito.
    const nomeNaCelula = (await page.locator('#fu_bases .fu-base-row td b').first().textContent()).trim();
    igual(nomeNaCelula, db,
      'o nome do banco mudou de caixa na célula da tabela -- dado virou rótulo');

    await page.check('#fu_bases .fu-perm[data-bi="0"][data-at="ler"]');
    await page.check('#fu_bases .fu-perm[data-bi="0"][data-at="inserir"]');

    // Adiciona uma SEGUNDA base e a remove de novo -- prova real de
    // `.fu-rm-base`, que nenhum outro passo deste caso exercita. `dbTemp`
    // nunca chega a sobrar no pedido de `usuario_criar`.
    await page.selectOption('#fu_addBaseSel', dbTemp);
    await page.click('#fu_addBaseBtn');
    await page.waitForFunction(() => document.querySelectorAll('#fu_bases .fu-base-row').length === 2);
    await page.click('.fu-rm-base[data-bi="1"]');
    await page.waitForFunction(() => document.querySelectorAll('#fu_bases .fu-base-row').length === 1);
    igual(await page.locator('#fu_bases .fu-base-row td b').first().textContent(), db,
      'remover a SEGUNDA base levou junto a primeira');

    // ------------------------------------------------- poder por TABELA
    await page.selectOption('.fu-addTabSel[data-bi="0"]', tab);
    await page.click('.fu-addTabBtn[data-bi="0"]');
    await page.waitForSelector('.fu-permt[data-bi="0"][data-ti="0"]');
    await page.check('.fu-permt[data-bi="0"][data-ti="0"][data-at="ler"]');
    await capturar(ctx, ctx.nomeCaptura('form-com-tabela'));

    await page.click('#fu_salvar');
    await page.waitForSelector('#gradeAdmUsuarios .phx-grid', { timeout: 15000 });
    await page.waitForTimeout(250);
    const avisoCriar = await page.evaluate(() => {
      const a = document.querySelector('#aviso:not([hidden])');
      return a ? { txt: a.textContent.trim(), mal: a.classList.contains('mal') } : null;
    });
    verdade(avisoCriar && !avisoCriar.mal,
      `criar o usuário pela tela falhou: ${avisoCriar ? avisoCriar.txt : 'nem aviso saiu'}`);

    // -------------------------------------------------- prova real: no SERVIDOR
    let u = await lerUsuario();
    verdade(!!u, 'o usuário criado pela tela não apareceu em {"op":"usuarios"}');
    igual(u.nome, 'Usuária de Prova', 'o nome não gravou como foi digitado');
    igual(u.ativo, true, 'o usuário nasceu inativo -- a caixa "ativo" começa marcada');
    igual(u.supervisor, false, 'o usuário nasceu supervisor sem ninguém marcar a caixa');
    verdade(u.bases && u.bases[db] && u.bases[db].ler === true && u.bases[db].inserir === true,
      `o poder por base não gravou como marcado: ${JSON.stringify(u.bases)}`);
    igual(u.bases[db].excluir, false,
      'uma atividade não marcada gravou true -- "nega por omissão" quebrou');
    verdade(u.tabelas && u.tabelas[db] && u.tabelas[db][tab] && u.tabelas[db][tab].ler === true,
      `o poder por TABELA não gravou como marcado: ${JSON.stringify(u.tabelas)}`);
    verdade(JSON.stringify(u).toLowerCase().indexOf('senha') === -1,
      'a ficha do usuário devolvida pelo servidor trouxe "senha" -- ela nunca deveria voltar');

    // -------------------------------------------------------------- editar
    await page.click(`#gradeAdmUsuarios .bt-editar-usuario[data-login="${login}"]`);
    await page.waitForSelector('#fichaUsuario');
    igual(await page.inputValue('#fu_login'), login,
      'a ficha de EDITAR não veio com o login certo');
    verdade(await page.getAttribute('#fu_login', 'readonly') !== null,
      'o campo login continua editável numa ficha de alterar -- `usuario_alterar` não muda login');
    igual((await page.textContent('#titulo')).trim(), `Editar ${login}`,
      'o título da ficha de edição não nomeia o usuário');
    await capturar(ctx, ctx.nomeCaptura('form-editar'));

    // Tira o poder da TABELA (a regra deixa de existir) e desliga "inserir"
    // na BASE -- as duas mudanças que `usuario_alterar` precisa aplicar como
    // SUBSTITUIÇÃO do bloco inteiro, e não como remendo por cima do que já
    // estava lá.
    await page.click('.fu-rm-tab[data-bi="0"][data-ti="0"]');
    await page.waitForFunction(() => document.querySelectorAll('.fu-permt').length === 0);
    await page.uncheck('#fu_bases .fu-perm[data-bi="0"][data-at="inserir"]');
    await page.click('#fu_salvar');
    await page.waitForSelector('#gradeAdmUsuarios .phx-grid', { timeout: 15000 });
    await page.waitForTimeout(250);

    u = await lerUsuario();
    verdade(!!u, 'o usuário sumiu depois de um simples alterar');
    igual(u.bases[db].inserir, false,
      'desmarcar "inserir" na tela não desligou a atividade no servidor');
    igual(u.bases[db].ler, true, 'alterar apagou uma atividade que ninguém tocou');
    verdade(!u.tabelas || !u.tabelas[db] || !u.tabelas[db][tab],
      'remover a tabela na tela não tirou a regra da tabela no servidor');

    // -------------------------------------------------------------- excluir
    await page.click(`#gradeAdmUsuarios .bt-editar-usuario[data-login="${login}"]`);
    await page.waitForSelector('#fichaUsuario');
    await page.click('#fu_excluir');
    await page.waitForSelector('.sobre .caixa');
    await capturar(ctx, ctx.nomeCaptura('confirmar-exclusao'));

    // Cancela uma vez primeiro -- prova real de `#fexcNao`: o dialogo fecha
    // e a ficha continua ali, com o usuario intacto no servidor.
    await page.click('#fexcNao');
    await page.waitForSelector('.sobre', { state: 'detached' });
    verdade(await page.locator('#fichaUsuario').isVisible(),
      'cancelar a exclusao fechou a propria ficha, e nao so o dialogo');
    verdade(!!(await lerUsuario()),
      'cancelar a exclusao excluiu o usuario mesmo assim');

    await page.click('#fu_excluir');
    await page.waitForSelector('.sobre .caixa');
    await page.click('#fexcSim');
    await page.waitForSelector('#gradeAdmUsuarios .phx-grid', { timeout: 15000 });
    await page.waitForTimeout(250);

    verdade(!(await lerUsuario()),
      'o usuário excluído pela tela continua em {"op":"usuarios"}');
  },
};

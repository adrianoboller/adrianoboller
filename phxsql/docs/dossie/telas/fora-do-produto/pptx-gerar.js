// Gera o PPTX com as 79 telas do console na sequencia de uso.
const pptxgen = require('pptxgenjs');
const telas = require('./telas.json');

const COR = { fundo: '010418', painel: '0A1122', prata: 'DDE2EB', mudo: '8A93A6',
  ambar: 'FFC43D', laranja: 'FF8A1C', vermelhao: 'FF4D10' };
const TIT = 'Exo 2', CORPO = 'Calibri';

// [capitulo, [ [id, descricao, tambem?] ... ]]
const ROTEIRO = [
  ['Entrar', 'Login, identidade e a primeira visão do servidor', [
    [0, 'Entrada com servidor, porta, usuário, senha, token do servidor e o idioma da sessão.'],
    [58, 'Quem é o usuário da sessão e o que ele pode fazer.'],
    [6, 'O painel: estado do servidor, bancos e números de uso de relance.'],
  ]],
  ['Configurar', 'O servidor, o banco, os usuários e a língua da tela', [
    [75, 'Configurações gerais do servidor.'],
    [76, 'Configurações do banco aberto.'],
    [77, 'Configurações por usuário.'],
    [78, 'Diretivas de acesso: quem pode o quê.'],
    [79, 'Diretivas do banco.'],
    [82, 'Idiomas da interface: a fábrica de textos da tela.'],
    [81, 'Mensagens do servidor, por código e idioma.'],
  ]],
  ['Usuários e acesso', 'Quem entra, de onde, e o que está bloqueado', [
    [49, 'Cadastro de usuários.'],
    [50, 'Acessos registrados.'],
    [51, 'Bloqueios de acesso.'],
  ]],
  ['O banco', 'Gerir o banco e enxergar o catálogo', [
    [13, 'Gerir o banco aberto.'],
    [8, 'Visão do database.'],
    [15, 'SysTables: o catálogo das tabelas.'],
    [16, 'SysColumns: o catálogo das colunas.'],
  ]],
  ['Criar e estruturar tabelas', 'Da tabela nova aos índices', [
    [27, 'Gerir as tabelas do banco.'],
    [28, 'Criar uma tabela nova.'],
    [29, 'Estrutura da tabela: colunas, tipos e chaves.'],
    [1, 'A mesma estrutura, pela aba da tabela aberta.'],
    [32, 'Configurações e diretivas da tabela.'],
    [31, 'Partições da tabela.'],
    [33, 'Índices da tabela.'],
    [3, 'Os índices, pela aba da tabela aberta.'],
  ]],
  ['Trabalhar com os dados', 'Editar, importar, consultar e combinar', [
    [30, 'Editar o conteúdo, linha a linha.'],
    [2, 'O conteúdo, pela aba da tabela aberta.'],
    [42, 'Importar carga para a tabela.'],
    [41, 'Exportar a tabela.'],
    [60, 'Consulta SQL.'],
    [17, 'Tabela dinâmica (pivô).', 'também em Ferramentas'],
    [61, 'A mesma tabela dinâmica, pelo menu Ferramentas.'],
    [18, 'Junção de tabelas.', 'também em Ferramentas'],
    [62, 'A mesma junção, pelo menu Ferramentas.'],
    [19, 'União de tabelas.', 'também em Ferramentas'],
    [63, 'A mesma união, pelo menu Ferramentas.'],
    [20, 'Sequências do banco.'],
    [21, 'Diagrama entidade-relacionamento.'],
  ]],
  ['Integridade e histórico', 'Chaves, diário, lixeira e conferências', [
    [35, 'Integridade referencial: mãe com filhas nunca se exclui.'],
    [5, 'A integridade, pela aba da tabela aberta.'],
    [34, 'Diário da tabela: o que mudou e quando.'],
    [4, 'O diário, pela aba da tabela aberta.'],
    [36, 'Lixeira da tabela: o excluído que volta.'],
    [37, 'Motivos das exclusões.'],
    [39, 'Verificar a tabela.'],
    [40, 'Soma de verificação da tabela.'],
  ]],
  ['Transações e travas', 'O que está em curso e o que está preso', [
    [26, 'Transações do banco.'],
    [59, 'Gestão de transações.'],
    [25, 'Arquivos bloqueados.'],
  ]],
  ['Memória', 'Tabelas residentes em RAM', [
    [46, 'Carregar a tabela na RAM.'],
    [47, 'Tabelas residentes.'],
    [48, 'Liberar a tabela da RAM.'],
  ]],
  ['Backup e cópia', 'Guardar, conferir, restaurar e copiar', [
    [9, 'Fazer backup agora.'],
    [10, 'Conferir um backup.'],
    [24, 'Backup e restauração do banco.'],
    [11, 'Restaurar um backup.'],
    [23, 'Copiar e colar tabela entre bancos.'],
  ]],
  ['Replicação e ligações', 'Réplica, cluster e DbLink', [
    [68, 'Replicação.'],
    [69, 'Cluster.'],
    [80, 'Definições do DbLink.'],
  ]],
  ['Monitorar', 'Sessões, uso, telemetria e serviço', [
    [54, 'Sessões agora.'],
    [66, 'Sessões e conexões.'],
    [55, 'Estatísticas de uso.', 'também em Ferramentas'],
    [67, 'As mesmas estatísticas, pelo menu Ferramentas.'],
    [56, 'De onde vêm os acessos.'],
    [70, 'Telemetria ao vivo.'],
    [71, 'Profiler.'],
    [64, 'Serviço.'],
    [53, 'Jobs de execução.', 'também em Ferramentas'],
    [65, 'Os mesmos jobs, pelo menu Ferramentas.'],
  ]],
  ['Dado pessoal (LGPD)', 'O que é dado pessoal e onde está', [
    [22, 'Dado pessoal no banco.', 'também em Administração'],
    [52, 'O mesmo, pelo menu Administração.'],
  ]],
  ['Avançado e ajuda', 'Integrações, layout e sobre', [
    [57, 'Configuração do servidor (Administração).'],
    [83, 'Integração com a Claude.'],
    [84, 'Editor de menu.'],
    [92, 'Quatro regiões: o modo multitela.'],
    [95, 'Sobre o modo multitela.'],
    [96, 'Sobre o PhxSql.'],
    [97, 'Quem fez.'],
  ]],
];

// Confere: todas as 79 telas, nenhuma repetida
const usados = ROTEIRO.flatMap(c => c[2].map(t => t[0]));
const todos = Object.keys(telas).map(Number);
const falta = todos.filter(i => !usados.includes(i));
const repet = usados.filter((i, k) => usados.indexOf(i) !== k);
const fantasma = usados.filter(i => !todos.includes(i));
if (falta.length || repet.length || fantasma.length) {
  console.error('roteiro errado', { falta, repet, fantasma }); process.exit(1);
}
const TOTAL = usados.length;

const nomeDe = i => telas[i].titulo.split(' — ')[1];

const pres = new pptxgen();
pres.layout = 'LAYOUT_WIDE'; // 13.333 x 7.5
pres.title = 'PhxSql — as telas na sequência de uso';

function fundo(s) { s.background = { color: COR.fundo }; }

// Capa
{
  const s = pres.addSlide(); fundo(s);
  s.addImage({ path: 'phxsql-logo-560.png', x: 0.55, y: 0.35, w: 2.3, h: 2.3 });
  s.addText('As telas do console', { x: 0.7, y: 2.7, w: 8, h: 1.0, fontFace: TIT, fontSize: 44, bold: true, color: COR.prata, isTextBox: true, margin: 0 });
  s.addText('na sequência de uso', { x: 0.7, y: 3.6, w: 8, h: 0.7, fontFace: TIT, fontSize: 28, color: COR.laranja, isTextBox: true, margin: 0 });
  s.addText([
    { text: `${TOTAL} telas · ${ROTEIRO.length} capítulos`, options: { breakLine: true } },
    { text: 'Capturadas do servidor real, tema escuro, commit 6fc8244e (01/10/2026)' },
  ], { x: 0.7, y: 5.3, w: 8, h: 0.9, fontFace: CORPO, fontSize: 16, color: COR.mudo, isTextBox: true, margin: 0 });
  s.addText('Built to store. Engineered to scale.', { x: 0.7, y: 6.6, w: 8, h: 0.4, fontFace: TIT, fontSize: 14, italic: true, color: COR.ambar, isTextBox: true, margin: 0 });
  s.addImage({ path: 'phxsql-simbolo-440.png', x: 8.4, y: 2.2, w: 4.4, h: 4.4 * 262 / 440 });
}

// Sumario
{
  const s = pres.addSlide(); fundo(s);
  s.addText('O caminho', { x: 0.6, y: 0.4, w: 12, h: 0.8, fontFace: TIT, fontSize: 36, bold: true, color: COR.prata, isTextBox: true, margin: 0 });
  let passo = 1;
  ROTEIRO.forEach((c, k) => {
    const col = k < 7 ? 0 : 1, lin = k % 7;
    const x = 0.6 + col * 6.2, y = 1.45 + lin * 0.82;
    s.addShape(pres.shapes.ROUNDED_RECTANGLE, { x, y, w: 5.9, h: 0.68, fill: { color: COR.painel }, rectRadius: 0.08, line: { color: COR.painel } });
    s.addText(String(k + 1).padStart(2, '0'), { x: x + 0.15, y, w: 0.7, h: 0.68, fontFace: TIT, fontSize: 22, bold: true, color: COR.ambar, valign: 'middle', isTextBox: true, margin: 0 });
    s.addText([
      { text: c[0], options: { bold: true, color: COR.prata, breakLine: true } },
      { text: `passos ${passo}–${passo + c[2].length - 1} · ${c[1]}`, options: { color: COR.mudo, fontSize: 11 } },
    ], { x: x + 0.9, y, w: 4.9, h: 0.68, fontFace: CORPO, fontSize: 15, valign: 'middle', isTextBox: true, margin: 0 });
    passo += c[2].length;
  });
}

let passo = 1;
ROTEIRO.forEach((c, k) => {
  // Abertura do capitulo
  {
    const s = pres.addSlide(); fundo(s);
    s.addText(String(k + 1).padStart(2, '0'), { x: 0.7, y: 1.0, w: 3.5, h: 1.95, fontFace: TIT, fontSize: 110, bold: true, color: COR.ambar, isTextBox: true, margin: 0 });
    s.addText(c[0], { x: 0.7, y: 3.1, w: 7.5, h: 1.0, fontFace: TIT, fontSize: 40, bold: true, color: COR.prata, isTextBox: true, margin: 0 });
    s.addText(c[1], { x: 0.7, y: 4.1, w: 7.5, h: 0.6, fontFace: CORPO, fontSize: 18, color: COR.laranja, isTextBox: true, margin: 0 });
    const itens = c[2].map((t, j) => ({ text: `${passo + j}. ${nomeDe(t[0])}`, options: { breakLine: j < c[2].length - 1 } }));
    const h = Math.min(6.2, 0.36 * c[2].length + 0.3);
    s.addShape(pres.shapes.ROUNDED_RECTANGLE, { x: 8.6, y: 0.65, w: 4.2, h: h + 0.2, fill: { color: COR.painel }, rectRadius: 0.1, line: { color: COR.painel } });
    s.addText(itens, { x: 8.85, y: 0.75, w: 3.8, h, fontFace: CORPO, fontSize: c[2].length > 11 ? 11 : 13, color: COR.prata, valign: 'top', isTextBox: true, margin: 0, paraSpaceAfter: 3 });
  }
  // Uma tela por slide
  c[2].forEach(([id, desc, tambem]) => {
    const s = pres.addSlide(); fundo(s);
    const t = telas[id];
    s.addText(`${String(k + 1).padStart(2, '0')} · ${c[0].toUpperCase()}`, { x: 0.5, y: 0.25, w: 9, h: 0.3, fontFace: CORPO, fontSize: 11, bold: true, color: COR.laranja, charSpacing: 2, isTextBox: true, margin: 0 });
    s.addText(nomeDe(id), { x: 0.5, y: 0.55, w: 9.6, h: 0.6, fontFace: TIT, fontSize: 26, bold: true, color: COR.prata, isTextBox: true, margin: 0, fit: 'shrink' });
    // Moldura arredondada: o motivo visual do deck
    const ix = 0.5, iy = 1.35, iw = 9.6;
    const ih = Math.min(5.75, iw * t.h / t.w);
    const w = ih * t.w / t.h;
    s.addShape(pres.shapes.ROUNDED_RECTANGLE, { x: ix - 0.08, y: iy - 0.08, w: w + 0.16, h: ih + 0.16, fill: { color: COR.painel }, rectRadius: 0.1, line: { color: '1C2740', width: 1 } });
    s.addImage({ path: t.png, x: ix, y: iy, w, h: ih });
    // Coluna lateral
    s.addText(String(passo), { x: 10.45, y: 1.3, w: 2.4, h: 1.1, fontFace: TIT, fontSize: 60, bold: true, color: COR.ambar, isTextBox: true, margin: 0 });
    s.addText(`de ${TOTAL}`, { x: 10.45, y: 2.35, w: 2.4, h: 0.35, fontFace: CORPO, fontSize: 13, color: COR.mudo, isTextBox: true, margin: 0 });
    s.addText(desc, { x: 10.45, y: 2.95, w: 2.4, h: 2.4, fontFace: CORPO, fontSize: 15, color: COR.prata, valign: 'top', isTextBox: true, margin: 0 });
    if (tambem) s.addText(tambem, { x: 10.45, y: 5.4, w: 2.4, h: 0.4, fontFace: CORPO, fontSize: 11, italic: true, color: COR.laranja, isTextBox: true, margin: 0 });
    s.addText(id === 0 ? 'tela inicial' : 'menu ' + nomeDe(id).split(' › ')[0], { x: 10.45, y: 6.7, w: 2.4, h: 0.35, fontFace: CORPO, fontSize: 10, color: COR.mudo, isTextBox: true, margin: 0 });
    s.addNotes(`${t.titulo}. ${desc}`);
    passo++;
  });
});

// Fecho: o que nao virou tela
{
  const s = pres.addSlide(); fundo(s);
  s.addText('O que não entrou, e por quê', { x: 0.6, y: 0.4, w: 12, h: 0.8, fontFace: TIT, fontSize: 32, bold: true, color: COR.prata, isTextBox: true, margin: 0 });
  const blocos = [
    ['6', 'abrem diálogo do navegador', 'Novo database, Duplicar, Reparar índice, Reparar pelo espelho, Excluir tabela, Reparar — a caixa nativa não sai na captura.'],
    ['2', 'desligados no código', 'Server Mail e Blockchain: o item existe no menu e ainda não abre tela.'],
    ['10', 'são ações, não telas', 'Sair, alternar o tema, Atualizar e as ações de layout do menu Ver.'],
  ];
  blocos.forEach((b, k) => {
    const x = 0.6 + k * 4.15;
    s.addShape(pres.shapes.ROUNDED_RECTANGLE, { x, y: 1.6, w: 3.9, h: 3.6, fill: { color: COR.painel }, rectRadius: 0.1, line: { color: COR.painel } });
    s.addText(b[0], { x: x + 0.3, y: 1.8, w: 3.3, h: 1.2, fontFace: TIT, fontSize: 64, bold: true, color: COR.ambar, isTextBox: true, margin: 0 });
    s.addText(b[1], { x: x + 0.3, y: 3.0, w: 3.3, h: 0.5, fontFace: CORPO, fontSize: 17, bold: true, color: COR.prata, isTextBox: true, margin: 0 });
    s.addText(b[2], { x: x + 0.3, y: 3.5, w: 3.3, h: 1.6, fontFace: CORPO, fontSize: 13, color: COR.mudo, valign: 'top', isTextBox: true, margin: 0 });
  });
  s.addText('As 79 telas também existem no tema claro, em SVG, no pacote telas-svg.zip.', { x: 0.6, y: 5.7, w: 12, h: 0.5, fontFace: CORPO, fontSize: 15, color: COR.prata, isTextBox: true, margin: 0 });
  s.addText('Built to store. Engineered to scale.', { x: 0.6, y: 6.6, w: 12, h: 0.4, fontFace: TIT, fontSize: 14, italic: true, color: COR.ambar, isTextBox: true, margin: 0 });
}

pres.writeFile({ fileName: 'PhxSql-telas-na-sequencia-de-uso.pptx' }).then(f => console.log('gravado', f, TOTAL, 'telas'));

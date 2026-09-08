// Gera docs/apresentacao-wx-claude-code.pptx. Precisa de pptxgenjs, react-icons, react, react-dom e sharp
// (npm install numa pasta a parte e NODE_PATH=<ela>/node_modules); os numeros vem de numeros.json.
import pptxgen from 'pptxgenjs';
import { readFileSync } from 'node:fs';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import sharp from 'sharp';
import { FiShield, FiCheckCircle, FiGitBranch, FiFileText, FiLayers, FiCpu, FiUsers, FiLock, FiBarChart2, FiBox, FiSearch, FiZap, FiXCircle, FiPlay, FiDatabase, FiMonitor } from 'react-icons/fi';

const n = JSON.parse(readFileSync('/home/user/adrianoboller/wx-claude-code/docs/dossie/numeros.json', 'utf8'));
const MARCA = 'image/png;base64,' + readFileSync('/home/user/adrianoboller/wx-claude-code/marca-wx-claude-code.png').toString('base64');
const PRINT = (f) => 'image/png;base64,' + readFileSync('/home/user/adrianoboller/wx-claude-code/docs/prints/' + f).toString('base64');
const versao = JSON.parse(readFileSync('/home/user/adrianoboller/wx-claude-code/.claude-plugin/plugin.json', 'utf8')).version;

// paleta da marca do plugin: fundo profundo, vermelhão, e os tons dos vídeos
const P = { fundo: '010418', painel: '121527', linha: '2E3454', texto: 'EDEDF3', mudo: '9AA0B8', ver: 'E2261C', verde: '2FBF71', ouro: 'F7B733', azul: '8FD3FF', papel: 'FBFAF7', tinta: '14161F', cinza: '6B6F82', luz: 'E4E2DB' };
const F = { t: 'Century Schoolbook', b: 'Calibri' };

async function icone(Comp, cor, tam = 256) {
  const svg = renderToStaticMarkup(React.createElement(Comp, { size: tam, color: '#' + cor, strokeWidth: 1.8 }));
  const png = await sharp(Buffer.from(svg)).png().toBuffer();
  return 'image/png;base64,' + png.toString('base64');
}

const pres = new pptxgen();
pres.layout = 'LAYOUT_WIDE'; // 13.33 x 7.5
pres.author = 'WX Claude Code';
pres.title = 'WX Claude Code — vantagens e benefícios';

function escuro(s) { s.background = { color: P.fundo }; }
function claro(s) { s.background = { color: P.papel }; }
function titulo(s, t, cor = P.tinta, y = 0.45) {
  s.addText(t, { x: 0.6, y, w: 12.1, h: 0.9, fontFace: F.t, fontSize: 34, bold: true, color: cor, isTextBox: true, margin: 0 });
}
function rodape(s, escuroP) {
  s.addText(`WX Claude Code ${versao} · Built to convert. Engineered to prove.`, { x: 0.6, y: 7.0, w: 9, h: 0.3, fontFace: F.b, fontSize: 10, color: escuroP ? P.mudo : P.cinza, isTextBox: true, margin: 0 });
}
function circulo(s, x, y, d, cor, dado) {
  s.addShape(pres.ShapeType.ellipse, { x, y, w: d, h: d, fill: { color: cor } });
  s.addImage({ data: dado, x: x + d * 0.22, y: y + d * 0.22, w: d * 0.56, h: d * 0.56 });
}

const ic = {};
for (const [k, C, cor] of [['shield', FiShield, 'FFFFFF'], ['check', FiCheckCircle, 'FFFFFF'], ['git', FiGitBranch, 'FFFFFF'], ['file', FiFileText, 'FFFFFF'], ['layers', FiLayers, 'FFFFFF'], ['cpu', FiCpu, 'FFFFFF'], ['users', FiUsers, 'FFFFFF'], ['lock', FiLock, 'FFFFFF'], ['chart', FiBarChart2, 'FFFFFF'], ['box', FiBox, 'FFFFFF'], ['search', FiSearch, 'FFFFFF'], ['zap', FiZap, 'FFFFFF'], ['x', FiXCircle, 'FFFFFF'], ['play', FiPlay, 'FFFFFF'], ['db', FiDatabase, 'FFFFFF'], ['monitor', FiMonitor, 'FFFFFF']]) ic[k] = await icone(C, cor);

// 1 · capa ------------------------------------------------------------
{
  const s = pres.addSlide(); escuro(s);
  s.addImage({ data: MARCA, x: 0.9, y: 1.2, w: 3.6, h: 3.6 });
  s.addText('WX Claude Code', { x: 5.0, y: 1.6, w: 7.8, h: 1.1, fontFace: F.t, fontSize: 48, bold: true, color: 'FFFFFF', isTextBox: true, margin: 0 });
  s.addText('Conversão governada de WINDEV, WEBDEV, WINDEV Mobile e PHP — e projetos novos que nascem com contexto, provas e governança — dentro do Claude Code.', { x: 5.0, y: 2.8, w: 7.6, h: 1.4, fontFace: F.b, fontSize: 18, color: P.mudo, isTextBox: true, margin: 0 });
  s.addText('Built to convert. Engineered to prove.', { x: 5.0, y: 4.4, w: 7.6, h: 0.5, fontFace: F.t, fontSize: 20, italic: true, color: P.ver, isTextBox: true, margin: 0 });
  s.addText(`versão ${versao} · ${n.medido_em}`, { x: 5.0, y: 5.0, w: 7.6, h: 0.4, fontFace: F.b, fontSize: 12, color: P.mudo, isTextBox: true, margin: 0 });
  s.addNotes('Abertura: o plugin roda dentro do Claude Code. Uma frase: converter com prova, não com palpite; e começar projeto novo já com contexto.');
}

// 2 · o problema -------------------------------------------------------
{
  const s = pres.addSlide(); claro(s);
  titulo(s, 'O problema que a IA sozinha não resolve');
  const itens = [
    ['A regra de negócio mora no código, não em documento', 'Desconto, juros, baixa de estoque: ninguém tem a lista. Quem converte lendo o código inventa o que não entendeu.'],
    ['Conversão por IA sem governança é palpite bonito', 'Compila, parece certo, e o arredondamento do dinheiro mudou. Só aparece em produção.'],
    ['Sessão nova esquece tudo', 'O que a sessão de ontem decidiu não chega à de hoje. O projeto vira uma sequência de recomeços.'],
    ['Quem valida é quem escreveu', 'Sem separação de papéis, o mesmo agente escreve e aprova. Ninguém tenta refutar.'],
  ];
  itens.forEach(([t, d], i) => {
    const y = 1.6 + i * 1.3;
    circulo(s, 0.7, y + 0.05, 0.7, P.ver, ic.x);
    s.addText(t, { x: 1.7, y, w: 10.8, h: 0.4, fontFace: F.b, fontSize: 18, bold: true, color: P.tinta, isTextBox: true, margin: 0 });
    s.addText(d, { x: 1.7, y: y + 0.42, w: 10.8, h: 0.6, fontFace: F.b, fontSize: 14, color: P.cinza, isTextBox: true, margin: 0 });
  });
  rodape(s, false);
}

// 3 · o que é, com numeros medidos --------------------------------------
{
  const s = pres.addSlide(); escuro(s);
  titulo(s, 'O que é: um plugin do Claude Code, medido', 'FFFFFF');
  s.addText('Instala uma vez por máquina e vale para todos os projetos. Os números saem do repositório, não de slide.', { x: 0.6, y: 1.35, w: 12, h: 0.5, fontFace: F.b, fontSize: 16, color: P.mudo, isTextBox: true, margin: 0 });
  const kpis = [[n.comandos, 'comandos'], [n.agentes, 'agentes'], [n.skills, 'skills'], [n.testes, 'testes'], [n.especialistas_wl, 'especialistas WLanguage'], [n.corpus_paginas_validas.toLocaleString('pt-BR'), 'páginas do Help']];
  kpis.forEach(([v, r], i) => {
    const x = 0.6 + (i % 3) * 4.1, y = 2.2 + Math.floor(i / 3) * 2.2;
    s.addShape(pres.ShapeType.roundRect, { x, y, w: 3.8, h: 1.9, fill: { color: P.painel }, line: { color: P.linha, width: 1 }, rectRadius: 0.15 });
    s.addText(String(v), { x: x + 0.3, y: y + 0.25, w: 3.2, h: 0.9, fontFace: F.t, fontSize: 48, bold: true, color: P.ouro, isTextBox: true, margin: 0 });
    s.addText(r, { x: x + 0.3, y: y + 1.2, w: 3.2, h: 0.4, fontFace: F.b, fontSize: 14, color: P.mudo, isTextBox: true, margin: 0 });
  });
  s.addText('Zero dependências fora da std do Python e do Rust. Corpus do Help WLanguage de 12 mil páginas, verificado por hash.', { x: 0.6, y: 6.5, w: 12, h: 0.4, fontFace: F.b, fontSize: 12, color: P.mudo, isTextBox: true, margin: 0 });
  rodape(s, true);
}

// 4 · como funciona: 6 etapas --------------------------------------------
{
  const s = pres.addSlide(); claro(s);
  titulo(s, 'Como funciona: seis etapas e dois portões');
  const etapas = [['1', 'Perguntar', 'Questionário de 60 perguntas gera o contexto que a primeira sessão lê', ic.search],
    ['2', 'G0 · portão', 'Evidências com hash. Bloqueado, ninguém escreve código', ic.shield],
    ['3', 'Converter', 'Gates G1–G7, com matriz de rastreabilidade e aprovação humana', ic.git],
    ['4', 'Provar', 'Golden master, evidências, grafo, tela exercitada', ic.check],
    ['5', 'Governar', 'PMO com sprints, contrato ativo, papéis separados', ic.users],
    ['6', 'Entregar', 'Sete pastas com SHA-256, procedência SLSA e CycloneDX', ic.box]];
  etapas.forEach(([num, t, d, icn], i) => {
    const x = 0.6 + (i % 3) * 4.1, y = 1.7 + Math.floor(i / 3) * 2.6;
    s.addShape(pres.ShapeType.roundRect, { x, y, w: 3.8, h: 2.3, fill: { color: 'FFFFFF' }, line: { color: P.luz, width: 1 }, rectRadius: 0.12, shadow: { type: 'outer', blur: 6, offset: 2, angle: 90, color: '000000', opacity: 0.08 } });
    circulo(s, x + 0.25, y + 0.25, 0.65, i === 1 || i === 3 ? P.ver : P.tinta, icn);
    s.addText(`${num} · ${t}`, { x: x + 1.05, y: y + 0.3, w: 2.6, h: 0.55, fontFace: F.b, fontSize: 18, bold: true, color: P.tinta, isTextBox: true, margin: 0 });
    s.addText(d, { x: x + 0.25, y: y + 1.05, w: 3.3, h: 1.1, fontFace: F.b, fontSize: 13, color: P.cinza, isTextBox: true, margin: 0 });
  });
  s.addText('Os dois em vermelho negam: o G0 devolve para as evidências; F-GATE e C-GATE devolvem ao gate da conversão.', { x: 0.6, y: 6.55, w: 12, h: 0.4, fontFace: F.b, fontSize: 12, italic: true, color: P.cinza, isTextBox: true, margin: 0 });
  rodape(s, false);
}

// 5 · vantagens na conversao ----------------------------------------------
{
  const s = pres.addSlide(); claro(s);
  titulo(s, 'Na conversão: origem, prova e limite');
  const linhas = [
    [ic.file, 'PDF citável', 'Cada PDF do WINDEV vira Markdown com página e hash. O código convertido cita a página de origem dentro dele.'],
    [ic.git, 'Matriz de rastreabilidade', 'BR, QRY, UI, DB, INT: cada item com origem, destino, teste e estado. O grafo acusa o que ficou sem ligação.'],
    [ic.chart, 'Golden master', 'Captura o legado rodando e compara com o novo. Igualdade vira número, com tolerância declarada.'],
    [ic.layers, 'Semântica do WLanguage', 'Sete especialistas por tema sobre o Help, e o runtime wl-rt em Rust: currency de ponto fixo, datas, comparação de strings como o WX.'],
    [ic.monitor, 'Tela provada, não lida', 'Playwright exercita a tela nos estados que o PDF de interfaces descreve. O defeito que só aparece olhando entra na prova.'],
  ];
  linhas.forEach(([icn, t, d], i) => {
    const y = 1.55 + i * 1.05;
    circulo(s, 0.7, y, 0.6, P.tinta, icn);
    s.addText(t, { x: 1.6, y: y - 0.02, w: 3.6, h: 0.6, fontFace: F.b, fontSize: 16, bold: true, color: P.tinta, isTextBox: true, margin: 0, valign: 'top' });
    s.addText(d, { x: 5.4, y: y - 0.02, w: 7.3, h: 0.9, fontFace: F.b, fontSize: 13.5, color: P.cinza, isTextBox: true, margin: 0, valign: 'top' });
  });
  rodape(s, false);
}

// 6 · prova, nao promessa ---------------------------------------------------
{
  const s = pres.addSlide(); escuro(s);
  titulo(s, 'Prova, não promessa', 'FFFFFF');
  const d = n.destinos;
  const cards = [[d[1].golden, 'golden master', 'WINDEV 2025 (só PDFs) → Rust + Axum + PostgreSQL + React'], [d[0].golden, 'golden master', 'PHP + MySQL → Rust + MySQL + React, o primeiro projeto de ponta a ponta'], [String(Object.keys(n).filter(k => k.endsWith('_cenas')).length), 'vídeos', 'Todos de saída real de sessão; nenhuma linha digitada'], [`${n.testes}`, 'testes', `Mais ${n.testes_wl_rt} do runtime Rust, cada um com vetor do Help`]];
  cards.forEach(([v, r, t], i) => {
    const x = 0.6 + (i % 2) * 6.2, y = 1.6 + Math.floor(i / 2) * 2.4;
    s.addShape(pres.ShapeType.roundRect, { x, y, w: 5.9, h: 2.1, fill: { color: P.painel }, line: { color: P.linha, width: 1 }, rectRadius: 0.15 });
    s.addText(v, { x: x + 0.3, y: y + 0.25, w: 2.3, h: 1.0, fontFace: F.t, fontSize: 46, bold: true, color: P.verde, isTextBox: true, margin: 0 });
    s.addText(r, { x: x + 0.3, y: y + 1.25, w: 2.3, h: 0.4, fontFace: F.b, fontSize: 13, color: P.mudo, isTextBox: true, margin: 0 });
    s.addText(t, { x: x + 2.7, y: y + 0.35, w: 3.0, h: 1.5, fontFace: F.b, fontSize: 14, color: P.texto, isTextBox: true, margin: 0 });
  });
  s.addText('Regra do projeto: todo teste novo falha com o defeito reposto e passa com o conserto. Teste que passa por engano é pior que teste que falta.', { x: 0.6, y: 6.5, w: 12.1, h: 0.45, fontFace: F.b, fontSize: 12, italic: true, color: P.mudo, isTextBox: true, margin: 0 });
  rodape(s, true);
}

// 7 · o codigo que sai --------------------------------------------------------
{
  const s = pres.addSlide(); claro(s);
  titulo(s, 'O que sai: código que cita a origem');
  s.addImage({ data: PRINT('48-rust-gerado.png'), x: 0.6, y: 1.5, w: 6.0, h: 3.9, rounding: false });
  s.addImage({ data: PRINT('49-cargo-test-e-semantica.png'), x: 6.75, y: 1.5, w: 6.0, h: 3.9 });
  s.addText('Rust gerado de uma procedure WLanguage, com a página do PDF citada no comentário.', { x: 0.6, y: 5.5, w: 6.0, h: 0.6, fontFace: F.b, fontSize: 12, color: P.cinza, isTextBox: true, margin: 0 });
  s.addText('O cargo test contra o golden master — e o que mudou de semântica dito, não escondido.', { x: 6.75, y: 5.5, w: 6.0, h: 0.6, fontFace: F.b, fontSize: 12, color: P.cinza, isTextBox: true, margin: 0 });
  s.addText('Capturas de sessão real, em docs/prints/.', { x: 0.6, y: 6.3, w: 12, h: 0.4, fontFace: F.b, fontSize: 11, italic: true, color: P.cinza, isTextBox: true, margin: 0 });
  rodape(s, false);
}

// 8 · projetos novos ------------------------------------------------------------
{
  const s = pres.addSlide(); claro(s);
  titulo(s, 'Projetos novos: nascer com contexto');
  s.addText('O mesmo questionário serve para o projeto que começa do zero. O que ele gera é o que a primeira sessão do Claude Code lê antes de qualquer comando.', { x: 0.6, y: 1.4, w: 12, h: 0.7, fontFace: F.b, fontSize: 15, color: P.cinza, isTextBox: true, margin: 0 });
  const blocos = [[ic.file, 'Contexto', `${n.arquivos_gerados_pelo_questionario} arquivos: CLAUDE.md, INDEX_FILES, respostas por id, ADRs, esqueleto de pastas`],
    [ic.zap, 'Primeira sessão', 'Prompt de kickoff, hooks de teste e lint, MCP sem chaves, Dockerfile e compose por perfil'],
    [ic.users, 'PMO', 'Sprints, Kanban que segue a matriz sozinho, PDCA com base de conhecimento, orçamento de tokens'],
    [ic.shield, 'Restrições', 'Regras com validador executável. O C-GATE confere se o resultado está conforme'],
    [ic.lock, 'Guardas', `${n.hooks} hooks: anexos somente leitura, segredo recusado, papel da sessão, licença`],
    [ic.box, 'Entrega auditável', 'Exportação com SHA-256, procedência SLSA e CycloneDX, decisões com a base delas']];
  blocos.forEach(([icn, t, d], i) => {
    const x = 0.6 + (i % 3) * 4.1, y = 2.3 + Math.floor(i / 3) * 2.15;
    circulo(s, x, y, 0.6, P.tinta, icn);
    s.addText(t, { x: x + 0.8, y, w: 3.0, h: 0.6, fontFace: F.b, fontSize: 17, bold: true, color: P.tinta, isTextBox: true, margin: 0, valign: 'middle' });
    s.addText(d, { x, y: y + 0.75, w: 3.8, h: 1.2, fontFace: F.b, fontSize: 13, color: P.cinza, isTextBox: true, margin: 0 });
  });
  rodape(s, false);
}

// 9 · beneficios ao negocio -------------------------------------------------------
{
  const s = pres.addSlide(); escuro(s);
  titulo(s, 'Benefícios para quem paga a conta', 'FFFFFF');
  const cols = [
    ['Custo', ['Laudo de tokens em três fases, só leitura', 'Modelo local pelo Magnitude para o que não precisa sair da máquina', 'Roteador escolhe o modelo pelo peso da tarefa, com orçamento que bloqueia']],
    ['Risco', ['Nada afirmado sem baseline: classe da prova dita no G0', 'Quem valida não escreve o que valida', 'O que o PDF não diz vira GAP escrito, não invenção']],
    ['Auditoria', ['Registro de toda operação do plugin, com código de saída', 'Procedência SLSA e BOM CycloneDX medidos', 'Licença com aceite, aviso ao fornecedor e marca d’água']],
  ];
  cols.forEach(([t, itens], i) => {
    const x = 0.6 + i * 4.15;
    s.addShape(pres.ShapeType.roundRect, { x, y: 1.6, w: 3.9, h: 4.3, fill: { color: P.painel }, line: { color: P.linha, width: 1 }, rectRadius: 0.15 });
    s.addText(t, { x: x + 0.3, y: 1.85, w: 3.3, h: 0.5, fontFace: F.t, fontSize: 24, bold: true, color: P.ouro, isTextBox: true, margin: 0 });
    s.addText(itens.map((tx, k) => ({ text: tx, options: { bullet: true, breakLine: k < itens.length - 1, paraSpaceAfter: 10 } })), { x: x + 0.3, y: 2.5, w: 3.3, h: 3.7, fontFace: F.b, fontSize: 14, color: P.texto, isTextBox: true, margin: 0, valign: 'top' });
  });
  rodape(s, true);
}

// 10 · destinos -------------------------------------------------------------------
{
  const s = pres.addSlide(); claro(s);
  titulo(s, 'O legado é E/OU. O destino é livre.');
  s.addText('Origem', { x: 0.6, y: 1.5, w: 5.5, h: 0.5, fontFace: F.t, fontSize: 22, bold: true, color: P.tinta, isTextBox: true, margin: 0 });
  s.addText('Destino', { x: 6.9, y: 1.5, w: 5.8, h: 0.5, fontFace: F.t, fontSize: 22, bold: true, color: P.tinta, isTextBox: true, margin: 0 });
  const orig = ['WINDEV, WEBDEV, WINDEV Mobile (WLanguage) — o caso principal, com o Help de 12 mil páginas', 'PHP procedural ou OOP', 'C, C++, Clarion, COBOL', 'Só PDFs, sem projeto nativo: o G0 classifica a prova como FORENSIC e diz o limite'];
  const dest = ['Rust (Axum ou tiny_http) com o runtime wl-rt', 'Python, Go, Java, Node', 'C# .NET 8 com a biblioteca WL_C#', 'PHP 8.3', 'Telas: React, Vue, Svelte, Blazor, Flutter, Tauri'];
  s.addText(orig.map((t, k) => ({ text: t, options: { bullet: true, breakLine: k < orig.length - 1, paraSpaceAfter: 10 } })), { x: 0.6, y: 2.1, w: 5.6, h: 3.6, fontFace: F.b, fontSize: 15, color: P.tinta, isTextBox: true, margin: 0, valign: 'top' });
  s.addText(dest.map((t, k) => ({ text: t, options: { bullet: true, breakLine: k < dest.length - 1, paraSpaceAfter: 10 } })), { x: 6.9, y: 2.1, w: 5.8, h: 3.6, fontFace: F.b, fontSize: 15, color: P.tinta, isTextBox: true, margin: 0, valign: 'top' });
  s.addShape(pres.ShapeType.roundRect, { x: 0.6, y: 5.2, w: 12.1, h: 0.85, fill: { color: P.tinta }, rectRadius: 0.12 });
  s.addText('A letra H orienta antes de perguntar: três opções com o porquê, a recomendada primeiro. A escolha é sua e vira decisão registrada.', { x: 0.9, y: 5.25, w: 11.5, h: 0.75, fontFace: F.b, fontSize: 14, color: 'FFFFFF', isTextBox: true, margin: 0, valign: 'middle' });
  rodape(s, false);
}

// 11 · o que ele nao faz --------------------------------------------------------------
{
  const s = pres.addSlide(); claro(s);
  titulo(s, 'O que ele não faz, dito antes');
  const itens = ['Não lê o formato binário do WX; o projeto nativo é o item 1 da lista do que falta', 'Não faz OCR sozinho: página sem texto é marcada, não inventada', 'Não certifica LGPD nem aprova gate no lugar do humano', 'Não afirma equivalência sem baseline executável do legado', 'A licença dissuade o cliente honesto; a proteção real é servir o corpus de um servidor seu'];
  itens.forEach((t, i) => {
    const y = 1.6 + i * 0.95;
    circulo(s, 0.7, y, 0.55, P.ver, ic.x);
    s.addText(t, { x: 1.5, y, w: 11.2, h: 0.55, fontFace: F.b, fontSize: 16, color: P.tinta, isTextBox: true, margin: 0, valign: 'middle' });
  });
  s.addText('A lista completa do que falta está em docs/o-que-falta.html, com estado medido por item.', { x: 0.6, y: 6.45, w: 12, h: 0.4, fontFace: F.b, fontSize: 12, italic: true, color: P.cinza, isTextBox: true, margin: 0 });
  rodape(s, false);
}


// 12a · por que comprar: os argumentos, um por cartao ------------------------------
function porque(titulo, itens, escuroP) {
  const s = pres.addSlide(); if (escuroP) escuro(s); else claro(s);
  titulo_(s, titulo, escuroP);
  itens.forEach(([t, d], i) => {
    const x = 0.6 + (i % 2) * 6.2, y = 1.55 + Math.floor(i / 2) * 2.55;
    s.addShape(pres.ShapeType.roundRect, { x, y, w: 5.9, h: 2.3, fill: { color: escuroP ? P.painel : 'FFFFFF' }, line: { color: escuroP ? P.linha : P.luz, width: 1 }, rectRadius: 0.15, shadow: escuroP ? undefined : { type: 'outer', blur: 6, offset: 2, angle: 90, color: '000000', opacity: 0.08 } });
    s.addText(t, { x: x + 0.3, y: y + 0.22, w: 5.3, h: 0.6, fontFace: F.t, fontSize: 18, bold: true, color: escuroP ? P.ouro : P.ver, isTextBox: true, margin: 0, valign: 'top' });
    s.addText(d, { x: x + 0.3, y: y + 0.85, w: 5.3, h: 1.35, fontFace: F.b, fontSize: 13, color: escuroP ? P.texto : P.cinza, isTextBox: true, margin: 0, valign: 'top' });
  });
  rodape(s, escuroP);
}
function titulo_(s, t, escuroP) { titulo(s, t, escuroP ? 'FFFFFF' : P.tinta); }
porque('Por que comprar: a prova', [
  ['O que você compra é a prova', 'Qualquer sessão converte uma procedure em minutos. O que ela não faz sozinha é dizer se a regra ficou igual. O plugin captura o golden master do legado rodando, compara e devolve um número.'],
  ['Impede o erro mais caro: inventar', 'Se o PDF não diz como a procedure funciona, vira lacuna escrita e linha bloqueada. Não completa por conta própria. O exemplo WINDEV tem uma lacuna plantada, e o grafo a acusa até hoje.'],
  ['Sessões não esquecem mais', 'O questionário vira o contexto que toda sessão lê antes do primeiro comando. O contrato ativo diz o que vale hoje, com hash. Decisões guardam a base delas.'],
  ['Quem escreve não valida', 'O G0 nega escrita enquanto as evidências não estão em ordem. Dois portões na saída perguntam se funciona e se está conforme. Quem valida entra com outro papel e não conserta o que detecta.'],
], true);
porque('Por que comprar: o resto do argumento', [
  ['Sabe WLanguage de verdade', `${n.especialistas_wl} especialistas por tema sobre ${n.corpus_paginas_validas.toLocaleString('pt-BR')} páginas do Help, e um runtime em Rust que faz o Round e a comparação de strings como o WINDEV. Saiu de ler o Help, não da memória do modelo.`],
  ['Serve para projeto novo', 'O mesmo questionário gera o esqueleto, os hooks, o PMO e a entrega auditável para quem não tem legado nenhum.'],
  ['Custa menos do que parece', 'Laudo de uso de tokens, roteador que escolhe o modelo pelo peso da tarefa, orçamento que bloqueia, modelo local para o que não precisa sair da máquina.'],
  ['O mais forte: o que ele admite não fazer', 'Não lê o binário do WX, não faz OCR sozinho, não certifica LGPD, não aprova gate no lugar de gente. Quem esconde limites vende promessa. Quem publica limites vende ferramenta.'],
], false);
{
  const s = pres.addSlide(); escuro(s);
  titulo(s, 'A objeção honesta', 'FFFFFF');
  s.addShape(pres.ShapeType.roundRect, { x: 0.6, y: 1.7, w: 12.1, h: 3.2, fill: { color: P.painel }, line: { color: P.linha, width: 1 }, rectRadius: 0.15 });
  s.addText('Nenhum projeto WINDEV real com contrato passou pelos gates ainda.', { x: 1.0, y: 2.0, w: 11.3, h: 1.0, fontFace: F.t, fontSize: 28, bold: true, color: P.ouro, isTextBox: true, margin: 0 });
  s.addText('Os exemplos são sintéticos. Isso está escrito como item 15 da lista do que falta, com estado medido por item — e é o que um primeiro cliente piloto compra com desconto e paga com o número que falta: horas, tokens, lacunas abertas, defeitos achados em homologação.', { x: 1.0, y: 3.1, w: 11.3, h: 1.6, fontFace: F.b, fontSize: 16, color: P.texto, isTextBox: true, margin: 0 });
  s.addText('A lista inteira: docs/o-que-falta.html', { x: 0.6, y: 5.3, w: 12, h: 0.5, fontFace: F.b, fontSize: 13, italic: true, color: P.mudo, isTextBox: true, margin: 0 });
  rodape(s, true);
}

// 12 · como comecar -----------------------------------------------------------------------
{
  const s = pres.addSlide(); claro(s);
  titulo(s, 'Como começar: quatro passos');
  const passos = [['1', 'Instalar', './instalar.sh --serial … --corpus …', 'Uma vez por máquina. Aceita a licença, confere o corpus, instala no Claude Code.'],
    ['2', 'Perguntar', '/wx-claude-code:questionario', 'Sessenta perguntas, com pausa e retomada. Sai o contexto do projeto.'],
    ['3', 'Passar o G0', '/wx-claude-code:preflight', 'Evidências com hash. O relatório diz a classe da prova e o limite.'],
    ['4', 'Piloto com prova', '/wx-claude-code:golden comparar', 'Uma vertical inteira, igualdade em número. Daí, as ondas.']];
  passos.forEach(([num, t, cmd, d], i) => {
    const x = 0.6 + i * 3.1;
    s.addShape(pres.ShapeType.roundRect, { x, y: 1.6, w: 2.9, h: 4.6, fill: { color: 'FFFFFF' }, line: { color: P.luz, width: 1 }, rectRadius: 0.12, shadow: { type: 'outer', blur: 6, offset: 2, angle: 90, color: '000000', opacity: 0.08 } });
    s.addText(num, { x: x + 0.25, y: 1.8, w: 1, h: 0.9, fontFace: F.t, fontSize: 44, bold: true, color: P.ver, isTextBox: true, margin: 0 });
    s.addText(t, { x: x + 0.25, y: 2.75, w: 2.5, h: 0.5, fontFace: F.b, fontSize: 18, bold: true, color: P.tinta, isTextBox: true, margin: 0 });
    s.addText(cmd, { x: x + 0.25, y: 3.3, w: 2.5, h: 0.8, fontFace: 'Courier New', fontSize: 11, color: P.ver, isTextBox: true, margin: 0 });
    s.addText(d, { x: x + 0.25, y: 4.2, w: 2.5, h: 1.8, fontFace: F.b, fontSize: 13, color: P.cinza, isTextBox: true, margin: 0 });
  });
  s.addText('Manual: MANUAL.md e docs/manual-de-uso.pdf · Fluxograma: docs/dossie/fluxo-atual.pdf · Vídeos: docs/video/', { x: 0.6, y: 6.5, w: 12.1, h: 0.4, fontFace: F.b, fontSize: 12, color: P.cinza, isTextBox: true, margin: 0 });
  rodape(s, false);
}

// 13 · fecho ----------------------------------------------------------------------------------
{
  const s = pres.addSlide(); escuro(s);
  s.addImage({ data: MARCA, x: 4.9, y: 1.1, w: 3.5, h: 3.5 });
  s.addText('Built to convert. Engineered to prove.', { x: 0.6, y: 4.9, w: 12.1, h: 0.8, fontFace: F.t, fontSize: 34, bold: true, color: 'FFFFFF', align: 'center', isTextBox: true, margin: 0 });
  s.addText('claude plugin install wx-claude-code@wx-claude-code', { x: 0.6, y: 5.8, w: 12.1, h: 0.5, fontFace: 'Courier New', fontSize: 16, color: P.ouro, align: 'center', isTextBox: true, margin: 0 });
  s.addText('adrianoboller · github.com/adrianoboller/adrianoboller', { x: 0.6, y: 6.4, w: 12.1, h: 0.4, fontFace: F.b, fontSize: 13, color: P.mudo, align: 'center', isTextBox: true, margin: 0 });
}

await pres.writeFile({ fileName: process.argv[2] ?? 'apresentacao-wx-claude-code.pptx' });
console.log('ok');

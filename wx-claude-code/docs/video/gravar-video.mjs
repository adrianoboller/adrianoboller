// Grava um video de uso a partir das capturas reais (caps/*.txt): terminal
// animado com comando digitado e saida revelada linha a linha. Playwright grava
// em WebM (VP8), que e o que o ffmpeg do Playwright sabe codificar.
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { readFileSync, readdirSync, renameSync, rmSync } from 'node:fs';

const [, , outDir, capsDir, roteiroNome = 'uso'] = process.argv;
const cap = (n) => readFileSync(`${capsDir}/${n}.txt`, 'utf8').replace(/\/tmp\/claude-0\/[^ ]*?\/scratchpad\/(proj|pmo2|demo|ex2?)/g, '.');
const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
function fmt(line) {
  let l = esc(line);
  if (/^\$ /.test(l)) return `<span class="prompt">$</span> <span class="cmd">${l.slice(2)}</span>`;
  if (/^&gt; /.test(l)) return `<span class="prompt">&gt;</span> <span class="cmd">${l.slice(5)}</span>`;
  l = l.replace(/\*\*(.+?)\*\*/g, '<b>$1</b>').replace(/`([^`]+)`/g, '<code>$1</code>');
  if (/^#{1,3} /.test(l)) return `<span class="h">${l.replace(/^#+ /, '')}</span>`;
  if (/CREATED|√|"valid": true|READY|frutifero\)|sim$/.test(l)) return `<span class="ok">${l}</span>`;
  if (/BLOCKED|erro|Erros|INVALID|MISSING|×|infrutifero\)|ESTOURADO/.test(l)) return `<span class="warn">${l}</span>`;
  return l;
}
// cenas: [titulo, legenda, texto, comandoDigitado?, maxLinhas?]
const MARCA = 'data:image/png;base64,' + readFileSync('/home/user/adrianoboller/wx-claude-code/marca-wx-claude-code.png').toString('base64');
const ROTEIROS = {};
ROTEIROS.uso = [
  ['card', 'WX Claude Code', 'Conversão governada de projetos WINDEV, WEBDEV e WINDEV Mobile\nQuestionário: bloco 0 da empresa e letras A–L · Gates G0–G7 · Equipe WLanguage sobre o Help da PC SOFT · PMO com Scrum, Kanban e PDCA\n\nTudo que aparece a seguir é saída real de sessões do Claude Code e dos scripts do plugin.'],
  ['instalar.sh --conferir', '1 · Instalação: pré-requisitos, corpus, conferência do pacote e licença — em modo conferir nada é instalado', cap('45-instalacao')],
  ['claude plugin validate', '2 · O manifesto aceito pelo Claude Code', cap('validate')],
  ['/wx-claude-code:questionario · bloco 0', '3 · Antes da letra A: quem pede, diretores, endereço, logotipos, prazo, orçamento, riscos e GitHub, um item por vez', cap('questionario-0')],
  ['/wx-claude-code:questionario · 0.15', '4 · Senha colada na conversa não é gravada nem repetida: só o nome da credencial entra no entrega.json', cap('senha').split('\n').slice(0,12).join('\n'), 'Boller Sistemas Ltda. Converter o ESTOQUE para Rust + React. Já adianto o GitHub: usuário adrianoboller, senha ●●●●●●●, repositório https://github.com/adrianoboller/estoque-rs'],
  ['/wx-claude-code:questionario', '5 · As letras A a J: uma por vez, e a resposta decide a próxima', cap('questionario')],
  ['/wx-claude-code:questionario · letra H', '6 · Para qual linguagem converter: sinais, três opções, a recomendada primeiro', cap('questionario-h')],
  ['/wx-claude-code:questionario · letra H · processo', '7 · Como seria a conversão: o que cada peça do WX vira na linguagem escolhida, e depois a estratégia', cap('processo').split('\n').slice(17).join('\n')],
  ['exemplos/estoque-wx', '8 · Projeto de exemplo real: G0 sem erros, texto com localizador, golden master 9/10', cap('exemplo').split('\n').slice(14).join('\n')],
  ['DESIGN.md · letra F', '9 · Qualidade de ERP: treze subperguntas viram a tabela de botões, posição, ícone, cor e fundo', cap('design-erp').split('\n').slice(12, 48).join('\n')],
  ['query_wlanguage_help.py', '10 · O corpus WLanguage 12k, verificado por hash e consultado por tema', cap('help').split('\n').slice(0, 24).join('\n') + '\n…'],
  ['subagentes wl-*-specialist', '11 · Cada símbolo vai ao especialista WLanguage do tema certo do Help', cap('equipe')],
  ['/wx-claude-code:pmo', '12 · PMO: sprint Scrum, ciclos PDCA e a base de conhecimento', cap('pmo2').split('\n').slice(0, 33).join('\n')],
  ['pmo.py kanban', '13 · Kanban gerado da matriz, com limite de WIP', cap('pmo2').split('\n').slice(33, 65).join('\n')],
  ['/wx-claude-code:pmo status', '14 · O agente do PMO lê o painel e aponta o que trava', cap('pmo-sessao')],
  ['/wx-claude-code:laudo-tokens', '15 · Laudo de uso de tokens: somente leitura, MEDIDO ou INDISPONÍVEL', cap('laudo'), '/wx-claude-code:laudo-tokens fase-1'],
  ['/wx-claude-code:questionario · F0', '16 · A tela principal do legado como modelo: aberta antes de registrar, o que preservar e o que mudar', cap('tela-modelo')],
  ['sessão nova · respostas_questionario.md', '17 · Uma sessão nova acha o aprovador e o prazo nas respostas gravadas, sem perguntar', cap('respostas')],
  ['sessão nova · INDEX_FILES.md e kickoff', '18 · A primeira sessão lê o mapa e o kickoff, sabe o escopo da v1 e recusa código sem G0', cap('kickoff')],
  ['/wx-claude-code:questionario · K2', '19 · Ambiente: PostgreSQL, papéis por nível, e a senha do root que não é gravada nem repetida', cap('k2')],
  ['/wx-claude-code:questionario · K7', '20 · n8n integrado ao projeto: sim ou não, e cada item da integração um por mensagem', cap('k7')],
  ['licenca.py', '21 · Serial de ativação: sem ele o PMO recusa; instalado, a mesma sessão roda', cap('licenca')],
  ['/wx-claude-code:pmo exportar', '22 · O projeto resultante salvo, organizado, na pasta do usuário, sem segredo e com hashes', cap('exportar')],
  ['zelador.py · SessionStart', '23 · O zelador limpa temporários uma vez por dia e deixa o registro medido', cap('zelador')],
  ['sessão real · Bloco-SP', '24 · Toda resposta abre com a identificação BlocoNNNN-SPNNNNN-Título · data, e cada sprint fechada vira .md e .zip', cap('identificacao')],
  ['projeto com L6 = sim · sessão real', '25 · Esqueleto de ERP gerado pelo questionário: módulo → skill no CLAUDE.md, ADR lida, skill erp-inventory carregada e citada', cap('esqueleto-erp')],
  ['tests/cenarios.py', '26 · Bateria pesada: doze situações que um cliente real traz — sem licença, PDF que é foto, legado que nunca foi WX, resposta que se contradiz', cap('46-cenarios')],
  ['estoque-codigo.md · página 1', '27 · O ponto de partida: a procedure WLanguage, lida do PDF do legado com a página preservada', cap('47-wlanguage')],
  ['src/regras/desconto.rs', '28 · O ponto de chegada: Rust gerado por uma sessão real, com a página de origem citada dentro do próprio código', cap('48-rust')],
  ['cargo test · sessão real', '29 · A prova e a diferença: seis testes passando, e o que mudou de semântica na tradução — dito, não escondido', cap('49-prova-e-semantica')],
  ['card', 'Built to convert. Engineered to prove.', 'claude plugin marketplace add adrianoboller/adrianoboller\nclaude plugin install wx-claude-code@wx-claude-code\n\nManual completo em MANUAL.md'],
];

// Segundo roteiro: um legado que nao tem NADA de WINDEV. O plugin converte
// WLanguage e isso nao muda; o legado, porem, e E/OU -- pode chegar em PHP, C,
// C++, Clarion ou COBOL. Aqui o de origem e PHP procedural de 2009, e o video
// mostra o caminho inteiro: instalar, liberar a licenca, usar, chegar em Rust.
ROTEIROS.php = [
  ['card', 'De PHP para Rust', 'Um sistema PHP procedural de 2009 — sem nada de WINDEV — atravessando o WX Claude Code inteiro.\nInstalação · liberação da licença · questionário · portão G0 · conversão · prova\n\nTudo a seguir é saída real de sessões do Claude Code e dos scripts do plugin.'],
  ['instalar.sh --conferir', '1 · Instalação: pré-requisitos, corpus, conferência do pacote e licença — em modo conferir nada é instalado', cap('45-instalacao')],
  ['licenca.py verificar', '2 · Sem serial o plugin não roda: o verificador diz «ausente» e o hook nega o próprio script do plugin', cap('50-licenca-sem-serial')],
  ['licenca.py gerar · instalar · verificar', '3 · A liberação: quem vende assina com a chave privada, o cliente instala o serial, e o mesmo comando de antes passa', cap('51-licenca-liberada')],
  ['inputs/legado-php/lib/regras.php', '4 · O legado de origem: PHP procedural, mysqli, HTML no meio do código — a regra do financeiro mora aqui', cap('52-legado-php')],
  ['questionário + portão G0', '5 · O mesmo questionário de 60 perguntas, e o G0 aceitando um projeto sem um único PDF de WINDEV: o código-fonte é a evidência', cap('53-questionario-e-g0-php')],
  ['php capturar-golden.php', '6 · O golden master não foi digitado: ele é capturado rodando as regras do próprio legado, com os dados de amostra', cap('56-golden-do-legado')],
  ['sessão nova · sem contexto', '7 · Uma sessão nova responde sobre o legado com localizador: acha o aprovador, a baixa sem transação e a view que nenhum PHP usa', cap('57-sessao-nova')],
  ['src/regras/encargos.rs', '8 · O Rust que uma sessão real gerou, citando arquivo e linha do PHP dentro do próprio código', cap('54-rust-do-php')],
  ['cargo test · sessão real', '9 · A prova pelo golden master capturado do legado — e o que a sessão se recusou a converter sozinha', cap('55-prova-php-rust')],
  ['exportar_projeto.py · registro.py', '10 · A entrega: sete pastas numeradas, SHA-256 de cada arquivo, nada sensível junto — e o registro de tudo que o plugin fez', cap('58-entrega-e-registro')],
  ['tests/cenarios.py', '11 · A bateria pesada com o cenário deste projeto: treze situações, e a de número 13 é este legado PHP inteiro', cap('59-bateria-com-php')],
  ['card', 'O legado é E/OU. O destino é livre.', 'WLanguage (WINDEV, WEBDEV, WINDEV Mobile) é o caso principal e nunca sai do plugin.\nPHP, C, C++, Clarion, COBOL entram junto ou sozinhos.\n\nclaude plugin install wx-claude-code@wx-claude-code'],
];

const scenes = ROTEIROS[roteiroNome];
if (!scenes) { console.error(`roteiro desconhecido: ${roteiroNome} (existem: ${Object.keys(ROTEIROS).join(', ')})`); process.exit(2); }
const SAIDA = roteiroNome === 'uso' ? 'wx-claude-code-video-de-uso' : `wx-claude-code-video-${roteiroNome}`;
const html = `<!doctype html><meta charset="utf-8"><style>
html,body{margin:0;height:100%;background:#0b0d17;font-family:"DejaVu Sans Mono",Menlo,monospace;overflow:hidden}
.win{position:absolute;inset:26px 40px 44px 40px;border-radius:12px;overflow:hidden;background:#010418;border:1px solid #232742;box-shadow:0 20px 60px #0009;display:flex;flex-direction:column}
.bar{height:38px;flex:none;background:#141830;display:flex;align-items:center;padding:0 14px;gap:8px;color:#9aa0b8;font-size:13px}
.dot{width:12px;height:12px;border-radius:50%}.t{margin-left:12px}.brand{margin-left:auto;color:#E2261C;font-weight:700}
pre{margin:0;padding:16px 22px;color:#e6e8f2;font-size:14px;line-height:1.45;white-space:pre-wrap;word-break:break-word;flex:1;overflow:hidden}
.prompt{color:#2FBF71;font-weight:700}.cmd{color:#fff;font-weight:600}.h{color:#F7B733;font-weight:700}.ok{color:#2FBF71}.warn{color:#F5A15A}b{color:#fff}code{color:#8fd3ff}
.cap{position:absolute;left:40px;right:40px;bottom:10px;color:#c7cbe0;font-size:14px;text-align:center}
.card{position:absolute;inset:0;background:#010418;display:flex;flex-direction:column;align-items:center;justify-content:center;text-align:center;color:#fff;padding:60px}
.card img{width:230px;margin-bottom:10px}.card h1{font-size:44px;margin:0 0 18px;color:#E2261C;letter-spacing:1px}.card p{font-size:18px;line-height:1.6;color:#c7cbe0;white-space:pre-wrap;margin:0}
.cursor{display:inline-block;width:9px;height:16px;background:#2FBF71;vertical-align:-2px}
</style><div id="root"></div>`;

const browser = await chromium.launch();
const ctx = await browser.newContext({ viewport: { width: 1280, height: 720 }, recordVideo: { dir: outDir, size: { width: 1280, height: 720 } } });
const page = await ctx.newPage();
await page.setContent(html);
const sleep = (ms) => page.waitForTimeout(ms);
for (const [title, caption, text, typed] of scenes) {
  if (title === 'card') {
    await page.evaluate(([h, p, m]) => { document.getElementById('root').innerHTML = `<div class="card"><img src="${m}"><h1>${h}</h1><p>${p}</p></div>`; }, [esc(caption), esc(text), MARCA]);
    await sleep(4500); continue;
  }
  await page.evaluate(([t, c]) => { document.getElementById('root').innerHTML = `<div class="win"><div class="bar"><span class="dot" style="background:#ff5f57"></span><span class="dot" style="background:#febc2e"></span><span class="dot" style="background:#28c840"></span><span class="t">${t}</span><span class="brand">WX CLAUDE CODE</span></div><pre id="pre"></pre></div><div class="cap">${c}</div>`; }, [esc(title), esc(caption)]);
  const lines = text.split('\n');
  if (typed) {
    let s = '';
    for (const ch of typed) { s += ch; await page.evaluate((h) => { document.getElementById('pre').innerHTML = h; }, `<span class="prompt">&gt;</span> <span class="cmd">${esc(s)}</span><span class="cursor"></span>`); await sleep(28); }
    await sleep(700);
    lines.unshift(`> ${typed}`, '');
  }
  let acc = [];
  for (let i = 0; i < lines.length; i++) {
    acc.push(fmt(lines[i]));
    const isCmd = /^[$>] /.test(lines[i]);
    await page.evaluate((h) => { const p = document.getElementById('pre'); p.innerHTML = h; p.scrollTop = p.scrollHeight; }, acc.join('\n'));
    // rolagem: mantem as ultimas ~30 linhas visiveis
    if (acc.length > 30) { acc = acc.slice(-30); await page.evaluate((h) => { document.getElementById('pre').innerHTML = h; }, acc.join('\n')); }
    await sleep(isCmd ? 900 : Math.min(160, 40 + lines[i].length));
  }
  await sleep(3200);
}
await ctx.close(); await browser.close();
const f = readdirSync(outDir).find((n) => n.endsWith('.webm'));
renameSync(`${outDir}/${f}`, `${outDir}/${SAIDA}.webm`);
console.log('ok');

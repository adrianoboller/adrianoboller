// Grava um video de uso a partir das capturas reais (caps/*.txt): terminal
// animado com comando digitado e saida revelada linha a linha. Playwright grava
// em WebM (VP8), que e o que o ffmpeg do Playwright sabe codificar.
import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { readFileSync, readdirSync, renameSync, rmSync } from 'node:fs';

const [, , outDir, capsDir, roteiroNome = 'uso'] = process.argv;
// Captura ausente nao derruba o carregamento: os tres roteiros sao avaliados
// juntos, e gravar a bateria com uma pasta que so tem as capturas DELA quebrava
// no cap('45-instalacao') do roteiro de uso. Ausente vira sentinela, e o
// roteiro ESCOLHIDO e conferido depois -- ai sim falta e erro, com o nome.
const FALTA = '\u0000FALTA:';
const cap = (n) => {
  try { return readFileSync(`${capsDir}/${n}.txt`, 'utf8').replace(/\/tmp\/claude-0\/[^ ]*?\/scratchpad\/primeiro\/projeto/g, '.').replace(/\/tmp\/claude-0\/[^ ]*?\/scratchpad\/estoque\/projeto/g, '.').replace(/\/tmp\/claude-0\/[^ ]*?\/scratchpad\/passos\/projeto/g, '.').replace(/\/tmp\/claude-0\/[^ ]*?\/scratchpad\/passos\/entrega/g, '~/entregas/estoque').replace(/\/tmp\/claude-0\/[^ ]*?\/scratchpad\/passos\/plugins/g, '~/plugins').replace(/\/home\/user\/adrianoboller\/wx-claude-code\/entregas\/cliente/g, '~/Downloads').replace(/\/home\/user\/adrianoboller\/wx-claude-code/g, '~/plugins/wx-claude-code').replace(/\/tmp\/claude-0\/[^ ]*?\/scratchpad\/estoque\/entrega/g, '~/entregas/estoque').replace(/\/tmp\/claude-0\/[^ ]*?\/primeiro\/cliente-casa/g, '~/.wx-claude-code').replace(/\/tmp\/claude-0\/[^ ]*?\/scratchpad\/(proj|pmo2|demo|ex2?)/g, '.').replace(/\/root\//g, '~/'); }
  catch { return FALTA + n; }
};
const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
function fmt(line) {
  let l = esc(line);
  if (/^\$ /.test(l)) return `<span class="prompt">$</span> <span class="cmd">${l.slice(2)}</span>`;
  if (/^&gt; /.test(l)) return `<span class="prompt">&gt;</span> <span class="cmd">${l.slice(5)}</span>`;
  l = l.replace(/\*\*(.+?)\*\*/g, '<b>$1</b>').replace(/`([^`]+)`/g, '<code>$1</code>');
  if (/^#{1,3} /.test(l)) return `<span class="h">${l.replace(/^#+ /, '')}</span>`;
  // o verde tambem cobre a bateria: "... ok", "OK", "N/N cenarios", "passed", "Validation passed"
  if (/CREATED|√|"valid": true|READY|frutifero\)|sim$|\.\.\. ok$|^OK$|^\s*ok |\d+\/\d+ (cenários|passos)|test result: ok|passed$|Validation passed|tudo em \d/.test(l)) return `<span class="ok">${l}</span>`;
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

// Terceiro roteiro: a BATERIA inteira, rodada de verdade na hora da gravacao.
// Cada cena e a saida real de um comando (caps/*.txt gravados pelo shell
// imediatamente antes); nenhuma linha e digitada a mao. O que se mostra e o
// que qualquer um reproduz rodando os mesmos comandos.
ROTEIROS.bateria = [
  ['card', 'A bateria de testes', 'WX Claude Code 3.42.0\nTudo que aparece a seguir é a saída real dos comandos, gravada no momento da gravação.\nSete provas, nenhuma montagem.'],
  ['tests/testes.py -v', '1 · A bateria unitária: cada peça isolada — validação, hooks, licença, PMO, grafo, emissor de serial, aviso de instalação', cap('testes-v')],
  ['tests/cenarios.py', '2 · A bateria pesada: os caminhos que um cliente real traz — sem licença, PDF que é foto, legado PHP e C++, interface do destino', cap('cenarios')],
  ['tests/fluxo.py', '3 · O fluxo inteiro num projeto novo: questionário → contexto → G0 → artefato → PDF → PMO → RAG → entrega → registro', cap('fluxo')],
  ['validate_plugin_bundle.py --strict', '4 · O validador estrito: manifesto, arquivos obrigatórios, e a bateria rodando por dentro — e o aviso da chave de demonstração, fora de warnings', cap('validador')],
  ['claude plugin validate', '5 · O manifesto pelos olhos do próprio Claude Code', cap('plugin-validate')],
  ['cargo test · wx-modelos', '6 · O binário Rust, std pura: os 18 testes do medidor de modelo local', cap('cargo')],
  ['atualizar-paginas.py --conferir', '7 · Nenhuma página envelheceu calada: todos os geradores conferidos contra a versão', cap('paginas')],
  ['card', 'Built to convert. Engineered to prove.', '105 testes · 19 cenários · 13 passos · validador estrito · claude plugin validate · 18 testes Rust · páginas em dia\n\nReproduza: python3 tests/testes.py'],
];

// Quarto roteiro: o PRIMEIRO PROJETO, do zelador a entrega. Um mini CRUD PHP +
// MySQL (uma tabela) virando Rust + MySQL com tela React, passo a passo, com o
// golden master capturado do proprio legado e o grafo fechando em zero. As
// capturas sao as saidas reais da sessao em que o projeto foi feito; o que
// deu errado no caminho (a tela com os botoes vazando) aparece, porque foi o
// que aconteceu -- e a prova refeita depois do conserto tambem.
ROTEIROS.primeiro = [
  ['card', 'O primeiro projeto', 'WX Claude Code 3.43.0\nInstalação · liberação da licença · um mini CRUD PHP + MySQL (uma tabela) → Rust + MySQL + React\n\nTudo a seguir é saída real da sessão em que o projeto foi feito.'],
  ['zelador.py --forcar', '0 · Antes de tudo, o zelador libera espaço: caches, alvos de compilação e execuções velhas', cap('00-zelador')],
  ['unzip … -d ~/plugins', '1 · O pacote do cliente descompactado: um plugin, instalado uma vez, vale para todos os projetos', cap('01-descompactar')],
  ['emitir.py chaves', '2 · No vendedor: o par de chaves. A privada nunca entra no repositório nem no pacote', cap('02-chaves')],
  ['receber.py · emitir.py serial', '3 · O receptor de avisos no ar, e o serial assinado para «Loja do Bairro Ltda», com a URL do aviso dentro da assinatura', cap('03-serial')],
  ['instalar.sh --serial …', '4 · No cliente: a licença na tela, o aceite gravado com o hash dos termos, o serial conferido contra a chave pública, o corpus, o aviso enviado', cap('04-instalar')],
  ['instalar.sh · claude plugin', '5 · O plugin instalado no Claude Code a partir do marketplace local — e listado', cap('04b-instalar-no-claude')],
  ['o que chegou ao vendedor', '6 · O e-mail que o vendedor recebeu na instalação: serial, empresa, máquina, versão. Segunda máquina viraria «POSSÍVEL RECOMPARTILHAMENTO»', cap('05-aviso')],
  ['/wx-claude-code:questionario · aplicar', '7 · As 60 respostas do questionário aplicadas ao projeto: contexto, mapa, manifesto, o legado PHP inventariado', cap('06-aplicar')],
  ['/wx-claude-code:progresso', '8 · O progresso sabe o que ninguém confirmou ainda, e por onde retomar', cap('07-progresso')],
  ['/wx-claude-code:preflight (G0)', '9 · O portão G0: CONDITIONAL, com os motivos — sem PDF nenhum, porque o código PHP é a evidência', cap('08-g0')],
  ['/wx-claude-code:dependencias', '10 · As dependências externas do legado, medidas do código: mysqli e nada mais', cap('09-dependencias')],
  ['/wx-claude-code:interface', '11 · A interface do executável de destino: o que esta máquina compila, medido com o rustc de verdade', cap('10-interface')],
  ['php capturar-golden.php', '12 · O golden master capturado rodando o PRÓPRIO legado no MySQL — inclusive o auto-increment que salta no e-mail repetido', cap('11-golden-capturar')],
  ['src/regras.rs · repo.rs · main.rs', '13 · O Rust: regras puras citando clientes.php#linha, o repositório com mysql, o binário que responde ao golden e serve a API', cap('12-rust')],
  ['/wx-claude-code:golden comparar', '14 · A prova: 5/5 casos do golden master reproduzidos pelo binário Rust contra o mesmo MySQL', cap('13-golden-comparar')],
  ['/wx-claude-code:converter inventario', '15 · A matriz de rastreabilidade: cada regra, consulta, tabela e tela com origem, destino e teste', cap('14-matriz-e-testes')],
  ['/wx-claude-code:evidencia do-golden', '16 · A evidência nasce do golden master: o que ela prova, e o que não prova, escrito', cap('15-evidencia-grafo')],
  ['/wx-claude-code:evidencia registrar', '17 · Uma evidência a mais, do cargo test — e o grafo conferido de novo', cap('16-grafo-de-novo')],
  ['npm create vite · react-ts', '18 · A tela: React 19 + Vite, TypeScript, sobre a API Rust', cap('17-react')],
  ['clientes-rs servir 8080', '19 · A API Rust no ar, com o MySQL do legado por baixo', cap('18-api')],
  ['node tela.mjs', '20 · A tela exercitada de verdade pelo Playwright: incluir, alterar, excluir, cada passo conferido na API', cap('19-tela')],
  ['/wx-claude-code:grafo conferir', '21 · O grafo fecha: 0 código sem requisito, 0 requisito sem prova', cap('20-grafo-fechado')],
  ['o conserto e a prova refeita', '22 · Olhando a tela: os botões vazavam da tabela. Consertado, a prova venceu; refeita, o grafo volta a zero — o histórico fica', cap('21-conserto-e-prova')],
  ['/wx-claude-code:procedencia slsa', '23 · A procedência de cada artefato, com o nível SLSA dito com honestidade', cap('22-procedencia')],
  ['/wx-claude-code:exportar', '24 · A entrega: pastas numeradas, SHA-256 de cada arquivo, nada sensível junto', cap('23-exportar')],
  ['card', 'Built to convert. Engineered to prove.', 'Uma tabela, um CRUD, três linguagens — e nenhuma regra sem prova.\n\nO exemplo inteiro está em exemplos/clientes-php-mysql/'],
];

// Quinto roteiro: os PDFs do WINDEV virando Rust + React. O exemplo ESTOQUE (WINDEV 2025,
// quatro PDFs, sem projeto nativo) atravessa o plugin ate a entrega: o G0 diz FORENSIC, o
// PDF vira Markdown com pagina, o golden master vem da amostra, o Rust cita a pagina do PDF,
// o PostgreSQL 16 e o Axum sao os da letra H, a tela React reproduz os quatro estados que o
// PDF de interfaces descreve, e o grafo fecha com UMA lacuna: o GAP plantado no exemplo.
ROTEIROS.windev = [
  ['card', 'Dos PDFs do WINDEV ao Rust + React', 'WX Claude Code 3.45.0\nO exemplo ESTOQUE: WINDEV 2025, quatro PDFs, sete tabelas, oito regras — e nenhum projeto nativo.\nDestino: Rust + Axum + PostgreSQL 16, tela em React 19.\n\nTudo a seguir é saída real da sessão.'],
  ['/wx-claude-code:questionario · aplicar', '1 · O questionário aplicado: WINDEV 2025, os quatro PDFs como evidência, H = Rust + Axum + PostgreSQL, I = React', cap('01-aplicar')],
  ['/wx-claude-code:preflight (G0)', '2 · O portão G0: CONDITIONAL, classe FORENSIC — sem projeto nativo nem baseline executável, e o relatório diz o limite disso', cap('02-g0')],
  ['/wx-claude-code:pdf', '3 · Cada PDF vira Markdown citável: uma seção por página, SHA-256 no cabeçalho, o WLanguage marcado como código', cap('03-pdf')],
  ['/wx-claude-code:dependencias', '4 · As dependências externas achadas no texto: o HFSQL Client/Server e o estoque.ini', cap('03b-dependencias')],
  ['/wx-claude-code:golden capturar', '5 · O golden master: 10 casos capturados do legado com os dados de amostra, do desconto à comissão', cap('04-golden-capturar')],
  ['src/regras.rs · cargo test', '6 · O Rust: cada regra cita estoque-codigo.pdf e a página; dinheiro em centavos inteiros, porque currency do WLanguage é ponto fixo', cap('05-rust')],
  ['/wx-claude-code:golden comparar', '7 · A prova: 10/10 — inclusive a comissão, que roda a query no PostgreSQL 16 sobre a amostra migrada', cap('06-golden-comparar')],
  ['estoque-rs servir · Axum', '8 · A API no ar, com o esquema HFSQL traduzido para PostgreSQL e o CHECK que o legado não tinha', cap('07-api')],
  ['web/src/App.tsx · React 19', '9 · WIN_Venda em React: os mesmos controles, na mesma ordem do PDF de interfaces; as regras ficam no servidor', cap('08-react')],
  ['node web/tela.mjs', '10 · A tela exercitada pelo Playwright: vazia, com itens (os números do screenshot do legado), erro de estoque, acima do limite — 16/16', cap('09-tela')],
  ['psql estoque', '11 · No banco: venda, itens, baixa e títulos numa transação só — e a diferença de arredondamento na última parcela', cap('09-banco')],
  ['/wx-claude-code:converter inventario', '12 · A matriz: 17 linhas, cada uma apontando para a página do PDF de origem — e o GAP plantado, aberto', cap('10-matriz')],
  ['/wx-claude-code:evidencia · grafo', '13 · As evidências dizem o que provam e o que não provam; o grafo fecha com uma lacuna: EstornaEstoque, que o PDF de código não tem', cap('11-evidencias-e-grafo')],
  ['/wx-claude-code:procedencia slsa', '14 · A procedência, com o nível SLSA dito com honestidade', cap('12-procedencia')],
  ['/wx-claude-code:exportar', '15 · A entrega: pastas numeradas, SHA-256 de cada arquivo', cap('13-exportar')],
  ['card', 'Built to convert. Engineered to prove.', 'Quatro PDFs, oito regras, uma tela — e o que o PDF não diz fica escrito como lacuna, não inventado.\n\nO exemplo inteiro está em exemplos/estoque-wx/'],
];

// Sexto roteiro: o PASSO A PASSO na versao atual, comando por comando, do instalador a
// entrega -- a lista do manual virando saida real. Cada captura foi rodada nesta versao
// no momento da gravacao; o que o C-GATE achou no caminho (matriz sem test_result_ref)
// foi corrigido antes, e a saida mostrada e a de depois do conserto.
ROTEIROS.passos = [
  ['card', 'Passo a passo', 'WX Claude Code 3.47.0\nDo instalador à entrega, um comando por passo, na versão atual.\nProjeto: o ESTOQUE (WINDEV 2025, quatro PDFs) → Rust + Axum + PostgreSQL + React.\n\nTudo a seguir é saída real, gravada nesta versão.'],
  ['instalar.sh --conferir', '1 · Instalar: o zip do cliente, e o instalador em modo conferir — mostra os cinco passos sem mudar nada', cap('01-instalar')],
  ['/wx-claude-code:licenca verificar', '2 · A licença: sem serial válido nada roda', cap('02-licenca')],
  ['/wx-claude-code:questionario · aplicar', '3 · O questionário aplicado: contexto, mapa, manifesto e esqueleto', cap('03-aplicar')],
  ['/wx-claude-code:progresso', '4 · O progresso sabe o que ninguém confirmou ainda', cap('04-progresso')],
  ['/wx-claude-code:dependencias', '5 · As dependências externas lidas do texto do legado', cap('05-dependencias')],
  ['/wx-claude-code:preflight (G0)', '6 · O primeiro portão: CONDITIONAL, classe FORENSIC, e o relatório diz o limite', cap('06-preflight')],
  ['/wx-claude-code:pdf', '7 · O PDF vira Markdown citável: página e SHA-256 no cabeçalho, WLanguage marcado como código', cap('07-pdf')],
  ['/wx-claude-code:golden capturar · interface', '8 · O golden master capturado da amostra, e a interface do destino medida no rustc', cap('08-golden-e-interface')],
  ['/wx-claude-code:converter inventario', '9 · A matriz: 17 linhas, cada uma com a página do PDF de origem — e o validador do plugin dizendo VALID', cap('09-inventario')],
  ['cargo test · golden comparar', '10 · A prova: 10/10 contra o golden, inclusive a query no PostgreSQL', cap('10-golden-comparar')],
  ['/wx-claude-code:testes-da-matriz', '11 · O teste que a matriz ainda pede nasce falhando: a tela bloqueada pelo GAP plantado, sem prova', cap('11-testes-da-matriz')],
  ['/wx-claude-code:evidencia listar', '12 · O livro de evidências: o que cada uma prova, e o que não prova', cap('12-evidencia')],
  ['/wx-claude-code:constraints c-gate', '13 · O C-GATE: restrições sem validador são INCONCLUSIVAS, e isso não é aprovação', cap('13-constraints')],
  ['/wx-claude-code:grafo conferir', '14 · O grafo: uma lacuna, e ela é a tela bloqueada pelo GAP plantado no exemplo', cap('14-grafo')],
  ['/wx-claude-code:pmo', '15 · O PMO: plano, orçamento, Kanban e o painel medido', cap('15-pmo')],
  ['/wx-claude-code:contrato', '16 · O contrato ativo: o que vale hoje, com hash', cap('16-contrato')],
  ['/wx-claude-code:procedencia · exportar', '17 · Procedência com o nível SLSA dito com honestidade, e a entrega com SHA-256', cap('17-procedencia-e-exportar')],
  ['/wx-claude-code:log · zelador', '18 · O registro de tudo que o plugin fez, e o zelador limpando o que sobrou', cap('18-log-e-zelador')],
  ['card', 'Built to convert. Engineered to prove.', 'Dezoito passos, um comando cada — e o que o portão achou no caminho foi consertado, não escondido.\n\nManual: MANUAL.md · Fluxograma: docs/dossie/fluxo-atual.pdf'],
];

const scenes = ROTEIROS[roteiroNome];
if (!scenes) { console.error(`roteiro desconhecido: ${roteiroNome} (existem: ${Object.keys(ROTEIROS).join(', ')})`); process.exit(2); }
const faltando = scenes.map((c) => c[2]).filter((t) => typeof t === 'string' && t.startsWith(FALTA)).map((t) => t.slice(FALTA.length));
if (faltando.length) { console.error(`capturas ausentes para o roteiro ${roteiroNome}: ${faltando.join(', ')}`); process.exit(2); }
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

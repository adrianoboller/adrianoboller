#!/usr/bin/env node
/* Mede os `innerHTML` da `ui/` que interpolam DADO sem escapar (pedido 771).
 *
 *     node phxsql/testes-web/medir-innerhtml.mjs            # o resumo
 *     node phxsql/testes-web/medir-innerhtml.mjs --lista    # cada suspeito
 *
 * Por que analisar e nao casar texto: a mesma lei do Profiler -- «redige
 * ANALISANDO, nunca recortando». Um `grep 'innerHTML'` conta 218 atribuicoes e
 * nao diz nada sobre quais sao perigosas; o que decide e o que entra em cada
 * `${...}` do modelo, e isso so se le numa arvore de sintaxe. O analisador e
 * o `acorn` que ja vem instalado com o eslint da maquina -- ferramenta de
 * MEDIDA, fora do produto, do mesmo jeito que o playwright da bateria.
 *
 * O que conta como seguro, folha por folha:
 *   - literal, modelo sem `${}`, numero (aritmetica, `.length`, `Math.*`,
 *     `Number(...)`, `.toFixed`, `.toLocaleString`, comparacao, `!`);
 *   - chamada a uma funcao ESCAPADORA -- reconhecida pelo CORPO (troca `<`
 *     por `&lt;`), e nao pelo nome, para nao depender de quem batizou;
 *   - chamada a uma funcao local, ou uma variavel local, cujo valor tambem
 *     passa por esta mesma analise (seguida ate 6 niveis);
 *   - `.map(fn).join(...)`, com o corpo do `fn` analisado.
 *
 * O resto e SUSPEITO e vai para a lista com arquivo e linha. Suspeito nao e
 * condenado: parametro de funcao, por exemplo, depende do chamador, e a
 * lista existe para um humano olhar. O numero que vai para o pedido e o dos
 * suspeitos que, lidos, eram dado de verdade. */
import { readFileSync, readdirSync } from 'node:fs';
import { join, dirname, resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as acorn from '/opt/node22/lib/node_modules/eslint/node_modules/acorn/dist/acorn.mjs';
import * as walk from '/opt/node22/lib/node_modules/ts-node/node_modules/acorn-walk/dist/walk.mjs';

const AQUI = dirname(fileURLToPath(import.meta.url));
const iUi = process.argv.indexOf('--ui');
const UI = iUi > 0 ? resolve(process.argv[iUi + 1]) : resolve(AQUI, '..', 'crates', 'phxsql-server', 'ui');
const LISTA = process.argv.includes('--lista');

/* As fontes: os `.js` inteiros e os `<script>` embutidos dos `.html`, com a
   linha de partida para o numero bater com o arquivo de verdade. */
function fontes() {
  const saida = [];
  const andar = d => {
    for (const e of readdirSync(d, { withFileTypes: true })) {
      const p = join(d, e.name);
      if (e.isDirectory()) { andar(p); continue; }
      if (e.name.endsWith('.js')) saida.push({ arq: p, texto: readFileSync(p, 'utf8'), base: 0 });
      else if (e.name.endsWith('.html')) {
        const html = readFileSync(p, 'utf8');
        const re = /<script(?![^>]*\bsrc=)[^>]*>([\s\S]*?)<\/script>/g;
        let m;
        while ((m = re.exec(html)) !== null) {
          const antes = html.slice(0, m.index + m[0].indexOf('>') + 1);
          saida.push({ arq: p, texto: m[1], base: antes.split('\n').length - 1 });
        }
      }
    }
  };
  andar(UI);
  return saida;
}

const SEGUROS_GLOBAIS = new Set(['Number', 'parseInt', 'parseFloat', 'Boolean', 'encodeURIComponent']);
const METODOS_NUMERICOS = new Set(['toFixed', 'toLocaleString', 'toPrecision', 'getTime', 'toISOString',
  'getFullYear', 'getMonth', 'getDate', 'getHours', 'getMinutes', 'getSeconds', 'indexOf', 'findIndex']);

/* Os nomes se resolvem por ESCOPO de funcao (o `let` de bloco cai no escopo da
   funcao: e medida, nao compilador). O que nao se acha em nenhuma funcao
   acima cai no escopo GLOBAL, que e um so para todos os arquivos -- a pagina
   e um documento so, e o `esc` do `index.html` e o mesmo que o `claude.js`
   chama. */
const globais = new Map();
const memo = new Map();
const ehEscopo = n => n.type === 'Program' || /Function/.test(n.type);
const escopoDe = n => { let p = n.__pai; while (p && !ehEscopo(p)) p = p.__pai; return p; };
const declarar = (escopo, nome, d) => {
  const m = escopo.type === 'Program' ? globais : (escopo.__decl ||= new Map());
  if (!m.has(nome)) m.set(nome, { valores: [], fns: [], param: false, opaco: false });
  const r = m.get(nome);
  if (d.valor) r.valores.push(d.valor);
  if (d.fn) r.fns.push(d.fn);
  if (d.param) r.param = true;
  if (d.opaco) r.opaco = true;
};
function resolver(id) {
  let s = escopoDe(id);
  while (s && s.type !== 'Program') {
    if (s.__decl && s.__decl.has(id.name)) return s.__decl.get(id.name);
    s = escopoDe(s);
  }
  return globais.get(id.name) || null;
}
const nomesDoPadrao = p => {
  if (!p) return [];
  if (p.type === 'Identifier') return [p];
  if (p.type === 'ObjectPattern') return p.properties.flatMap(q => nomesDoPadrao(q.value || q.argument));
  if (p.type === 'ArrayPattern') return p.elements.flatMap(nomesDoPadrao);
  if (p.type === 'AssignmentPattern') return nomesDoPadrao(p.left);
  if (p.type === 'RestElement') return nomesDoPadrao(p.argument);
  return [];
};

function lerArquivo(f) {
  f.ast = acorn.parse(f.texto, { ecmaVersion: 'latest', sourceType: 'script', locations: true,
    allowReturnOutsideFunction: true, allowHashBang: true });
  walk.fullAncestor(f.ast, (n, _st, anc) => { n.__f = f; n.__pai = anc[anc.length - 2] || null; });
}

function declararArquivo(f) {
  walk.full(f.ast, n => {
    if (n.type === 'FunctionDeclaration' && n.id) declarar(escopoDe(n), n.id.name, { fn: n });
    if (/Function/.test(n.type))
      for (const q of n.params.flatMap(nomesDoPadrao)) declarar(n, q.name, { param: true });
    if (n.type === 'VariableDeclarator') {
      const esc = escopoDe(n);
      if (n.id.type === 'Identifier') {
        if (n.init && /Function/.test(n.init.type)) declarar(esc, n.id.name, { fn: n.init });
        else if (n.init) declarar(esc, n.id.name, { valor: n.init });
        else declarar(esc, n.id.name, {});
      } else for (const q of nomesDoPadrao(n.id)) declarar(esc, q.name, { opaco: true });
    }
    if (n.type === 'CatchClause' && n.param)
      for (const q of nomesDoPadrao(n.param)) declarar(escopoDe(n), q.name, { opaco: true });
    if ((n.type === 'ForOfStatement' || n.type === 'ForInStatement') && n.left.type === 'VariableDeclaration')
      for (const q of n.left.declarations.flatMap(d => nomesDoPadrao(d.id))) declarar(escopoDe(n), q.name, { opaco: true });
  });
}

function atribuicoesDoArquivo(f) {
  walk.full(f.ast, n => {
    if (n.type === 'AssignmentExpression' && n.left.type === 'Identifier') {
      const d = resolver(n.left);
      if (d) d.valores.push(n.right);
    }
  });
}

/* Quem chama cada funcao local: o parametro suspeito se resolve olhando o
   ARGUMENTO que cada chamador passa. */
const chamadores = new Map();
function chamadasDoArquivo(f) {
  walk.full(f.ast, n => {
    if (n.type !== 'CallExpression' || n.callee.type !== 'Identifier') return;
    const d = resolver(n.callee);
    if (!d) return;
    for (const fn of d.fns) {
      if (!chamadores.has(fn)) chamadores.set(fn, []);
      chamadores.get(fn).push(n);
    }
  });
}
/* Onde o parametro mora: a funcao e a posicao dele. */
function paramDe(id) {
  let s = escopoDe(id);
  while (s && s.type !== 'Program') {
    if (s.__decl && s.__decl.has(id.name)) {
      const i = s.params.findIndex(q => q.type === 'Identifier' && q.name === id.name);
      return i < 0 ? null : { fn: s, i };
    }
    s = escopoDe(s);
  }
  return null;
}
const METODOS_DE_LACO = new Set(['map', 'forEach', 'filter', 'flatMap', 'some', 'every', 'find', 'findIndex']);

const escapadora = fn => /&lt;/.test(fn.__f.texto.slice(fn.start, fn.end));

function analisarArquivo({ arq, ast }) {
  // Os `return` de uma funcao (sem descer em funcoes internas).
  const retornos = fn => {
    if (fn.body.type !== 'BlockStatement') return [fn.body];
    const r = [];
    walk.recursive(fn.body, null, {
      Function() {},
      ReturnStatement(n) { if (n.argument) r.push(n.argument); },
    });
    return r;
  };

  /* Devolve a lista de folhas suspeitas da expressao. */
  const emCurso = new Set();
  const suspeitas = e => {
    if (!e) return [];
    if (memo.has(e)) return memo.get(e);
    if (emCurso.has(e)) return [];
    emCurso.add(e);
    const r = folhas(e);
    emCurso.delete(e);
    memo.set(e, r);
    return r;
  };
  const rec = x => suspeitas(x);
  const folhas = e => {
    switch (e.type) {
      case 'Literal': return [];
      case 'TemplateLiteral': return e.expressions.flatMap(rec);
      case 'TaggedTemplateExpression': return [{ no: e, porque: 'modelo marcado' }];
      case 'BinaryExpression':
        if (e.operator === '+') return [...rec(e.left), ...rec(e.right)];
        return [];
      case 'UnaryExpression': case 'UpdateExpression': return [];
      case 'ConditionalExpression': return [...rec(e.consequent), ...rec(e.alternate)];
      case 'LogicalExpression':
        return e.operator === '&&' ? rec(e.right) : [...rec(e.left), ...rec(e.right)];
      case 'SequenceExpression': return rec(e.expressions.at(-1));
      case 'AwaitExpression': return rec(e.argument);
      case 'ArrayExpression': return e.elements.flatMap(rec);
      case 'Identifier': {
        if (e.name === 'undefined' || e.name === 'NaN' || e.name === 'Infinity') return [];
        const d = resolver(e);
        if (!d) return [{ no: e, porque: 'nome de fora' }];
        if (d.param) {
          const p = paramDe(e);
          if (p) {
            const pai = p.fn.__pai;
            // O segundo parametro de um `.map((x, i) => ...)` e o INDICE.
            if (p.i === 1 && pai && pai.type === 'CallExpression' && pai.callee.type === 'MemberExpression'
                && METODOS_DE_LACO.has(pai.callee.property.name) && pai.arguments[0] === p.fn) return [];
            const cs = chamadores.get(p.fn);
            if (cs && cs.length) return cs.flatMap(c => c.arguments[p.i] ? rec(c.arguments[p.i]) : []);
          }
          return [{ no: e, porque: 'parametro' }];
        }
        if (d.opaco) return [{ no: e, porque: 'desestruturado/laco' }];
        if (d.fns.length && !d.valores.length) return [];
        return d.valores.flatMap(rec);
      }
      case 'MemberExpression': {
        const p = e.property.name || (e.property.type === 'Literal' ? String(e.property.value) : '');
        if (!e.computed && (p === 'length' || p === 'size')) return [];
        return [{ no: e, porque: 'campo' }];
      }
      case 'CallExpression': {
        const c = e.callee;
        if (c.type === 'Identifier') {
          if (SEGUROS_GLOBAIS.has(c.name)) return [];
          if (c.name === 'String') return rec(e.arguments[0]);
          const d = resolver(c);
          if (d && d.fns.length) {
            if (d.fns.every(escapadora)) return [];
            return d.fns.flatMap(fn => retornos(fn).flatMap(rec));
          }
          return [{ no: e, porque: `funcao de fora (${c.name})` }];
        }
        if (c.type === 'MemberExpression' && !c.computed && c.object.type === 'Identifier'
            && c.object.name === 'window') {
          // `window.marcado(...)` e a funcao GLOBAL `marcado`.
          const d = globais.get(c.property.name);
          if (d && d.fns.length) {
            if (d.fns.every(escapadora)) return [];
            return d.fns.flatMap(fn => retornos(fn).flatMap(rec));
          }
        }
        if (c.type === 'MemberExpression') {
          const m = c.property.name;
          if (c.object.type === 'Identifier' && c.object.name === 'Math') return [];
          if (/^esc/.test(m)) return [];
          if (METODOS_NUMERICOS.has(m)) return [];
          if (m === 'join') {
            // `.map(fn).join` -> o que o `fn` devolve; senao, o proprio arranjo.
            const o = c.object;
            if (o.type === 'CallExpression' && o.callee.type === 'MemberExpression'
                && ['map', 'flatMap'].includes(o.callee.property.name)) {
              const fn = o.arguments[0];
              if (fn && /Function/.test(fn.type)) return retornos(fn).flatMap(rec);
              if (fn && fn.type === 'Identifier') return rec({ ...e, type: 'CallExpression', callee: fn, arguments: [] });
            }
            return rec(o);
          }
          if (['trim', 'slice', 'substring', 'toUpperCase', 'toLowerCase', 'concat', 'padStart', 'padEnd',
            'repeat', 'filter', 'sort', 'reverse'].includes(m)) {
            return [...rec(c.object), ...e.arguments.flatMap(rec)];
          }
          if (m === 'replace' || m === 'replaceAll') return rec(c.object);
          if (m === 'map' || m === 'flatMap') {
            const fn = e.arguments[0];
            if (fn && /Function/.test(fn.type)) return retornos(fn).flatMap(rec);
          }
          return [{ no: e, porque: `metodo .${m}()` }];
        }
        return [{ no: e, porque: 'chamada' }];
      }
      case 'ArrowFunctionExpression': case 'FunctionExpression': return [];
      case 'NewExpression': return [{ no: e, porque: 'new' }];
      default: return [{ no: e, porque: e.type }];
    }
  };

  const achados = [];
  walk.full(ast, n => {
    let alvo = null;
    if (n.type === 'AssignmentExpression' && n.left.type === 'MemberExpression'
        && ['innerHTML', 'outerHTML'].includes(n.left.property.name)) alvo = n.right;
    if (n.type === 'CallExpression' && n.callee.type === 'MemberExpression'
        && n.callee.property.name === 'insertAdjacentHTML') alvo = n.arguments[1];
    if (!alvo) return;
    const vistas = new Set();
    const s = suspeitas(alvo).filter(x => !vistas.has(x.no) && vistas.add(x.no));
    achados.push({ arq, linha: n.loc.start.line + n.__f.base, suspeitas: s.map(x => ({ onde: x.no.__f === n.__f ? '' : relative(UI, x.no.__f.arq), linha: x.no.loc.start.line + x.no.__f.base, porque: x.porque, texto: x.no.__f.texto.slice(x.no.start, x.no.end).slice(0, 90).replace(/\s+/g, ' ') })) });
  });
  return achados;
}

const lidas = fontes();
lidas.forEach(lerArquivo);
lidas.forEach(declararArquivo);
lidas.forEach(atribuicoesDoArquivo);
lidas.forEach(chamadasDoArquivo);
const todos = lidas.flatMap(analisarArquivo);
const comSuspeita = todos.filter(a => a.suspeitas.length);
const unicas = new Set(comSuspeita.flatMap(a => a.suspeitas.map(x => `${x.onde}:${x.linha}:${x.texto}`)));
const folhas = unicas.size;
console.log(`sinks de HTML (innerHTML/outerHTML/insertAdjacentHTML): ${todos.length}`);
console.log(`  sem nenhuma folha suspeita: ${todos.length - comSuspeita.length}`);
console.log(`  com folha suspeita:         ${comSuspeita.length} (${folhas} folhas distintas)`);
if (LISTA) {
  for (const a of comSuspeita) {
    console.log(`\n${relative(resolve(AQUI, '..'), a.arq)}:${a.linha}`);
    for (const s of a.suspeitas) console.log(`    ${s.onde}:${s.linha}  {${s.porque}}  ${s.texto}`);
  }
}

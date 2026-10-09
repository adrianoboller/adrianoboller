// Os cabecalhos de seguranca da tela e o vigia de CSP, para os roteiros que servem a UI do
// disco (page.route, servidor de revisao) exercitarem a MESMA politica do agente.
//
// A politica sai do fonte (crates/phxclaw-agent/src/pwa.rs: CSP, PERMISSOES, CABECALHOS),
// nunca de uma copia aqui: copia digitada envelhece calada, e o roteiro passaria a provar a
// tela contra uma CSP que o servidor ja nao manda. Os roteiros que sobem o `phxclaw servir`
// de verdade (ui_fluxos, ide_web, pwa_ponte) recebem os cabecalhos do proprio agente e usam
// so o vigia.
import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const FONTE = join(RAIZ, 'crates/phxclaw-agent/src/pwa.rs');

// Os literais de string de um trecho de Rust, colados (concat! e a continuacao `\` de linha).
function literais(trecho) {
  return [...trecho.matchAll(/"((?:[^"\\]|\\[\s\S])*)"/g)]
    .map(m => m[1].replace(/\\\n\s*/g, '').replace(/\\"/g, '"'))
    .join('');
}

export function cabecalhosDaUi() {
  const rs = readFileSync(FONTE, 'utf8');
  const constante = nome => {
    const m = rs.match(new RegExp(`pub const ${nome}: &str = ([\\s\\S]*?);\\n`));
    if (!m) throw new Error(`${nome} nao achada em ${FONTE}`);
    return literais(m[1]);
  };
  const valores = { CSP: constante('CSP'), PERMISSOES: constante('PERMISSOES') };
  const bloco = rs.match(/pub const CABECALHOS: &\[\(&str, &str\)\] = &\[([\s\S]*?)\];/);
  if (!bloco) throw new Error(`CABECALHOS nao achada em ${FONTE}`);
  const saida = {};
  for (const m of bloco[1].matchAll(/\(\s*"([a-z-]+)",\s*(CSP|PERMISSOES|"[^"]*")\s*,?\s*\)/g)) {
    saida[m[1]] = m[2].startsWith('"') ? m[2].slice(1, -1) : valores[m[2]];
  }
  if (!saida['content-security-policy']) throw new Error('CABECALHOS sem a CSP');
  return saida;
}

// O vigia: o evento `securitypolicyviolation` (na pagina, antes de qualquer script dela) e a
// linha do console do Chromium («Refused to ... Content Security Policy»), que tambem pega o
// que acontece fora do documento. Uma violacao e defeito: a tela quebrou um pedaco calada.
export async function vigiarCsp(alvo) {
  const vistas = [];
  await alvo.addInitScript(() => {
    window.__violacoesCsp = [];
    document.addEventListener('securitypolicyviolation', e => {
      window.__violacoesCsp.push(`${e.effectiveDirective} ${e.blockedURI || '(em linha)'} @ ${e.sourceFile || ''}:${e.lineNumber || ''}`);
    });
  });
  const ouvir = page => page.on('console', m => {
    const t = m.text();
    if (/Content Security Policy|Permissions-Policy|Refused to/i.test(t)) vistas.push(t.slice(0, 300));
  });
  if (typeof alvo.pages === 'function') {
    alvo.pages().forEach(ouvir);
    alvo.on('page', ouvir);
  } else {
    ouvir(alvo);
  }
  return {
    vistas,
    async todas(paginas) {
      const daPagina = [];
      for (const p of paginas) {
        try { daPagina.push(...await p.evaluate(() => window.__violacoesCsp || [])); } catch { /* pagina fechada */ }
      }
      return [...new Set([...vistas, ...daPagina])];
    },
  };
}

// O unico aviso de console que a politica provoca de proposito: o Chromium ignora o COOP em
// origem sem TLS que nao seja loopback (aqui, a origem falsa http://phxclaw.local dos roteiros
// que servem do disco) e diz isso no console. No agente real em 127.0.0.1 ele nao aparece; na
// rede sem TLS aparece, e e o recado certo (o COOP so vale com HTTPS). Nao e violacao de CSP.
export const avisoDoCoopSemTls = texto => /Cross-Origin-Opener-Policy header has been ignored, because the URL's origin was untrustworthy/.test(texto);

// A rota da politica da tela (pwa.rs, ROTA_POLITICA) com o padrao do catalogo: quem serve a UI
// do disco responde por ela como o agente, senao cada pagina sai com um 404 no console.
export const ROTA_POLITICA = '/ui/politica';
export const POLITICA_PADRAO = JSON.stringify({ bloquear_inspecao: true });

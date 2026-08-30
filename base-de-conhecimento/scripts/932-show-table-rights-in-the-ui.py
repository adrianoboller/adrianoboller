# Show table rights in the UI
# 29/08 00:29

import pathlib
p = pathlib.Path("crates/phxsql-server/ui/index.html")
s = p.read_text()

alvo = '''     <h3 class="secao">Poder sobre cada base</h3>
     ${lista.map(x => `<div class="bloco-user">
        <b>${esc(x.login || "")}</b>
        ${x.supervisor ? `<span class="pino ok">supervisor — pode tudo em toda base</span>`
          : (() => {
              const bases = x.bases || {};
              const chaves = Object.keys(bases);
              if (!chaves.length) return `<span class="pino mal">sem base listada e sem "*" — nega tudo</span>`;
              return chaves.map(b => `<span class="pino">${esc(b)}: ${
                Object.entries(bases[b] || {}).filter(([, v]) => v).map(([k]) => esc(k)).join(", ") || "nada"
              }</span>`).join(" ");
            })()}
      </div>`).join("")}'''
novo = '''     <h3 class="secao">Poder sobre cada base</h3>
     ${lista.map(x => `<div class="bloco-user">
        <b>${esc(x.login || "")}</b>
        ${x.supervisor ? `<span class="pino ok">supervisor — pode tudo em toda base</span>`
          : (() => {
              const bases = x.bases || {};
              const chaves = Object.keys(bases);
              if (!chaves.length) return `<span class="pino mal">sem base listada e sem "*" — nega tudo</span>`;
              return chaves.map(b => `<span class="pino">${esc(b)}: ${
                Object.entries(bases[b] || {}).filter(([, v]) => v).map(([k]) => esc(k)).join(", ") || "nada"
              }</span>`).join(" ");
            })()}
        ${(() => {
            // A regra de tabela SUBSTITUI a da base naquela tabela. Mostrá-la
            // na mesma linha da base seria dizer que uma soma da outra.
            const porTab = x.tabelas || {};
            const bs = Object.keys(porTab);
            if (x.supervisor || !bs.length) return "";
            return `<div class="por-tabela">${bs.map(b =>
              Object.entries(porTab[b] || {}).map(([t, perm]) => {
                const pode = Object.entries(perm || {}).filter(([, v]) => v).map(([k]) => esc(k));
                return `<span class="pino ${pode.length ? "" : "mal"}">${esc(b)}.${esc(t)}: ${
                  pode.join(", ") || "nada"}</span>`;
              }).join(" ")).join(" ")}</div>`;
          })()}
      </div>`).join("")}'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

alvo = '''       <p><strong>Três regras decidem tudo.</strong> Nega por omissão: atividade
       que não aparece vale <code>false</code>. A base listada manda — o
       <code>"*"</code> não completa o que faltou, e base listada vazia nega
       tudo. Sem a base e sem <code>"*"</code>, nega tudo.</p>'''
novo = '''       <p><strong>Três regras decidem tudo.</strong> Nega por omissão: atividade
       que não aparece vale <code>false</code>. A base listada manda — o
       <code>"*"</code> não completa o que faltou, e base listada vazia nega
       tudo. Sem a base e sem <code>"*"</code>, nega tudo.</p>
       <p><strong>E a tabela ganha da base.</strong> Dentro do objeto da base,
       <code>"tabelas"</code> escreve a regra de cada tabela — e ela
       <em>substitui</em> a da base naquela tabela, não soma nem corta. É o que
       permite as duas coisas que a prática pede: tirar <code>folha</code> de
       quem lê o banco inteiro, e dar <code>clientes</code> a quem não lê o
       banco nenhum. A árvore e o catálogo só mostram o que dá para abrir.</p>'''
assert s.count(alvo) == 1
s = s.replace(alvo, novo, 1)

# CSS pequena
s = s.replace('''.marca-excluida{color:var(--log);font-weight:600}''',
'''.marca-excluida{color:var(--log);font-weight:600}
.por-tabela{margin-top:6px;display:flex;flex-wrap:wrap;gap:5px}''',1)
p.write_text(s)
print("ok")

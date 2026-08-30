# Show web sessions in the UI and check
# 28/08 16:38

p='crates/phxsql-server/ui/index.html'
s=open(p).read()
a='''       ${ficha(d.quantas, "conexões")}
       ${ficha(d.executando, "executando")}
       ${ficha(dur(d.mais_longa_ms), "a mais demorada", d.mais_longa_ms > 5000 ? "há mais de 5 s" : "")}
     </div>` +'''
b='''       ${ficha(d.quantas, "na porta de dados")}
       ${ficha(d.sessoes_web ?? 0, "sessões web")}
       ${ficha(d.executando, "executando")}
       ${ficha(dur(d.mais_longa_ms), "a mais demorada", d.mais_longa_ms > 5000 ? "há mais de 5 s" : "")}
     </div>` +'''
assert a in s; s=s.replace(a,b,1)
a='''    `<div class="aviso"><b>Encerrar fecha o soquete.</b>'''
b='''    ((d.web || []).length
      ? `<h3>Sessões do navegador</h3>` + tabela(
          [{t:"id"},{t:"usuário"},{t:"aberta há",cls:"num"},{t:"expira em",cls:"num"},{t:""}],
          d.web, x => `<tr>
            <td class="dado">${esc(x.id)}…</td>
            <td class="dado">${esc(x.usuario || "—")}</td>
            <td class="num">${dur(x.aberta_s * 1000)}</td>
            <td class="num">${dur(x.expira_em_s * 1000)}</td>
            <td><button class="botao secundario mini" data-killweb="${esc(x.id)}">Encerrar</button></td>
          </tr>`)
      : "") +
    `<div class="aviso"><b>Encerrar fecha o soquete.</b>'''
assert a in s; s=s.replace(a,b,1)
a='''  $$("#painel [data-kill]").forEach(b => b.onclick = async () => {'''
b='''  $$("#painel [data-killweb]").forEach(b => b.onclick = async () => {
    const id = b.dataset.killweb;
    if (!confirm(`Encerrar a sessão web ${id}…?\\n\\nO próximo clique de quem estava usando cai no login.`)) return;
    try {
      const r = await api("encerrar_sessao", { id });
      avisar(`sessão ${r.encerrada} encerrada — ${r.aviso}`);
      verSessoes();
    } catch (e) { avisar(String(e), true); }
  });
  $$("#painel [data-kill]").forEach(b => b.onclick = async () => {'''
assert a in s; s=s.replace(a,b,1)
open(p,'w').write(s)

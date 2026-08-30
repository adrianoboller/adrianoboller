# Add the dashboard entry and its styles
# 27/08 22:35

p='crates/phxsql-server/ui/index.html'
s=open(p).read()

# 1. O painel entra no topo da arvore e vira a tela de entrada.
s=s.replace('''  const bancos = await api("bancos");
  est.bancos = bancos;
  let html = `<div class="grupo">Bancos de dados</div>`;''',
'''  const bancos = await api("bancos");
  est.bancos = bancos;
  let html = `<div class="no painel" data-admin="painel"><span class="ic">◎</span>Painel</div>
    <div class="grupo">Bancos de dados</div>`;''')
s=s.replace('''  const primeira = a.querySelector("[data-tab]");
  if (primeira) primeira.click();
  else $("#painel").innerHTML = `<div class="centro">Nenhuma tabela neste servidor.<br>
    Crie uma com <code>phxsql demo dados/Z</code>.</div>`;''',
'''  // O painel e a primeira tela: quem entra ve o servidor inteiro antes de
  // escolher uma tabela.
  a.querySelector('[data-admin="painel"]').click();''')

# 2. CSS do painel e dos graficos
s=s.replace('''.dica{font-size:11.5px;color:var(--texto-3);opacity:.8;font-style:italic;margin-left:auto}''',
'''.dica{font-size:11.5px;color:var(--texto-3);opacity:.8;font-style:italic;margin-left:auto}

/* ------------------------------------------------------------------ painel */
.no.painel{font-weight:600;margin-bottom:4px}
.kpis{display:grid;grid-template-columns:repeat(auto-fit,minmax(132px,1fr));gap:12px;
      margin-bottom:22px}
.kpi{background:var(--painel-2);border:1px solid var(--linha);border-radius:9px;
     padding:13px 15px}
.kpi .v{font-size:26px;font-weight:700;letter-spacing:-.02em;line-height:1.1;
        font-variant-numeric:tabular-nums}
.kpi .r{font-size:10px;letter-spacing:.11em;text-transform:uppercase;
        color:var(--texto-3);margin-top:5px}
.kpi .u{font-size:11px;color:var(--texto-3);margin-top:2px}
.kpi.viva .v{color:var(--laranja)}
.kpi.mal .v{color:var(--log)}
.cartas{display:grid;grid-template-columns:repeat(auto-fit,minmax(330px,1fr));gap:16px}
.carta{background:var(--painel-2);border:1px solid var(--linha);border-radius:10px;
       padding:15px 17px 12px}
.carta h4{margin:0 0 2px;font-size:13px;font-weight:600;letter-spacing:-.01em}
.carta .leg{font-size:11px;color:var(--texto-3);margin:0 0 12px}
.carta svg{width:100%;height:auto;display:block;overflow:visible}
.carta.larga{grid-column:1/-1}
.vazioc{color:var(--texto-3);font-size:12.5px;font-style:italic;padding:14px 0}''')

open(p,'w').write(s)
print('estrutura ok')

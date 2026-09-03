from pathlib import Path
import json
PASSOS=[("Chaves","Par de chaves"),("Cliente","Dados do cliente"),("Maquina","Máquina"),("Emissao","Validade e emissão"),("Entrega","Entrega ao cliente"),("Instalacao","Instalação e verificação")]
def ico(kind,size=20):
    s={"key":'<path d="M14 4a6 6 0 0 0-5.7 7.9L3 17.2V21h3.8l1.2-1.2v-2.4h2.4l1.2-1.2v-1.5A6 6 0 1 0 14 4z"/><circle cx="15.5" cy="8.5" r="1.5"/>',
       "cpu":'<rect x="6" y="6" width="12" height="12" rx="2"/><path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4"/>',
       "send":'<path d="M22 2 11 13M22 2 15 22l-4-9-9-4z"/>',
       "check":'<circle cx="12" cy="12" r="9"/><path d="m8 12 3 3 5-6"/>',
       "copy":'<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h10"/>',
       "lock":'<rect x="4" y="11" width="16" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
       "warn":'<path d="M12 3 2 21h20L12 3z"/><path d="M12 10v5M12 18h.01"/>'}[kind]
    return f'<svg width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{s}</svg>'
EXO="font-family: 'Exo 2', sans-serif;"
MONO="font-family: 'JetBrains Mono', Menlo, Consolas, monospace;"
P="<span style='color: #2FBF71'>$</span>"
G="#2FBF71";Y="#F7B733";R="#E2261C";B="#4EA1FF";C="#9AA0B8"
def h2(t,cor="#FFFFFF"): return f'<span style="{EXO} font-weight: 700; font-size: 16px; color: {cor}">{t}</span>'
def shell(n, titulo, sub, corpo, rodape):
    itens=""
    for i,(f,nome) in enumerate(PASSOS):
        st="done" if i<n else ("cur" if i==n else "todo")
        col={"done":G,"cur":R,"todo":C}[st]
        bg="rgba(226,38,28,0.12)" if st=="cur" else "transparent"
        tc="#E6E8F2" if st!="todo" else C
        itens+=f'<div style="display: flex; align-items: center; gap: 12px; padding: 12px 14px; border-radius: 8px; background: {bg}; color: {col}"><span style="display: flex; width: 28px; height: 28px; border-radius: 50%; border: 1.5px solid {col}; align-items: center; justify-content: center; {EXO} font-weight: 700; font-size: 13px">{i+1}</span><span style="font-size: 14px; color: {tc}">{nome}</span></div>'
    return f'''<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
  <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@500;700&amp;display=swap">
  <style>
    body {{ margin: 0; font-family: system-ui, "Segoe UI", sans-serif; color: #E6E8F2; }}
    a {{ color: #4EA1FF; }} a:hover {{ color: #8FD3FF; }}
    code {{ font-family: "JetBrains Mono", Menlo, Consolas, monospace; font-size: 13px; color: #8FD3FF; }}
  </style>
</helmet>
<div style="width: 1200px; height: 760px; background: #010418; display: flex; flex-direction: row; overflow: hidden">
  <div style="width: 264px; flex-shrink: 0; box-sizing: border-box; background: #060A22; border-right: 1px solid #232742; padding: 24px 16px; display: flex; flex-direction: column; gap: 6px">
    <div style="display: flex; align-items: center; gap: 10px; padding: 0 6px 22px">
      <img src="marca.png" style="width: 40px; height: 40px" alt="WX Claude Code">
      <div style="display: flex; flex-direction: column"><span style="{EXO} font-weight: 700; font-size: 15px; color: {R}; letter-spacing: 0.5px">WX CLAUDE CODE</span><span style="font-size: 11px; color: {C}">Liberação de licença</span></div>
    </div>
    {itens}
    <div style="margin-top: auto; padding: 12px 6px; font-size: 11px; color: {C}; line-height: 1.5">Built to store. Engineered to scale.</div>
  </div>
  <div style="flex-grow: 1; min-width: 0; padding: 32px 40px; display: flex; flex-direction: column; gap: 20px">
    <div style="display: flex; flex-direction: column; gap: 6px">
      <span style="font-size: 12px; color: {C}; letter-spacing: 1px; text-transform: uppercase">{"Visão geral" if n<0 else f"Passo {n+1} de 6"}</span>
      <h1 style="margin: 0; {EXO} font-weight: 700; font-size: 28px; color: #FFFFFF">{titulo}</h1>
      <p style="margin: 0; font-size: 15px; color: {C}; line-height: 1.5; max-width: 760px; text-wrap: pretty">{sub}</p>
    </div>
    {corpo}
    <div style="margin-top: auto; display: flex; flex-direction: row; justify-content: space-between; align-items: center; border-top: 1px solid #232742; padding-top: 16px">
      {rodape}
    </div>
  </div>
</div>
</x-dc>
</body>
</html>
'''
def btn(txt,cor,icon=None,fill=False):
    bg=cor if fill else "transparent"; fg="#010418" if fill else cor
    return f'<button style="display: flex; align-items: center; gap: 8px; min-height: 44px; padding: 0 20px; border-radius: 8px; border: 1.5px solid {cor}; background: {bg}; color: {fg}; {EXO} font-weight: 700; font-size: 13px; letter-spacing: 0.8px; cursor: pointer">{ico(icon) if icon else ""}{txt}</button>'
def card(*inner):
    return '<div style="box-sizing: border-box; min-width: 0; background: #0B1030; border: 1px solid #232742; border-radius: 12px; padding: 20px 22px; display: flex; flex-direction: column; gap: 14px">'+"".join(inner)+'</div>'
def campo(label, valor, mono=False, hint=""):
    fam=MONO if mono else ""
    h=f'<span style="font-size: 12px; color: {C}">{hint}</span>' if hint else ""
    return f'<div style="display: flex; flex-direction: column; gap: 6px"><label style="font-size: 11px; color: {C}; letter-spacing: 1px; text-transform: uppercase">{label}</label><div style="min-height: 44px; box-sizing: border-box; display: flex; align-items: center; padding: 0 14px; border: 1px solid #2E3454; border-radius: 8px; background: #060A22; font-size: 14px; color: #E6E8F2; {fam}">{valor}</div>{h}</div>'
def aviso(txt,cor=Y,icon="warn"):
    return f'<div style="display: flex; gap: 12px; align-items: flex-start; padding: 14px 16px; border: 1px solid {cor}; border-radius: 10px; color: #E6E8F2; font-size: 13px; line-height: 1.5"><span style="color: {cor}; flex-shrink: 0">{ico(icon)}</span><span>{txt}</span></div>'
def term(lines):
    return f'<div style="background: #000000; border: 1px solid #232742; border-radius: 10px; padding: 14px 18px; {MONO} font-size: 12.5px; line-height: 1.6; color: #E6E8F2; white-space: pre-wrap; word-break: break-word">'+"\n".join(lines)+'</div>'
def grid(cols, *inner, gap=20):
    return f'<div style="display: grid; grid-template-columns: repeat({cols}, minmax(0, 1fr)); gap: {gap}px">'+"".join(inner)+'</div>'
def muted(t): return f'<span style="font-size: 13px; color: {C}; line-height: 1.5">{t}</span>'
def linha_ok(t): return f'<div style="display: flex; gap: 10px; align-items: center; color: {G}; font-size: 13px">{ico("check")} {t}</div>'
def resumo(cols=2):
    return f'<div style="display: grid; grid-template-columns: repeat({cols}, minmax(0, 1fr)); gap: 8px 24px; font-size: 14px">'+"".join(f'<div style="display: flex; justify-content: space-between; border-bottom: 1px solid #232742; padding: 8px 0"><span style="color: {C}">{k}</span><span>{v}</span></div>' for k,v in [("Cliente","Boller Sistemas Ltda"),("E-mail","adriano@exemplo.com.br"),("Máquina","3f9c2a71b804e5d6"),("Validade","03/09/2027"),("Emitido em","03/09/2026"),("Serial nº","B0A24C5C")])+'</div>'

# 1
corpo=grid(2,
  card(h2("Chave privada"),muted("Fica só com quem emite. É gerada com permissão 0600 fora do repositório e nunca sai desta máquina."),campo("Local","~/.wx-claude-code/chaves/chave-privada.json",mono=True),linha_ok("gerada há 3 s · RSA-2048")),
  card(h2("Chave pública"),muted("Vai dentro do plugin. Com ela o plugin confere o serial, mas não consegue emitir nem forjar um."),campo("Copiar para","licenca/chave-publica.json",mono=True),f'<div style="display: flex; gap: 10px">{btn("COPIAR PARA O PLUGIN",B,"copy")}</div>'))
corpo+=aviso("A chave pública que vem no repositório é de demonstração. Gere o seu par uma única vez, substitua o arquivo no plugin e só então distribua. Se a privada vazar, gere outro par e reemita os seriais em vigor.")
corpo+=term([f"{P} python3 skills/conversao-wx/scripts/licenca.py chaves gerar --saida ~/.wx-claude-code/chaves","CREATED ~/.wx-claude-code/chaves/chave-privada.json (0600; nunca entra no repositorio)","CREATED ~/.wx-claude-code/chaves/chave-publica.json (copie para licenca/chave-publica.json do plugin)"])
Path("Chaves.dc.html").write_text(shell(0,"Par de chaves","Feito uma vez por distribuidor. A privada assina os seriais; a pública, dentro do plugin, só confere.",corpo,btn("GERAR OUTRO PAR",C,"key")+btn("CONTINUAR",B)),encoding="utf-8")

# 2
corpo=card(grid(2,campo("Softhouse (razão social)","Boller Sistemas Ltda"),campo("Nome fantasia","Boller Sistemas"),campo("CNPJ (opcional)","00.000.000/0001-00"),campo("E-mail do responsável","adriano@exemplo.com.br"),campo("Contato","Adriano Boller · Diretor de tecnologia"),campo("Projeto","ESTOQUE (WINDEV 2025) → Rust + React"),gap=18))
corpo+=aviso("Só o nome do cliente e o e-mail entram no serial. Nada de senha, CNPJ ou dado do projeto vai para dentro dele.",B,"lock")
Path("Cliente.dc.html").write_text(shell(1,"Dados do cliente","Quem vai receber o serial. O nome e o e-mail ficam gravados dentro dele, assinados.",corpo,btn("VOLTAR",C)+btn("CONTINUAR",B)),encoding="utf-8")

# 3
radio_off='<span style="width: 20px; height: 20px; border-radius: 50%; border: 1.5px solid #9AA0B8; display: block; flex-shrink: 0"></span>'
radio_on=f'<span style="width: 20px; height: 20px; border-radius: 50%; border: 1.5px solid {R}; background: radial-gradient(circle, {R} 45%, transparent 50%); display: block; flex-shrink: 0"></span>'
corpo=grid(2,
  card(f'<div style="display: flex; align-items: center; gap: 12px">{radio_off}{h2("Livre")}</div>',muted("O serial vale em qualquer máquina do cliente. Mais simples de suportar; mais fácil de passar adiante.")),
  card(f'<div style="display: flex; align-items: center; gap: 12px">{radio_on}{h2("Presa à máquina")}</div>',muted("O serial só vale na máquina cuja impressão você informar. Em outra, o plugin responde <code>maquina-diferente</code>."),campo("Impressão da máquina do cliente","3f9c2a71b804e5d6",mono=True,hint="16 caracteres, sem segredo: hash do nome da máquina, do usuário e do machine-id")))
corpo+=aviso("Peça ao cliente para rodar o comando abaixo e mandar o resultado. Ele não expõe nada além do hash.",B,"cpu")
corpo+=term([f'{P} python3 "$CLAUDE_PLUGIN_ROOT/skills/conversao-wx/scripts/licenca.py" maquina',"3f9c2a71b804e5d6"])
Path("Maquina.dc.html").write_text(shell(2,"Máquina","Decida se o serial vale em qualquer máquina ou só numa.",corpo,btn("VOLTAR",C)+btn("CONTINUAR",B)),encoding="utf-8")

# 4
chips="".join(f'<span style="padding: 9px 14px; border-radius: 999px; border: 1.5px solid {R if t=="12 meses" else "#2E3454"}; color: {"#FFFFFF" if t=="12 meses" else C}; font-size: 13px">{t}</span>' for t in ["3 meses","6 meses","12 meses","24 meses","outra"])
serial_txt="WX2.eyJjbGllbnRlIjoiQm9sbGVyIFNpc3RlbWFzIEx0ZGEiLCJlbWFpbCI6ImFkcmlhbm9AZXhlbXBsby5jb20uYnIiLCJlbWl0aWRvX2VtIjoiMjAyNi0wOS0wMyIsImlkIjoiQjBBMjRDNUMiLCJtYXF1aW5hIjoiM2Y5YzJhNzFiODA0ZTVkNiIsInZhbGlkYWRlIjoiMjAyNy0wOS0wMyJ9.Qm9…[assinatura RSA-2048, 344 caracteres]"
corpo=grid(2,
  card(h2("Validade"),f'<div style="display: flex; gap: 10px; flex-wrap: wrap">{chips}</div>',campo("Válido até","2027-09-03",mono=True),muted("Vencido, o plugin para e pede serial novo. Não há período de tolerância.")),
  card(h2("Conferir antes de emitir"),resumo(1)))
corpo+=card(f'<div style="display: flex; justify-content: space-between; align-items: center"><span style="{EXO} font-weight: 700; font-size: 16px; color: {G}; display: flex; align-items: center; gap: 8px">{ico("check")} Serial emitido</span>{btn("COPIAR SERIAL",B,"copy")}</div>',f'<div style="{MONO} font-size: 12px; line-height: 1.6; color: #8FD3FF; word-break: break-all; background: #060A22; border: 1px solid #2E3454; border-radius: 8px; padding: 12px 14px">{serial_txt}</div>')
Path("Emissao.dc.html").write_text(shell(3,"Validade e emissão","Confira o resumo. Emitir assina o serial com a chave privada; depois disso um byte alterado o invalida.",corpo,btn("VOLTAR",C)+f'<div style="display: flex; gap: 12px">{btn("EMITIR SERIAL",G,"key")}{btn("CONTINUAR",B)}</div>'),encoding="utf-8")

# 5
email='''Olá Adriano,

Segue o serial do WX Claude Code para a Boller Sistemas Ltda, válido até 03/09/2027 e preso à máquina 3f9c2a71b804e5d6.

Para instalar, na máquina em que o Claude Code roda:

  python3 "$CLAUDE_PLUGIN_ROOT/skills/conversao-wx/scripts/licenca.py" instalar "WX2.eyJj…"
  python3 "$CLAUDE_PLUGIN_ROOT/skills/conversao-wx/scripts/licenca.py" verificar

A resposta esperada é «valida: Boller Sistemas Ltda ate 2027-09-03 (serial B0A24C5C)».
O serial fica em ~/.wx-claude-code/licenca. Se trocar de máquina, peça outro serial informando a nova impressão (licenca.py maquina).'''
corpo=f'<div style="display: grid; grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr); gap: 20px">'+card(f'<div style="display: flex; justify-content: space-between; align-items: center">{h2("E-mail pronto")}<div style="display: flex; gap: 10px">{btn("COPIAR",B,"copy")}{btn("SALVAR .TXT",C)}</div></div>',f'<div style="white-space: pre-wrap; font-size: 13px; line-height: 1.55; color: #E6E8F2; background: #060A22; border: 1px solid #2E3454; border-radius: 8px; padding: 14px 16px">{email}</div>')+'<div style="display: flex; flex-direction: column; gap: 20px">'+card(h2("O que vai, o que fica"),f'<div style="display: flex; flex-direction: column; gap: 10px; font-size: 13px; line-height: 1.5"><div style="display: flex; gap: 10px; color: {G}">{ico("check")}<span style="color: #E6E8F2">Vai: o serial, as duas linhas de instalação, a validade.</span></div><div style="display: flex; gap: 10px; color: {R}">{ico("lock")}<span style="color: #E6E8F2">Fica: a chave privada. Nunca sai desta máquina, nem por e-mail, nem no zip.</span></div></div>')+card(h2("Registro da emissão"),resumo(1))+'</div></div>'
Path("Entrega.dc.html").write_text(shell(4,"Entrega ao cliente","O serial não é segredo: quem o tiver só ativa o plugin para a Boller Sistemas, nesta máquina, até a validade.",corpo,btn("VOLTAR",C)+btn("MARCAR COMO ENTREGUE",G,"send")),encoding="utf-8")

# 6
estados="".join(f'<span style="padding: 6px 12px; border-radius: 999px; border: 1.5px solid {c}; color: {c}; font-size: 12px; {MONO}">{t}</span>' for t,c in [("ausente",C),("vencida",Y),("maquina-diferente",Y),("assinatura-invalida",R),("formato-invalido",R)])
corpo=grid(2,
  card(h2("Na máquina do cliente"),term([f'{P} python3 "$CLAUDE_PLUGIN_ROOT/skills/conversao-wx/scripts/licenca.py" instalar "WX2.eyJj…"',"instalada em ~/.wx-claude-code/licenca: valida, cliente Boller Sistemas Ltda, ate 2027-09-03","",f'{P} python3 "$CLAUDE_PLUGIN_ROOT/skills/conversao-wx/scripts/licenca.py" verificar',f"<span style='color: {G}'>valida: Boller Sistemas Ltda ate 2027-09-03 (serial B0A24C5C)</span>"])),
  card(h2("Estado da licença"),f'<div style="display: flex; align-items: center; gap: 14px; padding: 16px; border: 1.5px solid {G}; border-radius: 10px"><span style="color: {G}">{ico("check",36)}</span><div style="display: flex; flex-direction: column; gap: 4px"><span style="{EXO} font-weight: 700; font-size: 18px; color: #FFFFFF">Licença válida</span><span style="font-size: 13px; color: {C}">Boller Sistemas Ltda · até 03/09/2027 · serial B0A24C5C · esta máquina</span></div></div>',f'<span style="font-size: 12px; color: {C}; letter-spacing: 1px; text-transform: uppercase">Outros estados possíveis</span><div style="display: flex; flex-wrap: wrap; gap: 8px">{estados}</div>'))
corpo+=card(h2("Ao abrir o Claude Code"),f'<div style="font-size: 13px; line-height: 1.55; color: #E6E8F2"><span style="color: {C}">Contexto injetado pelo hook SessionStart:</span> <code>WX Claude Code licenciado para Boller Sistemas Ltda (serial B0A24C5C, valido ate 2027-09-03).</code></div>',muted("Sem licença válida o aviso é o oposto: os comandos <code>/wx-claude-code:*</code> param na primeira linha e o hook nega os scripts do plugin e a escrita em <code>.wx-migration/</code>. O resto do Claude Code segue normal."))
Path("Instalacao.dc.html").write_text(shell(5,"Instalação e verificação","O cliente instala com um comando e confere com outro. A partir daí toda sessão abre licenciada.",corpo,btn("VOLTAR",C)+btn("CONCLUIR",G,"check")),encoding="utf-8")

# Main
etapas=[("1","Par de chaves","uma vez por distribuidor","chaves gerar"),("2","Dados do cliente","nome e e-mail entram no serial","—"),("3","Máquina","livre ou presa à impressão","maquina"),("4","Validade e emissão","assinatura RSA-2048","gerar --cliente --validade"),("5","Entrega","serial e duas linhas de instalação","—"),("6","Instalação","o cliente instala e verifica","instalar · verificar")]
cards="".join(f'<div style="display: flex; flex-direction: column; gap: 8px; background: #0B1030; border: 1px solid #232742; border-radius: 12px; padding: 18px; min-height: 150px; box-sizing: border-box"><span style="display: flex; width: 32px; height: 32px; border-radius: 50%; border: 1.5px solid {R}; color: {R}; align-items: center; justify-content: center; {EXO} font-weight: 700; font-size: 14px">{n}</span><span style="{EXO} font-weight: 700; font-size: 15px; color: #FFFFFF">{t}</span><span style="font-size: 13px; color: {C}; line-height: 1.45">{d}</span><code style="margin-top: auto">{c}</code></div>' for n,t,d,c in etapas)
corpo=grid(3,cards,gap=16)+aviso("Quem emite tem a chave privada e faz os passos 1 a 5. O cliente faz só o 6. Nada disso impede alguém de apagar o hook: é dissuasão para o cliente honesto; a proteção real é servir o corpus e os agentes de um servidor com o serial conferido a cada chamada.")
Path("Main.dc.html").write_text(shell(-1,"Liberar uma licença, passo a passo","Seis telas: cinco de quem distribui, uma do cliente. Cada uma corresponde a um subcomando do licenca.py.",corpo,f'<span style="font-size: 13px; color: {C}">WX Claude Code 3.8.0 · licenca/LEIA-ME.md</span>'+btn("COMEÇAR",R,"key")),encoding="utf-8")

arts=[("Main.dc.html",0,0)]+[(f"{f}.dc.html",(i%3)*1290,900+(i//3)*880) for i,(f,_) in enumerate(PASSOS)]
json.dump({"artboards":[{"file":f,"x":x,"y":y,"w":1200,"h":760} for f,x,y in arts],"annotations":[{"id":"nota-fluxo","x":1290,"y":0,"w":420,"text":"Fluxo de liberação de licença do WX Claude Code 3.8.0.\nAcima: visão geral. Abaixo: os seis passos, na ordem.\nBotões seguem a convenção do plugin: contorno, verde conclui, azul segue, vermelho é a ação da marca."}],"launch":{"view":"canvas"}}, open("canvas.json","w",encoding="utf-8"), ensure_ascii=False, indent=2)
print("ok")

#!/usr/bin/env python3
"""Gera docs/dossie/dossie-wx-claude-code.html de numeros.json e do git log.
Numero nenhum e digitado aqui: rode numeros-do-plugin.py antes.
Uso: python3 docs/dossie/gerar-dossie.py  (a partir de wx-claude-code/)
"""
from __future__ import annotations
import html, json, re, subprocess
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]
n = json.loads((RAIZ / "docs/dossie/numeros.json").read_text(encoding="utf-8"))
log = subprocess.run(["git", "log", "--date=short", "--format=%ad|%s", "-80", "--", str(RAIZ)], capture_output=True, text=True, cwd=RAIZ).stdout.splitlines()
versoes = []
for l in log:
    d, s = l.split("|", 1)
    m = re.match(r"^(\d+\.\d+\.\d+): (.+)$", s)
    if m:
        versoes.append((m.group(1), d, m.group(2)))
gerar = (RAIZ / "docs/prints/gerar.md").read_text(encoding="utf-8")
provas = [(a, b.strip()) for a, b in re.findall(r"^\| (\d\d) \| (.+?) \|$", gerar, re.M)]
provas.sort()
E = html.escape

def secao(id_, titulo, corpo):
    return f'<section id="{id_}"><h2>{E(titulo)}</h2>{corpo}</section>'

kpis = [("agentes", n["agentes"]), ("papéis com PDCA", f"{n['papeis']} + {n['subagentes_pdca']}"), ("testes de regressão", n["testes"]), ("scripts Python", n["scripts"]), ("blocos do questionário", n["blocos_do_questionario"]), ("provas em sessão real", n["prints"]), ("cenas no vídeo", n["video_cenas"]), ("linhas de Python", f"{n['linhas_de_python']:,}".replace(",", "."))]
kpi_html = "".join(f'<div class="kpi"><b>{E(str(v))}</b><span>{E(k)}</span></div>' for k, v in kpis)
fluxo = ["Questionário<br><small>bloco 0, A–M</small>", "Pré-flight G0<br><small>evidências conferidas</small>",
         "Inventário G1–G3<br><small>matriz, decisões</small>", "Piloto G4<br><small>golden master</small>",
         "Ondas G5–G6<br><small>papéis A–J, PDCA</small>", "F-GATE e C-GATE<br><small>funciona? conforme?</small>",
         "Evidência e grafo<br><small>o que ficou sem prova</small>", "Cutover G7<br><small>entrega e auditoria</small>"]
fluxo_html = "".join(f'<div class="passo">{p}</div>' for p in fluxo)
provas_html = "".join(f'<tr><td class="num">{a}</td><td>{E(b)}</td></tr>' for a, b in provas)
versoes_html = "".join(f'<tr><td class="num">{E(v)}</td><td>{E(d)}</td><td>{E(t)}</td></tr>' for v, d, t in versoes)
regras = [
    ("Senha nunca em texto puro", "o questionário guarda só o nome da variável; o gerador recusa chave senha/token com valor e valores com formato de token; a sessão real com a senha colada não a repetiu."),
    ("Valor de questionário é vetor de injeção", "tudo que vira bash, SQL, YAML ou JSON passa por validação antes de gravar; identificador é identificador, porta é inteiro; teste com onze injeções."),
    ("Número visível sai de um gerador", "os deste dossiê vêm de numeros-do-plugin.py; a página para investidores já ficou quatro versões com 34 agentes e 12 testes por ter sido digitada."),
    ("Interface só se prova exercitando", "cada funcionalidade nova teve sessão real gravada; uma delas achou o .env.exemplo caindo no filtro de .env, outra o manifesto dizendo missing para dados que existiam."),
    ("O que não se mede fica INDISPONÍVEL", "painel, relatório e medidor de ambiente nunca mostram zero no lugar do desconhecido."),
    ("Guarda nova entra pedida, não imposta", "a licença trava só os scripts e a escrita em .wx-migration; o resto do Claude Code segue; o hook custa 54 ms medidos."),
]
regras += [
    ("Regra de negócio se esconde em arquivo de declaração", "o piloto vertical achou uma função de arredondamento de dinheiro dentro de um mod.rs — e por isso o grafo só perdoa arquivo cuja última linha útil ainda seja declaração ou importação. Critério medido; lista de nomes teria deixado passar."),
    ("Portão que aprova o que não conferiu é pior que portão nenhum", "restrição sem validador volta INCONCLUSIVA e nunca aprovada; efeito que não deu para ler tem código de saída próprio, para não virar sucesso dentro de um script."),
    ("Toda evidência declara o que NÃO prova", "o campo é obrigatório e o script recusa gravar sem ele: a frase que falta é a que o leitor completa sozinho, sempre para o lado otimista. E a prova vence quando o arquivo muda."),
    ("Quando o portão passa a olhar um campo novo, procure quem não tem esse campo", "o exemplo PHP achou o G0 cobrando wx_version de quem não usa WINDEV, em oito lugares. O teste que trava o conserto é o do comportamento velho."),
    ("Grafo que completa lacuna sozinho é pior que planilha", "coluna vazia é ligação inexistente, e isso vira lacuna, não palpite — porque planilha incompleta parece incompleta, e grafo completo parece completo."),
    ("Documento de auditoria vale pelo que recusa afirmar", "o SLSA deixa o nível INDISPONÍVEL porque depende da infraestrutura; o atestado diz em letras que não é attestation. Um campo attested: true sem quote é a mentira que alguém leva para auditoria."),
    ("Defeito aparece rodando, não lendo", "grep sai 1 quando não acha, e a regra «não há segredo aqui» reprovava o projeto limpo; a telemetria saía com duração zero porque o campo do registro chama-se ms; a seta de retorno do fluxograma passava por cima da própria caixa."),
]
regras.append(("Skill que não aparece na listagem não existe.", "As oito skills de ERP vieram com descrições de até 900 caracteres, e a impeccable já tinha provado que 895 some da listagem de uma sessão nova. Entraram com 150, os originais guardados ao lado; a prova é a sessão listar as onze pelo nome."))
regras_html = "".join(f'<div class="regra"><b>{E(a)}</b><p>{E(b)}</p></div>' for a, b in regras)
faltas = [
    "Nenhum projeto real, de cliente, passou pelos gates G1 a G7 de ponta a ponta; os exemplos são sintéticos. É a coisa mais valiosa que continua sem medição.",
    "O piloto vertical G4 provou um MÓDULO (cinco regras do ESTOQUE, 10/10 no golden master capturado do legado), não um sistema: telas, relatórios, integrações e o banco real ficaram de fora, e a query saiu com confiança média por não ter banco por trás.",
    "Os quatro comandos de adoção da governança nunca rodaram num cliente; a ordem proposta é raciocínio, não medição.",
    "O instalador em PowerShell nunca foi executado — só tem prova estrutural, porque não há Windows neste ambiente.",
    "A licença é dissuasão por hook; servir corpus e agentes de um servidor ficou para depois, por decisão do dono.",
    "O custo em tokens do questionário inteiro numa sessão real não foi medido.",
    "O questionário não pausa nem retoma; com mais de setenta itens, isso pesa.",
    "Dos seis documentos de auditoria, só a procedência tem caso de uso comercial claro; os outros cinco ainda não foram exigidos por ninguém.",
]
faltas_html = "".join(f"<li>{E(f)}</li>" for f in faltas)
mapa = [("MANUAL.md, docs/manual-de-uso.pdf", "manual de oito capítulos"), ("docs/relatorio-do-plugin.md", "relatório medido do estado atual"), ("docs/investidor/", "página e PDF para investidores"), ("docs/prints/, docs/video/", "provas reais e o vídeo de uso"), ("docs/relatorio-de-cenarios.pdf", "relatório da bateria pesada, gerado rodando os doze cenários"), ("docs/analise-aula-vibe-coding.md", "o que a aula de vibe coding ensinou e o que não se copiou"), ("docs/telas-licenca/", "sete telas do fluxo de liberação de licença"), ("licenca/LEIA-ME.md", "o que o serial protege e o que não"), ("skills/LEIA-ME-erp.md", "as oito skills de ERP do pacote skills.sh e o esqueleto que L6 gera"), ("exemplos/estoque-wx/", "exemplo em WINDEV: teste de regressão do fluxo inteiro"), ("exemplos/faturamento-php/", "exemplo em PHP procedural, sem nada de WX; golden master capturado rodando o legado"), ("ferramentas/wx-modelos/", "binário Rust, std pura, para escolher e controlar o modelo local"), ("docs/dossie/fluxo-atual.pdf", "o fluxograma, gerado do repositório desde a 3.34.0"), ("tests/testes.py", f"{n['testes']} testes; o validador estrito os roda")]
mapa_html = "".join(f'<tr><td><code>{E(a)}</code></td><td>{E(b)}</td></tr>' for a, b in mapa)

page = f'''<title>Dossiê WX Claude Code</title>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@500;700;800&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=JetBrains+Mono:wght@400&display=swap">
<style>
:root{{--g:#FBFAF7;--p:#FFFFFF;--i:#14161F;--m:#6B6F82;--l:#E4E2DB;--a:#C63C0A;--a2:#1F5FBF;--ok:#1F7A4D;--capa:#010418;--capai:#FFFFFF;--capam:#9AA0B8;--capaa:#E2261C}}
@media(prefers-color-scheme:dark){{:root:not([data-theme="light"]){{--g:#0B0D17;--p:#121527;--i:#EDEDF3;--m:#9AA0B8;--l:#252A42;--a:#E2261C;--a2:#4EA1FF;--ok:#2FBF71}}}}
:root[data-theme="dark"]{{--g:#0B0D17;--p:#121527;--i:#EDEDF3;--m:#9AA0B8;--l:#252A42;--a:#E2261C;--a2:#4EA1FF;--ok:#2FBF71}}
body{{margin:0;background:var(--g);color:var(--i);font-family:"Source Serif 4",Georgia,serif;font-size:16px;line-height:1.55}}
h1,h2,h3,.kpi b,.passo,.selo{{font-family:"Exo 2",system-ui,sans-serif}}
.capa{{background:var(--capa);color:var(--capai);padding:56px 24px 48px}}
.capa .w{{display:flex;gap:28px;align-items:center;flex-wrap:wrap}}
.capa img{{width:120px;height:120px}}
.capa h1{{margin:0;font-size:40px;font-weight:800;letter-spacing:.5px;color:var(--capaa);text-wrap:balance}}
.capa p{{margin:6px 0 0;color:var(--capam);max-width:60ch}}
.selo{{display:inline-block;margin-top:14px;border:1.5px solid var(--capaa);color:var(--capai);padding:6px 12px;border-radius:999px;font-size:13px;letter-spacing:.8px;text-transform:uppercase}}
.w{{max-width:1040px;margin:0 auto;padding:0 24px}}
section{{padding:36px 0;border-bottom:1px solid var(--l)}}
h2{{font-size:22px;font-weight:700;margin:0 0 16px;color:var(--a)}}
.kpis{{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:14px}}
@media(max-width:760px){{.kpis{{grid-template-columns:repeat(2,minmax(0,1fr))}}}}
.kpi{{background:var(--p);border:1px solid var(--l);border-radius:10px;padding:16px 18px;display:flex;flex-direction:column;gap:4px}}
.kpi b{{font-size:30px;font-weight:800;font-variant-numeric:tabular-nums}}
.kpi span{{font-size:12px;letter-spacing:.8px;text-transform:uppercase;color:var(--m)}}
.fluxo{{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:8px}}
@media(max-width:760px){{.fluxo{{grid-template-columns:repeat(2,minmax(0,1fr))}}}}
.passo{{border:1.5px solid var(--a2);color:var(--i);border-radius:8px;padding:12px 10px;font-size:14px;font-weight:700;text-align:center}}
.passo small{{display:block;font-family:"Source Serif 4",serif;font-weight:400;color:var(--m);font-size:12px;margin-top:4px}}
table{{border-collapse:collapse;width:100%;font-size:14px;background:var(--p)}}
th,td{{text-align:left;padding:7px 10px;border-bottom:1px solid var(--l);vertical-align:top}}
th{{font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:var(--m)}}
td.num{{font-family:"JetBrains Mono",monospace;font-variant-numeric:tabular-nums;white-space:nowrap;color:var(--a2)}}
code{{font-family:"JetBrains Mono",monospace;font-size:13px}}
.regras{{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:14px}}
@media(max-width:760px){{.regras{{grid-template-columns:1fr}}}}
.regra{{background:var(--p);border:1px solid var(--l);border-radius:10px;padding:14px 16px}}
.regra b{{display:block;font-family:"Exo 2",sans-serif;margin-bottom:4px}}
.regra p{{margin:0;font-size:14px;color:var(--m)}}
.scroll{{overflow-x:auto}}
ul{{padding-left:20px}} li{{margin:.3em 0}}
footer{{padding:24px;color:var(--m);font-size:12px;text-align:center}}
p{{max-width:70ch}}
</style>
<div class="capa"><div class="w">
<img src="data:image/png;base64,{(RAIZ / 'docs/telas-licenca/marca.png').exists() and __import__('base64').b64encode((RAIZ / 'docs/telas-licenca/marca.png').read_bytes()).decode() or ''}" alt="">
<div><h1>WX Claude Code</h1><p>Conversão governada de projetos WINDEV, WEBDEV e WINDEV Mobile para outra plataforma. Questionário, gates com aprovação humana, equipe WLanguage sobre o Help, PMO com Scrum, Kanban e PDCA, e o contexto da primeira sessão do Claude Code gerado das respostas.</p>
<span class="selo">versão {E(n["versao"])} · medido em {E(n["medido_em"])}</span></div></div></div>
<div class="w">
{secao("numeros", "Em números", f'<div class="kpis">{kpi_html}</div><p style="margin-top:12px;color:var(--m);font-size:13px">Cada número é contado no repositório por <code>docs/dossie/numeros-do-plugin.py</code>; a tabela completa está em <code>docs/relatorio-do-plugin.md</code>.</p>')}
{secao("fluxo", "Como um projeto atravessa o plugin", f'<div class="fluxo">{fluxo_html}</div><p style="margin-top:14px">O questionário tem o bloco 0 da empresa ({n["itens_do_bloco_0"]} itens), as letras A a J, K do ambiente ({n["itens_k"]} itens) e L do contexto ({n["itens_l"]} itens) e M dos artefatos do cliente; a letra F sozinha tem {n["subperguntas_f"]} subperguntas de qualidade de ERP. Dele saem até {n["arquivos_gerados_pelo_questionario"]} arquivos: manifesto, configuração, respostas legíveis, empresa, processo, ambiente com instalador e SQL dos papéis, prompts de kickoff e prototipação, <code>INDEX_FILES.md</code>, o <code>.claude/</code> do projeto, Dockerfile e compose. O PMO fecha cada sprint com o relatório de onze seções e a exportação entrega o projeto organizado, com hashes, na pasta do usuário.</p>')}
{secao("regras", "Regras que o projeto aprendeu medindo", f'<div class="regras">{regras_html}</div>')}
{secao("provas", "O que foi provado em sessão real", f'<p>Cada linha é um print em <code>docs/prints/</code>: saída real de uma sessão do Claude Code ou de um script, sem edição. O vídeo de uso ({E(n["video_duracao"])}, {n["video_cenas"]} cenas) reproduz as mesmas capturas.</p><div class="scroll"><table><thead><tr><th>print</th><th>origem</th></tr></thead><tbody>{provas_html}</tbody></table></div>')}
{secao("faltas", "O que ainda não foi provado", f'<ul>{faltas_html}</ul>')}
{secao("versoes", "Versões", f'<div class="scroll"><table><thead><tr><th>versão</th><th>data</th><th>o que entrou</th></tr></thead><tbody>{versoes_html}</tbody></table></div><p style="color:var(--m);font-size:13px">Lido do <code>git log</code>: só os commits cuja mensagem começa pela versão.</p>')}
{secao("mapa", "Onde está cada coisa", f'<div class="scroll"><table><thead><tr><th>arquivo</th><th>o que é</th></tr></thead><tbody>{mapa_html}</tbody></table></div>')}
</div>
<footer>Gerado por <code>docs/dossie/gerar-dossie.py</code> em {E(n["medido_em"])}; regenerar em vez de editar. Built to store. Engineered to scale.</footer>
'''
(RAIZ / "docs/dossie/dossie-wx-claude-code.html").write_text(page, encoding="utf-8")
print("ok", len(versoes), "versoes", len(provas), "provas")

#!/usr/bin/env python3
"""O dossie dos testes: o que este banco prova, e como.

    python3 docs/dossie/pagina-dos-testes.py [saida.html]

Pedido do dono em 05/09/2026, na leva das cinco baterias. E' a TERCEIRA
pagina do projeto, ao lado do dossie tecnico e da relacao dos pedidos, e
nasce pela mesma lei que fez as outras duas nascerem geradas: **todo numero
visivel sai de um gerador, ou esta errado e ninguem percebeu ainda.**

# O que ele faz de diferente dos outros geradores da pasta

Os outros escrevem numero DENTRO de um documento que uma pessoa redigiu.
Este monta a pagina inteira, porque ela e' quase toda numero -- e a prosa
que sobra e' julgamento que nao se mede: o que cada camada de prova
alcanca, e o que ela nao alcanca.

# A disciplina da DATA, e ela e' o motivo deste arquivo ser mais chato do
# que parece

Os `resultados.json` das bancadas sao de corridas de dias diferentes. Uma
pagina que os junta sem dizer QUANDO cada um foi medido publica um retrato
que nunca existiu -- e e' exatamente o erro que o selo da capa cometeu por
quatro lancamentos. Entao cada numero aqui carrega a data de onde veio:

    campo `quando` do proprio JSON, quando ele tem;
    senao o mtime do arquivo, e a pagina DIZ que foi o mtime.

# E a disciplina do que FALTA

Bancada cujo arquivo de resultado nao existe **aparece como nao medida**,
com o motivo, em vez de sumir da tabela. Papel que nao esta cumprindo
aparece como nao cumprindo -- e uma pagina de testes que esconde a bancada
que nao rodou e' a pior de todas, porque ela existe justamente para dizer
em que a gente pode confiar.
"""
import datetime
import json
import re
import subprocess
import sys
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parent.parent
SAIDA_PADRAO = AQUI / "testes.html"


# ---------------------------------------------------------------- fontes

def quando_de(caminho: Path, dados=None) -> tuple[str, bool]:
    """A data da medicao, e se ela veio do mtime em vez do proprio arquivo.

    O segundo valor e' o que a pagina imprime como ressalva: mtime nao e' a
    hora da medicao, e' a hora em que alguem gravou -- um `git checkout`
    move o mtime sem medir nada.
    """
    if isinstance(dados, dict):
        for chave in ("quando", "medido_em", "data"):
            if dados.get(chave):
                return str(dados[chave])[:19], False
    if not caminho.exists():
        return "—", False
    ts = datetime.datetime.fromtimestamp(caminho.stat().st_mtime)
    return ts.strftime("%Y-%m-%d %H:%M"), True


def ler_json(rel: str):
    """Le um JSON da raiz do projeto. Devolve (dados, caminho) ou (None, caminho)."""
    p = RAIZ / rel
    if not p.exists():
        return None, p
    try:
        return json.loads(p.read_text(encoding="utf-8")), p
    except (json.JSONDecodeError, UnicodeDecodeError) as e:
        # Nao morre: uma bancada com JSON quebrado e' um achado a mostrar,
        # nao um motivo para a pagina inteira nao sair.
        return {"__erro__": str(e)}, p


def capabilities():
    d, p = ler_json("CAPABILITIES.json")
    if d is None:
        return {}, ("—", False)
    return d, quando_de(p, d)


def guardas():
    """Quantas guardas o catalogo tem, importando-o em vez de contando texto.

    Contar `"id":` por regex mediria o arquivo; importar mede a LISTA, que e'
    o que a bateria percorre. Se um dia alguem montar a lista por laco, o
    regex passaria a contar zero e ninguem veria.
    """
    sys.path.insert(0, str(RAIZ / "bancada" / "guardas"))
    try:
        import catalogo  # noqa: PLC0415
        return len(catalogo.GUARDAS), None
    except Exception as e:  # noqa: BLE001 -- qualquer falha vira linha na pagina
        return None, str(e)


def catracas():
    """As catracas do repositorio, lidas do fonte.

    A lista NAO se digita aqui: uma catraca nova que ninguem lembrasse de
    acrescentar sumiria da pagina, e a pagina passaria a dizer que ha menos
    trava do que ha. E' o mesmo motivo pelo qual a receita do KiB de
    interface passou a sair do `http.rs`.
    """
    achadas = []
    padrao = re.compile(r"const (TETO[A-Z_0-9]*)\s*:\s*\w+\s*=\s*([0-9_]+)")
    for rs in sorted((RAIZ / "crates").glob("*/src/**/*.rs")):
        try:
            texto = rs.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for m in padrao.finditer(texto):
            achadas.append((m.group(1), int(m.group(2).replace("_", "")),
                            str(rs.relative_to(RAIZ))))
    achadas.sort()
    return achadas


def casos_de_tela():
    d = RAIZ / "testes-web" / "casos"
    return sorted(p.name for p in d.glob("*.mjs")) if d.exists() else []


def botoes_exercitados():
    p = RAIZ / "testes-web" / "botoes-exercitados.txt"
    if not p.exists():
        return None, ("—", False)
    linhas = [x for x in p.read_text(encoding="utf-8").splitlines()
              if x.strip() and not x.startswith("#")]
    return len(linhas), quando_de(p)


# As bancadas, e o que se le de cada uma. A tabela e' DECLARADA porque cada
# `resultados.json` tem forma propria -- um extrator generico teria de
# adivinhar qual chave e' resultado e qual e' configuracao, e adivinhar e'
# o que esta pagina existe para nao fazer.
BANCADAS = [
    {
        "nome": "Cluster — eleição e promoção",
        "json": "bancada/cluster/resultados.json",
        "roda": "python3 bancada/cluster/provar.py",
        "prova": "três nós de verdade, um SMTP falso, e a promoção acontecendo "
                 "quando o master cai",
        "campos": [("promocao_s", "promoção em", "s"),
                   ("escrita_aceita_s", "escrita aceita em", "s"),
                   ("epoca_do_isolado", "época do nó isolado", ""),
                   ("linhas_no_fim", "linhas no fim", "")],
    },
    {
        "nome": "Replicação — quatro servidores",
        "json": "bancada/replicacao/resultados.json",
        "roda": "python3 bancada/replicacao/montar.py && …/medir.py",
        "prova": "master e três espelhos no ar, com retrato SHA-256 de cada "
                 "linha nos quatro",
        "campos": [("master_linhas_s", "master", "linhas/s"),
                   ("replica_eventos_s", "réplica aplica", "eventos/s"),
                   ("alcance_s", "alcance", "s"),
                   ("iguais_no_fim", "os quatro iguais no fim", "")],
    },
    {
        "nome": "ACID — as quatro letras",
        "json": "bancada/acid/resultado.json",
        "roda": "python3 bancada/acid/prova.py",
        "prova": "cada afirmação com o controle da mesma corrida; o nível de "
                 "isolamento sai medido, não citado",
        "campos": [],  # o extrator proprio, abaixo
    },
    {
        "nome": "Durabilidade — o fecho da janela",
        "json": "bancada/durabilidade/resultado-do-fecho.json",
        "roda": "python3 bancada/durabilidade/prova-do-fecho.py",
        "prova": "os `fsync` contados por `strace` contra o `phxsqld` de pé",
        "campos": [],
    },
    {
        "nome": "Carga — uma a uma contra o lote",
        "json": "bancada/carga/resultados.json",
        "roda": "python3 bancada/carga/medir.py 20000",
        "prova": "o custo da viagem de rede, separado do custo de gravar",
        "campos": [],
    },
    {
        "nome": "Os três motores a um milhão",
        "json": "bancada/comparacao/um-milhao.json",
        "roda": "python3 bancada/comparacao/medir.py",
        "prova": "PhxSql, MySQL® e SQLite® intercalados na MESMA rodada, "
                 "porque medidas de dias diferentes carregam o ambiente junto",
        "campos": [],
    },
    {
        "nome": "Bateria única — o motor por dentro",
        "json": "bancada/bateria/resultados.json",
        "roda": "python3 bancada/bateria/prova-bateria.py",
        "prova": "as guardas do catálogo, cada uma contra o defeito que a motivou",
        "campos": [],
    },
]


def valor_legivel(v):
    if isinstance(v, bool):
        return "sim" if v else "NÃO"
    if isinstance(v, float):
        return f"{v:g}".replace(".", ",")
    if isinstance(v, int):
        return f"{v:,}".replace(",", ".")
    return str(v)[:60]


# ---------------------------------------------------------------- pagina

def esc(s):
    return (str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;"))


def linha_bancada(b):
    dados, p = ler_json(b["json"])
    if dados is None:
        return (f'<tr class="ausente"><td class="nome">{esc(b["nome"])}</td>'
                f'<td class="q">não medida</td>'
                f'<td class="v">o arquivo <code>{esc(b["json"])}</code> não existe — '
                f'rode <code>{esc(b["roda"])}</code></td></tr>')
    if "__erro__" in dados:
        return (f'<tr class="ausente"><td class="nome">{esc(b["nome"])}</td>'
                f'<td class="q">ilegível</td>'
                f'<td class="v">{esc(dados["__erro__"])}</td></tr>')
    quando, do_mtime = quando_de(p, dados)
    marca = ' <span class="mtime" title="a data saiu do mtime do arquivo, '\
            'e nao do proprio resultado">(mtime)</span>' if do_mtime else ""

    partes = []
    for chave, rotulo, unidade in b["campos"]:
        if chave in dados:
            u = f" {unidade}" if unidade else ""
            partes.append(f'<b>{esc(rotulo)}</b> {esc(valor_legivel(dados[chave]))}{u}')
    if not partes:
        # Sem campos declarados, mostra as chaves de topo que sao escalares --
        # e' melhor mostrar o que ha do que uma celula vazia que parece falha.
        for k, v in list(dados.items())[:4]:
            if isinstance(v, (int, float, bool, str)) and not k.startswith("_"):
                partes.append(f'<b>{esc(k)}</b> {esc(valor_legivel(v))}')
    return (f'<tr><td class="nome">{esc(b["nome"])}<div class="prova">'
            f'{esc(b["prova"])}</div></td>'
            f'<td class="q mono">{esc(quando)}{marca}</td>'
            f'<td class="v">{" · ".join(partes) or "—"}</td></tr>')


def montar():
    cap, (cap_quando, cap_mtime) = capabilities()
    n_guardas, erro_guardas = guardas()
    tetos = catracas()
    casos = casos_de_tela()
    n_botoes, (botoes_quando, botoes_mtime) = botoes_exercitados()
    agora = datetime.datetime.now().strftime("%d/%m/%Y %H:%M")

    def c(k, padrao="—"):
        """O valor do `CAPABILITIES.json`, com o ponto de milhar.

        O JSON guarda o numero cru, que e' o certo para quem le por
        programa. Quem le esta pagina e' gente, e `138426` se le pior que
        `138.426` -- separador nao e' enfeite, e' o que faz a ordem de
        grandeza aparecer sem contar digito.
        """
        v = cap.get(k, padrao)
        if isinstance(v, int) and abs(v) >= 1000:
            return esc(f"{v:,}".replace(",", "."))
        return esc(v)

    linhas_bancadas = "\n      ".join(linha_bancada(b) for b in BANCADAS)
    linhas_tetos = "\n      ".join(
        f'<tr><td class="mono">{esc(n)}</td><td class="n mono">{v:,}</td>'
        f'<td class="mono peq">{esc(a)}</td></tr>'.replace(",", ".")
        for n, v, a in tetos)
    lista_casos = "".join(f"<li><code>{esc(x)}</code></li>" for x in casos)

    return TEMPLATE.format(
        agora=agora,
        versao=c("versao"), commit=esc(str(cap.get("commit", "—"))[:7]),
        testes=c("testes"), operacoes=c("operacoes"),
        linhas_rust=c("linhas_rust"), deps=c("dependencias_externas"),
        cap_quando=esc(cap_quando),
        cap_mtime=" (mtime)" if cap_mtime else "",
        guardas=(f"{n_guardas}" if n_guardas is not None
                 else f"não contadas: {esc(erro_guardas)}"),
        n_casos=len(casos), lista_casos=lista_casos,
        n_botoes=(n_botoes if n_botoes is not None else "não medido"),
        botoes_quando=esc(botoes_quando),
        botoes_mtime=" (mtime)" if botoes_mtime else "",
        n_tetos=len(tetos), linhas_tetos=linhas_tetos,
        linhas_bancadas=linhas_bancadas,
    )


TEMPLATE = """<title>Dossiê dos testes do PhxSql</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Exo+2:wght@400;500;600;700&family=Source+Serif+4:opsz,wght@8..60,400;8..60,600&family=IBM+Plex+Mono:wght@400;500&display=swap">
<style>
:root{{
  --papel:#fbf9f7; --papel-2:#f3efec; --tinta:#1a1210; --tinta-2:#4a3f3a;
  --tinta-3:#7a6d66; --linha:#ded6d0; --acento:#c63c0a;
  --ok:#2f7a3e; --falta:#8a6a1f;
}}
@media (prefers-color-scheme:dark){{
  :root:not([data-theme="light"]){{
    --papel:#040814; --papel-2:#0a1122; --tinta:#dde2eb; --tinta-2:#a8b0c0;
    --tinta-3:#7c8598; --linha:#1e2940; --acento:#ff8a1c;
    --ok:#5cbf74; --falta:#d5a83c;
  }}
}}
:root[data-theme="dark"]{{
  --papel:#040814; --papel-2:#0a1122; --tinta:#dde2eb; --tinta-2:#a8b0c0;
  --tinta-3:#7c8598; --linha:#1e2940; --acento:#ff8a1c;
  --ok:#5cbf74; --falta:#d5a83c;
}}
*{{box-sizing:border-box}}
body{{margin:0;background:var(--papel);color:var(--tinta);
  font-family:"Source Serif 4",Georgia,serif;font-size:16px;line-height:1.55;
  -webkit-font-smoothing:antialiased}}
h1,h2,.rotulo,.pino{{font-family:"Exo 2","Helvetica Neue",Arial,sans-serif}}
code,.mono{{font-family:"IBM Plex Mono",ui-monospace,Menlo,monospace}}
code{{font-size:.86em;background:var(--papel-2);padding:1px 4px;border-radius:3px;
  color:var(--tinta-2)}}
.envelope{{max-width:1000px;margin:0 auto;padding:0 20px 80px}}
header{{padding:52px 0 26px;border-bottom:1px solid var(--linha)}}
.rotulo{{font-size:10.5px;letter-spacing:.18em;text-transform:uppercase;
  color:var(--acento);font-weight:600;margin-bottom:12px}}
h1{{font-size:clamp(28px,5vw,44px);font-weight:700;line-height:1.08;margin:0 0 14px;
  letter-spacing:-.015em;text-wrap:balance}}
.chamada{{max-width:64ch;color:var(--tinta-2);font-size:17px;margin:0}}
.placar{{display:grid;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));
  gap:10px;margin:28px 0 0}}
.placar .c{{border:1px solid var(--linha);border-radius:6px;padding:13px 15px;
  background:var(--papel-2)}}
.placar .v{{font-family:"Exo 2",sans-serif;font-size:30px;font-weight:700;
  line-height:1;font-variant-numeric:tabular-nums}}
.placar .r{{font-family:"IBM Plex Mono",monospace;font-size:10px;letter-spacing:.13em;
  text-transform:uppercase;color:var(--tinta-3);margin-top:7px}}
h2{{font-size:21px;font-weight:600;margin:48px 0 6px;letter-spacing:-.01em}}
h2 + .sub{{color:var(--tinta-3);font-size:15px;margin:0 0 18px;max-width:66ch}}
.rolo{{overflow-x:auto}}
table{{border-collapse:collapse;width:100%;min-width:600px}}
thead th{{font-family:"IBM Plex Mono",monospace;font-weight:500;font-size:10px;
  letter-spacing:.13em;text-transform:uppercase;color:var(--tinta-3);text-align:left;
  padding:11px 11px 7px;border-bottom:1px solid var(--linha)}}
tbody td{{padding:13px 11px;border-bottom:1px solid var(--linha);vertical-align:top;
  font-size:14.5px}}
td.nome{{width:32%;color:var(--tinta)}}
td.q{{width:150px;color:var(--tinta-3);font-size:12.5px;white-space:nowrap}}
td.n{{text-align:right;font-variant-numeric:tabular-nums;width:90px}}
td.v{{color:var(--tinta-2)}}
.prova{{color:var(--tinta-3);font-size:13px;margin-top:4px}}
.peq{{font-size:11.5px;color:var(--tinta-3)}}
tr.ausente td{{color:var(--falta)}}
.mtime{{color:var(--falta);font-size:11px}}
.nota{{border-left:3px solid var(--acento);background:var(--papel-2);
  padding:14px 18px;border-radius:0 5px 5px 0;margin:24px 0;font-size:15px;
  color:var(--tinta-2);max-width:68ch}}
.nota b{{color:var(--tinta)}}
ul.casos{{columns:250px;list-style:none;padding:0;margin:0;font-size:13.5px}}
ul.casos li{{padding:2px 0}}
footer{{margin-top:52px;padding-top:20px;border-top:1px solid var(--linha);
  color:var(--tinta-3);font-size:13.5px;max-width:68ch}}
@media (prefers-reduced-motion:reduce){{*{{transition:none!important}}}}
</style>
<div class="envelope">
<header>
  <div class="rotulo">PhxSql · o dossiê dos testes</div>
  <h1>O que este banco prova,<br>e como</h1>
  <p class="chamada">Cinco camadas de prova, do byte no disco ao clique no
  navegador. Cada número desta página sai de um gerador e traz a data em que
  foi medido — porque um retrato montado com medições de dias diferentes é um
  retrato que nunca existiu.</p>

  <div class="placar">
    <div class="c"><div class="v">{testes}</div><div class="r">testes</div></div>
    <div class="c"><div class="v">{guardas}</div><div class="r">guardas</div></div>
    <div class="c"><div class="v">{n_casos}</div><div class="r">casos de tela</div></div>
    <div class="c"><div class="v">{n_tetos}</div><div class="r">catracas</div></div>
    <div class="c"><div class="v">{deps}</div><div class="r">dependências</div></div>
  </div>
</header>

<h2>1. O motor, por dentro</h2>
<p class="sub"><code>cargo test --workspace</code>. É a camada mais barata e a
mais rasa: ela prova que a função faz o que o autor dela achou que faz.</p>
<div class="nota">
  <b>{testes} testes</b> em {linhas_rust} linhas de Rust, versão
  <code>{versao}</code>, commit <code>{commit}</code>, {operacoes} operações no
  protocolo. Medido em {cap_quando}{cap_mtime}, por
  <code>docs/dossie/numeros-do-projeto.py</code>, que <b>aborta se o
  <code>cargo test</code> falhar</b>.
</div>

<h2>2. As guardas, e o defeito que motivou cada uma</h2>
<p class="sub">Um teste diz que o código passa. Uma <b>guarda</b> diz que ele
<b>falha</b> quando o defeito volta — e é provada contra esse defeito
periodicamente. Teste que passa por engano é pior que teste que falta, e esta
camada existe por causa disso.</p>
<div class="nota">
  <b>{guardas} guardas</b> no catálogo (<code>bancada/guardas/catalogo.py</code>),
  cada uma com o defeito reposto, o arquivo, o trecho e quais testes têm de cair.
  Roda por <code>python3 bancada/guardas/provar-guardas.py</code>.
</div>

<h2>3. As catracas</h2>
<p class="sub">Número que <b>só desce</b>. E que <b>nunca sobe</b> — nem quando
a régua muda: régua que passa a medir mais <b>aposenta</b> a catraca antiga e
faz nascer outra, no número medido do dia. A lista abaixo sai do fonte, não de
uma lista digitada aqui.</p>
<div class="rolo">
  <table>
    <thead><tr><th>catraca</th><th class="n">hoje</th><th>onde</th></tr></thead>
    <tbody>
      {linhas_tetos}
    </tbody>
  </table>
</div>

<h2>4. As bancadas — contra o servidor de verdade</h2>
<p class="sub">Teste unitário não prova queda de conexão, nem durabilidade, nem
eleição de master. O que depende do sistema operacional se prova contra o
sistema operacional. Bancada sem arquivo de resultado <b>aparece como não
medida</b> em vez de sumir da tabela.</p>
<div class="rolo">
  <table>
    <thead><tr><th>bancada</th><th>medida em</th><th>o número de então</th></tr></thead>
    <tbody>
      {linhas_bancadas}
    </tbody>
  </table>
</div>

<h2>5. A tela — exercitando, não lendo</h2>
<p class="sub">Interface só se prova exercitando. Gravar um vídeo de
demonstração achou três defeitos em cinco minutos que ler o código não acharia,
e o pior deles quebrava todo salvar e todo incluir pela tela.</p>
<div class="nota">
  <b>{n_casos} casos</b> em <code>testes-web/casos/</code>, quase todos nos dois
  temas, contra um <code>phxsqld</code> de verdade num navegador de verdade — sem
  maquete e sem mockup. E <b>{n_botoes} botões</b> com clique gravado
  (<code>testes-web/botoes-exercitados.txt</code>, {botoes_quando}{botoes_mtime}):
  a evidência é escrita pela própria corrida, e só em corrida <b>inteira e
  verde</b>, porque uma corrida que cai no meio grava menos do que provou.
</div>
<ul class="casos">{lista_casos}</ul>

<h2>6. O que estas provas NÃO cobrem</h2>
<p class="sub">A parte que uma página de testes costuma esconder, e a única que
diz onde não confiar.</p>
<div class="nota">
  <b>O <code>SIGKILL</code> não distingue «está na mídia» de «está no cache».</b>
  Duas inserções em <code>por_lote</code>, com <b>zero</b> <code>fsync</code> no
  <code>.reg</code>, voltam inteiras depois da queda — do cache do núcleo, que a
  morte do processo não esvazia. Quem mede durabilidade aqui é a contagem de
  <code>fsync</code>; o <code>SIGKILL</code> prova o protocolo de commit, não a
  mídia. Só queda de energia mostraria o resto.
</div>
<div class="nota">
  <b>Número de tempo medido com a máquina ocupada mede a carga, não o item.</b>
  Por isso existe o portão <code>bancada/esta-medindo.sh</code>, e por isso há
  medições nesta casa que ficam <b>nomeadas e não medidas</b>, com o motivo
  escrito. Isso é resultado, não falha.
</div>
<div class="nota">
  <b>Cobertura não é prova.</b> A tabela de testes por área diz onde a
  cobertura é rala, e não que o resto está certo — um arquivo sem
  <code>#[test]</code> dentro pode estar inteiramente coberto por
  <code>tests/</code>, e um com muitos pode estar provando a intenção em vez do
  efeito.
</div>

<footer>
  Gerado por <code>docs/dossie/pagina-dos-testes.py</code> em {agora}. Nenhum
  número desta página foi digitado: eles saem do <code>CAPABILITIES.json</code>,
  do catálogo de guardas, dos <code>resultados.json</code> das bancadas, dos
  arquivos de <code>testes-web/</code> e das constantes do próprio fonte. O
  dossiê técnico e a relação dos pedidos são as outras duas páginas.
</footer>
</div>
"""


def principal():
    saida = Path(sys.argv[1]) if len(sys.argv) > 1 else SAIDA_PADRAO
    html = montar()
    saida.write_text(html, encoding="utf-8")
    n = len(html.encode("utf-8"))
    print(f"pagina gravada: {saida} ({n:,} bytes)".replace(",", "."))
    faltando = [b["nome"] for b in BANCADAS if not (RAIZ / b["json"]).exists()]
    if faltando:
        print("bancadas sem resultado (aparecem como NAO MEDIDAS na pagina):")
        for f in faltando:
            print(f"   · {f}")


if __name__ == "__main__":
    principal()

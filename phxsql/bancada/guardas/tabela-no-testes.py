#!/usr/bin/env python3
"""Regrava a tabela das guardas no `docs/TESTES.md` com o resultado MEDIDO.

Existe pela mesma lei dos geradores do dossie: numero digitado a mao envelhece
calado. Uma tabela que diz «quinze guardas provadas» e digitada mente no dia em
que a decima sexta entra -- e mente justamente sobre a unica coisa que ela
serve para dizer, que e quais provas ainda pegam o defeito que as motivou.

    python3 bancada/guardas/provar-guardas.py --json /tmp/guardas.json
    python3 bancada/guardas/tabela-no-testes.py /tmp/guardas.json

    python3 bancada/guardas/tabela-no-testes.py /tmp/guardas.json --so-medir
    python3 bancada/guardas/tabela-no-testes.py /tmp/guardas.json --parcial
    python3 bancada/guardas/tabela-no-testes.py --autoteste

O arquivo de entrada e o `--json` do executor: o veredito nao se digita aqui,
ele vem de uma rodada. Sem rodada nao ha tabela -- e nao ter tabela e melhor
que ter uma que ninguem mediu.

# O que ele passou a dizer quando faz menos -- pedido 269

A rodada inteira custa ~3.374 s de mutacao mais a compilacao, e por isso ela
NAO roda a cada commit. A consequencia medida em 16/09/2026: o catalogo tinha
160 entradas e a tabela publicada era de uma corrida que julgou **143** --
honesta sobre a DATA (ela traz o `medido em`) e **muda sobre o TAMANHO**. Quem
lesse «143 guardas» num catalogo de 160 leria um inventario, e ele estava 17
entradas curto.

Duas coisas nasceram daqui, e as duas sao a lei que o `pagina-dos-pedidos.py`
pagou -- *gerador certo chamado pela metade entrega numero velho anunciando
sucesso*:

* **A pagina NOMEIA o que a rodada nao julgou**, sob um aviso que nao e linha
  de exito, e a linha de resumo passa a dizer «143 das 160 guardas do
  catalogo» em vez de «143 guardas». Nao julgada nao e um veredito: ela nao
  esta provada nem reprovada, e fica FORA da tabela de propositado -- linha
  na tabela com um veredito inventado seria pior que a ausencia.
* **Ele RECUSA encolher a tabela publicada** sem um `--parcial` escrito.
  Rodar o provador com `--so uma-guarda` e publicar trocaria 143 linhas por 1,
  escondendo 159 -- e ate aqui nada no caminho recusava.

## A armadilha da recusa, e por que ela olha o CATALOGO e nao a tabela

Encolher pode ser legitimo: uma guarda APOSENTADA sai do catalogo de proposito
(a lista `APOSENTADAS` do `trecho-vivo.py` existe para isso), e a corrida
seguinte julga uma entrada a menos. Uma recusa que so comparasse «quantas
linhas tinha antes x quantas tem agora» transformaria toda aposentadoria numa
parada permanente, pedindo um `--parcial` para uma corrida que nao tem nada de
parcial.

Entao a pergunta que decide NAO e o tamanho, e sim a **cobertura**: *esta
rodada julgou o catalogo inteiro?* Se julgou, ela publica livre, encolhendo ou
nao -- e a aposentadoria passa sem ninguem escrever interruptor nenhum. A
recusa so acontece quando as DUAS coisas valem ao mesmo tempo: a rodada deixou
entrada do catalogo sem julgar **e** a tabela que ja esta publicada e maior
que ela. E' o caso do `--so`, e so ele.

E o corolario, que e o caso de todo dia: uma rodada parcial que NAO encolhe --
republicar a corrida de ontem depois de o catalogo crescer -- passa sem
`--parcial`, e passa porque e exatamente o conserto. Ela nao troca medida
nenhuma: so faz a pagina nomear as entradas novas que aquela corrida nunca viu.
"""

import importlib.util
import json
import pathlib
import sys

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]
ALVO = RAIZ / "docs" / "TESTES.md"

INICIO = "<!-- guardas:inicio -->"
FIM = "<!-- guardas:fim -->"

# O comeco de uma linha de veredito, escrito UMA vez e usado pelos dois lados:
# quem escreve a linha e quem conta quantas ja estao publicadas. Duas receitas
# do mesmo formato divergem na primeira vez que alguem mexer numa delas, e a
# que ficaria para tras e a que RECUSA -- portao que mede errado o tamanho da
# tabela anterior recusa o certo e deixa passar o defeito.
PREFIXO_DA_LINHA = "| `"

sys.path.insert(0, str(AQUI))
from catalogo import GUARDAS  # noqa: E402

MARCA = {
    "PROVADA": "✅ provada",
    "REDUNDANTE": "🟰 redundante",
    "NAO PEGOU": "❌ **não pegou**",
    "ESTRAGOU": "⚠️ estragou demais",
    "QUEBRADA": "⚠️ quebrada",
}

_APOSENTADAS_DO_TRECHO_VIVO = None


def aposentadas_do_trecho_vivo():
    """A lista `APOSENTADAS` do `trecho-vivo.py`, pelo proprio modulo.

    Nunca copiada aqui: duas listas da mesma coisa divergem na primeira vez
    que uma aprender algo (um id renomeado, um motivo reescrito) -- e a
    divergencia seria dupla, porque a tabela publicaria uma guarda como viva
    ou como aposentada errado e as duas fontes continuariam concordando
    consigo mesmas. E a mesma lei do KiB da interface: quando um gerador
    depende de uma lista, a lista tem de sair do codigo.

    O `trecho-vivo.py` tem hifen no nome (nao da para `import trecho_vivo`) e
    termina com um `if __name__ == "__main__": sys.exit(principal())` que
    dispara a catraca do catalogo inteiro -- uma varredura de `crates/**/*.rs`
    so para ler uma lista de tres campos. `exec_module` roda as DEFINICOES do
    modulo (funcoes, constantes, a propria `APOSENTADAS`) sob um `__name__`
    que NAO e `"__main__"`, entao aquele bloco final fica inerte: ele so
    dispara quando o interpretador chama o arquivo como script, nunca quando
    outro modulo o carrega assim. E a mesma tecnica que o proprio
    `trecho-vivo.py` ja usa para carregar `catalogo.py` e este arquivo --
    quem tentar `import` do jeito obvio vai bater no hifen primeiro."""
    global _APOSENTADAS_DO_TRECHO_VIVO
    if _APOSENTADAS_DO_TRECHO_VIVO is None:
        caminho = AQUI / "trecho-vivo.py"
        spec = importlib.util.spec_from_file_location(
            "trecho_vivo_das_guardas", caminho)
        modulo = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(modulo)
        _APOSENTADAS_DO_TRECHO_VIVO = {a["id"]: a for a in modulo.APOSENTADAS}
    return _APOSENTADAS_DO_TRECHO_VIVO


def nao_julgadas(guardas, dados):
    """Os ids do catalogo que esta rodada NAO julgou, na ordem do catalogo.

    Receita unica de proposito: o `trecho-vivo.py` faz a mesma pergunta para a
    catraca, e duas subtracoes de conjunto escritas em dois arquivos divergem
    na primeira vez que uma delas aprender algo (um id normalizado, uma
    aposentadoria contada de outro jeito) -- e divergiriam em silencio, uma
    dizendo que a pagina esta inteira e a outra publicando que nao esta.

    O catalogo vem por ARGUMENTO, e nao lido aqui dentro, porque quem confere
    ja o tem carregado: a catraca roda em toda bateria e carregar o
    `catalogo.py` duas vezes cobraria o dobro de uma regua de 0,2 s."""
    julgados = {r.get("id") for r in dados.get("guardas") or []}
    return [g["id"] for g in guardas if g.get("id") not in julgados]


def tabela(dados, guardas=None, aposentadas=None):
    guardas = GUARDAS if guardas is None else guardas
    aposentadas = (aposentadas_do_trecho_vivo() if aposentadas is None
                   else aposentadas)
    por_id = {g["id"]: g for g in guardas}
    linhas = [
        "| guarda | o defeito reposto | testes que caem | veredito |",
        "|---|---|---:|---|",
    ]
    conta = {}
    citadas = []
    for r in dados["guardas"]:
        g = por_id.get(r["id"], {})
        aposentada = aposentadas.get(r["id"])
        if aposentada:
            # A corrida julgou esta guarda ANTES dela sair do catalogo -- o
            # veredito que ela trouxe (PROVADA, QUEBRADA...) descrevia um
            # defeito que nao existe mais. Publica-lo como vivo mentiria: e
            # a familia da "chave morta", e pior que a linha faltar, porque
            # uma linha ausente ninguem confunde com garantia.
            conta["APOSENTADA"] = conta.get("APOSENTADA", 0) + 1
            citadas.append((r["id"], aposentada))
            veredito_txt = "🪦 aposentada (%s)" % aposentada["data"]
        else:
            conta[r["veredito"]] = conta.get(r["veredito"], 0) + 1
            veredito_txt = MARCA.get(r["veredito"], r["veredito"])
        quantos = len(g.get("caem", []))
        linhas.append(
            "%s%s` | %s | %s | %s |"
            % (PREFIXO_DA_LINHA, r["id"], r.get("titulo", g.get("titulo", "")),
               quantos if quantos else "—",
               veredito_txt))
    total = len(dados["guardas"])
    # O plural sai do numero, e nao de uma segunda tabela escrita a mao: uma
    # guarda «provada» e catorze «provada» e o tipo de erro que ninguem revisa.
    plural = {"PROVADA": ("provada", "provadas"),
              "REDUNDANTE": ("redundante", "redundantes"),
              "NAO PEGOU": ("não pegou", "não pegaram"),
              "ESTRAGOU": ("estragou", "estragaram"),
              "QUEBRADA": ("quebrada", "quebradas"),
              "APOSENTADA": ("aposentada", "aposentadas")}
    resumo = ", ".join(
        "%d %s" % (n, plural.get(v, (v.lower(), v.lower()))[0 if n == 1 else 1])
        for v, n in sorted(conta.items()))
    segundos = sum(r["segundos"] for r in dados["guardas"])
    faltam = nao_julgadas(guardas, dados)
    # A frase so muda quando ela PRECISA mudar: corrida inteira continua
    # dizendo «N guardas», porque ali o numero da rodada e o do catalogo sao o
    # mesmo e o «de N» seria ruido. Corrida parcial carrega os dois numeros,
    # que e a unica coisa que a versao anterior desta linha nao dizia.
    quantas = ("%d das %d guardas do catálogo" % (total, len(guardas))
               if faltam else "%d guardas" % total)
    linhas += [
        "",
        "**%s: %s** — %d s de mutação, medido em %s."
        % (quantas, resumo, round(segundos), dados.get("quando", "?")),
    ]
    if faltam:
        por_id_cat = {g["id"]: g for g in guardas}
        linhas += [
            "",
            "> **Esta rodada NÃO julgou %d das %d entradas do catálogo.** Elas "
            "não estão provadas nem reprovadas — a rodada não chegou nelas, e "
            "ler a tabela acima como inventário do catálogo a lê %d entradas "
            "curta. Para julgá-las é preciso uma corrida do "
            "`provar-guardas.py` que as alcance."
            % (len(faltam), len(guardas), len(faltam)),
            "",
        ]
        for ident in faltam:
            titulo = por_id_cat.get(ident, {}).get("titulo", "")
            linhas.append("- `%s` — %s" % (ident, titulo))
    if citadas:
        # A guarda saiu do catalogo, mas a corrida publicada e de ANTES da
        # aposentadoria e ainda a cita -- a pagina nao pode calar isso, senao
        # o motivo da aposentadoria (por que aquele veredito parou de valer)
        # fica so no `trecho-vivo.py`, que quem le a tabela nao abre.
        linhas += ["", "As guardas que esta corrida ainda cita, hoje "
                   "aposentadas:", ""]
        for ident, a in citadas:
            linhas.append("- `%s` (%s) — %s" % (ident, a["data"], a["motivo"]))
    notas = [(r["id"], n) for r in dados["guardas"] for n in r["notas"]]
    if notas:
        linhas += ["", "As notas que a rodada deixou:", ""]
        for ident, n in notas:
            linhas.append("- `%s` — %s" % (ident, n))
    return "\n".join(linhas)


def publicadas(texto):
    """Quantos vereditos a tabela que JA esta no arquivo publica.

    Conta pelo comeco de linha que este mesmo gerador escreve, e nao por
    numero achado na prosa do resumo: a prosa se reescreve (esta rodada
    reescreveu), e catraca que le a propria redacao quebra em silencio no dia
    da reescrita. Arquivo sem as marcas devolve `None` -- nao ha tabela
    anterior, e nao ha o que encolher."""
    if INICIO not in texto or FIM not in texto:
        return None
    bloco = texto.split(INICIO)[1].split(FIM)[0]
    return sum(1 for l in bloco.splitlines() if l.startswith(PREFIXO_DA_LINHA))


def publicar(alvo, dados, parcial=False, guardas=None):
    """Grava o bloco, ou RECUSA e diz por que. Devolve (codigo, recados)."""
    guardas = GUARDAS if guardas is None else guardas
    faltam = nao_julgadas(guardas, dados)
    bloco = tabela(dados, guardas)
    txt = alvo.read_text(encoding="utf-8")
    antes = publicadas(txt)
    if antes is None:
        return 1, ["%s nao tem as marcas %s / %s" % (alvo, INICIO, FIM)]
    agora = len(dados["guardas"])
    if faltam and agora < antes and not parcial:
        return 1, [
            "RECUSADO: esta rodada julgou %d guardas e a tabela publicada tem "
            "%d." % (agora, antes),
            "  %d das %d entradas do catálogo ficariam de fora, e a página "
            "nao diria que elas existem sem estas linhas." % (len(faltam),
                                                              len(guardas)),
            "  A rodada nao cobre o catalogo: nao e' aposentadoria, e' corrida "
            "curta. Se for mesmo para publicar a menor, escreva `--parcial` --",
            "  a pagina sai com o aviso e com os %d nomes." % len(faltam),
        ]
    cabeca = txt.split(INICIO)[0]
    cauda = txt.split(FIM)[1]
    alvo.write_text("%s%s\n%s\n%s%s" % (cabeca, INICIO, bloco, FIM, cauda),
                    encoding="utf-8")
    recados = ["%s: %d guardas" % (alvo, agora)]
    # Gerador que faz menos do que o nome dele promete TEM de dizer que fez
    # menos, e sob um cabecalho que ninguem confunde com exito: o
    # `pagina-dos-pedidos.py` imprimia tres linhas de sucesso e pulava um
    # painel, e tres paineis ficaram atrasados sem um digito digitado.
    if faltam:
        recados.append("  ATENCAO -- esta rodada julgou %d das %d entradas do "
                       "catalogo." % (agora, len(guardas)))
        recados.append("  as %d que ela NAO julgou (a pagina as nomeia, sob o "
                       "mesmo aviso):" % len(faltam))
        recados += ["    - %s" % i for i in faltam]
    else:
        recados.append("  a rodada julgou o catalogo inteiro (%d de %d)"
                       % (agora, len(guardas)))
    return 0, recados


# ------------------------------------------------------------- AUTOTESTE
#
# Prova real nos dois sentidos, sem provador e sem `cargo`: cada caso repoe o
# defeito e confere que a recusa acontece, e confere tambem que ela NAO
# acontece onde seria estrago. O segundo sentido e o que importa aqui -- uma
# recusa que tambem barrasse a aposentadoria seria pior que recusa nenhuma,
# porque a saida dela seria desligar a conferencia.

def _falso(ids, quando="2026-01-01 00:00"):
    return {"quando": quando,
            "guardas": [{"id": i, "titulo": "t " + i, "veredito": "PROVADA",
                         "segundos": 1.0, "notas": []} for i in ids]}


def _catalogo_falso(n):
    return [{"id": "g%02d" % i, "titulo": "titulo %02d" % i, "caem": ["t%d" % i]}
            for i in range(n)]


def _arquivo(tmp, bloco):
    p = tmp / "TESTES.md"
    p.write_text("antes\n%s\n%s\n%s\ndepois\n" % (INICIO, bloco, FIM),
                 encoding="utf-8")
    return p


def autoteste():
    import tempfile
    falhas = []

    def conferir(nome, cond, detalhe=""):
        print("   %s  %s%s" % ("ok  " if cond else "FALHOU", nome,
                               "" if cond else "  -- " + detalhe))
        if not cond:
            falhas.append(nome)

    with tempfile.TemporaryDirectory() as d:
        tmp = pathlib.Path(d)
        cat = _catalogo_falso(160)
        ids = [g["id"] for g in cat]

        # 1. O DEFEITO REPOSTO: a corrida de um `--so` chegando por cima de uma
        #    tabela de 143. Sem a recusa, ela troca 143 vereditos por 9.
        grande = _falso(ids[:143])
        alvo = _arquivo(tmp, tabela(grande, cat))
        antes_txt = alvo.read_text(encoding="utf-8")
        conferir("a pagina de partida publica 143", publicadas(antes_txt) == 143,
                 str(publicadas(antes_txt)))
        codigo, recados = publicar(alvo, _falso(ids[:9]), False, cat)
        conferir("um `--so` de 9 sobre 143 e' RECUSADO", codigo == 1,
                 " / ".join(recados))
        conferir("a pagina nao mudou um byte na recusa",
                 alvo.read_text(encoding="utf-8") == antes_txt)

        # 2. O mesmo `--so`, com o `--parcial` escrito: publica, e a pagina
        #    NOMEIA as 151 -- a escolha e' de quem digitou, e nao e' silenciosa.
        codigo, recados = publicar(alvo, _falso(ids[:9]), True, cat)
        depois = alvo.read_text(encoding="utf-8")
        conferir("com `--parcial` ele publica", codigo == 0, " / ".join(recados))
        conferir("e a pagina encolheu para 9", publicadas(depois) == 9)
        conferir("e nomeia as 151 que ficaram sem veredito",
                 all(("`%s`" % i) in depois for i in ids[9:]))
        conferir("e a linha de resumo diz «de 160»",
                 "9 das 160 guardas do catálogo" in depois)
        conferir("e o recado impresso NAO e' linha de exito",
                 any("ATENCAO" in r for r in recados))

        # 3. A APOSENTADORIA: o catalogo encolheu, a corrida cobre o catalogo
        #    inteiro e MESMO ASSIM traz menos linhas que a publicada. Tem de
        #    passar sem `--parcial`, senao aposentar vira parada permanente.
        alvo = _arquivo(tmp, tabela(grande, cat))
        cat_menor = cat[:142]
        codigo, recados = publicar(alvo, _falso([g["id"] for g in cat_menor]),
                                   False, cat_menor)
        conferir("corrida INTEIRA menor que a publicada passa sem --parcial",
                 codigo == 0, " / ".join(recados))
        conferir("e a pagina nao ganha aviso nenhum",
                 "NÃO julgou" not in alvo.read_text(encoding="utf-8"))
        conferir("e o recado diz que cobriu o catalogo inteiro",
                 any("catalogo inteiro" in r for r in recados))

        # 4. O caso de TODO DIA: rodada parcial que nao encolhe -- republicar a
        #    corrida de ontem depois de o catalogo crescer. Passa sem
        #    `--parcial`, porque e' exatamente o conserto.
        alvo = _arquivo(tmp, tabela(grande, cat))
        codigo, recados = publicar(alvo, grande, False, cat)
        texto = alvo.read_text(encoding="utf-8")
        conferir("republicar a mesma corrida passa sem --parcial", codigo == 0,
                 " / ".join(recados))
        conferir("e a pagina nomeia as 17 que faltam",
                 all(("`%s`" % i) in texto for i in ids[143:]))
        conferir("e continua com as 143 linhas", publicadas(texto) == 143)

        # 5. Corrida inteira: nenhum aviso, e a frase antiga fica.
        alvo = _arquivo(tmp, tabela(grande, cat))
        codigo, _ = publicar(alvo, _falso(ids), False, cat)
        texto = alvo.read_text(encoding="utf-8")
        conferir("corrida inteira publica e nao inventa aviso",
                 codigo == 0 and "NÃO julgou" not in texto)
        conferir("e a linha de resumo volta a dizer «160 guardas»",
                 "**160 guardas:" in texto)

        # 6. A GUARDA APOSENTADA (pedido `cifra-do-fio-imposta`, 18/09/2026):
        #    a corrida julgou a guarda ANTES dela sair do catalogo, e o
        #    veredito que ela trouxe (PROVADA) descrevia um defeito que nao
        #    existe mais. Catalogo e corrida PROPRIOS deste caso, para nao
        #    herdar o "faltam" dos casos 1-5 e nao depender da APOSENTADAS
        #    de producao (que muda com o tempo).
        cat6 = _catalogo_falso(3)
        corrida6 = _falso(["g00", "g01", "aposentada-de-hoje"])
        motivo6 = "o defeito que ela repunha virou o PRODUTO"
        aposentadas6 = {"aposentada-de-hoje": {"data": "18/09/2026",
                                                "motivo": motivo6}}
        bloco6 = tabela(corrida6, cat6, aposentadas6)
        linha6 = [l for l in bloco6.splitlines()
                  if l.startswith("| `aposentada-de-hoje")][0]
        conferir("a guarda aposentada NAO aparece como provada",
                 "provada" not in linha6, linha6)
        conferir("e aparece marcada como aposentada, com a data",
                 "🪦 aposentada (18/09/2026)" in linha6, linha6)
        conferir("e o motivo aparece publicado, na secao propria",
                 motivo6 in bloco6)
        resumo6 = [l for l in bloco6.splitlines() if l.startswith("**")][0]
        conferir("o resumo conta 1 aposentada e 2 provadas -- nao 3 provadas",
                 "2 provadas" in resumo6 and "1 aposentada" in resumo6,
                 resumo6)

    print("   %s" % ("todos passaram" if not falhas
                     else "FALHOU: " + ", ".join(falhas)))
    return 1 if falhas else 0


def main():
    if "--autoteste" in sys.argv:
        print("=== autoteste do gerador da tabela das guardas (pedido 269) ===")
        return autoteste()
    entrada = [a for a in sys.argv[1:] if not a.startswith("--")]
    if not entrada:
        raise SystemExit(__doc__)
    dados = json.loads(pathlib.Path(entrada[0]).read_text(encoding="utf-8"))
    if "--so-medir" in sys.argv:
        print(tabela(dados))
        return 0
    codigo, recados = publicar(ALVO, dados, "--parcial" in sys.argv)
    for r in recados:
        print(r)
    return codigo


if __name__ == "__main__":
    sys.exit(main())

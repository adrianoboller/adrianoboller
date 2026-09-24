#!/usr/bin/env python3
"""Le os `cognicao_*.md`, nunca escreve neles, e gera AVOID.md e REUSE.md.

    python3 docs/cognicao/avoid-e-reuse.py             gera as duas paginas, mostra a contagem por estado
    python3 docs/cognicao/avoid-e-reuse.py --catraca   confere (a)(b)(c) abaixo, sem escrever nada
    python3 docs/cognicao/avoid-e-reuse.py --autoteste prova real da propria regua, em memoria
    python3 docs/cognicao/avoid-e-reuse.py --raiz DIR  usa DIR em vez desta pasta (prova em copia)
    python3 docs/cognicao/avoid-e-reuse.py --git DIR   roda os comandos git em DIR (arvore sem .git)
    PHXSQL_GIT_DIR=DIR python3 ... --catraca            o mesmo, por variavel de ambiente

Por que ele existe (pedido do dono, 24/09/2026)
------------------------------------------------
A petrea nova do `CLAUDE.md` ("aprendizado PENDENTE nao vira FRUTIFERO sem
evidencia") probe promocao automatica: toda cognicao nasce PENDENTE, e so'
muda de estado com prova ESCRITA NO PROPRIO ARQUIVO. Este script e' o
extrator que a petrea pede -- ele LE os `cognicao_*.md` de
`docs/cognicao/LEIA-ME.md` e monta duas paginas, `AVOID.md` (os
INFRUTIFEROS, para nao repetir) e `REUSE.md` (os FRUTIFEROS, para aplicar de
novo) -- e a CATRACA que reprova quem alega um estado sem a prova que o
estado exige.

O que ele NUNCA faz: promover uma cognicao. Nenhum modo deste arquivo
escreve dentro de um `cognicao_*.md` -- so' le. Quem muda o estado de uma
cognicao e' o dono (ou quem ele autorizar), a mao, no proprio arquivo.

O formato dos quatro campos, com exemplos e o porque de cada exigencia, esta
em `docs/cognicao/LEIA-ME.md` -- este comentario nao o repete por inteiro
para as duas fontes nao poderem divergir (a mesma razao por que o extrator
nao digita a lista que gera).

Como ele confere uma referencia de evidencia, sem lista digitada
------------------------------------------------------------------
Uma referencia entre crases e' de UM dos tres tipos, decidido pelo formato do
proprio texto (nunca por uma lista de exemplos):

* hexadecimal puro (7 a 40 caracteres) -- commit; confere com
  `git cat-file -e <hash>^{commit}` (subprocesso, no proprio repositorio);
* contem `::` -- teste. Se a parte antes do ULTIMO `::` termina em `.rs`,
  e' `arquivo.rs::nome`: confere o ARQUIVO citado e o `fn nome` dentro dele.
  Senao e' `crate::modulo::nome`: confere so' o NOME, contra o conjunto de
  todo `#[test]` do repositorio -- MESMO motor que o `bancada/catracas/
  todas.py` usa para casar `TETO_*` com teste (`_testes_do_arquivo`,
  reaproveitado por importacao direta do arquivo dele: e' a mesma pergunta,
  "qual e' o nome de um teste dado o corpo do arquivo?", e a lei do dono de
  23/09/2026 probe repetir a decisao em dois lugares);
* qualquer outra coisa -- arquivo de medicao; confere que o CAMINHO existe,
  relativo a raiz do projeto (`phxsql/`) ou a raiz do repositorio (um nivel
  acima, onde tambem cabe `phxsql/bancada/...`).

So' precisa UMA referencia valida por Evidencia (a petrea pede "pelo menos
uma"); as outras podem ate' estar erradas -- mas SE so' houver uma e ela for
inventada, a cognicao reprova.

Como ele acha o cabecalho, sem confundir com o corpo
-------------------------------------------------------
Os quatro campos (`Estado`, `Evidencia`, `Causa`, `Prevencao`) so' contam
quando aparecem no CABECALHO -- da linha depois do titulo ate' a primeira
linha que comeca com `## ` (a primeira secao numerada). Um texto de corpo que
mencione a palavra "estado" (ha varios, em prosa) nunca casa, porque o crivo
exige o padrao em negrito `**Nome:** valor`, que nenhum dos 303 arquivos de
hoje usa (conferido por grep antes de escrever este arquivo).

A catraca, e por que ela roda o autoteste antes de medir
------------------------------------------------------------
`--catraca` NUNCA escreve: ele gera as duas paginas EM MEMORIA e compara
contra o que esta no disco (item (c)); quem precisa do arquivo atualizado
roda o modo padrao (sem flag), que escreve de verdade. Antes de medir
qualquer coisa ele roda `autoteste(silencioso=True)` -- regua que perdeu uma
parte do crivo mediria zero problemas na arvore sa, e esse zero e' identico
ao zero de uma arvore realmente sem problema (o mesmo cuidado do
`debug-com-segredo.py`, que motivou este molde).

Arvore sem `.git` nao finge que confere commit (defeito achado pelo integrador, 24/09/2026)
----------------------------------------------------------------------------------------------
O integrador conferiu esta regua na ARVORE EXATA do commit -- uma copia sem
`.git` (`git archive HEAD phxsql | tar -x`), a mesma que o provador de
guardas usa. La', o caso de autoteste que precisa do HEAD real (`__HEAD__`)
nao tinha como confirmar nada (nao ha `git` ali) e o autoteste inteiro
FALHAVA -- e a catraca, por cautela ("regua que perdeu uma parte do crivo"),
recusava confiar em QUALQUER numero e reprovava tudo, inclusive uma copia com
evidencia INVENTADA, pelo motivo ERRADO: caiu no autoteste, nao na evidencia.
E' o proprio "teste que passa por engano", ao contrario -- aqui um REPROVA por
engano, escondendo o motivo real.

O conserto tem duas pernas:

* `existe_commit()` so' tenta o `git` se `git_disponivel()` disser que ha um
  repositorio em `GIT_CWD` (a raiz do projeto por padrao, `--git DIR` ou
  `PHXSQL_GIT_DIR` para apontar para outro -- o `todas.py` nao muda, porque
  ele so' passa `--catraca`; a variavel de ambiente e' o que atravessa esse
  subprocesso sem precisar editar o chamador). Sem `git` ali, um commit conta
  como "nao conferivel aqui" -- NAO como "existe" nem como silencio: o
  FRUTIFERO cuja UNICA evidencia e' um hash reprova, porque a petrea proibe
  promover sem evidencia VALIDADA, e "nao consegui checar" nao e' validacao.
  Teste e arquivo continuam conferidos do mesmo jeito (nao dependem de git).
* O caso de autoteste que PRECISA de HEAD real (`requer_git=True`) vira
  PULADO -- nem ok, nem FALHOU -- quando `git_disponivel()` e' falso, e isso
  NAO derruba o autoteste como um todo: os outros 15 continuam rodando e
  decidindo se a regua confia em si mesma. Um segundo caso, direto (fora do
  `caso()`/`Cognicao`), forca `git_disponivel()` para os dois lados e confere
  `existe_commit()` sem depender do ambiente ter `.git` ou nao -- prova o
  proprio ramo "sem git" mesmo quando quem roda o autoteste tem git disponivel.
"""
import difflib
import importlib.util
import os
import re
import subprocess
import sys
import unicodedata
from pathlib import Path

AQUI = Path(__file__).resolve().parent  # docs/cognicao
RAIZ_PHXSQL = AQUI.parent.parent  # phxsql/ -- mesma conta do bancada/catracas/todas.py
RAIZ_REPO = RAIZ_PHXSQL.parent  # um nivel acima -- onde tambem cabe "phxsql/..." citado por inteiro
EU = Path(__file__).resolve().relative_to(RAIZ_PHXSQL)

TETO_FRUTIFERO_SEM_EVIDENCIA_VALIDA = 0
TETO_INFRUTIFERO_SEM_CAUSA_OU_PREVENCAO = 0
TETO_ESTADO_DESCONHECIDO = 0

ESTADOS_VALIDOS = ("PENDENTE", "FRUTIFERO", "INFRUTIFERO")

NOME_AVOID = "AVOID.md"
NOME_REUSE = "REUSE.md"


def _git_dir_configurado():
    """`--git DIR` (linha de comando) ou `PHXSQL_GIT_DIR` (ambiente) -- onde
    RODAR os comandos git. A variavel existe porque o `bancada/catracas/
    todas.py` chama esta regua so' com `--catraca` (sem flag extra) e NAO
    muda por causa disto; a variavel de ambiente e' o que atravessa esse
    subprocesso sem editar o chamador. Sem nenhum dos dois, cai na raiz do
    proprio projeto -- o comportamento de sempre, quando ha `.git` ali."""
    if "--git" in sys.argv:
        return Path(sys.argv[sys.argv.index("--git") + 1]).resolve()
    var = os.environ.get("PHXSQL_GIT_DIR")
    if var:
        return Path(var).resolve()
    return None


GIT_CWD = _git_dir_configurado() or RAIZ_PHXSQL

_GIT_DISPONIVEL = None


def git_disponivel():
    """So' roda o `git` uma vez por processo -- uma arvore extraida por
    `git archive` (sem `.git`) faz TODA chamada de `git` falhar do mesmo
    jeito, entao perguntar de novo a cada referencia so' custaria tempo."""
    global _GIT_DISPONIVEL
    if _GIT_DISPONIVEL is None:
        r = subprocess.run(
            ["git", "rev-parse", "--is-inside-work-tree"],
            cwd=GIT_CWD, capture_output=True, text=True,
        )
        _GIT_DISPONIVEL = (r.returncode == 0 and r.stdout.strip() == "true")
    return _GIT_DISPONIVEL


# ---------------------------------------------------------------------------
# O motor de teste do `bancada/catracas/todas.py`, reaproveitado por
# importacao -- nunca reescrito aqui (lei do dono, 23/09/2026: funcao e
# comando nao se duplicam, vem do mesmo motor).
# ---------------------------------------------------------------------------

def _carregar_motor_de_testes():
    caminho = RAIZ_PHXSQL / "bancada" / "catracas" / "todas.py"
    spec = importlib.util.spec_from_file_location("_catracas_todas_reuso", caminho)
    modulo = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(modulo)
    return modulo


_MOTOR = None


def motor():
    global _MOTOR
    if _MOTOR is None:
        _MOTOR = _carregar_motor_de_testes()
    return _MOTOR


# ---------------------------------------------------------------------------
# Leitura do cabecalho: campos `**Nome:** valor`, com continuacao.
# ---------------------------------------------------------------------------

RE_CAMPO = re.compile(r"^\*\*([^*:]+):\*\*\s*(.*)$")


def tirar_acento(s):
    """Maiusculo e sem diacritico -- so para COMPARAR rotulo/estado, nunca
    para o texto que vai para as paginas geradas (esse fica como foi escrito)."""
    nfkd = unicodedata.normalize("NFKD", s)
    return "".join(c for c in nfkd if not unicodedata.combining(c)).upper().strip()


def linhas_do_cabecalho(linhas):
    """Da linha 1 (depois do titulo, linha 0) ate a primeira `## ` -- exclusive."""
    fim = len(linhas)
    for i in range(1, len(linhas)):
        if linhas[i].startswith("## "):
            fim = i
            break
    return linhas[1:fim]


def extrair_campos(linhas_cabecalho):
    """`{"ESTADO": "...", "EVIDENCIA": "...", ...}` -- chave sem acento (so
    para achar o campo certo), valor como foi escrito (com acento e crase)."""
    campos = {}
    i, n = 0, len(linhas_cabecalho)
    while i < n:
        m = RE_CAMPO.match(linhas_cabecalho[i].strip())
        if not m:
            i += 1
            continue
        chave = tirar_acento(m.group(1))
        partes = [m.group(2).strip()]
        i += 1
        while i < n:
            s = linhas_cabecalho[i].strip()
            if not s or RE_CAMPO.match(s):
                break
            partes.append(s)
            i += 1
        campos[chave] = " ".join(p for p in partes if p).strip()
    return campos


# ---------------------------------------------------------------------------
# As referencias de evidencia: tipo pelo FORMATO, nunca por lista de exemplos.
# ---------------------------------------------------------------------------

RE_BACKTICK = re.compile(r"`([^`]+)`")
RE_HASH = re.compile(r"^[0-9a-fA-F]{7,40}$")


def referencias_de(texto_evidencia):
    """As referencias entre crases; sem nenhuma crase, cai para separar por
    `;`/quebra de linha -- so para nao reprovar por falta de crase quando o
    texto ja e' um caminho/hash isolado."""
    achadas = [a.strip() for a in RE_BACKTICK.findall(texto_evidencia) if a.strip()]
    if achadas:
        return achadas
    return [p.strip() for p in re.split(r"[;\n]", texto_evidencia) if p.strip()]


def tipo_da_referencia(ref):
    if RE_HASH.match(ref):
        return "commit"
    if "::" in ref:
        return "teste"
    return "arquivo"


def existe_commit(hash_texto):
    """`False` tanto para "conferi e nao achei" quanto para "nao tenho como
    conferir" -- a diferenca dos dois (importa para a MENSAGEM, nao para o
    booleano) mora em `validar_referencia()`, que e' quem sabe qual dos dois
    aconteceu."""
    if not git_disponivel():
        return False
    r = subprocess.run(
        ["git", "cat-file", "-e", f"{hash_texto}^{{commit}}"],
        cwd=GIT_CWD, capture_output=True,
    )
    return r.returncode == 0


def _resolve_em_alguma_raiz(caminho_texto):
    for base in (RAIZ_PHXSQL, RAIZ_REPO):
        candidato = (base / caminho_texto)
        if candidato.is_file():
            return candidato
    return None


_CACHE_NOMES_DE_TESTE = None


def nomes_de_teste_no_repo():
    """Todo nome de `#[test]` em `crates/**/*.rs` -- calculado so' quando
    alguma referencia `crate::modulo::nome` precisa dele (a maioria das
    corridas de hoje, com zero FRUTIFERO, nunca paga este custo)."""
    global _CACHE_NOMES_DE_TESTE
    if _CACHE_NOMES_DE_TESTE is None:
        nomes = set()
        for arq in sorted(RAIZ_PHXSQL.glob("crates/**/*.rs")):
            texto = arq.read_text(encoding="utf-8", errors="replace")
            for nome, _corpo in motor()._testes_do_arquivo(texto):
                nomes.add(nome)
        _CACHE_NOMES_DE_TESTE = nomes
    return _CACHE_NOMES_DE_TESTE


def existe_teste(ref):
    caminho_texto, _, nome = ref.rpartition("::")
    nome = nome.strip()
    if not nome:
        return False
    if caminho_texto.strip().endswith(".rs"):
        arq = _resolve_em_alguma_raiz(caminho_texto.strip())
        if arq is None:
            return False
        texto = arq.read_text(encoding="utf-8", errors="replace")
        return any(n == nome for n, _c in motor()._testes_do_arquivo(texto))
    return nome in nomes_de_teste_no_repo()


def existe_arquivo(ref):
    return _resolve_em_alguma_raiz(ref) is not None


def validar_referencia(ref):
    """`(tipo, ok, motivo)` -- `motivo` so' importa quando `ok` e' False, e
    distingue "conferi e nao achei" de "nao tenho como conferir aqui" (esse
    segundo so' existe para commit, porque teste e arquivo se conferem pela
    ARVORE, que existe em toda copia, com ou sem `.git`)."""
    tipo = tipo_da_referencia(ref)
    if tipo == "commit":
        if not git_disponivel():
            return tipo, False, ("nao conferivel aqui (sem acesso a git -- "
                                  "use --git DIR ou PHXSQL_GIT_DIR)")
        ok = existe_commit(ref)
        return tipo, ok, ("" if ok else "commit nao encontrado no git")
    if tipo == "teste":
        ok = existe_teste(ref)
        return tipo, ok, ("" if ok else "teste nao encontrado no repositorio")
    ok = existe_arquivo(ref)
    return tipo, ok, ("" if ok else "arquivo nao encontrado")


# ---------------------------------------------------------------------------
# Uma cognicao analisada.
# ---------------------------------------------------------------------------

class Cognicao:
    def __init__(self, nome_arquivo, titulo, campos):
        self.nome_arquivo = nome_arquivo
        self.titulo = titulo
        bruto = campos.get("ESTADO")
        if bruto is None:
            self.estado = "PENDENTE"
            self.estado_desconhecido = None
        else:
            norm = tirar_acento(bruto)
            if norm in ESTADOS_VALIDOS:
                self.estado = norm
                self.estado_desconhecido = None
            else:
                self.estado = None
                self.estado_desconhecido = bruto

        self.evidencia_bruta = campos.get("EVIDENCIA", "")
        self.causa = campos.get("CAUSA", "").strip()
        self.prevencao = campos.get("PREVENCAO", "").strip()

        self.referencias = referencias_de(self.evidencia_bruta) if self.evidencia_bruta else []
        self.avaliacoes = [(r,) + validar_referencia(r) for r in self.referencias]
        self.evidencia_valida = any(ok for _, _, ok, _ in self.avaliacoes)

    def problemas(self):
        """Motivos de reprovacao desta cognicao para a catraca -- lista vazia
        quando esta em ordem (inclusive todo PENDENTE, que nao exige nada)."""
        p = []
        if self.estado_desconhecido is not None:
            p.append(f"estado desconhecido: {self.estado_desconhecido!r} "
                     "(so PENDENTE, FRUTIFERO ou INFRUTIFERO)")
            return p
        if self.estado == "FRUTIFERO" and not self.evidencia_valida:
            if not self.evidencia_bruta:
                p.append("FRUTIFERO sem **Evidencia:**")
            else:
                ruins = "; ".join(f"`{r}` ({t}: {m})"
                                   for r, t, ok, m in self.avaliacoes if not ok)
                p.append(f"FRUTIFERO com evidencia sem NENHUMA referencia valida: {ruins}")
        if self.estado == "INFRUTIFERO" and not (self.causa and self.prevencao):
            faltando = []
            if not self.causa:
                faltando.append("**Causa:**")
            if not self.prevencao:
                faltando.append("**Prevencao:**")
            p.append("INFRUTIFERO sem " + " e ".join(faltando))
        return p


def analisar_conteudo(nome_arquivo, texto):
    linhas = texto.splitlines()
    titulo = linhas[0].lstrip("#").strip() if linhas else nome_arquivo
    campos = extrair_campos(linhas_do_cabecalho(linhas))
    return Cognicao(nome_arquivo, titulo, campos)


def listar_cognicoes(raiz):
    saida = []
    for arq in sorted(raiz.glob("cognicao_*.md")):
        texto = arq.read_text(encoding="utf-8", errors="replace")
        saida.append(analisar_conteudo(arq.name, texto))
    return saida


def contagem_por_estado(cognicoes):
    c = {"PENDENTE": 0, "FRUTIFERO": 0, "INFRUTIFERO": 0, "invalido": 0}
    for cog in cognicoes:
        if cog.estado is None:
            c["invalido"] += 1
        else:
            c[cog.estado] += 1
    return c


# ---------------------------------------------------------------------------
# As duas paginas geradas -- string determinística a partir da lista.
# ---------------------------------------------------------------------------

CABECALHO_GERADO = (
    "<!-- GERADO por docs/cognicao/avoid-e-reuse.py -- NAO EDITE A MAO.\n"
    "     `--catraca` reprova se este arquivo nao bater com o que o extrator\n"
    "     geraria agora; rode o comando sem flag para atualizar. -->"
)


def gerar_avoid(cognicoes):
    infrutiferos = [c for c in cognicoes if c.estado == "INFRUTIFERO"]
    linhas = [
        "# AVOID -- o que ja falhou aqui, e por que",
        "",
        CABECALHO_GERADO,
        "",
        f"Gerado dos `cognicao_*.md` com `**Estado:** INFRUTIFERO` -- "
        f"{len(infrutiferos)} hoje, de {len(cognicoes)} cognicoes no total.",
        "",
    ]
    if not infrutiferos:
        linhas.append(
            "Nenhuma cognicao esta marcada INFRUTIFERO ainda. A petrea de "
            "24/09/2026 proibe promover PENDENTE sem prova, e a promocao e' "
            "sempre do dono -- nao deste script."
        )
    else:
        for c in infrutiferos:
            linhas += [
                f"## {c.titulo}",
                "",
                f"- Causa: {c.causa}",
                f"- Prevencao: {c.prevencao}",
                f"- Arquivo: [{c.nome_arquivo}]({c.nome_arquivo})",
                "",
            ]
    return "\n".join(linhas).rstrip() + "\n"


def gerar_reuse(cognicoes):
    frutiferos = [c for c in cognicoes if c.estado == "FRUTIFERO"]
    linhas = [
        "# REUSE -- o que ja deu certo aqui, comprovado",
        "",
        CABECALHO_GERADO,
        "",
        f"Gerado dos `cognicao_*.md` com `**Estado:** FRUTIFERO` -- "
        f"{len(frutiferos)} hoje, de {len(cognicoes)} cognicoes no total.",
        "",
    ]
    if not frutiferos:
        linhas.append(
            "Nenhuma cognicao esta marcada FRUTIFERO ainda. A petrea de "
            "24/09/2026 exige evidencia validada para promover, e a "
            "promocao e' sempre do dono -- nao deste script."
        )
    else:
        for c in frutiferos:
            linhas += [
                f"## {c.titulo}",
                "",
                f"- Evidencia: {c.evidencia_bruta}",
                f"- Arquivo: [{c.nome_arquivo}]({c.nome_arquivo})",
                "",
            ]
    return "\n".join(linhas).rstrip() + "\n"


# ---------------------------------------------------------------------------
# Os modos.
# ---------------------------------------------------------------------------

def raiz_pedida(args):
    if "--raiz" in args:
        return Path(args[args.index("--raiz") + 1]).resolve()
    return AQUI


def imprimir_contagem(cognicoes):
    c = contagem_por_estado(cognicoes)
    print(f"=== {len(cognicoes)} cognicao(oes) por estado ===")
    print(f"   PENDENTE:     {c['PENDENTE']}")
    print(f"   FRUTIFERO:    {c['FRUTIFERO']}")
    print(f"   INFRUTIFERO:  {c['INFRUTIFERO']}")
    if c["invalido"]:
        print(f"   invalido:     {c['invalido']}  (Estado presente mas nao e "
              "nenhum dos tres -- corrija a mao)")


def gerar(args):
    raiz = raiz_pedida(args)
    cognicoes = listar_cognicoes(raiz)
    (raiz / NOME_AVOID).write_text(gerar_avoid(cognicoes), encoding="utf-8")
    (raiz / NOME_REUSE).write_text(gerar_reuse(cognicoes), encoding="utf-8")
    imprimir_contagem(cognicoes)
    print(f"\n   escrito: {raiz / NOME_AVOID}")
    print(f"   escrito: {raiz / NOME_REUSE}")
    return 0


def catraca(args):
    print("=== a catraca do estado das cognicoes (avoid/reuse) ===")
    falhas, pulados = autoteste(silencioso=True)
    if falhas:
        print("   AUTOTESTE FALHOU: " + ", ".join(falhas))
        print("   Reprovado: a regua perdeu uma parte do crivo; o numero "
              "abaixo nao vale.")
        return 1
    rodados = N_CASOS - len(pulados)
    extra = f" ({len(pulados)} pulado(s) sem git: {', '.join(pulados)})" if pulados else ""
    print(f"   autoteste: {rodados} caso(s) ok{extra}")
    if git_disponivel():
        print(f"   git: disponivel em {GIT_CWD}")
    else:
        print(f"   git: NAO disponivel em {GIT_CWD} -- evidencia de commit "
              "reprova como \"nao conferivel\"; use --git DIR ou PHXSQL_GIT_DIR")

    raiz = raiz_pedida(args)
    cognicoes = listar_cognicoes(raiz)
    imprimir_contagem(cognicoes)

    ruim = False

    # (a) e (b): cada cognicao que alega um estado sem a prova que ele exige.
    problemas_por_arquivo = [(c.nome_arquivo, c.problemas()) for c in cognicoes]
    n_frutiferos_invalidos = sum(
        1 for c in cognicoes if c.estado == "FRUTIFERO" and not c.evidencia_valida)
    n_infrutiferos_invalidos = sum(
        1 for c in cognicoes if c.estado == "INFRUTIFERO"
        and not (c.causa and c.prevencao))
    n_estado_desconhecido = sum(1 for c in cognicoes if c.estado is None)

    for nome, probs in problemas_por_arquivo:
        for p in probs:
            print(f"   REPROVADO  {nome}  -- {p}")

    if n_frutiferos_invalidos > TETO_FRUTIFERO_SEM_EVIDENCIA_VALIDA:
        print(f"\n   SUBIU  FRUTIFERO sem evidencia valida: "
              f"{n_frutiferos_invalidos} (teto {TETO_FRUTIFERO_SEM_EVIDENCIA_VALIDA})")
        ruim = True
    if n_infrutiferos_invalidos > TETO_INFRUTIFERO_SEM_CAUSA_OU_PREVENCAO:
        print(f"\n   SUBIU  INFRUTIFERO sem causa/prevencao: "
              f"{n_infrutiferos_invalidos} (teto {TETO_INFRUTIFERO_SEM_CAUSA_OU_PREVENCAO})")
        ruim = True
    if n_estado_desconhecido > TETO_ESTADO_DESCONHECIDO:
        print(f"\n   SUBIU  estado desconhecido: {n_estado_desconhecido} "
              f"(teto {TETO_ESTADO_DESCONHECIDO})")
        ruim = True

    # (c) AVOID.md/REUSE.md tem de bater com o que o extrator geraria agora.
    for nome, gerador in ((NOME_AVOID, gerar_avoid), (NOME_REUSE, gerar_reuse)):
        esperado = gerador(cognicoes)
        caminho = raiz / nome
        atual = caminho.read_text(encoding="utf-8") if caminho.is_file() else None
        if atual != esperado:
            print(f"\n   DIVERGE  {nome} no disco != o que o extrator geraria agora")
            if atual is None:
                print(f"      {nome} nao existe -- rode sem flag para criar")
            else:
                diff = list(difflib.unified_diff(
                    atual.splitlines(), esperado.splitlines(),
                    fromfile=f"{nome} (disco)", tofile=f"{nome} (gerado agora)",
                    lineterm="", n=1))
                for linha in diff[:12]:
                    print(f"      {linha}")
            ruim = True

    if ruim:
        print("\n   REPROVADO")
        return 1
    print("\n   ok")
    return 0


# ---------------------------------------------------------------------------
# O autoteste -- casos em memoria, no molde do `debug-com-segredo.py`: cada
# parte do crivo, nos dois sentidos, contra o MESMO `analisar_conteudo` que
# le a arvore de verdade.
# ---------------------------------------------------------------------------

CASOS = []


def caso(nome, texto, estado_esperado, problema_esperado, requer_git=False):
    """`problema_esperado`: True se `problemas()` deve ser NAO vazio.
    `requer_git`: o caso so' pode DAR CERTO com um `.git` de verdade
    acessivel (usa `__HEAD__`) -- sem git, ele fica PULADO, nem ok nem
    FALHOU (o defeito que o integrador achou, 24/09/2026: sem isto, este
    UNICO caso derrubava o autoteste inteiro numa arvore sem `.git`, e a
    catraca reprovava tudo por desconfiar de si mesma, escondendo o motivo
    real de qualquer outra cognicao que estivesse errada)."""
    CASOS.append((nome, texto, estado_esperado, problema_esperado, requer_git))


caso("sem linha Estado vira PENDENTE, sem problema",
     "# Titulo\n\nData: 01/01/2026.\n\n## 1. O que aconteceu\n\ntexto\n",
     "PENDENTE", False)

caso("Estado FRUTIFERO sem Evidencia reprova",
     "# Titulo\n\n**Estado:** FRUTÍFERO\n\n## 1. O que aconteceu\n\ntexto\n",
     "FRUTIFERO", True)

caso("Estado FRUTIFERO com commit HEAD (real) passa",
     "# Titulo\n\n**Estado:** FRUTÍFERO\n\n**Evidência:** `__HEAD__`\n\n## 1.\n",
     "FRUTIFERO", False, requer_git=True)

caso("Estado FRUTIFERO com commit inventado reprova",
     "# Titulo\n\n**Estado:** FRUTÍFERO\n\n**Evidência:** `0000000`\n\n## 1.\n",
     "FRUTIFERO", True)

caso("Estado FRUTIFERO com teste real (nome achado no repo) passa",
     "# Titulo\n\n**Estado:** FRUTÍFERO\n\n**Evidência:** `crate::modulo::__TESTE_REAL__`\n\n## 1.\n",
     "FRUTIFERO", False)

caso("Estado FRUTIFERO com teste inventado reprova",
     "# Titulo\n\n**Estado:** FRUTÍFERO\n\n"
     "**Evidência:** `crate::modulo::este_teste_nao_existe_de_jeito_nenhum_zzz`\n\n## 1.\n",
     "FRUTIFERO", True)

caso("Estado FRUTIFERO com arquivo.rs::nome inventado reprova",
     "# Titulo\n\n**Estado:** FRUTÍFERO\n\n"
     "**Evidência:** `docs/cognicao/isto-nao-existe.rs::nada`\n\n## 1.\n",
     "FRUTIFERO", True)

caso("Estado FRUTIFERO com arquivo de medicao real (LEIA-ME.md) passa",
     "# Titulo\n\n**Estado:** FRUTÍFERO\n\n**Evidência:** `docs/cognicao/LEIA-ME.md`\n\n## 1.\n",
     "FRUTIFERO", False)

caso("Estado FRUTIFERO com arquivo inventado reprova",
     "# Titulo\n\n**Estado:** FRUTÍFERO\n\n"
     "**Evidência:** `docs/cognicao/isto-nao-existe-nunca.json`\n\n## 1.\n",
     "FRUTIFERO", True)

caso("Evidencia com duas referencias, uma invalida e uma valida: passa (basta uma)",
     "# Titulo\n\n**Estado:** FRUTÍFERO\n\n"
     "**Evidência:** `0000000`; `docs/cognicao/LEIA-ME.md`\n\n## 1.\n",
     "FRUTIFERO", False)

caso("Estado INFRUTIFERO com causa e prevencao passa",
     "# Titulo\n\n**Estado:** INFRUTÍFERO\n\n**Causa:** X\n\n**Prevenção:** Y\n\n## 1.\n",
     "INFRUTIFERO", False)

caso("Estado INFRUTIFERO sem Prevencao reprova",
     "# Titulo\n\n**Estado:** INFRUTÍFERO\n\n**Causa:** X\n\n## 1.\n",
     "INFRUTIFERO", True)

caso("Estado INFRUTIFERO sem Causa reprova",
     "# Titulo\n\n**Estado:** INFRUTÍFERO\n\n**Prevenção:** Y\n\n## 1.\n",
     "INFRUTIFERO", True)

caso("Estado com valor desconhecido nao vira PENDENTE por engano -- reprova",
     "# Titulo\n\n**Estado:** TALVEZ\n\n## 1.\n",
     None, True)

caso("campo em varias linhas (continuacao) e' lido inteiro",
     "# Titulo\n\n**Estado:** INFRUTÍFERO\n\n**Causa:** primeira linha\nsegunda linha\n\n"
     "**Prevenção:** unica\n\n## 1.\n",
     "INFRUTIFERO", False)

caso("Estado FRUTIFERO SEM acento no rotulo tambem conta (so' o VALOR compara sem acento)",
     "# Titulo\n\n**Estado:** FRUTIFERO\n\n**Evidência:** `docs/cognicao/LEIA-ME.md`\n\n## 1.\n",
     "FRUTIFERO", False)

N_CASOS = len(CASOS)


def _resolver_marcadores(texto):
    """Os tres marcadores usam dado REAL do repositorio (HEAD, um teste que
    existe), nunca um valor cravado -- para o autoteste nao descolar do
    repositorio no dia em que um teste for renomeado."""
    if "__HEAD__" in texto:
        r = subprocess.run(["git", "rev-parse", "HEAD"], cwd=GIT_CWD,
                            capture_output=True, text=True)
        texto = texto.replace("__HEAD__", r.stdout.strip() or "0000000")
    if "__TESTE_REAL__" in texto:
        nomes = nomes_de_teste_no_repo()
        nome = next(iter(nomes)) if nomes else "nenhum_teste_achado"
        texto = texto.replace("__TESTE_REAL__", nome)
    return texto


def autoteste(silencioso=False):
    """Devolve `(falhas, pulados)`. `pulados` NAO conta como falha -- e' o
    caso que precisa de `.git` de verdade, numa arvore que nao tem um (a
    catraca continua confiando no restante do crivo; so' avisa o que nao deu
    para exercitar aqui)."""
    falhas, pulados = [], []
    for nome, texto, estado_esperado, problema_esperado, requer_git in CASOS:
        if requer_git and not git_disponivel():
            pulados.append(nome)
            if not silencioso:
                print(f"   PULADO  {nome}  -- sem acesso a git em {GIT_CWD} "
                      "(use --git DIR ou PHXSQL_GIT_DIR)")
            continue
        cog = analisar_conteudo("caso.md", _resolver_marcadores(texto))
        tem_problema = bool(cog.problemas())
        ok = (cog.estado == estado_esperado) and (tem_problema == problema_esperado)
        if not silencioso:
            detalhe = (f"estado={cog.estado} (esperava {estado_esperado}), "
                       f"problema={tem_problema} (esperava {problema_esperado}), "
                       f"{cog.problemas()}")
            print("   %s  %s%s" % ("ok  " if ok else "FALHOU", nome,
                                   "" if ok else "  -- " + detalhe))
        if not ok:
            falhas.append(nome)

    # o ramo "sem git" de `existe_commit()`, provado nos dois sentidos
    # MESMO quando quem roda o autoteste tem `.git` disponivel -- forca a
    # bandeira em vez de depender do ambiente ter ou nao repositorio.
    nome_forcado = "ramo sem-git de existe_commit()/validar_referencia() (forcado)"
    antes = len(falhas)
    global _GIT_DISPONIVEL
    salvo = _GIT_DISPONIVEL
    try:
        _GIT_DISPONIVEL = False
        if existe_commit("1" * 40) is not False:
            falhas.append(nome_forcado)
        _, ok, motivo = validar_referencia("1" * 40)
        if ok or "nao conferivel" not in motivo:
            falhas.append(nome_forcado)
    finally:
        _GIT_DISPONIVEL = salvo
    if not silencioso:
        ok_forcado = len(falhas) == antes
        print("   %s  %s" % ("ok  " if ok_forcado else "FALHOU", nome_forcado))

    # a geracao e' deterministica: rodar duas vezes com a mesma entrada da o
    # mesmo texto -- sem isto, um bug de ordenacao faria (c) oscilar sozinho.
    exemplo = [analisar_conteudo("a.md", "# A\n\n**Estado:** INFRUTÍFERO\n\n"
                                          "**Causa:** c\n\n**Prevenção:** p\n\n## 1.\n")]
    if gerar_avoid(exemplo) != gerar_avoid(exemplo):
        falhas.append("gerar_avoid nao e' deterministico")
    if gerar_reuse(exemplo) != gerar_reuse(exemplo):
        falhas.append("gerar_reuse nao e' deterministico")

    if not silencioso:
        resumo = "todos passaram" if not falhas else "FALHOU: " + ", ".join(falhas)
        if pulados:
            resumo += f"  ({len(pulados)} pulado(s) sem git)"
        print("   %s" % resumo)
    return falhas, pulados


def principal():
    args = sys.argv[1:]
    if "--autoteste" in sys.argv:  # sys.argv e --catraca na MESMA linha em algum ponto do arquivo
        print("=== autoteste do estado das cognicoes (avoid/reuse) ===")
        falhas, _pulados = autoteste()
        return 1 if falhas else 0
    if "--catraca" in sys.argv:  # e' esta linha que o bancada/catracas/todas.py acha sozinho
        return catraca(args)
    return gerar(args)


if __name__ == "__main__":
    sys.exit(principal())

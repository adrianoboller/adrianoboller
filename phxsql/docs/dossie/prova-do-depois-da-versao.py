#!/usr/bin/env python3
"""Prova real do estado `⏸` (depois da versão) no `pagina-dos-pedidos.py`.

    python3 docs/dossie/prova-do-depois-da-versao.py

Decisão do dono, 24/09/2026: a versão 0.19 congela escopo, e um achado de
revisão que não é defeito ativo nem bloqueia a entrega vira `⏸` no
`PENDENCIAS.md` -- visível, mas fora da conta do que falta para a versão.
Esta prova cobre exatamente as quatro garantias do pedido que criou o estado:

1. Numa CÓPIA do `PENDENCIAS.md` com dois pedidos marcados `⏸`, a contagem
   os mostra como "depois da versão".
2. A PORCENTAGEM «falta» -- (parcial + planejado) / (feito + parcial +
   planejado) -- os EXCLUI dos dois lados da conta.
3. A página (o `corpo()` de uma faixa que os contém) os MOSTRA, com pino e
   rótulo próprios.
4. Com o suporte REPOSTO AUSENTE -- o `pagina-dos-pedidos.py` de ANTES desta
   mudança, lido do próprio `git show 257f854:...` -- o leitor PARA com o
   "estado desconhecido", a mesma guarda que já protegia o pedido 150.

E a quinta garantia, a mais fácil de quebrar sem querer: SEM nenhum `⏸`, o
gerador novo produz exatamente os MESMOS bytes que o gerador de HOJE (o de
`git show 257f854:...`) -- contagem, painel do dossiê e as páginas inteiras,
uma por uma. A mudança não pode alterar nada enquanto ninguém usa o estado
novo -- é a mesma disciplina de "guarda nova entra pedida, não imposta".

Teste que passa por engano é pior que teste que falta: por isso o item 4 usa
o código de ANTES de verdade (via `git show`), não uma reconstrução manual do
bug -- reconstruir errado já custou uma lição nesta mesma pasta
(`prova-do-leitor-de-pedidos.py`, comentário do `REPOSTOS[2]`).
"""

import filecmp
import importlib.util
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]                       # docs/dossie -> docs -> phxsql
CAMINHO_MODULO = RAIZ / "docs" / "dossie" / "pagina-dos-pedidos.py"
FONTE_REAL = RAIZ / "docs" / "PENDENCIAS.md"

# O «antes» e' um commit FIXO, o ultimo sem o `⏸`, e nao o `HEAD`. Com o
# `HEAD` a prova so' valia ate o proprio commit dela: dali em diante o `HEAD`
# ja traz o conserto, o «suporte reposto ausente» deixa de estar ausente, e
# a prova passaria a acusar o conserto de ser o defeito.
ANTES = "257f854"

LINHA_DEPOIS = re.compile(r"^\| ⏸ \|", re.M)


def sem_depois(texto_md: str) -> str:
    """O `PENDENCIAS.md` real com cada `⏸` de volta a `☐` -- o mundo SEM o
    quarto estado, que e' onde a garantia «sem `⏸` nada muda» se confere.
    Desde 24/09/2026 o arquivo real ja traz `⏸` de verdade (a triagem do
    escopo da 0.19), entao le-lo cru deixou de ser esse mundo."""
    return LINHA_DEPOIS.sub("| ☐ |", texto_md)

DOSSIE_FALSO = """<html><body>
<!-- pedidos:inicio (gerado por docs/dossie/pagina-dos-pedidos.py) -->
<!-- pedidos:fim -->
</body></html>
"""


def raiz_do_git() -> pathlib.Path:
    r = subprocess.run(["git", "rev-parse", "--show-toplevel"],
                       capture_output=True, text=True, cwd=RAIZ, check=True)
    return pathlib.Path(r.stdout.strip())


def caminho_relativo_ao_git(caminho: pathlib.Path, raiz_git: pathlib.Path) -> str:
    return str(caminho.resolve().relative_to(raiz_git))


def carregar(caminho: pathlib.Path, apelido: str, pasta_do_irmao: pathlib.Path):
    """Importa o modulo pelo CAMINHO, com o `sys.path` apontando para a pasta
    de quem tem o `dossie_da_pasta.py` -- o modulo faz
    `from dossie_da_pasta import achar_o_dossie` na carga, e sem isso a
    importacao do modulo VELHO (rodando de um arquivo temporario) quebraria
    por um motivo que nada tem a ver com o que esta prova testa."""
    spec = importlib.util.spec_from_file_location(apelido, caminho)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[apelido] = mod
    antigo = list(sys.path)
    sys.path.insert(0, str(pasta_do_irmao))
    try:
        spec.loader.exec_module(mod)
    finally:
        sys.path[:] = antigo
    return mod


def carregar_versao_de_antes(raiz_git: pathlib.Path, rel: str, pasta_tmp: pathlib.Path):
    """O `pagina-dos-pedidos.py` de ANTES desta mudanca -- literalmente o
    `git show ANTES:...`, nao uma reconstrucao manual do estado anterior.
    E' o "suporte reposto ausente" do pedido: o `⏸` simplesmente nao existe
    nesse texto, porque esta sessao ainda nao commitou nada."""
    r = subprocess.run(["git", "show", f"{ANTES}:{rel}"],
                       capture_output=True, text=True, cwd=raiz_git)
    if r.returncode != 0:
        raise SystemExit(f"nao consegui ler {ANTES}:{rel} -- {r.stderr.strip()}")
    if not r.stdout.strip():
        raise SystemExit(f"{ANTES}:{rel} veio vazio -- caminho errado?")
    shutil.copy(AQUI / "dossie_da_pasta.py", pasta_tmp / "dossie_da_pasta.py")
    alvo = pasta_tmp / "pagina-dos-pedidos.py"
    alvo.write_text(r.stdout, encoding="utf-8")
    return carregar(alvo, "g_velho", pasta_tmp)


LINHA_PLANEJADO = re.compile(r"^\| ☐ \| (\d+) \|")


def marcar_dois_como_depois(texto_md: str) -> tuple:
    """Copia o texto do PENDENCIAS.md trocando os DOIS primeiros `☐` por `⏸`.
    Devolve (texto_novo, [n1, n2]) -- os numeros dos pedidos escolhidos, para
    as conferencias de baixo saberem qual faixa procurar."""
    escolhidos = []
    linhas = texto_md.split("\n")
    for i, l in enumerate(linhas):
        m = LINHA_PLANEJADO.match(l)
        if m:
            linhas[i] = "| ⏸ " + l[len("| ☐ "):]
            escolhidos.append(int(m.group(1)))
            if len(escolhidos) == 2:
                break
    if len(escolhidos) != 2:
        raise SystemExit("nao achei dois pedidos '☐' no PENDENCIAS.md real "
                          "para marcar como '⏸' -- a prova precisa de dois")
    return "\n".join(linhas), escolhidos


def bloco_entre(texto: str, abre: str, fecha: str) -> str:
    i, j = texto.find(abre), texto.find(fecha)
    if i < 0 or j < 0:
        raise SystemExit(f"marca {abre!r}/{fecha!r} nao encontrada")
    return texto[i:j + len(fecha)]


def comparar_pastas(a: pathlib.Path, b: pathlib.Path) -> list:
    """Devolve a lista de diferencas entre duas pastas de paginas geradas --
    vazia significa byte a byte identicas, arquivo a arquivo."""
    arquivos_a = sorted(p.name for p in a.glob("*.html"))
    arquivos_b = sorted(p.name for p in b.glob("*.html"))
    diffs = []
    if arquivos_a != arquivos_b:
        diffs.append(f"nomes de arquivo diferentes: {arquivos_a} x {arquivos_b}")
        return diffs
    for nome in arquivos_a:
        if not filecmp.cmp(a / nome, b / nome, shallow=False):
            diffs.append(f"{nome}: bytes diferentes")
    return diffs


def main() -> int:
    falhas = []
    raiz_git = raiz_do_git()
    rel = caminho_relativo_ao_git(CAMINHO_MODULO, raiz_git)

    tmp = pathlib.Path(tempfile.mkdtemp(prefix="prova484-"))
    try:
        g_novo = carregar(CAMINHO_MODULO, "g_novo_484", AQUI)
        pasta_velho_src = tmp / "velho"
        pasta_velho_src.mkdir()
        g_velho = carregar_versao_de_antes(raiz_git, rel, pasta_velho_src)

        base = sem_depois(FONTE_REAL.read_text(encoding="utf-8"))

        # ================================================= 1, 2 e 3: com `⏸`
        com_depois, escolhidos = marcar_dois_como_depois(base)
        copia_com_depois = tmp / "com_depois.md"
        copia_com_depois.write_text(com_depois, encoding="utf-8")

        g_novo.FONTE = copia_com_depois
        itens = g_novo.ler()
        contas = g_novo.contas_de(itens)

        if contas["depois"] != 2:
            falhas.append(f"contagem nao marcou os dois '⏸' -- contas={contas}")
        else:
            print(f"  ok    contagem: 2 pedidos ({escolhidos}) viram "
                  f"'depois da versão' (contas={contas})")

        # a porcentagem tem de EXCLUIR -- prova diferencial: se o denominador
        # incluisse os dois `⏸` (como se ainda fossem planejado), o numero
        # seria OUTRO. Comparar os dois de proposito, nao so' reafirmar a
        # formula: reafirmar a formula so' prova que copiei a conta certo,
        # nao que ela exclui de verdade.
        pct = g_novo.percentual_falta(contas)
        total_sem = contas["feito"] + contas["parcial"] + contas["planejado"]
        total_com = total_sem + contas["depois"]
        pct_se_contasse = (100.0 * (contas["parcial"] + contas["planejado"])
                           / total_com) if total_com else None
        pct_esperado = (100.0 * (contas["parcial"] + contas["planejado"])
                        / total_sem) if total_sem else None
        if pct != pct_esperado:
            falhas.append(f"percentual_falta nao bate com a formula do dono: "
                          f"{pct} != {pct_esperado}")
        elif pct == pct_se_contasse:
            falhas.append("percentual_falta deu o MESMO numero que contando "
                          "os '⏸' no denominador -- ele nao esta excluindo "
                          f"nada de verdade (pct={pct})")
        else:
            print(f"  ok    porcentagem exclui os '⏸': {pct:.2f}% falta "
                  f"(seria {pct_se_contasse:.2f}% se os '⏸' contassem) -- "
                  f"e os {contas['depois']} continuam visiveis ao lado, "
                  "nunca somem de `contas['depois']`")

        # gravar_contagem: a frase tem de nomear "depois da versão" e o 2.
        g_novo.gravar_contagem(itens)
        bloco = bloco_entre(copia_com_depois.read_text(encoding="utf-8"),
                            g_novo.ABRE_C, g_novo.FECHA_C)
        if "depois da versão" not in bloco or "2 depois da versão" not in bloco:
            falhas.append(f"a linha de contagem nao nomeou os '⏸': {bloco!r}")
        else:
            print(f"  ok    linha gravada no PENDENCIAS.md: {bloco.splitlines()[1]}")

        # a pagina: renderiza as faixas de verdade (pasta temporaria, nunca a
        # pasta real) e confere que os dois pedidos aparecem com pino/rotulo
        # proprios em algum HTML gerado.
        pasta_paginas = tmp / "paginas_com_depois"
        pasta_paginas.mkdir()
        g_novo.PASTA_SAIDA = pasta_paginas
        faixas, _saidas = g_novo.gravar_faixas(itens)
        achou_os_dois = set()
        for arquivo in pasta_paginas.glob("*.html"):
            txt = arquivo.read_text(encoding="utf-8")
            if 'class="pino depois"' in txt and "Depois da versão" in txt:
                for n in escolhidos:
                    if f'<td class="n mono">{n}</td>' in txt:
                        achou_os_dois.add(n)
        if achou_os_dois != set(escolhidos):
            falhas.append(f"a pagina nao mostrou os dois '⏸' esperados -- "
                          f"achou {achou_os_dois}, esperava {set(escolhidos)}")
        else:
            print(f"  ok    a pagina mostra os dois pedidos {escolhidos} "
                  "com pino e rotulo 'Depois da versão'")

        # ============================ 3b: extrair.py usa o MESMO leitor
        # Achado da revisao de QA (pedido 484): `docs/tecnologias/extrair.py`
        # tinha a SUA PROPRIA copia da regex/guarda, e a copia deixava o '⏸'
        # sumir calado -- fere "funcao e comando nao se duplicam". Prova nos
        # dois sentidos, na MESMA copia com dois '⏸': a copia ANTIGA (git
        # show ANTES) perde os dois calada; o conserto de hoje os mostra como
        # 'depois da versao'.
        caminho_extrair = RAIZ / "docs" / "tecnologias" / "extrair.py"
        rel_extrair = caminho_relativo_ao_git(caminho_extrair, raiz_git)
        r_extrair_velho = subprocess.run(
            ["git", "show", f"{ANTES}:{rel_extrair}"],
            capture_output=True, text=True, cwd=raiz_git)
        if r_extrair_velho.returncode != 0 or not r_extrair_velho.stdout.strip():
            falhas.append(f"nao consegui ler {ANTES}:{rel_extrair} -- "
                          f"{r_extrair_velho.stderr.strip()}")
        else:
            pasta_extrair_velho = tmp / "extrair_velho"
            pasta_extrair_velho.mkdir()
            alvo_extrair_velho = pasta_extrair_velho / "extrair.py"
            alvo_extrair_velho.write_text(r_extrair_velho.stdout, encoding="utf-8")
            ex_velho = carregar(alvo_extrair_velho, "ex_velho_484", pasta_extrair_velho)
            # A copia ANTIGA computa `fonte = RAIZ / "docs" / "PENDENCIAS.md"`
            # a cada chamada -- redirecionar `RAIZ` para uma raiz de mentira
            # com o MESMO PENDENCIAS.md com '⏸' basta, sem tocar o real.
            raiz_de_mentira = tmp / "raiz_extrair_velho"
            (raiz_de_mentira / "docs").mkdir(parents=True)
            (raiz_de_mentira / "docs" / "PENDENCIAS.md").write_text(
                com_depois, encoding="utf-8")
            ex_velho.RAIZ = raiz_de_mentira
            itens_extrair_velho = ex_velho.ler_pedidos()

            ex_novo = carregar(caminho_extrair, "ex_novo_484", AQUI)
            # Injeta o modulo dos pedidos JA carregado (FONTE = copia com os
            # dois '⏸') no lugar do importador interno -- mesma tecnica de
            # troca de dependencia que `g_novo.FONTE = copia` usa em cima,
            # so que pela funcao inteira em vez de um atributo.
            ex_novo._importar_pagina_dos_pedidos = lambda: g_novo
            itens_extrair_novo = ex_novo.ler_pedidos()

            if len(itens_extrair_velho) == len(itens) - 2:
                print(f"  ok    extrair.py de ANTES (git {ANTES}) perde os dois "
                      f"'⏸' calado: {len(itens_extrair_velho)} de "
                      f"{len(itens)} pedidos")
            else:
                falhas.append(
                    "extrair.py de ANTES nao perdeu os dois '⏸' como "
                    f"esperado -- leu {len(itens_extrair_velho)} de "
                    f"{len(itens)} (esperava {len(itens) - 2})")

            estados_novo = {i["n"]: i["estado"] for i in itens_extrair_novo}
            if (len(itens_extrair_novo) == len(itens)
                    and all(estados_novo.get(n) == "depois" for n in escolhidos)):
                print(f"  ok    extrair.py consertado (mesmo motor) mostra "
                      f"os {len(itens_extrair_novo)} pedidos, com os dois "
                      "'⏸' como 'depois'")
            else:
                falhas.append(
                    "extrair.py consertado nao leu todos os pedidos com o "
                    f"estado certo -- {len(itens_extrair_novo)} de "
                    f"{len(itens)}, escolhidos={escolhidos} -> "
                    f"{[estados_novo.get(n) for n in escolhidos]}")

            # E sem nenhum '⏸' (o PENDENCIAS.md de verdade, hoje), o texto
            # que o `bloco_recusados()` produz para o TECNOLOGIAS.md tem de
            # sair IGUAL entre a copia de antes e o conserto -- o motor unico
            # nao pode mudar o que ja funcionava. Carrega uma copia LIMPA do
            # modulo novo (a de cima ficou com `_importar_pagina_dos_pedidos`
            # trocado para o teste com '⏸' acima) para nao herdar aquele
            # remendo aqui.
            raiz_sem_depois = tmp / "raiz_sem_depois"
            (raiz_sem_depois / "docs").mkdir(parents=True)
            copia_sem_depois = raiz_sem_depois / "docs" / "PENDENCIAS.md"
            copia_sem_depois.write_text(base, encoding="utf-8")
            ex_velho.RAIZ = raiz_sem_depois
            ex_novo_limpo = carregar(caminho_extrair, "ex_novo_484_limpo", AQUI)
            g_novo.FONTE = copia_sem_depois
            ex_novo_limpo._importar_pagina_dos_pedidos = lambda: g_novo
            bloco_velho = ex_velho.bloco_recusados()
            bloco_novo = ex_novo_limpo.bloco_recusados()
            g_novo.FONTE = copia_com_depois
            if bloco_velho != bloco_novo:
                falhas.append("bloco_recusados() do extrair.py mudou sem "
                              "nenhum '⏸' no PENDENCIAS.md real")
            else:
                print("  ok    bloco_recusados() do extrair.py (usado no "
                      "TECNOLOGIAS.md) sai byte a byte igual, contra o "
                      "PENDENCIAS.md real, sem nenhum '⏸'")

        # =========================================== 4: suporte reposto AUSENTE
        # o modulo de ANTES (git show ANTES) contra a MESMA copia com '⏸'.
        g_velho.FONTE = copia_com_depois
        try:
            g_velho.ler()
            falhas.append("o leitor de ANTES do pedido 484 nao parou com o "
                          "'⏸' -- o defeito reposto deveria ter travado aqui")
        except SystemExit as e:
            msg = str(e)
            if "PENDENCIAS.md:" not in msg or "um dos estados" not in msg:
                falhas.append(f"parou, mas sem nomear a linha e o motivo: {msg}")
            else:
                print(f"  ok    suporte reposto ausente para com 'estado "
                      f"desconhecido': {msg.splitlines()[0][:110]}...")

        # ======================== 5: SEM nenhum '⏸', tudo sai igual, byte a byte
        base_velho = tmp / "base_velho.md"
        base_novo = tmp / "base_novo.md"
        base_velho.write_text(base, encoding="utf-8")
        base_novo.write_text(base, encoding="utf-8")
        g_velho.FONTE = base_velho
        g_novo.FONTE = base_novo

        itens_velho = g_velho.ler()
        itens_novo = g_novo.ler()
        # Compara so os campos que o modulo VELHO tambem tem -- o novo ganhou
        # `pedido_md`/`estado_md` (texto bruto, para quem alimenta markdown
        # em vez de HTML, achado da revisao de QA no `extrair.py`), e essa
        # adicao nao esta coberta pela promessa "sem `⏸` nada muda": ela nao
        # entra em NENHUM artefato gerado (pagina, contagem, painel), so' no
        # retorno de `ler()` -- e e' exatamente por isso que os artefatos
        # comparados abaixo continuam byte a byte iguais.
        campos_comuns = ("classe", "rotulo", "n", "pedido", "estado")
        divergiu = [
            (v, n) for v, n in zip(itens_velho, itens_novo)
            if any(v[c] != n[c] for c in campos_comuns)
        ]
        if len(itens_velho) != len(itens_novo) or divergiu:
            falhas.append("ler() do modulo novo diverge do modulo velho "
                          "quando ninguem usa '⏸'")
        else:
            print(f"  ok    ler() identico ao de hoje sem '⏸' nos campos "
                  f"em comum ({len(itens_novo)} pedidos; o novo so' ganhou "
                  "'pedido_md'/'estado_md', que nao entram em nenhum "
                  "artefato gerado)")

        c_velho = g_velho.contas_de(itens_velho)
        c_novo = g_novo.contas_de(itens_novo)
        if any(c_velho[k] != c_novo[k] for k in ("feito", "parcial", "planejado")):
            falhas.append(f"contas_de diverge sem '⏸': velho={c_velho} "
                          f"novo={c_novo}")
        elif c_novo["depois"] != 0:
            falhas.append(f"contas_de novo mostrou depois != 0 sem nenhum "
                          f"'⏸' no arquivo: {c_novo}")
        else:
            print("  ok    contas_de identico (mais 'depois'=0, que nunca "
                  "aparece em lugar nenhum)")

        g_velho.gravar_contagem(itens_velho)
        g_novo.gravar_contagem(itens_novo)
        bloco_velho = bloco_entre(base_velho.read_text(encoding="utf-8"),
                                  g_velho.ABRE_C, g_velho.FECHA_C)
        bloco_novo = bloco_entre(base_novo.read_text(encoding="utf-8"),
                                 g_novo.ABRE_C, g_novo.FECHA_C)
        if bloco_velho != bloco_novo:
            falhas.append("linha de contagem MUDOU sem nenhum '⏸':\n"
                          f"  velho: {bloco_velho!r}\n  novo : {bloco_novo!r}")
        else:
            print("  ok    linha de contagem byte a byte igual sem '⏸'")

        dossie_velho = tmp / "dossie_velho.html"
        dossie_novo = tmp / "dossie_novo.html"
        dossie_velho.write_text(DOSSIE_FALSO, encoding="utf-8")
        dossie_novo.write_text(DOSSIE_FALSO, encoding="utf-8")
        g_velho.gravar_no_dossie(dossie_velho, itens_velho)
        g_novo.gravar_no_dossie(dossie_novo, itens_novo)
        if dossie_velho.read_bytes() != dossie_novo.read_bytes():
            falhas.append("painel do dossie MUDOU sem nenhum '⏸'")
        else:
            print("  ok    painel do dossie byte a byte igual sem '⏸'")

        pasta_velho = tmp / "paginas_velho"
        pasta_novo = tmp / "paginas_novo"
        pasta_velho.mkdir()
        pasta_novo.mkdir()
        g_velho.PASTA_SAIDA = pasta_velho
        g_novo.PASTA_SAIDA = pasta_novo
        g_velho.gravar_faixas(itens_velho)
        g_novo.gravar_faixas(itens_novo)
        diffs = comparar_pastas(pasta_velho, pasta_novo)
        if diffs:
            falhas.append("as paginas geradas MUDARAM sem nenhum '⏸': "
                          + "; ".join(diffs[:5]))
        else:
            n_paginas = len(list(pasta_novo.glob("*.html")))
            print(f"  ok    as {n_paginas} paginas geradas sao byte a byte "
                  "identicas as de hoje, sem nenhum '⏸'")

    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    print()
    if falhas:
        print(f"VERMELHO: {len(falhas)} caso(s) falharam:")
        for f in falhas:
            print("  - " + f)
        return 1
    print("VERDE: o estado '⏸ depois da versão' funciona nos dois sentidos -- "
          "contagem, porcentagem e pagina o mostram; suporte reposto ausente "
          "para; e sem nenhum '⏸' tudo sai igual a hoje, byte a byte.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

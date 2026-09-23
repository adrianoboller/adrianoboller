#!/usr/bin/env python3
"""O PORTAO DA VERSAO -- acusa quando o numero selado envelheceu.

O buraco que ele fecha: `version = "0.18.0"` no Cargo.toml foi selado em
29/08/2026 (commit `baff46e`) e a arvore andou **887 commits** sem que nada
acusasse. O `empacotar.sh` ja tinha um portao (`confere_versoes`), mas ele so
confere que Cargo.toml / Cargo.lock / MANUAL.txt / CHANGELOG.md CONCORDAM entre
si -- quatro copias do mesmo numero velho passam por ele de bracos abertos,
porque concordar nao e o mesmo que DESCREVER o que existe. Faltava o portao que
mede a DISTANCIA entre a selagem e o HEAD.

    python3 docs/versao/portao-da-versao.py            # confere e acusa
    python3 docs/versao/portao-da-versao.py --alcance  # so imprime o que ele mede e o que nao mede

Sai != 0 quando a versao selada esta VELHA (distancia acima do teto medido) ou
quando o portao nao consegue medir nada (repositorio raso, git ausente).

## A REGUA escolhida, e por que nao as outras

A distancia e contada em **commits desde o commit que selou a versao atual**
(`git rev-list --count <selagem>..HEAD`), e o commit da selagem NUNCA e
digitado -- sai de `git log -S 'version = "X.Y.Z"' -- Cargo.toml`, o mesmo
pente (pickaxe) usado para medir este defeito. Tres alternativas foram
pesadas e descartadas, cada uma com o motivo:

* **Tempo de parede (dias desde a selagem).** Descartada: um dia de rodada
  feroz com quinze commits e um fim de semana parado nao valem o mesmo, e o
  numero de commits ja captura "quanto a arvore andou" melhor do que o
  relogio -- foi o proprio caso desta rodada, 887 commits em 25 dias corridos,
  que provou que o relogio sozinho mentiria por falta (pareceria "so 25").
* **Linhas alteradas (`git diff --stat`).** Descartada: o dossie e as paginas
  satelites sao regeneradas por script e pesam centenas de KB por commit
  (`docs/dossie/*.html`) -- um commit que so re-roda `numeros-do-projeto.py`
  pesaria mais nesta regua do que dez commits de codigo real. Contar bytes
  mediria o gerador de paginas, nao a deriva da versao.
* **Pedidos fechados no `PENDENCIAS.md`.** Descartada pelo mesmo motivo que o
  `docs/versao/conferir.py` ja recusa classificar historia por texto: decidir
  se um pedido "fechou desde a selagem" exige ler prosa e julgar, e e
  exatamente o tipo de classificacao por comparacao de frase que esta casa ja
  proibiu (mensagens de tela) por quebrar calado quando a redacao muda.

Commit e barato, deterministico, e nao exige julgamento nenhum: um commit
aconteceu ou nao aconteceu. E o preco declarado abaixo (o que ele NAO mede) e
o correspondente dessa escolha.

## O TETO -- medido em 23/09/2026, nasce no numero do dia

`TETO_COMMITS_SEM_SELAR` e o MAIOR intervalo entre duas selagens consecutivas
em toda a historia do projeto, ANTES desta rodada (0.1.0 ate 0.18.0, 21
selagens, 20 intervalos). Medido com o mesmo pente usado acima:

    772b8e5->d2d895d 25   d2d895d->852337e 4    852337e->8dc5f71 3
    8dc5f71->b2237bf  1   b2237bf->321ef68 8    321ef68->ef2493b 1
    ef2493b->db7eaec  1   db7eaec->9453c90 1    9453c90->52c7bd8 1
    52c7bd8->4dbaedd  1   4dbaedd->6f1ad37 2    6f1ad37->57a7ac7 1
    57a7ac7->3e1ec78  5   3e1ec78->f0962fc 8    f0962fc->bbba31d 1
    bbba31d->86d116a  2   86d116a->4fb39c4 1    4fb39c4->59394de 12
    59394de->c4af6d0  2   c4af6d0->baff46e 51

O maior e **51** (`c4af6d0..baff46e`, a propria rodada que fechou a 0.18.0).
887 e **17,4x** esse maximo -- nao e ruido de medicao, e uma selagem esquecida.

Esta e uma CATRACA, e catraca so desce (`CLAUDE.md`): o numero acima nao se
levanta por decisao de estilo. Se o ritmo real do projeto mudar e alguem
quiser um teto maior, a resposta e aposentar esta constante e nascer outra,
nomeada e medida no dia, como o `TETO_TABELA_NA_MAO` do QA -- nunca editar o
numero desta linha para cima.

## O que ele NAO mede -- declarado, nao escondido

* **Relevancia de cada commit.** Um commit gigante conta como 1; cem commits
  triviais (typo, reformatacao) contam como 100. O portao mede DISTANCIA da
  arvore, nao ESFORCO nem RISCO -- ele nao le diff nenhum.
* **Repositorio raso.** Um `git clone --depth` nao tem o commit da selagem no
  historico local; o portao RECUSA em vez de medir uma distancia curta falsa
  (o mesmo cuidado do `tetos-da-trava.py`, que reprova em vez de emitir vazio
  quando a fonte sumiu).
* **Selo commitado, mas em branch que nao contem HEAD.** `git rev-list --count`
  exige que a selagem seja ancestral do HEAD atual; se nao for (historia
  reescrita, cherry-pick sem a selagem), o portao avisa em vez de supor.
* **Selagem feita e AINDA NAO commitada.** Se a versao do `Cargo.toml` na
  arvore de trabalho nao aparece em commit nenhum, o portao trata o HEAD atual
  como a base (distancia 0) -- e o caso de quem acabou de editar o numero e
  vai commitar em seguida; ele NAO acusa VELHO por causa disso, mas diz que
  fez essa suposicao, em vez de fingir certeza que nao tem.
"""

import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[2]

# Medido em 23/09/2026 -- o maior intervalo entre duas selagens consecutivas
# de toda a historia do projeto (0.1.0 ate 0.18.0), lido no cabecalho acima.
# So desce: ver a secao "O TETO" no docstring.
TETO_COMMITS_SEM_SELAR = 51


def git(*args):
    r = subprocess.run(
        ["git", *args], cwd=RAIZ, capture_output=True, text=True
    )
    return r.returncode, r.stdout.strip(), r.stderr.strip()


def versao_da_arvore():
    texto = (RAIZ / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'^version = "(\d+\.\d+\.\d+)"', texto, re.M)
    if not m:
        sys.exit("portao-da-versao: nao achei `version = \"X.Y.Z\"` no Cargo.toml")
    return m.group(1)


def eh_raso():
    codigo, saida, _ = git("rev-parse", "--is-shallow-repository")
    return codigo == 0 and saida == "true"


def commit_da_selagem(versao):
    """O commit que introduziu `version = "<versao>"` no Cargo.toml, pelo
    pente (`-S`, pickaxe) -- nunca digitado. `git log` lista do mais novo para
    o mais velho, e o mais novo e o que importa: esta arvore nunca reusou uma
    string de versao (medido nos 21 bumps de 0.1.0 a 0.18.0), mas se algum dia
    reusar, e a selagem MAIS RECENTE que decide a distancia."""
    alvo = f'version = "{versao}"'
    codigo, saida, erro = git("log", "--format=%H", "-S", alvo, "--", "Cargo.toml")
    if codigo != 0:
        return None, erro
    linhas = [l for l in saida.splitlines() if l.strip()]
    return (linhas[0] if linhas else None), None


def principal():
    so_alcance = "--alcance" in sys.argv

    if so_alcance:
        print(__doc__)
        return 0

    codigo, _, erro = git("rev-parse", "--is-inside-work-tree")
    if codigo != 0:
        print(f"VERMELHO: nao consegui falar com o git ({erro or 'sem detalhe'})")
        return 2

    if eh_raso():
        print("VERMELHO: repositorio RASO (git clone --depth) -- o commit da "
              "selagem pode nao estar no historico local. O portao RECUSA "
              "medir em vez de devolver uma distancia curta falsa. Rode contra "
              "um clone completo (`git fetch --unshallow`).")
        return 2

    versao = versao_da_arvore()
    print(f"versao na arvore (Cargo.toml): {versao}")

    selagem, erro = commit_da_selagem(versao)
    if erro:
        print(f"VERMELHO: o pente do git falhou: {erro}")
        return 2

    if selagem is None:
        print(f"selo ainda NAO commitado -- nenhum commit introduz "
              f"`version = \"{versao}\"` no Cargo.toml. Medindo a distancia a "
              f"partir do HEAD atual (0 commits): a selagem esta em curso, "
              f"nao envelhecida.")
        print(f"\nVERDE: 0 commits desde a selagem (nao commitada), dentro do "
              f"teto de {TETO_COMMITS_SEM_SELAR}.")
        return 0

    codigo, distancia_txt, erro = git("rev-list", "--count", f"{selagem}..HEAD")
    if codigo != 0:
        print(f"VERMELHO: nao consegui contar `{selagem}..HEAD`: {erro}\n"
              f"   (a selagem pode nao ser ancestral do HEAD -- historia "
              f"reescrita ou cherry-pick sem ela)")
        return 2

    distancia = int(distancia_txt)
    codigo, data_selagem, _ = git("log", "-1", "--format=%ci", selagem)
    curto = selagem[:7]

    print(f"selagem: commit {curto} ({data_selagem})")
    print(f"distancia medida: {distancia} commit(s) desde a selagem")
    print(f"teto (maior intervalo historico, medido em 23/09/2026): "
          f"{TETO_COMMITS_SEM_SELAR}")

    if distancia > TETO_COMMITS_SEM_SELAR:
        vezes = distancia / TETO_COMMITS_SEM_SELAR
        print(f"\nVERMELHO: a versao {versao} envelheceu -- {distancia} "
              f"commits e {vezes:.1f}x o maior intervalo ja registrado entre "
              f"duas selagens ({TETO_COMMITS_SEM_SELAR}). Sele uma nova "
              f"versao (Cargo.toml, Cargo.lock, MANUAL.txt, CHANGELOG.md).")
        return 1

    print(f"\nVERDE: {distancia} commit(s) desde a selagem, dentro do teto de "
          f"{TETO_COMMITS_SEM_SELAR}.")
    return 0


if __name__ == "__main__":
    sys.exit(principal())

#!/usr/bin/env python3
"""Escreve o `docs/GESTAO.md` do `resultados.json` medido.

    python3 bancada/gestao/medir.py
    python3 bancada/gestao/documento.py

Mesma disciplina do `docs/COMPARATIVO.md`: a prosa mora aqui, a medicao mora
no JSON, e o documento **nao se edita**.
"""

import datetime
import json
import pathlib
import sys
import textwrap

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]
FONTE = AQUI / "resultados.json"
ALVO = RAIZ / "docs" / "GESTAO.md"
COLUNA = 78

MARCA = {"ok": "✅", "parcial": "◐", "planejado": "☐"}

# A que area cada linha pertence, para o documento agrupar como o dono pediu.
AREA = {
    "select_protocolo": "Os quatro verbos, pelo protocolo",
    "select_where": "Os quatro verbos, pelo protocolo",
    "insert_protocolo": "Os quatro verbos, pelo protocolo",
    "insert_lote": "Os quatro verbos, pelo protocolo",
    "update_protocolo": "Os quatro verbos, pelo protocolo",
    "delete_suave": "Os quatro verbos, pelo protocolo",
    "delete_restaurar": "Os quatro verbos, pelo protocolo",
    "delete_de_vez": "Os quatro verbos, pelo protocolo",
    "integridade": "Os quatro verbos, pelo protocolo",
    "sql_select": "Os quatro verbos, pela camada SQL",
    "sql_insert": "Os quatro verbos, pela camada SQL",
    "sql_update": "Os quatro verbos, pela camada SQL",
    "sql_delete": "Os quatro verbos, pela camada SQL",
    "backup": "Backup e restauração",
    "restaurar_backup": "Backup e restauração",
}
ORDEM = ["Os quatro verbos, pelo protocolo",
         "Os quatro verbos, pela camada SQL",
         "Backup e restauração"]


class T:
    def __init__(self):
        self.p_ = []

    def p(self, t):
        self.p_ += [textwrap.fill(" ".join(t.split()), COLUNA), ""]

    def l(self, t=""):
        self.p_.append(t)

    def fim(self):
        return "\n".join(self.p_).rstrip() + "\n"


def main():
    if not FONTE.exists():
        sys.exit(f"falta {FONTE} -- rode `python3 bancada/gestao/medir.py`")
    d = json.loads(FONTE.read_text(encoding="utf-8"))
    quando = datetime.datetime.fromisoformat(d["quando"]).strftime("%d/%m/%Y %H:%M")
    linhas = d["linhas"]
    fora = d["de_outras_bancadas"]
    conta = {e: sum(1 for l in linhas if l["estado"] == e)
             for e in ("ok", "parcial", "planejado")}

    t = T()
    t.l("# As atividades de gestão do banco, medidas")
    t.l()
    t.p(f"""Medido em **{quando}** contra o motor vivo — `{d['versao']}`. Cada
        linha desta página subiu o servidor, exercitou a atividade e olhou o
        **efeito**: inseriu e leu de volta, excluiu e conferiu que sumiu,
        gerou o backup e o **restaurou com outro nome** para ler a linha de
        dentro da cópia.""")
    t.p(f"""**{conta['ok']} ok · {conta['parcial']} parcial ·
        {conta['planejado']} planejado**, mais replicação e cluster, que têm
        bancada própria e entram aqui com a data da medição delas.""")
    t.l("> Refaça com `python3 bancada/gestao/medir.py` e depois")
    t.l("> `python3 bancada/gestao/documento.py`. **Este arquivo não se edita.**")
    t.l()

    t.l("---")
    t.l()
    t.l("## 1. O portão que este medidor tem, e por que ele existe")
    t.l()
    t.p(f"""Antes de medir, ele confere **{d['operacoes_conferidas_no_catalogo']}
        operações** contra o catálogo do próprio servidor: o nome existe? os
        parâmetros obrigatórios estão todos sendo mandados? Só então começa.""")
    t.p("""A primeira corrida publicou **cinco** «planejado» que eram defeito
        meu, não do motor: inventei `contar` e `excluir_de_vez` (não existem),
        mandei `linha` onde o contrato pede `valores`, chamei `bulkinsert`
        achando que era `inserir_lote`, e esqueci o `destino` obrigatório do
        `backup`. Cada erro voltou como recusa — e **recusa não lida vira
        ausência publicada.** Com o portão, isso vira uma parada com o nome do
        erro em vez de uma linha na tabela.""")
    t.l()

    for area in ORDEM:
        t.l("---")
        t.l()
        t.l(f"## {ORDEM.index(area) + 2}. {area}")
        t.l()
        t.l("| | atividade | o que foi medido |")
        t.l("|---|---|---|")
        for l in linhas:
            if AREA.get(l["chave"]) != area:
                continue
            t.l(f"| {MARCA[l['estado']]} | {l['titulo']} | {l['prova']} |")
        t.l()

    t.l("---")
    t.l()
    t.l(f"## {len(ORDEM) + 2}. Replicação e cluster")
    t.l()
    t.p("""Estas duas pedem vários servidores e têm bancada própria. Os números
        abaixo saem do `resultados.json` de cada uma, **com a data da corrida**
        — juntar medições de dias diferentes sem dizer quando publica um
        retrato que nunca existiu.""")
    for chave, rot in (("replicacao", "Replicação"), ("cluster", "Cluster")):
        v = fora.get(chave) or {}
        t.l(f"### {MARCA.get(v.get('estado'), '☐')} {rot} — medida em "
            f"{v.get('quando', '—')}"
            + (" *(data do mtime do arquivo)*" if v.get("data_do_mtime") else ""))
        t.l()
        b = v.get("bruto") or {}
        if not b:
            t.l(f"**NÃO MEDIDA** — {v.get('nota', '')}")
            t.l()
            continue
        for k, val in b.items():
            if isinstance(val, (int, float, bool, str)) and k not in ("quando",):
                t.l(f"- `{k}` — **{val}**")
        t.l()

    t.l("---")
    t.l()
    t.l(f"## {len(ORDEM) + 3}. O que esta página NÃO diz")
    t.l()
    t.l("1. **Não é veredito de produção.** Ela diz que a atividade funciona,")
    t.l("   não que o conjunto está pronto para carga real com gente dentro.")
    t.l("   O que falta para isso está no `docs/COMPARATIVO.md` e no")
    t.l("   `docs/PENDENCIAS.md`.")
    t.l("2. **`ok` aqui é «faz o que promete», não «faz rápido».** Custo é")
    t.l("   assunto da `bancada/`, que mede outra coisa.")
    t.l("3. **A tela tem caminho próprio, e ele é filmado.** O vídeo de")
    t.l("   `testes-web/video-gestao.mjs` clica os botões de verdade — porque")
    t.l("   interface só se prova exercitando, e o protocolo passar não")
    t.l("   garante que a tela chegue lá.")

    ALVO.write_text(t.fim(), encoding="utf-8")
    print(f"escrito: {ALVO.relative_to(RAIZ)}")
    print(f"  {conta['ok']} ok, {conta['parcial']} parcial, {conta['planejado']} planejado")


if __name__ == "__main__":
    main()

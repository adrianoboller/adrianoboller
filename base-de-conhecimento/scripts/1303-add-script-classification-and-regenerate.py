# Add script classification and regenerate
# 30/08 16:26

p='base-de-conhecimento/extrair.py'
s=open(p,encoding='utf-8').read()
velho='''    with open(esc("00-INDICE.md"), "w", encoding="utf-8") as o:'''
novo='''    # 5. Classificar os scripts. Sem isto, mil e trezentos arquivos escondem os
    #    vinte que valem: a maioria e conserto de uma vez so, e o que se
    #    reaproveita e a MEDICAO e a PROVA.
    def classe(txt):
        t = txt.lower()
        if "time.time()" in t or "perf_counter" in t or "mediana" in t or " ms/" in t:
            return "medicao"
        if "defeito reposto" in t or ("assert" in t and "reprov" in t):
            return "prova"
        if "socket" in t and "json" in t:
            return "sonda-de-protocolo"
        if "re.sub" in t or "replace(" in t:
            return "edicao-em-massa"
        if "os.walk" in t or "glob" in t or "listdir" in t:
            return "varredura"
        return "outro"

    por_classe = {}
    for arq in sorted(os.listdir(esc("scripts"))):
        corpo = open(esc(f"scripts/{arq}"), encoding="utf-8").read()
        por_classe.setdefault(classe(corpo), []).append((arq, corpo.splitlines()[0].lstrip("# ")))
    ordem = ["medicao", "prova", "sonda-de-protocolo", "varredura", "edicao-em-massa", "outro"]
    titulo = {
        "medicao": "Medicao -- cronometram, contam, comparam",
        "prova": "Prova real -- repoem o defeito e conferem que reprova",
        "sonda-de-protocolo": "Sondas de protocolo -- falam com o servidor por soquete",
        "varredura": "Varredura -- percorrem arvore de arquivos",
        "edicao-em-massa": "Edicao em massa -- mexem no fonte por padrao",
        "outro": "Os demais",
    }
    with open(esc("scripts/00-INDICE.md"), "w", encoding="utf-8") as o:
        o.write("# Os scripts, por tecnica\\n\\n")
        o.write("As tres primeiras familias sao as que se reaproveitam em outro\\n")
        o.write("projeto. `edicao-em-massa` e conserto de uma vez so -- guardado\\n")
        o.write("pelo padrao, nao pelo conteudo.\\n\\n")
        for c in ordem:
            itens = por_classe.get(c, [])
            if not itens:
                continue
            o.write(f"## {titulo[c]}  ({len(itens)})\\n\\n")
            for arq, desc in itens:
                o.write(f"- `{arq}` -- {desc}\\n")
            o.write("\\n")

    with open(esc("00-INDICE.md"), "w", encoding="utf-8") as o:'''
assert s.count(velho)==1
s=s.replace(velho,novo)
s=s.replace('| `scripts/` | {salvos} scripts Python que rodaram por heredoc |',
            '| `scripts/` | {salvos} scripts Python, classificados em `scripts/00-INDICE.md` |')
open(p,'w',encoding='utf-8').write(s)
print("classificacao acrescentada")

# Conserta a formatacao dos milhares
# 01/09 18:26

from pathlib import Path
p = Path("bancada/comparacao/medir.py")
s = p.read_text(encoding="utf-8")

# O `.replace(",", ".")` trocava tambem as virgulas de SEPARAR FRASE, e a
# linha saia «20.000 linhas. 1.000 operacoes.». Cada numero se formata sozinho.
s = s.replace(
    '''    print(f"=== {n:,} linhas, {ops:,} operacoes, {rodadas} rodadas ==="
          .replace(",", "."), flush=True)''',
    '''    print(f"=== {mil(n)} linhas, {mil(ops)} operacoes, {rodadas} rodadas ===",
          flush=True)''',
)
s = s.replace(
    '''            problemas.append(f"  apos {etapa}: " + ", ".join(
                f"{m}=({q:,} linhas, valor {v:,}, cadastro {c:,})".replace(",", ".")
                for m, (q, v, c) in vistos.items()
            ))''',
    '''            problemas.append(f"  apos {etapa}: " + ", ".join(
                f"{m}=({mil(q)} linhas, valor {mil(v)}, cadastro {mil(c)})"
                for m, (q, v, c) in vistos.items()
            ))''',
)
s = s.replace(
    '''def alvo(k, n):''',
    '''def mil(x):
    """Ponto de milhar, sem estragar as virgulas da frase em volta."""
    return f"{x:,}".replace(",", ".")


def alvo(k, n):''',
)
# As duas ressalvas usavam o mesmo truque e sofriam do mesmo defeito.
s = s.replace(
    '''        f"A carga inicial nao tem a mesma FORMA nos tres: o PhxSql faz {n:,} "
        "chamadas de funcao, o SQLite(R) executa a mesma instrucao preparada "
        f"{n:,} vezes, e o MySQL(R) recebe {(n + LOTE - 1) // LOTE} instrucoes "
        f"de {LOTE:,} linhas. A forma do MySQL(R) e a mais barata das tres por "
        "linha, entao a barra dele nesta fase e OTIMISTA. As fases pontuais "
        "sao uma instrucao por operacao nos tres.".replace(",", "."),''',
    '''        f"A carga inicial nao tem a mesma FORMA nos tres: o PhxSql faz {mil(n)} "
        "chamadas de funcao, o SQLite(R) executa a mesma instrucao preparada "
        f"{mil(n)} vezes, e o MySQL(R) recebe {(n + LOTE - 1) // LOTE} instrucoes "
        f"de {mil(LOTE)} linhas. A forma do MySQL(R) e a mais barata das tres por "
        "linha, entao a barra dele nesta fase e OTIMISTA. As fases pontuais "
        "sao uma instrucao por operacao nos tres.",''',
)
s = s.replace(
    '''        + (f"{statistics.median(piso):.3f} s para {ops:,} instrucoes que nao "
           "fazem nada (`DO 1;`), que e o que ha para subtrair da barra dele "
           "nas fases pontuais.".replace(",", ".") if piso
           else "nao medido nesta corrida."),''',
    '''        + (f"{statistics.median(piso):.3f} s para {mil(ops)} instrucoes que nao "
           "fazem nada (`DO 1;`), que e o que ha para subtrair da barra dele "
           "nas fases pontuais." if piso else "nao medido nesta corrida."),''',
)
p.write_text(s, encoding="utf-8")
print("formatacao dos milhares consertada")

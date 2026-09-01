# Durabilidade com acento, e o ajuste do servidor como dado
# 01/09 18:35

from pathlib import Path
g = Path("bancada/comparacao/grafico.py")
s = g.read_text(encoding="utf-8")
s = s.replace('"achar {ops} linhas pela chave, uma instrucao cada"',
              '"achar {ops} linhas pela chave, uma instrução cada"')
g.write_text(s, encoding="utf-8")

m = Path("bancada/comparacao/medir.py")
s = m.read_text(encoding="utf-8")

# A durabilidade tambem APARECE NA PAGINA, e saia sem acento. Para ela poder
# ser refeita como as ressalvas, o que o servidor respondeu vira DADO
# guardado, e a frase vira uma funcao que le esse dado.
s = s.replace(
    '''        "durabilidade": {
            "PhxSql": "sincroniza uma vez por fase (exclusao na janela)",
            "MySQL(R)": "uma transacao por fase; "
                        + ", ".join(f"{k}={v}" for k, v in ajuste_do_mysql().items()),
            "SQLite(R)": "synchronous=FULL, journal DELETE, uma transacao por fase",
        },''',
    '''        "ajuste_do_mysql": ajuste_do_mysql(),
        "durabilidade": durabilidade(ajuste_do_mysql()),''',
)
s = s.replace(
    '''def ressalvas(n, ops, rodadas, piso):''',
    '''def durabilidade(ajuste):
    """O regime de cada motor, em texto de tela -- montado do que o servidor
    respondeu, e nao de uma frase digitada aqui."""
    return {
        "PhxSql": "sincroniza uma vez por fase (exclusão na janela)",
        "MySQL(R)": "uma transação por fase; "
                    + ", ".join(f"{k}={v}" for k, v in ajuste.items()),
        "SQLite(R)": "synchronous=FULL, journal DELETE, uma transação por fase",
    }


def ressalvas(n, ops, rodadas, piso):''',
)
s = s.replace(
    '''    d["ressalvas"] = ressalvas(
        d["linhas"], d["operacoes_por_fase_pontual"], d["rodadas"], piso
    )''',
    '''    d["ressalvas"] = ressalvas(
        d["linhas"], d["operacoes_por_fase_pontual"], d["rodadas"], piso
    )
    # O ajuste do servidor e DADO medido: se ele nao esta guardado, pergunta-se
    # de novo -- e se o servidor nao responder, a durabilidade fica como esta,
    # em vez de virar uma frase sem os numeros dentro.
    ajuste = d.get("ajuste_do_mysql") or ajuste_do_mysql()
    if ajuste:
        d["ajuste_do_mysql"] = ajuste
        d["durabilidade"] = durabilidade(ajuste)''',
)
m.write_text(s, encoding="utf-8")
print("medir.py: durabilidade virou funcao, e o ajuste do servidor virou dado")

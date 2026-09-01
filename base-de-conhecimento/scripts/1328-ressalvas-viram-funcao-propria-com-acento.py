# Ressalvas viram funcao propria, com acento
# 01/09 18:33

from pathlib import Path
p = Path("bancada/comparacao/medir.py")
s = p.read_text(encoding="utf-8")

velho_ini = '    ress = [\n'
i = s.index(velho_ini)
j = s.index('    return {\n        "linhas": n,')
antigo = s[i:j]

novo = '''    ress = ressalvas(n, ops, rodadas, piso)

'''
s = s[:i] + novo + s[j:]

# A prosa vira funcao propria, e passa a ser REFEITA a partir dos numeros
# guardados. Antes ela nascia dentro do `monta` e so se podia corrigir
# remedindo: uma palavra sem acento custava quinze minutos de bancada.
funcao = '''def ressalvas(n, ops, rodadas, piso):
    """O que estes numeros nao dizem, montado a partir do que foi medido.

    Fica em funcao propria, e nao dentro do `monta`, porque este texto APARECE
    NA PAGINA: e texto de interface, leva acento, e um dia alguem vai querer
    melhorar a redacao. Se a unica forma de reescrever fosse remedir, uma
    virgula custaria quinze minutos de bancada -- e o que se faria em vez
    disso e editar o JSON a mao, que e como um numero gerado vira digitado.
    """
    piso_txt = (
        f"{statistics.median(piso):.3f} s para {mil(ops)} instruções que não "
        "fazem nada (`DO 1;`), que é o que há para subtrair da barra dele nas "
        "fases pontuais." if piso else "não medido nesta corrida."
    )
    return [
        f"A carga inicial não tem a mesma FORMA nos três: o PhxSql faz {mil(n)} "
        "chamadas de função, o SQLite(R) executa a mesma instrução preparada "
        f"{mil(n)} vezes, e o MySQL(R) recebe {(n + LOTE - 1) // LOTE} instruções "
        f"de {mil(LOTE)} linhas. A forma do MySQL(R) é a mais barata das três por "
        "linha, então a barra dele nesta fase é OTIMISTA. As fases pontuais são "
        "uma instrução por operação nos três.",
        "O MySQL(R) é o único que recebe o trabalho como TEXTO por soquete — não "
        "existe MySQL(R) embutido nesta máquina, e os outros dois são biblioteca "
        "no próprio processo. O piso desse formato foi medido: " + piso_txt,
        "O SQLite(R) publicado é a variante `rowid` (`id INTEGER PRIMARY KEY`), "
        "que é a que casa com o InnoDB por ter a chave agrupada e a que FAVORECE "
        "o SQLite(R) — são duas estruturas contra as três da variante `2ind`. A "
        "outra corre na mesma rodada e está no JSON, em `sqlite_2ind`.",
        "Durabilidade casada: uma sincronização no fim de cada fase nos três. Não "
        "é o regime de quem grava pedido a pedido — uma bancada com `commit` por "
        "linha daria outros números, e é a que importa para esse caso.",
        "Uma máquina só, com o que mais estivesse rodando nela. O bigode de mínimo "
        f"a máximo das {rodadas} rodadas é a medida dessa inquietude: barra lisa "
        "afirmaria uma precisão que o número não tem.",
    ]


def monta(n, ops, rodadas'''
s = s.replace("def monta(n, ops, rodadas", funcao, 1)

# E o modo que refaz so a prosa, sem remedir.
s = s.replace(
    '''def principal():
    argv = sys.argv[1:]''',
    '''def refazer_prosa():
    """Reescreve as ressalvas do JSON a partir dos numeros que ja estao nele.

    Nao remede nada e nao toca em numero nenhum -- so na prosa que sai deles.
    """
    d = json.loads(RESULTADOS.read_text(encoding="utf-8"))
    piso = (d.get("piso_do_mysql_s") or {}).get("amostras") or []
    d["ressalvas"] = ressalvas(
        d["linhas"], d["operacoes_por_fase_pontual"], d["rodadas"], piso
    )
    PARCIAL.write_text(json.dumps(d, indent=2, ensure_ascii=False), encoding="utf-8")
    os.replace(PARCIAL, RESULTADOS)
    print(f"prosa refeita em {RESULTADOS.name}; os numeros nao foram tocados")


def principal():
    argv = sys.argv[1:]
    if "--so-prosa" in argv:
        return refazer_prosa()''',
)
p.write_text(s, encoding="utf-8")
print("medir.py: ressalvas viraram funcao, e ganharam o modo --so-prosa")

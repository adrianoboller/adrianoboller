# Make the comparison fair and retest
# 27/08 21:40

p='/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/bench/medir.py'
s=open(p).read()
velho = s[s.index('    # ------------------------------------------- as outras fases, 100.000 cada'):s.index('    # ------------------------------------------------------- tamanho em disco')]
novo = '''    # ------------------------------------------ as outras fases, 20.000 cada
    #
    # UMA INSTRUCAO POR OPERACAO dos dois lados. A primeira versao desta
    # bancada mandava ao MySQL(R) um unico "WHERE id IN (100.000 ids)" e ao
    # PhxSql 100.000 buscas separadas -- e comparava os dois tempos. Nao era
    # comparacao: era uma consulta em lote contra vinte mil consultas. O
    # numero saia 41x a favor do MySQL(R) por causa da FORMA da pergunta, nao
    # do motor.
    #
    # Agora os dois recebem vinte mil instrucoes independentes. E mais lento
    # dos dois lados, e e a unica forma de o numero querer dizer alguma coisa.
    OPS = 20_000
    alvos = [(k * 7919) % n + 1 for k in range(OPS)]

    fases = [
        ("buscar", "".join(f"SELECT id FROM precos WHERE id={a};\\n" for a in alvos)),
        # A varredura por faixa e naturalmente uma consulta so dos dois lados.
        ("varrer", "SELECT COUNT(*), SUM(valor) FROM precos WHERE cidade='Blumenau';"),
        ("atualizar",
         "START TRANSACTION;\\n"
         + "".join(f"UPDATE precos SET valor=9999.00 WHERE id={a};\\n" for a in alvos)
         + "COMMIT;\\n"),
        ("excluir",
         "START TRANSACTION;\\n"
         + "".join(f"DELETE FROM precos WHERE id={a};\\n" for a in alvos)
         + "COMMIT;\\n"),
    ]
    for fase, comando in fases:
        print(f"{fase}…", flush=True)
        resultados.append(fase_phxsql(fase, OPS))
        guardar(resultados)
        resultados.append(fase_mysql(fase, OPS, comando))
        guardar(resultados)
        print(f"  PhxSql {resultados[-2]['segundos']:7.2f}s  |  "
              f"MySQL {resultados[-1]['segundos']:7.2f}s", flush=True)

'''
s = s.replace(velho, novo)
open(p,'w').write(s)
print('justica corrigida')

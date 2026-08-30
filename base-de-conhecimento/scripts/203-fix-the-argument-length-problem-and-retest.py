# Fix the argument length problem and retest
# 27/08 21:39

p='/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/bench/medir.py'
s=open(p).read()
s=s.replace('''def sql(comando, banco="bench"):
    return subprocess.run(
        ["mysql", "--protocol=socket", "-N", "-B", banco, "-e", comando],
        capture_output=True,
        text=True,
    )''','''def sql(comando, banco="bench"):
    """Manda o comando por ARQUIVO, sempre.

    Uma lista IN com 100.000 identificadores nao cabe na linha de comando --
    o sistema recusa com "Argument list too long". Por arquivo cabe, e o
    caminho e o mesmo para todos os comandos, entao a medicao nao muda de
    forma no meio do experimento."""
    arq = BASE / "comando.sql"
    arq.write_text(comando)
    r = subprocess.run(
        ["mysql", "--protocol=socket", "-N", "-B", banco, "-e", f"SOURCE {arq};"],
        capture_output=True,
        text=True,
    )
    return r''')
s=s.replace('        r = fase_mysql("inserir", quantos, f"SOURCE {gerador};")',
            '        r = fase_mysql("inserir", quantos, gerador.read_text())')
open(p,'w').write(s)

# Fix the temp scan to be O(n) and dry-run again
# 30/08 15:57

p='zelador.sh'
s=open(p,encoding='utf-8').read()
velho='''echo "-- temporarios"
apagar "$RAIZ/target/tmp" "temporario de compilacao"
for d in /tmp/phxsql-* /tmp/phx-*; do
  [ -e "$d" ] || continue
  em_uso "$d" || apagar "$d" "temporario solto"
done'''
novo='''echo "-- temporarios"
apagar "$RAIZ/target/tmp" "temporario de compilacao"

# A bateria deixa MILHARES de diretorios em /tmp -- 13.743 ocupando 4,7 GB na
# primeira vez que se olhou. Conferir um por um chamando `em_uso` varreria o
# /proc inteiro treze mil vezes; a lista de processos vivos se levanta UMA vez.
#
# Dois criterios guardam o que pode estar em uso, e os dois erram para o lado
# seguro: o nome carrega o PID de quem criou, entao PID vivo fica; e mexido nos
# ultimos 30 minutos fica, porque teste em curso ainda escreve.
python3 - "$VER" <<'FIM'
import os, sys, time, shutil, glob
so_ver = sys.argv[1] == "--ver"
vivos = set(os.listdir("/proc"))
corte = time.time() - 30 * 60
apagados = bytes_ = 0
guardados = 0
for d in glob.glob("/tmp/phxsql-*") + glob.glob("/tmp/phx-*"):
    partes = os.path.basename(d).split("-")
    pid_vivo = any(p.isdigit() and p in vivos for p in partes)
    try:
        novo_demais = os.path.getmtime(d) > corte
    except OSError:
        continue
    if pid_vivo or novo_demais:
        guardados += 1
        continue
    try:
        for raiz, _, arqs in os.walk(d):
            for a in arqs:
                try: bytes_ += os.path.getsize(os.path.join(raiz, a))
                except OSError: pass
        if not so_ver: shutil.rmtree(d, ignore_errors=True)
        apagados += 1
    except OSError:
        pass
print("  %d diretorios de teste soltos, %d MiB%s" %
      (apagados, bytes_ // (1024 * 1024), " (nao apagados: --ver)" if so_ver else ""))
if guardados:
    print("  %d guardados: PID vivo ou mexidos ha menos de 30 min" % guardados)
FIM'''
assert s.count(velho)==1
open(p,'w',encoding='utf-8').write(s.replace(velho,novo))
print("varredura dos temporarios refeita: lista de processos levantada UMA vez")

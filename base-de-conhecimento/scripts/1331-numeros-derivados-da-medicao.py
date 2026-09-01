# Numeros derivados da medicao
# 01/09 18:35

import json
d = json.load(open("bancada/comparacao/um-milhao.json"))
f, n, ops = d["fases"], d["linhas"], d["operacoes_por_fase_pontual"]
piso = d["piso_do_mysql_s"]["mediana_s"]
def m(fase, mot): return f[fase][mot]["mediana_s"]
print(f"{'fase':<10} {'PhxSql':>9} {'SQLite':>9} {'MySQL':>9}   {'MySQL-piso':>10}")
for fase in ("inserir","buscar","atualizar","excluir"):
    liq = m(fase,"mysql") - (piso if fase != "inserir" else 0)
    print(f"{fase:<10} {m(fase,'phxsql'):9.3f} {m(fase,'sqlite'):9.3f} {m(fase,'mysql'):9.3f}   {liq:10.3f}")
print()
print("taxa de insercao, linhas/s:")
for mot in ("phxsql","sqlite","mysql"):
    print(f"  {mot:<8} {n/m('inserir',mot):>10,.0f}".replace(",","."))
print()
print("por operacao nas fases pontuais, µs:")
for fase in ("buscar","atualizar","excluir"):
    p,s,my = (m(fase,x)/ops*1e6 for x in ("phxsql","sqlite","mysql"))
    liq = (m(fase,"mysql")-piso)/ops*1e6
    print(f"  {fase:<10} phx {p:7.1f}  sqlite {s:7.1f}  mysql {my:7.1f} (liquido {liq:6.1f})")
print()
print("razoes (quantas vezes o outro custa o nosso):")
for fase in ("inserir","buscar","atualizar","excluir"):
    p = m(fase,"phxsql")
    liq = m(fase,"mysql") - (piso if fase != "inserir" else 0)
    print(f"  {fase:<10} SQLite {m(fase,'sqlite')/p:5.2f}x   MySQL {m(fase,'mysql')/p:6.2f}x   MySQL liquido {liq/p:6.2f}x")
print()
print("piso do MySQL como fatia da barra dele:")
for fase in ("buscar","atualizar","excluir"):
    print(f"  {fase:<10} {piso/m(fase,'mysql')*100:5.1f}%")
print()
mib = lambda b: b/1048576
db = d["disco_bytes"]
print("disco depois da carga:")
for k,v in db.items(): print(f"  {k:<8} {mib(v):8.1f} MiB")
print(f"  PhxSql / SQLite = {db['phxsql']/db['sqlite']:.2f}x ; PhxSql / MySQL = {db['phxsql']/db['mysql']:.2f}x")
print()
print("sqlite 2ind contra rowid (medianas):")
for fase in ("inserir","buscar","atualizar","excluir"):
    a = d["sqlite_2ind"][fase]["mediana_s"]; b = m(fase,"sqlite")
    print(f"  {fase:<10} rowid {b:7.3f}  2ind {a:7.3f}   ({a/b:.2f}x)")

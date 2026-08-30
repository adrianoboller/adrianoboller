# Retest the five formats with typed columns
# 28/08 19:24

import json, socket
s = socket.create_connection(("127.0.0.1", 5741)); f = s.makefile("rwb")
def op(p):
    p.setdefault("token","prova")
    f.write((json.dumps(p)+"\n").encode()); f.flush()
    return json.loads(f.readline().decode())
op({"op":"login","usuario":"adm","senha":"segredo1"})
op({"op":"criar_database","database":"loja"})
cols=[{"nome":"id","tipo":"Int4","obrigatoria":True},
      {"nome":"nome","tipo":"Str(40)","obrigatoria":True},
      {"nome":"cidade","tipo":"Str(30)"},
      {"nome":"limite","tipo":"Decimal(15,2)"},
      {"nome":"ativo","tipo":"Bool"},
      {"nome":"cadastro","tipo":"Date"}]
cargas = {
 "csv":  "id;nome;cidade;limite;ativo;cadastro\n1;Adriano;Blumenau;1500,50;sim;2026-08-28\n"
         "2;\"Silva, Souza & Cia\";Itajai;;nao;2026-01-15\n",
 "txt":  "id\tnome\tcidade\tlimite\tativo\n1\tAdriano\tBlumenau\t1500.50\ttrue\n",
 "json": '[{"id":1,"nome":"Adriano","cidade":"Blumenau","limite":"1500.50","ativo":true}]',
 "xml":  '<?xml version="1.0"?><linhas><linha><id>1</id><nome>Silva &amp; Souza</nome>'
         '<cidade>Itajai</cidade><limite>99,90</limite><ativo>1</ativo></linha></linhas>',
 "html": '<table><thead><tr><th>id</th><th>nome</th><th>limite</th></tr></thead>'
         '<tbody><tr><td>1</td><td><b>Adriano</b></td><td>2.000,00</td></tr></tbody></table>',
}
for fmt, texto in cargas.items():
    tab=f"c_{fmt}"
    op({"op":"criar_tabela","database":"loja","tabela":tab,"colunas":cols})
    r=op({"op":"inserir_lote","database":"loja","tabela":tab,"texto":texto})
    r=r.get("resultado", r)
    v=op({"op":"varrer","database":"loja","tabela":tab,"max":10})["resultado"]
    print(f"{fmt:5} fmt={r.get('formato'):5} gravadas={r.get('gravadas')} recusadas={r.get('recusadas')}")
    for l in v["linhas"]:
        print("     ", {k:l[k] for k in ("id","nome","cidade","limite","ativo","cadastro") if k in l})
    if r.get("erros"): print("      erros:", r["erros"])

# Test all five paste formats and error handling
# 28/08 19:22

import json, socket
s = socket.create_connection(("127.0.0.1", 5741)); f = s.makefile("rwb")
def op(p):
    p.setdefault("token","prova")
    f.write((json.dumps(p)+"\n").encode()); f.flush()
    return json.loads(f.readline().decode())
op({"op":"login","usuario":"adm","senha":"segredo1"})
cols=[{"nome":"id","tipo":"Int4","obrigatoria":True},
      {"nome":"nome","tipo":"Str(40)","obrigatoria":True},
      {"nome":"cidade","tipo":"Str(30)"}]

cargas = {
 "csv":  "id;nome;cidade\n1;Adriano;Blumenau\n2;Maria;Itajai\n3;\"Silva, Souza & Cia\";Joinville\n",
 "txt":  "id\tnome\tcidade\n1\tAdriano\tBlumenau\n2\tMaria\tItajai\n",
 "json": '[{"id":1,"nome":"Adriano","cidade":"Blumenau"},{"id":2,"nome":"Maria","cidade":"Itajai"}]',
 "xml":  '<?xml version="1.0"?><linhas><linha><id>1</id><nome>Adriano</nome><cidade>Blumenau</cidade></linha>'
         '<linha><id>2</id><nome>Silva &amp; Souza</nome><cidade>Itajai</cidade></linha></linhas>',
 "html": '<table><thead><tr><th>id</th><th>nome</th><th>cidade</th></tr></thead><tbody>'
         '<tr><td>1</td><td><b>Adriano</b></td><td>Blumenau</td></tr>'
         '<tr><td>2</td><td>Silva &amp; Souza</td><td>Itajai</td></tr></tbody></table>',
}
for fmt, texto in cargas.items():
    tab = f"c_{fmt}"
    op({"op":"criar_tabela","database":"loja","tabela":tab,"colunas":cols})
    # sem dizer o formato: adivinha
    r = op({"op":"inserir_lote","database":"loja","tabela":tab,"texto":texto})["resultado"]
    v = op({"op":"varrer","database":"loja","tabela":tab,"max":10})["resultado"]
    nomes = [l["nome"] for l in v["linhas"]]
    print(f"{fmt:5} adivinhou={r['formato']:5} gravadas={r['gravadas']}  ->  {nomes}")

# um erro no meio, com parar_no_erro
op({"op":"criar_tabela","database":"loja","tabela":"c_erro","colunas":cols})
r = op({"op":"inserir_lote","database":"loja","tabela":"c_erro",
        "texto":"id;nome;cidade\n1;Ana;Blumenau\n2;;Itajai\n3;Bia;Joinville\n"})["resultado"]
print("\ncom erro no meio (parar):", {k:r[k] for k in ("recebidas","gravadas","recusadas")})
print("  erros:", r["erros"])
print("  aviso:", r["aviso"])
r = op({"op":"inserir_lote","database":"loja","tabela":"c_erro","parar_no_erro":False,
        "texto":"id;nome;cidade\n11;Ana;Blumenau\n12;;Itajai\n13;Bia;Joinville\n"})["resultado"]
print("com erro no meio (seguir):", {k:r[k] for k in ("recebidas","gravadas","recusadas")})
print("  erros:", r["erros"])

#!/usr/bin/env python3
"""As atividades de GESTAO DO BANCO, medidas contra o motor vivo.

    python3 bancada/gestao/medir.py

Responde a pergunta do dono -- «liste os recursos com ok, parcial e planejado»
-- sem consultar o `PENDENCIAS.md` nem a memoria. Cada linha desta bancada
SOBE o servidor, exercita a atividade e olha o EFEITO.

# As tres regras que este medidor herda das bancadas anteriores

1. **Efeito, nunca «aceitou».** Campo desconhecido pode ser engolido em
   silencio -- foi o que o `bancada/comparativo/` achou em quatro campos de
   esquema. Entao insere-se e LE-SE de volta; exclui-se e CONFERE-SE que
   sumiu.
2. **Controle antes do veredito.** Recusa sozinha nao prova nada: uma tabela
   que nao nasceu recusa tudo. Cada sonda que espera uma recusa traz o caso
   legitimo ao lado.
3. **O leitor se prova primeiro.** Grava-se um valor conhecido e exige-se
   le-lo de volta antes de qualquer veredito -- senao «nao achei» e «nao sei
   olhar» viram a mesma frase.

# O que ele NAO mede, e diz que nao mede

Replicacao e cluster pedem varios servidores e ja tem bancada propria
(`bancada/replicacao/`, `bancada/cluster/`). Este medidor LE o
`resultados.json` de cada uma **com a data**, e marca a linha como NAO MEDIDA
quando o arquivo nao existe -- bancada que nao rodou aparece como nao tendo
rodado.
"""

import datetime
import json
import pathlib
import subprocess
import sys
import tempfile

AQUI = pathlib.Path(__file__).resolve().parent
RAIZ = AQUI.parents[1]
ALVO = AQUI / "resultados.json"
PORTA = 7741

sys.path.insert(0, str(RAIZ / "bancada" / "utilizacao-padrao"))

OK, PARCIAL, PLANEJADO = "ok", "parcial", "planejado"


def compilar():
    """Binario velho mede o passado -- petrea desta casa."""
    r = subprocess.run(["flock", "/tmp/phx-cargo.lock", "cargo", "build",
                        "--release", "-p", "phxsql-server"],
                       cwd=RAIZ, capture_output=True, text=True)
    if r.returncode:
        raise SystemExit("nao compilou:\n" + r.stderr[-1500:])


# As operacoes que este medidor chama. O portao abaixo confere CADA UMA contra
# o catalogo do servidor -- nome e parametro obrigatorio -- antes de medir.
#
# Ele existe porque a primeira corrida publicou CINCO «planejado» que eram
# defeito meu: inventei `contar` e `excluir_de_vez` (nao existem), mandei
# `linha` onde o contrato pede `valores`, chamei `bulkinsert` (que e o modo de
# carga, nao o insere-muitos) achando que era `inserir_lote`, e esqueci o
# `destino` obrigatorio do `backup`. Cada erro desses voltou como recusa, e
# recusa nao lida vira AUSENCIA PUBLICADA -- pela terceira vez nesta sessao.
CONTRATO = {
    "criar_database": {"database"},
    "criar_tabela": {"database", "tabela", "colunas"},
    "inserir": {"database", "tabela", "valores"},
    "inserir_lote": {"database", "tabela"},
    "ler": {"database", "tabela", "rowid"},
    "varrer": {"database", "tabela"},
    "buscar": {"database", "tabela", "indice", "chave"},
    "atualizar": {"database", "tabela", "rowid"},
    "excluir": {"database", "tabela", "rowid"},
    "restaurar": {"database", "tabela", "rowid"},
    "lixeira": {"database", "tabela"},
    "declarar_fk": {"database", "tabela", "nome", "colunas", "tabela_ref"},
    "sql": {"database", "texto"},
    "backup": {"destino"},
    "restaurar_backup": {"origem"},
}


def conferir_contrato(c):
    """Pergunta ao catalogo do SERVIDOR se cada operacao existe e o que exige.

    Duas reprovacoes, e as duas param o medidor:
    - operacao que o catalogo nao conhece: eu inventei o nome;
    - parametro obrigatorio que eu nao mando: a recusa seria minha, nao dele.
    """
    problemas = []
    for op, meus in CONTRATO.items():
        r = c.fala({"op": "catalogo", "database": "g", "operacao": op})
        o = (r.get("resultado") or {}).get("operacao")
        if not o:
            problemas.append(f"{op}: o catalogo nao conhece esta operacao")
            continue
        obrig = {x["nome"] for x in (o.get("parametros") or [])
                 if x.get("obrigatorio")}
        faltam = obrig - meus
        if faltam:
            problemas.append(f"{op}: nao mando o(s) obrigatorio(s) {sorted(faltam)}")
    if problemas:
        raise SystemExit("CONTRATO QUEBRADO -- o medidor esta errado, nao o "
                         "motor:\n  " + "\n  ".join(problemas))
    return len(CONTRATO)


def medir_no_motor(base):
    from oficina import Conexao, subir  # noqa: E402
    p = subir(base, PORTA)
    saida = {}
    try:
        c = Conexao(PORTA)
        c.fala({"op": "criar_database", "database": "g"})
        saida["__contrato__"] = conferir_contrato(c)

        def corpo(r):
            return r.get("resultado") or {}

        def cria(tab, colunas, indices=None, exigir=True):
            r = c.fala({"op": "criar_tabela", "database": "g", "tabela": tab,
                        "colunas": colunas,
                        "indices": indices or [{"nome": "pk",
                                                "colunas": [colunas[0]["nome"]],
                                                "unico": True}]})
            if exigir and not r.get("ok"):
                raise SystemExit(f"MESA NAO POSTA: {tab} -- {r.get('erro')}")
            return r

        def linha(tab, rowid):
            return corpo(c.fala({"op": "ler", "database": "g",
                                 "tabela": tab, "rowid": rowid}))

        COLS = [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                {"nome": "nome", "tipo": "Str(40)"},
                {"nome": "valor", "tipo": "Int8"}]

        # --- O CONTROLE DO LEITOR, antes de qualquer veredito.
        cria("ctl", COLS)
        c.fala({"op": "inserir", "database": "g", "tabela": "ctl",
                "valores": {"id": 1, "nome": "prova", "valor": 42}})
        if linha("ctl", 1).get("valor") != 42:
            raise SystemExit("LEITOR QUEBRADO: gravei 42 e nao li 42 de volta")

        # ------------------------------------------------ INSERT (protocolo)
        cria("t", COLS)
        r = c.fala({"op": "inserir", "database": "g", "tabela": "t",
                    "valores": {"id": 1, "nome": "ana", "valor": 10}})
        veio = linha("t", 1)
        saida["insert_protocolo"] = (
            (OK, "inseriu e a linha voltou com o valor gravado")
            if r.get("ok") and veio.get("nome") == "ana" else
            (PLANEJADO, f"nao voltou: {veio or r.get('erro')}"))

        # --------------------------------------------------- INSERT em lote
        lote = [{"id": i, "nome": f"n{i}", "valor": i} for i in range(2, 202)]
        d = corpo(c.fala({"op": "inserir_lote", "database": "g", "tabela": "t",
                          "linhas": lote}))
        # `registros` do varrer e a contagem da tabela -- nao ha op de contar.
        total = corpo(c.fala({"op": "varrer", "database": "g", "tabela": "t",
                              "max": 1})).get("registros")
        saida["insert_lote"] = (
            (OK, f"{d.get('gravadas')} gravadas, {d.get('recusadas')} recusadas; "
                 f"a tabela ficou com {total}")
            if d.get("gravadas") == 200 and total == 201 else
            (PLANEJADO, f"gravadas={d.get('gravadas')!r} total={total!r}"))

        # ------------------------------------------------ SELECT (protocolo)
        v = corpo(c.fala({"op": "varrer", "database": "g", "tabela": "t",
                          "max": 5}))
        b = corpo(c.fala({"op": "buscar", "database": "g", "tabela": "t",
                          "indice": "pk", "chave": [7]}))
        saida["select_protocolo"] = (
            (OK, f"varrer devolveu {len(v.get('linhas') or [])} linhas; "
                 f"buscar pelo indice achou {b.get('encontrados')}")
            if v.get("linhas") and b.get("encontrados") == 1 else
            (PLANEJADO, f"varrer={v!r} buscar={b!r}"))

        # --- SELECT com WHERE: o filtro tem de REMOVER linha, nao so aceitar
        w = corpo(c.fala({"op": "varrer", "database": "g", "tabela": "t",
                          "max": 500, "onde": [{"coluna": "valor",
                                                "op": ">", "valor": 100}]}))
        n_w = len(w.get("linhas") or [])
        saida["select_where"] = (
            (OK, f"o WHERE cortou para {n_w} de 201")
            if 0 < n_w < 201 else
            (PLANEJADO, f"o filtro nao cortou nada: {n_w} de 201"))

        # ------------------------------------------------ UPDATE (protocolo)
        r = c.fala({"op": "atualizar", "database": "g", "tabela": "t",
                    "rowid": 1, "valores": {"id": 1, "nome": "ANA", "valor": 99}})
        veio = linha("t", 1)
        saida["update_protocolo"] = (
            (OK, "alterou e a leitura seguinte trouxe o valor novo")
            if r.get("ok") and veio.get("valor") == 99 else
            (PLANEJADO, f"nao mudou: {veio!r}"))

        # ------------------------------- DELETE suave, e o que ele preserva
        r = c.fala({"op": "excluir", "database": "g", "tabela": "t",
                    "rowid": 2, "motivo": "prova da bancada"})
        v = corpo(c.fala({"op": "varrer", "database": "g", "tabela": "t",
                          "max": 1}))
        lix = corpo(c.fala({"op": "lixeira", "database": "g", "tabela": "t"}))
        n_sem, n_lix = v.get("visiveis"), lix.get("total")
        # PREVISAO MINHA, DERRUBADA PELA MEDICAO: eu afirmei que o excluir
        # suave manda a linha para a lixeira. Nao manda, e o codigo diz por
        # que -- `excluir_suave` so MARCA e registra o motivo no `.reason`; a
        # linha continua inteira no `.reg`, entao nao ha o que preservar. A
        # lixeira e a rede do excluir que DESTROI (ver a linha de baixo).
        saida["delete_suave"] = (
            (OK, f"marca e some da grade: {n_sem} visiveis de "
                 f"{v.get('registros')} registros, e a lixeira NAO e usada "
                 f"({n_lix}) -- a linha continua inteira no `.reg`")
            if r.get("ok") and n_lix == 0 and n_sem == 200 else
            (PLANEJADO, f"visiveis={n_sem!r} lixeira={n_lix!r} "
                        f"erro={r.get('erro')}"))

        # ------------------------------------------------------- RESTAURAR
        r = c.fala({"op": "restaurar", "database": "g", "tabela": "t",
                    "rowid": 2})
        vis = corpo(c.fala({"op": "varrer", "database": "g", "tabela": "t",
                            "max": 1})).get("visiveis")
        lix2 = corpo(c.fala({"op": "lixeira", "database": "g",
                             "tabela": "t"})).get("total")
        saida["delete_restaurar"] = (
            (OK, f"voltou com o MESMO rowid: {vis} visiveis, lixeira em {lix2}")
            if r.get("ok") and vis == 201 and lix2 == 0 else
            (PLANEJADO, f"visiveis={vis!r} lixeira={lix2!r}"))

        # ------------------------------------------------- DELETE de vez
        r = c.fala({"op": "excluir", "database": "g", "tabela": "t",
                    "rowid": 3, "motivo": "prova da bancada", "fisico": True})
        dep = corpo(c.fala({"op": "varrer", "database": "g", "tabela": "t",
                            "max": 1}))
        lix3 = corpo(c.fala({"op": "lixeira", "database": "g",
                             "tabela": "t"})).get("total")
        saida["delete_de_vez"] = (
            (OK, f"destroi a linha ({dep.get('visiveis')} visiveis) e guarda "
                 f"o conteudo inteiro na lixeira antes ({lix3}) -- inclusive "
                 f"o `.bin` e o `.memo`, que esta mesma exclusao vai liberar")
            if r.get("ok") and dep.get("visiveis") == 200 and lix3 == 1 else
            (PLANEJADO, f"visiveis={dep.get('visiveis')!r} lixeira={lix3!r} "
                        f"erro={r.get('erro')}"))

        # --- A REGRA PRIMORDIAL: nunca se mata o pai que tem filhos.
        # Controle junto, senao a recusa nao prova nada.
        cria("mae", [{"nome": "id", "tipo": "Int8", "obrigatoria": True}])
        cria("filha", [{"nome": "id", "tipo": "Int8", "obrigatoria": True},
                       {"nome": "mae_id", "tipo": "Int8"}],
             [{"nome": "pk", "colunas": ["id"], "unico": True},
              {"nome": "por_mae", "colunas": ["mae_id"]}])
        c.fala({"op": "declarar_fk", "database": "g", "tabela": "filha",
                "nome": "fk_mae", "colunas": ["mae_id"],
                "tabela_ref": "mae", "colunas_ref": ["id"]})
        c.fala({"op": "inserir", "database": "g", "tabela": "mae",
                "valores": {"id": 1}})
        c.fala({"op": "inserir", "database": "g", "tabela": "mae",
                "valores": {"id": 2}})
        c.fala({"op": "inserir", "database": "g", "tabela": "filha",
                "valores": {"id": 1, "mae_id": 1}})
        com_filha = c.fala({"op": "excluir", "database": "g", "tabela": "mae",
                            "rowid": 1, "motivo": "prova"})
        sem_filha = c.fala({"op": "excluir", "database": "g", "tabela": "mae",
                            "rowid": 2, "motivo": "prova"})
        if not sem_filha.get("ok"):
            saida["integridade"] = (
                PLANEJADO, "veredito ANULADO pelo controle: a mae SEM filha "
                           f"tambem foi recusada ({sem_filha.get('erro')})")
        elif com_filha.get("ok"):
            saida["integridade"] = (PLANEJADO, "matou a mae que tem filha")
        else:
            saida["integridade"] = (
                OK, "recusou a mae COM filha e deixou passar a SEM filha")

        # ---------------------------------------------- a camada SQL, hoje
        for chave, sql in (("sql_select", "SELECT * FROM t"),
                           ("sql_insert", "INSERT INTO t (id) VALUES (900)"),
                           ("sql_update", "UPDATE t SET valor = 1 WHERE id = 1"),
                           ("sql_delete", "DELETE FROM t WHERE id = 1")):
            r = c.fala({"op": "sql", "database": "g", "texto": sql})
            saida[chave] = ((OK, "a camada SQL executa") if r.get("ok") else
                            (PLANEJADO, (r.get("erro") or "")[:110]))

        # ------------------------------------------------------- BACKUP
        destino = str(pathlib.Path(base) / "bkp")
        r = c.fala({"op": "backup", "database": "g", "destino": destino,
                    "zip": True})
        d = corpo(r)
        arq = d.get("arquivo") or d.get("destino") or d.get("caminho")
        saida["backup"] = (
            (OK, f"gerou {arq}") if r.get("ok") and arq else
            (PLANEJADO, (r.get("erro") or json.dumps(d))[:110]))

        # RESTAURAR nao se prova pelo «ok»: restaura-se com OUTRO nome e le-se
        # uma linha conhecida de dentro da copia. Backup que grava e nao volta
        # e o pior tipo de backup, porque parece pronto.
        rr = c.fala({"op": "restaurar_backup", "origem": arq,
                     "database": "g_voltou", "confirmar": True})
        lida = corpo(c.fala({"op": "ler", "database": "g_voltou",
                             "tabela": "t", "rowid": 1}))
        saida["restaurar_backup"] = (
            (OK, f"restaurou com outro nome e a linha 1 voltou: {lida.get('nome')!r}")
            if rr.get("ok") and lida.get("nome") == "ANA" else
            (PLANEJADO, f"nao voltou: {(rr.get('erro') or '')[:80]} lida={lida!r}"))
        return saida
    finally:
        p.kill()
        p.wait()


def das_outras_bancadas():
    """Replicacao e cluster: le o resultado DELAS, com a data, ou diz que falta."""
    fora = {}
    for chave, rel, comando in (
            ("replicacao", "bancada/replicacao/resultados.json",
             "python3 bancada/replicacao/montar.py && …/medir.py"),
            ("cluster", "bancada/cluster/resultados.json",
             "python3 bancada/cluster/provar.py")):
        p = RAIZ / rel
        if not p.exists():
            fora[chave] = {"estado": PLANEJADO, "quando": None,
                           "nota": f"NAO MEDIDA -- rode `{comando}`"}
            continue
        d = json.loads(p.read_text(encoding="utf-8"))
        quando = (d.get("quando") or d.get("medido_em") or d.get("data")
                  or datetime.datetime.fromtimestamp(p.stat().st_mtime)
                  .strftime("%Y-%m-%d %H:%M"))
        do_mtime = not (d.get("quando") or d.get("medido_em") or d.get("data"))
        fora[chave] = {"estado": OK, "quando": str(quando)[:19],
                       "data_do_mtime": do_mtime, "bruto": d}
    return fora


TITULOS = {
    "select_protocolo": "SELECT — `varrer` e `buscar` pelo índice",
    "select_where": "SELECT com filtro — `varrer` com `onde`",
    "insert_protocolo": "INSERT — `inserir`",
    "insert_lote": "INSERT em lote — `inserir_lote`",
    "update_protocolo": "UPDATE — `atualizar`",
    "delete_suave": "DELETE suave — some da grade, fica no arquivo",
    "delete_restaurar": "DELETE suave → `restaurar`",
    "delete_de_vez": "DELETE de vez — `excluir` com `fisico`",
    "integridade": "Integridade: nunca matar o pai que tem filhos",
    "sql_select": "SELECT pela camada SQL",
    "sql_insert": "INSERT pela camada SQL",
    "sql_update": "UPDATE pela camada SQL",
    "sql_delete": "DELETE pela camada SQL",
    "backup": "Backup do banco pelo protocolo",
    "restaurar_backup": "Restauração do backup, provada por leitura",
}


def main():
    print("Gestao do banco — medindo contra o motor vivo\n")
    compilar()
    print("  phxsqld recompilado antes de medir")
    with tempfile.TemporaryDirectory(prefix="phx-gestao-") as base:
        m = medir_no_motor(base)
    n_contrato = m.pop("__contrato__", 0)
    fora = das_outras_bancadas()

    linhas = [{"chave": k, "titulo": TITULOS[k], "estado": v[0], "prova": v[1]}
              for k, v in m.items() if k in TITULOS]
    d = {
        "quando": datetime.datetime.now().isoformat(timespec="seconds"),
        "versao": subprocess.run([str(RAIZ / "target/release/phxsqld"), "--version"],
                                 capture_output=True, text=True).stdout.strip(),
        "operacoes_conferidas_no_catalogo": n_contrato,
        "linhas": linhas,
        "de_outras_bancadas": fora,
    }
    ALVO.write_text(json.dumps(d, indent=2, ensure_ascii=False) + "\n",
                    encoding="utf-8")

    largura = max(len(l["titulo"]) for l in linhas)
    for l in linhas:
        print(f"  {l['titulo']:{largura}}  {l['estado']:9}  {l['prova'][:60]}")
    for k, v in fora.items():
        print(f"  {k:{largura}}  {v['estado']:9}  medida em {v['quando']}")
    print(f"\ngravado: {ALVO}")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Uma tabela COMPLEXA com e sem senha: o que a cifra custa em disco e em tempo.

    flock /tmp/phx-cargo.lock cargo build --release --bin phxsqld
    python3 bancada/cifra/com-e-sem-senha.py [n_linhas]

Pedido do dono: *«uma tabela complexa com 1.000.000 de registros com e sem
senha, mostrando o tamanho em disco e os tempos de select, insert, update e
delete»*.

# As tres decisoes que fazem o numero querer dizer alguma coisa

**Mesmo trabalho, e nao so mesma pergunta.** E a primeira regra da
`bancada/LEIA-ME.md`, e ela ja custou dois numeros errados nesta casa em lados
opostos. Aqui os dois lados recebem o MESMO esquema, as MESMAS linhas na mesma
ordem e as MESMAS operacoes; o unico campo que difere entre as duas corridas e
`cifra` no `config.json`. Nem o `max_linhas` muda.

**Em serie, com limpeza entre os lados.** Medido antes de rodar, extrapolando a
corrida de 20.000 da `bancada/utilizacao-padrao`: a tabela `com` ocupa
29,2 MiB em 20.000 linhas, entao 1,46 GiB em 1.000.000 -- e os dois lados
juntos passariam de 2,9 GiB. Guardar os dois ao mesmo tempo num disco que hoje
tem 7,7 GiB livres e desnecessario: o lado que ja foi medido sai antes do
proximo entrar, e o pico vira o de UM lado. *Medir a premissa vem antes de
rodar o item* -- e esta premissa mudou o desenho da bancada.

**Carga que nao confere o que gravou mede o soquete.** A leitura de volta
compara campo a campo, e o `.bin` byte a byte. Sem isso, uma cifra que
gravasse lixo passaria com tempo excelente.

# O que ele NAO mede, e o motivo

Nao mede o custo de ABRIR com senha (o PBKDF2 de 210.000 voltas, ~298 ms).
Aquele custo e por (sal, iteracoes) e o cofre o guarda em cache, entao ele
aparece uma vez por arquivo e nao por operacao -- misturar os dois numa media
por linha esconderia os dois. Ele esta medido em `--example custo-da-cifra`.

A ultima linha e `RESULTADO <json>`, e `resultado.json` fica ao lado.
"""
import datetime
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import time

RAIZ = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
UTIL = os.path.join(RAIZ, "bancada", "utilizacao-padrao")
sys.path.insert(0, UTIL)


def _modulo(nome, caminho):
    spec = importlib.util.spec_from_file_location(nome, caminho)
    m = importlib.util.module_from_spec(spec)
    sys.modules[nome] = m
    spec.loader.exec_module(m)
    return m


oficina = _modulo("oficina", os.path.join(UTIL, "oficina.py"))
# O `medir.py` da utilizacao padrao e reusado inteiro: esquema, linhas e
# conferencia. Escrever um segundo gerador de linhas seria abrir a porta para
# os dois divergirem, e ai «com senha e mais lento» poderia ser «as linhas eram
# outras».
medir = _modulo("medir_util", os.path.join(UTIL, "medir.py"))

PORTA = int(os.environ.get("PHX_PORTA_CIFRA", "6371"))
BASE = os.environ.get("PHX_CIFRA_BASE", "/tmp/phx-cifra-%d" % os.getpid())
LADO = "com"          # 11 colunas + Memo + Bin: a tabela complexa do pedido

# AS COLUNAS MARCADAS, e por que a bancada nao funciona sem elas.
#
# Medido em 05/09/2026, e foi o achado que salvou o numero: com `cifra.ligada`
# e senha no `config.json`, os dois lados sairam com o disco IDENTICO --
# 4.897.786 contra 4.897.978 bytes em 2.000 linhas. A cifra estava ligada e o
# servidor a enxergava (`{"op":"config"}` devolvia `ligada: true`, 210.000
# iteracoes, modo `aead`). Nada era cifrado porque NADA PEDIU.
#
# `RegFile::selar_externo` devolve o dado intacto quando a coluna nao esta
# marcada (`reg.rs:1591`), e `externa_marcada` exige `dado_pessoal.e_pessoal()`
# (`reg.rs:1747`). O cabecalho do modulo ja dizia: *«cifra-se so a coluna
# marcada como dado pessoal, e nao [a tabela inteira]»*.
#
# Entao «com e sem senha» so quer dizer alguma coisa se houver coluna marcada.
# Os DOIS lados marcam as mesmas colunas com o mesmo grau -- so o
# `cifra.ligada` difere. Marcar so no lado cifrado compararia esquemas
# diferentes, que e o erro que a `bancada/LEIA-ME.md` proibe.
MARCADAS = {"observacao": "sensivel", "foto": "sensivel"}
SENHA_DO_BANCO = "senha-de-bancada-nao-usar-em-producao"
LOTE = 5000
PAGINA = 2000


# O `medir.colunas_do_lado` e envolvido, e nao copiado: uma segunda lista de
# colunas aqui divergiria da de la no dia em que alguem mexesse numa so, e a
# bancada passaria a comparar duas tabelas diferentes sem avisar.
_colunas_original = medir.colunas_do_lado


def colunas_marcadas(lado):
    cols = _colunas_original(lado)
    for c in cols:
        if c["nome"] in MARCADAS:
            c["dado_pessoal"] = MARCADAS[c["nome"]]
    return cols


medir.colunas_do_lado = colunas_marcadas


def cfg_do_lado(cifrado):
    """O config de fabrica, mais `cifra` num dos lados. Nada mais muda."""
    c = oficina.config(PORTA, max_linhas=PAGINA + 10)
    if cifrado:
        # `iteracoes` fica no PADRAO de propósito: baixa-lo barataria o abrir e
        # publicaria um numero que producao nenhuma tem.
        c["cifra"] = {"ligada": True, "senha": SENHA_DO_BANCO}
    return c


def cronometrar(fn):
    t = time.perf_counter()
    r = fn()
    return (time.perf_counter() - t), r


def uma_corrida(cifrado, n):
    """Sobe um servidor, faz o caminho inteiro, mede, derruba e APAGA."""
    base = "%s-%s" % (BASE, "com-senha" if cifrado else "sem-senha")
    subprocess.run(["rm", "-rf", base], check=False)
    p = oficina.subir(base, PORTA, cfg_do_lado(cifrado))
    saida = {"cifrado": cifrado, "linhas": n}
    try:
        c = oficina.Conexao(PORTA)
        medir.criar_tudo(c, n)

        # ---- INSERT, em lotes, como um cliente de carga faz
        c.zerar()
        dt, _ = cronometrar(lambda: medir.carregar(c, LADO, n))
        saida["insert"] = {"s": round(dt, 3), "linhas_por_s": int(n / dt),
                           "ms_no_servidor": round(c.ms, 1),
                           "fio_enviado": c.enviados}

        # ---- SELECT: le tudo de volta paginado, e CONFERE
        c.zerar()
        dt, volta = cronometrar(lambda: medir.ler_de_volta(c, LADO, n))
        saida["select"] = {"s": round(dt, 3), "linhas_por_s": int(n / dt),
                           "ms_no_servidor": round(c.ms, 1),
                           "divergentes": volta.get("divergentes"),
                           "linhas_lidas": volta.get("linhas_lidas")}
        if volta.get("divergentes"):
            saida["ERRO"] = "a leitura divergiu: os tempos nao valem"

        # ---- os rowids, COLHIDOS FORA DO CRONOMETRO
        #
        # `atualizar` e `excluir` vao por `rowid`, e nao por indice. Buscar
        # dentro do laco mediria uma busca junto com a alteracao, e o numero
        # publicado seria a soma das duas sem dizer isso. Aqui a varredura
        # acontece antes, com o relogio parado.
        #
        # E os rowids se COLHEM, nao se supoem: numa tabela recem-carregada
        # eles saem 1..n, mas «sai assim hoje» nao e garantia -- o `.reg` nunca
        # reaproveita slot, e uma bancada que assume a numeracao quebraria
        # calada no dia em que rodasse sobre tabela usada.
        quantas = min(n, 20_000)
        rowids = []
        pos = 0
        while len(rowids) < quantas:
            r = c.ok({"op": "varrer", "database": medir.DB, "tabela": LADO,
                      "limite": min(PAGINA, quantas - len(rowids)),
                      "desde_rownum": pos})
            linhas = r.get("linhas", [])
            if not linhas:
                break
            # O CODIGO vem junto porque o `atualizar` precisa da linha
            # INTEIRA (ver abaixo), e e do codigo que sai o indice dela.
            rowids += [(l["rowid"], l["codigo"]) for l in linhas]
            pos = linhas[-1]["rownum"]
        saida["rowids_colhidos"] = len(rowids)

        # ---- UPDATE
        c.zerar()

        # O `atualizar` SUBSTITUI a linha, nao mescla: medido em 05/09/2026,
        # `{"valores": {"saldo": "1.23"}}` volta
        # «coluna filial e obrigatoria e recebeu NULL». So as colunas de
        # sistema sao preservadas quando ausentes (`servidor.rs:11465`).
        # Mandar a linha inteira e o caminho certo, e e o que a tela faz ao
        # salvar uma ficha -- entao o numero medido aqui e o do uso real.
        def atualiza():
            for rid, cod in rowids:
                l = medir.linha(int(cod[2:]), LADO)
                l["saldo"] = "1.23"
                c.ok({"op": "atualizar", "database": medir.DB, "tabela": LADO,
                      "rowid": rid, "valores": l})
        dt, _ = cronometrar(atualiza)
        saida["update"] = {"s": round(dt, 3), "linhas": len(rowids),
                           "por_s": int(len(rowids) / dt),
                           "ms_no_servidor": round(c.ms, 1)}

        # ---- disco ANTES do delete: e o tamanho da tabela cheia
        saida["disco"] = oficina.bytes_no_disco(base, medir.DB, LADO)
        saida["disco"]["total"] = sum(saida["disco"].values())

        # ---- DELETE de vez, a mesma fracao
        c.zerar()

        def exclui():
            for rid, _cod in rowids:
                c.ok({"op": "excluir", "database": medir.DB, "tabela": LADO,
                      "rowid": rid, "fisico": True, "motivo": "bancada"})
        dt, _ = cronometrar(exclui)
        saida["delete"] = {"s": round(dt, 3), "linhas": len(rowids),
                           "por_s": int(len(rowids) / dt),
                           "ms_no_servidor": round(c.ms, 1)}
    finally:
        try:
            p.terminate()
            p.wait(timeout=20)
        except Exception:
            pass
        tam = saida.get("disco", {}).get("total", 0)
        # A limpeza e o que mantem o pico em UM lado. Sem ela os dois lados
        # somados passariam de 2,9 GiB em 1.000.000 de linhas.
        shutil.rmtree(base, ignore_errors=True)
        saida["apagado_apos_medir"] = True
        saida["_tam_medido"] = tam
    return saida


def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 1_000_000
    ocupado, quem = oficina.portao_de_medicao()
    d = {"quando": datetime.datetime.now().isoformat(timespec="seconds"),
         "linhas": n, "lado": LADO, "por_lote": LOTE,
         "esta_medindo_antes": ocupado, "esta_medindo_quem_antes": quem}
    if ocupado:
        print("== ha medicao em curso; os TEMPOS desta corrida nao se publicam")

    for cifrado in (False, True):
        rot = "com senha" if cifrado else "sem senha"
        print("== %s: %d linhas, tabela `%s`" % (rot, n, LADO), flush=True)
        r = uma_corrida(cifrado, n)
        d["com_senha" if cifrado else "sem_senha"] = r
        print("   insert %7.1fs   select %7.1fs   update %6.1fs   delete %6.1fs   disco %6.1f MiB"
              % (r["insert"]["s"], r["select"]["s"], r["update"]["s"],
                 r["delete"]["s"], r["_tam_medido"] / 1024 / 1024), flush=True)

    a, b = d["sem_senha"], d["com_senha"]
    d["razao"] = {k: round(b[k]["s"] / a[k]["s"], 3)
                  for k in ("insert", "select", "update", "delete")}
    d["razao"]["disco"] = round(b["_tam_medido"] / a["_tam_medido"], 4)
    print("\n== a cifra custa (com senha / sem senha):")
    for k, v in d["razao"].items():
        print("   %-8s %.3fx" % (k, v))

    saida = os.path.join(os.path.dirname(os.path.abspath(__file__)), "resultado.json")
    with open(saida, "w") as f:
        json.dump(d, f, indent=1, ensure_ascii=False)
    print("\nRESULTADO " + json.dumps(d, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())

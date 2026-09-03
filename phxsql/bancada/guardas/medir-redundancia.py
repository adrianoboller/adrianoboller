#!/usr/bin/env python3
"""Qual metade sustenta as duas redundancias da cifra -- medido, nao lido.

    python3 bancada/guardas/medir-redundancia.py

# Por que este medidor existe

`aad-fora-do-slot` e `nonce-sem-endereco` nao provam guarda nenhuma: elas
AFIRMAM que tirar aquela metade nao e sentida, porque a outra cobre sozinha.
O `provar-guardas.py` audita o VEREDITO dessas afirmacoes -- se algum teste
cair, a redundancia acabou e ele avisa.

O que ninguem auditava era o MOTIVO. As duas notas creditavam a redundancia a
`(rowid, volume, versao)`, os tres valores que as duas fechaduras carregam. E
o teste que decide -- `trocar_o_corpo_de_uma_linha_pela_outra_nao_passa` --
copia o slot 5 INTEIRO por cima do slot 9: o cabecalho vai junto, entao a
versao e o tempero viajam com a copia, e os dois slots moram no mesmo volume.
Dos tres, DOIS sao iguais dos dois lados. So o `rowid` difere, e ele difere
porque nao esta gravado em lugar nenhum -- sai da posicao em que o slot foi
encontrado.

As quatro medicoes abaixo separam o que a leitura nao separa. A e C repoem os
defeitos das duas entradas como elas estao hoje (tem de dar «nada cai»); B e D
repoem o mesmo defeito MAIS a retirada de SO o rowid da outra fechadura, e sao
elas que decidem: se o teste cai em B e em D, quem segura e o rowid sozinho.

Medido em 03/09/2026: cai nas duas.
"""
import importlib.util
import os
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location(
    "provar_guardas", os.path.join(AQUI, "provar-guardas.py"))
pg = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pg)

TIRA_AAD = [
    {"arquivo": "crates/phxsql-store/src/reg.rs",
     "trecho": """            let selado = material.selar(&nonce, &aad_do_slot(volume, rowid, versao), &claro);
""",
     "troca": """            let selado = material.selar(&nonce, b"", &claro);
"""},
    {"arquivo": "crates/phxsql-store/src/reg.rs",
     "trecho": """    let claro = material.abrir(&nonce, &aad_do_slot(volume, rowid, versao), &guardado, nome)?;
""",
     "troca": """    let claro = material.abrir(&nonce, b"", &guardado, nome)?;
"""},
]

TIRA_ENDERECO_DO_NONCE = [
    {"arquivo": "crates/phxsql-store/src/cofre.rs",
     "trecho": """    let mut n = [0u8; XNONCE_LEN];
    n[0..8].copy_from_slice(&onde.to_le_bytes());
    n[8..12].copy_from_slice(&quem.to_le_bytes());
""",
     "troca": """    let mut n = [0u8; XNONCE_LEN];
    let _ = (onde, quem);
"""},
]

SO_O_ROWID_DO_NONCE = [
    {"arquivo": "crates/phxsql-store/src/cofre.rs",
     "trecho": """    let mut n = [0u8; XNONCE_LEN];
    n[0..8].copy_from_slice(&onde.to_le_bytes());
    n[8..12].copy_from_slice(&quem.to_le_bytes());
""",
     "troca": """    // MEDICAO: so o ROWID sai do nonce -- volume, contador e tempero ficam.
    let mut n = [0u8; XNONCE_LEN];
    let _ = onde;
    n[8..12].copy_from_slice(&quem.to_le_bytes());
"""},
]

SO_O_ROWID_DO_AAD = [
    {"arquivo": "crates/phxsql-store/src/reg.rs",
     "trecho": """    a[4..12].copy_from_slice(&rowid.to_le_bytes());
""",
     "troca": """    // MEDICAO: so o ROWID sai do dado associado -- volume e versao ficam.
    let _ = rowid;
"""},
]

MEDICOES = [
    ("A. so o AAD sai -- a guarda `aad-fora-do-slot` como ela esta hoje",
     TIRA_AAD, "nada cai"),
    ("B. o AAD sai E so o ROWID sai do nonce -- volume e versao ficam nos dois",
     TIRA_AAD + SO_O_ROWID_DO_NONCE, "CAI, e e o que prova que o rowid segura"),
    ("C. so o endereco do nonce sai -- a guarda `nonce-sem-endereco` de hoje",
     TIRA_ENDERECO_DO_NONCE, "nada cai"),
    ("D. o endereco sai do nonce E so o ROWID sai do AAD -- volume e versao ficam",
     TIRA_ENDERECO_DO_NONCE + SO_O_ROWID_DO_AAD,
     "CAI, e e o que prova que o rowid segura do outro lado"),
]

DECIDE = "trocar_o_corpo_de_uma_linha_pela_outra_nao_passa"
PRAZO = 900


def main():
    arvore = pg.Arvore(os.path.join(os.path.expanduser("~"), ".cache", "phx-guardas"))
    print("=" * 72)
    print("QUAL METADE SUSTENTA A REDUNDANCIA DA CIFRA")
    print("=" * 72)
    esperou = arvore.trancar()
    if esperou:
        print("esperou %.0f s pela vez" % esperou)
    print("copia:", arvore.montar(reaproveitar=True))
    arvore.garantir_frescor(m["arquivo"] for _, ms, _ in MEDICOES for m in ms)

    # A arvore LIMPA primeiro, e ela PARA a rodada. Sem isto, um binario que
    # nem compila devolve o mesmo veredito para as quatro medicoes e a rodada
    # parece ter medido quatro coisas sem medir uma.
    v, d, s, g = pg.rodar(arvore, "phxsql-store", ["--test", "cifra-dos-dados"], PRAZO)
    vermelhos = sum(1 for r in v.values() if r == "FAILED")
    print("arvore limpa: %s  %.1f s  %d testes  %d vermelhos\n"
          % (d, g, len(v), vermelhos))
    if d != "rodou" or vermelhos:
        print("A ARVORE LIMPA NAO ESTA VERDE -- nada aqui prova nada.")
        for l in [l for l in s.split("\n") if l.startswith("error")][:5]:
            print("   ", l)
        return 2

    problemas = 0
    for titulo, mudancas, esperado in MEDICOES:
        motivo = arvore.repor(mudancas)
        if motivo:
            print("%s\n   NAO DEU PARA APLICAR: %s\n" % (titulo, motivo))
            arvore.desfazer_tudo()
            problemas += 1
            continue
        try:
            v, d, s, g = pg.rodar(
                arvore, "phxsql-store", ["--test", "cifra-dos-dados"], PRAZO)
        finally:
            arvore.desfazer_tudo()
        if d == "nao compilou":
            print("%s\n   NAO COMPILOU\n" % titulo)
            problemas += 1
            continue
        cairam = [n for n, r in v.items() if r == "FAILED"]
        print("%s\n   esperado: %s" % (titulo, esperado))
        print("   %s  %.1f s  %d testes, %d cairam" % (d, g, len(v), len(cairam)))
        print("   %s: %s\n" % (DECIDE, v.get(DECIDE, "AUSENTE")))
    return 1 if problemas else 0


if __name__ == "__main__":
    sys.exit(main())

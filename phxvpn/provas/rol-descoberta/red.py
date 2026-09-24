#!/usr/bin/env python3
"""Prova RED: tira UMA conferencia de cada vez do fonte, roda os testes que
deviam acusar e devolve o fonte como estava. Um teste que passa com a
conferencia removida e um teste que passa por engano -- e aqui reprova.

Uso (de phxvpn/):  python3 provas/rol-descoberta/red.py [saida.json]
"""
import json
import pathlib
import subprocess
import sys

RAIZ = pathlib.Path(__file__).resolve().parents[2]

# (arquivo, trecho com a conferencia, trecho sem ela, testes que tem de falhar)
CASOS = [
    ("src/rol.rs", "if !r.conferir(dono, rede) {", "if false {",
     ["rol::testes::rol_adulterado_e_recusado",
      "rol::testes::assinatura_de_outra_chave_e_recusada"]),
    ("src/rol.rs", "if r.versao <= versao_atual {", "if false {",
     ["rol::testes::rol_velho_e_recusado",
      "p2p::ganchos_do_rol::testes::rol_sem_o_membro_derruba_o_tunel_com_ele"]),
    ("src/p2p.rs", "if self.fora_do_rol(&chamada.estatica_dele) {", "if false {",
     ["p2p::ganchos_do_rol::testes::par_fora_do_rol_nao_fecha_aperto"]),
    ("src/p2p.rs", "None if assinada => {}", "None if false => {}",
     ["p2p::ganchos_do_rol::testes::lista_de_pares_nao_injeta_par_em_rede_assinada"]),
    ("src/p2p_rol.rs", "reter_pares(e, |p| rol.membro(&p.publica).is_some());",
     "reter_pares(e, |_| true);",
     ["p2p::ganchos_do_rol::testes::rol_sem_o_membro_derruba_o_tunel_com_ele"]),
    ("src/descoberta.rs", "if self.vistos.get(&publica).is_some_and(|&v| carimbo <= v) {",
     "if false {", ["descoberta::testes::anuncio_repetido_e_recusado"]),
    ("src/descoberta.rs", "if agora_ms().abs_diff(carimbo) > JANELA_S * 1000 {",
     "if false {", ["descoberta::testes::anuncio_fora_da_janela_e_recusado"]),
    ("src/descoberta.rs", "        if !self.ligada {\n            return None;\n        }\n        if dado.len()",
     "        if dado.len()",
     ["descoberta::testes::desligada_nao_abre_nada_e_teto_vem_antes_do_selo"]),
]


def rodar(testes):
    r = subprocess.run(
        ["cargo", "test", "--lib", "--", "--exact", *testes],
        cwd=RAIZ, capture_output=True, text=True)
    saida = r.stdout + r.stderr
    falharam = [t for t in testes if f"{t} ... FAILED" in saida]
    return r.returncode, falharam, saida


def main():
    resultado = []
    for arquivo, com, sem, testes in CASOS:
        caminho = RAIZ / arquivo
        original = caminho.read_text()
        if original.count(com) != 1:
            sys.exit(f"{arquivo}: trecho nao achado uma vez: {com!r}")
        try:
            caminho.write_text(original.replace(com, sem))
            codigo, falharam, saida = rodar(testes)
        finally:
            caminho.write_text(original)
        if "error[" in saida:
            sys.exit(f"nao compilou sem a conferencia de {arquivo}:\n{saida[-2000:]}")
        ok = sorted(falharam) == sorted(testes)
        print(f"{'RED ok ' if ok else 'FALHOU'}  {arquivo}: {com.strip()[:60]!r} -> "
              f"{len(falharam)}/{len(testes)} testes acusaram")
        resultado.append({"arquivo": arquivo, "conferencia_removida": com.strip(),
                          "testes": testes, "acusaram": falharam, "red": ok})
    # E com o fonte de volta, os mesmos testes passam.
    todos = sorted({t for c in CASOS for t in c[3]})
    codigo, falharam, _ = rodar(todos)
    print(f"fonte restaurado: {len(todos) - len(falharam)}/{len(todos)} passam")
    final = {"casos": resultado, "restaurado_passam": len(todos) - len(falharam),
             "restaurado_total": len(todos)}
    if len(sys.argv) > 1:
        pathlib.Path(sys.argv[1]).write_text(json.dumps(final, indent=2, ensure_ascii=False))
    if not all(c["red"] for c in resultado) or falharam:
        sys.exit(1)


if __name__ == "__main__":
    main()

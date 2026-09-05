#!/usr/bin/env python3
"""Escolhe modelo e effort para uma tarefa da conversao WX.

A regra esta em references/balanceamento-de-modelos.md; aqui ela vira codigo
para que orquestrador e PMO nao decidam cada um de um jeito. Le o orcamento do
gate em .wx-migration/pmo/orcamento.json (se existir) e registra cada decisao
em .wx-migration/pmo/roteamento.jsonl.
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

DEGRAUS = ["haiku", "sonnet", "opus"]
CLASSES = {
    "mecanica": ("haiku", "medium"),
    "analise": ("sonnet", "high"),
    "decisao": ("opus", "high"),
    "revisao": ("opus", "max"),
}
SINAIS_SOBE = {"conflito", "fiscal", "dinheiro", "permissao", "dado-pessoal", "decisao-humana", "falhou-antes"}
SINAIS_DESCE = {"padrao-aprovado", "volume-grande", "criterio-objetivo"}
# Modelo local (Magnitude, J.modelos_locais): degrau abaixo do haiku, so para
# tarefa mecanica. Nunca para o que produz regra, decisao ou prova -- o barato
# entra onde o erro e barato, e ai nao e. Servico fora do ar volta ao pago.
LOCAL = "local"
CLASSES_SEM_LOCAL = {"analise", "decisao", "revisao"}
SINAIS_SEM_LOCAL = {"conflito", "fiscal", "dinheiro", "permissao", "decisao-humana", "falhou-antes"}
ENDERECO_LOCAL = "http://127.0.0.1:10100"


def servico_local_no_ar(endereco: str = ENDERECO_LOCAL, timeout: float = 1.5) -> bool:
    """Confere o servico do Magnitude sem depender de biblioteca de fora."""
    import urllib.error
    import urllib.request
    try:
        with urllib.request.urlopen(f"{endereco}/inference/v1/models", timeout=timeout) as r:
            return 200 <= r.status < 300
    except (urllib.error.URLError, OSError, ValueError):
        return False


def pode_ir_para_local(classe: str, sinais: set[str]) -> tuple[bool, str]:
    """Diz se a tarefa PODE ir para o modelo local, e por que nao quando nao pode."""
    if classe in CLASSES_SEM_LOCAL:
        return False, f"classe {classe} nao vai para local: produz regra, decisao ou prova"
    impeditivos = sinais & SINAIS_SEM_LOCAL
    if impeditivos:
        return False, "sinal impede local: " + ", ".join(sorted(impeditivos))
    # 'dado-pessoal' nao aparece aqui de proposito: ele ja SOBE o modelo (regra
    # antiga, deliberada -- dado delicado merece o modelo melhor), entao a tarefa
    # nunca chega ao degrau mais baixo e nao cai no local. Manter o dado na
    # maquina seria outro argumento, e trocar essa regra e decisao do dono, nao
    # efeito colateral de uma funcionalidade nova.
    return True, "tarefa mecanica sem sinal impeditivo"


def rotear(classe: str, sinais: set[str], gasto_pct: float | None, indisponiveis: set[str],
           local: bool = False, local_no_ar: bool | None = None) -> dict:
    if classe not in CLASSES:
        raise ValueError(f"classe inválida: {classe!r} (aceitas: {', '.join(CLASSES)})")
    modelo, effort = CLASSES[classe]
    motivos: list[str] = [f"classe {classe}"]
    grau = DEGRAUS.index(modelo)
    if classe != "revisao":
        if sinais & SINAIS_SOBE:
            teto = 1 if classe == "mecanica" else 2
            if grau < teto:
                grau += 1
                motivos.append("subiu: " + ", ".join(sorted(sinais & SINAIS_SOBE)))
        elif sinais & SINAIS_DESCE and grau > 0:
            grau -= 1
            motivos.append("desceu: " + ", ".join(sorted(sinais & SINAIS_DESCE)))
    estado = "OK"
    if gasto_pct is not None:
        if gasto_pct >= 100:
            estado = "BLOQUEADO"
            motivos.append(f"orçamento do gate em {gasto_pct:.0f}%")
        elif gasto_pct >= 80 and classe != "revisao" and grau > 0:
            grau -= 1
            motivos.append(f"orçamento em {gasto_pct:.0f}%: rebaixado")
    while grau > 0 and DEGRAUS[grau] in indisponiveis:
        motivos.append(f"fallback: {DEGRAUS[grau]} indisponível")
        grau -= 1
    escolhido = DEGRAUS[grau]
    if local and estado != "BLOQUEADO" and grau == 0:
        pode, porque = pode_ir_para_local(classe, sinais)
        if not pode:
            motivos.append(porque)
        else:
            no_ar = servico_local_no_ar() if local_no_ar is None else local_no_ar
            if no_ar:
                escolhido = LOCAL
                motivos.append(f"local: {porque}")
            else:
                motivos.append("local pedido, mas o serviço do Magnitude não respondeu; seguiu no modelo pago")
    return {"modelo": escolhido, "effort": effort, "estado": estado, "motivos": motivos}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--classe", required=True, choices=sorted(CLASSES))
    parser.add_argument("--local", action="store_true", help="permite cair no modelo local (Magnitude) quando a tarefa e mecanica")
    parser.add_argument("--local-no-ar", choices=["sim", "nao"], help="pula a conferencia do servico (para teste)")
    parser.add_argument("--sinal", action="append", default=[], help="conflito, fiscal, dinheiro, permissao, dado-pessoal, decisao-humana, falhou-antes, padrao-aprovado, volume-grande, criterio-objetivo")
    parser.add_argument("--gate", default="", help="G0..G7, para ler o orçamento")
    parser.add_argument("--tarefa", default="", help="identificação curta, só para o registro")
    parser.add_argument("--indisponivel", action="append", default=[], help="modelo sem acesso na organização")
    parser.add_argument("--project-root", type=Path, default=Path("."))
    args = parser.parse_args()

    pmo = args.project_root / ".wx-migration" / "pmo"
    gasto_pct = None
    orcamento = pmo / "orcamento.json"
    if args.gate and orcamento.is_file():
        dados = json.loads(orcamento.read_text(encoding="utf-8"))
        g = dados.get("gates", {}).get(args.gate)
        if g and g.get("tokens_previstos"):
            gasto_pct = 100.0 * float(g.get("tokens_gastos", 0)) / float(g["tokens_previstos"])

    # J.modelos_locais liga o degrau local por padrao; --local forca sem o questionario
    local = args.local
    q = args.project_root / ".wx-migration" / "questionario.json"
    if not local and q.is_file():
        try:
            j = json.loads(q.read_text(encoding="utf-8")).get("J_economia_de_tokens") or {}
            local = bool((j.get("modelos_locais") or {}).get("ativar"))
        except (OSError, json.JSONDecodeError):
            pass
    no_ar = None if args.local_no_ar is None else (args.local_no_ar == "sim")
    decisao = rotear(args.classe, set(args.sinal), gasto_pct, set(args.indisponivel), local=local, local_no_ar=no_ar)
    decisao.update({"gate": args.gate, "tarefa": args.tarefa, "quando": datetime.now(timezone.utc).isoformat(timespec="seconds")})
    if pmo.is_dir():
        with (pmo / "roteamento.jsonl").open("a", encoding="utf-8") as f:
            f.write(json.dumps(decisao, ensure_ascii=False) + "\n")
    print(json.dumps(decisao, ensure_ascii=False))
    return 0 if decisao["estado"] == "OK" else 3


# Registro das operacoes do plugin (.wx-migration/logs/): sem projeto por
# perto, nao grava nada; falha de registro nunca derruba a operacao.
try:
    import registro
except ImportError:  # rodando de outro diretorio
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import registro


if __name__ == "__main__":
    try:
        sys.exit(registro.envolver(__file__, main))
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(f"erro: {exc}", file=sys.stderr)
        sys.exit(2)

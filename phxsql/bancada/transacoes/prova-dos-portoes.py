#!/usr/bin/env python3
"""A prova de que a prova PEGA -- roda o `provar.py` com defeito reposto.

    python3 bancada/transacoes/prova-dos-portoes.py

Passar quando esta tudo certo nao prova nada. Esta casa ja teve prova que
passava COM o defeito reposto, porque conferia o veredito e a conferencia
acontecia depois do dano. Entao cada defeito daqui diz qual conferencia ele TEM
de derrubar, e este script reprova se ela sobreviver.

O que o unico defeito de hoje mostrou, e vale mais que o portao
--------------------------------------------------------------
Apagando a marca `transacao_<id>.tx` entre o SIGKILL e a reabertura, o banco
volta com PARTE das linhas -- 43 de 3.000 na corrida de 07/09/2026, e o numero
muda a cada corrida porque matar o processo no meio da passada e uma corrida.

Isso NAO e defeito do motor; e a demonstracao do contrario. O SIGKILL cai no
meio da passada de commit, entao ha mesmo meia gravacao no disco naquele
instante -- e a marca e a UNICA coisa que a resolve, reaplicando o que faltava.
Tirar a marca faz aparecer a metade que ela existe para consertar.

Ou seja: o «nunca metade» nao e acidente do caminho de escrita. E PAGO pela
marca, e o preco aparece quando ela some.
"""

import os
import subprocess
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
PROVAR = os.path.join(AQUI, "provar.py")

# defeito -> trecho da conferencia que ele TEM de derrubar
DEFEITOS = {
    "apaga-a-marca": "ficou pela METADE",
}


def main():
    falhou = []
    for defeito, exigida in DEFEITOS.items():
        print(f"== defeito reposto: {defeito}")
        amb = dict(os.environ, PHX_TX_DEFEITO=defeito)
        r = subprocess.run([sys.executable, PROVAR], env=amb,
                           capture_output=True, text=True, timeout=900)
        saida = r.stdout + r.stderr
        if r.returncode == 0:
            falhou.append(f"{defeito}: o medidor PASSOU com o defeito reposto")
            print("  FALHA  passou com o defeito reposto")
            continue
        if f"FALHA  {exigida}" not in saida and exigida not in saida:
            falhou.append(f"{defeito}: reprovou, mas nao por «{exigida}»")
            print(f"  FALHA  reprovou por outro motivo, nao por «{exigida}»")
            continue
        quantas = saida.count("  FALHA")
        print(f"  OK     reprovou, e «{exigida}» esta entre as {quantas} falhas")

    # O CONTROLE POSITIVO: sem defeito, tem de passar. Um portao que reprova
    # tudo nao protege nada -- e a recusa sem controle que passa ja custou dois
    # vereditos errados nesta casa.
    print("== controle positivo: sem defeito nenhum")
    r = subprocess.run([sys.executable, PROVAR], capture_output=True,
                       text=True, timeout=900)
    if r.returncode != 0:
        falhou.append("controle positivo: reprovou SEM defeito nenhum")
        print("  FALHA  reprovou sem defeito -- o medidor esta quebrado")
    else:
        print("  OK     passou limpo")

    print()
    if falhou:
        for f in falhou:
            print(f"  - {f}")
        sys.exit(1)
    print("os portoes pegam.")


if __name__ == "__main__":
    main()

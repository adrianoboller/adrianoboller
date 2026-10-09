#!/usr/bin/env python3
"""Extrai dos vetores OFICIAIS so o que a prova do RSA PKCS#1 v1.5 + SHA-256 le.

Por que extrair em vez de copiar: o arquivo do Wycheproof tem 211 KB (cada grupo traz a
chave em PEM, DER, ASN.1 e JWK) e o do NIST traz cinco hashes e tres tamanhos misturados;
a prova le n, e, mensagem, assinatura e veredito. O cabecalho de cada arquivo gerado diz
a origem, o commit ou o pacote, o SHA-256 do original e a licenca -- quem duvidar baixa
o original, confere o hash e roda isto de novo.

Uso:
  python3 -I extrair.py wycheproof <rsa_signature_2048_sha256_test.json> <commit> > wycheproof_rsa_2048_sha256.txt
  python3 -I extrair.py nist <SigVer15_186-3.rsp> > nist_sigver15_sha256.txt

Formato de saida (uma linha por registro, campos separados por espaco):
  # comentario
  chave <n_hex> <e_hex>
  caso <id> <valido|invalido|aceitavel> <flags separadas por virgula ou -> <msg_hex ou -> <sig_hex ou ->
"""
import hashlib
import json
import re
import sys


def sha256_de(caminho):
    with open(caminho, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def wycheproof(caminho, commit):
    d = json.load(open(caminho, encoding="utf-8"))
    print("# Wycheproof, https://github.com/C2SP/wycheproof testvectors_v1/rsa_signature_2048_sha256_test.json")
    print(f"# commit {commit}; SHA-256 do original {sha256_de(caminho)}")
    print("# Licenca: Apache-2.0 (Copyright Google LLC e contribuidores do Wycheproof)")
    print(f"# {d['numberOfTests']} casos; algoritmo {d['algorithm']}")
    vered = {"valid": "valido", "invalid": "invalido", "acceptable": "aceitavel"}
    for g in d["testGroups"]:
        assert g["sha"] == "SHA-256", g["sha"]
        pk = g["publicKey"]
        print(f"chave {pk['modulus']} {pk['publicExponent']}")
        for t in g["tests"]:
            flags = ",".join(t["flags"]) or "-"
            print(f"caso {t['tcId']} {vered[t['result']]} {flags} {t['msg'] or '-'} {t['sig'] or '-'}")


def nist(caminho):
    print("# NIST CAVP, https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Algorithm-Validation-Program/documents/dss/186-3rsatestvectors.zip")
    print("# -> SigVer15_186-3.rsp, so SHAAlg = SHA256 (mod 1024, 2048 e 3072)")
    print(f"# SHA-256 do original {sha256_de(caminho)}")
    print("# Licenca: obra do governo dos EUA (NIST), dominio publico")
    print("# S, Msg e e com o zero a esquerda reposto (k bytes, 128 bytes, numero par de digitos):")
    print("# o .rsp os escreve como inteiros")
    mod, n, atual, caso = None, None, {}, 0
    ultima_chave = None
    for linha in open(caminho, encoding="utf-8"):
        linha = linha.strip()
        m = re.match(r"\[mod = (\d+)\]", linha)
        if m:
            mod = int(m.group(1))
            continue
        if "=" not in linha or linha.startswith("#"):
            continue
        k, v = [x.strip() for x in linha.split("=", 1)]
        if k == "n":
            n = v
            continue
        atual[k] = v
        if k != "Result":
            continue
        if atual["SHAAlg"] == "SHA256":
            caso += 1
            par = lambda h: h.rjust(len(h) + len(h) % 2, "0")
            chave = (par(n), par(atual["e"]))
            if chave != ultima_chave:
                print(f"# mod = {mod}")
                print(f"chave {chave[0]} {chave[1]}")
                ultima_chave = chave
            vered = "valido" if atual["Result"].startswith("P") else "invalido"
            # O .rsp escreve S e Msg como inteiros: zero a esquerda some (255 digitos onde
            # cabem 256). A assinatura e a I2OSP de k bytes (RFC 8017 §8.2.2 passo 1) e a
            # mensagem do SigVer do CAVS tem 128 bytes; os zeros voltam aqui, e so eles.
            k2 = len(n.lstrip("0")) + len(n.lstrip("0")) % 2
            s = atual["S"].rjust(k2, "0")
            msg = atual["Msg"].rjust(256, "0")
            print(f"caso {caso} {vered} mod{mod} {msg} {s}")
        atual = {}


if __name__ == "__main__":
    if sys.argv[1] == "wycheproof":
        wycheproof(sys.argv[2], sys.argv[3])
    elif sys.argv[1] == "nist":
        nist(sys.argv[2])
    else:
        sys.exit("uso: extrair.py wycheproof|nist ...")

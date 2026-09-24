#!/usr/bin/env python3
"""Provas do phxjev.py. Rodar: python3 plugins/phxjev/scripts/teste_phxjev.py"""
import io
import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import phxjev as j  # noqa: E402


def noul(p, evid="x.rs:1"):
    return {"tipo": "noul", "p": p, "evid": evid}


def rodar(itens, registro, preset="revisar"):
    saida = io.StringIO()
    j.cmd_veredito(json.dumps({"preset": preset, "estado": ["x.rs:1"], "itens": itens}), saida, registro)
    return saida.getvalue()


class Limiares(unittest.TestCase):
    def setUp(self):
        self.reg = os.path.join(tempfile.mkdtemp(), "r.jsonl")

    def achado(self, **p):
        base = {"real": 0.9, "alcancavel": 0.9, "ja_tratado": 0.1, "defeito_ativo": 0.9}
        base.update(p)
        return [{"id": "a1", "perguntas": {k: noul(v) for k, v in base.items()}}]

    def test_real_baixo_descarta(self):
        self.assertIn("DESCARTAR (real<0.50)", rodar(self.achado(real=0.49), self.reg))

    def test_ja_tratado_alto_descarta(self):
        self.assertIn("DESCARTAR (ja_tratado>0.70)", rodar(self.achado(ja_tratado=0.71), self.reg))

    def test_defeito_ativo_fica_na_conta(self):
        self.assertIn("MANTER ☐", rodar(self.achado(defeito_ativo=0.6), self.reg))

    def test_nao_ativo_e_confiante_vai_para_depois(self):
        self.assertIn("MANTER ⏸", rodar(self.achado(defeito_ativo=0.05), self.reg))

    def test_na_duvida_fica_na_conta(self):
        # 0.45 esta abaixo do limiar, mas a conf e baixa: a decisao do dono manda ficar.
        self.assertIn("MANTER ☐", rodar(self.achado(defeito_ativo=0.45), self.reg))

    def test_severidade_alta_bloqueia(self):
        itens = self.achado()
        itens[0]["perguntas"]["severidade"] = {"tipo": "score", "p": {"0": 0, "1": 0.1, "2": 0.4, "3": 0.5}, "evid": "x"}
        self.assertIn("bloqueia", rodar(itens, self.reg))

    def test_empate_de_choice_escala(self):
        itens = [{"id": "h", "perguntas": {"causa": {"tipo": "choice", "p": {"A": 0.5, "B": 0.45, "outra": 0.05}, "evid": "x"}}}]
        self.assertIn("h.causa: empate", rodar(itens, self.reg, "porque"))

    def test_fere_petrea_escala(self):
        itens = [{"id": "o", "perguntas": {"fere_petrea_A": noul(0.3)}}]
        self.assertIn("choque com petrea", rodar(itens, self.reg, "escolher"))


class Recusas(unittest.TestCase):
    def setUp(self):
        self.reg = os.path.join(tempfile.mkdtemp(), "r.jsonl")

    def test_p_sem_evidencia_recusa(self):
        with self.assertRaises(j.Invalido):
            rodar([{"id": "a", "perguntas": {"real": noul(0.9, evid="")}}], self.reg)

    def test_sem_evidencia_meio_passa(self):
        rodar([{"id": "a", "perguntas": {"x": noul(0.5, evid="")}}], self.reg)

    def test_choice_que_nao_soma_um_recusa(self):
        with self.assertRaises(j.Invalido):
            rodar([{"id": "a", "perguntas": {"c": {"tipo": "choice", "p": {"A": 0.7, "B": 0.7}, "evid": "x"}}}], self.reg)

    def test_estado_vazio_recusa(self):
        with self.assertRaises(j.Invalido):
            j.cmd_veredito(json.dumps({"estado": [], "itens": []}), io.StringIO(), self.reg)


class Calibracao(unittest.TestCase):
    def test_ciclo_registro_desfecho_calibrar(self):
        reg = os.path.join(tempfile.mkdtemp(), "r.jsonl")
        out = rodar([{"id": "a", "perguntas": {"real": noul(0.8)}}], reg)
        self.assertIn("calibracao: nao medida (0/50", out)
        rid = j.ler(reg)[0]["id"]
        j.cmd_desfecho(rid, "real", "1", reg)
        cal = j.cmd_calibrar(reg)
        # sim 0.8 acertou, nao 0.2 errou: (0.2^2 + 0.2^2)/2 = 0.04
        self.assertIn("brier: 0.0400", cal)
        self.assertIn("INSUFICIENTE", cal)


class Selo(unittest.TestCase):
    def test_saida_editada_se_detecta(self):
        reg = os.path.join(tempfile.mkdtemp(), "r.jsonl")
        out = rodar([{"id": "a", "perguntas": {"real": noul(0.8)}}], reg)
        selo = out.strip().splitlines()[-1].split()[1]
        original = j.cmd_mostrar(selo, reg)
        self.assertEqual(out.split("\nselo:")[0], original)
        # quem encurtar a saida (como o juiz fez com o caminho) nao reproduz o selo
        editada = original.replace(reg, "...")
        self.assertNotEqual(j.hashlib.sha256(editada.encode()).hexdigest()[:12], selo)

    def test_registro_nao_obedece_variavel_de_ambiente(self):
        os.environ["PHXJEV_REGISTRO"] = "/tmp/outro.jsonl"
        try:
            import importlib
            importlib.reload(j)
            self.assertTrue(j.REGISTRO.endswith(os.path.join(".phxjev", "registro.jsonl")))
            self.assertNotEqual(j.REGISTRO, "/tmp/outro.jsonl")
        finally:
            del os.environ["PHXJEV_REGISTRO"]


if __name__ == "__main__":
    unittest.main(verbosity=1)

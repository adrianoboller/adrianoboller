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


class Melhorias(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.reg = os.path.join(self.dir, "r.jsonl")

    def test_severidade_incerta_escala_e_nao_bloqueia_calada(self):
        itens = [{"id": "a", "perguntas": {"real": noul(0.9), "severidade": {
            "tipo": "score", "p": {"0": 0.1, "1": 0.2, "2": 0.3, "3": 0.4}, "evid": "x"}}}]
        out = rodar(itens, self.reg)
        self.assertIn("a.severidade: incerta", out)
        self.assertIn("bloqueia?", out)

    def test_desfecho_so_acrescenta(self):
        rodar([{"id": "a", "perguntas": {"real": noul(0.8)}}], self.reg)
        antes = open(self.reg, encoding="utf-8").read()
        j.cmd_desfecho(j.ler(self.reg)[0]["id"], "real", "1", self.reg)
        depois = open(self.reg, encoding="utf-8").read()
        self.assertTrue(depois.startswith(antes))
        self.assertEqual(j.ler(self.reg)[0]["desfecho"], {"real": "sim"})

    def test_calibrar_separa_por_pergunta(self):
        rodar([{"id": "a", "perguntas": {"real": noul(0.9), "ja_tratado": noul(0.1)}}], self.reg)
        rid = j.ler(self.reg)[0]["id"]
        j.cmd_desfecho(rid, "real", "0", self.reg)
        j.cmd_desfecho(rid, "ja_tratado", "0", self.reg)
        cal = j.cmd_calibrar(self.reg)
        linha_real = [l for l in cal.splitlines() if l.startswith("real ")][0]
        linha_jt = [l for l in cal.splitlines() if l.startswith("ja_tratado")][0]
        self.assertIn("0.8100", linha_real)   # errou com 0,9
        self.assertIn("0.0100", linha_jt)     # acertou com 0,9 no «nao»

    def git(self, *a, data=None):
        import subprocess
        env = dict(os.environ, GIT_COMMITTER_DATE=data, GIT_AUTHOR_DATE=data) if data else None
        subprocess.run(["git", "-C", self.dir, *a], check=True, capture_output=True, env=env)

    def test_colher_so_o_pedido_fechado_depois_do_veredito(self):
        pend = os.path.join(self.dir, "PENDENCIAS.md")
        self.git("init", "-q")
        self.git("config", "user.email", "t@t"); self.git("config", "user.name", "t")
        open(pend, "w").write("| ☑️ | 10 | ja fechado |\n| ☐ | 11 | aberto |\n| ☐ | 12 | fica |\n")
        self.git("add", "."); self.git("commit", "-qm", "a", data="2020-01-01T00:00:00")
        itens = [{"id": f"{n}-x", "perguntas": {"real": noul(0.9), "ja_tratado": noul(0.1)}} for n in (10, 11, 12)]
        itens.append({"id": "11b-parte", "perguntas": {"real": noul(0.9)}})
        rodar(itens, self.reg)
        open(pend, "w").write("| ☑️ | 10 | ja fechado |\n| ☑️ | 11 | aberto |\n| ☐ | 12 | fica |\n")
        import time as t
        t.sleep(1.1)
        self.git("commit", "-qam", "fecha 11")
        feitos, pulados = j.cmd_colher(pend, self.reg)
        self.assertEqual(len(feitos), 2, feitos)          # real e ja_tratado do 11
        self.assertTrue(all("-11-x." in f for f in feitos))
        self.assertEqual(pulados, 1)                      # o 10 ja estava fechado
        self.assertEqual(j.cmd_colher(pend, self.reg)[0], [])  # nao colhe duas vezes

    def test_colher_com_caminho_relativo_nao_inventa_fechamento(self):
        pend = os.path.join(self.dir, "PENDENCIAS.md")
        self.git("init", "-q")
        self.git("config", "user.email", "t@t"); self.git("config", "user.name", "t")
        open(pend, "w").write("| ☑️ | 10 | ja fechado |\n")
        self.git("add", "."); self.git("commit", "-qm", "a", data="2020-01-01T00:00:00")
        rodar([{"id": "10-x", "perguntas": {"real": noul(0.9)}}], self.reg)
        antes = os.getcwd()
        os.chdir(os.path.dirname(self.dir))
        try:
            feitos, pulados = j.cmd_colher(os.path.join(os.path.basename(self.dir), "PENDENCIAS.md"), self.reg)
        finally:
            os.chdir(antes)
        self.assertEqual((feitos, pulados), ([], 1))


class Gancho(unittest.TestCase):
    def transcricao(self, saida, final, pedido="/phxjev-perguntar x"):
        return [
            {"type": "user", "message": {"role": "user", "content": pedido}},
            {"type": "assistant", "message": {"content": [{"type": "tool_use", "input": {"command": "phxjev.py veredito"}}]}},
            {"type": "user", "message": {"content": [{"type": "tool_result", "content": saida}]}},
            {"type": "assistant", "message": {"content": [{"type": "text", "text": final}]}},
        ]

    def test_resumo_no_lugar_da_saida_e_recusado(self):
        import gancho_selo as g
        reg = os.path.join(tempfile.mkdtemp(), "r.jsonl")
        saida = rodar([{"id": "a", "perguntas": {"real": noul(0.8)}}], reg)
        self.assertTrue(g.faltas(self.transcricao(saida, "Veredito: real 0,8. Resumo."), reg))
        encurtada = saida.replace(reg, "...")
        self.assertTrue(g.faltas(self.transcricao(saida, encurtada), reg))

    def test_saida_colada_passa_mesmo_com_cerca_e_resumo(self):
        import gancho_selo as g
        reg = os.path.join(tempfile.mkdtemp(), "r.jsonl")
        saida = rodar([{"id": "a", "perguntas": {"real": noul(0.8)}}], reg)
        self.assertEqual(g.faltas(self.transcricao(saida, "```\n" + saida + "```\nResumo depois."), reg), [])

    def test_comando_phxjev_sem_script_e_recusado(self):
        import gancho_selo as g
        linhas = [
            {"type": "user", "message": {"content": [{"type": "text", "text": "Use a skill `phxjev` e aplique o preset escolher"}]}},
            {"type": "assistant", "message": {"content": [{"type": "text", "text": "Empate 5 a 5, sobe ao dono."}]}},
        ]
        self.assertTrue(g.faltas(linhas, "/nao/existe"))

    def test_turno_sem_phxjev_passa(self):
        import gancho_selo as g
        self.assertEqual(g.faltas(self.transcricao("ok", "pronto", "rode os testes"), "/nao/existe"), [])


class Autocalibracao(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.reg = os.path.join(self.dir, "r.jsonl")

    def povoar(self, n, p, taxa, juiz="claude"):
        """n vereditos noul 'real' com p fixa, dos quais taxa*n eram verdade."""
        linhas = []
        for i in range(n):
            rid = f"20260101-000000-{juiz}-{i}"
            linhas.append({"id": rid, "juiz": juiz, "item": f"x{i}", "preset": "t", "veredito": "",
                           "perguntas": {"real": {"tipo": "noul", "dist": {"sim": p, "nao": 1 - p}, "conf": 0}},
                           "desfecho": {}})
            linhas.append({"tipo": "desfecho", "ref": rid, "pergunta": "real",
                           "valor": "sim" if i < round(taxa * n) else "nao"})
        j.gravar(self.reg, linhas)

    def test_platt_corrige_o_excesso_de_confianca(self):
        self.povoar(60, 0.9, 0.6)
        a = j.ajuste(self.reg, "claude", "real")
        self.assertIsNotNone(a)
        self.assertAlmostEqual(j.aplicar_ajuste(a, 0.9), 0.6, delta=0.03)

    def test_sem_minimo_nao_ha_ajuste(self):
        self.povoar(49, 0.9, 0.6)
        self.assertIsNone(j.ajuste(self.reg, "claude", "real"))

    def test_veredito_decide_pela_p_corrigida(self):
        self.povoar(60, 0.9, 0.3)   # o juiz diz 0,9 e acerta 30%
        out = rodar([{"id": "a", "perguntas": {"real": noul(0.9)}}], self.reg)
        self.assertRegex(out, r"real 0\.90→0\.3[0-2]")
        self.assertIn("DESCARTAR (real<0.50)", out)

    def test_juizes_nao_se_misturam(self):
        self.povoar(60, 0.9, 0.3, juiz="local:x")
        self.assertIsNone(j.ajuste(self.reg, "claude", "real"))
        self.assertIn("0 desfechos", j.cmd_historico("claude", self.reg))

    def test_historico_manda_rebaixar_quem_exagera(self):
        self.povoar(20, 0.9, 0.5)
        self.assertIn("REBAIXE", j.cmd_historico("claude", self.reg))


class Chave(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.cfg = os.path.join(self.dir, "c", "config.json")
        self.ban = os.path.join(self.dir, "b.json")

    def bancada(self, **r):
        json.dump({"juizes": {"local m": r}}, open(self.ban, "w"))

    def test_padrao_e_claude(self):
        self.assertEqual(j.juiz_efetivo(self.cfg, self.ban)[0], "claude")

    def test_auto_sem_qualificar_volta_ao_claude_dizendo_por_que(self):
        self.bancada(n=12, brier=0.17, acerto=0.8, pior_erro=1.0)
        frase = j.cmd_juiz(["auto", "m"], self.cfg, self.ban)
        self.assertTrue(frase.startswith("juiz claude: auto pedido"), frase)
        self.assertIn("n 12 < 50", frase)

    def test_auto_qualificado_vale(self):
        self.bancada(n=60, brier=0.05, acerto=0.95, pior_erro=0.8)
        self.assertEqual(j.cmd_juiz(["auto", "m"], self.cfg, self.ban), "juiz auto com m (brier 0.05 em 60)")

    def test_modo_invalido_recusa(self):
        with self.assertRaises(j.Invalido):
            j.cmd_juiz(["sempre"], self.cfg, self.ban)


if __name__ == "__main__":
    unittest.main(verbosity=1)

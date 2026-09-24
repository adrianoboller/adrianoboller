#!/usr/bin/env python3
"""RED das guardas do tunel total e do DNS: tira cada guarda do fonte, roda o
teste dela e exige que ele REPROVE; depois devolve o fonte. Grava o placar em
resultados.json -> red_das_guardas.

Uso: python3 provas/tunel-total/red.py  (da pasta phxvpn; o teste do banco
pede PHXVPN_PG_TESTE, sem ele aquela linha sai como «nao rodou»)."""
import json, os, subprocess, sys, datetime

RAIZ = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
# (guarda, arquivo, trecho, troca, teste que tem de reprovar)
GUARDAS = [
    ("tunel total nao alcanca faixa privada", "src/rotas.rs",
     '"ip saddr {o} ip daddr {p} drop\\nip saddr {o} accept\\n"',
     '"ip saddr {o} accept\\n"',
     "rotas::testes::tunel_total_sai_com_nat_so_para_a_internet"),
    ("NAT de saida nao mascara o privado (rota de volta)", "src/rotas.rs",
     'nat.push_str(&format!("ip saddr {o} ip daddr != {p} masquerade\\n"));',
     'nat.push_str(&format!("ip saddr {o} masquerade\\n"));',
     "rotas::testes::tunel_total_sai_com_nat_so_para_a_internet"),
    ("iptables: DROP do privado antes do ACCEPT", "src/rotas.rs",
     'f.push_str(&format!("-A {CADEIA_ENC} -s {o} -d {p} -j DROP\\n"));',
     '',
     "rotas::testes::tunel_total_sai_com_nat_so_para_a_internet"),
    ("roteador da filial fora do tunel total", "src/rotas.rs",
     'for o in crate::saida::FORA_DO_ROTEADOR_DA_FILIAL {',
     'for o in [""; 0] {',
     "rotas::testes::conf_e_ccd_levam_as_diretivas"),
    ("resolvedor so responde a propria rede", "src/dns.rs",
     "if !origem_aceita(&de, &cfg.aceita) {",
     "if false && !origem_aceita(&de, &cfg.aceita) {",
     "dns::testes::resolvedor_responde_repassa_e_ignora_quem_e_de_fora"),
    ("nome so de ccd inteiro (sem .gravando)", "src/dns.rs",
     'if nome.ends_with(".gravando") {',
     'if nome.ends_with(".nunca") {',
     "dns::testes::nomes_saem_do_ccd_e_so_dos_arquivos_inteiros"),
    ("resposta de fora da zona nao e local", "src/dns.rs",
     'let dentro = nome == zona || nome.ends_with(&format!(".{zona}"));',
     'let dentro = nome == zona || nome.ends_with(zona);',
     "dns::testes::membro_da_zona_responde_o_ip_e_o_resto_vai_adiante"),
    ("tunel total pede DNS pela VPN", "src/saida.rs",
     "if s.tunel_total && !s.dns_nomes && s.dns_empresa.is_empty() {",
     "if false {",
     "saida::testes::combinacao_torta_e_recusada_com_o_motivo"),
    ("block-local so com tunel total", "src/saida.rs",
     "if s.bloquear_local && !s.tunel_total {",
     "if false {",
     "saida::testes::combinacao_torta_e_recusada_com_o_motivo"),
    ("DNS privado precisa de rota", "src/saida.rs",
     "if crate::rotas::privada(n) && !rotas.iter().any(|r| r.cidr.contem_ip(n)) {",
     "if false {",
     "saida::testes::combinacao_torta_e_recusada_com_o_motivo"),
    ("pacote: block-outside-dns empurrado", "src/saida.rs",
     'c.push_str("push \\"block-outside-dns\\"\\n");',
     '',
     "saida::testes::tunel_total_leva_o_pacote_inteiro"),
    ("pacote: block-ipv6", "src/saida.rs",
     'c.push_str("push \\"block-ipv6\\"\\nblock-ipv6\\n");',
     '',
     "saida::testes::tunel_total_leva_o_pacote_inteiro"),
    ("pacote: ifconfig-ipv6 ficticio", "src/saida.rs",
     'c.push_str(&format!("push \\"ifconfig-ipv6 {IPV6_FICTICIO}\\"\\n"));',
     '',
     "saida::testes::tunel_total_leva_o_pacote_inteiro"),
    ("DNS da empresa fora do espaco da VPN", "src/saida.rs",
     "if ESPACO_VPN.contem_ip(u32::from(ip)) {",
     "if false {",
     "saida::testes::dns_da_empresa_so_aceita_servidor_de_verdade"),
    ("dns_linux so da lista (sem injecao)", "src/saida.rs",
     '_ => return Err("dns_linux: resolvconf, systemd-resolved ou vazio".into()),',
     '_ => Some(DnsLinux::Resolvconf),',
     "saida::testes::perfil_do_cliente_so_leva_o_que_foi_pedido"),
    ("DOMAIN-ROUTE . so com tunel total", "src/saida.rs",
     "if s.tunel_total {\n                p.push_str(\"dhcp-option DOMAIN-ROUTE .\\n\");",
     "if true {\n                p.push_str(\"dhcp-option DOMAIN-ROUTE .\\n\");",
     "saida::testes::perfil_do_cliente_so_leva_o_que_foi_pedido"),
    ("so o admin alarga a saida", "src/saida.rs",
     "if amplia(&antiga, nova) && !ator.admin {",
     "if false {",
     "postgres_real:saida_tunel_total_dns_permissao_e_conf"),
]


def rodar(teste):
    if teste.startswith("postgres_real:"):
        if not os.environ.get("PHXVPN_PG_TESTE"):
            return None
        cmd = ["cargo", "test", "-q", "--test", "postgres_real", "--", "--exact", teste.split(":", 1)[1]]
    else:
        cmd = ["cargo", "test", "-q", "--lib", "--", "--exact", teste]
    r = subprocess.run(cmd, cwd=RAIZ, capture_output=True, text=True)
    return r.returncode == 0


placar = []
controle = {}
for nome, arq, trecho, troca, teste in GUARDAS:
    # Controle: o teste tem de PASSAR com a guarda no lugar. Sem isto, um
    # teste que reprova por outro motivo (banco fora do ar) contaria como RED.
    if teste not in controle:
        controle[teste] = rodar(teste)
    if controle[teste] is not True:
        res = "nao rodou" if controle[teste] is None else "CONTROLE REPROVOU"
        placar.append({"guarda": nome, "teste": teste, "resultado": res})
        print(f"{res:28} {nome}")
        continue
    caminho = os.path.join(RAIZ, arq)
    original = open(caminho).read()
    if original.count(trecho) != 1:
        placar.append({"guarda": nome, "teste": teste, "resultado": f"trecho achado {original.count(trecho)}x"})
        continue
    try:
        open(caminho, "w").write(original.replace(trecho, troca))
        passou = rodar(teste)
    finally:
        open(caminho, "w").write(original)
    res = "nao rodou" if passou is None else ("REPROVOU (bom)" if not passou else "PASSOU COM A GUARDA TIRADA")
    placar.append({"guarda": nome, "teste": teste, "resultado": res})
    print(f"{res:28} {nome}")

ok = sum(1 for p in placar if p["resultado"] == "REPROVOU (bom)")
arq = os.path.join(RAIZ, "provas/tunel-total/resultados.json")
tudo = json.load(open(arq)) if os.path.exists(arq) else {}
tudo["red_das_guardas"] = {
    "data": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
    "reprovaram": f"{ok}/{len(placar)}", "guardas": placar,
}
json.dump(tudo, open(arq, "w"), ensure_ascii=False, indent=2)
open(arq, "a").write("\n")
print(f"== RED: {ok}/{len(placar)}")
sys.exit(0 if ok == len(placar) else 1)

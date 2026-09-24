#!/usr/bin/env python3
"""RED das guardas do alcance: tira cada guarda do fonte, roda o teste que a
trava e confere que ele REPROVA; depois devolve o fonte. Teste que passa com
a guarda tirada nao guarda nada.

Uso (da pasta phxvpn): python3 provas/servidor-alcance/red.py
Grava a chave "red_das_guardas" no resultados.json ao lado.
"""
import json, os, subprocess, sys

AQUI = os.path.dirname(os.path.abspath(__file__))
RAIZ = os.path.abspath(os.path.join(AQUI, "..", ".."))

# (arquivo, trecho com a guarda, trecho sem ela, teste que tem de reprovar)
GUARDAS = [
    ("src/ovpn.rs", '{tc}\\\n{fim}",', '{fim}\\\n{tc}",',
     "blocos_de_conexao_sao_a_ultima_coisa_do_perfil"),
    ("src/alcance.rs", '.all(|b| b.is_ascii_alphanumeric() || b".-:".contains(&b));',
     ".all(|_| true);", "endereco_aceita_as_formas_e_barra_injecao"),
    ("src/alcance.rs", "x.is_control() || x == '\"' || x == '\\''", "x.is_control()",
     "caminho_da_credencial_nao_injeta_nem_e_relativo"),
    ("src/alcance.rs", "if self.aleatorio && self.queda_tcp.is_some() {",
     "if false {", "validar_recusa_as_combinacoes_sem_sentido"),
    ("src/alcance.rs", "        if rede.tcp {\n            p.push_str(&proxy);\n        }",
     "        p.push_str(&proxy);", "rede_tcp_vira_tcp_server_e_perfil_tcp_client_com_proxy"),
    ("src/alcance.rs", "if !tcp && c.alcance.queda_tcp.is_none() {",
     "if false {", "proxy_vai_so_no_bloco_tcp_e_nunca_com_senha"),
    ("src/alcance.rs", '        topo,\n        saida: "",', "        topo,\n        saida: rede.aviso_de_saida(),",
     "queda_tcp_poe_udp_primeiro_e_tcp_depois"),
    ("src/alcance.rs", "if lista.len() > 1 || queda.is_some() {", "if true {",
     "sem_alcance_o_perfil_nao_muda"),
    ("src/alcance.rs", "if a.aleatorio && lista.len() > 1 {", "if false {",
     "remotos_alternativos_com_espera_curta_e_sorteio"),
    ("src/alcance.rs", "        None => match std::fs::remove_file(&arq) {",
     "        None => match Ok::<(), std::io::Error>(()) {",
     "servidor_tcp_ganha_port_share_e_udp_ganha_a_ponte"),
    ("src/saida.rs", "    let mut p = String::new();\n    if let Some(d) = c.dns_linux {",
     "    let mut p = String::from(\"mssfix 1400\\n\");\n    if let Some(d) = c.dns_linux {",
     "nada_pendurado_depois_dos_blocos_e_opcao_de_conexao"),
    ("src/queda_tcp.rs", "UdpSocket::bind((local, 0))", "UdpSocket::bind((\"127.0.0.1\", 0))",
     "quadro_tcp_vira_datagrama_e_volta"),
    ("src/queda_tcp.rs", "    if cab.len() < 3 {\n        return false;\n    }\n    let tam",
     "    if cab.len() < 3 {\n        return true;\n    }\n    let tam",
     "distingue_openvpn_de_https"),
    ("src/queda_tcp.rs", "if !e_openvpn(&cab) {", "if false {",
     "port_share_manda_o_https_ao_dono_da_porta"),
    ("src/queda_tcp.rs", "if self.por_id.len() >= MAX_CONEXOES {", "if false {",
     "tetos_por_ip_e_da_ponte"),
    ("src/queda_tcp.rs", "if self.por_ip.get(&ip).copied().unwrap_or(0) >= MAX_POR_IP {", "if false {",
     "um_ip_nao_passa_do_teto_dele"),
    ("src/queda_tcp.rs", "if !e_openvpn(&cab) {", "if !e_openvpn(&cab) && conf.port_share.is_some() {",
     "lixo_sem_port_share_fecha_na_hora"),
    ("src/queda_tcp.rs", "let falta = fim.saturating_duration_since(Instant::now());",
     "let falta = PRIMEIROS_BYTES.max(fim.saturating_duration_since(Instant::now()));",
     "gotejar_nao_segura_a_conexao"),
    ("src/queda_tcp.rs", "apertou |= matches!(quadro[0] >> 3, 6 | 9);", "apertou = true;",
     "aperto_sem_dados_cai_no_prazo"),
    ("src/queda_tcp.rs", 'm.extend_from_slice(crate::guarda::chave_de_ip("", &ip.to_string()).as_bytes());',
     "m.extend_from_slice(ip.to_string().as_bytes());", "origem_local_separa_quem_vem_de_fora"),
    ("src/queda_tcp.rs", 'anotar(registro, &format!("{origem} -> port-share {h}:{p}"));', "",
     "port_share_manda_o_https_ao_dono_da_porta"),
    ("src/alcance.rs", "    if proibido {", "    if false {", "port_share_nao_expoe_o_loopback_nem_o_painel"),
    ("src/alcance.rs", "if portas_do_painel.contains(p) {", "if false {",
     "port_share_nao_expoe_o_loopback_nem_o_painel"),
    ("src/alcance.rs", "if c.alcance.proxy_sem_texto_claro && p.cred != CredProxy::Nenhuma {", "if false {",
     "sem_texto_claro_recusa_o_que_mandaria_a_senha_em_claro"),
    ("src/alcance.rs", "(TipoProxy::Http, CredProxy::Arquivo(c)) if em_bloco => {",
     "(TipoProxy::Http, CredProxy::Arquivo(c)) if false && em_bloco => {",
     "linhas_de_proxy_por_tipo_e_credencial"),
    ("src/dns.rs", "if RESERVADOS.contains(&r.as_str()) {", "if false {",
     "colisao_e_nome_reservado_nao_viram_nome"),
    ("src/dns.rs", "        m.remove(r);", "        let _ = r;", "colisao_e_nome_reservado_nao_viram_nome"),
    ("src/saida.rs", "        || (depois.dns_nomes && depois.dns_empresa != antes.dns_empresa)\n", "",
     "so_ligar_alarga"),
    ("src/supervisor.rs", "            if p.conf() == c {", "            if p.conf().porta == c.porta {",
     "ponte_segue_o_arquivo_da_pasta"),
    ("src/supervisor.rs", "        drop(pontes.remove(dir));", "        let _velha = pontes.remove(dir);",
     "ponte_segue_o_arquivo_da_pasta"),
]


def rodar(teste):
    r = subprocess.run(["cargo", "test", "--lib", "-q", teste], cwd=RAIZ, capture_output=True, text=True)
    return r.returncode, r.stdout + r.stderr


def main():
    res = []
    for arq, com, sem, teste in GUARDAS:
        caminho = os.path.join(RAIZ, arq)
        original = open(caminho).read()
        if original.count(com) != 1:
            print(f"GUARDA NAO ACHADA em {arq}: {com!r}", file=sys.stderr)
            sys.exit(2)
        try:
            open(caminho, "w").write(original.replace(com, sem))
            codigo, saida = rodar(teste)
        finally:
            open(caminho, "w").write(original)
        reprovou = codigo != 0 and "panicked" in saida
        print(f"{'RED ok ' if reprovou else 'PASSOU!'} {teste} ({arq})", file=sys.stderr)
        res.append({"arquivo": arq, "teste": teste, "reprovou_sem_a_guarda": reprovou})
    codigo, saida = rodar("alcance")
    verde = codigo == 0
    arq = os.path.join(AQUI, "resultados.json")
    r = json.load(open(arq)) if os.path.exists(arq) else {}
    r["red_das_guardas"] = {
        "reprovaram": sum(x["reprovou_sem_a_guarda"] for x in res),
        "de": len(res),
        "verde_depois_de_devolver": verde,
        "guardas": res,
    }
    open(arq, "w").write(json.dumps(r, ensure_ascii=False, indent=1) + "\n")
    print(f"{r['red_das_guardas']['reprovaram']}/{len(res)} reprovaram; verde depois: {verde}")
    sys.exit(0 if verde and all(x["reprovou_sem_a_guarda"] for x in res) else 1)


main()

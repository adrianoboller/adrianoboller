# Add web to config field list
# 27/08 19:55

s=open('MANUAL.txt').read()
s = s.replace('''    log_acessos       arquivo do log de acessos
    ips_permitidos    lista de IPs. VAZIO LIBERA TODOS - preencha sempre
                      que a porta nao estiver atras de firewall ou VPN''',
'''    log_acessos       arquivo do log de acessos
    ips_permitidos    lista de IPs. VAZIO LIBERA TODOS - preencha sempre
                      que a porta nao estiver atras de firewall ou VPN
    web               Centro de Controle pelo navegador. Desligado por
                      padrao; ver a secao 9''')
open('MANUAL.txt','w').write(s)

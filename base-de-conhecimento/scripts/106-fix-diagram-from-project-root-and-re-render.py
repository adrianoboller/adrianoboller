# Fix diagram from project root and re-render
# 27/08 20:02

p='docs/dossie/dossie-phxsql.html'
s=open(p).read()
velho_ini = '          <path d="M236 82 L236 232" stroke="var(--log)"'
i = s.index(velho_ini)
j = s.index('</g>\n      </svg>', i)
novo = '''          <path d="M190 82 L190 108 L152 108 L152 240" fill="none" stroke="var(--log)" stroke-width="1.3" stroke-dasharray="4 3" marker-end="url(#setaWlog)"/>
          <path d="M236 192 L236 240" stroke="var(--log)" stroke-width="1.3" stroke-dasharray="4 3" marker-end="url(#setaWlog)"/>
          <path d="M596 146 L596 204 L470 204 L470 240" fill="none" stroke="var(--log)" stroke-width="1.3" stroke-dasharray="4 3" marker-end="url(#setaWlog)"/>

          <rect x="120" y="244" width="500" height="54" rx="4" fill="none" stroke="var(--log)" stroke-width="1.7"/>
          <text x="370" y="266" text-anchor="middle" fill="var(--log)" font-weight="600" font-size="12.5">blacklist.json &#183; acessos.log</text>
          <text x="370" y="285" text-anchor="middle" font-size="10.5" opacity=".7">são do servidor, não da porta — bloqueou numa, bloqueou nas duas</text>

          <line x1="16" y1="322" x2="824" y2="322" stroke="currentColor" stroke-width="1" opacity=".3"/>
          <text x="16" y="346" font-size="11.5" opacity=".75">
            <tspan font-weight="600">A única diferença é onde a identidade mora.</tspan> HTTP não tem conexão que dure — ela mora na sessão.
          </text>
          <text x="16" y="368" font-size="11" opacity=".6">O PBKDF2 de 210.000 iterações custa 300 ms no login e 0 ms em cada clique seguinte — medido.</text>
          <text x="16" y="388" font-size="11" opacity=".6">A porta vem desligada; ligada, escuta só em 127.0.0.1. O config recusa as duas portas no mesmo endereço.</text>
        '''
s = s[:i] + novo + s[j:]
s = s.replace('viewBox="0 0 840 396" role="img" aria-label="Navegador e cliente',
              'viewBox="0 0 840 404" role="img" aria-label="Navegador e cliente')
open(p,'w').write(s)
print("ok")

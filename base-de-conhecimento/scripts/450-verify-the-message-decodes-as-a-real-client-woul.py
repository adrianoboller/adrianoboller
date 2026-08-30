# Verify the message decodes as a real client would read it
# 28/08 15:05

import email, base64, re
t=open('/tmp/claude-0/-home-user-adrianoboller/34595649-0af6-575a-8f79-80dbe8cb7a5d/scratchpad/rele-recebido.txt').read()
corpo=t.split('--- corpo ---\n')[1].split('--- fim ---')[0]
m=email.message_from_string(corpo)
from email.header import decode_header, make_header
print('Subject:', str(make_header(decode_header(m['Subject']))))
print('To     :', m['To'])
print()
print(m.get_payload(decode=True).decode('utf-8'))

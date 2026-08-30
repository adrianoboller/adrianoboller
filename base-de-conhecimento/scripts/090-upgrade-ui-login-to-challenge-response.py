# Upgrade UI login to challenge-response
# 27/08 19:50

p='crates/phxsql-server/ui/index.html'
s=open(p).read()

# 1. api() passa a guardar a sessao que o servidor devolver
velho_api = '''  const j = await r.json();
  if (!j.ok) throw new Error(j.erro || "falha");
  return j.resultado;
}'''
novo_api = '''  const j = await r.json();
  if (j.sessao) est.sessao = j.sessao;
  if (!j.ok) throw new Error(j.erro || "falha");
  return j.resultado;
}

// ---------------------------------------------------------------------
// Desafio-resposta. Base64 tira a senha do olho de quem passa; isto tira a
// senha do FIO. Quem grava o pacote nao consegue reproduzir a prova, porque
// o nonce do servidor vale uma vez so.
//
// Depende de crypto.subtle, que so existe em contexto seguro: https, ou
// http em 127.0.0.1. Fora disso a interface cai no Base64 e avisa.
// ---------------------------------------------------------------------
const podeProvar = () => !!(window.crypto && crypto.subtle && crypto.subtle.deriveBits);

const bytesDeHex = h => Uint8Array.from(h.match(/../g) || [], b => parseInt(b, 16));
const hexDeBytes = b => [...new Uint8Array(b)].map(x => x.toString(16).padStart(2, "0")).join("");

async function calcularProva(senha, salHex, iteracoes, nonce, nonceCliente, usuario) {
  const cru = await crypto.subtle.importKey(
    "raw", new TextEncoder().encode(senha), "PBKDF2", false, ["deriveBits"]);
  const dk = await crypto.subtle.deriveBits(
    { name:"PBKDF2", salt: bytesDeHex(salHex), iterations: iteracoes, hash:"SHA-256" },
    cru, 256);
  const chave = await crypto.subtle.importKey(
    "raw", dk, { name:"HMAC", hash:"SHA-256" }, false, ["sign"]);
  const msg = new TextEncoder().encode(`${nonce},${nonceCliente},${usuario}`);
  return hexDeBytes(await crypto.subtle.sign("HMAC", chave, msg));
}'''
assert s.count(velho_api)==1
s = s.replace(velho_api, novo_api)

# 2. entrar() usa desafio-resposta quando da, Base64 quando nao da
velho_entrar = '''    } else {
      // Base64 no login. Nao e cifra -- so tira a senha do olho e do escape
      // do JSON. O que protege de verdade e o tunel.
      const b64 = s => btoa(unescape(encodeURIComponent(s)));
      const r = await fetch("/api", { method:"POST",
        headers:{"Content-Type":"application/json"},
        body: JSON.stringify({ token, op:"login",
          usuario_b64:b64(usuario), senha_b64:b64(senha) }) });
      const j = await r.json();
      if (!j.ok) throw new Error(j.erro);
      est.sessao = j.sessao || null;
      est.usuario = j.resultado;
    }'''
novo_entrar = '''    } else if (podeProvar()) {
      // Caminho bom: a senha nao sai desta maquina.
      recado.textContent = "Derivando a prova…";
      const d = await api("desafio", { usuario });
      const nonceCliente = hexDeBytes(crypto.getRandomValues(new Uint8Array(12)));
      const prova = await calcularProva(
        senha, d.sal, d.iteracoes, d.nonce, nonceCliente, usuario);
      est.usuario = await api("login", { usuario, prova, nonce_cliente:nonceCliente });
    } else {
      // Reserva: Base64. Nao e cifra -- so tira a senha do olho e do escape
      // do JSON. O que protege de verdade e o tunel.
      const b64 = s => btoa(unescape(encodeURIComponent(s)));
      est.usuario = await api("login",
        { usuario_b64:b64(usuario), senha_b64:b64(senha) });
    }'''
assert s.count(velho_entrar)==1
s = s.replace(velho_entrar, novo_entrar)
open(p,'w').write(s)
print("ui ok")

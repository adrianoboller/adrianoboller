# Add blacklist with auto-blocking and firewall hook
# 27/08 19:25

p='crates/phxsql-server/src/lib.rs'
s=open(p).read()
s=s.replace("pub mod acesso;\npub mod config;","pub mod acesso;\npub mod blacklist;\npub mod config;")
s=s.replace("pub use acesso::{Acesso, LogAcessos, ResumoIp};","pub use acesso::{Acesso, LogAcessos, ResumoIp};\npub use blacklist::{Blacklist, Bloqueio, Firewall, Politica};")
open(p,'w').write(s)

#!/bin/bash
# Monta o juiz local do PhxJev num conteiner sem acesso as releases do GitHub.
#
# Por que do fonte: o binario da release redireciona para github.com/.../releases,
# que o proxy desta casa recusa; o git do repositorio publico e o proxy.golang.org
# passam. Medido em 24/09/2026: go build 1 min 03 s, llama-server 2 min 52 s.
#
# Duas armadilhas ja pagas:
#   - GGML_NATIVE=ON nao combina com GGML_BACKEND_DL (o cmake recusa); a saida e
#     uma variante fixa com as instrucoes da CPU (AVX2/FMA/F16C aqui).
#   - Sem o llama-server ao lado, o `ollama serve` sobe e so falha na primeira
#     pergunta, com «llama-server binary not found».
set -euo pipefail
BASE=${BASE:-/home/user/ollama}
MODELOS=${MODELOS:-"qwen2.5:1.5b qwen2.5:3b qwen2.5:7b"}

[ -d "$BASE/ollama/.git" ] || GIT_LFS_SKIP_SMUDGE=1 git clone -q --depth 1 https://github.com/ollama/ollama "$BASE/ollama"
cd "$BASE/ollama"
[ -x "$BASE/bin/ollama" ] || go build -o "$BASE/bin/ollama" .

if [ ! -x "$BASE/lib/ollama/llama-server" ]; then
  cmake -S llama/server --preset cpu -DGGML_CPU_ALL_VARIANTS=OFF -DGGML_NATIVE=OFF \
        -DGGML_AVX=ON -DGGML_AVX2=ON -DGGML_FMA=ON -DGGML_F16C=ON
  cmake --build build/llama-server-cpu -j"$(nproc)"
  mkdir -p "$BASE/lib/ollama"
  cp -a build/llama-server-cpu/bin/. "$BASE/lib/ollama/"
fi

export OLLAMA_HOST=127.0.0.1:11434 OLLAMA_MODELS="$BASE/modelos"
if ! curl -fsS "http://$OLLAMA_HOST/api/version" >/dev/null 2>&1; then
  nohup "$BASE/bin/ollama" serve >"$BASE/serve.log" 2>&1 &
  for _ in $(seq 50); do curl -fsS "http://$OLLAMA_HOST/api/version" >/dev/null 2>&1 && break; sleep 0.2; done
fi
for m in $MODELOS; do "$BASE/bin/ollama" pull "$m" >/dev/null; echo "modelo pronto: $m"; done
echo "juiz local no ar em http://$OLLAMA_HOST"

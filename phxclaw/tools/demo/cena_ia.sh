source "$(dirname "$0")/comum.sh"
titulo "IA local: modelo de linguagem (Ollama) e voz para texto (whisper.cpp)" \
       "sem nuvem e sem credencial; o modelo de 135M parâmetros é mínimo de propósito — a qualidade da resposta é dele, não do PhxClaw"
roda "./target/debug/examples/perguntar smollm2:135m 'Name three benefits of the Rust programming language.'" 3
roda "./target/debug/examples/transcrever /var/tmp/whisper.cpp/build/bin/whisper-cli /var/tmp/ggml-tiny.en.bin 921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f /var/tmp/whisper.cpp/samples/jfk.wav" 2
nota "o modelo de voz só roda depois de conferido contra o SHA-256 publicado pela fonte" 4

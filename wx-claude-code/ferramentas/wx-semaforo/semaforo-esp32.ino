// Semaforo fisico do WX Claude Code: ESP32 (ou ESP8266) com tres LEDs.
// Recebe GET /luz?cor=verde|amarelo|vermelho do hook e acende um por vez.
// Configure o WiFi, grave, e ponha o IP em ~/.wx-claude-code/semaforo.json:
//   {"url": "http://<ip-do-esp32>/luz"}
#include <WiFi.h>
#include <WebServer.h>

const char* SSID = "sua-rede";
const char* SENHA = "sua-senha";
const int PINO_VERMELHO = 25, PINO_AMARELO = 26, PINO_VERDE = 27;  // resistor de 220 ohm em cada LED

WebServer servidor(80);

void acender(int pino) {
  digitalWrite(PINO_VERMELHO, pino == PINO_VERMELHO);
  digitalWrite(PINO_AMARELO, pino == PINO_AMARELO);
  digitalWrite(PINO_VERDE, pino == PINO_VERDE);
}

void luz() {
  String cor = servidor.arg("cor");
  if (cor == "vermelho") acender(PINO_VERMELHO);
  else if (cor == "amarelo") acender(PINO_AMARELO);
  else if (cor == "verde") acender(PINO_VERDE);
  else { servidor.send(400, "text/plain", "cor: verde, amarelo ou vermelho"); return; }
  servidor.send(204);
}

void setup() {
  pinMode(PINO_VERMELHO, OUTPUT); pinMode(PINO_AMARELO, OUTPUT); pinMode(PINO_VERDE, OUTPUT);
  acender(PINO_AMARELO);                       // ligando: amarelo ate o WiFi subir
  WiFi.begin(SSID, SENHA);
  while (WiFi.status() != WL_CONNECTED) delay(200);
  servidor.on("/luz", luz);
  servidor.begin();
  acender(PINO_VERDE);                         // pronto
}

void loop() { servidor.handleClient(); }

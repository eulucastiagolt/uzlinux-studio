# Guia Oficial de Desenvolvimento de Plugins — UzlinuxStudioDeck

> **Compatibilidade:** UzlinuxStudioDeck v0.1.0+ / OpenDeck / Padrão Elgato Stream Deck & OpenAction  
> **Público-alvo:** Desenvolvedores de software, integradores de sistemas e criadores de automação.  
> **Linguagens Suportadas:** Qualquer linguagem que execute no Linux (Rust, Python, Node.js, Go, C/C++, Shell Script, etc.).

---

## 1. Visão Geral e Arquitetura

O **UzlinuxStudioDeck** utiliza uma arquitetura baseada em **isolamento de processos (Process Isolation)** e comunicação assíncrona orientada a eventos. Diferente de aplicações que executam scripts dentro do próprio processo principal (o que pode travar a interface gráfica ou causar falhas de memória), o UzlinuxStudioDeck gerencia plugins como processos independentes do sistema operacional Linux.

### Diagrama de Comunicação

```text
┌────────────────────────────────────────────────────────────────────────┐
│                          UzlinuxStudioDeck                             │
│                                                                        │
│  ┌───────────────────────┐             ┌────────────────────────────┐  │
│  │   Plugin WebSocket    │             │      Asset HTTP Server     │  │
│  │  Hub (ws://127.0.0.1) │             │ (http://127.0.0.1:<port>)  │  │
│  └───────────▲───────────┘             └─────────────▲──────────────┘  │
│              │                                       │                 │
│              │ JSON RPC                              │ HTTP / Assets   │
│              │ Bidirecional                          │ & HTML Bridge   │
└──────────────┼───────────────────────────────────────┼─────────────────┘
               │                                       │
               ▼                                       ▼
┌──────────────────────────────┐       ┌─────────────────────────────────┐
│     Processo do Plugin       │       │       Property Inspector        │
│   (Python / Rust / Node /    │◄──────┤ (Painel de Configuração HTML/JS │
│         Binário ELF)         │       │     dentro da janela Tauri)     │
└──────────────────────────────┘       └─────────────────────────────────┘
```

### Componentes Chave:
1. **Plugin Process**: Um executável ou script lançado pelo UzlinuxStudioDeck no início da aplicação ou sob demanda.
2. **Plugin WebSocket Hub**: Servidor WebSocket local seguro que recebe comandos e despacha eventos de hardware (cliques de botão, giros de dials, foco de janelas).
3. **Asset Server**: Servidor HTTP embutido que serve os arquivos estáticos (ícones, páginas HTML, scripts CSS/JS) das pastas dos plugins.
4. **Property Inspector (PI)**: A interface visual que aparece na coluna lateral direita do UzlinuxStudioDeck quando o usuário clica em um botão para configurá-lo. É renderizada em uma WebView isolada.

---

## 2. Estrutura de Diretórios de um Plugin

No Linux, todos os plugins instalados ficam localizados em:
```bash
~/.config/uzlinux-studiodeck/plugins/
```

Cada plugin reside em uma pasta cujo nome **deve terminar obrigatoriamente** com `.sdPlugin` e utilizar a convenção de nomenclatura reversa de domínio (`com.autor.nomeplugin.sdPlugin`).

### Layout de Pastas Padrão:

```text
com.exemplo.meuplugin.sdPlugin/
├── manifest.json                  # [OBRIGATÓRIO] Metadados, ações e caminhos
├── meu_plugin                     # [OBRIGATÓRIO] Executável ou script principal (chmod +x)
├── assets/                        # Ícones das ações e estados do botão
│   ├── icon.png                   # Ícone de 72x72 exibido na paleta de ações
│   ├── icon@2x.png                # Ícone em alta resolução de 144x144
│   ├── state_off.png              # Imagem do estado desligado
│   └── state_on.png               # Imagem do estado ligado
└── property_inspector/            # [OPCIONAL] Interface de configuração
    ├── pi.html                    # Página HTML renderizada na barra lateral
    ├── pi.css                     # Estilos visuais da interface
    └── pi.js                      # Lógica de comunicação com o plugin
```

---

## 3. O Arquivo `manifest.json`

O arquivo `manifest.json` é a certidão de nascimento do seu plugin. Ele define as ações disponíveis, categorias, ícones e qual executável o UzlinuxStudioDeck deve iniciar.

### Exemplo Completo de `manifest.json`:

```json
{
  "Name": "Monitor do Sistema",
  "Description": "Exibe o uso de CPU, memória RAM e temperatura no UzlinuxStudioDeck",
  "Author": "Seu Nome",
  "Version": "1.0.0",
  "PluginUUID": "com.exemplo.monitor",
  "CodePathLin": "meu_plugin",
  "Icon": "assets/icon",
  "Category": "Sistema Linux",
  "OS": [
    {
      "Platform": "linux"
    }
  ],
  "Actions": [
    {
      "UUID": "com.exemplo.monitor.cpu",
      "Name": "Uso de CPU",
      "Tooltip": "Mostra o percentual atual de CPU",
      "Icon": "assets/cpu",
      "Controllers": [
        "Keypad"
      ],
      "PropertyInspectorPath": "property_inspector/pi.html",
      "States": [
        {
          "Image": "assets/cpu_idle",
          "Title": "CPU",
          "TitleColor": "#ffffff",
          "TitleAlignment": "middle",
          "FontSize": 12,
          "ShowTitle": true
        }
      ]
    }
  ]
}
```

### Detalhamento dos Campos do `manifest.json`:

| Campo | Tipo | Descrição |
| :--- | :--- | :--- |
| `Name` | String | Nome visível do plugin na interface. |
| `Description` | String | Pequeno resumo do propósito do plugin. |
| `Author` | String | Nome ou organização do criador. |
| `Version` | String | Versão SemVer (`1.0.0`). |
| `PluginUUID` | String | Identificador único global (ex: `com.suaempresa.seuplugin`). |
| `CodePathLin` | String | Caminho relativo para o executável principal no Linux. Se for Python ou Node, pode ser um script com Shebang (`#!/usr/bin/env python3`) e permissão `+x`. |
| `Icon` | String | Caminho relativo para o ícone sem extensão (o sistema procura `.png`). |
| `Category` | String | Nome da categoria onde as ações serão agrupadas na barra lateral de ações. |
| `OS` | Array | Lista de sistemas operacionais suportados. No Linux: `[{"Platform": "linux"}]`. |
| `Actions` | Array | Lista de ações fornecidas por este plugin. |

### Detalhamento de cada Ação (`Actions`):
- `UUID`: Identificador único da ação (ex: `com.exemplo.monitor.cpu`).
- `Name`: Nome da ação exibido ao usuário.
- `Controllers`: Tipos de controle suportados. Para teclados de botões LCD normais como o Ulanzi D200 e Stream Deck, use `["Keypad"]`.
- `PropertyInspectorPath`: Caminho para a página HTML de configuração (opcional).
- `States`: Lista de estados gráficos da tecla (estado 0 = desligado/padrão, estado 1 = ligado, etc.).

---

## 4. Ciclo de Vida e Inicialização do Plugin

Quando o **UzlinuxStudioDeck** inicia, ele varre a pasta de plugins e executa o binário especificado em `CodePathLin` passando **exatamente** os seguintes argumentos de linha de comando:

```bash
./meu_plugin -port <PORT> -pluginUUID <UUID> -registerEvent registerPlugin -info '<INFO_JSON>'
```

### Argumentos da Linha de Comando:
* `-port`: Número da porta TCP do WebSocket Server local (ex: `57116`).
* `-pluginUUID`: O mesmo UUID informado no `manifest.json`.
* `-registerEvent`: O nome do evento de registro (sempre `registerPlugin`).
* `-info`: Uma string JSON contendo dados da aplicação e dos decks conectados:

```json
{
  "application": {
    "font": "Liberation Sans",
    "language": "pt",
    "platform": "linux",
    "platformVersion": "6.x",
    "version": "2.14.0"
  },
  "plugin": {
    "uuid": "com.exemplo.monitor",
    "version": "1.0.0"
  },
  "devicePixelRatio": 1,
  "devices": [
    {
      "id": "e9-02C53A015U3671380",
      "name": "Ulanzi D200",
      "size": {
        "columns": 5,
        "rows": 3
      },
      "type": 0
    }
  ],
  "colors": {}
}
```

---

## 5. Protocolo de Comunicação WebSocket

Assim que o executável do plugin é iniciado, ele deve se conectar ao endereço `ws://127.0.0.1:<PORT>`.

### Passo 1: Handshake de Registro (Obrigatório)
A primeira mensagem que o plugin deve enviar pelo WebSocket é o evento de registro:

```json
{
  "event": "registerPlugin",
  "uuid": "com.exemplo.monitor"
}
```

Sem esse evento, o UzlinuxStudioDeck não associará as ações do perfil ao seu processo e descartará qualquer comunicação posterior.

---

### Eventos Enviados pela Aplicação ao Plugin (Outbound)

#### 1. `willAppear`
Disparado quando um botão configurado com a ação do seu plugin se torna visível na tela (o usuário abriu a página ou trocou de perfil).
```json
{
  "event": "willAppear",
  "action": "com.exemplo.monitor.cpu",
  "context": "ctx_unique_key_id_1234",
  "device": "e9-02C53A015U3671380",
  "payload": {
    "settings": {
      "interval": 2,
      "alertThreshold": 85
    },
    "coordinates": {
      "column": 1,
      "row": 0
    },
    "state": 0,
    "isInMultiAction": false,
    "controller": "Keypad"
  }
}
```
> [!NOTE]
> O campo **`context`** é um identificador único daquela instância física do botão. Você deve guardar esse `context` para atualizar a imagem ou o texto daquele botão no futuro.

#### 2. `willDisappear`
Disparado quando o botão deixa de estar visível (o usuário mudou de perfil ou foi para outra página de botões). Use este evento para pausar timers de atualização e economizar recursos.
```json
{
  "event": "willDisappear",
  "action": "com.exemplo.monitor.cpu",
  "context": "ctx_unique_key_id_1234",
  "device": "e9-02C53A015U3671380",
  "payload": {
    "settings": {},
    "coordinates": { "column": 1, "row": 0 },
    "state": 0,
    "isInMultiAction": false,
    "controller": "Keypad"
  }
}
```

#### 3. `keyDown`
Disparado no momento exato em que o usuário aperta a tecla física no aparelho.
```json
{
  "event": "keyDown",
  "action": "com.exemplo.monitor.cpu",
  "context": "ctx_unique_key_id_1234",
  "device": "e9-02C53A015U3671380",
  "payload": {
    "settings": { "interval": 2 },
    "coordinates": { "column": 1, "row": 0 },
    "state": 0,
    "userDesiredState": 0,
    "isInMultiAction": false,
    "controller": "Keypad"
  }
}
```

#### 4. `keyUp`
Disparado quando o usuário solta a tecla física.
```json
{
  "event": "keyUp",
  "action": "com.exemplo.monitor.cpu",
  "context": "ctx_unique_key_id_1234",
  "device": "e9-02C53A015U3671380",
  "payload": {
    "settings": { "interval": 2 },
    "coordinates": { "column": 1, "row": 0 },
    "state": 0,
    "userDesiredState": 0,
    "isInMultiAction": false,
    "controller": "Keypad"
  }
}
```

#### 5. `didReceiveSettings`
Disparado quando o usuário altera e salva configurações na interface do Property Inspector.
```json
{
  "event": "didReceiveSettings",
  "action": "com.exemplo.monitor.cpu",
  "context": "ctx_unique_key_id_1234",
  "device": "e9-02C53A015U3671380",
  "payload": {
    "settings": {
      "interval": 5,
      "unit": "percentage"
    },
    "coordinates": { "column": 1, "row": 0 },
    "isInMultiAction": false
  }
}
```

#### 6. `sendToPlugin`
Disparado quando o Property Inspector (HTML/JS) envia uma mensagem personalizada diretamente para o backend do seu plugin.
```json
{
  "event": "sendToPlugin",
  "action": "com.exemplo.monitor.cpu",
  "context": "ctx_unique_key_id_1234",
  "payload": {
    "command": "testSensor",
    "params": [1, 2, 3]
  }
}
```

---

### Eventos Enviados pelo Plugin à Aplicação (Inbound)

#### 1. `setImage` (Atualizar Imagem do Botão)
Atualiza a imagem exibida na tela LCD do botão. O UzlinuxStudioDeck suporta URIs no formato Base64 PNG.
```json
{
  "event": "setImage",
  "context": "ctx_unique_key_id_1234",
  "payload": {
    "image": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAFAAAABQCAYAAACOEfKtAAA...",
    "target": 0,
    "state": 0
  }
}
```
* `target`: `0` para ambos (hardware e tela virtual), `1` para hardware apenas, `2` para software apenas.
* Para telas como Ulanzi D200, a resolução padrão é **72x72 pixels**.

#### 2. `setTitle` (Atualizar Texto do Botão)
Atualiza o título dinâmico textual do botão. O UzlinuxStudioDeck renderiza o texto dinamicamente com tipografia nítida e contorno de alto contraste.
```json
{
  "event": "setTitle",
  "context": "ctx_unique_key_id_1234",
  "payload": {
    "title": "42%",
    "target": 0,
    "state": 0
  }
}
```

#### 3. `showAlert` (Indicador de Alerta / Erro)
Exibe momentaneamente um ícone amarelo de alerta sobre o botão por 1.5 segundos (útil se uma requisição de rede falhar ou a API retornar erro).
```json
{
  "event": "showAlert",
  "context": "ctx_unique_key_id_1234"
}
```

#### 4. `showOk` (Indicador de Sucesso)
Exibe momentaneamente um ícone verde de visto (check) sobre o botão por 1.5 segundos.
```json
{
  "event": "showOk",
  "context": "ctx_unique_key_id_1234"
}
```

#### 5. `setState` (Alternar Estado da Tecla)
Muda o estado visual atual da tecla (por exemplo, de Desligado `0` para Ligado `1`).
```json
{
  "event": "setState",
  "context": "ctx_unique_key_id_1234",
  "payload": {
    "state": 1
  }
}
```

#### 6. `setSettings` (Salvar Configurações da Tecla)
Grava dados persistentes específicos para essa instância de botão no arquivo de perfil no disco.
```json
{
  "event": "setSettings",
  "context": "ctx_unique_key_id_1234",
  "payload": {
    "targetIp": "192.168.1.100",
    "port": 8080
  }
}
```

#### 7. `getSettings` (Ler Configurações Atuais)
Solicita que o UzlinuxStudioDeck responda com o evento `didReceiveSettings` contendo os dados salvos.
```json
{
  "event": "getSettings",
  "context": "ctx_unique_key_id_1234"
}
```

#### 8. `openUrl` (Abrir Navegador Web)
Pede ao sistema operacional para abrir uma página web no navegador padrão do Linux.
```json
{
  "event": "openUrl",
  "payload": {
    "url": "https://meuservico.com/status"
  }
}
```

#### 9. `sendToPropertyInspector` (Enviar Dados para a UI)
Envia dados em tempo real para a janela de configuração aberta.
```json
{
  "event": "sendToPropertyInspector",
  "action": "com.exemplo.monitor.cpu",
  "context": "ctx_unique_key_id_1234",
  "payload": {
    "availableSensors": ["cpu_thermal", "nvme0", "gpu_core"]
  }
}
```

---

## 6. Desenvolvendo o Property Inspector (UI em HTML/JS)

O **Property Inspector** é carregado no UzlinuxStudioDeck em um painel lateral sempre que o botão correspondente é selecionado. Ele é servido via HTTP local (`http://127.0.0.1:<ASSET_PORT>/plugins/...`).

### A Ponte de Mensagens Automática
O UzlinuxStudioDeck injeta automaticamente uma ponte `window.postMessage` que funciona nativamente tanto no Stream Deck tradicional quanto no ambiente Tauri v2 do UzlinuxStudioDeck:

```html
<!DOCTYPE html>
<html lang="pt-BR">
<head>
  <meta charset="UTF-8">
  <title>Configurações da Ação</title>
  <style>
    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      font-size: 13px;
      color: #e0e0e0;
      background: #18181b;
      margin: 0;
      padding: 16px;
    }
    .form-group {
      margin-bottom: 12px;
    }
    label {
      display: block;
      margin-bottom: 4px;
      color: #a1a1aa;
      font-size: 11px;
      text-transform: uppercase;
      font-weight: 600;
    }
    input, select {
      width: 100%;
      box-sizing: border-box;
      padding: 8px 10px;
      border: 1px solid #3f3f46;
      background: #27272a;
      color: #ffffff;
      border-radius: 6px;
      outline: none;
    }
    input:focus, select:focus {
      border-color: #3b82f6;
    }
    button {
      background: #2563eb;
      color: white;
      border: none;
      padding: 8px 16px;
      border-radius: 6px;
      cursor: pointer;
      font-weight: 600;
    }
    button:hover {
      background: #1d4ed8;
    }
  </style>
</head>
<body>

  <div class="form-group">
    <label for="apiUrl">URL da API / Servidor</label>
    <input type="text" id="apiUrl" placeholder="http://127.0.0.1:8080/metrics">
  </div>

  <div class="form-group">
    <label for="refreshRate">Intervalo de Atualização (segundos)</label>
    <input type="number" id="refreshRate" value="5" min="1" max="60">
  </div>

  <button id="saveBtn">Salvar Alterações</button>

  <script>
    let currentContext = null;
    let currentAction = null;

    // 1. O UzlinuxStudioDeck chama esta função quando a WebView carrega
    function connectElgatoStreamDeckSocket(inPort, inPropertyInspectorUUID, inRegisterEvent, inInfo, inActionInfo) {
      const actionData = JSON.parse(inActionInfo);
      currentContext = inPropertyInspectorUUID;
      currentAction = actionData.action;

      // Se já existirem configurações salvas, preenche os campos
      if (actionData.payload && actionData.payload.settings) {
        populateFields(actionData.payload.settings);
      }
    }

    // 2. Escuta eventos enviados pela aplicação ou pelo plugin
    window.addEventListener("message", (event) => {
      const data = event.data;
      if (!data) return;

      if (data.event === "didReceiveSettings") {
        if (data.payload && data.payload.settings) {
          populateFields(data.payload.settings);
        }
      } else if (data.event === "sendToPropertyInspector") {
        console.log("Mensagem em tempo real vinda do plugin:", data.payload);
      }
    });

    function populateFields(settings) {
      if (settings.apiUrl) document.getElementById("apiUrl").value = settings.apiUrl;
      if (settings.refreshRate) document.getElementById("refreshRate").value = settings.refreshRate;
    }

    // 3. Salvar configurações quando o usuário clica ou altera valores
    document.getElementById("saveBtn").addEventListener("click", () => {
      const settings = {
        apiUrl: document.getElementById("apiUrl").value,
        refreshRate: parseInt(document.getElementById("refreshRate").value, 10)
      };

      // Dispara o evento de gravação para a aplicação
      window.parent.postMessage({
        event: "setSettings",
        context: currentContext,
        payload: settings
      }, "*");
    });
  </script>
</body>
</html>
```

---

## 7. Exemplos Práticos de Implementação

### Exemplo A: Plugin em Python 3

O Python é uma das linguagens mais fáceis e rápidas para criar plugins. Não necessita compilação prévia e funciona com a biblioteca padrão ou pacotes como `websockets`.

Crie o arquivo `main.py` e execute `chmod +x main.py`:

```python
#!/usr/bin/env python3
import sys
import json
import asyncio
import websockets
import urllib.request

# 1. Parse dos argumentos passados pelo UzlinuxStudioDeck
port = None
plugin_uuid = None

for i, arg in enumerate(sys.argv):
    if arg == "-port" and i + 1 < len(sys.argv):
        port = int(sys.argv[i + 1])
    elif arg == "-pluginUUID" and i + 1 < len(sys.argv):
        plugin_uuid = sys.argv[i + 1]

if not port or not plugin_uuid:
    sys.exit("Erro: Argumentos -port e -pluginUUID são obrigatórios.")

# Armazena botões ativos: { context: settings }
active_buttons = {}

async def update_button(ws, context):
    """Consulta dados e atualiza o display do botão"""
    try:
        # Exemplo: consulta IP público ou qualquer API JSON
        with urllib.request.urlopen("https://api.ipify.org?format=json", timeout=2) as res:
            data = json.loads(res.read().decode())
            ip_str = data.get("ip", "Offline")
            # Atualiza o título do botão
            await ws.send(json.dumps({
                "event": "setTitle",
                "context": context,
                "payload": {
                    "title": ip_str,
                    "target": 0
                }
            }))
    except Exception as e:
        # Mostra ícone amarelo de alerta em caso de falha
        await ws.send(json.dumps({
            "event": "showAlert",
            "context": context
        }))

async def background_loop(ws):
    """Loop que roda a cada 10 segundos atualizando todos os botões visíveis"""
    while True:
        for ctx in list(active_buttons.keys()):
            await update_button(ws, ctx)
        await asyncio.sleep(10)

async def main():
    uri = f"ws://127.0.0.1:{port}"
    async with websockets.connect(uri) as ws:
        # Envia handshake de registro imediatamente
        await ws.send(json.dumps({
            "event": "registerPlugin",
            "uuid": plugin_uuid
        }))

        # Inicia loop de segundo plano
        asyncio.create_task(background_loop(ws))

        # Escuta mensagens enviadas pelo UzlinuxStudioDeck
        async for message in ws:
            event_data = json.loads(message)
            event = event_data.get("event")
            context = event_data.get("context")

            if event == "willAppear":
                settings = event_data.get("payload", {}).get("settings", {})
                active_buttons[context] = settings
                await update_button(ws, context)

            elif event == "willDisappear":
                active_buttons.pop(context, None)

            elif event == "keyDown":
                # Quando o usuário pressiona a tecla física, força a atualização imediata
                await update_button(ws, context)
                await ws.send(json.dumps({
                    "event": "showOk",
                    "context": context
                }))

            elif event == "didReceiveSettings":
                new_settings = event_data.get("payload", {}).get("settings", {})
                active_buttons[context] = new_settings
                await update_button(ws, context)

if __name__ == "__main__":
    asyncio.run(main())
```

---

### Exemplo B: Plugin de Alta Performance em Rust

Para plugins que exigem máxima velocidade, controle de concorrência ou comunicação de baixo nível (como drivers ou monitores de hardware).

`Cargo.toml`:
```toml
[package]
name = "meu-plugin-rust"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.26"
futures-util = "0.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
clap = { version = "4.4", features = ["derive"] }
```

`src/main.rs`:
```rust
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short = 'p', long = "port")]
    port: u16,
    #[arg(long = "pluginUUID")]
    plugin_uuid: String,
    #[arg(long = "registerEvent")]
    register_event: String,
    #[arg(long = "info")]
    info: String,
}

#[derive(Serialize)]
struct RegisterMessage {
    event: String,
    uuid: String,
}

#[derive(Deserialize, Debug)]
struct InboundEvent {
    event: String,
    context: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let url = format!("ws://127.0.0.1:{}", args.port);

    let (ws_stream, _) = connect_async(&url).await?;
    let (mut write, mut read) = ws_stream.split();

    // Handshake
    let reg = RegisterMessage {
        event: "registerPlugin".to_string(),
        uuid: args.plugin_uuid.clone(),
    };
    write.send(Message::Text(serde_json::to_string(&reg)?)).await?;

    println!("Plugin {} registrado com sucesso na porta {}", args.plugin_uuid, args.port);

    while let Some(msg_res) = read.next().await {
        if let Ok(Message::Text(text)) = msg_res {
            if let Ok(ev) = serde_json::from_str::<InboundEvent>(&text) {
                match ev.event.as_str() {
                    "keyDown" => {
                        if let Some(ctx) = ev.context {
                            let ok_msg = serde_json::json!({
                                "event": "showOk",
                                "context": ctx
                            });
                            let _ = write.send(Message::Text(ok_msg.to_string())).await;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
```

---

## 8. Boas Práticas e Resolução de Problemas

1. **Permissões de Execução no Linux**:
   Lembre-se sempre de conceder permissão de execução ao executável ou script antes de testar:
   ```bash
   chmod +x ~/.config/uzlinux-studiodeck/plugins/com.exemplo.meuplugin.sdPlugin/meu_plugin
   ```

2. **Gerenciamento de Recursos (Contextos)**:
   * Sempre mantenha uma tabela de instâncias ativas (`active_contexts`).
   * Adicione a instância no evento `willAppear`.
   * Remova a instância no evento `willDisappear`.
   * **Nunca** continue rodando loops com timers para botões que já receberam `willDisappear`.

3. **Resolução de Imagens**:
   * O Ulanzi D200 e a maioria dos Stream Decks operam na grade com botões de proporção 1:1.
   * Gere imagens com **72x72 pixels** (ou **144x144** para densidade 2x).
   * Utilize formato PNG com canal alfa (RGBA) para bordas transparentes e cantos suaves.

4. **Logs e Depuração**:
   * O UzlinuxStudioDeck captura e exibe no terminal qualquer saída enviada pelo seu plugin para o `stdout` e `stderr`.
   * Para visualizar os logs em tempo real, basta rodar o `UzlinuxStudioDeck` diretamente em um terminal.

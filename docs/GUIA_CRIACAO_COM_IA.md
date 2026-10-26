# Como Criar Plugins para o UzlinuxStudioDeck Usando Inteligência Artificial (Sem Saber Programar)

> **Você não precisa saber programação para criar botões incríveis para o seu deck!**  
> Este guia ensina passo a passo como instruir qualquer Inteligência Artificial (ChatGPT, Claude, Google Gemini, Antigravity, etc.) a gerar plugins completos, funcionais e prontos para uso no seu UzlinuxStudioDeck.

---

## 1. Como Funciona o Processo

As IAs modernas são excelentes programadoras, mas elas precisam saber as **regras exatas** de como o UzlinuxStudioDeck se comunica com os plugins (que utiliza uma conexão WebSocket local em segundo plano).

Se você apenas pedir *"crie um plugin para stream deck"*, a IA pode criar um código genérico que não vai funcionar no Linux. 

Para que a IA crie o código perfeito na primeira tentativa, você só precisa fazer duas coisas:
1. **Copiar o nosso "Prompt Mestre"** (que ensina à IA todas as regras do aplicativo).
2. **Preencher a sua ideia** respondendo a 4 perguntas simples em português.

---

## 2. Antes de Pedir à IA: O Roteiro em 4 Perguntas

Pegue um bloco de notas e responda às 4 perguntas abaixo sobre o botão que você quer criar:

### Pergunta 1: O que o botão vai fazer?
* *Exemplo A:* "Quero ver a cotação do Dólar e do Bitcoin na tela do botão."
* *Exemplo B:* "Quero um botão que pause ou toque músicas no Spotify."
* *Exemplo C:* "Quero um botão que ligue e desligue uma lâmpada inteligente da minha casa via requisição HTTP."
* *Exemplo D:* "Quero um botão que tire uma captura de tela da tela inteira e salve na minha pasta Imagens."

### Pergunta 2: O que deve aparecer na telinha do botão?
* *Texto:* Um texto simples que muda (ex: "R$ 5,60", "Tocando", "LIGADO / DESLIGADO").
* *Imagem/Ícone:* Um ícone estático ou um ícone que muda de cor dependendo do status.

### Pergunta 3: O que acontece quando você aperta o botão?
* Ele executa uma ação instantânea? (Ex: mutar o microfone, mudar a música, fazer um backup).
* Ele força a atualização das informações na tela? (Ex: buscar a cotação mais recente agora).

### Pergunta 4: O usuário precisa configurar alguma coisa?
* Precisa de uma chave de API secreta?
* Precisa digitar o IP de um dispositivo na rede local?
* Precisa escolher um intervalo de tempo em segundos?
*(Se a resposta for sim, a IA criará uma tela com campos para você preencher dentro do aplicativo).*

---

## 3. O "Prompt Mestre" (Copie e Cole na sua IA)

Copie todo o texto abaixo e cole na sua IA de preferência (ChatGPT, Claude, Gemini). **Lembre-se apenas de preencher a parte final com a sua ideia!**

```markdown
Você é um desenvolvedor sênior de software especialista no ecossistema Stream Deck e OpenDeck para Linux (aplicativo UzlinuxStudioDeck).
Sua tarefa é criar um plugin completo, 100% funcional e pronto para uso com base na especificação que fornecerei no final.

### REGRAS TÉCNICAS RÍGIDAS DO UZLINUXSTUDIODECK:
1. LINGUAGEM: Utilize preferencialmente Python 3 (com `asyncio` e `websockets`) ou Node.js, pois não exigem compilação manual por parte do usuário no Linux. O arquivo principal deve ter a linha Shebang no início (ex: `#!/usr/bin/env python3`).
2. INICIALIZAÇÃO VIA LINHA DE COMANDO: O UzlinuxStudioDeck inicia o plugin passando obrigatoriamente os seguintes parâmetros:
   `-port <PORTA_WEBSOCKET> -pluginUUID <UUID_DO_PLUGIN> -registerEvent registerPlugin -info '<INFO_JSON>'`
   Seu script deve capturar o `-port` e o `-pluginUUID` da lista de argumentos `sys.argv`.
3. HANDSHAKE INICIAL OBRIGATÓRIO: Assim que conectar ao WebSocket `ws://127.0.0.1:<port>`, o script DEVE enviar imediatamente a mensagem de registro:
   `{"event": "registerPlugin", "uuid": "<pluginUUID>"}`
4. EVENTOS RECEBIDOS PELO PLUGIN:
   - `willAppear`: Enviado quando a tecla se torna visível na tela. Contém `context` (identificador único daquela tecla física) e `payload.settings` (configurações salvas daquela tecla). O plugin deve guardar esse `context` para saber onde atualizar as informações.
   - `willDisappear`: A tecla saiu de visão (trocou de página ou perfil). O plugin deve parar de atualizar esse `context`.
   - `keyDown`: A tecla física foi pressionada pelo usuário.
   - `keyUp`: A tecla física foi solta.
   - `didReceiveSettings`: O usuário salvou novas configurações no Property Inspector.
5. EVENTOS ENVIADOS PELO PLUGIN PARA ATUALIZAR O BOTÃO:
   - `setTitle`: Atualiza o texto na telinha:
     `{"event": "setTitle", "context": context, "payload": {"title": "Texto", "target": 0}}`
   - `setImage`: Atualiza a imagem da tecla com um Base64 PNG de 72x72:
     `{"event": "setImage", "context": context, "payload": {"image": "data:image/png;base64,...", "target": 0}}`
   - `showOk`: Pisca um visto verde de sucesso por 1.5s:
     `{"event": "showOk", "context": context}`
   - `showAlert`: Pisca um alerta amarelo de erro por 1.5s:
     `{"event": "showAlert", "context": context}`
6. ESTRUTURA DE ARQUIVOS QUE VOCÊ DEVE GERAR:
   - `manifest.json`: Arquivo de metadados com as ações, nome, versão, UUID e caminhos corretos.
   - Arquivo de código executável principal (ex: `main.py`).
   - Se houver configurações a preencher, gere o arquivo `property_inspector/pi.html` com campos HTML e integração via `postMessage`.

---

### ESPECIFICAÇÃO DO PLUGIN QUE EU QUERO QUE VOCÊ CRIE:
- NOME DO PLUGIN: [Digite o nome aqui, ex: Monitor de Cotação de Moedas]
- O QUE O PLUGIN DEVE FAZER: [Descreva em detalhes o que o botão deve fazer]
- INFORMAÇÕES EXIBIDAS NA TELA: [O que deve aparecer no visor do botão]
- O QUE ACONTECE AO CLICAR: [O que acontece quando o botão for pressionado]
- CONFIGURAÇÕES QUE O USUÁRIO PODE ALTERAR: [Ex: Moeda desejada, intervalo de segundos, token]

Por favor, forneça o código completo de cada arquivo, explicando exatamente em qual pasta salvar cada um e qual comando dar no terminal para que ele funcione.
```

---

## 4. Exemplo Real: Passo a Passo Completo

Vamos ver como isso funciona na prática com um exemplo real.

### O Que Pedimos à IA:
> *"Quero um plugin que consulte a cotação atual do Bitcoin em Dólares na Binance. Na tela do botão, quero que apareça 'BTC' na primeira linha e o valor formatado (ex: '$92.450') na segunda linha. Se eu apertar o botão, ele deve forçar uma atualização imediata e piscar o visto verde. Na tela de configurações, quero poder mudar o intervalo de atualização em segundos."*

### O Que a IA vai Responder:
A IA responderá fornecendo a estrutura e o código dos arquivos:

#### Arquivo 1: `manifest.json`
```json
{
  "Name": "Cotação Bitcoin",
  "Description": "Exibe o preço atual do Bitcoin na Binance em tempo real",
  "Author": "Meu Nome",
  "Version": "1.0.0",
  "PluginUUID": "com.usuario.btcprice",
  "CodePathLin": "main.py",
  "Icon": "assets/icon",
  "Category": "Cripto & Finanças",
  "OS": [
    { "Platform": "linux" }
  ],
  "Actions": [
    {
      "UUID": "com.usuario.btcprice.ticker",
      "Name": "Preço do Bitcoin",
      "Tooltip": "Acompanhe o preço do Bitcoin em tempo real",
      "Icon": "assets/icon",
      "Controllers": ["Keypad"],
      "PropertyInspectorPath": "property_inspector/pi.html",
      "States": [
        {
          "Title": "BTC\nCarregando...",
          "TitleColor": "#f59e0b",
          "TitleAlignment": "middle",
          "FontSize": 11,
          "ShowTitle": true
        }
      ]
    }
  ]
}
```

#### Arquivo 2: `main.py`
```python
#!/usr/bin/env python3
import sys
import json
import asyncio
import urllib.request
import websockets

# Captura argumentos passados pelo UzlinuxStudioDeck
port = None
plugin_uuid = None

for i, arg in enumerate(sys.argv):
    if arg == "-port" and i + 1 < len(sys.argv):
        port = int(sys.argv[i + 1])
    elif arg == "-pluginUUID" and i + 1 < len(sys.argv):
        plugin_uuid = sys.argv[i + 1]

if not port:
    sys.exit(1)

# Armazena os botões ativos no momento: { context: settings }
active_buttons = {}

def get_btc_price():
    """Consulta a API pública da Binance sem precisar de chave"""
    url = "https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT"
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
    with urllib.request.urlopen(req, timeout=3) as resp:
        data = json.loads(resp.read().decode())
        price = float(data["price"])
        return f"${price:,.0f}"

async def update_button(ws, context):
    try:
        price_text = await asyncio.to_thread(get_btc_price)
        title = f"BTC\n{price_text}"
        await ws.send(json.dumps({
            "event": "setTitle",
            "context": context,
            "payload": {
                "title": title,
                "target": 0
            }
        }))
    except Exception:
        await ws.send(json.dumps({
            "event": "showAlert",
            "context": context
        }))

async def loop_atualizacao(ws):
    while True:
        for ctx in list(active_buttons.keys()):
            await update_button(ws, ctx)
        await asyncio.sleep(15)  # Atualiza a cada 15 segundos

async def main():
    async with websockets.connect(f"ws://127.0.0.1:{port}") as ws:
        # 1. Handshake de Registro
        await ws.send(json.dumps({
            "event": "registerPlugin",
            "uuid": plugin_uuid
        }))

        # 2. Inicia o loop de atualização automática
        asyncio.create_task(loop_atualizacao(ws))

        # 3. Escuta eventos do teclado físico
        async for msg_str in ws:
            msg = json.loads(msg_str)
            event = msg.get("event")
            context = msg.get("context")

            if event == "willAppear":
                settings = msg.get("payload", {}).get("settings", {})
                active_buttons[context] = settings
                await update_button(ws, context)

            elif event == "willDisappear":
                active_buttons.pop(context, None)

            elif event == "keyDown":
                # Clique físico: atualiza imediatamente e pisca confirmação
                await update_button(ws, context)
                await ws.send(json.dumps({
                    "event": "showOk",
                    "context": context
                }))

            elif event == "didReceiveSettings":
                settings = msg.get("payload", {}).get("settings", {})
                active_buttons[context] = settings
                await update_button(ws, context)

if __name__ == "__main__":
    asyncio.run(main())
```

---

## 5. Como Instalar o Plugin Gerado no seu Linux

Você não precisa compilar nada. Basta salvar os arquivos na pasta correta:

### Passo 1: Abrir a pasta de plugins
1. Abra o gerenciador de arquivos do seu Linux (Arquivos / Nautilus, Dolphin, etc.).
2. Vá para a sua pasta de usuário (`Pasta Pessoal`).
3. Pressione as teclas **`Ctrl + H`** no teclado para exibir as pastas ocultas.
4. Entre na pasta `.config` e depois na pasta `uzlinux-studiodeck`.
5. Se não existir a pasta `plugins`, crie-a. O caminho completo é:
   ```text
   /home/SEU_USUARIO/.config/uzlinux-studiodeck/plugins/
   ```

### Passo 2: Criar a pasta do seu plugin
Dentro da pasta `plugins`, crie uma pasta com o nome do seu plugin terminando com `.sdPlugin`.  
*Exemplo:* `com.usuario.btcprice.sdPlugin`

### Passo 3: Salvar os arquivos
Coloque dentro dessa pasta os arquivos gerados pela IA:
* `manifest.json`
* `main.py`
* Uma pasta `assets/` com um ícone `icon.png` (qualquer imagem PNG de 72x72 pixels).

### Passo 4: Dar permissão de execução ao script
Abra um terminal (pressione `Ctrl + Alt + T`) e digite o seguinte comando (ajustando o nome da pasta se necessário):

```bash
chmod +x ~/.config/uzlinux-studiodeck/plugins/com.usuario.btcprice.sdPlugin/main.py
```
> *(No Linux, arquivos de programa só podem ser executados pelo sistema se tiverem essa permissão de execução).*

### Passo 5: Abrir o UzlinuxStudioDeck
Feche e abra novamente o **UzlinuxStudioDeck**.  
Seu novo plugin aparecerá instantaneamente na coluna de ações à direita! Basta arrastar a ação para qualquer tela do seu deck.

---

## 6. O Que Fazer se Algo Der Errado (Prompts de Correção)

Se você instalou o plugin e ele não funcionou como esperado, não se preocupe! Copie um dos "Prompts de Correção" abaixo e mande para a mesma conversa com a sua IA:

### Caso 1: O plugin não aparece na lista de ações do UzlinuxStudioDeck
**Mande este prompt para a IA:**
> *"O plugin não está aparecendo na lista de ações do UzlinuxStudioDeck. Verifique se o meu arquivo `manifest.json` tem algum erro de sintaxe, se a categoria está preenchida corretamente e se o campo `CodePathLin` aponta exatamente para o nome do meu executável principal."*

### Caso 2: O botão aparece, mas a tela fica preta ou não atualiza
**Mande este prompt para a IA:**
> *"O botão aparece no deck, mas o texto e a imagem não são atualizados. Lembre-se de que o UzlinuxStudioDeck exige que o script capture o argumento `-port` de `sys.argv` e envie imediatamente o handshake inicial via WebSocket com `{'event': 'registerPlugin', 'uuid': '<meu_uuid>'}`. Além disso, verifique se o script está escutando o evento `willAppear` para capturar o `context` da tecla antes de tentar chamar o `setTitle` ou `setImage`."*

### Caso 3: Ao clicar no botão, nada acontece
**Mande este prompt para a IA:**
> *"Quando eu aperto o botão físico no meu deck, nada acontece. Verifique se o loop do WebSocket está escutando o evento `keyDown` e se a função chamada dentro do `keyDown` não está travando o loop assíncrono."*

### Caso 4: Faltou uma biblioteca Python no meu computador
Se ao rodar o script no terminal aparecer `ModuleNotFoundError: No module named 'websockets'`, basta instalar a biblioteca no seu Linux com o comando:
```bash
pip install websockets
# ou no Debian/Ubuntu/Fedora:
sudo apt install python3-websockets
```
Ou peça para a IA:
> *"Reescreva o script para utilizar apenas a biblioteca padrão do Python (sem bibliotecas externas pip) ou me informe o comando exato para instalar as dependências necessárias no Ubuntu/Debian/Arch."*

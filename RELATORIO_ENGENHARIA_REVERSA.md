# Relatório Técnico de Engenharia Reversa e Compatibilidade: OpenDeck (v2.14.2)

> **Projeto de Referência:** OpenDeck (`me.amankhanna.opendeck`)  
> **Engenharia Reversa realizada com:** MCP REA, Ghidra 12.1.2 e inspeção estática/dinâmica de binário ELF  
> **Objetivo:** Garantir compatibilidade de perfis, plugins e WebSocket no **Uzlinux Studio**  
> **Data:** 25 de Outubro de 2026

---

## 1. Identificação do Binário e Ambiente

* **Caminho do Executável:** `/var/lib/flatpak/app/me.amankhanna.opendeck/x86_64/stable/active/files/bin/opendeck`
* **Formato:** ELF 64-bit LSB PIE (x86-64), dinamicamente linkado
* **Hash SHA-256:** `a35f1de5cf2b8ac714933db3d0e9f7d479401b18a09e9e87a18c9f8df2a9d7bf`
* **Tamanho do Binário:** 57.644.776 bytes (~55 MB)
* **Ponto de Entrada (Entry Point):** `0x20e7d80`
* **Compilador Inspecionado:** Rustc Commit Hash `58cf8f9c35b355199727627d07ae34a4e2846d67` (Rust 1.84+)
* **Ambiente de Execução:** GNOME Platform Runtime 50 (Flatpak), GTK 3, WebKit2GTK 4.1, Wayland/X11, D-Bus

---

## 2. Bibliotecas Dinâmicas Vinculadas (ELF Dynamic Section)

A análise das entradas `NEEDED` no cabeçalho dinâmico do ELF revelou as interfaces do sistema operacional com as quais o OpenDeck interage:

| Biblioteca | Finalidade no OpenDeck |
| :--- | :--- |
| `libwebkit2gtk-4.1.so.0` | Backend de Webview do Tauri v2 para renderizar a interface e os Property Inspectors |
| `libgtk-3.so.0` & `libgdk-3.so.0` | Gerenciamento de janelas, renderização nativa de interface e bandeja do sistema |
| `libcairo.so.2` & `libgdk_pixbuf-2.0.so.0` | Manipulação gráfica em baixo nível, renderização e conversão de buffers de imagem |
| `libudev.so.1` | Descoberta e monitoramento de eventos de conexão USB (hot-plugging de Stream Decks) |
| `libdbus-1.so.3` | Comunicação com o desktop (Single Instance, MPRIS Player, StatusNotifier Tray, Login1 Power) |
| `libxcb.so.1` | Rastreamento de foco de janelas sob X11 e simulação de entradas |
| `libssl.so.3` & `libcrypto.so.3` | Suporte a conexões de rede seguras e downloads de plugins |
| `libfontconfig.so.1` | Descoberta e resolução de fontes instaladas no sistema operacional |

---

## 3. Matriz Exata de Dependências (Crates Rust)

Através da inspeção dos símbolos do módulo `built 0.8.0` embutido na seção `.rodata` do binário, foi possível recuperar a relação completa e exata das crates e versões utilizadas pelo OpenDeck:

| Crate | Versão | Função na Arquitetura |
| :--- | :--- | :--- |
| **`tauri`** | `2.12.1` | Framework central da aplicação desktop |
| **`tauri-build`** | `2.7.1` | Script de compilação e empacotamento Tauri |
| **`tokio`** | `1.49.0` | Runtime assíncrono multithread |
| **`tokio-tungstenite`** | `0.28.0` | Servidor WebSocket assíncrono para os plugins |
| **`tiny_http`** | `0.12.0` | Servidor HTTP embutido para entrega de assets dos Property Inspectors |
| **`elgato-streamdeck`** | `0.14.0` | Driver assíncrono para controladores Elgato Stream Deck |
| **`hidapi`** | `2.6.4` | Camada de baixo nível de comunicação USB HID |
| **`streamdeck-strip-render`**| `0.1.0` | Renderizador de faixas de LCD / telas sensíveis ao toque (ex: Stream Deck Plus) |
| **`image`** | `0.25.9` | Manipulação, codificação e decodificação gráfica (PNG, JPEG, BMP, ICO) |
| **`font-loader`** | `0.11.0` | Carregamento de fontes TrueType/OpenType do sistema para títulos de teclas |
| **`active-win-pos-rs`** | `0.10.1` | Monitoramento da janela atualmente em foco para troca automática de perfil |
| **`sysinfo`** | `0.38.0` | Inspeção de processos e métricas de sistema |
| **`dashmap`** | `6.1.0` | HashMaps concorrentes sem locks globais para instâncias e conexões |
| **`serde`** | `1.0.228` | Framework de serialização / deserialização |
| **`serde_json`** | `1.0.149` | Manipulação de payloads JSON do SDK Elgato e arquivos de perfil |
| **`serde_with`** | `3.16.1` | Helpers avançados de deserialização |
| **`serde-inline-default`** | `1.0.1` | Valores padrão em tempo de deserialização |
| **`json-patch`** | `4.1.0` | Atualizações atômicas em árvores de dados JSON |
| **`reqwest`** | `0.13.1` | Cliente HTTP para requisições externas e downloads da loja de plugins |
| **`zip`** | `6.0.0` | Descompactador de pacotes `.streamDeckPlugin` |
| **`tauri-plugin-single-instance`** | `2.4.0` | Garantia de execução de instância única no sistema operacional |
| **`tauri-plugin-autostart`** | `2.5.1` | Configuração de inicialização automática no boot |
| **`tauri-plugin-dialog`** | `2.6.0` | Caixas de diálogo nativas do sistema |
| **`tauri-plugin-deep-link`** | `2.4.7` | Associação e tratamento de URIs de plugins |
| **`tauri-plugin-log`** | `2.8.0` | Sistema de logging estruturado |
| **`tauri-plugin-cors-fetch`**| `4.2.0` | Suporte a requisições com bypass de restrições CORS |
| **`tauri-plugin-aptabase`** | `1.0.0` | Telemetria anônima opcional |
| **`log`** | `0.4.29` | Facade padrão de logging do ecossistema Rust |
| **`log-panics`** | `2.1.0` | Captura de panics em threads e redirecionamento para arquivo de log |
| **`fix-path-env`** | `0.0.0` | Normalização da variável de ambiente `$PATH` em ambientes desktop |
| **`open`** | `5.3.3` | Abertura de links e URLs no navegador padrão do sistema |
| **`chrono`** | `0.4.43` | Manipulação de datas e timestamps para arquivos de backup e logs |
| **`base64`** | `0.22.1` | Decodificação de imagens inline enviadas por plugins (`data:image/...`) |
| **`urlencoding`** | `2.1.3` | Decodificação de parâmetros de URLs locais |
| **`os_info`** | `3.14.0` | Detecção da distribuição Linux e versão do kernel |
| **`semver`** | `1.0.28` | Comparação de versões semânticas de plugins e perfis |
| **`path-slash`** | `0.2.1` | Conversão de caminhos de arquivos entre padrões Unix e Windows |
| **`psp`** | `0.1.0` | Biblioteca auxiliar de parsing |
| **`anyhow`** | `1.0.101` | Tratamento idiomático e propagação de erros |
| **`futures`** | `0.3.31` | Utilitários para programação assíncrona com Tokio |

---

## 4. Arquitetura de Comunicação e Portas

O OpenDeck opera como uma ponte central (*broker*) de mensageria entre o hardware físico, a interface gráfica do usuário e os plugins externos:

```
+-----------------------------------------------------------------------------------------+
|                                    UzlinuxStudioDeck                                    |
|                                                                                         |
|   +--------------------------+  +--------------------------+  +---------------------+   |
|   |   WebSocket Hub          |  |   WebSocket Hub          |  |   HTTP WebServer    |   |
|   |   (Plugins Elgato SDK)   |  |   (Property Inspectors)  |  |   (Assets Locais)   |   |
|   |   Porta: PORT_BASE       |  |   Porta: PORT_BASE + 1   |  |   Porta: PORT_BASE+2|   |
|   +-------------+------------+  +-------------+------------+  +----------+----------+   |
+-----------------|-----------------------------|--------------------------|--------------+
                  |                             |                          |
                  v                             v                          v
      +-----------------------+     +-----------------------+     +-------------------+
      | Processo do Plugin    |     | Webview do Inspector  |     | HTML / JS / CSS   |
      | (Binário compilado,   |     | (Renderiza UI custom  |     | dos plugins       |
      | Node.js ou via Wine)  |     | de cada ação)         |     | embutidos         |
      +-----------------------+     +-----------------------+     +-------------------+
```

### Inicialização de um Plugin
Quando um plugin é iniciado, o OpenDeck executa o binário registrado no `manifest.json` com os seguintes parâmetros:
```bash
<executavel> -port <PORT_BASE> -pluginUUID <UUID> -registerEvent <EVENT> -info <JSON_INFO>
```
* **`-port`:** Porta base onde o servidor WebSocket está escutando (ex: `57116`).
* **`-pluginUUID`:** Identificador único da extensão (ex: `com.amansprojects.starterpack.sdPlugin`).
* **`-registerEvent`:** Evento de registro que o plugin deve enviar logo após conectar (geralmente `registerPlugin`).
* **`-info`:** JSON serializado contendo metadados da aplicação, versão do OS, lista de dispositivos conectados e cores da interface.

---

## 5. Mapeamento de Comandos Tauri (IPC Frontend <-> Backend)

Os manipuladores registrados em `main.rs` via `tauri::generate_handler!` que controlam toda a aplicação são:

* **Gerenciamento de Instâncias de Ações:**
  * `frontend::instances::create_instance`: Cria uma nova instância de ação em uma tecla (`context: Keypad.R.C`).
  * `frontend::instances::move_instance`: Move/troca ações entre teclas.
  * `frontend::instances::remove_instance`: Remove a ação atribuída a uma tecla.
  * `frontend::instances::set_state`: Define o estado ativo de uma ação multi-estado.
  * `frontend::instances::set_child_delay`: Configura delays para sub-ações (Multi-Action).
  * `frontend::instances::update_image`: Atualiza o buffer de imagem renderizado na tecla.
  * `frontend::instances::trigger_virtual_press`: Simula o clique de uma tecla física a partir da interface gráfica.

* **Gerenciamento de Perfis:**
  * `frontend::profiles::get_profiles`: Lista todos os perfis cadastrados para o dispositivo.
  * `frontend::profiles::get_selected_profile`: Obtém o perfil atualmente em exibição no hardware.
  * `frontend::profiles::set_selected_profile`: Troca o perfil ativo no dispositivo.
  * `frontend::profiles::delete_profile`: Remove um perfil salvo.
  * `frontend::profiles::rename_profile`: Renomeia um perfil existente.

* **Property Inspectors:**
  * `frontend::property_inspector::make_info`: Gera o objeto JSON de inicialização para o inspector.
  * `frontend::property_inspector::switch_property_inspector`: Alterna o frame do inspector quando outra tecla é clicada.
  * `frontend::property_inspector::open_url`: Abre hiperlinks presentes nos inspectors no navegador do sistema.

* **Plugins:**
  * `frontend::plugins::list_plugins`: Lista todas as extensões instaladas e seus metadados.
  * `frontend::plugins::install_plugin`: Descompacta e instala um arquivo `.streamDeckPlugin`.
  * `frontend::plugins::remove_plugin`: Remove o diretório do plugin e encerra seus processos.
  * `frontend::plugins::reload_plugin`: Reinicia o processo do plugin e recarrega suas instâncias.
  * `frontend::plugins::show_settings_interface`: Exibe diálogo de configurações globais do plugin.

* **Configurações e Sistema:**
  * `frontend::get_devices`: Retorna os dispositivos Stream Deck conectados e ativos.
  * `frontend::get_port_base`: Informa as portas WebSocket alocadas.
  * `frontend::get_categories`: Retorna as categorias de ações disponíveis para a barra lateral.
  * `frontend::get_localisations`: Retorna as strings de internacionalização (i18n).
  * `frontend::get_applications` / `get_application_profiles` / `set_application_profiles`: Mapeamento de perfis automáticos por aplicação ativa.
  * `frontend::get_fonts`: Lista as fontes tipográficas instaladas para estilização de teclas.
  * `frontend::settings::get_settings` / `set_settings`: Leitura e escrita de `settings.json`.
  * `frontend::settings::backup_config_directory` / `restore_config_directory`: Criação e restauração de backups em arquivo `.zip`.
  * `frontend::restart`: Reinicia toda a aplicação de forma limpa.

---

## 6. O Caso Específico do Hardware Ulanzi D200 / D200H

Durante a inspeção das configurações do usuário em:
`~/.var/app/me.amankhanna.opendeck/config/opendeck/plugins/com.glmagalhaes.ulanzi.d200.sdPlugin/`

Foi identificado o binário não-stripped `rs-ulanzi-d200-linux`. A análise de seus símbolos exportados revelou:
1. **Comunicação por Ponte:** O driver do D200 se conecta ao OpenDeck como se fosse um plugin de Stream Deck através da crate `openaction`.
2. **Registro de Dispositivo:** Ele emite a mensagem customizada `device_registered` com o namespace `"e9"`, gerando o identificador de dispositivo `e9-02C53A015U3671380`.
3. **Controle de Hardware:**
   * `UlanziDevice::set_button_image`: Redimensiona e transmite imagens para os botões matriciais.
   * `UlanziDevice::set_small_window_data`: Atualiza os dados exibidos na tela informativa secundária LCD.
   * `UlanziDevice::set_brightness`: Regula a intensidade do backlight.

### Vantagem Estratégica do UzlinuxStudioDeck:
O OpenDeck original depende de um processo intermediário em background para falar com o Ulanzi D200. O **UzlinuxStudioDeck** implementará o driver do **Ulanzi D200 diretamente no núcleo em Rust** como uma implementação nativa de `StreamDeckDevice`, alcançando:
* Latência zero de renderização.
* Eliminação de deadlocks de IPC entre múltiplos daemons.
* Controle unificado de brilho, sleep de hardware e renderização de tela.

---

## 7. Esquema dos Arquivos de Configuração do Usuário

O UzlinuxStudioDeck garantirá **100% de retrocompatibilidade** com os arquivos já gerados pelo OpenDeck em `$XDG_CONFIG_HOME/opendeck/`:

* **`settings.json`:**
  ```json
  {
    "autolaunch": false,
    "background": true,
    "brightness": 21,
    "developer": false,
    "disableelgato": true,
    "language": "pt_BR",
    "rotation": 0,
    "separatewine": false,
    "sleep_timeout_minutes": 30,
    "sleep_when_computer_locked": true,
    "statistics": true,
    "updatecheck": true,
    "version": "2.14.0"
  }
  ```

* **`applications.json`:**
  ```json
  {
    "com.obsproject.Studio": {
      "e9-02C53A015U3671380": "Live"
    },
    "deezer-enhanced": {
      "e9-02C53A015U3671380": "Player"
    },
    "vlc": {
      "e9-02C53A015U3671380": "Player"
    }
  }
  ```

* **`profiles/<device_id>/<profile_name>.json`:**
  Armazena a matriz de teclas (`keys[]`), ações atribuídas, caminhos de ícones, configurações específicas do plugin e estilos tipográficos.

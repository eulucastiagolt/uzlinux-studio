# Uzlinux Studio 🎛️

> **Estação Nativa de Controle e Automação para Stream Controllers em Rust.**  
> O **Uzlinux Studio** é um software desktop moderno de alto desempenho, desenvolvido com foco primordial na integração com dispositivos da **Ulanzi** (como o **Ulanzi D200 / D200H** e outros modelos), mas com suporte aberto a outros stream controllers do mercado.  
> Oferece total interoperabilidade com o ecossistema de configurações, perfis, ações e plugins compatíveis com OpenDeck e Elgato Stream Deck, porém reconstruído com arquitetura nativa em Rust (Wayland / X11, PipeWire nativo, MPRIS nativo e Tauri v2).

---

## 🌟 Principais Recursos

- **Foco e Otimização para Hardware Ulanzi (e compatíveis):**
  - Integração profunda com o **Ulanzi Stream Controller D200 / D200H** com suas 14 telas LCD independentes e barra contínua de status.
  - Driver de hardware embutido no núcleo (*Zero-Setup*) com monitoramento de conexão e loop de heartbeat contínuo para evitar suspensões indesejadas de firmware.
  - Abertura para outros stream controllers e modo **Virtual Deck** para testes ou uso em tela sem hardware físico conectado.
- **Ações e Plugins Nativos Integrados ao Core:**
  - Áudio Linux nativo via **PipeWire** (controle fino de volume, alternância de saídas/entradas, mute) e controle de mídia nativo via **MPRIS** sem depender de plugins externos lentos.
  - Comandos de sistema em tempo real: execução Shell (`runcommand`), abertura de URLs (`openurl`), alternância de perfis (`switchprofile`), controle de brilho de hardware (`brightness`) e simulação de atalhos e teclado (`simulateinput`).
- **Compatibilidade com Ecossistema Aberto:**
  - Importação seletiva e compatibilidade transparente com arquivos de perfil, configurações e plugins do ecossistema OpenDeck / Stream Deck.
  - Suporte a Property Inspectors em HTML/JS embutidos via WebSocket e ponte `postMessage`.
- **Renderização e Tipografia Nítidas:**
  - Renderização nativa de rótulos e títulos com contorno de alto contraste, alinhamentos configuráveis e subpixel blending com `ab_glyph`.

---

## 📚 Documentação de Plugins

Criamos guias completos e detalhados na pasta [`docs/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/docs):

1. 🛠️ **[Guia Técnico de Desenvolvimento de Plugins](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/docs/GUIA_DESENVOLVIMENTO_PLUGINS.md)**
   - Especificação técnica profunda da arquitetura de processos isolados.
   - Detalhamento de argumentos CLI (`-port`, `-pluginUUID`, `-registerEvent`, `-info`).
   - Todos os esquemas de mensagens JSON para WebSockets (`setImage`, `setTitle`, `keyDown`, `willAppear`, etc.).
   - Criação de Property Inspectors (HTML/JS) e comunicação via `postMessage`.
   - Exemplos práticos completos em **Python**, **Rust** e **Shell Script**.

2. 🤖 **[Como Criar Plugins com Inteligência Artificial (Sem Saber Programar)](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/docs/GUIA_CRIACAO_COM_IA.md)**
   - Guia passo a passo voltado para usuários finais e criadores de conteúdo.
   - Roteiro mental em 4 perguntas para estruturar o seu botão.
   - **Prompt Mestre** pronto para copiar e colar no ChatGPT, Claude, Gemini ou Antigravity.
   - Exemplo prático de ponta a ponta (Plugin de Cotação de Criptomoedas).
   - Como instalar os arquivos no Linux e prompts de correção para erros comuns.

---

## 🚀 Compilação e Execução

### Pré-requisitos (Debian/Ubuntu/Fedora/Arch):
- Rust 1.80+ (`cargo`, `rustc`)
- Node.js 18+ e npm
- Bibliotecas de sistema: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libudev-dev`

### Executando em Modo Desenvolvimento:
```bash
./dev.sh
```

### Compilando o Binário Release:
```bash
cargo build --release
```
O executável final otimizado será gerado em:
```text
target/release/uzlinux-studio
```

---

## 📂 Estrutura do Workspace

- [`crates/uzlinux-app/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/crates/uzlinux-app): Aplicação desktop central e comandos Tauri v2.
- [`crates/uzlinux-core/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/crates/uzlinux-core): Gerenciamento de configurações, perfis e plugins instalados.
- [`crates/uzlinux-renderer/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/crates/uzlinux-renderer): Motor gráfico de composição de botões e tipografia com `ab_glyph`.
- [`crates/uzlinux-server/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/crates/uzlinux-server): Servidor WebSocket para plugins e HTTP Asset Server.
- [`crates/uzlinux-protocol/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/crates/uzlinux-protocol): Definições de eventos Inbound e Outbound do protocolo Stream Deck.
- [`crates/uzlinux-device-driver/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/crates/uzlinux-device-driver): Drivers de dispositivos e Virtual Deck.
- [`bundled_plugins/`](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/bundled_plugins): Plugins oficiais embutidos por padrão com o software.

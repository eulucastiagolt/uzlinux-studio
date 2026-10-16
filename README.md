# Uzlinux Studio

**Uzlinux Studio** é uma versão nativa e de código aberto do aplicativo **Ulanzi Studio / Ulanzi Deck** para **Linux**, desenvolvida em **Rust** utilizando **Slint UI**.

O projeto foi construído através de **engenharia reversa** do aplicativo original do Windows (`UlanziDeck.exe` e `Ulanzi_Studio.dll`), decifrando o protocolo USB HID de baixo nível utilizado pelos teclados macro da Ulanzi (ex: **D200H**, **D200**, **D100H Dial**).

---

## 🎯 Funcionalidades

- **Interface Moderna em Slint**: Grade visual 5x3 fiel ao hardware físico, com tema escuro e visualização instantânea de estado.
- **Feedback em Tempo Real**: Animação na interface ao pressionar qualquer botão físico no hardware.
- **Ações Configuráveis**:
  - **Aplicativos**: Execução de binários ou comandos do sistema (ex: `firefox`, `kitty`, `code`, `steam`, `discord`).
  - **Websites / URLs**: Abertura no navegador padrão via `open::that`.
  - **Atalhos / Teclas de Atalho**: Simulação de teclas via `xdotool` e `ydotool` (compatível com X11 e Wayland).
  - **Controle Multimídia**: Play/Pause, Próxima, Anterior, Aumentar Volume, Diminuir Volume, Mudo (via D-Bus MPRIS e teclas multimídia XF86).
  - **Digitação de Texto**: Macros de digitação de texto instantâneo.
- **Controle de Hardware**:
  - Slider de brilho de 0% a 100% transmitido em tempo real para a tela do Deck.
  - Temporizador de suspensão / sleep time.
  - Envio de imagens e ícones para as teclas do LCD.
- **Perfis Persistentes**: Configurações salvas automaticamente em formato JSON em `~/.config/uzlinux-studio/profile.json`.
- **Auto-detecção USB HID**: Conexão automática com `/dev/hidraw*` sem necessidade de configurações manuais.

---

## 🔬 Protocolo de Comunicação Revertido

### Camada de Transporte USB HID
- **Endpoints**: Interface 0 (`/dev/hidraw0`), blocos fixos de **1024 bytes**.
- **Estrutura do Pacote**:
  ```
  Offset   Tamanho  Campo          Descrição
  0..1     2 bytes  Magic          0x7C, 0x7C ("||")
  2        1 byte   Protocol ID    0x00 (Comando do Host) ou 0x01 (Evento do Dispositivo)
  3        1 byte   Command Code   Código do comando (ex: 0x0A brilho, 0x01 tecla)
  4..7     4 bytes  Data Length    Tamanho do payload N (u32 Little-Endian)
  8..8+N   N bytes  Payload        Dados brutos (ASCII, strings numéricas ou bytes de imagem)
  8+N..    2 bytes  Trailer        0x00, 0x00
  ...      Pad      Padding        Preenchido com zeros até o próximo múltiplo de 1024
  ```

### Códigos de Comando
- **0x01 (Keytap Event - Dispositivo -> Host)**:
  - `payload[0]`: Código da tecla (`key`)
  - `payload[1]`: Posição absoluta (`pos`): `col = pos % 5`, `row = pos / 5`
  - `payload[2]`: Tipo (0 ou 1: Teclado de botões, 2: Knob/Encoder)
  - `payload[3]`: Status (1: Pressionado, 0: Solto)
- **0x03 (Heartbeat)**: Relatório de status periódico enviado pelo hardware.
- **0x0A (Set Brightness)**: String decimal em ASCII (`"0"` a `"100"`).
- **0x1A (Set Sleep Time)**: Tempo em segundos em ASCII.
- **0x0F / 0x0E (Send Image Data)**: Envio de ícones/imagens em JPEG/PNG para o LCD.

---

## 🚀 Como Executar

### Pré-requisitos
- Rust toolchain (`rustc`, `cargo`)
- Acesso de leitura/escrita em `/dev/hidraw0` (já configurado por padrão para o usuário no sistema)

### Compilação e Execução
```bash
cd /home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudio
cargo run --release
```

Para executar a versão de desenvolvimento:
```bash
cargo run
```

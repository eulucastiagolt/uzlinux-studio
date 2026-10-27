/**
 * Uzlinux Studio - Internationalization (i18n) Engine
 * Inspired by Godot (tr) and WordPress (__) translation architectures.
 * 
 * Automatically detects the user's system language (via navigator.language / OS locale).
 * Defaults to English if not identified or unsupported.
 * Allows instant live-switching from the application settings without restart.
 */

(function () {
  'use strict';

  const translations = {
    en: {
      // Header & Navigation
      "app.title": "Uzlinux Studio",
      "app.core_version": "v0.1.0 (Rust Core)",
      "header.device": "📟 Device:",
      "header.profile": "Profile:",
      "header.new_profile": "New Profile",
      "header.rename_profile": "Rename Profile",
      "header.delete_profile": "Delete Profile",
      "header.auto_profile": "Auto Profiles by Application (Automatic Switch)",
      "header.plugins_btn": "🧩 Plugins",
      "header.settings_btn": "⚙️ Settings",

      // Workspace & Deck
      "palette.title": "Action Library",
      "palette.search_placeholder": "Search actions...",
      "palette.no_actions": "No actions found",
      "palette.loading": "Loading actions...",
      "deck.badge_d200": "3 × 5 Grid (14 Keys - Ulanzi D200)",
      "deck.footer_hint": "💡 Tip: Drag an action to the grid, or drag keys between each other to move or swap positions.",

      // Inspector
      "inspector.title": "Property Inspector",
      "inspector.no_key_selected": "No key selected",
      "inspector.empty_hint": "Select a key on the grid to edit its title, image, or configure plugin options.",
      "inspector.action_name": "Action Name",
      "inspector.plugin_author": "Plugin Provider",
      "inspector.key_title": "Display Title",
      "inspector.title_placeholder": "Enter text...",
      "inspector.text_color": "Text Color",
      "inspector.font_size": "Font Size",
      "inspector.action_icon": "🖼️ Action Icon",
      "inspector.key_state": "Key State:",
      "inspector.choose_image": "📁 Choose Image",
      "inspector.pick_system": "🔍 System",
      "inspector.reset_icon": "↺ Restore Default Icon",
      "inspector.icon_scale": "Scale:",
      "inspector.show_title": "Show Title",
      "inspector.test_click": "⚡ Test Click",
      "inspector.clear_key": "🗑️ Clear Key",
      "inspector.action_config": "Action Settings:",
      "inspector.plugin_panel": "Plugin Panel (Property Inspector):",

      // Native Actions Inspector
      "native.app_to_execute": "Application to Execute:",
      "native.no_app_selected": "No application selected",
      "native.click_to_choose_app": "Click the button below to choose an installed app",
      "native.select_app_btn": "Select Application...",
      "native.extra_args": "Additional Arguments (optional):",
      "native.url_site": "Web Address / URL:",
      "native.shell_command": "Shell Command (Bash):",
      "native.target_profile": "Target Profile:",
      "native.brightness_action": "Brightness Action:",
      "native.brightness_set": "Set Exact Brightness (%)",
      "native.brightness_increase": "Increase Brightness (+)",
      "native.brightness_decrease": "Decrease Brightness (-)",
      "native.brightness_value": "Value (%s%):",
      "native.input_key_down": "On Key Down:",
      "native.input_key_up": "On Key Up (optional):",
      "native.input_hint": "💡 Examples: [t(\"hello!\")], [k(ctrl,p)], [k(f5)], [b(l)], [m(10,10,r)]",

      // PipeWire Audio Inspector
      "pw.master_title": "🔊 Master Volume (System)",
      "pw.action_button": "Button Action:",
      "pw.volume_up": "🔊 Increase Volume (+)",
      "pw.volume_down": "🔉 Decrease Volume (-)",
      "pw.volume_mute": "🔇 Toggle Mute (Mute/Unmute)",
      "pw.volume_step": "Volume Step (%s%):",
      "pw.mic_title": "🎤 Default Microphone Volume",
      "pw.mic_up": "🎤 Increase Microphone (+)",
      "pw.mic_down": "🔉 Decrease Microphone (-)",
      "pw.mic_mute": "🔇 Mute / Unmute Microphone",
      "pw.output_title": "🎧 Specific Output Volume",
      "pw.output_device": "Output Device:",
      "pw.input_title": "🎙️ Specific Input Volume",
      "pw.input_device": "Input Device (Microphone):",
      "pw.app_title": "🎵 Application Volume",
      "pw.app_with_audio": "Application with Audio:",
      "pw.switch_output_title": "🎧 Switch Output Device",
      "pw.target_output": "Target Output Device:",
      "pw.cycle_outputs": "🔄 Cycle between all output devices",
      "pw.switch_output_hint": "💡 Select a specific device to activate directly with this key, or select \"Cycle between all\" to cycle outputs on each click.",
      "pw.switch_input_title": "🎙️ Switch Input Device",
      "pw.target_input": "Target Microphone / Input:",
      "pw.cycle_inputs": "🔄 Cycle between all microphones",
      "pw.switch_input_hint": "💡 Select a specific microphone to activate directly, or use \"Cycle between all\" to switch between connected microphones.",
      "pw.ptt_title": "📻 Push to Talk (Hold to Speak)",
      "pw.target_mic": "Target Microphone:",
      "pw.default_mic": "System Default Microphone",
      "pw.ptt_hint": "The microphone is kept muted and will be unmuted only while you hold this key pressed on the deck. Upon release, it mutes automatically.",
      "pw.loading_devices": "Loading audio devices...",

      // MPRIS Media Inspector
      "mpris.media_command": "Media Command:",
      "mpris.hint": "💡 Natively compatible with Spotify, YouTube, VLC and any Linux MPRIS player.",
      "mpris.cmd_play_pause": "⏯️ Play / Pause",
      "mpris.cmd_next": "⏭️ Next Track",
      "mpris.cmd_prev": "⏮️ Previous Track",
      "mpris.cmd_stop": "⏹️ Stop Playback",
      "mpris.cmd_volup": "🔊 Player Volume (+5%)",
      "mpris.cmd_voldown": "🔉 Player Volume (-5%)",
      "mpris.cmd_seekfwd": "⏩ Seek Forward 10s",
      "mpris.cmd_seekbwd": "⏪ Seek Backward 10s",
      "mpris.cmd_shuffle": "🔀 Toggle Shuffle",
      "mpris.cmd_repeat": "🔁 Toggle Repeat",

      // Settings Modal
      "settings.modal_title": "Uzlinux Studio Settings",
      "settings.tab_general": "⚙️ General & Hardware",
      "settings.tab_obs": "🎥 OBS Studio",
      "settings.tab_plugins": "🧩 Manage Plugins",
      "settings.tab_auto_profiles": "🔀 Auto Profiles",
      "settings.services_status": "Service Status:",
      "settings.language_label": "Interface Language:",
      "settings.lang_auto": "🌐 Automatic (Detect from System)",
      "settings.lang_en": "🇺🇸 English",
      "settings.lang_pt_br": "🇧🇷 Portuguese (Português do Brasil)",
      "settings.brightness": "Backlight Brightness (D200):",
      "settings.sleep_timeout": "Inactivity Sleep Timeout (minutes):",
      "settings.background": "Keep running in background (System Tray)",
      "settings.sleep_lock": "Turn off deck screens when computer is locked",
      "settings.opendeck_import": "OpenDeck Import:",
      "settings.opendeck_import_desc": "Copy profiles, settings and plugins from OpenDeck into Uzlinux Studio's isolated directory without altering the originals.",
      "settings.btn_import_opendeck": "📥 Import from OpenDeck",
      "settings.save_btn": "Save All Settings",

      // Settings OBS Tab
      "obs.info": "📡 Connect Uzlinux Studio to OBS Studio via OBS WebSocket v5.",
      "obs.hint": "In OBS Studio, go to: Tools > WebSocket Server Settings and make sure it is enabled.",
      "obs.host": "Host / IP Address:",
      "obs.port": "WebSocket Port:",
      "obs.password": "WebSocket Password:",
      "obs.pwd_placeholder": "Password configured in OBS (if any)",
      "obs.dual_pc": "Configure 2nd OBS Instance (Dual PC / Backup)",
      "obs.host2": "Host / IP (OBS 2):",
      "obs.port2": "Port (OBS 2):",
      "obs.pwd2": "Password (OBS 2):",
      "obs.test_btn": "🔌 Test OBS Connection",
      "obs.connecting": "⏳ Connecting...",

      // Settings Plugins Tab
      "plugins.tab_info": "🧩 Manage plugins installed in Uzlinux Studio or import from OpenDeck.",
      "plugins.tab_hint": "Plugins directory is isolated at ~/.config/uzlinux-studio/plugins/.",
      "plugins.open_full_manager": "🧩 Open Full Manager",
      "plugins.open_folder": "📂 Open Folder in File Manager",
      "plugins.loading": "Loading plugins...",
      "plugins.built_in": "Built-in / Core",
      "plugins.status_active": "Active",
      "plugins.status_disabled": "Disabled",
      "plugins.btn_disable": "Disable",
      "plugins.btn_enable": "Enable",
      "plugins.btn_delete": "Uninstall",

      // Settings Auto Profiles Tab
      "autoprof.title": "🔀 Automatic Profile Switching by Application",
      "autoprof.desc": "The deck automatically switches profiles when you focus a linked application (e.g. switch to Live profile when focusing OBS Studio, or Player when opening VLC/Spotify).",
      "autoprof.add_title": "➕ Add New Binding:",
      "autoprof.app_label": "System Application / Window Class:",
      "autoprof.app_placeholder": "e.g. com.obsproject.Studio or vlc",
      "autoprof.pick_app_btn": "🔍 Browse",
      "autoprof.target_profile": "Target Profile:",
      "autoprof.bind_btn": "💾 Bind",
      "autoprof.configured_list": "Configured Bindings on this Device:",
      "autoprof.no_bindings": "No automatic profile bindings configured yet.",

      // App Selector Modal
      "appsel.title": "Select System Application",
      "appsel.search_placeholder": "Search application by name, command, or description...",
      "appsel.cancel": "Cancel",
      "appsel.count": "%s applications found",

      // OpenDeck Import Modal
      "od.title": "OpenDeck Import",
      "od.desc": "Choose what you want to import from OpenDeck to Uzlinux Studio. Original OpenDeck files remain untouched.",
      "od.detecting": "Detecting installation...",
      "od.analyzing": "⏳ Analyzing OpenDeck data...",
      "od.not_found": "⚠️ No OpenDeck installation was found at ~/.var/app/me.amankhanna.opendeck or ~/.config/opendeck.",
      "od.profiles": "📁 Device Profiles (%s)",
      "od.plugins": "🧩 Plugins (%s)",
      "od.other_settings": "⚙️ Settings & Resources:",
      "od.opt_auto_profiles": "🔀 Automatic Profile Switching (applications.json)",
      "od.opt_images": "🖼️ Custom Images & Icons (images/ directory)",
      "od.opt_settings": "🛠️ Plugin Settings (settings/ directory)",
      "od.select_all": "Select all",
      "od.unselect_all": "Unselect all",
      "od.close": "Close",
      "od.import_selected": "📥 Import Selected",
      "od.import_all": "⚡ Import Everything",
      "od.importing_all": "⚡ Importing all data from OpenDeck...",
      "od.importing_selected": "⏳ Importing selected items...",
      "od.success": "✅ Import completed successfully!",

      // Categories and Native Actions
      "cat.system": "⚡ System & Shortcuts (Pre-installed)",
      "cat.mpris": "🎵 Media Controller (MPRIS / Pre-installed)",
      "cat.pipewire": "🔊 Linux Audio (PipeWire / Pre-installed)",
      "cat.ulanzi": "🎛️ Ulanzi D200 (Integrated Hardware)",

      // Dialogs & Confirmations
      "dialog.prompt_profile_name": "Enter the name for the new profile:",
      "dialog.prompt_rename_profile": "Enter the new profile name:",
      "dialog.confirm_delete_profile": "Are you sure you want to delete profile \"%s\"? This cannot be undone.",
      "dialog.settings_saved": "Settings saved successfully!",
      "dialog.profile_created": "Profile \"%s\" created successfully!",
      "dialog.cannot_disable_driver": "Hardware driver cannot be disabled.",
      "dialog.import_success": "Import completed successfully!"
    },

    pt_BR: {
      // Header & Navigation
      "app.title": "Uzlinux Studio",
      "app.core_version": "v0.1.0 (Rust Core)",
      "header.device": "📟 Disp:",
      "header.profile": "Perfil:",
      "header.new_profile": "Novo Perfil",
      "header.rename_profile": "Renomear Perfil",
      "header.delete_profile": "Excluir Perfil",
      "header.auto_profile": "Perfis Automáticos por Aplicativo (Troca Automática)",
      "header.plugins_btn": "🧩 Plugins",
      "header.settings_btn": "⚙️ Configurações",

      // Workspace & Deck
      "palette.title": "Biblioteca de Ações",
      "palette.search_placeholder": "Buscar ações...",
      "palette.no_actions": "Nenhuma ação encontrada",
      "palette.loading": "Carregando ações...",
      "deck.badge_d200": "Grade 3 × 5 (14 Teclas - Ulanzi D200)",
      "deck.footer_hint": "💡 Dica: Arraste uma ação para o grid, ou arraste teclas entre si para mover ou trocar posições.",

      // Inspector
      "inspector.title": "Inspetor de Propriedades",
      "inspector.no_key_selected": "Nenhuma tecla selecionada",
      "inspector.empty_hint": "Selecione uma tecla na grade ao lado para editar o título, imagem ou abrir as opções do plugin.",
      "inspector.action_name": "Nome da Ação",
      "inspector.plugin_author": "Plugin Responsável",
      "inspector.key_title": "Título Exibido",
      "inspector.title_placeholder": "Digite o texto...",
      "inspector.text_color": "Cor do Texto",
      "inspector.font_size": "Tamanho da Fonte",
      "inspector.action_icon": "🖼️ Ícone da Ação",
      "inspector.key_state": "Estado da Tecla:",
      "inspector.choose_image": "📁 Escolher Imagem",
      "inspector.pick_system": "🔍 Sistema",
      "inspector.reset_icon": "↺ Restaurar Ícone Padrão",
      "inspector.icon_scale": "Tamanho:",
      "inspector.show_title": "Exibir Título",
      "inspector.test_click": "⚡ Testar Clique",
      "inspector.clear_key": "🗑️ Limpar Tecla",
      "inspector.action_config": "Configurações da Ação:",
      "inspector.plugin_panel": "Painel do Plugin (Property Inspector):",

      // Native Actions Inspector
      "native.app_to_execute": "Aplicativo a Executar:",
      "native.no_app_selected": "Nenhum aplicativo selecionado",
      "native.click_to_choose_app": "Clique no botão abaixo para escolher",
      "native.select_app_btn": "Selecionar Aplicativo...",
      "native.extra_args": "Argumentos Adicionais (opcional):",
      "native.url_site": "Endereço URL / Site:",
      "native.shell_command": "Comando Shell (Bash):",
      "native.target_profile": "Perfil de Destino:",
      "native.brightness_action": "Ação de Brilho:",
      "native.brightness_set": "Definir Brilho Exato (%)",
      "native.brightness_increase": "Aumentar Brilho (+)",
      "native.brightness_decrease": "Diminuir Brilho (-)",
      "native.brightness_value": "Valor (%s%):",
      "native.input_key_down": "Ao Pressionar Tecla (Key Down):",
      "native.input_key_up": "Ao Soltar Tecla (Key Up - opcional):",
      "native.input_hint": "💡 Exemplos: [t(\"olá!\")], [k(ctrl,p)], [k(f5)], [b(l)], [m(10,10,r)]",

      // PipeWire Audio Inspector
      "pw.master_title": "🔊 Volume Master (Sistema)",
      "pw.action_button": "Ação do Botão:",
      "pw.volume_up": "🔊 Aumentar Volume (+)",
      "pw.volume_down": "🔉 Diminuir Volume (-)",
      "pw.volume_mute": "🔇 Alternar Mudo (Mute/Unmute)",
      "pw.volume_step": "Passo de Volume (%s%):",
      "pw.mic_title": "🎤 Volume do Microfone Padrão",
      "pw.mic_up": "🎤 Aumentar Microfone (+)",
      "pw.mic_down": "🔉 Diminuir Microfone (-)",
      "pw.mic_mute": "🔇 Mutar / Desmutar Microfone",
      "pw.output_title": "🎧 Volume de Saída Específica",
      "pw.output_device": "Dispositivo de Saída:",
      "pw.input_title": "🎙️ Volume de Entrada Específica",
      "pw.input_device": "Dispositivo de Entrada (Microfone):",
      "pw.app_title": "🎵 Volume de Aplicativo",
      "pw.app_with_audio": "Aplicativo com Áudio:",
      "pw.switch_output_title": "🎧 Alternar Dispositivo de Saída",
      "pw.target_output": "Dispositivo de Saída Alvo:",
      "pw.cycle_outputs": "🔄 Alternar ciclicamente entre todos os dispositivos",
      "pw.switch_output_hint": "💡 Selecione um dispositivo específico para ativá-lo diretamente com esta tecla, ou selecione \"Alternar ciclicamente\" para alternar entre todas as saídas a cada clique.",
      "pw.switch_input_title": "🎙️ Alternar Dispositivo de Entrada",
      "pw.target_input": "Microfone / Entrada Alvo:",
      "pw.cycle_inputs": "🔄 Alternar ciclicamente entre todos os microfones",
      "pw.switch_input_hint": "💡 Selecione um microfone específico para ativá-lo diretamente, ou use \"Alternar ciclicamente\" para trocar entre microfones conectados.",
      "pw.ptt_title": "📻 Push to Talk (Pressione para Falar)",
      "pw.target_mic": "Microfone Alvo:",
      "pw.default_mic": "Microfone Padrão do Sistema",
      "pw.ptt_hint": "O microfone é mantido mudo e será desmutado apenas enquanto você mantiver esta tecla pressionada no deck. Ao soltar, volta automaticamente ao mudo.",
      "pw.loading_devices": "Carregando dispositivos...",

      // MPRIS Media Inspector
      "mpris.media_command": "Comando de Mídia:",
      "mpris.hint": "💡 Compatível nativamente com Spotify, YouTube, VLC e qualquer reprodutor MPRIS do Linux.",
      "mpris.cmd_play_pause": "⏯️ Play / Pause",
      "mpris.cmd_next": "⏭️ Próxima Faixa",
      "mpris.cmd_prev": "⏮️ Faixa Anterior",
      "mpris.cmd_stop": "⏹️ Parar Reprodução",
      "mpris.cmd_volup": "🔊 Volume do Player (+5%)",
      "mpris.cmd_voldown": "🔉 Volume do Player (-5%)",
      "mpris.cmd_seekfwd": "⏩ Avançar 10 segundos",
      "mpris.cmd_seekbwd": "⏪ Retroceder 10 segundos",
      "mpris.cmd_shuffle": "🔀 Alternar Aleatório",
      "mpris.cmd_repeat": "🔁 Alternar Repetição",

      // Settings Modal
      "settings.modal_title": "Configurações do Uzlinux Studio",
      "settings.tab_general": "⚙️ Geral & Hardware",
      "settings.tab_obs": "🎥 OBS Studio",
      "settings.tab_plugins": "🧩 Gerenciar Plugins",
      "settings.tab_auto_profiles": "🔀 Perfis Automáticos",
      "settings.services_status": "Status dos Serviços:",
      "settings.language_label": "Idioma da Interface:",
      "settings.lang_auto": "🌐 Automático (Detectar do Sistema)",
      "settings.lang_en": "🇺🇸 English",
      "settings.lang_pt_br": "🇧🇷 Português (Brasil)",
      "settings.brightness": "Brilho do Backlight (D200):",
      "settings.sleep_timeout": "Tempo de Sleep por Inatividade (minutos):",
      "settings.background": "Continuar executando em segundo plano (Bandeja / Tray)",
      "settings.sleep_lock": "Desligar tela quando o computador estiver bloqueado",
      "settings.opendeck_import": "Importação do OpenDeck:",
      "settings.opendeck_import_desc": "Copie perfis, configurações e plugins do OpenDeck para a pasta isolada do Uzlinux Studio sem alterar os originais.",
      "settings.btn_import_opendeck": "📥 Importar do OpenDeck",
      "settings.save_btn": "Salvar Todas as Configurações",

      // Settings OBS Tab
      "obs.info": "📡 Conecte o Uzlinux Studio ao OBS Studio através do OBS WebSocket v5.",
      "obs.hint": "No OBS Studio, acesse: Ferramentas > Configurações do servidor WebSocket e certifique-se de que está ativado.",
      "obs.host": "Host / Endereço IP:",
      "obs.port": "Porta WebSocket:",
      "obs.password": "Senha do WebSocket:",
      "obs.pwd_placeholder": "Senha configurada no OBS (se houver)",
      "obs.dual_pc": "Configurar 2ª Instância do OBS (Dual PC / Backup)",
      "obs.host2": "Host / IP (OBS 2):",
      "obs.port2": "Porta (OBS 2):",
      "obs.pwd2": "Senha do OBS 2:",
      "obs.test_btn": "🔌 Testar Conexão com OBS",
      "obs.connecting": "⏳ Conectando...",

      // Settings Plugins Tab
      "plugins.tab_info": "🧩 Gerencie os plugins instalados no Uzlinux Studio ou importe do OpenDeck.",
      "plugins.tab_hint": "A pasta de plugins fica isolada em ~/.config/uzlinux-studio/plugins/.",
      "plugins.open_full_manager": "🧩 Abrir Gerenciador Completo",
      "plugins.open_folder": "📂 Abrir Pasta no Linux",
      "plugins.loading": "Carregando plugins...",
      "plugins.built_in": "Pré-instalado / Integrado",
      "plugins.status_active": "Ativo",
      "plugins.status_disabled": "Desativado",
      "plugins.btn_disable": "Desativar",
      "plugins.btn_enable": "Ativar",
      "plugins.btn_delete": "Desinstalar",

      // Settings Auto Profiles Tab
      "autoprof.title": "🔀 Troca Automática de Perfil por Aplicativo",
      "autoprof.desc": "O deck alterna automaticamente de perfil quando você abre ou foca em um aplicativo vinculado (ex: mudar para o perfil Live ao focar no OBS Studio, ou Player ao abrir o VLC/Spotify).",
      "autoprof.add_title": "➕ Adicionar Novo Vínculo:",
      "autoprof.app_label": "Aplicativo do Sistema / Identificador:",
      "autoprof.app_placeholder": "ex: com.obsproject.Studio ou vlc",
      "autoprof.pick_app_btn": "🔍 Escolher",
      "autoprof.target_profile": "Perfil de Destino:",
      "autoprof.bind_btn": "💾 Vincular",
      "autoprof.configured_list": "Vínculos Configurados neste Dispositivo:",
      "autoprof.no_bindings": "Nenhum vínculo automático configurado ainda.",

      // App Selector Modal
      "appsel.title": "Selecionar Aplicativo do Sistema",
      "appsel.search_placeholder": "Buscar aplicativo por nome, comando ou descrição...",
      "appsel.cancel": "Cancelar",
      "appsel.count": "%s aplicativos encontrados",

      // OpenDeck Import Modal
      "od.title": "Importação do OpenDeck",
      "od.desc": "Escolha o que deseja importar do OpenDeck para o Uzlinux Studio. Os arquivos originais do OpenDeck permanecem intactos.",
      "od.detecting": "Detectando instalação...",
      "od.analyzing": "⏳ Analisando dados do OpenDeck...",
      "od.not_found": "⚠️ Nenhuma instalação do OpenDeck foi localizada em ~/.var/app/me.amankhanna.opendeck ou ~/.config/opendeck.",
      "od.profiles": "📁 Perfis de Dispositivos (%s)",
      "od.plugins": "🧩 Plugins (%s)",
      "od.other_settings": "⚙️ Configurações & Recursos:",
      "od.opt_auto_profiles": "🔀 Troca Automática de Perfis (applications.json)",
      "od.opt_images": "🖼️ Imagens e Ícones Personalizados (pasta images/)",
      "od.opt_settings": "🛠️ Configurações dos Plugins (pasta settings/)",
      "od.select_all": "Marcar todos",
      "od.unselect_all": "Desmarcar",
      "od.close": "Fechar",
      "od.import_selected": "📥 Importar Selecionados",
      "od.import_all": "⚡ Importar Tudo",
      "od.importing_all": "⚡ Importando todos os dados do OpenDeck...",
      "od.importing_selected": "⏳ Importando itens selecionados...",
      "od.success": "✅ Importação concluída com sucesso!",

      // Categories and Native Actions
      "cat.system": "⚡ Sistema & Atalhos (Pré-instalado)",
      "cat.mpris": "🎵 Controle de Mídia (MPRIS / Pré-instalado)",
      "cat.pipewire": "🔊 Áudio Linux (PipeWire / Pré-instalado)",
      "cat.ulanzi": "🎛️ Ulanzi D200 (Hardware Integrado)",

      // Dialogs & Confirmations
      "dialog.prompt_profile_name": "Digite o nome para o novo perfil:",
      "dialog.prompt_rename_profile": "Digite o novo nome para o perfil:",
      "dialog.confirm_delete_profile": "Tem certeza que deseja excluir o perfil \"%s\"? Esta ação não pode ser desfeita.",
      "dialog.settings_saved": "Configurações salvas com sucesso!",
      "dialog.profile_created": "Perfil \"%s\" criado com sucesso!",
      "dialog.cannot_disable_driver": "O driver de hardware não pode ser desativado.",
      "dialog.import_success": "Importação concluída com sucesso!"
    }
  };

  class I18nEngine {
    constructor() {
      this.currentLang = 'en';
      this.currentLocale = 'en';
      this.translations = translations;
    }

    /**
     * Inspects the browser and system environment locale.
     * If starts with 'pt', returns 'pt_BR', otherwise defaults to 'en'.
     */
    detectSystemLanguage() {
      const candidates = [];
      if (Array.isArray(navigator.languages)) {
        candidates.push(...navigator.languages);
      }
      if (navigator.language) {
        candidates.push(navigator.language);
      }

      for (const cand of candidates) {
        if (!cand) continue;
        const normalized = cand.toLowerCase().trim();
        if (normalized.startsWith('pt')) {
          return 'pt_BR';
        }
        if (normalized.startsWith('en')) {
          return 'en';
        }
      }

      // Default fallback
      return 'en';
    }

    /**
     * Resolves user preference ('auto', 'pt_BR', 'en') to a concrete locale code.
     */
    resolveLanguage(settingLang) {
      if (!settingLang || settingLang === 'auto') {
        return this.detectSystemLanguage();
      }
      const norm = settingLang.toLowerCase().trim();
      if (norm.startsWith('pt')) {
        return 'pt_BR';
      }
      return 'en';
    }

    /**
     * Translates a string by key, replacing parameters or positional arguments.
     * Usage: i18n.t("header.device") or tr("dialog.confirm_delete_profile", [name])
     */
    t(key, params) {
      if (!key) return '';
      const dict = this.translations[this.currentLocale] || this.translations.en;
      let text = dict[key];

      // Fallback to English if not found in current locale
      if (text === undefined && this.currentLocale !== 'en') {
        text = this.translations.en[key];
      }

      // If still not found, return key
      if (text === undefined) {
        return key;
      }

      // Parameter replacement: %s, %d or {named}
      if (params !== undefined && params !== null) {
        if (Array.isArray(params)) {
          let i = 0;
          text = text.replace(/%s|%d/g, () => (params[i] !== undefined ? params[i++] : ''));
        } else if (typeof params === 'object') {
          for (const [k, v] of Object.entries(params)) {
            text = text.replace(new RegExp(`\\{${k}\\}`, 'g'), v);
          }
        } else {
          text = text.replace(/%s|%d/g, params);
        }
      }

      return text;
    }

    /**
     * Sets active language and updates all tagged DOM elements.
     */
    setLanguage(langPreference) {
      const concreteLocale = this.resolveLanguage(langPreference);
      this.currentLang = langPreference || 'auto';
      this.currentLocale = concreteLocale;

      document.documentElement.lang = concreteLocale === 'pt_BR' ? 'pt-BR' : 'en';

      this.updateDOM();

      // Dispatch event for dynamic re-renders
      window.dispatchEvent(new CustomEvent('uzlinux:language-changed', {
        detail: {
          preference: this.currentLang,
          locale: this.currentLocale
        }
      }));
    }

    /**
     * Updates all DOM nodes having data-i18n attributes.
     */
    updateDOM(root = document) {
      // Text content
      root.querySelectorAll('[data-i18n]').forEach((el) => {
        const key = el.getAttribute('data-i18n');
        if (key) {
          el.textContent = this.t(key);
        }
      });

      // Title attribute
      root.querySelectorAll('[data-i18n-title]').forEach((el) => {
        const key = el.getAttribute('data-i18n-title');
        if (key) {
          el.title = this.t(key);
        }
      });

      // Placeholder attribute
      root.querySelectorAll('[data-i18n-placeholder]').forEach((el) => {
        const key = el.getAttribute('data-i18n-placeholder');
        if (key) {
          el.placeholder = this.t(key);
        }
      });

      // ARIA label attribute
      root.querySelectorAll('[data-i18n-aria-label]').forEach((el) => {
        const key = el.getAttribute('data-i18n-aria-label');
        if (key) {
          el.setAttribute('aria-label', this.t(key));
        }
      });
    }

    getLocale() {
      return this.currentLocale;
    }

    getPreference() {
      return this.currentLang;
    }
  }

  const i18n = new I18nEngine();

  // Expose globally Godot-style (tr) and WordPress-style (__)
  window.i18n = i18n;
  window.tr = i18n.t.bind(i18n);
  window.__ = i18n.t.bind(i18n);

})();

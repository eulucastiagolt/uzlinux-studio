// Uzlinux Studio UI Controller
function invoke(cmd, args) {
  const invoker = window.__TAURI__?.core?.invoke || window.__TAURI__?.invoke;
  if (invoker) {
    return invoker(cmd, args);
  }
  console.warn(`[Mock Invoke - Tauri not detected yet] ${cmd}`, args);
  return Promise.resolve(null);
}

function listen(event, callback) {
  const listener = window.__TAURI__?.event?.listen;
  if (listener) {
    return listener(event, callback);
  }
  return Promise.resolve(() => {});
}

let currentDeviceId = null;
let currentProfileName = null;
let currentProfileData = null;
let selectedKeyIndex = null;
let currentDraggedAction = null;
let currentDraggedKeyIndex = null;
let categories = [];
let serverPorts = { plugin_ws: 57116, pi_ws: 57117, http_assets: 57118 };
let pipewireAudioCache = null;

async function fetchPipewireDevices() {
  try {
    pipewireAudioCache = await invoke('get_pipewire_audio_devices');
    return pipewireAudioCache;
  } catch (err) {
    console.error('Failed to get pipewire devices:', err);
    return { sinks: [], sources: [], apps: [] };
  }
}

document.addEventListener('DOMContentLoaded', async () => {
  // Wait briefly if window.__TAURI__ is still being injected
  let retries = 10;
  while (!window.__TAURI__ && retries > 0) {
    await new Promise((r) => setTimeout(r, 50));
    retries--;
  }

  // Real-time synchronization of key settings modified by plugins or Property Inspectors
  listen('key_settings_changed', (evt) => {
    if (!evt || !evt.payload) return;
    const { deviceId, keyIndex, settings } = evt.payload;
    if (currentDeviceId === deviceId && currentProfileData?.keys?.[keyIndex]) {
      currentProfileData.keys[keyIndex].settings = settings;
      console.log(`[UI] Settings synced for key ${keyIndex}:`, settings);
    }
  });

  // Real-time synchronization when a profile is auto-switched by WindowWatcher
  listen('profile_changed', async (evt) => {
    if (!evt || !evt.payload) return;
    const { deviceId, profileName } = evt.payload;
    if (currentDeviceId === deviceId && profileName && profileName !== currentProfileName) {
      console.log(`[Auto-Profile] Auto-switching UI to profile "${profileName}"`);
      currentProfileName = profileName;
      const profileSelect = document.getElementById('profile-select');
      if (profileSelect) profileSelect.value = profileName;
      await loadProfileData(deviceId, profileName);
    }
  });

  // Handle openUrl and settings persistence messages from Property Inspector iframes
  window.addEventListener('message', async (event) => {
    const data = event.data;
    if (!data) return;
    if (data.event === 'openUrl' && data.payload) {
      await invoke('open_url', { url: data.payload }).catch(() => {});
    } else if (data.event === 'setSettings' || data.event === 'SetSettings' || data.event === 'set_settings') {
      const payload = data.payload || data.settings || {};
      let keyIdx = selectedKeyIndex;
      if (data.context && currentProfileData?.keys) {
        const foundIdx = currentProfileData.keys.findIndex(k => k && k.context === data.context);
        if (foundIdx !== -1) keyIdx = foundIdx;
      }
      if (keyIdx !== null && currentProfileData?.keys?.[keyIdx]) {
        console.log(`[UI postMessage] Received settings for key ${keyIdx}:`, payload);
        currentProfileData.keys[keyIdx].settings = payload;
        if (currentDeviceId) {
          await invoke('save_key_settings', {
            deviceId: currentDeviceId,
            keyIndex: keyIdx,
            settings: payload
          }).catch((err) => console.error('[UI] Failed to save key settings via Tauri invoke:', err));
        }
      }
    }
  });

  // Initialize internationalization (i18n)
  try {
    const s = await invoke('get_settings');
    const lang = s?.language || 'auto';
    if (window.i18n) {
      window.i18n.setLanguage(lang);
    }
  } catch (_) {
    if (window.i18n) window.i18n.setLanguage('auto');
  }

  window.addEventListener('uzlinux:language-changed', async () => {
    await loadCategories();
    if (selectedKeyIndex !== null) {
      renderInspector(selectedKeyIndex);
    }
  });

  await initPorts();
  await loadDevices();
  await loadCategories();
  initHtmlPlugins();
  setupEventListeners();
});

async function initHtmlPlugins() {
  const container = document.getElementById('html-plugins-container');
  if (!container) return;
  container.innerHTML = '';

  const activeLang = window.i18n ? window.i18n.getLocale() : 'en';

  try {
    const plugins = await invoke('get_plugins_list');
    plugins.forEach((plugin) => {
      if (plugin.pluginType === 'html' && plugin.executablePath) {
        const iframe = document.createElement('iframe');
        const infoPayload = encodeURIComponent(JSON.stringify({
          application: { font: "Liberation Sans", language: activeLang, platform: "linux", version: "2.14.0" },
          plugin: { uuid: plugin.uuid, version: plugin.version }
        }));
        const srcPath = `plugins/${plugin.uuid}/${plugin.executablePath}`;
        iframe.src = `http://127.0.0.1:${serverPorts.http_assets}/${srcPath}?port=${serverPorts.plugin_ws}&pluginUUID=${plugin.uuid}&registerEvent=registerPlugin&info=${infoPayload}`;
        container.appendChild(iframe);
        console.log(`Mounted HTML background plugin for ${plugin.uuid}: ${iframe.src}`);
      }
    });
  } catch (err) {
    console.warn('Failed to load HTML plugins dynamically, falling back to Multi OBS:', err);
    const multiObsIframe = document.createElement('iframe');
    const infoPayload = encodeURIComponent(JSON.stringify({
      application: { font: "Liberation Sans", language: activeLang, platform: "linux", version: "2.14.0" },
      plugin: { uuid: "dev.theca11.multiobs.sdPlugin", version: "0.8.2" }
    }));
    multiObsIframe.src = `http://127.0.0.1:${serverPorts.http_assets}/plugins/dev.theca11.multiobs.sdPlugin/plugin/app.html?port=${serverPorts.plugin_ws}&pluginUUID=dev.theca11.multiobs.sdPlugin&registerEvent=registerPlugin&info=${infoPayload}`;
    container.appendChild(multiObsIframe);
  }
}

async function initPorts() {
  try {
    const ports = await invoke('get_port_base');
    if (ports) {
      serverPorts = ports;
      console.log('Server ports:', serverPorts);
    }
  } catch (e) {
    console.warn('Could not get ports:', e);
  }
}

function updateDeviceTitle(deviceId, name) {
  const isD200 = !deviceId || deviceId.startsWith('e9-') || (name && name.includes('D200'));
  const activeName = document.getElementById('active-device-name');
  const badge = document.getElementById('grid-dimensions-badge');
  const statusHw = document.getElementById('status-hardware');

  if (activeName) {
    activeName.textContent = isD200 ? 'Ulanzi Stream Controller D200' : (name || 'Dispositivo');
  }
  if (badge) {
    badge.textContent = isD200 ? 'Grade 3 × 5 (14 Teclas - Ulanzi D200)' : 'Grade 3 × 5 (15 Teclas)';
  }
  if (statusHw) {
    statusHw.innerHTML = `<span class="status-dot"></span> ${isD200 ? 'Ulanzi D200' : (name || 'Conectado')}`;
  }
}

async function loadDevices() {
  const select = document.getElementById('device-select');
  select.innerHTML = '';

  try {
    const devices = await invoke('get_devices');
    if (devices && devices.length > 0) {
      devices.forEach((d) => {
        const opt = document.createElement('option');
        opt.value = d.id;
        const shortName = d.id.startsWith('e9-') || d.name.toLowerCase().includes('d200')
          ? 'Ulanzi D200'
          : (d.name.replace(/Stream Controller\s*/i, '').trim() || d.name);
        opt.textContent = shortName;
        opt.title = `${d.name} (${d.id})`;
        select.appendChild(opt);
      });
      currentDeviceId = devices[0].id;
      updateDeviceTitle(currentDeviceId, devices[0].name);
    } else {
      currentDeviceId = 'e9-02C53A015U3671380';
      const opt = document.createElement('option');
      opt.value = currentDeviceId;
      opt.textContent = 'Ulanzi D200';
      opt.title = `Ulanzi D200 (${currentDeviceId})`;
      select.appendChild(opt);
      updateDeviceTitle(currentDeviceId, 'Ulanzi D200');
    }
    await loadProfiles(currentDeviceId);
  } catch (e) {
    console.error('Failed to load devices:', e);
  }
}

async function loadProfiles(deviceId) {
  const select = document.getElementById('profile-select');
  select.innerHTML = '';

  try {
    const profiles = await invoke('get_profiles', { deviceId });
    const selected = await invoke('get_selected_profile', { deviceId });

    currentProfileName = selected || 'Default';

    profiles.forEach((p) => {
      const opt = document.createElement('option');
      opt.value = p;
      opt.textContent = p;
      if (p === currentProfileName) opt.selected = true;
      select.appendChild(opt);
    });

    await loadProfileData(deviceId, currentProfileName);
    // Pre-warm auto-profiles in background so opening the tab is instantaneous
    loadAutoProfiles().catch(() => {});
  } catch (e) {
    console.error('Failed to load profiles:', e);
  }
}

async function loadProfileData(deviceId, profileName) {
  try {
    currentProfileData = await invoke('get_profile_data', { deviceId, profileName });
    renderKeysGrid();
  } catch (e) {
    console.error('Failed to load profile data:', e);
  }
}

let installedAppsCache = null;
let appSelectorCallback = null;

async function loadInstalledApps(forceRefresh = false) {
  if (!installedAppsCache || forceRefresh) {
    try {
      installedAppsCache = await invoke('get_installed_applications') || [];
    } catch (e) {
      console.warn('Could not load desktop apps:', e);
      if (!installedAppsCache) installedAppsCache = [];
    }
  }
  return installedAppsCache;
}

function initAppSelectorModal() {
  const modal = document.getElementById('app-selector-modal');
  const searchInput = document.getElementById('app-selector-search');
  const closeBtn = document.getElementById('btn-close-app-selector');
  const cancelBtn = document.getElementById('btn-cancel-app-selector');

  const closeModal = () => {
    if (modal) modal.style.display = 'none';
    appSelectorCallback = null;
  };

  if (closeBtn) closeBtn.onclick = closeModal;
  if (cancelBtn) cancelBtn.onclick = closeModal;
  if (modal) {
    modal.onclick = (e) => {
      if (e.target === modal) closeModal();
    };
  }

  if (searchInput) {
    searchInput.oninput = () => {
      const query = searchInput.value.toLowerCase().trim();
      const apps = installedAppsCache || [];
      const filtered = query ? apps.filter((app) => {
        return (app.name && app.name.toLowerCase().includes(query)) ||
               (app.id && app.id.toLowerCase().includes(query)) ||
               (app.exec && app.exec.toLowerCase().includes(query)) ||
               (app.comment && app.comment.toLowerCase().includes(query));
      }) : apps;
      renderAppSelectorList(filtered);
    };
  }
}

async function openAppSelectorModal(currentAppTarget, onSelect) {
  const modal = document.getElementById('app-selector-modal');
  const searchInput = document.getElementById('app-selector-search');
  if (!modal) return;

  appSelectorCallback = onSelect;
  modal.style.display = 'flex';
  if (searchInput) {
    searchInput.value = '';
  }

  // Update cache on open
  const apps = await loadInstalledApps(true);
  renderAppSelectorList(apps, currentAppTarget);

  if (searchInput) {
    searchInput.focus();
  }
}

function renderAppSelectorList(apps, currentAppTarget) {
  const list = document.getElementById('app-selector-list');
  const countEl = document.getElementById('app-selector-count');
  if (!list) return;

  list.innerHTML = '';
  if (countEl) {
    countEl.textContent = `${apps.length} aplicativo${apps.length === 1 ? '' : 's'} encontrado${apps.length === 1 ? '' : 's'}`;
  }

  if (apps.length === 0) {
    list.innerHTML = '<div style="text-align: center; color: var(--text-muted); padding: 24px;">Nenhum aplicativo encontrado.</div>';
    return;
  }

  apps.forEach((app) => {
    const item = document.createElement('div');
    item.className = 'app-selector-item';
    const isSelected = currentAppTarget && (currentAppTarget === app.path || currentAppTarget === app.exec || currentAppTarget === app.id);
    if (isSelected) item.classList.add('selected');

    const iconBox = document.createElement('div');
    iconBox.className = 'app-selector-item-icon';
    const iconSrc = app.iconData || (app.icon && (app.icon.startsWith('data:') || app.icon.startsWith('/') || app.icon.startsWith('http')) ? app.icon : null);
    if (iconSrc) {
      const img = document.createElement('img');
      img.src = iconSrc;
      img.alt = app.name;
      img.onerror = () => {
        iconBox.innerHTML = '';
        iconBox.textContent = '🚀';
      };
      iconBox.innerHTML = '';
      iconBox.appendChild(img);
    } else {
      iconBox.textContent = '🚀';
    }

    const infoBox = document.createElement('div');
    infoBox.className = 'app-selector-item-info';

    const nameEl = document.createElement('div');
    nameEl.className = 'app-selector-item-name';
    nameEl.textContent = app.name;

    const descEl = document.createElement('div');
    descEl.className = 'app-selector-item-desc';
    descEl.textContent = app.comment || app.exec || app.id;

    infoBox.appendChild(nameEl);
    infoBox.appendChild(descEl);

    item.appendChild(iconBox);
    item.appendChild(infoBox);

    item.onclick = () => {
      if (typeof appSelectorCallback === 'function') {
        appSelectorCallback(app);
      }
      const modal = document.getElementById('app-selector-modal');
      if (modal) modal.style.display = 'none';
      appSelectorCallback = null;
    };

    list.appendChild(item);
  });
}

function renderKeysGrid() {
  const grid = document.getElementById('keys-grid');
  grid.innerHTML = '';

  const isD200 = currentDeviceId && currentDeviceId.startsWith('e9-');
  const totalKeys = isD200 ? 14 : (currentDevice?.keyCount || 15);

  if (isD200) {
    grid.className = 'keys-grid grid-ulanzi-d200';
  } else {
    grid.className = 'keys-grid grid-3x5';
  }

  const keys = currentProfileData?.keys || [];

  for (let i = 0; i < totalKeys; i++) {
    const keyData = keys[i];
    const keyEl = document.createElement('div');
    keyEl.className = 'deck-key';
    keyEl.dataset.index = i;

    if (isD200 && i === 13) {
      keyEl.classList.add('key-double-wide');
    }

    if (selectedKeyIndex === i) {
      keyEl.classList.add('selected');
    }

    const row = Math.floor(i / 5);
    const col = i % 5;
    const coordEl = document.createElement('span');
    coordEl.className = 'key-coord';
    if (isD200 && i === 13) {
      coordEl.textContent = '2,3-4 (Larga)';
    } else {
      coordEl.textContent = `${row},${col}`;
    }
    keyEl.appendChild(coordEl);

    if (keyData && keyData.action) {
      keyEl.classList.add('has-action');
      keyEl.draggable = true;

      // Image
      const activeState = keyData.states?.[keyData.current_state] || keyData.states?.[0];
      const imgSrc = activeState?.image_override || activeState?.image;
      if (imgSrc) {
        const imgEl = document.createElement('img');
        imgEl.className = 'key-img';
        if (imgSrc.startsWith('data:') || imgSrc.startsWith('http://') || imgSrc.startsWith('https://')) {
          imgEl.src = imgSrc;
        } else {
          imgEl.src = `http://127.0.0.1:${serverPorts.http_assets}/${imgSrc}`;
        }
        imgEl.onerror = () => { imgEl.style.display = 'none'; };
        keyEl.appendChild(imgEl);
      }

      // Title
      if (activeState && activeState.show !== false && activeState.text) {
        const titleEl = document.createElement('span');
        titleEl.className = 'key-title';
        titleEl.textContent = activeState.text;
        titleEl.style.color = activeState.colour || '#FFFFFF';
        keyEl.appendChild(titleEl);
      }

      // Key Drag Start & End
      keyEl.addEventListener('dragstart', (e) => {
        currentDraggedKeyIndex = i;
        currentDraggedAction = null;
        e.dataTransfer.effectAllowed = 'move';
        const payload = { type: 'key_move', sourceIndex: i };
        e.dataTransfer.setData('application/json', JSON.stringify(payload));
        e.dataTransfer.setData('text/plain', JSON.stringify(payload));
        setTimeout(() => {
          keyEl.classList.add('dragging');
        }, 0);
      });

      keyEl.addEventListener('dragend', () => {
        keyEl.classList.remove('dragging');
        currentDraggedKeyIndex = null;
        document.querySelectorAll('.deck-key.dragover').forEach(el => el.classList.remove('dragover'));
      });
    }

    // Click handler
    keyEl.addEventListener('click', () => {
      selectKey(i);
    });

    // Drag & Drop handlers
    keyEl.addEventListener('dragover', (e) => {
      e.preventDefault();
      if (currentDraggedKeyIndex !== null) {
        if (currentDraggedKeyIndex !== i) {
          e.dataTransfer.dropEffect = 'move';
          keyEl.classList.add('dragover');
        } else {
          e.dataTransfer.dropEffect = 'none';
        }
      } else {
        e.dataTransfer.dropEffect = 'copy';
        keyEl.classList.add('dragover');
      }
    });

    keyEl.addEventListener('dragleave', (e) => {
      if (e.relatedTarget && keyEl.contains(e.relatedTarget)) return;
      keyEl.classList.remove('dragover');
    });

    keyEl.addEventListener('drop', async (e) => {
      e.preventDefault();
      keyEl.classList.remove('dragover');

      // 0. Image file dropped directly on key
      if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
        const file = e.dataTransfer.files[0];
        if (file.type.startsWith('image/') || file.name.match(/\.(png|jpe?g|svg|webp|gif|bmp)$/i)) {
          const reader = new FileReader();
          reader.onload = async (ev) => {
            const dataUri = ev.target.result;
            const targetKey = currentProfileData?.keys?.[i];
            if (targetKey) {
              const curIdx = targetKey.current_state || 0;
              if (!targetKey.states || targetKey.states.length === 0) {
                targetKey.states = [{
                  alignment: "middle", background_colour: "#000000", colour: "#FFFFFF",
                  family: "Liberation Sans", image: dataUri, image_override: dataUri,
                  image_scale: 100, name: "", show: true, size: 16, stroke_colour: "#000000",
                  stroke_size: 3, style: "Regular", text: "", text_override: null, underline: false
                }];
              } else {
                const curState = targetKey.states[curIdx] || targetKey.states[0];
                curState.image = dataUri;
                curState.image_override = dataUri;
              }
              await saveCurrentProfile();
              if (selectedKeyIndex === i) {
                updateInspectorIcon(targetKey);
              }
            }
          };
          reader.readAsDataURL(file);
          return;
        }
      }

      // 1. Move or Swap Key
      let moveSource = currentDraggedKeyIndex;
      if (moveSource === null) {
        try {
          const raw = e.dataTransfer.getData('application/json') || e.dataTransfer.getData('text/plain');
          if (raw) {
            const parsed = JSON.parse(raw);
            if (parsed && (parsed.type === 'key_move' || parsed.sourceIndex !== undefined)) {
              moveSource = parsed.sourceIndex;
            }
          }
        } catch (_) {}
      }

      if (moveSource !== null && moveSource !== undefined) {
        const fromIdx = parseInt(moveSource, 10);
        currentDraggedKeyIndex = null;
        if (!isNaN(fromIdx) && fromIdx !== i) {
          await moveOrSwapKeys(fromIdx, i);
        }
        return;
      }
      currentDraggedKeyIndex = null;

      // 2. Action from sidebar library
      let action = currentDraggedAction;
      if (!action) {
        try {
          const raw = e.dataTransfer.getData('application/json') || e.dataTransfer.getData('text/plain');
          if (raw) {
            const parsed = JSON.parse(raw);
            if (parsed && !parsed.type) action = parsed;
          }
        } catch (_) {}
      }
      if (action) {
        await assignActionToKey(i, action);
      }
      currentDraggedAction = null;
    });

    grid.appendChild(keyEl);
  }
}

function selectKey(index) {
  // Notify previous property inspector of disappearance
  if (selectedKeyIndex !== null && selectedKeyIndex !== index && currentDeviceId) {
    invoke('notify_property_inspector_did_disappear', { deviceId: currentDeviceId, keyIndex: selectedKeyIndex }).catch(() => {});
  }

  selectedKeyIndex = index;
  renderKeysGrid();

  const keyData = currentProfileData?.keys?.[index];
  const emptyEl = document.querySelector('.inspector-empty');
  const formEl = document.getElementById('inspector-form');
  const badgeEl = document.getElementById('selected-key-badge');

  badgeEl.textContent = `Tecla ${index} (Linha ${Math.floor(index / 5)}, Col ${index % 5})`;

  if (!keyData || !keyData.action) {
    emptyEl.style.display = 'flex';
    formEl.style.display = 'none';
    const piWrapper = document.getElementById('pi-frame-wrapper');
    const piIframe = document.getElementById('pi-iframe');
    const nativePiWrapper = document.getElementById('native-pi-wrapper');
    if (piWrapper) piWrapper.style.display = 'none';
    if (piIframe) piIframe.src = 'about:blank';
    if (nativePiWrapper) nativePiWrapper.style.display = 'none';
    return;
  }

  emptyEl.style.display = 'none';
  formEl.style.display = 'block';

  document.getElementById('insp-action-name').textContent = keyData.action.name;
  document.getElementById('insp-action-plugin').textContent = keyData.action.plugin;

  const state = keyData.states?.[keyData.current_state] || keyData.states?.[0] || {};
  const titleInput = document.getElementById('insp-title-input');
  titleInput.value = state.text || '';

  const colorInput = document.getElementById('insp-color-input');
  colorInput.value = state.colour || '#FFFFFF';

  const sizeInput = document.getElementById('insp-size-input');
  sizeInput.value = state.size || 16;

  updateInspectorIcon(keyData);

  // Native Property Inspector handling
  const nativePiWrapper = document.getElementById('native-pi-wrapper');
  const nativePiLaunch = document.getElementById('native-pi-launchapp');
  const nativePiUrl = document.getElementById('native-pi-openurl');
  const nativePiCmd = document.getElementById('native-pi-command');
  const nativePiProf = document.getElementById('native-pi-profile');
  const nativePiBright = document.getElementById('native-pi-brightness');
  const nativePiSim = document.getElementById('native-pi-simulateinput');
  const nativePiMedia = document.getElementById('native-pi-media');

  const nativePiPwMaster = document.getElementById('native-pi-pw-master');
  const nativePiPwMic = document.getElementById('native-pi-pw-mic');
  const nativePiPwOutput = document.getElementById('native-pi-pw-output');
  const nativePiPwInput = document.getElementById('native-pi-pw-input');
  const nativePiPwApp = document.getElementById('native-pi-pw-app');
  const nativePiPwSwitchOutput = document.getElementById('native-pi-pw-switchoutput');
  const nativePiPwSwitchInput = document.getElementById('native-pi-pw-switchinput');
  const nativePiPwPTT = document.getElementById('native-pi-pw-ptt');

  [
    nativePiLaunch, nativePiUrl, nativePiCmd, nativePiProf, nativePiBright, nativePiSim,
    nativePiMedia, nativePiPwMaster, nativePiPwMic, nativePiPwOutput, nativePiPwInput,
    nativePiPwApp, nativePiPwSwitchOutput, nativePiPwSwitchInput, nativePiPwPTT
  ].forEach(el => { if (el) el.style.display = 'none'; });

  const actUuid = keyData.action.uuid || '';
  const actPlugin = keyData.action.plugin || '';
  const piType = keyData.action.property_inspector || '';

  const isPwSwitchOutput = piType === 'native:pipewire:switchoutput'
    || actUuid.includes('switchoutput');

  const isPwSwitchInput = piType === 'native:pipewire:switchinput'
    || actUuid.includes('switchinput');

  const isPwOutputVol = piType === 'native:pipewire:outputvolume'
    || actUuid.endsWith('.outputvolume');

  const isPwInputVol = piType === 'native:pipewire:inputvolume'
    || actUuid.endsWith('.inputvolume');

  const isPwAppVol = piType === 'native:pipewire:appvolume'
    || actUuid.endsWith('.appvolume');

  const isPwPTT = piType === 'native:pipewire:pushtotalk'
    || actUuid.includes('pushtotalk');

  const isPwMicVol = (piType === 'native:pipewire:micvolume'
    || actUuid.endsWith('.micvolume')
    || actUuid.endsWith('.micvolup')
    || actUuid.endsWith('.micvoldown')
    || actUuid.endsWith('.micmute'))
    && !isPwSwitchInput && !isPwPTT && !isPwInputVol;

  const isPwMasterVol = (piType === 'native:pipewire:volume'
    || actUuid.endsWith('.volume')
    || actUuid.endsWith('.volup')
    || actUuid.endsWith('.voldown')
    || actUuid.endsWith('.mute'))
    && !isPwSwitchOutput && !isPwOutputVol && !isPwMicVol;

  const isStarterPack = actPlugin === 'com.amansprojects.starterpack'
    || actPlugin === 'com.uzlinux.system-starterpack'
    || actUuid.startsWith('com.amansprojects.starterpack.')
    || actUuid.startsWith('com.uzlinux.system-starterpack.');

  const isMedia = piType === 'native:media'
    || actUuid.startsWith('com.uzlinux.mpris-media.')
    || actUuid.startsWith('uzlinux.native.media.')
    || actPlugin.includes('oampris')
    || actUuid.startsWith('me.amankhanna.oampris.');

  const isNative = actPlugin === 'uzlinux.native'
    || actPlugin.startsWith('com.uzlinux.')
    || actPlugin.startsWith('com.sfgrimes.pipewire-audio')
    || isStarterPack || isMedia
    || isPwSwitchOutput || isPwSwitchInput || isPwOutputVol || isPwInputVol
    || isPwAppVol || isPwPTT || isPwMicVol || isPwMasterVol;

  const isLaunchApp = keyData.action.uuid === 'uzlinux.native.launchapp'
    || keyData.action.uuid === 'com.uzlinux.system-starterpack.launchapp';

  const isOpenUrl = keyData.action.uuid === 'uzlinux.native.openurl'
    || keyData.action.uuid === 'com.uzlinux.system-starterpack.openurl'
    || keyData.action.uuid === 'com.amansprojects.starterpack.openurl';

  const isCommand = keyData.action.uuid === 'uzlinux.native.command'
    || keyData.action.uuid === 'com.uzlinux.system-starterpack.command'
    || keyData.action.uuid === 'com.amansprojects.starterpack.runcommand';

  const isSwitchProf = keyData.action.uuid === 'uzlinux.native.switchprofile'
    || keyData.action.uuid === 'com.uzlinux.system-starterpack.switchprofile'
    || keyData.action.uuid === 'com.amansprojects.starterpack.switchprofile';

  const isBrightness = keyData.action.uuid === 'uzlinux.native.brightness'
    || keyData.action.uuid === 'com.uzlinux.system-starterpack.brightness'
    || keyData.action.uuid === 'com.amansprojects.starterpack.devicebrightness';

  const isSimulateInput = keyData.action.uuid === 'uzlinux.native.simulateinput'
    || keyData.action.uuid === 'com.uzlinux.system-starterpack.simulateinput'
    || keyData.action.uuid === 'com.amansprojects.starterpack.inputsimulation';

  if (isNative || isLaunchApp) {
    if (nativePiWrapper) nativePiWrapper.style.display = 'block';

    if (isLaunchApp && nativePiLaunch) {
      nativePiLaunch.style.display = 'block';
      const appArgs = document.getElementById('native-app-args');
      const openSelectorBtn = document.getElementById('btn-open-app-selector');
      const cardName = document.getElementById('native-selected-app-name');
      const cardExec = document.getElementById('native-selected-app-exec');
      const cardIcon = document.getElementById('native-selected-app-icon');

      const updateCardDisplay = (name, exec, iconSrc) => {
        if (cardName) cardName.textContent = name || 'Nenhum aplicativo selecionado';
        if (cardExec) cardExec.textContent = exec || (name ? 'Aplicativo configurado' : 'Clique no botão abaixo para escolher');
        if (cardIcon) {
          if (iconSrc) {
            cardIcon.innerHTML = `<img src="${iconSrc}" style="width: 20px; height: 20px; object-fit: contain;">`;
          } else {
            cardIcon.innerHTML = '';
            cardIcon.textContent = '🚀';
          }
        }
      };

      const currentTarget = keyData.settings?.path || keyData.settings?.app || keyData.settings?.exec || '';
      let currentName = keyData.settings?.name || '';
      let currentIcon = keyData.settings?.iconData || '';
      if (currentTarget) {
        const found = installedAppsCache?.find(a => a.path === currentTarget || a.exec === currentTarget || a.id === currentTarget);
        if (found) {
          if (!currentName) currentName = found.name;
          if (!currentIcon) currentIcon = found.iconData;
        } else if (!currentName) {
          currentName = currentTarget;
        }
      }
      updateCardDisplay(currentName, currentTarget, currentIcon);

      if (openSelectorBtn) {
        openSelectorBtn.onclick = () => {
          openAppSelectorModal(currentTarget, async (selectedApp) => {
            if (!keyData.settings) keyData.settings = {};
            keyData.settings.app = selectedApp.exec;
            keyData.settings.exec = selectedApp.exec;
            keyData.settings.name = selectedApp.name;
            keyData.settings.path = selectedApp.path;
            keyData.settings.icon = selectedApp.icon;
            keyData.settings.iconData = selectedApp.iconData;

            updateCardDisplay(selectedApp.name, selectedApp.exec, selectedApp.iconData);

            if (!keyData.states || !keyData.states[keyData.current_state]) {
              keyData.states = keyData.states || [];
              if (!keyData.states[0]) {
                keyData.states[0] = {
                  alignment: "middle",
                  background_colour: "#000000",
                  colour: "#FFFFFF",
                  family: "Liberation Sans",
                  image: "",
                  image_override: null,
                  image_scale: 100,
                  name: "",
                  show: true,
                  size: 16,
                  stroke_colour: "#000000",
                  stroke_size: 3,
                  style: "Regular",
                  text: selectedApp.name,
                  text_override: null,
                  underline: false
                };
              }
            }

            const activeSt = keyData.states[keyData.current_state] || keyData.states[0];

            if (selectedApp.iconData) {
              activeSt.image = selectedApp.iconData;
              activeSt.image_override = selectedApp.iconData;
              if (keyData.action) {
                keyData.action.icon = selectedApp.iconData;
              }
            }

            if (!activeSt.text || activeSt.text === 'App' || activeSt.text === 'Abrir Aplicativo') {
              activeSt.text = selectedApp.name;
              const titleInput = document.getElementById('insp-title-input');
              if (titleInput) titleInput.value = selectedApp.name;
            }
            await saveCurrentProfile();
          });
        };
      }

      if (appArgs) {
        appArgs.value = keyData.settings?.args || '';
        appArgs.oninput = async () => {
          if (!keyData.settings) keyData.settings = {};
          keyData.settings.args = appArgs.value;
          await saveCurrentProfile();
        };
      }
    } else if (isOpenUrl && nativePiUrl) {
      nativePiUrl.style.display = 'block';
      const urlInput = document.getElementById('native-url-input');
      urlInput.value = keyData.settings?.url || keyData.settings?.down || '';
      urlInput.oninput = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.url = urlInput.value;
        keyData.settings.down = urlInput.value;
        await saveCurrentProfile();
      };
    } else if (isCommand && nativePiCmd) {
      nativePiCmd.style.display = 'block';
      const cmdInput = document.getElementById('native-cmd-input');
      cmdInput.value = keyData.settings?.command || keyData.settings?.down || '';
      cmdInput.oninput = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.command = cmdInput.value;
        keyData.settings.down = cmdInput.value;
        await saveCurrentProfile();
      };
    } else if (isSwitchProf && nativePiProf) {
      nativePiProf.style.display = 'block';
      const profSelect = document.getElementById('native-profile-select');
      profSelect.innerHTML = '<option value="">-- Escolha o perfil --</option>';
      const currentProfs = Array.from(document.getElementById('profile-select').options).map(o => o.value);
      currentProfs.forEach(p => {
        const opt = document.createElement('option');
        opt.value = p;
        opt.textContent = p;
        if (keyData.settings?.profile === p) opt.selected = true;
        profSelect.appendChild(opt);
      });
      profSelect.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.profile = profSelect.value;
        await saveCurrentProfile();
      };
    } else if (isBrightness && nativePiBright) {
      nativePiBright.style.display = 'block';
      const brightAction = document.getElementById('native-bright-action');
      const brightSlider = document.getElementById('native-bright-slider');
      const brightValDisplay = document.getElementById('native-bright-val-display');

      brightAction.value = keyData.settings?.action || 'set';
      const currentVal = (keyData.settings?.value !== undefined) ? keyData.settings.value : 50;
      brightSlider.value = currentVal;
      if (brightValDisplay) brightValDisplay.textContent = currentVal;

      brightAction.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.action = brightAction.value;
        await saveCurrentProfile();
      };

      brightSlider.oninput = () => {
        if (brightValDisplay) brightValDisplay.textContent = brightSlider.value;
      };
      brightSlider.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.value = parseInt(brightSlider.value, 10);
        await saveCurrentProfile();
      };
    } else if (isSimulateInput && nativePiSim) {
      nativePiSim.style.display = 'block';
      const simDown = document.getElementById('native-sim-down');
      const simUp = document.getElementById('native-sim-up');

      simDown.value = keyData.settings?.down || '';
      simUp.value = keyData.settings?.up || '';

      simDown.oninput = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.down = simDown.value;
        await saveCurrentProfile();
      };

      simUp.oninput = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.up = simUp.value;
        await saveCurrentProfile();
      };
    } else if (isPwSwitchOutput && nativePiPwSwitchOutput) {
      nativePiPwSwitchOutput.style.display = 'block';
      const select = document.getElementById('native-pw-switchoutput-select');
      const curVal = keyData.settings?.outputName || keyData.settings?.device || 'cycle';

      const populate = (pw) => {
        select.innerHTML = '<option value="cycle">🔄 Alternar ciclicamente entre todos os dispositivos</option>';
        if (pw && pw.sinks) {
          pw.sinks.forEach(sink => {
            const opt = document.createElement('option');
            opt.value = sink.nodeName;
            opt.textContent = `${sink.name} (${sink.nodeName.split('.')[0] || 'alsa'})`;
            if (sink.nodeName === curVal) opt.selected = true;
            select.appendChild(opt);
          });
        }
        if (curVal === 'cycle') select.value = 'cycle';
      };

      if (pipewireAudioCache) {
        populate(pipewireAudioCache);
      }
      fetchPipewireDevices().then(pw => populate(pw));

      select.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.outputName = select.value;
        keyData.settings.device = select.value;
        if (select.value !== 'cycle') {
          const found = pipewireAudioCache?.sinks?.find(s => s.nodeName === select.value);
          if (found) {
            const activeSt = keyData.states[keyData.current_state] || keyData.states[0];
            if (activeSt && (!activeSt.text || activeSt.text === 'Saída' || activeSt.text === 'Switch Output' || pipewireAudioCache?.sinks?.some(s => s.name === activeSt.text))) {
              activeSt.text = found.name;
              const titleInput = document.getElementById('insp-title-input');
              if (titleInput) titleInput.value = found.name;
            }
          }
        }
        await saveCurrentProfile();
      };
    } else if (isPwSwitchInput && nativePiPwSwitchInput) {
      nativePiPwSwitchInput.style.display = 'block';
      const select = document.getElementById('native-pw-switchinput-select');
      const curVal = keyData.settings?.inputName || keyData.settings?.device || 'cycle';

      const populate = (pw) => {
        select.innerHTML = '<option value="cycle">🔄 Alternar ciclicamente entre todos os microfones</option>';
        if (pw && pw.sources) {
          pw.sources.forEach(src => {
            const opt = document.createElement('option');
            opt.value = src.nodeName;
            opt.textContent = `${src.name} (${src.nodeName.split('.')[0] || 'alsa'})`;
            if (src.nodeName === curVal) opt.selected = true;
            select.appendChild(opt);
          });
        }
        if (curVal === 'cycle') select.value = 'cycle';
      };

      if (pipewireAudioCache) {
        populate(pipewireAudioCache);
      }
      fetchPipewireDevices().then(pw => populate(pw));

      select.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.inputName = select.value;
        keyData.settings.device = select.value;
        if (select.value !== 'cycle') {
          const found = pipewireAudioCache?.sources?.find(s => s.nodeName === select.value);
          if (found) {
            const activeSt = keyData.states[keyData.current_state] || keyData.states[0];
            if (activeSt && (!activeSt.text || activeSt.text === 'Entrada' || activeSt.text === 'Switch Input' || pipewireAudioCache?.sources?.some(s => s.name === activeSt.text))) {
              activeSt.text = found.name;
              const titleInput = document.getElementById('insp-title-input');
              if (titleInput) titleInput.value = found.name;
            }
          }
        }
        await saveCurrentProfile();
      };
    } else if (isPwOutputVol && nativePiPwOutput) {
      nativePiPwOutput.style.display = 'block';
      const select = document.getElementById('native-pw-output-select');
      const dirSelect = document.getElementById('native-pw-output-dir');
      const stepSlider = document.getElementById('native-pw-output-step');
      const stepVal = document.getElementById('native-pw-output-step-val');

      const curDev = keyData.settings?.outputName || keyData.settings?.output || keyData.settings?.device || '';
      const populate = (pw) => {
        select.innerHTML = '';
        if (pw && pw.sinks && pw.sinks.length > 0) {
          pw.sinks.forEach(sink => {
            const opt = document.createElement('option');
            opt.value = sink.nodeName;
            opt.textContent = `${sink.name} (${sink.nodeName.split('.')[0] || 'alsa'})`;
            if (sink.nodeName === curDev) opt.selected = true;
            select.appendChild(opt);
          });
        } else {
          select.innerHTML = '<option value="">Nenhuma saída de áudio detectada</option>';
        }
      };

      if (pipewireAudioCache) populate(pipewireAudioCache);
      fetchPipewireDevices().then(pw => populate(pw));

      const savedDir = keyData.settings?.direction || keyData.settings?.action || 'up';
      dirSelect.value = savedDir;

      const savedStep = keyData.settings?.volumeStep || keyData.settings?.step || 5;
      stepSlider.value = savedStep;
      if (stepVal) stepVal.textContent = savedStep;

      select.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.outputName = select.value;
        keyData.settings.device = select.value;
        await saveCurrentProfile();
      };

      dirSelect.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.direction = dirSelect.value;
        await saveCurrentProfile();
      };

      stepSlider.oninput = () => {
        if (stepVal) stepVal.textContent = stepSlider.value;
      };
      stepSlider.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.volumeStep = parseInt(stepSlider.value, 10) || 5;
        keyData.settings.step = keyData.settings.volumeStep;
        await saveCurrentProfile();
      };
    } else if (isPwInputVol && nativePiPwInput) {
      nativePiPwInput.style.display = 'block';
      const select = document.getElementById('native-pw-input-select');
      const dirSelect = document.getElementById('native-pw-input-dir');
      const stepSlider = document.getElementById('native-pw-input-step');
      const stepVal = document.getElementById('native-pw-input-step-val');

      const curDev = keyData.settings?.inputName || keyData.settings?.input || keyData.settings?.device || '';
      const populate = (pw) => {
        select.innerHTML = '';
        if (pw && pw.sources && pw.sources.length > 0) {
          pw.sources.forEach(src => {
            const opt = document.createElement('option');
            opt.value = src.nodeName;
            opt.textContent = `${src.name} (${src.nodeName.split('.')[0] || 'alsa'})`;
            if (src.nodeName === curDev) opt.selected = true;
            select.appendChild(opt);
          });
        } else {
          select.innerHTML = '<option value="">Nenhum microfone detectado</option>';
        }
      };

      if (pipewireAudioCache) populate(pipewireAudioCache);
      fetchPipewireDevices().then(pw => populate(pw));

      const savedDir = keyData.settings?.direction || keyData.settings?.action || 'up';
      dirSelect.value = savedDir;

      const savedStep = keyData.settings?.volumeStep || keyData.settings?.step || 5;
      stepSlider.value = savedStep;
      if (stepVal) stepVal.textContent = savedStep;

      select.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.inputName = select.value;
        keyData.settings.device = select.value;
        await saveCurrentProfile();
      };

      dirSelect.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.direction = dirSelect.value;
        await saveCurrentProfile();
      };

      stepSlider.oninput = () => {
        if (stepVal) stepVal.textContent = stepSlider.value;
      };
      stepSlider.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.volumeStep = parseInt(stepSlider.value, 10) || 5;
        keyData.settings.step = keyData.settings.volumeStep;
        await saveCurrentProfile();
      };
    } else if (isPwAppVol && nativePiPwApp) {
      nativePiPwApp.style.display = 'block';
      const select = document.getElementById('native-pw-app-select');
      const dirSelect = document.getElementById('native-pw-app-dir');
      const stepSlider = document.getElementById('native-pw-app-step');
      const stepVal = document.getElementById('native-pw-app-step-val');

      const curApp = keyData.settings?.appName || keyData.settings?.app || '';
      const populate = (pw) => {
        select.innerHTML = '';
        if (pw && pw.apps && pw.apps.length > 0) {
          pw.apps.forEach(app => {
            const opt = document.createElement('option');
            opt.value = app.appName || app.name;
            opt.textContent = `🎵 ${app.name}`;
            if (opt.value === curApp) opt.selected = true;
            select.appendChild(opt);
          });
        } else {
          select.innerHTML = '<option value="">Nenhum app reproduzindo áudio no momento</option>';
        }
      };

      if (pipewireAudioCache) populate(pipewireAudioCache);
      fetchPipewireDevices().then(pw => populate(pw));

      const savedDir = keyData.settings?.direction || keyData.settings?.action || 'up';
      dirSelect.value = savedDir;

      const savedStep = keyData.settings?.volumeStep || keyData.settings?.step || 5;
      stepSlider.value = savedStep;
      if (stepVal) stepVal.textContent = savedStep;

      select.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.appName = select.value;
        keyData.settings.app = select.value;
        await saveCurrentProfile();
      };

      dirSelect.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.direction = dirSelect.value;
        await saveCurrentProfile();
      };

      stepSlider.oninput = () => {
        if (stepVal) stepVal.textContent = stepSlider.value;
      };
      stepSlider.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.volumeStep = parseInt(stepSlider.value, 10) || 5;
        keyData.settings.step = keyData.settings.volumeStep;
        await saveCurrentProfile();
      };
    } else if (isPwMasterVol && nativePiPwMaster) {
      nativePiPwMaster.style.display = 'block';
      const dirSelect = document.getElementById('native-pw-master-dir');
      const stepSlider = document.getElementById('native-pw-master-step');
      const stepVal = document.getElementById('native-pw-master-step-val');

      let defaultDir = 'up';
      if (actUuid.endsWith('.voldown')) defaultDir = 'down';
      else if (actUuid.endsWith('.mute')) defaultDir = 'mute';

      const savedDir = keyData.settings?.direction || keyData.settings?.action || defaultDir;
      dirSelect.value = savedDir;

      const savedStep = keyData.settings?.volumeStep || keyData.settings?.step || 5;
      stepSlider.value = savedStep;
      if (stepVal) stepVal.textContent = savedStep;

      dirSelect.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.direction = dirSelect.value;
        await saveCurrentProfile();
      };

      stepSlider.oninput = () => {
        if (stepVal) stepVal.textContent = stepSlider.value;
      };
      stepSlider.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.volumeStep = parseInt(stepSlider.value, 10) || 5;
        keyData.settings.step = keyData.settings.volumeStep;
        await saveCurrentProfile();
      };
    } else if (isPwMicVol && nativePiPwMic) {
      nativePiPwMic.style.display = 'block';
      const dirSelect = document.getElementById('native-pw-mic-dir');
      const stepSlider = document.getElementById('native-pw-mic-step');
      const stepVal = document.getElementById('native-pw-mic-step-val');

      let defaultDir = 'up';
      if (actUuid.endsWith('.micvoldown')) defaultDir = 'down';
      else if (actUuid.endsWith('.micmute')) defaultDir = 'mute';

      const savedDir = keyData.settings?.direction || keyData.settings?.action || defaultDir;
      dirSelect.value = savedDir;

      const savedStep = keyData.settings?.volumeStep || keyData.settings?.step || 5;
      stepSlider.value = savedStep;
      if (stepVal) stepVal.textContent = savedStep;

      dirSelect.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.direction = dirSelect.value;
        await saveCurrentProfile();
      };

      stepSlider.oninput = () => {
        if (stepVal) stepVal.textContent = stepSlider.value;
      };
      stepSlider.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.volumeStep = parseInt(stepSlider.value, 10) || 5;
        keyData.settings.step = keyData.settings.volumeStep;
        await saveCurrentProfile();
      };
    } else if (isPwPTT && nativePiPwPTT) {
      nativePiPwPTT.style.display = 'block';
      const select = document.getElementById('native-pw-ptt-select');
      const curDev = keyData.settings?.inputName || keyData.settings?.device || '@DEFAULT_AUDIO_SOURCE@';

      const populate = (pw) => {
        select.innerHTML = '<option value="@DEFAULT_AUDIO_SOURCE@">Microfone Padrão do Sistema</option>';
        if (pw && pw.sources) {
          pw.sources.forEach(src => {
            const opt = document.createElement('option');
            opt.value = src.nodeName;
            opt.textContent = `${src.name} (${src.nodeName.split('.')[0] || 'alsa'})`;
            if (src.nodeName === curDev) opt.selected = true;
            select.appendChild(opt);
          });
        }
        if (curDev === '@DEFAULT_AUDIO_SOURCE@') select.value = '@DEFAULT_AUDIO_SOURCE@';
      };

      if (pipewireAudioCache) populate(pipewireAudioCache);
      fetchPipewireDevices().then(pw => populate(pw));

      select.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.inputName = select.value;
        keyData.settings.device = select.value;
        await saveCurrentProfile();
      };
    } else if (isMedia && nativePiMedia) {
      nativePiMedia.style.display = 'block';
      const mediaCmd = document.getElementById('native-media-cmd');

      let defaultCmd = 'PlayPause';
      if (keyData.action.uuid.includes('next')) defaultCmd = 'Next';
      else if (keyData.action.uuid.includes('prev')) defaultCmd = 'Previous';
      else if (keyData.action.uuid.includes('stop')) defaultCmd = 'Stop';
      else if (keyData.action.uuid.includes('volup') || keyData.action.uuid.includes('volumeup')) defaultCmd = 'VolumeUp';
      else if (keyData.action.uuid.includes('voldown') || keyData.action.uuid.includes('volumedown')) defaultCmd = 'VolumeDown';
      else if (keyData.action.uuid.includes('seekforward')) defaultCmd = 'SeekForward';
      else if (keyData.action.uuid.includes('seekbackward')) defaultCmd = 'SeekBackward';
      else if (keyData.action.uuid.includes('shuffle')) defaultCmd = 'Shuffle';
      else if (keyData.action.uuid.includes('repeat')) defaultCmd = 'Repeat';

      mediaCmd.value = keyData.settings?.command || defaultCmd;

      mediaCmd.onchange = async () => {
        if (!keyData.settings) keyData.settings = {};
        keyData.settings.command = mediaCmd.value;
        await saveCurrentProfile();
      };
    }
  } else {
    if (nativePiWrapper) nativePiWrapper.style.display = 'none';
  }

  // Property Inspector iframe for non-native plugins
  const piWrapper = document.getElementById('pi-frame-wrapper');
  const piIframe = document.getElementById('pi-iframe');

  if (keyData.action.property_inspector && !keyData.action.property_inspector.startsWith('native:') && !isNative) {
    piWrapper.style.display = 'flex';
    const context = keyData.context || `Keypad.${index}.0`;
    const appInfo = { font: "Liberation Sans", language: window.i18n ? window.i18n.getLocale() : "en", platform: "linux", version: "2.14.0" };
    const pluginMeta = { uuid: keyData.action.plugin, version: "1.0.0" };
    const actionInfo = {
      action: keyData.action.uuid,
      context: context,
      device: currentDeviceId,
      payload: { settings: keyData.settings || {}, coordinates: { column: index % 5, row: Math.floor(index / 5) } }
    };

    const infoPayload = encodeURIComponent(JSON.stringify({
      application: appInfo,
      plugin: pluginMeta,
      devicePixelRatio: 1,
      devices: [{ id: currentDeviceId, size: { columns: 5, rows: 3 }, type: 0 }],
      colors: { buttonPressedBackgroundColor: "#303030FF", buttonPressedBorderColor: "#606060FF", buttonPressedTextColor: "#FFFFFFFF", highlightColor: "#0078D7FF" },
      actionInfo: actionInfo
    }));

    piIframe.src = `http://127.0.0.1:${serverPorts.http_assets}/${keyData.action.property_inspector}?port=${serverPorts.pi_ws}&pluginUUID=${keyData.action.plugin}&registerEvent=registerPropertyInspector&context=${encodeURIComponent(context)}&uuid=${encodeURIComponent(context)}&info=${infoPayload}`;

    const sendConnect = () => {
      try {
        if (piIframe && piIframe.contentWindow) {
          piIframe.contentWindow.postMessage({
            event: 'connect',
            payload: [
              serverPorts.pi_ws,
              context,
              'registerPropertyInspector',
              JSON.stringify(appInfo),
              JSON.stringify(actionInfo)
            ]
          }, '*');
        }
      } catch (err) {
        console.warn('postMessage to PI failed:', err);
      }
    };

    function tryConnectIframe() {
      try {
        const cw = piIframe.contentWindow;
        if (!cw) return false;
        const connectFn = cw.connectElgatoStreamDeckSocket || cw.connectOpenActionSocket;
        if (typeof connectFn === 'function') {
          connectFn(
            serverPorts.pi_ws,
            context,
            "registerPropertyInspector",
            JSON.stringify(appInfo),
            JSON.stringify(actionInfo)
          );
          console.log("Connected PI iframe via direct connectElgatoStreamDeckSocket");
          return true;
        }
      } catch (err) {
        // Cross-origin access blocked by browser; postMessage handles it cleanly
      }
      return false;
    }

    piIframe.onload = () => {
      sendConnect();
      tryConnectIframe();
      [60, 150, 300, 600, 1000].forEach(ms => setTimeout(() => {
        sendConnect();
        tryConnectIframe();
      }, ms));
      setTimeout(() => {
        invoke('notify_property_inspector_did_appear', { deviceId: currentDeviceId, keyIndex: index }).catch(() => {});
      }, 250);
    };
  } else {
    piWrapper.style.display = 'none';
    piIframe.src = 'about:blank';
  }
}

async function assignActionToKey(index, action) {
  if (!currentProfileData) return;
  if (!currentProfileData.keys) currentProfileData.keys = [];
  while (currentProfileData.keys.length < 15) {
    currentProfileData.keys.push(null);
  }

  const context = `Keypad.${index}.0`;

  const states = (action.states && action.states.length > 0)
    ? JSON.parse(JSON.stringify(action.states))
    : [{
        alignment: "middle",
        background_colour: "#000000",
        colour: "#FFFFFF",
        family: "Liberation Sans",
        image: action.icon || "",
        image_override: null,
        image_scale: 100,
        name: "",
        show: true,
        size: 16,
        stroke_colour: "#000000",
        stroke_size: 3,
        style: "Regular",
        text: action.name || "",
        text_override: null,
        underline: false
      }];

  currentProfileData.keys[index] = {
    context,
    current_state: 0,
    action: {
      uuid: action.uuid,
      name: action.name,
      tooltip: action.tooltip || '',
      icon: action.icon || '',
      plugin: action.plugin,
      property_inspector: action.property_inspector || '',
      controllers: action.controllers || ['Keypad'],
      supported_in_multi_actions: true,
      visible_in_action_list: true,
      disable_automatic_states: false,
      encoder: null,
      states: states
    },
    settings: {},
    states: states,
    children: null
  };

  await saveCurrentProfile();
  selectKey(index);
}

function handleIconFileUpload(file) {
  if (!file) return;
  if (!file.type.startsWith('image/') && !file.name.match(/\.(png|jpe?g|svg|webp|gif|bmp)$/i)) {
    alert('Por favor selecione um arquivo de imagem válido (PNG, SVG, JPG, WEBP, GIF, BMP).');
    return;
  }
  const reader = new FileReader();
  reader.onload = async (e) => {
    const dataUri = e.target.result;
    await setCustomIconForSelectedKey(dataUri);
  };
  reader.readAsDataURL(file);
}

async function setCustomIconForSelectedKey(iconDataUri) {
  if (selectedKeyIndex === null || !currentProfileData?.keys?.[selectedKeyIndex]) return;
  const keyData = currentProfileData.keys[selectedKeyIndex];
  if (!keyData.states || keyData.states.length === 0) {
    keyData.states = [{
      alignment: "middle",
      background_colour: "#000000",
      colour: "#FFFFFF",
      family: "Liberation Sans",
      image: iconDataUri,
      image_override: iconDataUri,
      image_scale: 100,
      name: "",
      show: true,
      size: 16,
      stroke_colour: "#000000",
      stroke_size: 3,
      style: "Regular",
      text: "",
      text_override: null,
      underline: false
    }];
  } else {
    const curIdx = keyData.current_state || 0;
    const curState = keyData.states[curIdx] || keyData.states[0];
    curState.image = iconDataUri;
    curState.image_override = iconDataUri;
  }
  await saveCurrentProfile();
  updateInspectorIcon(keyData);
}

async function resetCustomIconForSelectedKey() {
  if (selectedKeyIndex === null || !currentProfileData?.keys?.[selectedKeyIndex]) return;
  const keyData = currentProfileData.keys[selectedKeyIndex];
  if (!keyData.states || keyData.states.length === 0) return;
  const curIdx = keyData.current_state || 0;
  const curState = keyData.states[curIdx] || keyData.states[0];
  curState.image_override = null;
  // Restore default action icon
  const defaultIcon = keyData.action?.icon || '';
  curState.image = defaultIcon;
  await saveCurrentProfile();
  updateInspectorIcon(keyData);
}

function updateInspectorIcon(keyData) {
  if (!keyData) return;
  const states = keyData.states || [];
  const curStateIdx = keyData.current_state || 0;
  const activeSt = states[curStateIdx] || states[0] || {};

  // Multiple states dropdown
  const statesWrapper = document.getElementById('insp-states-wrapper');
  const stateBadge = document.getElementById('insp-state-badge');
  const stateSelect = document.getElementById('insp-state-select');

  if (states.length > 1) {
    if (statesWrapper) statesWrapper.style.display = 'block';
    if (stateBadge) {
      stateBadge.style.display = 'inline-block';
      stateBadge.textContent = `Estado ${curStateIdx + 1} de ${states.length}`;
    }
    if (stateSelect) {
      stateSelect.innerHTML = '';
      states.forEach((st, idx) => {
        const opt = document.createElement('option');
        opt.value = idx;
        const label = st.name || st.text || (idx === 0 ? 'Estado 0 (Padrão)' : `Estado ${idx}`);
        opt.textContent = `${idx}: ${label}`;
        if (idx === curStateIdx) opt.selected = true;
        stateSelect.appendChild(opt);
      });
      stateSelect.onchange = () => {
        keyData.current_state = parseInt(stateSelect.value, 10) || 0;
        selectKey(selectedKeyIndex);
      };
    }
  } else {
    if (statesWrapper) statesWrapper.style.display = 'none';
    if (stateBadge) stateBadge.style.display = 'none';
  }

  // Preview Box
  const previewImg = document.getElementById('insp-icon-preview-img');
  const previewFallback = document.getElementById('insp-icon-preview-fallback');
  const imgSrc = activeSt.image_override || activeSt.image || keyData.action?.icon;

  if (imgSrc) {
    let resolvedSrc = imgSrc;
    if (!imgSrc.startsWith('data:') && !imgSrc.startsWith('http://') && !imgSrc.startsWith('https://')) {
      resolvedSrc = `http://127.0.0.1:${serverPorts.http_assets}/${imgSrc}`;
    }
    if (previewImg) {
      previewImg.src = resolvedSrc;
      previewImg.style.display = 'block';
      previewImg.onerror = () => {
        previewImg.style.display = 'none';
        if (previewFallback) previewFallback.style.display = 'block';
      };
    }
    if (previewFallback) previewFallback.style.display = 'none';
  } else {
    if (previewImg) previewImg.style.display = 'none';
    if (previewFallback) previewFallback.style.display = 'block';
  }

  // Scale Slider
  const scaleInput = document.getElementById('insp-icon-scale');
  const scaleVal = document.getElementById('insp-icon-scale-val');
  const scale = activeSt.image_scale || 100;
  if (scaleInput) scaleInput.value = scale;
  if (scaleVal) scaleVal.textContent = `${scale}%`;

  // Show Title Checkbox
  const showTitleCheck = document.getElementById('insp-show-title-check');
  if (showTitleCheck) showTitleCheck.checked = activeSt.show !== false;
}

async function moveOrSwapKeys(fromIndex, toIndex) {
  if (!currentProfileData || fromIndex === toIndex) return;
  if (!currentProfileData.keys) currentProfileData.keys = [];
  while (currentProfileData.keys.length < 15) {
    currentProfileData.keys.push(null);
  }

  const sourceKey = currentProfileData.keys[fromIndex];
  if (!sourceKey || !sourceKey.action) return;

  const targetKey = currentProfileData.keys[toIndex];

  // If property inspector was open for the source key, notify disappearance
  if (selectedKeyIndex === fromIndex && currentDeviceId) {
    invoke('notify_property_inspector_did_disappear', {
      deviceId: currentDeviceId,
      keyIndex: fromIndex
    }).catch(() => {});
  }

  if (targetKey && targetKey.action) {
    // Both keys have actions -> SWAP
    const sourceClone = JSON.parse(JSON.stringify(sourceKey));
    const targetClone = JSON.parse(JSON.stringify(targetKey));

    sourceClone.context = `Keypad.${toIndex}.0`;
    targetClone.context = `Keypad.${fromIndex}.0`;

    currentProfileData.keys[toIndex] = sourceClone;
    currentProfileData.keys[fromIndex] = targetClone;
  } else {
    // Target key is empty -> MOVE
    const sourceClone = JSON.parse(JSON.stringify(sourceKey));
    sourceClone.context = `Keypad.${toIndex}.0`;

    currentProfileData.keys[toIndex] = sourceClone;
    currentProfileData.keys[fromIndex] = null;
  }

  await saveCurrentProfile();
  selectKey(toIndex);
}

async function saveCurrentProfile() {
  if (!currentDeviceId || !currentProfileName || !currentProfileData) return;
  try {
    await invoke('save_profile_data', {
      deviceId: currentDeviceId,
      profileName: currentProfileName,
      profile: currentProfileData
    });
    renderKeysGrid();
  } catch (e) {
    console.error('Failed to save profile:', e);
  }
}

async function loadCategories() {
  const container = document.getElementById('categories-list');
  container.innerHTML = '';

  try {
    categories = await invoke('get_categories') || [];
    renderCategories(categories);
  } catch (e) {
    console.error('Failed to load categories:', e);
  }
}

function renderCategories(cats) {
  const container = document.getElementById('categories-list');
  container.innerHTML = '';

  cats.forEach((cat) => {
    const catBlock = document.createElement('div');
    catBlock.className = 'category-block';

    const titleEl = document.createElement('div');
    titleEl.className = 'category-title';
    titleEl.textContent = cat.name;

    const itemsContainer = document.createElement('div');
    itemsContainer.className = 'category-items';

    cat.actions.forEach((act) => {
      const actEl = document.createElement('div');
      actEl.className = 'action-item';
      actEl.draggable = true;

      if (act.icon) {
        const iconImg = document.createElement('img');
        iconImg.className = 'action-icon';
        iconImg.src = `http://127.0.0.1:${serverPorts.http_assets}/${act.icon}`;
        iconImg.onerror = () => { iconImg.style.display = 'none'; };
        actEl.appendChild(iconImg);
      }

      const nameEl = document.createElement('span');
      nameEl.className = 'action-name';
      nameEl.textContent = act.name;
      actEl.appendChild(nameEl);

      actEl.addEventListener('dragstart', (e) => {
        currentDraggedAction = act;
        e.dataTransfer.setData('text/plain', JSON.stringify(act));
        e.dataTransfer.setData('application/json', JSON.stringify(act));
      });

      actEl.addEventListener('dragend', () => {
        currentDraggedAction = null;
      });

      // Click action to assign to selected or first free key
      actEl.addEventListener('click', async () => {
        let targetIndex = selectedKeyIndex;
        if (targetIndex === null) {
          const keys = currentProfileData?.keys || [];
          const emptyIdx = keys.findIndex((k) => !k);
          targetIndex = (emptyIdx !== -1 && emptyIdx < 14) ? emptyIdx : 0;
        }
        await assignActionToKey(targetIndex, act);
      });

      itemsContainer.appendChild(actEl);
    });

    catBlock.appendChild(titleEl);
    catBlock.appendChild(itemsContainer);
    container.appendChild(catBlock);
  });
}

let autoProfilesCache = {};
let autoProfilesDeviceProfilesCache = {};
let isAutoProfilesLoading = false;

function renderAutoProfilesUI(entries, deviceProfiles) {
  const listEl = document.getElementById('auto-profiles-list');
  const countEl = document.getElementById('auto-profiles-count');
  const targetSelect = document.getElementById('auto-profile-target-select');
  if (!listEl) return;

  if (targetSelect && deviceProfiles && deviceProfiles.length > 0) {
    const curVal = targetSelect.value;
    targetSelect.innerHTML = '';
    deviceProfiles.forEach((p) => {
      const opt = document.createElement('option');
      opt.value = p;
      opt.textContent = p;
      if (p === curVal) opt.selected = true;
      targetSelect.appendChild(opt);
    });
  }

  if (countEl) {
    countEl.textContent = `${entries.length} vínculo${entries.length === 1 ? '' : 's'}`;
  }

  listEl.innerHTML = '';

  if (entries.length === 0) {
    listEl.innerHTML = '<div style="text-align: center; color: var(--text-muted); padding: 20px; font-size: 12px;">Nenhum vínculo configurado ainda para este dispositivo. Adicione um acima!</div>';
    return;
  }

  entries.forEach(({ appKey, targetProfile, deviceId }) => {
    const item = document.createElement('div');
    item.className = 'auto-profile-item';

    const foundApp = installedAppsCache?.find(a =>
      a.id?.toLowerCase() === appKey.toLowerCase() ||
      a.name?.toLowerCase() === appKey.toLowerCase() ||
      (a.exec && a.exec.toLowerCase().includes(appKey.toLowerCase())) ||
      (a.path && a.path.toLowerCase().includes(appKey.toLowerCase()))
    );

    const left = document.createElement('div');
    left.className = 'auto-profile-item-left';

    const iconBox = document.createElement('div');
    iconBox.className = 'auto-profile-item-icon';
    if (foundApp?.iconData) {
      iconBox.innerHTML = `<img src="${foundApp.iconData}" alt="">`;
    } else {
      iconBox.textContent = '🔀';
    }

    const textBox = document.createElement('div');
    textBox.className = 'auto-profile-item-text';

    const appTitle = document.createElement('div');
    appTitle.className = 'auto-profile-item-app';
    appTitle.textContent = foundApp ? `${foundApp.name} (${appKey})` : appKey;

    const targetDesc = document.createElement('div');
    targetDesc.className = 'auto-profile-item-target';
    targetDesc.innerHTML = `<span>Alterna para:</span> <span class="auto-profile-badge">${targetProfile}</span>`;

    textBox.appendChild(appTitle);
    textBox.appendChild(targetDesc);

    left.appendChild(iconBox);
    left.appendChild(textBox);

    const actionsBox = document.createElement('div');
    actionsBox.className = 'auto-profile-item-actions';

    const editBtn = document.createElement('button');
    editBtn.className = 'btn-icon';
    editBtn.title = 'Editar este vínculo';
    editBtn.innerHTML = '✏️';

    editBtn.onclick = () => {
      item.classList.add('editing');
      item.innerHTML = '';

      const editForm = document.createElement('div');
      editForm.className = 'auto-profile-edit-form';

      const appInputWrapper = document.createElement('div');
      appInputWrapper.style.display = 'flex';
      appInputWrapper.style.gap = '4px';
      appInputWrapper.style.flex = '1';
      appInputWrapper.style.minWidth = '180px';

      const editAppInput = document.createElement('input');
      editAppInput.type = 'text';
      editAppInput.className = 'text-input';
      editAppInput.style.padding = '4px 8px';
      editAppInput.style.fontSize = '12px';
      editAppInput.style.flex = '1';
      editAppInput.value = appKey;

      const editPickBtn = document.createElement('button');
      editPickBtn.type = 'button';
      editPickBtn.className = 'btn secondary btn-sm';
      editPickBtn.style.padding = '4px 8px';
      editPickBtn.title = 'Escolher Aplicativo do Sistema';
      editPickBtn.innerHTML = '🔍';
      editPickBtn.onclick = () => {
        openAppSelectorModal(null, (app) => {
          editAppInput.value = app.id || app.exec || app.name;
        });
      };

      appInputWrapper.appendChild(editAppInput);
      appInputWrapper.appendChild(editPickBtn);

      const editProfileSelect = document.createElement('select');
      editProfileSelect.className = 'dropdown';
      editProfileSelect.style.padding = '4px 8px';
      editProfileSelect.style.fontSize = '12px';
      editProfileSelect.style.width = '140px';

      deviceProfiles.forEach((p) => {
        const opt = document.createElement('option');
        opt.value = p;
        opt.textContent = p;
        if (p === targetProfile) opt.selected = true;
        editProfileSelect.appendChild(opt);
      });

      const btnSave = document.createElement('button');
      btnSave.type = 'button';
      btnSave.className = 'btn primary btn-sm';
      btnSave.style.padding = '4px 10px';
      btnSave.title = 'Salvar Alterações';
      btnSave.innerHTML = '✓ Salvar';

      const btnCancel = document.createElement('button');
      btnCancel.type = 'button';
      btnCancel.className = 'btn secondary btn-sm';
      btnCancel.style.padding = '4px 8px';
      btnCancel.title = 'Cancelar Edição';
      btnCancel.innerHTML = '✕';

      btnCancel.onclick = () => {
        const cached = autoProfilesCache[currentDeviceId || 'default'] || [];
        renderAutoProfilesUI(cached, deviceProfiles);
      };

      btnSave.onclick = async () => {
        const newAppKey = editAppInput.value.trim();
        const newTarget = editProfileSelect.value;
        if (!newAppKey) {
          alert('O identificador do aplicativo não pode ficar vazio.');
          return;
        }
        if (!newTarget) {
          alert('Selecione um perfil de destino.');
          return;
        }

        try {
          if (newAppKey !== appKey) {
            await invoke('remove_application_profile', { appId: appKey, deviceId });
          }
          await invoke('set_application_profile', {
            appId: newAppKey,
            deviceId,
            profileName: newTarget,
          });
          await loadAutoProfiles(true);
        } catch (err) {
          console.error('Failed to update auto-profile:', err);
          alert(`Erro ao atualizar: ${err}`);
        }
      };

      editForm.appendChild(appInputWrapper);
      editForm.appendChild(editProfileSelect);
      editForm.appendChild(btnSave);
      editForm.appendChild(btnCancel);

      item.appendChild(editForm);
    };

    const delBtn = document.createElement('button');
    delBtn.className = 'btn-icon danger';
    delBtn.title = 'Remover este vínculo';
    delBtn.innerHTML = '🗑️';
    delBtn.onclick = async () => {
      if (confirm(`Remover troca automática para "${appKey}"?`)) {
        try {
          await invoke('remove_application_profile', { appId: appKey, deviceId: deviceId });
          await loadAutoProfiles(true);
        } catch (err) {
          console.error('Failed to remove auto-profile binding:', err);
        }
      }
    };

    actionsBox.appendChild(editBtn);
    actionsBox.appendChild(delBtn);

    item.appendChild(left);
    item.appendChild(actionsBox);
    listEl.appendChild(item);
  });
}

async function loadAutoProfiles(forceRefresh = false) {
  const listEl = document.getElementById('auto-profiles-list');
  const devId = currentDeviceId || 'default';

  // Se já temos em cache e não é forceRefresh, renderiza instantaneamente sem atraso!
  if (autoProfilesCache[devId] && !forceRefresh) {
    renderAutoProfilesUI(autoProfilesCache[devId], autoProfilesDeviceProfilesCache[devId] || ['Default']);
  } else if (!autoProfilesCache[devId]) {
    if (listEl) {
      listEl.innerHTML = '<div style="color: var(--text-muted); font-size: 12px; padding: 10px;">Carregando vínculos...</div>';
    }
  }

  if (isAutoProfilesLoading) return;
  isAutoProfilesLoading = true;

  try {
    let deviceProfiles = autoProfilesDeviceProfilesCache[devId] || ['Default'];
    if (currentDeviceId) {
      const [profiles, apps, mapping] = await Promise.all([
        invoke('get_profiles', { deviceId: currentDeviceId }).catch(() => ['Default']),
        loadInstalledApps(false),
        invoke('get_application_profiles').catch(() => ({}))
      ]);

      deviceProfiles = profiles && profiles.length > 0 ? profiles : ['Default'];
      autoProfilesDeviceProfilesCache[devId] = deviceProfiles;

      const entries = [];
      for (const [appKey, devMap] of Object.entries(mapping || {})) {
        if (devMap && typeof devMap === 'object') {
          if (currentDeviceId && devMap[currentDeviceId]) {
            entries.push({ appKey, targetProfile: devMap[currentDeviceId], deviceId: currentDeviceId });
          } else {
            for (const [dId, prof] of Object.entries(devMap)) {
              if (!currentDeviceId || dId === currentDeviceId) {
                entries.push({ appKey, targetProfile: prof, deviceId: dId });
              }
            }
          }
        }
      }

      autoProfilesCache[devId] = entries;
      renderAutoProfilesUI(entries, deviceProfiles);
    }
  } catch (e) {
    console.error('Failed to load auto-profiles:', e);
    if (!autoProfilesCache[devId]) {
      if (listEl) listEl.innerHTML = '<div style="color: var(--danger); font-size: 12px; padding: 10px;">Erro ao carregar vínculos de perfis.</div>';
    }
  } finally {
    isAutoProfilesLoading = false;
  }
}

function setupAutoProfilesUI() {
  const autoProfBtn = document.getElementById('btn-auto-profile');
  if (autoProfBtn) {
    autoProfBtn.onclick = () => {
      const modal = document.getElementById('settings-modal');
      if (modal) {
        modal.style.display = 'flex';
        document.querySelectorAll('.tab-btn').forEach((b) => b.classList.remove('active'));
        document.querySelectorAll('.tab-pane').forEach((p) => p.classList.remove('active'));
        const tabBtn = document.querySelector('[data-tab="tab-auto-profiles"]');
        const tabPane = document.getElementById('tab-auto-profiles');
        if (tabBtn) tabBtn.classList.add('active');
        if (tabPane) tabPane.classList.add('active');
        loadAutoProfiles();
      }
    };
  }

  const pickAppBtn = document.getElementById('btn-auto-profile-pick-app');
  const appInput = document.getElementById('auto-profile-app-input');
  if (pickAppBtn) {
    pickAppBtn.onclick = () => {
      openAppSelectorModal(null, (app) => {
        if (appInput) {
          appInput.value = app.id || app.exec || app.name;
        }
      });
    };
  }

  const saveBtn = document.getElementById('btn-save-auto-profile');
  const targetSelect = document.getElementById('auto-profile-target-select');
  const feedbackEl = document.getElementById('auto-profile-feedback');

  if (saveBtn) {
    saveBtn.onclick = async () => {
      const appKey = appInput?.value?.trim();
      const targetProfile = targetSelect?.value;

      if (!appKey) {
        if (feedbackEl) {
          feedbackEl.style.display = 'block';
          feedbackEl.className = 'test-feedback error';
          feedbackEl.textContent = 'Selecione ou digite um aplicativo primeiro.';
        }
        return;
      }
      if (!targetProfile) {
        if (feedbackEl) {
          feedbackEl.style.display = 'block';
          feedbackEl.className = 'test-feedback error';
          feedbackEl.textContent = 'Selecione um perfil de destino.';
        }
        return;
      }

      try {
        await invoke('set_application_profile', {
          appId: appKey,
          deviceId: currentDeviceId || '',
          profileName: targetProfile,
        });

        if (feedbackEl) {
          feedbackEl.style.display = 'block';
          feedbackEl.className = 'test-feedback success';
          feedbackEl.textContent = `✓ Vínculo criado com sucesso! Ao focar em ${appKey}, o perfil muda para "${targetProfile}".`;
          setTimeout(() => { if (feedbackEl) feedbackEl.style.display = 'none'; }, 4000);
        }

        if (appInput) appInput.value = '';
        await loadAutoProfiles();
      } catch (err) {
        if (feedbackEl) {
          feedbackEl.style.display = 'block';
          feedbackEl.className = 'test-feedback error';
          feedbackEl.textContent = `Erro ao salvar vínculo: ${err}`;
        }
      }
    };
  }
}

function setupEventListeners() {
  initAppSelectorModal();
  setupAutoProfilesUI();

  // Device select
  document.getElementById('device-select').addEventListener('change', async (e) => {
    currentDeviceId = e.target.value;
    const selectedOption = e.target.options[e.target.selectedIndex];
    updateDeviceTitle(currentDeviceId, selectedOption?.textContent);
    await loadProfiles(currentDeviceId);
  });

  // Profile select
  document.getElementById('profile-select').addEventListener('change', async (e) => {
    currentProfileName = e.target.value;
    await invoke('set_selected_profile', { deviceId: currentDeviceId, profileName: currentProfileName });
    await loadProfileData(currentDeviceId, currentProfileName);
  });

  // New Profile
  document.getElementById('btn-new-profile').addEventListener('click', async () => {
    const promptMsg = window.tr ? window.tr('dialog.prompt_profile_name') : 'Nome do novo perfil:';
    const name = prompt(promptMsg);
    if (name && name.trim()) {
      await invoke('create_profile', { deviceId: currentDeviceId, profileName: name.trim() });
      await loadProfiles(currentDeviceId);
    }
  });

  // Rename Profile
  document.getElementById('btn-rename-profile').addEventListener('click', async () => {
    if (!currentProfileName) return;
    const promptMsg = window.tr ? window.tr('dialog.prompt_rename_profile') : 'Novo nome para o perfil:';
    const newName = prompt(promptMsg, currentProfileName);
    if (newName && newName.trim() && newName.trim() !== currentProfileName) {
      await invoke('rename_profile', { deviceId: currentDeviceId, oldName: currentProfileName, newName: newName.trim() });
      await loadProfiles(currentDeviceId);
    }
  });

  // Delete Profile
  document.getElementById('btn-delete-profile').addEventListener('click', async () => {
    if (!currentProfileName || currentProfileName === 'Default') {
      alert(window.tr ? window.tr('dialog.cannot_delete_default') || 'Default profile cannot be deleted.' : 'Não é possível excluir o perfil padrão.');
      return;
    }
    const confirmMsg = window.tr ? window.tr('dialog.confirm_delete_profile', [currentProfileName]) : `Tem certeza que deseja excluir o perfil "${currentProfileName}"?`;
    if (confirm(confirmMsg)) {
      await invoke('delete_profile', { deviceId: currentDeviceId, profileName: currentProfileName });
      await loadProfiles(currentDeviceId);
    }
  });

  // Action Search
  document.getElementById('action-search').addEventListener('input', (e) => {
    const query = e.target.value.toLowerCase();
    const filtered = categories.map((cat) => ({
      ...cat,
      actions: cat.actions.filter((a) => a.name.toLowerCase().includes(query))
    })).filter((cat) => cat.actions.length > 0);
    renderCategories(filtered);
  });

  // Inspector Title Change
  document.getElementById('insp-title-input').addEventListener('input', async (e) => {
    if (selectedKeyIndex === null || !currentProfileData?.keys?.[selectedKeyIndex]) return;
    const key = currentProfileData.keys[selectedKeyIndex];
    if (key.states?.[key.current_state]) {
      key.states[key.current_state].text = e.target.value;
      await saveCurrentProfile();
    }
  });

  // Inspector Color Change
  document.getElementById('insp-color-input').addEventListener('input', async (e) => {
    if (selectedKeyIndex === null || !currentProfileData?.keys?.[selectedKeyIndex]) return;
    const key = currentProfileData.keys[selectedKeyIndex];
    if (key.states?.[key.current_state]) {
      key.states[key.current_state].colour = e.target.value;
      await saveCurrentProfile();
    }
  });

  // Inspector Size Change
  document.getElementById('insp-size-input').addEventListener('input', async (e) => {
    if (selectedKeyIndex === null || !currentProfileData?.keys?.[selectedKeyIndex]) return;
    const key = currentProfileData.keys[selectedKeyIndex];
    if (key.states?.[key.current_state]) {
      key.states[key.current_state].size = parseInt(e.target.value, 10) || 16;
      await saveCurrentProfile();
    }
  });

  // Inspector Icon Customization
  const btnUploadIcon = document.getElementById('btn-insp-upload-icon');
  const btnPickSystem = document.getElementById('btn-insp-pick-system');
  const btnResetIcon = document.getElementById('btn-insp-reset-icon');
  const iconFileInput = document.getElementById('insp-icon-file-input');
  const previewBox = document.getElementById('insp-icon-preview-box');
  const scaleInput = document.getElementById('insp-icon-scale');
  const scaleVal = document.getElementById('insp-icon-scale-val');
  const showTitleCheck = document.getElementById('insp-show-title-check');

  if (btnUploadIcon && iconFileInput) {
    btnUploadIcon.addEventListener('click', () => iconFileInput.click());
  }
  if (previewBox && iconFileInput) {
    previewBox.addEventListener('click', () => iconFileInput.click());
    previewBox.addEventListener('dragover', (e) => {
      e.preventDefault();
      previewBox.style.borderColor = 'var(--primary, #89b4fa)';
      previewBox.style.backgroundColor = 'rgba(137, 180, 250, 0.15)';
    });
    previewBox.addEventListener('dragleave', () => {
      previewBox.style.borderColor = 'var(--border-color)';
      previewBox.style.backgroundColor = 'rgba(0,0,0,0.35)';
    });
    previewBox.addEventListener('drop', (e) => {
      e.preventDefault();
      previewBox.style.borderColor = 'var(--border-color)';
      previewBox.style.backgroundColor = 'rgba(0,0,0,0.35)';
      if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
        handleIconFileUpload(e.dataTransfer.files[0]);
      }
    });
  }
  if (iconFileInput) {
    iconFileInput.addEventListener('change', () => {
      if (iconFileInput.files && iconFileInput.files.length > 0) {
        handleIconFileUpload(iconFileInput.files[0]);
        iconFileInput.value = '';
      }
    });
  }
  if (btnPickSystem) {
    btnPickSystem.addEventListener('click', () => {
      if (selectedKeyIndex === null) return;
      openAppSelectorModal(null, async (selectedApp) => {
        if (selectedApp.iconData) {
          await setCustomIconForSelectedKey(selectedApp.iconData);
        }
      });
    });
  }
  if (btnResetIcon) {
    btnResetIcon.addEventListener('click', async () => {
      await resetCustomIconForSelectedKey();
    });
  }
  if (scaleInput) {
    scaleInput.addEventListener('input', async (e) => {
      const val = parseInt(e.target.value, 10) || 100;
      if (scaleVal) scaleVal.textContent = `${val}%`;
      if (selectedKeyIndex === null || !currentProfileData?.keys?.[selectedKeyIndex]) return;
      const key = currentProfileData.keys[selectedKeyIndex];
      const curIdx = key.current_state || 0;
      if (key.states?.[curIdx]) {
        key.states[curIdx].image_scale = val;
        await saveCurrentProfile();
      }
    });
  }
  if (showTitleCheck) {
    showTitleCheck.addEventListener('change', async (e) => {
      if (selectedKeyIndex === null || !currentProfileData?.keys?.[selectedKeyIndex]) return;
      const key = currentProfileData.keys[selectedKeyIndex];
      const curIdx = key.current_state || 0;
      if (key.states?.[curIdx]) {
        key.states[curIdx].show = e.target.checked;
        await saveCurrentProfile();
      }
    });
  }

  // Test Key Press
  document.getElementById('btn-test-key').addEventListener('click', async () => {
    if (selectedKeyIndex === null || !currentDeviceId) return;
    await invoke('trigger_virtual_press', { deviceId: currentDeviceId, keyIndex: selectedKeyIndex });
  });

  // Clear Key (Click, Drop target & Delete shortcut)
  const clearBtn = document.getElementById('btn-clear-key');
  clearBtn.addEventListener('click', async () => {
    if (selectedKeyIndex === null || !currentProfileData?.keys) return;
    currentProfileData.keys[selectedKeyIndex] = null;
    await saveCurrentProfile();
    selectKey(selectedKeyIndex);
  });

  clearBtn.addEventListener('dragover', (e) => {
    if (currentDraggedKeyIndex !== null) {
      e.preventDefault();
      clearBtn.classList.add('dragover');
    }
  });

  clearBtn.addEventListener('dragleave', () => {
    clearBtn.classList.remove('dragover');
  });

  clearBtn.addEventListener('drop', async (e) => {
    if (currentDraggedKeyIndex !== null) {
      e.preventDefault();
      clearBtn.classList.remove('dragover');
      const fromIdx = currentDraggedKeyIndex;
      currentDraggedKeyIndex = null;
      if (currentProfileData?.keys?.[fromIdx]) {
        currentProfileData.keys[fromIdx] = null;
        await saveCurrentProfile();
        selectKey(fromIdx);
      }
    }
  });

  // Delete / Backspace key shortcut to clear selected key
  document.addEventListener('keydown', async (e) => {
    const activeTag = document.activeElement ? document.activeElement.tagName.toLowerCase() : '';
    if (activeTag === 'input' || activeTag === 'textarea' || activeTag === 'select') return;

    if ((e.key === 'Delete' || e.key === 'Backspace') && selectedKeyIndex !== null) {
      if (currentProfileData?.keys?.[selectedKeyIndex]) {
        e.preventDefault();
        currentProfileData.keys[selectedKeyIndex] = null;
        await saveCurrentProfile();
        selectKey(selectedKeyIndex);
      }
    }
  });

  // Settings Modal & Tabs
  const modal = document.getElementById('settings-modal');

  // Tab switching
  document.querySelectorAll('.tab-btn').forEach((btn) => {
    btn.addEventListener('click', () => {
      document.querySelectorAll('.tab-btn').forEach((b) => b.classList.remove('active'));
      document.querySelectorAll('.tab-pane').forEach((p) => p.classList.remove('active'));
      btn.classList.add('active');
      const target = document.getElementById(btn.dataset.tab);
      if (target) target.classList.add('active');
      if (btn.dataset.tab === 'tab-settings-plugins') {
        loadPluginsList();
      }
      if (btn.dataset.tab === 'tab-auto-profiles') {
        loadAutoProfiles();
      }
    });
  });

  // Toggle OBS password visibility
  document.getElementById('btn-toggle-obs-pwd').addEventListener('click', () => {
    const inp = document.getElementById('obs-pwd');
    inp.type = inp.type === 'password' ? 'text' : 'password';
  });

  // Toggle Dual OBS
  document.getElementById('check-obs-dual').addEventListener('change', (e) => {
    document.getElementById('obs-dual-container').style.display = e.target.checked ? 'block' : 'none';
  });

  // Test OBS Connection
  document.getElementById('btn-test-obs').addEventListener('click', async () => {
    const feedback = document.getElementById('obs-test-feedback');
    const host = document.getElementById('obs-ip').value.trim() || '127.0.0.1';
    const port = parseInt(document.getElementById('obs-port').value, 10) || 4455;
    feedback.className = 'test-feedback loading';
    feedback.textContent = '⏳ Conectando...';

    try {
      const res = await invoke('test_obs_connection', { host, port });
      feedback.className = 'test-feedback success';
      feedback.textContent = `✅ ${res}`;
    } catch (err) {
      feedback.className = 'test-feedback error';
      feedback.textContent = `❌ ${err}`;
    }
  });

  document.getElementById('btn-settings').addEventListener('click', async () => {
    // Reset test feedback
    document.getElementById('obs-test-feedback').textContent = '';

    // Load general settings
    try {
      const s = await invoke('get_settings');
      if (s) {
        document.getElementById('brightness-slider').value = s.brightness;
        document.getElementById('brightness-val').textContent = `${s.brightness}%`;
        document.getElementById('sleep-input').value = s.sleep_timeout_minutes;
        document.getElementById('check-background').checked = s.background;
        document.getElementById('check-sleep-lock').checked = s.sleep_when_computer_locked;
        const langSelect = document.getElementById('settings-language');
        if (langSelect) langSelect.value = s.language || 'auto';
      }
    } catch (_) {}

    // Load OBS settings
    try {
      const obs = await invoke('get_obs_settings');
      if (obs) {
        document.getElementById('obs-ip').value = obs.ip1 || '127.0.0.1';
        document.getElementById('obs-port').value = obs.port1 || '4455';
        document.getElementById('obs-pwd').value = obs.pwd1 || '';

        const hasDual = !!(obs.ip2 || obs.port2);
        document.getElementById('check-obs-dual').checked = hasDual;
        document.getElementById('obs-dual-container').style.display = hasDual ? 'block' : 'none';
        document.getElementById('obs-ip2').value = obs.ip2 || '';
        document.getElementById('obs-port2').value = obs.port2 || '4455';
        document.getElementById('obs-pwd2').value = obs.pwd2 || '';
      }
    } catch (_) {}

    loadPluginsList();
    modal.style.display = 'flex';
  });

  document.getElementById('btn-close-settings').addEventListener('click', () => {
    modal.style.display = 'none';
  });

  document.getElementById('brightness-slider').addEventListener('input', (e) => {
    document.getElementById('brightness-val').textContent = `${e.target.value}%`;
  });

  document.getElementById('btn-save-settings').addEventListener('click', async () => {
    // Save general settings
    try {
      const current = await invoke('get_settings') || {};
      current.brightness = parseInt(document.getElementById('brightness-slider').value, 10);
      current.sleep_timeout_minutes = parseInt(document.getElementById('sleep-input').value, 10);
      current.background = document.getElementById('check-background').checked;
      current.sleep_when_computer_locked = document.getElementById('check-sleep-lock').checked;
      const langSelect = document.getElementById('settings-language');
      if (langSelect) {
        current.language = langSelect.value;
        if (window.i18n) {
          window.i18n.setLanguage(current.language);
        }
      }
      await invoke('set_settings', { settings: current });
      await loadCategories();
    } catch (e) {
      console.error('Failed to save general settings:', e);
    }

    // Save OBS settings
    try {
      const obs = await invoke('get_obs_settings') || {};
      obs.ip1 = document.getElementById('obs-ip').value.trim() || '127.0.0.1';
      obs.port1 = document.getElementById('obs-port').value.trim() || '4455';
      obs.pwd1 = document.getElementById('obs-pwd').value;

      if (document.getElementById('check-obs-dual').checked) {
        obs.ip2 = document.getElementById('obs-ip2').value.trim();
        obs.port2 = document.getElementById('obs-port2').value.trim() || '4455';
        obs.pwd2 = document.getElementById('obs-pwd2').value;
      } else {
        obs.ip2 = '';
        obs.port2 = '';
        obs.pwd2 = '';
      }

      await invoke('save_obs_settings', { settings: obs });
    } catch (e) {
      console.error('Failed to save OBS settings:', e);
    }

    modal.style.display = 'none';
  });

  // Plugins Modal
  const pluginsModal = document.getElementById('plugins-modal');
  document.getElementById('btn-plugins').addEventListener('click', async () => {
    pluginsModal.style.display = 'flex';
    await loadPluginsList();
  });

  document.getElementById('btn-close-plugins-modal').addEventListener('click', () => {
    pluginsModal.style.display = 'none';
  });

  document.getElementById('btn-refresh-plugins').addEventListener('click', async () => {
    await loadPluginsList();
  });

  // OpenDeck Import Modal Controls & State
  const odModal = document.getElementById('opendeck-import-modal');
  const odLoading = document.getElementById('od-import-loading');
  const odNotFound = document.getElementById('od-not-found');
  const odContent = document.getElementById('od-content-wrapper');
  const odFeedback = document.getElementById('od-import-feedback');
  const odLocation = document.getElementById('od-location-info');
  const odProfilesList = document.getElementById('od-profiles-list');
  const odPluginsList = document.getElementById('od-plugins-list');
  const odProfilesCount = document.getElementById('od-profiles-count');
  const odPluginsCount = document.getElementById('od-plugins-count');
  const odCheckProfiles = document.getElementById('od-check-profiles');
  const odCheckPlugins = document.getElementById('od-check-plugins');
  const odCheckApps = document.getElementById('od-check-apps');
  const odCheckImages = document.getElementById('od-check-images');
  const odCheckSettings = document.getElementById('od-check-settings');
  const btnOdImportAll = document.getElementById('btn-od-import-all');
  const btnOdImportSelected = document.getElementById('btn-od-import-selected');

  async function openOpenDeckImportModal() {
    if (!odModal) return;
    odModal.style.display = 'flex';
    odLoading.style.display = 'block';
    odNotFound.style.display = 'none';
    odContent.style.display = 'none';
    odFeedback.style.display = 'none';
    if (btnOdImportAll) btnOdImportAll.disabled = false;
    if (btnOdImportSelected) btnOdImportSelected.disabled = false;

    try {
      const scan = await invoke('get_opendeck_scan_info');
      odLoading.style.display = 'none';
      if (!scan || !scan.found) {
        odNotFound.style.display = 'block';
        return;
      }
      odLocation.textContent = `Instalação encontrada em: ${scan.path}`;
      odContent.style.display = 'flex';

      // Profiles
      odProfilesCount.textContent = scan.profiles.length;
      odProfilesList.innerHTML = '';
      if (scan.profiles.length === 0) {
        odProfilesList.innerHTML = '<span style="font-size:12px; color:var(--text-muted); font-style:italic;">Nenhum perfil encontrado no OpenDeck.</span>';
      } else {
        scan.profiles.forEach(prof => {
          const row = document.createElement('label');
          row.className = 'od-item-row';
          const shortDev = prof.device_id.length > 14 ? prof.device_id.substring(0, 14) + '...' : prof.device_id;
          row.innerHTML = `
            <input type="checkbox" class="od-prof-check" value="${prof.rel_path}" checked>
            <span><strong>${prof.name}</strong> <span style="color:var(--text-muted); font-size:11px;">(${prof.keys_count} teclas · ${shortDev})</span></span>
          `;
          odProfilesList.appendChild(row);
        });
      }

      // Plugins
      odPluginsCount.textContent = scan.plugins.length;
      odPluginsList.innerHTML = '';
      if (scan.plugins.length === 0) {
        odPluginsList.innerHTML = '<span style="font-size:12px; color:var(--text-muted); font-style:italic;">Nenhum plugin adicional encontrado no OpenDeck.</span>';
      } else {
        scan.plugins.forEach(plugin => {
          const row = document.createElement('label');
          row.className = 'od-item-row';
          const isStarter = plugin.folder_name.includes('starterpack');
          const isNative = plugin.is_native;
          row.innerHTML = `
            <input type="checkbox" class="od-plug-check" value="${plugin.folder_name}" ${isStarter ? 'disabled' : 'checked'}>
            <span>
              <strong>${plugin.name || plugin.folder_name}</strong>
              ${plugin.version ? `<span style="font-size:10px; opacity:0.7;">v${plugin.version}</span>` : ''}
              ${isNative ? `<span style="background: rgba(16, 185, 129, 0.15); color: #10b981; font-size: 10px; padding: 1px 6px; border-radius: 4px; margin-left: 4px;">Nativo em Rust</span>` : ''}
              ${isStarter ? `<span style="color:var(--text-muted); font-size:10px; margin-left:4px;">(embutido no app)</span>` : ''}
            </span>
          `;
          odPluginsList.appendChild(row);
        });
      }

      odCheckProfiles.checked = true;
      odCheckPlugins.checked = true;
      odCheckApps.checked = scan.has_applications;
      odCheckImages.checked = scan.has_images;
      odCheckSettings.checked = scan.has_settings;
    } catch (err) {
      odLoading.style.display = 'none';
      odNotFound.style.display = 'block';
      odNotFound.textContent = `Erro ao verificar o OpenDeck: ${err}`;
    }
  }

  function closeOpenDeckImportModal() {
    if (odModal) odModal.style.display = 'none';
  }

  document.getElementById('btn-close-opendeck-import-modal')?.addEventListener('click', closeOpenDeckImportModal);
  document.getElementById('btn-close-od-modal')?.addEventListener('click', closeOpenDeckImportModal);

  // Profile selection toggles
  odCheckProfiles?.addEventListener('change', (e) => {
    document.querySelectorAll('.od-prof-check').forEach(cb => cb.checked = e.target.checked);
  });
  document.getElementById('btn-od-select-all-profiles')?.addEventListener('click', () => {
    if (odCheckProfiles) odCheckProfiles.checked = true;
    document.querySelectorAll('.od-prof-check').forEach(cb => cb.checked = true);
  });
  document.getElementById('btn-od-unselect-all-profiles')?.addEventListener('click', () => {
    if (odCheckProfiles) odCheckProfiles.checked = false;
    document.querySelectorAll('.od-prof-check').forEach(cb => cb.checked = false);
  });

  // Plugin selection toggles
  odCheckPlugins?.addEventListener('change', (e) => {
    document.querySelectorAll('.od-plug-check:not([disabled])').forEach(cb => cb.checked = e.target.checked);
  });
  document.getElementById('btn-od-select-all-plugins')?.addEventListener('click', () => {
    if (odCheckPlugins) odCheckPlugins.checked = true;
    document.querySelectorAll('.od-plug-check:not([disabled])').forEach(cb => cb.checked = true);
  });
  document.getElementById('btn-od-unselect-all-plugins')?.addEventListener('click', () => {
    if (odCheckPlugins) odCheckPlugins.checked = false;
    document.querySelectorAll('.od-plug-check:not([disabled])').forEach(cb => cb.checked = false);
  });

  async function executeOpenDeckImport(importAll) {
    if (btnOdImportAll) btnOdImportAll.disabled = true;
    if (btnOdImportSelected) btnOdImportSelected.disabled = true;
    odFeedback.style.display = 'block';
    odFeedback.className = 'test-feedback loading';
    odFeedback.textContent = importAll ? '⚡ Importando todos os dados do OpenDeck...' : '⏳ Importando itens selecionados...';

    const selectedProfiles = Array.from(document.querySelectorAll('.od-prof-check:checked')).map(cb => cb.value);
    const selectedPlugins = Array.from(document.querySelectorAll('.od-plug-check:checked')).map(cb => cb.value);

    const options = {
      import_all: importAll,
      import_profiles: importAll || (odCheckProfiles ? odCheckProfiles.checked : true),
      selected_profiles: selectedProfiles,
      import_plugins: importAll || (odCheckPlugins ? odCheckPlugins.checked : true),
      selected_plugins: selectedPlugins,
      import_applications: importAll || (odCheckApps ? odCheckApps.checked : true),
      import_settings: importAll || (odCheckSettings ? odCheckSettings.checked : true),
      import_images: importAll || (odCheckImages ? odCheckImages.checked : true),
    };

    try {
      const res = await invoke('import_opendeck_custom', { options });
      odFeedback.className = 'test-feedback success';
      odFeedback.textContent = `✅ ${res.message || 'Importação realizada com sucesso!'}`;

      // Refresh app state
      await loadDevices();
      await loadCategories();
      await loadPluginsList();
      await loadAutoProfiles();
      if (currentDeviceId && currentProfileName) {
        await loadProfileData(currentDeviceId, currentProfileName);
      }

      setTimeout(() => {
        closeOpenDeckImportModal();
      }, 1600);
    } catch (err) {
      odFeedback.className = 'test-feedback error';
      odFeedback.textContent = `❌ Falha ao importar: ${err}`;
      if (btnOdImportAll) btnOdImportAll.disabled = false;
      if (btnOdImportSelected) btnOdImportSelected.disabled = false;
    }
  }

  btnOdImportAll?.addEventListener('click', () => executeOpenDeckImport(true));
  btnOdImportSelected?.addEventListener('click', () => executeOpenDeckImport(false));

  const importSettingsBtn = document.getElementById('btn-import-opendeck-settings');
  if (importSettingsBtn) {
    importSettingsBtn.addEventListener('click', openOpenDeckImportModal);
  }

  const importPluginsBtn = document.getElementById('btn-import-opendeck-plugins');
  if (importPluginsBtn) {
    importPluginsBtn.addEventListener('click', openOpenDeckImportModal);
  }

  document.getElementById('btn-open-plugins-dir').addEventListener('click', async () => {
    try {
      await invoke('open_plugin_directory', { uuid: null });
    } catch (e) {
      console.error('Failed to open plugins dir:', e);
    }
  });

  const fileInput = document.getElementById('plugin-file-input');
  document.getElementById('btn-install-plugin-trigger').addEventListener('click', () => {
    fileInput.click();
  });

  fileInput.addEventListener('change', async (e) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const feedback = document.getElementById('plugins-feedback');
    feedback.style.display = 'block';
    feedback.className = 'plugins-feedback';
    feedback.textContent = `Instalando plugin ${file.name}...`;

    try {
      // In Tauri Webview, file.path contains full local path
      const filePath = file.path || file.name;
      const res = await invoke('install_plugin', { filePath });
      feedback.className = 'plugins-feedback success';
      feedback.textContent = `✅ ${res}`;
      await loadPluginsList();
      await loadCategories();
    } catch (err) {
      feedback.className = 'plugins-feedback error';
      feedback.textContent = `❌ Erro ao instalar: ${err}`;
    }
    fileInput.value = '';
  });

  const fullManagerBtn = document.getElementById('btn-settings-open-full-manager');
  if (fullManagerBtn) {
    fullManagerBtn.addEventListener('click', () => {
      document.getElementById('settings-modal').style.display = 'none';
      document.getElementById('plugins-modal').style.display = 'flex';
      loadPluginsList();
    });
  }

  const openSettingsPluginsDir = document.getElementById('btn-settings-open-plugins-dir');
  if (openSettingsPluginsDir) {
    openSettingsPluginsDir.addEventListener('click', async () => {
      try {
        await invoke('open_plugin_directory', { uuid: null });
      } catch (e) {
        console.error('Failed to open plugins dir:', e);
      }
    });
  }

  const importSettingsPlugins = document.getElementById('btn-settings-import-opendeck');
  if (importSettingsPlugins) {
    importSettingsPlugins.addEventListener('click', openOpenDeckImportModal);
  }
}

async function loadPluginsList() {
  const loading = document.getElementById('plugins-loading');
  const settingsLoading = document.getElementById('settings-plugins-loading');
  if (loading) loading.style.display = 'block';
  if (settingsLoading) settingsLoading.style.display = 'block';

  try {
    const plugins = await invoke('get_plugins_list') || [];
    if (loading) loading.style.display = 'none';
    if (settingsLoading) settingsLoading.style.display = 'none';
    renderPluginsList(plugins, 'plugins-list');
    renderPluginsList(plugins, 'settings-plugins-list');
  } catch (e) {
    if (loading) {
      loading.style.display = 'block';
      loading.textContent = `Falha ao listar plugins: ${e}`;
    }
    if (settingsLoading) {
      settingsLoading.style.display = 'block';
      settingsLoading.textContent = `Falha ao listar plugins: ${e}`;
    }
  }
}

function renderPluginsList(plugins, targetId = 'plugins-list') {
  const container = document.getElementById(targetId);
  if (!container) return;
  container.innerHTML = '';

  if (plugins.length === 0) {
    container.innerHTML = '<div class="plugins-loading">Nenhum plugin encontrado no diretório de plugins.</div>';
    return;
  }

  plugins.forEach((plugin) => {
    const card = document.createElement('div');
    card.className = 'plugin-card';
    if (!plugin.isEnabled) {
      card.style.opacity = '0.75';
    }

    // Header row
    const header = document.createElement('div');
    header.className = 'plugin-card-header';

    const left = document.createElement('div');
    left.className = 'plugin-card-left';

    if (plugin.icon) {
      const img = document.createElement('img');
      img.className = 'plugin-card-icon';
      img.src = `http://127.0.0.1:${serverPorts.http_assets}/${plugin.icon}`;
      img.onerror = () => {
        img.style.display = 'none';
        const fallback = document.createElement('div');
        fallback.className = 'plugin-card-icon-fallback';
        fallback.textContent = plugin.isBuiltin ? '⚡' : '🧩';
        left.insertBefore(fallback, left.firstChild);
      };
      left.appendChild(img);
    } else {
      const fallback = document.createElement('div');
      fallback.className = 'plugin-card-icon-fallback';
      fallback.textContent = plugin.isBuiltin ? '⚡' : '🧩';
      left.appendChild(fallback);
    }

    const info = document.createElement('div');
    info.className = 'plugin-card-info';

    const titleRow = document.createElement('div');
    titleRow.className = 'plugin-title-row';

    const nameEl = document.createElement('span');
    nameEl.className = 'plugin-name';
    nameEl.textContent = plugin.name;
    titleRow.appendChild(nameEl);

    const verEl = document.createElement('span');
    verEl.className = 'plugin-version';
    verEl.textContent = `v${plugin.version}`;
    titleRow.appendChild(verEl);

    if (plugin.author) {
      const authorEl = document.createElement('span');
      authorEl.className = 'plugin-author';
      authorEl.textContent = `por ${plugin.author}`;
      titleRow.appendChild(authorEl);
    }

    info.appendChild(titleRow);

    const uuidEl = document.createElement('span');
    uuidEl.className = 'plugin-uuid';
    uuidEl.textContent = `${plugin.uuid} (${plugin.actionsCount} ações)`;
    info.appendChild(uuidEl);

    left.appendChild(info);
    header.appendChild(left);

    // Badges
    const badges = document.createElement('div');
    badges.className = 'plugin-card-badges';

    // Built-in badge
    if (plugin.isBuiltin) {
      const builtinBadge = document.createElement('span');
      builtinBadge.className = 'badge type-native';
      builtinBadge.textContent = '🛡️ Pré-instalado (Nativo)';
      builtinBadge.title = 'Plugin nativo do aplicativo. Não pode ser desinstalado, mas pode ser desativado.';
      badges.appendChild(builtinBadge);
    } else {
      // Type badge for external
      const typeBadge = document.createElement('span');
      if (plugin.pluginType === 'native') {
        typeBadge.className = 'badge type-native';
        typeBadge.textContent = '⚙️ Binário';
      } else if (plugin.pluginType === 'node') {
        typeBadge.className = 'badge type-node';
        typeBadge.textContent = '🟩 Node.js';
      } else if (plugin.pluginType === 'html') {
        typeBadge.className = 'badge type-html';
        typeBadge.textContent = '🌐 Webview';
      } else {
        typeBadge.className = 'badge type-native';
        typeBadge.textContent = '📦 Pacote';
      }
      badges.appendChild(typeBadge);
    }

    // Status / Enabled badge
    const enabledBadge = document.createElement('span');
    if (plugin.isEnabled) {
      enabledBadge.className = 'badge status-connected';
      enabledBadge.textContent = '🟢 Ativado';
    } else {
      enabledBadge.className = 'badge status-stopped';
      enabledBadge.textContent = '⚪ Desativado';
    }
    badges.appendChild(enabledBadge);

    header.appendChild(badges);
    card.appendChild(header);

    // Description
    if (plugin.description) {
      const desc = document.createElement('div');
      desc.className = 'plugin-desc';
      desc.textContent = plugin.description;
      card.appendChild(desc);
    }

    // Actions
    const actions = document.createElement('div');
    actions.className = 'plugin-card-actions';

    // Toggle Ativar / Desativar button for ALL plugins!
    const btnToggleEnable = document.createElement('button');
    btnToggleEnable.className = plugin.isEnabled ? 'btn secondary btn-sm' : 'btn primary btn-sm';
    btnToggleEnable.innerHTML = plugin.isEnabled ? '⏸️ Desativar' : '▶️ Ativar';
    btnToggleEnable.title = plugin.isEnabled ? 'Desativa o plugin e oculta suas ações da biblioteca' : 'Ativa o plugin e disponibiliza suas ações';
    btnToggleEnable.addEventListener('click', async () => {
      btnToggleEnable.disabled = true;
      try {
        await invoke('set_plugin_enabled', { uuid: plugin.uuid, enabled: !plugin.isEnabled });
        await loadPluginsList();
        await loadCategories();
      } catch (err) {
        alert(`Erro ao alterar status do plugin: ${err}`);
      }
    });
    actions.appendChild(btnToggleEnable);

    // Process control for external non-HTML plugins when enabled
    if (!plugin.isBuiltin && plugin.pluginType !== 'html' && plugin.isEnabled) {
      if (plugin.isRunning) {
        const btnStop = document.createElement('button');
        btnStop.className = 'btn secondary btn-sm';
        btnStop.textContent = '⏹️ Parar';
        btnStop.addEventListener('click', async () => {
          btnStop.disabled = true;
          try {
            await invoke('stop_plugin', { uuid: plugin.uuid });
            await loadPluginsList();
          } catch (err) {
            alert(`Erro ao parar plugin: ${err}`);
          }
        });
        actions.appendChild(btnStop);
      } else {
        const btnStart = document.createElement('button');
        btnStart.className = 'btn secondary btn-sm';
        btnStart.textContent = '▶️ Iniciar';
        btnStart.addEventListener('click', async () => {
          btnStart.disabled = true;
          try {
            await invoke('start_plugin', { uuid: plugin.uuid });
            await loadPluginsList();
          } catch (err) {
            alert(`Erro ao iniciar plugin: ${err}`);
          }
        });
        actions.appendChild(btnStart);
      }

      const btnRestart = document.createElement('button');
      btnRestart.className = 'btn secondary btn-sm';
      btnRestart.textContent = '🔄 Reiniciar';
      btnRestart.addEventListener('click', async () => {
        btnRestart.disabled = true;
        try {
          await invoke('restart_plugin', { uuid: plugin.uuid });
          await loadPluginsList();
        } catch (err) {
          alert(`Erro ao reiniciar plugin: ${err}`);
        }
      });
      actions.appendChild(btnRestart);
    }

    if (!plugin.isBuiltin) {
      const btnFolder = document.createElement('button');
      btnFolder.className = 'btn secondary btn-sm';
      btnFolder.textContent = '📂 Pasta';
      btnFolder.addEventListener('click', async () => {
        try {
          await invoke('open_plugin_directory', { uuid: plugin.uuid });
        } catch (err) {
          alert(`Erro ao abrir pasta: ${err}`);
        }
      });
      actions.appendChild(btnFolder);

      const btnLogs = document.createElement('button');
      btnLogs.className = 'btn secondary btn-sm';
      btnLogs.textContent = '📋 Logs';
      actions.appendChild(btnLogs);

      const btnUninstall = document.createElement('button');
      btnUninstall.className = 'btn-icon danger';
      btnUninstall.title = 'Desinstalar Plugin';
      btnUninstall.textContent = '🗑️';
      btnUninstall.addEventListener('click', async () => {
        if (confirm(`Tem certeza que deseja desinstalar o plugin "${plugin.name}" (${plugin.uuid})?`)) {
          try {
            await invoke('uninstall_plugin', { uuid: plugin.uuid });
            await loadPluginsList();
            await loadCategories();
          } catch (err) {
            alert(`Erro ao desinstalar plugin: ${err}`);
          }
        }
      });
      actions.appendChild(btnUninstall);

      // Logs box
      const logsBox = document.createElement('pre');
      logsBox.className = 'plugin-logs-container';
      card.appendChild(logsBox);

      btnLogs.addEventListener('click', async () => {
        if (logsBox.style.display === 'block') {
          logsBox.style.display = 'none';
          btnLogs.textContent = '📋 Logs';
        } else {
          logsBox.style.display = 'block';
          logsBox.textContent = 'Carregando logs...';
          btnLogs.textContent = '📋 Ocultar Logs';
          try {
            const logs = await invoke('get_plugin_logs', { uuid: plugin.uuid });
            if (logs && logs.length > 0) {
              logsBox.textContent = logs.join('\n');
              logsBox.scrollTop = logsBox.scrollHeight;
            } else {
              logsBox.textContent = 'Nenhum log registrado para este plugin ainda.';
            }
          } catch (err) {
            logsBox.textContent = `Erro ao obter logs: ${err}`;
          }
        }
      });
    } else {
      const lockNote = document.createElement('span');
      lockNote.style.fontSize = '11px';
      lockNote.style.color = 'var(--text-muted)';
      lockNote.style.alignSelf = 'center';
      lockNote.textContent = '🔒 Pré-instalado (Não desinstalável)';
      lockNote.title = 'Plugins do aplicativo vêm pré-instalados e não podem ser excluídos, mas você pode desativá-los ou ativá-los a qualquer momento.';
      actions.appendChild(lockNote);
    }

    card.appendChild(actions);
    container.appendChild(card);
  });
}

use std::path::{Path, PathBuf};
use std::fs;
use std::sync::Arc;
use std::thread;
use log::{info, warn};
use tiny_http::{Header, Response, Server, StatusCode};

#[derive(Clone)]
pub struct AssetServer {
    port: u16,
    base_dir: Arc<PathBuf>,
}

impl AssetServer {
    pub fn new<P: AsRef<Path>>(port: u16, base_dir: P) -> Self {
        Self {
            port,
            base_dir: Arc::new(base_dir.as_ref().to_path_buf()),
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn start(&self) -> anyhow::Result<()> {
        let addr = format!("127.0.0.1:{}", self.port);
        let server = Server::http(&addr)
            .map_err(|e| anyhow::anyhow!("Failed to bind HTTP Asset Server to {}: {}", addr, e))?;
        info!("HTTP Asset Server listening on http://{}", addr);

        let base_dir = self.base_dir.clone();
        thread::spawn(move || {
            for request in server.incoming_requests() {
                let method = request.method().as_str();

                // CORS headers
                let cors_origin = Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap();
                let cors_methods = Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, POST, OPTIONS"[..]).unwrap();
                let cors_headers = Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"*"[..]).unwrap();

                if method == "OPTIONS" {
                    let mut resp = Response::empty(StatusCode(204));
                    resp.add_header(cors_origin);
                    resp.add_header(cors_methods);
                    resp.add_header(cors_headers);
                    let _ = request.respond(resp);
                    continue;
                }

                let raw_url = request.url();
                // Strip query string and decode URL encoding
                let path_part = raw_url.split('?').next().unwrap_or("/");
                let decoded_path = match urlencoding::decode(path_part) {
                    Ok(p) => p.into_owned(),
                    Err(_) => path_part.to_string(),
                };

                let relative_path = decoded_path.trim_start_matches('/');
                let maybe_target = find_asset_file(&base_dir, relative_path);

                let canonical_target = match maybe_target {
                    Some(p) => match fs::canonicalize(&p) {
                        Ok(can) => can,
                        Err(_) => {
                            let mut resp = Response::from_string("File Not Found")
                                .with_status_code(StatusCode(404));
                            resp.add_header(cors_origin);
                            let _ = request.respond(resp);
                            continue;
                        }
                    },
                    None => {
                        let mut resp = Response::from_string("File Not Found")
                            .with_status_code(StatusCode(404));
                        resp.add_header(cors_origin);
                        let _ = request.respond(resp);
                        continue;
                    }
                };

                let canonical_base = match fs::canonicalize(&*base_dir) {
                    Ok(p) => p,
                    Err(_) => (*base_dir).clone(),
                };

                // Prevent path traversal outside base_dir
                if !canonical_target.starts_with(&canonical_base) {
                    warn!("Path traversal attempt blocked: {:?}", canonical_target);
                    let mut resp = Response::from_string("Forbidden")
                        .with_status_code(StatusCode(403));
                    resp.add_header(cors_origin);
                    let _ = request.respond(resp);
                    continue;
                }

                // If path is a file, serve it
                if canonical_target.is_file() {
                    match fs::read(&canonical_target) {
                        Ok(bytes) => {
                            let mime = guess_mime_type(&canonical_target);
                            let final_bytes = if mime.starts_with("text/html") {
                                let raw_html = String::from_utf8_lossy(&bytes);
                                let bridge_script = r#"
<script>
    (function() {
        let currentSettings = {};

        // 0. Synchronously extract settings from URL search params (?info=...) if present
        try {
            const urlParams = new URLSearchParams(window.location.search);
            const infoParam = urlParams.get("info");
            if (infoParam) {
                const parsedInfo = JSON.parse(decodeURIComponent(infoParam));
                if (parsedInfo && parsedInfo.actionInfo && parsedInfo.actionInfo.payload && parsedInfo.actionInfo.payload.settings) {
                    Object.assign(currentSettings, parsedInfo.actionInfo.payload.settings);
                }
            }
        } catch(e) {}

        // Global settings store & form auto-restorer
        function restoreFormElements(settings) {
            if (!settings || typeof settings !== "object") return;
            for (const [key, val] of Object.entries(settings)) {
                if (val === undefined || val === null) continue;
                let el = document.getElementById(key) || document.querySelector(`[name="${key}"]`);
                if (el) {
                    applyValueToElement(el, key, val);
                } else {
                    const radios = document.querySelectorAll(`input[type="radio"][name="${key}"]`);
                    if (radios.length > 0) {
                        radios.forEach(r => { r.checked = (r.value === String(val)); });
                    }
                }
            }
        }

        function applyValueToElement(el, key, val) {
            if (el.tagName === "SELECT") {
                const strVal = String(val);
                const hasOpt = Array.from(el.options).some(o => o.value === strVal);
                if (hasOpt) {
                    if (el.value !== strVal) el.value = strVal;
                }
            } else if (el.tagName === "INPUT") {
                if (el.type === "checkbox") {
                    el.checked = (val === true || val === "true" || val === 1 || val === "1");
                } else if (el.type === "radio") {
                    const matchingRadio = document.querySelector(`input[type="radio"][name="${el.name || key}"][value="${val}"]`);
                    if (matchingRadio) matchingRadio.checked = true;
                } else {
                    el.value = String(val);
                    if (el.type === "range") {
                        const label = document.getElementById(key + "Label") || document.getElementById(key + "Value");
                        if (label) label.textContent = el.value + (label.textContent.includes("%") ? "%" : "");
                        try { el.dispatchEvent(new Event("input")); } catch(e) {}
                    }
                }
            } else if (el.tagName === "TEXTAREA") {
                el.value = String(val);
            }
        }

        // Observer for dynamically added options in <select> dropdowns (e.g. audio sinks, apps)
        function initMutationWatcher() {
            try {
                const observer = new MutationObserver((mutations) => {
                    for (const mutation of mutations) {
                        if (mutation.type === "childList") {
                            const target = mutation.target;
                            if (target && target.tagName === "SELECT") {
                                const key = target.id || target.name;
                                if (key && currentSettings[key] !== undefined) {
                                    const targetVal = String(currentSettings[key]);
                                    if (Array.from(target.options).some(o => o.value === targetVal)) {
                                        if (target.value !== targetVal) {
                                            target.value = targetVal;
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
                if (document.body) {
                    observer.observe(document.body, { childList: true, subtree: true });
                } else {
                    document.addEventListener("DOMContentLoaded", () => {
                        if (document.body) observer.observe(document.body, { childList: true, subtree: true });
                    });
                }
            } catch(e) {}
        }
        initMutationWatcher();

        function scheduleRestores() {
            restoreFormElements(currentSettings);
            [20, 60, 150, 300, 600, 1000].forEach(ms => {
                setTimeout(() => restoreFormElements(currentSettings), ms);
            });
        }

        if (document.readyState === "complete" || document.readyState === "interactive") {
            scheduleRestores();
        } else {
            document.addEventListener("DOMContentLoaded", scheduleRestores);
            window.addEventListener("load", scheduleRestores);
        }

        // 1. Monkey-patch WebSocket: rewrite ws://localhost: to ws://127.0.0.1:
        // and notify parent window of setSettings events + intercept incoming didReceiveSettings
        const OrigWebSocket = window.WebSocket;
        if (OrigWebSocket) {
            window.WebSocket = function(url, protocols) {
                if (typeof url === "string" && url.includes("ws://localhost:")) {
                    url = url.replace("ws://localhost:", "ws://127.0.0.1:");
                }
                const ws = protocols !== undefined ? new OrigWebSocket(url, protocols) : new OrigWebSocket(url);
                const origSend = ws.send.bind(ws);
                ws.send = function(data) {
                    try {
                        if (typeof data === "string") {
                            const parsed = JSON.parse(data);
                            if (parsed && (parsed.event === "setSettings" || parsed.event === "SetSettings")) {
                                if (parsed.payload && typeof parsed.payload === "object") {
                                    Object.assign(currentSettings, parsed.payload);
                                }
                                if (window.parent && window.parent !== window) {
                                    window.parent.postMessage({
                                        event: "setSettings",
                                        payload: parsed.payload,
                                        context: parsed.context
                                    }, "*");
                                }
                            }
                        }
                    } catch (e) {}
                    return origSend(data);
                };

                // Intercept incoming WebSocket messages
                function handleInbound(raw) {
                    try {
                        const parsed = typeof raw === "string" ? JSON.parse(raw) : raw;
                        if (parsed && parsed.event === "didReceiveSettings" && parsed.payload && parsed.payload.settings) {
                            Object.assign(currentSettings, parsed.payload.settings);
                            if (window.settings && typeof window.settings === "object") {
                                Object.assign(window.settings, parsed.payload.settings);
                            }
                            scheduleRestores();
                        }
                    } catch(e) {}
                }

                ws.addEventListener("message", function(evt) {
                    handleInbound(evt.data);
                });

                return ws;
            };
            window.WebSocket.prototype = OrigWebSocket.prototype;
            if (OrigWebSocket.CONNECTING !== undefined) {
                window.WebSocket.CONNECTING = OrigWebSocket.CONNECTING;
                window.WebSocket.OPEN = OrigWebSocket.OPEN;
                window.WebSocket.CLOSING = OrigWebSocket.CLOSING;
                window.WebSocket.CLOSED = OrigWebSocket.CLOSED;
            }
        }

        // 2. Cache connect payload and trigger connect callbacks as soon as they are defined
        let pendingConnectPayload = null;
        function tryConnect() {
            if (!pendingConnectPayload) return false;
            let connected = false;
            if (typeof window.connectOpenActionSocket === "function") {
                try {
                    window.connectOpenActionSocket(...pendingConnectPayload);
                    connected = true;
                } catch(e) { console.error("[PI Bridge] Error calling connectOpenActionSocket:", e); }
            } else if (typeof window.connectElgatoStreamDeckSocket === "function") {
                try {
                    window.connectElgatoStreamDeckSocket(...pendingConnectPayload);
                    connected = true;
                } catch(e) { console.error("[PI Bridge] Error calling connectElgatoStreamDeckSocket:", e); }
            }
            if (connected) {
                scheduleRestores();
            }
            return connected;
        }

        let _connectOAS = undefined;
        try {
            Object.defineProperty(window, "connectOpenActionSocket", {
                configurable: true,
                enumerable: true,
                get: () => _connectOAS,
                set: (fn) => {
                    _connectOAS = fn;
                    if (typeof fn === "function" && pendingConnectPayload) {
                        try { fn(...pendingConnectPayload); } catch(e) { console.error(e); }
                        scheduleRestores();
                    }
                }
            });
        } catch(e) {}

        let _connectESDS = undefined;
        try {
            Object.defineProperty(window, "connectElgatoStreamDeckSocket", {
                configurable: true,
                enumerable: true,
                get: () => _connectESDS,
                set: (fn) => {
                    _connectESDS = fn;
                    if (typeof fn === "function" && pendingConnectPayload) {
                        try { fn(...pendingConnectPayload); } catch(e) { console.error(e); }
                        scheduleRestores();
                    }
                }
            });
        } catch(e) {}

        window.addEventListener("message", (event) => {
            const data = event.data;
            if (!data) return;
            if (data.event === "connect" && Array.isArray(data.payload)) {
                pendingConnectPayload = data.payload;
                try {
                    if (pendingConnectPayload[4]) {
                        const actionInfo = typeof pendingConnectPayload[4] === "string" ? JSON.parse(pendingConnectPayload[4]) : pendingConnectPayload[4];
                        if (actionInfo && actionInfo.payload && actionInfo.payload.settings) {
                            Object.assign(currentSettings, actionInfo.payload.settings);
                            if (window.settings && typeof window.settings === "object") {
                                Object.assign(window.settings, actionInfo.payload.settings);
                            }
                        }
                    }
                } catch(e) {}
                tryConnect();
                scheduleRestores();
            } else if (data.event === "windowClosed") {
                const opendeck_iframe_container = document.getElementById("opendeck_iframe_container");
                if (opendeck_iframe_container && opendeck_iframe_container.firstElementChild) {
                    opendeck_iframe_container.firstElementChild.remove();
                }
                if (opendeck_iframe_container) {
                    opendeck_iframe_container.style.display = "none";
                }
            }
        });

        window.open = (url, target) => {
            if (target && !(target === "_self" || target === "_top")) {
                top.postMessage({ event: "openUrl", payload: url.startsWith("http") ? url : new URL(url, window.location.href).href }, "*");
                return null;
            }
            let iframe = document.createElement("iframe");
            iframe.style.flexGrow = "1";
            iframe.onload = () => {
                iframe.contentWindow.opener = window;
                iframe.contentWindow.onbeforeunload = () => top.postMessage({ event: "windowClosed", payload: window.name }, "*");
                iframe.contentWindow.close = () => { iframe.contentWindow.onbeforeunload(); iframe.remove(); };
                if (iframe.contentWindow.document && iframe.contentWindow.document.body) {
                    iframe.contentWindow.document.body.style.overflowY = "auto";
                }
            };
            iframe.src = url.startsWith("http") ? url : url;
            const opendeck_iframe_container = document.getElementById("opendeck_iframe_container");
            if (opendeck_iframe_container && opendeck_iframe_container.firstElementChild) {
                opendeck_iframe_container.firstElementChild.remove();
            }
            if (opendeck_iframe_container) {
                opendeck_iframe_container.appendChild(iframe);
                opendeck_iframe_container.style.display = "flex";
            }
            top.postMessage({ event: "windowOpened", payload: window.name }, "*");
            return iframe.contentWindow;
        };
    })();
</script>
<div id="opendeck_iframe_container" style="position: absolute; z-index: 100; top: 0; left: 0; width: 100%; height: 100%; display: none;"></div>
"#;
                                if let Some(idx) = raw_html.find("<head>") {
                                    let insert_pos = idx + "<head>".len();
                                    format!("{}{}{}", &raw_html[..insert_pos], bridge_script, &raw_html[insert_pos..]).into_bytes()
                                } else if let Some(idx) = raw_html.find("<HEAD>") {
                                    let insert_pos = idx + "<HEAD>".len();
                                    format!("{}{}{}", &raw_html[..insert_pos], bridge_script, &raw_html[insert_pos..]).into_bytes()
                                } else if let Some(idx) = raw_html.rfind("</body>") {
                                    format!("{}{}{}", &raw_html[..idx], bridge_script, &raw_html[idx..]).into_bytes()
                                } else if let Some(idx) = raw_html.rfind("</html>") {
                                    format!("{}{}{}", &raw_html[..idx], bridge_script, &raw_html[idx..]).into_bytes()
                                } else {
                                    format!("{}{}", bridge_script, raw_html).into_bytes()
                                }
                            } else {
                                bytes
                            };

                            let content_type = Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).unwrap();
                            let mut resp = Response::from_data(final_bytes)
                                .with_status_code(StatusCode(200));
                            resp.add_header(content_type);
                            resp.add_header(cors_origin);
                            let _ = request.respond(resp);
                        }
                        Err(e) => {
                            let mut resp = Response::from_string(format!("Error reading file: {}", e))
                                .with_status_code(StatusCode(500));
                            resp.add_header(cors_origin);
                            let _ = request.respond(resp);
                        }
                    }
                } else {
                    let mut resp = Response::from_string("Not Found")
                        .with_status_code(StatusCode(404));
                    resp.add_header(cors_origin);
                    let _ = request.respond(resp);
                }
            }
        });

        Ok(())
    }
}

fn guess_mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}

fn find_asset_file(base_dir: &Path, relative: &str) -> Option<PathBuf> {
    let norm = relative.trim_start_matches('/');
    let mut candidates = Vec::new();

    let parts: Vec<&str> = norm.split('/').collect();
    if parts.len() >= 2 && parts[0] == "plugins" {
        let uuid = parts[1];
        let rest = parts[2..].join("/");
        candidates.push(base_dir.join("plugins").join(format!("{}.sdPlugin", uuid)).join(&rest));
        candidates.push(base_dir.join(format!("{}.sdPlugin", uuid)).join(&rest));
        candidates.push(base_dir.join("plugins").join(uuid).join(&rest));
    } else if parts.len() >= 1 {
        let uuid = parts[0];
        let rest = parts[1..].join("/");
        candidates.push(base_dir.join("plugins").join(format!("{}.sdPlugin", uuid)).join(&rest));
        candidates.push(base_dir.join(format!("{}.sdPlugin", uuid)).join(&rest));
    }

    candidates.push(base_dir.join(norm));
    candidates.push(base_dir.join("plugins").join(norm));

    for cand in &candidates {
        if cand.is_file() {
            return Some(cand.clone());
        }
        for ext in &[".png", "@2x.png", ".svg", ".html", ".js", ".css", ".json"] {
            let cand_str = cand.to_string_lossy();
            let custom = PathBuf::from(format!("{}{}", cand_str, ext));
            if custom.is_file() {
                return Some(custom);
            }
        }
    }

    None
}

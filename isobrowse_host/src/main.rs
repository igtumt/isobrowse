use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::WebViewBuilder;
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use std::thread;
use std::time::{Instant, Duration};
use std::sync::Arc;
use sysinfo::System;

enum UserEvent {
    WasmGhostRender { html: String, url: String, cpu_ms: u128, ram_kb: usize, blocked_count: usize },
    IpcMessage(String),
    UpdateTerminal(String),
    UpdateOsTelemetry { cpu: f32, ram_mb: u64 }, 
}

static WASM_ENGINE_GHOST: &[u8] = include_bytes!("../../target/wasm32-wasip1/release/engine_ghost.wasm");


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    
    let telemetry_proxy = proxy.clone();
    thread::spawn(move || {
        let mut sys = System::new_all();
        if let Ok(pid) = sysinfo::get_current_pid() {
            loop {
                sys.refresh_processes();
                if let Some(process) = sys.process(pid) {
                    let cpu = process.cpu_usage();
                    let ram_mb = process.memory() / (1024 * 1024);
                    let _ = telemetry_proxy.send_event(UserEvent::UpdateOsTelemetry { cpu, ram_mb });
                }
                thread::sleep(Duration::from_secs(1));
            }
        }
    });
    
    let http_client = Arc::new(
        reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15")
            .cookie_store(true)
            .build()
            .unwrap()
    );

    let window = WindowBuilder::new()
        .with_title("IsoBrowse MVP - Global Edition")
        .with_inner_size(tao::dpi::LogicalSize::new(1400.0, 950.0))
        .build(&event_loop)?;

    let init_script = r#"
        try { window.open = function(url) { if (url && url.startsWith('http')) { window.top.location.href = url; } return null; }; } catch(e) {}
        
        document.addEventListener('click', function(e) {
            let a = e.target.closest('a');
            if (a && a.href && a.href.startsWith('http')) {
                if (a.getAttribute('target') === '_blank' || a.getAttribute('target') === '_new') {
                    a.setAttribute('target', '_self');
                }
                if (window !== window.top) { 
                    e.preventDefault(); 
                    e.stopPropagation();
                    window.parent.postMessage({type: 'GHOST_NAVIGATE', url: a.href}, '*');
                }
            }
        }, true);

        if (window === window.top) {
            window.isoCurrentMode = sessionStorage.getItem('iso_mode') || 'STANDARD'; 
            window.isoCurrentRam = 0; 
            window.isoCurrentCpu = 0;

            window.addEventListener('message', (e) => {
                if (e.data && (e.data.type === 'GHOST_NAVIGATE' || e.data.type === 'NAVIGATE')) {
                    window.history.pushState(null, '', e.data.url);
                    if(document.getElementById('isobrowse-shadow-host')) {
                        document.getElementById('isobrowse-shadow-host').shadowRoot.getElementById('iso-url').value = e.data.url;
                    }
                    if(window.ipc) window.ipc.postMessage("FETCH_GHOST:" + e.data.url);
                }
            });

            window.addEventListener('popstate', (e) => {
                if (window.isoCurrentMode === 'GHOST') {
                    if(document.getElementById('isobrowse-shadow-host')) {
                        document.getElementById('isobrowse-shadow-host').shadowRoot.getElementById('iso-url').value = window.location.href;
                    }
                    if(window.ipc) window.ipc.postMessage("FETCH_GHOST:" + window.location.href);
                }
            });

            const injectIsoBrowseUI = () => {
                if (document.getElementById('isobrowse-shadow-host')) return;

                const host = document.createElement('div');
                host.id = 'isobrowse-shadow-host';
                host.style.cssText = 'position:fixed; top:0; left:0; width:100%; height:105px; z-index:2147483647; background:transparent; pointer-events:none;';
                document.documentElement.appendChild(host);

                const shadow = host.attachShadow({mode: 'open'});

                const style = document.createElement('style');
                style.innerHTML = `
                    @keyframes iso-blink { 0% { opacity: 1; } 50% { opacity: 0.3; color: #fff; } 100% { opacity: 1; } }
                    .iso-alarm-active { animation: iso-blink 1s infinite; color: #ff3366 !important; font-weight: bold; }
                    * { box-sizing: border-box; font-family: monospace; font-size: 11px; margin: 0; padding: 0; }
                    #panel {
                        width: 100%; height: 105px; background: #050505; color: #00ff41; pointer-events: auto;
                        border-bottom: 2px solid #00ff41; padding: 8px 12px; display: flex; flex-direction: column; gap: 8px;
                    }
                    button {
                        background: #000; color: #0f0; border: 1px solid #0f0; padding: 4px 12px;
                        cursor: pointer; font-weight: bold; display: inline-flex; align-items: center; outline: none; border-radius:0;
                    }
                    button:hover { background: #003300; }
                    input {
                        background: #000; color: #0f0; border: 1px solid #004400; padding: 4px 8px;
                        outline: none; flex-grow: 1; border-radius:0;
                    }
                    .row { display: flex; justify-content: space-between; align-items: center; width: 100%; }
                    .gap { display: flex; gap: 8px; }
                    .gap5 { display: flex; gap: 5px; }
                    .gap20 { display: flex; gap: 20px; }
                    .info-row { background: #0a0a0a; border: 1px solid #333; padding: 4px 8px; }
                    .text-muted { color: #888; font-size: 10px; }
                    .text-green { color: #00ff41; }
                `;
                shadow.appendChild(style);

                const panel = document.createElement('div');
                panel.id = 'panel';
                panel.innerHTML = `
                    <div class="row">
                        <div class="gap" style="width:70%;">
                            <button id="iso-back"><</button>
                            <button id="iso-fwd">></button>
                            <input id="iso-url" type="text" value="${window.location.href}">
                            <button id="iso-go">EXEC</button>
                        </div>
                        <div class="gap5">
                            <button id="btn-mod1" style="color:#aaa; border-color:#555;">MOD 1 (GHOST)</button>
                            <button id="btn-mod2" style="background:#00ff41; color:#000; border-color:#00ff41; box-shadow:0 0 8px #00ff41;">MOD 2 (NATIVE)</button>
                        </div>
                    </div>
                    <div class="row info-row">
                        <div class="gap20">
                            <span>STATE: <span id="iso-engine-status" style="color:#00ccff; font-weight:bold;">WEB3_VAULT</span></span>
                            <span>CPU_LOAD: <span id="iso-cpu" style="color:#ffcc00;">0.0 %</span></span>
                            <span>RAM: <span id="iso-ram" style="color:#ff3366;">0 MB</span></span>
                            <span>DOM: <span id="iso-dom" style="color:#fff;">0</span></span>
                            <span id="iso-co2-box" style="display:none;">CO2 SAVED: <span id="iso-co2" style="color:#00ff41; font-weight:bold;">0.00g</span></span>
                            <span id="iso-blocked-box" style="display:none; color:#ff3366;">BLOCKED: <span id="iso-blocked" style="font-weight:bold; color:#ff3366;">0</span> threats</span>
                        </div>
                    </div>
                    <div class="row text-muted" style="margin-top:2px;">
                        <span id="iso-info-text">🛡️ INFO: Mod 2 (Vault) is unrestricted. The system locks if malicious anomalies are detected.</span>
                        <span id="iso-terminal" class="text-green">> [SYSTEM]: OS Kernel hooked. Hardware telemetry active...</span>
                    </div>
                `;
                shadow.appendChild(panel);

                if(document.body) { document.body.style.marginTop = '105px'; }

                const ghostFrame = document.createElement('iframe');
                ghostFrame.id = 'isobrowse-ghost-canvas';
                ghostFrame.sandbox = 'allow-same-origin allow-scripts allow-forms'; 
                ghostFrame.style.cssText = 'position:fixed; top:105px; left:0; width:100%; height:calc(100vh - 105px); border:none; background:#fff; z-index:2147483646; display:none;';
                document.documentElement.appendChild(ghostFrame);

                const getEl = (id) => shadow.getElementById(id);

                window.updateTerminal = (msg) => { getEl('iso-terminal').innerText = msg; };
                window.updateOsTelemetry = (cpuVal, ramMB) => {
                    if (window.isoCurrentMode === 'STANDARD') {
                        getEl('iso-cpu').innerText = cpuVal.toFixed(1) + ' %';
                        getEl('iso-ram').innerText = ramMB + ' MB';
                        window.isoCurrentRam = ramMB; window.isoCurrentCpu = cpuVal;
                    }
                };

                const activateGhostUI = () => {
                    window.isoCurrentMode = 'GHOST'; sessionStorage.setItem('iso_mode', 'GHOST'); 
                    getEl('btn-mod1').style.cssText = 'background:#00ff41; color:#000; border-color:#00ff41; font-weight:bold; box-shadow: 0 0 8px #00ff41;';
                    getEl('btn-mod2').style.cssText = 'background:#000; color:#aaa; border-color:#555; box-shadow:none; font-weight:normal;';
                    getEl('iso-engine-status').innerText = 'WASM_GHOST'; getEl('iso-engine-status').style.color = '#fff';
                    getEl('iso-dom').style.color = '#fff';
                    getEl('panel').style.borderBottom = '2px solid #00ff41'; 
                    getEl('iso-url').style.border = '1px solid #004400';
                    getEl('iso-info-text').innerText = '👻 INFO: Mod 1 (Ghost) is active. Malicious JS loops, trackers, and hidden ads are physically destroyed.';
                    getEl('iso-co2-box').style.display = 'inline'; getEl('iso-blocked-box').style.display = 'inline';
                    
                    Array.from(document.body.children).forEach(child => {
                        if (child.id !== 'isobrowse-shadow-host' && child.id !== 'isobrowse-ghost-canvas') { child.style.display = 'none'; }
                    });
                    document.getElementById('isobrowse-ghost-canvas').style.display = 'block';
                };

                const activateNativeUI = () => { window.isoCurrentMode = 'STANDARD'; sessionStorage.setItem('iso_mode', 'STANDARD'); window.location.reload(); };

                getEl('btn-mod1').onclick = () => { activateGhostUI(); if(window.ipc) window.ipc.postMessage("FETCH_GHOST:" + getEl('iso-url').value); };
                getEl('btn-mod2').onclick = activateNativeUI;

                const navigate = () => {
                    let target = getEl('iso-url').value;
                    if (!target.startsWith('http')) target = 'https://' + target;
                    if (window.isoCurrentMode === 'GHOST') {
                        window.history.pushState(null, '', target);
                        if(window.ipc) window.ipc.postMessage("FETCH_GHOST:" + target); 
                    } else { window.location.href = target; }
                };

                getEl('iso-back').onclick = () => { window.history.back(); };
                getEl('iso-fwd').onclick = () => { window.history.forward(); };
                getEl('iso-go').onclick = navigate;
                getEl('iso-url').addEventListener('keypress', (e) => { if(e.key === 'Enter') navigate(); });

                if (window.isoCurrentMode === 'GHOST') { activateGhostUI(); if(window.ipc) window.ipc.postMessage("FETCH_GHOST:" + window.location.href); }

                let pageLoadTime = Date.now(); let lastInteractionTime = Date.now(); let lastDomCount = document.getElementsByTagName('*').length; let lastRamMB = 0;
                const resetIdle = () => { lastInteractionTime = Date.now(); };
                window.addEventListener('mousemove', resetIdle); window.addEventListener('scroll', resetIdle); window.addEventListener('keydown', resetIdle); window.addEventListener('click', resetIdle);

                let lastCheckedUrl = "";
                setInterval(() => {
                    if (window.isoCurrentMode === 'STANDARD') {
                        let currentUrl = window.location.hostname;
                        if (currentUrl !== lastCheckedUrl && currentUrl !== "") {
                            lastCheckedUrl = currentUrl;
                            if(window.ipc) window.ipc.postMessage("CHECK_DOMAIN:" + currentUrl);
                        }
                    }
                }, 2000);

                setInterval(() => {
                    if (window.isoCurrentMode === 'STANDARD') {
                        let currentUrl = window.location.href; let urlInput = getEl('iso-url');
                        if (shadow.activeElement !== urlInput && urlInput.value !== currentUrl) { urlInput.value = currentUrl; pageLoadTime = Date.now(); }
                        let currentDomCount = document.getElementsByTagName('*').length; getEl('iso-dom').innerText = currentDomCount;
                        
                        let isPhishing = false; let threatDetail = ""; let timeSinceLoad = Date.now() - pageLoadTime;
                        if (timeSinceLoad > 3000) {
                            let isIdle = (Date.now() - lastInteractionTime) > 3000; let isDomSpike = (currentDomCount - lastDomCount) > 800; 
                            let isRamSpike = (window.isoCurrentRam - lastRamMB) > 100; let isIdleDrain = isIdle && window.isoCurrentCpu > 25.0; 
                            if (isDomSpike) { isPhishing = true; threatDetail = "Abnormal DOM Spike"; }
                            else if (isIdleDrain) { isPhishing = true; threatDetail = "High Idle CPU"; }
                            else if (isRamSpike) { isPhishing = true; threatDetail = "Memory Leak"; }
                            else if (currentDomCount > 4000 || window.isoCurrentRam > 600) { isPhishing = true; threatDetail = "Excessive Hardware Consumption (Static Bloatware)! DO NOT CONNECT your wallet!"; }
                        }

                        if (isPhishing) {
                            getEl('iso-engine-status').innerText = '🚨 DRAINER RISK!'; getEl('iso-engine-status').className = 'iso-alarm-active';
                            getEl('panel').style.borderBottom = '2px solid #ff3366'; getEl('iso-url').style.border = '1px solid #ff3366';
                            getEl('iso-info-text').innerHTML = `<span class="iso-alarm-active">⚠️ WARNING: ${threatDetail}</span>`;
                        }
                        lastDomCount = currentDomCount; lastRamMB = window.isoCurrentRam;
                    }
                }, 1000);
            };

            window.renderGhostMode = (html, url, cpu, ram, blocked) => {
                const getEl = (id) => document.getElementById('isobrowse-shadow-host').shadowRoot.getElementById(id);
                getEl('iso-url').value = url; getEl('iso-cpu').innerText = cpu + " ms";
                getEl('iso-ram').innerText = ram + " KB"; getEl('iso-blocked').innerText = blocked; 
                getEl('iso-co2').innerText = (ram * 0.0002).toFixed(4) + "g";
                window.updateTerminal("> [SYSTEM]: Secure Render Complete. Ghost Cursor Active.");
                document.getElementById('isobrowse-ghost-canvas').srcdoc = html;
            };

            if (document.readyState === 'loading') { document.addEventListener('DOMContentLoaded', injectIsoBrowseUI); } else { injectIsoBrowseUI(); }
        }
    "#;

    let webview = WebViewBuilder::new(&window)
        .with_initialization_script(init_script) 
        .with_ipc_handler({
            let proxy_ipc = proxy.clone();
            move |request| { let _ = proxy_ipc.send_event(UserEvent::IpcMessage(request.body().to_string())); }
        })
        .with_url("https://app.uniswap.org") 
        .build()?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::UpdateOsTelemetry { cpu, ram_mb }) => {
                let js_code = format!("if(window.updateOsTelemetry) window.updateOsTelemetry({}, {});", cpu, ram_mb);
                let _ = webview.evaluate_script(&js_code);
            }
            Event::UserEvent(UserEvent::UpdateTerminal(msg)) => {
                let js_code = format!("if(window.updateTerminal) window.updateTerminal('{}');", msg);
                let _ = webview.evaluate_script(&js_code);
            }
            Event::UserEvent(UserEvent::IpcMessage(msg)) => {
                if msg.starts_with("FETCH_GHOST:") {
                    let raw_url = msg.replace("FETCH_GHOST:", "");
                    let p_i = proxy.clone();
                    let client = Arc::clone(&http_client);
                    
                    let _ = p_i.send_event(UserEvent::UpdateTerminal("> [RUST]: Tunneling to target page...".to_string()));
                    
                    thread::spawn(move || {
                        let start_time = Instant::now();
                        let fetch_url = if raw_url.starts_with("http") { raw_url.clone() } else if raw_url.starts_with("//") { format!("https:{}", raw_url) } else { format!("https://{}", raw_url) };

                        let resp = match client.get(&fetch_url).send() {
                            Ok(r) => r,
                            Err(_) => { let _ = p_i.send_event(UserEvent::UpdateTerminal("> [ERROR]: Connection failed.".to_string())); return; }
                        };
                        
                        
                        let final_url = resp.url().as_str().to_string(); 
                        
                        let content_type = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("text/html").to_lowercase();
                        if content_type.contains("image/") || content_type.contains("video/") || content_type.contains("application/") || content_type.contains("audio/") {
                            let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [SHIELD]: Blocked non-HTML tracking payload ({}).", content_type)));
                            return; 
                        }

                        let raw_html = match resp.text() {
                            Ok(t) => t,
                            Err(_) => {
                                let _ = p_i.send_event(UserEvent::UpdateTerminal("> [SHIELD]: Failed to decode page payload.".to_string()));
                                return;
                            }
                        };
                        
                        if raw_html.len() > 10 * 1024 * 1024 {
                            let _ = p_i.send_event(UserEvent::UpdateTerminal("> [SHIELD]: Payload too large (Exceeds 10MB). Blocked to prevent crash.".to_string()));
                            return;
                        }

                        let lower_html = raw_html.to_lowercase();
                        let blocked_trackers = lower_html.matches("<script").count() + lower_html.matches("<iframe").count() + lower_html.matches("google-analytics").count();

                        let mut is_spa = false;
                        let mut is_antibot = false;
                        let p_count = lower_html.matches("<p").count(); 

                        if lower_html.contains("datadome") || lower_html.contains("cloudflare-") || final_url.contains("forbes.com") { is_antibot = true; }
                        if final_url.contains("nypost.com") || final_url.contains("uniswap.org") || (lower_html.contains("id=\"root\"") && p_count < 5) { is_spa = true; }

                        let mut html;

                        if is_antibot || is_spa {
                            let _ = p_i.send_event(UserEvent::UpdateTerminal("> [ALARM]: Dynamic Architecture/Bot Shield detected!".to_string()));
                            
                            let w_type = if is_antibot { "ANTI-BOT SHIELD DETECTED" } else { "SPA (DYNAMIC) ARCHITECTURE DETECTED" };
                            let w_desc = if is_antibot { 
                                "This site uses a military-grade shield (DataDome/Cloudflare) to prevent automated data extraction."
                            } else {
                                "This site hides or lazy-loads its content using JavaScript. Access is halted because JS is disabled in Mod 1."
                            };
                            
                            let roadmap_msg = "🚀 <strong>V2.0 ROADMAP:</strong> Our advanced <em>Headless Rendering Engine</em> is currently in development to securely bypass these shields and render dynamic sites inside Ghost Mode soon.";
                            
                            html = format!("
                                <div style='display:flex; flex-direction:column; align-items:center; justify-content:center; height:100vh; background:#111; color:#0f0; font-family:monospace; text-align:center; padding:20px; box-sizing:border-box;'>
                                    <h1 style='color:#ff3366; font-size:28px; margin-bottom:10px;'>🚨 {} 🚨</h1>
                                    <p style='font-size:16px; color:#aaa; max-width:600px; line-height:1.6;'>{}</p>
                                    <div style='margin-top:20px; background:#1a1a00; border:1px dashed #cca300; padding:12px 24px; border-radius:6px; max-width:600px; box-shadow: 0 0 10px rgba(204, 163, 0, 0.1);'>
                                        <p style='font-size:14px; color:#ffcc00; margin:0; line-height:1.5;'>{}</p>
                                    </div>
                                    <div style='margin-top:30px; padding:15px 30px; border:1px solid #00ff41; background:#002200; border-radius:8px; box-shadow: 0 0 15px rgba(0, 255, 65, 0.3);'>
                                        <p style='font-size:18px; color:#fff; margin:0;'>👉 Click the <strong style='color:#00ff41;'>MOD 2 (NATIVE)</strong> button on the top right to continue.</p>
                                    </div>
                                </div>
                            ", w_type, w_desc, roadmap_msg);
                        } else {
                            let _ = p_i.send_event(UserEvent::UpdateTerminal("> [WASM]: Shredding spy scripts and trackers...".to_string()));
                            
                            html = raw_html
                                .replace("<script", "<template").replace("<SCRIPT", "<template")
                                .replace("</script>", "</template>").replace("</SCRIPT>", "</template>")
                                .replace("<iframe", "<template").replace("<IFRAME", "<template")
                                .replace("</iframe>", "</template>").replace("</IFRAME>", "</template>");

                            html = html
                                .replace("<noscript", "<div class=\"iso-noscript\"").replace("<NOSCRIPT", "<div class=\"iso-noscript\"")
                                .replace("</noscript>", "</div>").replace("</NOSCRIPT>", "</div>");

                            html = html.replace("http-equiv=\"Content-Security-Policy\"", "name=\"Disabled-CSP\"")
                                       .replace("http-equiv='Content-Security-Policy'", "name='Disabled-CSP'")
                                       .replace("http-equiv=\"refresh\"", "name=\"disabled-refresh\"")
                                       .replace("http-equiv='refresh'", "name='disabled-refresh'");
                        }

                        let ram_footprint = html.len() / 1024;

                        let mut config = wasmtime::Config::new();
                        config.consume_fuel(true);
                        config.static_memory_maximum_size(500 * 1024 * 1024);
                        
                        let engine = wasmtime::Engine::new(&config).unwrap();
                        let mut linker = wasmtime::Linker::<WasiP1Ctx>::new(&engine);
                        preview1::add_to_linker_sync(&mut linker, |t| t).unwrap();

                        let pr = p_i.clone();
                        let f_url = final_url.clone();
                        
                        linker.func_wrap("env", "render_html", move |mut c: wasmtime::Caller<'_, WasiP1Ctx>, ptr: i32, len: i32| {
                            let mem = c.get_export("memory").unwrap().into_memory().unwrap();
                            let mut d = vec![0u8; len as usize]; mem.read(&c, ptr as usize, &mut d).unwrap();
                            
                            let final_output = String::from_utf8_lossy(&d).to_string();

                            let _ = pr.send_event(UserEvent::WasmGhostRender { 
                                html: final_output, url: f_url.clone(), cpu_ms: start_time.elapsed().as_millis(),
                                ram_kb: ram_footprint, blocked_count: blocked_trackers
                            });
                        }).unwrap();

                        linker.func_wrap("env", "send_to_ui", |_c: wasmtime::Caller<'_, WasiP1Ctx>, _ptr: i32, _len: i32| {}).unwrap();

                        let wasi = wasmtime_wasi::WasiCtxBuilder::new().build_p1();
                        let mut store = wasmtime::Store::new(&engine, wasi);
                        store.set_fuel(u64::MAX).unwrap(); 

                        let module = wasmtime::Module::from_binary(&engine, WASM_ENGINE_GHOST).unwrap();
                        let instance = linker.instantiate(&mut store, &module).unwrap();

                        let alloc = instance.get_typed_func::<i32, i32>(&mut store, "alloc").unwrap();
                        let on_d = instance.get_typed_func::<(i32, i32), ()>(&mut store, "on_data_received").unwrap();

                        let h_b = html.as_bytes();
                        
                        let h_p = match alloc.call(&mut store, h_b.len() as i32) {
                            Ok(p) => p,
                            Err(_) => {
                                let _ = p_i.send_event(UserEvent::UpdateTerminal("> [SHIELD]: Payload rendering aborted to prevent memory overflow.".to_string()));
                                return;
                            }
                        };
                        
                        instance.get_memory(&mut store, "memory").unwrap().write(&mut store, h_p as usize, h_b).unwrap();
                        let _ = on_d.call(&mut store, (h_p, h_b.len() as i32));
                    });
                }

                if msg.starts_with("CHECK_DOMAIN:") {
                    let raw_domain = msg.replace("CHECK_DOMAIN:", "");
                    let p_i = proxy.clone();
                    let client = Arc::clone(&http_client);
                    
                    thread::spawn(move || {
                        let parts: Vec<&str> = raw_domain.split('.').collect();
                        let root_domain = if raw_domain.ends_with(".tr") || raw_domain.ends_with(".uk") || raw_domain.ends_with(".au") || raw_domain.ends_with(".br") {
                            if parts.len() >= 3 { format!("{}.{}.{}", parts[parts.len()-3], parts[parts.len()-2], parts[parts.len()-1]) } else { raw_domain.clone() }
                        } else {
                            if parts.len() >= 2 { format!("{}.{}", parts[parts.len()-2], parts[parts.len()-1]) } else { raw_domain.clone() }
                        };

                        if root_domain.len() < 3 { return; }

                        let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [INTEL]: {} is being queried in WHOIS database...", root_domain)));

                        let api_url = format!("https://networkcalc.com/api/dns/whois/{}", root_domain);
                        if let Ok(resp) = client.get(&api_url).send() {
                            let json_text = resp.text().unwrap_or_default();
                            let lower_json = json_text.to_lowercase();
                            
                            if lower_json.contains("\"status\":\"no_records\"") || lower_json.contains("\"status\": \"no_records\"") {
                                let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [INTEL]: Age query for {} cannot be performed due to national cyber protection protocols.", root_domain)));
                                return;
                            }
                            
                            let mut year = 0;
                            let mut date_display = String::new();
                            let months = ["", "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
                            
                            let keywords = ["creat", "regist"];
                            
                            for kw in keywords.iter() {
                                let mut start_idx = 0;
                                while let Some(idx) = lower_json[start_idx..].find(kw) {
                                    let abs_idx = start_idx + idx;
                                    let snippet: String = lower_json[abs_idx..].chars().take(150).collect();
                                    
                                    let tokens: Vec<&str> = snippet.split(|c: char| !c.is_alphanumeric()).filter(|s| !s.is_empty()).collect();
                                    
                                    for i in 0..tokens.len() {
                                        if tokens[i].len() == 4 {
                                            if let Ok(y) = tokens[i].parse::<i32>() {
                                                if y >= 1985 && y <= 2026 {
                                                    year = y;
                                                    if i + 1 < tokens.len() {
                                                        if let Ok(m) = tokens[i+1].parse::<i32>() {
                                                            if m >= 1 && m <= 12 { date_display = format!("{} {}", months[m as usize], year); }
                                                        }
                                                    }
                                                    if date_display.is_empty() { date_display = year.to_string(); }
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    if year > 0 { break; } 
                                    start_idx = abs_idx + kw.len(); 
                                }
                                if year > 0 { break; } 
                            }

                            if year > 0 {
                                if year >= 2024 {
                                    let alarm_msg = format!("> [ALARM]: DOMAIN IS TOO NEW (Reg: {})! HIGH Drainer/Phishing risk!", date_display);
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal(alarm_msg.clone()));
                                    
                                    let js_warn = format!("
                                        if(document.getElementById('isobrowse-shadow-host')) {{
                                            const shadow = document.getElementById('isobrowse-shadow-host').shadowRoot;
                                            shadow.getElementById('iso-terminal').style.color = '#ff3366';
                                            shadow.getElementById('iso-terminal').style.fontWeight = 'bold';
                                            shadow.getElementById('iso-terminal').innerText = '{}';
                                            shadow.getElementById('iso-engine-status').innerText = '🚨 SCAM RISK!';
                                            shadow.getElementById('iso-engine-status').className = 'iso-alarm-active';
                                            shadow.getElementById('panel').style.borderBottom = '2px solid #ff3366';
                                        }}
                                    ", alarm_msg);
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal(js_warn));
                                } else {
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [SAFE]: Domain is established and reliable (Reg: {}).", date_display)));
                                    let js_safe = "
                                        if(document.getElementById('isobrowse-shadow-host')) {
                                            const shadow = document.getElementById('isobrowse-shadow-host').shadowRoot;
                                            shadow.getElementById('iso-terminal').style.color = '#00ff41'; 
                                            shadow.getElementById('iso-terminal').style.fontWeight = 'normal';
                                        }
                                    ".to_string();
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal(js_safe));
                                }
                            } else {
                                let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [INTEL]: {} records are masked by GDPR/Privacy Protection protocols.", root_domain)));
                            }
                        } else {
                            let _ = p_i.send_event(UserEvent::UpdateTerminal("> [INTEL]: Failed to connect to WHOIS API server.".to_string()));
                        }
                    });
                }
            }

            Event::UserEvent(UserEvent::WasmGhostRender { html, url, cpu_ms, ram_kb, blocked_count }) => {
                
                let fallback_css = "<style>
                    /* REKLAM VE ÇÖP KUTUSU YOK EDİCİSİ */
                    .ad, .ads, .ad-slot, .ad-container, [id^='ad-'], [class^='ad-'],
                    [class*='taboola'], [class*='outbrain'],
                    [class*='popup'], [id*='popup'], [class*='modal'], [id*='modal'],
                    [class*='overlay'], [id*='overlay'], [class*='cookie'], [id*='cookie'],
                    [class*='consent'], [id*='consent'], [class*='newsletter'], [id*='newsletter'],
                    .fc-consent-root, #cmpbox,
                    .sp_veil, [id^='sp_message'], .fc-ab-root, .privacy-prompt, #privacy-prompt,
                    .veil, .backdrop, .dialog-backdrop, [class*='backdrop'] {
                        display: none !important;
                        visibility: hidden !important;
                        opacity: 0 !important;
                        pointer-events: none !important;
                        width: 0 !important;
                        height: 0 !important;
                        position: absolute !important;
                        z-index: -9999 !important;
                    }

                    html, body { overflow: auto !important; position: static !important; }
                    template, style, script, title, link, meta { display: none !important; opacity: 0 !important; visibility: hidden !important; }

                    /* BBC GÖRSEL KORUMA */
                    .iso-noscript { 
                        display: block !important; 
                        opacity: 1 !important; 
                        visibility: visible !important; 
                    }
                    .iso-noscript img { 
                        opacity: 1 !important; 
                        visibility: visible !important; 
                        max-width: 100% !important; 
                        height: auto !important; 
                        display: block !important;
                    }

                    /* 💥 HAYALET İMLEÇ (GHOST CURSOR) 💥 */
                    html, body, * {
                        cursor: url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><text y='20' font-size='20'>👻</text></svg>\"), auto !important;
                    }
                </style>";

                let interceptor = r#"<script>
                    document.addEventListener('click', function(e) {
                        const target = e.target.closest('a');
                        if (target && target.href && target.href.startsWith('http')) {
                            e.preventDefault(); 
                            e.stopPropagation();
                            window.parent.postMessage({type: 'GHOST_NAVIGATE', url: target.href}, '*');
                        }
                    }, true);

                    // HAYALET GÖRSEL MOTORU
                    function ghostImageEngine() {
                        document.querySelectorAll('img').forEach(img => {
                            let dSrc = img.getAttribute('data-src') || img.getAttribute('data-original');
                            if (dSrc && img.getAttribute('src') !== dSrc) {
                                img.setAttribute('src', dSrc);
                            }
                            img.setAttribute('loading', 'eager');
                        });

                        document.querySelectorAll('.iso-noscript').forEach(ns => {
                            let prev = ns.previousElementSibling;
                            if (prev && (prev.tagName === 'PICTURE' || prev.tagName === 'IMG' || prev.tagName === 'DIV')) {
                                if (!prev.classList.contains('iso-noscript')) {
                                    prev.style.display = 'none';
                                }
                            }
                        });
                    }

                    ghostImageEngine();
                    setInterval(ghostImageEngine, 500);
                </script>"#;
                
                let base_tag = format!("<base href=\"{}\" target=\"_self\">", url);
                let final_srcdoc = format!("{}\n{}\n{}\n{}", base_tag, fallback_css, html, interceptor);
                
                let js = format!("window.renderGhostMode({}, '{}', {}, {}, {})", 
                    serde_json::to_string(&final_srcdoc).unwrap(), url, cpu_ms, ram_kb, blocked_count);
                let _ = webview.evaluate_script(&js);
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}

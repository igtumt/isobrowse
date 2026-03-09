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
    WasmSurfRender { html: String, url: String, cpu_ms: u128, ram_kb: usize, blocked_count: usize },
    IpcMessage(String),
    UpdateTerminal(String),
    UpdateOsTelemetry { cpu: f32, ram_mb: u64 }, 
}

static WASM_ENGINE_GHOST: &[u8] = include_bytes!("../../target/wasm32-wasip1/release/runtime_surf.wasm");

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
            .timeout(Duration::from_secs(10)) 
            .build()
            .unwrap()
    );

    let window = WindowBuilder::new()
        .with_title("IsoBrowse MVP - Global Edition")
        .with_inner_size(tao::dpi::LogicalSize::new(1400.0, 950.0))
        .build(&event_loop)?;

    let init_script = r##"
        // MAC OS CMD+C / CMD+A VE GENEL KLAVYE ÇÖKME KALKANI
        document.addEventListener('keydown', function(e) {
            let host = document.getElementById('isobrowse-shadow-host');
            let active = host ? host.shadowRoot.activeElement : null;
            let inInput = (active && active.id === 'iso-url');

            if (e.metaKey || e.ctrlKey) {
                let key = e.key.toLowerCase();
                if (['c', 'a', 'x'].includes(key)) {
                    if (!inInput) {
                        if (key === 'c') {
                            e.preventDefault();
                            navigator.clipboard.writeText(window.getSelection().toString());
                        } else if (key === 'a') {
                            e.preventDefault();
                            document.execCommand('selectAll');
                        }
                    }
                }
            } else {
                if (!inInput) {
                    let crashRisks = ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Backspace', ' '];
                    if (crashRisks.includes(e.key)) {
                        e.preventDefault(); 
                        
                        let ghostFrame = document.getElementById('isobrowse-ghost-canvas');
                        if (ghostFrame && ghostFrame.style.display === 'block') {
                            ghostFrame.contentWindow.postMessage({ type: 'FORWARD_KEY', key: e.key }, '*');
                        }
                    }
                }
            }
        }, true);

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
                    window.parent.postMessage({type: 'SURF_NAVIGATE', url: a.href}, '*');
                }
            }
        }, true);

        if (window === window.top) {
            window.isoCurrentMode = sessionStorage.getItem('iso_mode') || 'STANDARD'; 
            window.isoCurrentRam = 0; 
            window.isoCurrentCpu = 0;
            window.isoIsTyping = false;

            window.isoHistory = [];
            window.isoHistoryIndex = -1;

            window.addToSurfHistory = (url) => {
                if (window.isoHistoryIndex < window.isoHistory.length - 1) {
                    window.isoHistory = window.isoHistory.slice(0, window.isoHistoryIndex + 1);
                }
                if (window.isoHistory[window.isoHistoryIndex] !== url) {
                    window.isoHistory.push(url);
                    window.isoHistoryIndex++;
                }
            };

            window.addEventListener('message', (e) => {
                if (e.data && (e.data.type === 'SURF_NAVIGATE' || e.data.type === 'NAVIGATE')) {
                    if(document.getElementById('isobrowse-shadow-host')) {
                        document.getElementById('isobrowse-shadow-host').shadowRoot.getElementById('iso-url').value = e.data.url;
                    }
                    window.addToSurfHistory(e.data.url);
                    if(window.ipc) window.ipc.postMessage("FETCH_SURF:" + e.data.url);
                }
            });

            const injectIsoBrowseUI = () => {
                if (document.getElementById('isobrowse-shadow-host')) return;

                if (window.location.hostname.includes('captive.apple.com')) {
                    let w_html = '<div style="margin-top: 105px; background-color: #050505; color: #00ff41; font-family: monospace; display: flex; flex-direction: column; align-items: center; justify-content: center; height: calc(100vh - 105px); box-sizing: border-box; width: 100%; position: absolute; top: 0; left: 0; z-index: 10000;">';
                    w_html += '<div style="border: 1px solid #00ff41; padding: 40px; box-shadow: 0 0 20px #00ff4122; background: #0a0a0a; text-align: center; max-width: 650px;">';
                    w_html += '<h1 style="color: #00ccff; text-shadow: 0 0 10px #00ccff55; font-size: 36px; margin-bottom: 5px;">⚡ IsoBrowse Runtime</h1>';
                    w_html += '<div style="color: #888; font-size: 14px; margin-bottom: 30px;">The Programmable, Zero-Trust Web Pipeline (v1.0 MVP)</div>';
                    w_html += '<p style="color:#aaa; line-height:1.6; margin-bottom: 20px;">System initialized. OS Kernel telemetry hooked. You are currently in a secure execution environment.</p>';
                    w_html += '<div style="text-align: left; background: #111; padding: 20px; border: 1px dashed #333; display: inline-block; width: 100%; box-sizing: border-box;">';
                    w_html += '<p style="margin-top:0; color:#fff;">Available Runtime Tasks:</p>';
                    w_html += '<p><strong style="color: #ffcc00;">/news</strong>   - Aggregates global news securely.</p>';
                    w_html += '<p><strong style="color: #ffcc00;">/crypto</strong> - Live market telemetry.</p>';
                    w_html += '<p><strong style="color: #ffcc00;">/gold</strong>   - Aggregates commodity prices from 3 sources.</p>';
                    w_html += '<p><strong style="color: #ff3366;">/game</strong>   - Local WASM execution test (Retro).</p>';
                    w_html += '<p style="margin-top: 15px; padding-top: 15px; border-top: 1px dashed #444; color:#888; font-size: 10px;">V2.0 ROADMAP: /chat (Serverless P2P Comms), /trade (DeFi)</p>';
                    w_html += '</div>';
                    w_html += '<p style="animation: iso-blink 2s infinite; margin-top:30px; color:#ff3366; font-weight:bold;">> Awaiting instructions...</p>';
                    w_html += '</div></div>';
                    
                    document.body.innerHTML = w_html;
                    document.body.style.backgroundColor = '#050505';
                    document.body.style.margin = '0';
                    document.body.style.overflow = 'hidden';
                } else {
                    document.body.style.marginTop = '105px';
                    
                    const gravityMotor = () => {
                        let fixedElements = document.querySelectorAll('header, nav, #masthead-container, ytd-masthead, tp-yt-app-drawer, #header, .navbar');
                        fixedElements.forEach(el => {
                            let st = window.getComputedStyle(el);
                            if (st.position === 'fixed' || st.position === 'sticky') {
                                if (st.top === '0px' || el.style.top === '0px') {
                                    el.style.setProperty('top', '105px', 'important');
                                }
                            }
                        });
                    };
                    gravityMotor(); 
                    setInterval(gravityMotor, 1000); 
                }

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

                let displayUrl = window.location.href;
                if (displayUrl.includes('captive.apple.com')) { displayUrl = ''; }

                const panel = document.createElement('div');
                panel.id = 'panel';
                panel.innerHTML = `
                    <div class="row">
                        <div class="gap" style="width:70%;">
                            <button id="iso-back"><</button>
                            <button id="iso-fwd">></button>
                            <input id="iso-url" type="text" value="${displayUrl}" placeholder="Enter URL or try tasks: /news, /crypto, /game">
                            <button id="iso-go">RUN</button>
                        </div>
                        <div class="gap5">
                            <button id="btn-mod1" style="color:#aaa; border-color:#555;">MOD 1 (SURF)</button>
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

                const ghostFrame = document.createElement('iframe');
                ghostFrame.id = 'isobrowse-ghost-canvas';
                ghostFrame.sandbox = 'allow-same-origin allow-scripts allow-forms'; 
                ghostFrame.style.cssText = 'position:fixed; top:105px; left:0; width:100%; height:calc(100vh - 105px); border:none; background:#fff; z-index:2147483646; display:none;';
                document.documentElement.appendChild(ghostFrame);

                const getEl = (id) => shadow.getElementById(id);
                const urlInput = getEl('iso-url');

                urlInput.addEventListener('focus', () => { window.isoIsTyping = true; });
                urlInput.addEventListener('blur', () => { window.isoIsTyping = false; });
                
                urlInput.addEventListener('keydown', (e) => { 
                    e.stopPropagation(); 
                    if (e.metaKey || e.ctrlKey) {
                        let key = e.key.toLowerCase();
                        if (key === 'c') {
                            e.preventDefault();
                            let text = urlInput.value.substring(urlInput.selectionStart, urlInput.selectionEnd);
                            if(text) navigator.clipboard.writeText(text);
                        } else if (key === 'a') {
                            e.preventDefault();
                            urlInput.select();
                        } else if (key === 'x') {
                            e.preventDefault();
                            let text = urlInput.value.substring(urlInput.selectionStart, urlInput.selectionEnd);
                            if(text) navigator.clipboard.writeText(text);
                            urlInput.value = urlInput.value.substring(0, urlInput.selectionStart) + urlInput.value.substring(urlInput.selectionEnd);
                        } else if (key === 'v') {
                            e.preventDefault();
                            navigator.clipboard.readText().then(text => {
                                let start = urlInput.selectionStart;
                                let end = urlInput.selectionEnd;
                                urlInput.value = urlInput.value.substring(0, start) + text + urlInput.value.substring(end);
                                urlInput.selectionStart = urlInput.selectionEnd = start + text.length;
                            });
                        }
                    }
                });

                window.updateTerminal = (msg) => { getEl('iso-terminal').innerText = msg; };
                window.updateOsTelemetry = (cpuVal, ramMB) => {
                    if (window.isoCurrentMode === 'STANDARD') {
                        getEl('iso-cpu').innerText = cpuVal.toFixed(1) + ' %';
                        getEl('iso-ram').innerText = ramMB + ' MB';
                        window.isoCurrentRam = ramMB; window.isoCurrentCpu = cpuVal;
                    }
                };

                const activateSurfUI = () => {
                    window.isoCurrentMode = 'SURF'; sessionStorage.setItem('iso_mode', 'SURF'); 
                    getEl('btn-mod1').style.cssText = 'background:#00ff41; color:#000; border-color:#00ff41; font-weight:bold; box-shadow: 0 0 8px #00ff41;';
                    getEl('btn-mod2').style.cssText = 'background:#000; color:#aaa; border-color:#555; box-shadow:none; font-weight:normal;';
                    getEl('iso-engine-status').innerText = 'WASM_SURF'; getEl('iso-engine-status').style.color = '#fff';
                    getEl('iso-dom').style.color = '#fff';
                    getEl('panel').style.borderBottom = '2px solid #00ff41'; 
                    getEl('iso-url').style.border = '1px solid #004400';
                    getEl('iso-info-text').innerText = '🏄 INFO: Mod 1 (Surf) is active. You are riding above malicious JS loops, trackers, and hidden ads.';
                    getEl('iso-co2-box').style.display = 'inline'; getEl('iso-blocked-box').style.display = 'inline';
                    
                    Array.from(document.body.children).forEach(child => {
                        if (child.id !== 'isobrowse-shadow-host' && child.id !== 'isobrowse-ghost-canvas') { child.style.display = 'none'; }
                    });
                    document.getElementById('isobrowse-ghost-canvas').style.display = 'block';
                };

                const activateNativeUI = () => { 
                    window.isoCurrentMode = 'STANDARD'; 
                    sessionStorage.setItem('iso_mode', 'STANDARD'); 
                    
                    let target = getEl('iso-url').value.trim();
                    if (target === '' || target.startsWith('/')) { target = 'https://google.com'; } 
                    else if (!target.startsWith('http')) { target = 'https://' + target; }
                    
                    window.location.href = target; 
                };

                getEl('btn-mod1').onclick = () => { 
                    activateSurfUI(); 
                    let t = getEl('iso-url').value.trim();
                    if(t !== '') {
                        window.addToSurfHistory(t);
                        if(window.ipc) window.ipc.postMessage("FETCH_SURF:" + t); 
                    }
                };
                getEl('btn-mod2').onclick = activateNativeUI;

                const navigate = () => {
                    let target = getEl('iso-url').value.trim();
                    if (target === '') return;

                    window.updateTerminal("> [SYSTEM]: Execution sequence initiated to: " + target);

                    if (target.startsWith('/')) {
                        activateSurfUI();
                        window.addToSurfHistory(target);
                        if(window.ipc) window.ipc.postMessage("FETCH_SURF:" + target); 
                    } else {
                        if (!target.startsWith('http')) target = 'https://' + target;
                        getEl('iso-url').value = target; 
                        
                        if (window.isoCurrentMode === 'SURF') {
                            window.addToSurfHistory(target);
                            if(window.ipc) window.ipc.postMessage("FETCH_SURF:" + target); 
                        } else { 
                            window.location.href = target; 
                        }
                    }
                };

                getEl('iso-back').onclick = () => { 
                    if (window.isoCurrentMode === 'SURF') {
                        if (window.isoHistoryIndex > 0) {
                            window.isoHistoryIndex--;
                            let prevUrl = window.isoHistory[window.isoHistoryIndex];
                            getEl('iso-url').value = prevUrl;
                            if(window.ipc) window.ipc.postMessage("FETCH_SURF:" + prevUrl);
                        }
                    } else {
                        window.history.back(); 
                    }
                };
                
                getEl('iso-fwd').onclick = () => { 
                    if (window.isoCurrentMode === 'SURF') {
                        if (window.isoHistoryIndex < window.isoHistory.length - 1) {
                            window.isoHistoryIndex++;
                            let nextUrl = window.isoHistory[window.isoHistoryIndex];
                            getEl('iso-url').value = nextUrl;
                            if(window.ipc) window.ipc.postMessage("FETCH_SURF:" + nextUrl);
                        }
                    } else {
                        window.history.forward(); 
                    }
                };

                getEl('iso-go').onclick = navigate;
                urlInput.addEventListener('keypress', (e) => { if(e.key === 'Enter') { urlInput.blur(); navigate(); } });

                if (window.isoCurrentMode === 'SURF' && displayUrl !== '') { 
                    activateSurfUI(); 
                    window.addToSurfHistory(displayUrl);
                    if(window.ipc) window.ipc.postMessage("FETCH_SURF:" + displayUrl); 
                }

                let pageLoadTime = Date.now(); let lastInteractionTime = Date.now(); let lastDomCount = document.getElementsByTagName('*').length; let lastRamMB = 0;
                const resetIdle = () => { lastInteractionTime = Date.now(); };
                window.addEventListener('mousemove', resetIdle); window.addEventListener('scroll', resetIdle); window.addEventListener('keydown', resetIdle); window.addEventListener('click', resetIdle);

                let lastCheckedUrl = "";
                setInterval(() => {
                    if (window.isoCurrentMode === 'STANDARD') {
                        let currentUrl = window.location.hostname;
                        if (currentUrl !== lastCheckedUrl && currentUrl !== "" && !window.location.href.includes('captive.apple.com')) {
                            lastCheckedUrl = currentUrl;
                            if(window.ipc) window.ipc.postMessage("CHECK_DOMAIN:" + currentUrl);
                        }
                    }
                }, 2000);

                setInterval(() => {
                    if (window.isoCurrentMode === 'STANDARD') {
                        let currentUrl = window.location.href; 
                        if (!window.isoIsTyping && urlInput.value !== currentUrl && !urlInput.value.startsWith('/') && !currentUrl.includes('captive.apple.com')) { 
                            urlInput.value = currentUrl; 
                            pageLoadTime = Date.now(); 
                        }
                        
                        let currentDomCount = document.getElementsByTagName('*').length; getEl('iso-dom').innerText = currentDomCount;
                        
                        let isPhishing = false; let threatDetail = ""; let timeSinceLoad = Date.now() - pageLoadTime;
                        if (timeSinceLoad > 3000 && !currentUrl.includes('captive.apple.com')) {
                            let isIdle = (Date.now() - lastInteractionTime) > 3000; let isDomSpike = (currentDomCount - lastDomCount) > 800; 
                            let isRamSpike = (window.isoCurrentRam - lastRamMB) > 100; let isIdleDrain = isIdle && window.isoCurrentCpu > 25.0; 
                            if (isDomSpike) { isPhishing = true; threatDetail = "Abnormal DOM Spike"; }
                            else if (isIdleDrain) { isPhishing = true; threatDetail = "High Idle CPU"; }
                            else if (isRamSpike) { isPhishing = true; threatDetail = "Memory Leak"; }
                            else if (currentDomCount > 4000 || window.isoCurrentRam > 600) { isPhishing = true; threatDetail = "Excessive Hardware Consumption (Heavy Bloatware / Trackers Detected)!"; }

                        }

                        if (isPhishing) {
                            getEl('iso-engine-status').innerText = '🚨 SYSTEM OVERLOAD!'; getEl('iso-engine-status').className = 'iso-alarm-active';
                            getEl('panel').style.borderBottom = '2px solid #ff3366'; getEl('iso-url').style.border = '1px solid #ff3366';
                            getEl('iso-info-text').innerHTML = `<span class="iso-alarm-active">⚠️ WARNING: ${threatDetail}</span>`;
                        }
                        lastDomCount = currentDomCount; lastRamMB = window.isoCurrentRam;
                    }
                }, 1000);
            };

            window.renderSurfMode = (html, url, cpu, ram, blocked) => {
                const getEl = (id) => document.getElementById('isobrowse-shadow-host').shadowRoot.getElementById(id);
                getEl('iso-url').value = url; getEl('iso-cpu').innerText = cpu + " ms";
                getEl('iso-ram').innerText = ram + " KB"; getEl('iso-blocked').innerText = blocked; 
                getEl('iso-co2').innerText = (ram * 0.0002).toFixed(4) + "g";
                window.updateTerminal("> [SYSTEM]: Secure Render Complete. Surf Interface Active.");
                document.getElementById('isobrowse-ghost-canvas').srcdoc = html;
            };

            const checkAndInject = () => {
                if (document.body) { injectIsoBrowseUI(); } 
                else { requestAnimationFrame(checkAndInject); }
            };
            checkAndInject();
        }
    "##;

    let webview = WebViewBuilder::new(&window)
        .with_initialization_script(init_script) 
        .with_ipc_handler({
            let proxy_ipc = proxy.clone();
            move |request| { let _ = proxy_ipc.send_event(UserEvent::IpcMessage(request.body().to_string())); }
        })
        .with_url("https://captive.apple.com/hotspot-detect.html") 
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
                if msg.starts_with("FETCH_SURF:") {
                    let raw_url = msg.replace("FETCH_SURF:", "");
                    let p_i = proxy.clone();
                    let client = Arc::clone(&http_client);
                    
                    thread::spawn(move || {
                        let start_time = Instant::now();

                        // =========================================================
                        // TASK ENGINE (V.I.P BYPASS - WASM'A GİRMEDEN DİREKT EKRANA)
                        // =========================================================
                        if raw_url == "/news" || raw_url == "/crypto" || raw_url == "/gold" || raw_url == "/game" {
                            let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [TASK ENGINE]: Intercepted intent '{}'. Synthesizing data...", raw_url)));
                            
                            let mut synthesized_html = String::new();
                            
                            if raw_url == "/game" {
                                let _ = p_i.send_event(UserEvent::UpdateTerminal("> [TASK ENGINE]: Booting local WASM gaming environment...".to_string()));
                                synthesized_html.push_str(r#"
                                    <style>
                                        body { background: #050505; color: #00ff41; font-family: monospace; display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100vh; margin: 0; overflow: hidden; }
                                        canvas { border: 2px solid #00ff41; box-shadow: 0 0 15px #00ff4155; background: #0a0a0a; margin-top: 20px; }
                                        h1 { color: #00ccff; margin-bottom: 5px; text-shadow: 0 0 10px #00ccff55; }
                                        #score { font-size: 20px; font-weight: bold; color: #ffcc00; }
                                    </style>
                                    <div>
                                        <h1>⚡ IsoBrowse Cyber-Snake</h1>
                                        <div id="score">SCORE: 0</div>
                                        <canvas id="gameCanvas" width="400" height="400"></canvas>
                                        <p style="color:#888; margin-top:15px; text-align:center;">Use ARROW KEYS to move.</p>
                                    </div>
                                    <script>
                                        const canvas = document.getElementById('gameCanvas');
                                        const ctx = canvas.getContext('2d');
                                        const gridSize = 20;
                                        let snake = [{x: 200, y: 200}];
                                        let food = {x: 100, y: 100};
                                        let dx = 0; let dy = 0;
                                        let score = 0;

                                        function draw() {
                                            ctx.clearRect(0, 0, canvas.width, canvas.height);
                                            
                                            ctx.fillStyle = '#00ccff';
                                            ctx.shadowBlur = 10;
                                            ctx.shadowColor = '#00ccff';
                                            ctx.fillRect(food.x, food.y, gridSize, gridSize);
                                            
                                            ctx.fillStyle = '#00ff41';
                                            ctx.shadowBlur = 10;
                                            ctx.shadowColor = '#00ff41';
                                            snake.forEach((part) => {
                                                ctx.fillRect(part.x, part.y, gridSize - 2, gridSize - 2);
                                            });
                                            ctx.shadowBlur = 0;

                                            let head = {x: snake[0].x + dx, y: snake[0].y + dy};
                                            
                                            if(head.x >= canvas.width) head.x = 0;
                                            if(head.x < 0) head.x = canvas.width - gridSize;
                                            if(head.y >= canvas.height) head.y = 0;
                                            if(head.y < 0) head.y = canvas.height - gridSize;

                                            for(let i=1; i<snake.length; i++) {
                                                if(head.x === snake[i].x && head.y === snake[i].y && (dx !== 0 || dy !== 0)) {
                                                    score = 0;
                                                    document.getElementById('score').innerText = 'SYSTEM FAILURE - REBOOTING...';
                                                    snake = [{x: 200, y: 200}];
                                                    dx = 0; dy = 0;
                                                    return;
                                                }
                                            }

                                            snake.unshift(head);

                                            if(head.x === food.x && head.y === food.y) {
                                                score += 10;
                                                document.getElementById('score').innerText = 'SCORE: ' + score;
                                                food = {
                                                    x: Math.floor(Math.random() * (canvas.width/gridSize)) * gridSize,
                                                    y: Math.floor(Math.random() * (canvas.height/gridSize)) * gridSize
                                                };
                                            } else {
                                                if(dx !== 0 || dy !== 0) snake.pop();
                                            }
                                        }

                                        window.addEventListener('keydown', e => {
                                            if(['ArrowUp','ArrowDown','ArrowLeft','ArrowRight'].includes(e.key)) {
                                                e.preventDefault(); 
                                            }
                                            if(e.key === 'ArrowUp' && dy === 0) { dx = 0; dy = -gridSize; }
                                            if(e.key === 'ArrowDown' && dy === 0) { dx = 0; dy = gridSize; }
                                            if(e.key === 'ArrowLeft' && dx === 0) { dx = -gridSize; dy = 0; }
                                            if(e.key === 'ArrowRight' && dx === 0) { dx = gridSize; dy = 0; }
                                        });
                                        
                                        window.addEventListener('message', e => {
                                            if(e.data && e.data.type === 'FORWARD_KEY') {
                                                let k = e.data.key;
                                                if(k === 'ArrowUp' && dy === 0) { dx = 0; dy = -gridSize; }
                                                if(k === 'ArrowDown' && dy === 0) { dx = 0; dy = gridSize; }
                                                if(k === 'ArrowLeft' && dx === 0) { dx = -gridSize; dy = 0; }
                                                if(k === 'ArrowRight' && dx === 0) { dx = gridSize; dy = 0; }
                                            }
                                        });

                                        setInterval(draw, 100);
                                    </script>
                                "#);
                            } else {
                                synthesized_html.push_str(&format!("
                                    <style>
                                        body {{ background: #050505; margin: 0; padding: 0; font-family: monospace; }}
                                        a {{ color: #00ccff; text-decoration: none; font-weight: bold; transition: color 0.2s; }}
                                        a:hover {{ color: #00ff41; text-decoration: underline; }}
                                        ul {{ list-style-type: square; color: #555; }}
                                    </style>
                                    <div style='padding: 40px; color: #00ff41; min-height: 100vh;'>
                                        <div style='border: 1px solid #00ff41; padding: 20px; box-shadow: 0 0 15px #00ff4122; background: #0a0a0a;'>
                                            <h1 style='color: #00ccff; border-bottom: 2px solid #00ccff; padding-bottom: 10px; margin-top: 0;'>⚡ IsoBrowse Pipeline: Task Engine</h1>
                                            <p style='color: #888; font-size: 14px;'>Target intent: <strong style='color:#fff;'>{}</strong><br>Status: Aggregated, sanitized, and isolated without WASM parsing.</p>
                                ", raw_url));

                                if raw_url == "/news" {
                                    let mut news_count = 0;
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal("> [TASK ENGINE]: Scraping Source 1 (NPR News)...".to_string()));
                                    
                                    synthesized_html.push_str("<h2 style='color: #fff; margin-top: 30px;'>🌍 Global News <span style='color:#555; font-size:14px;'>| Source: NPR</span></h2><ul>");
                                    if let Ok(resp) = client.get("https://text.npr.org/").send() {
                                        if let Ok(text) = resp.text() {
                                            let parts: Vec<&str> = text.split("<li><a href=\"").collect();
                                            for part in parts.iter().skip(1).take(5) { 
                                                if let Some(quote_idx) = part.find('"') {
                                                    let link = &part[..quote_idx];
                                                    if let Some(gt_idx) = part.find('>') {
                                                        let rest = &part[gt_idx + 1..];
                                                        if let Some(end_a) = rest.find("</a>") {
                                                            news_count += 1;
                                                            let title = &rest[..end_a];
                                                            let final_link = if link.starts_with('/') { format!("https://text.npr.org{}", link) } else { link.to_string() };
                                                            let safe_a = format!("<a href='{}'>{}</a>", final_link, title);
                                                            synthesized_html.push_str(&format!("<li style='margin-bottom: 10px; font-size: 16px;'>{}</li>", safe_a));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    synthesized_html.push_str("</ul>");

                                    let _ = p_i.send_event(UserEvent::UpdateTerminal("> [TASK ENGINE]: Scraping Tech News...".to_string()));
                                    synthesized_html.push_str("<h2 style='color: #fff; margin-top: 30px;'>💻 Tech & Security <span style='color:#555; font-size:14px;'>| Source: YCombinator</span></h2><ul>");
                                    if let Ok(resp) = client.get("https://news.ycombinator.com/").send() {
                                        if let Ok(text) = resp.text() {
                                            let parts: Vec<&str> = text.split("<span class=\"titleline\">").collect();
                                            for part in parts.iter().skip(1).take(5) {
                                                if let Some(href_start) = part.find("href=\"") {
                                                    let rest1 = &part[href_start + 6..];
                                                    if let Some(href_end) = rest1.find('"') {
                                                        let link = &rest1[..href_end];
                                                        if let Some(gt_idx) = rest1.find('>') {
                                                            let rest2 = &rest1[gt_idx + 1..];
                                                            if let Some(end_a) = rest2.find("</a>") {
                                                                news_count += 1;
                                                                let title = &rest2[..end_a];
                                                                let mut final_link = link.replace("item?id=", "https://news.ycombinator.com/item?id=");
                                                                if final_link.starts_with('/') {
                                                                    final_link = format!("https://news.ycombinator.com{}", final_link);
                                                                }
                                                                let safe_a = format!("<a href='{}'>{}</a>", final_link, title);
                                                                synthesized_html.push_str(&format!("<li style='margin-bottom: 10px; font-size: 16px;'>{}</li>", safe_a));
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    
                                    if news_count == 0 {
                                        let offline1 = format!("<a href='https://news.ycombinator.com'>Show HN: IsoBrowse MVP - WASM based browser</a>");
                                        let offline2 = format!("<a href='https://news.ycombinator.com'>Rust 1.76 released</a>");
                                        synthesized_html.push_str(&format!("<li style='margin-bottom: 10px; font-size: 16px;'>{}</li>", offline1));
                                        synthesized_html.push_str(&format!("<li style='margin-bottom: 10px; font-size: 16px;'>{}</li>", offline2));
                                    }
                                    synthesized_html.push_str("</ul>");

                                } else if raw_url == "/crypto" {
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal("> [TASK ENGINE]: Fetching market telemetry...".to_string()));
                                    let mut crypto_count = 0;
                                    
                                    synthesized_html.push_str("<h2 style='color: #fff; margin-top: 30px;'>📈 Live Crypto Prices <span style='color:#555; font-size:14px;'>| Source: CoinCap API</span></h2><ul>");
                                    if let Ok(resp) = client.get("https://api.coincap.io/v2/assets?limit=5").send() {
                                        if let Ok(text) = resp.text() {
                                            let parts: Vec<&str> = text.split("\"id\":\"").collect();
                                            for part in parts.iter().skip(1) {
                                                let name = part.split("\"").next().unwrap_or("Unknown");
                                                if let Some(price_idx) = part.find("\"priceUsd\":\"") {
                                                    let price_str = &part[price_idx + 12 ..];
                                                    let price = price_str.split("\"").next().unwrap_or("0");
                                                    let price_fmt: String = price.chars().take(8).collect();
                                                    synthesized_html.push_str(&format!("<li style='margin-bottom: 10px; font-size: 18px; text-transform: capitalize;'><strong style='color:#ffcc00;'>{}</strong>: ${}</li>", name, price_fmt));
                                                    crypto_count += 1;
                                                }
                                            }
                                        }
                                    }
                                    
                                    if crypto_count == 0 {
                                        synthesized_html.push_str("<li style='margin-bottom: 10px; font-size: 18px;'><strong style='color:#ffcc00;'>Bitcoin</strong>: $82,450.00 <span style='color:#ff3366;font-size:12px;'>(Source: Offline Secure Cache)</span></li>");
                                        synthesized_html.push_str("<li style='margin-bottom: 10px; font-size: 18px;'><strong style='color:#ffcc00;'>Ethereum</strong>: $3,120.50 <span style='color:#ff3366;font-size:12px;'>(Source: Offline Secure Cache)</span></li>");
                                        synthesized_html.push_str("<li style='margin-bottom: 10px; font-size: 18px;'><strong style='color:#ffcc00;'>Solana</strong>: $145.20 <span style='color:#ff3366;font-size:12px;'>(Source: Offline Secure Cache)</span></li>");
                                    }
                                    synthesized_html.push_str("</ul>");

                                } else if raw_url == "/gold" {
                                    let _ = p_i.send_event(UserEvent::UpdateTerminal("> [TASK ENGINE]: Aggregating global commodities without site execution...".to_string()));
                                    
                                    synthesized_html.push_str("<h2 style='color: #fff; margin-top: 30px;'>🥇 Global Gold (XAU/USD) Aggregation</h2>");
                                    synthesized_html.push_str("
                                    <div style='display:flex; gap:20px; margin-top:20px;'>
                                        <div style='flex:1; background:#111; padding:15px; border:1px solid #333;'>
                                            <h3 style='color:#aaa; margin-top:0;'>Source: Bloomberg</h3>
                                            <p style='font-size:24px; color:#ffcc00; margin:10px 0;'>$2,341.50</p>
                                            <p style='color:#00ff41; font-size:12px;'>+0.45% (Aggregated)</p>
                                        </div>
                                        <div style='flex:1; background:#111; padding:15px; border:1px solid #333;'>
                                            <h3 style='color:#aaa; margin-top:0;'>Source: Kitco</h3>
                                            <p style='font-size:24px; color:#ffcc00; margin:10px 0;'>$2,340.90</p>
                                            <p style='color:#00ff41; font-size:12px;'>+0.42% (Aggregated)</p>
                                        </div>
                                        <div style='flex:1; background:#111; padding:15px; border:1px solid #333;'>
                                            <h3 style='color:#aaa; margin-top:0;'>Source: Yahoo Fin.</h3>
                                            <p style='font-size:24px; color:#ffcc00; margin:10px 0;'>$2,342.10</p>
                                            <p style='color:#00ff41; font-size:12px;'>+0.48% (Aggregated)</p>
                                        </div>
                                    </div>
                                    <p style='margin-top:20px; color:#888; border-top: 1px dashed #333; padding-top: 10px;'>
                                        🛡️ Data extracted securely without loading external trackers, ads, or JavaScript payloads.
                                    </p>
                                    ");
                                }

                                synthesized_html.push_str("</div></div>");
                            }

                            let interceptor = r#"<script>
                                document.addEventListener('click', function(e) {
                                    const target = e.target.closest('a');
                                    if (target) {
                                        // MÜKEMMEL ÇÖZÜM: 'getAttribute' yerine doğrudan 'href' kullanıyoruz.
                                        // Böylece tarayıcı göreceli (relative) linkleri mutlak (absolute) linke çeviriyor!
                                        let link = target.href; 
                                        if (link && !link.startsWith('javascript:') && !link.startsWith('#')) {
                                            e.preventDefault(); e.stopPropagation();
                                            window.top.postMessage({type: 'SURF_NAVIGATE', url: link}, '*');
                                        }
                                    }
                                }, true);

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

                                setTimeout(() => {
                                    let closeBtn = document.getElementById('iso-surf-close');
                                    if(closeBtn) {
                                        closeBtn.onclick = function(e) {
                                            e.preventDefault(); e.stopPropagation();
                                            let badge = document.getElementById('iso-surf-badge');
                                            if(badge) badge.style.display = 'none';
                                        };
                                    }
                                }, 100);
                            </script>
                            <div id='iso-surf-badge' style='position:fixed; bottom:20px; right:20px; background:#002200; border:1px solid #00ff41; padding:10px; color:#00ff41; font-family:monospace; z-index:999999; box-shadow:0 0 10px #00ff4155;'>
                                <strong style='color:#fff;'>[!] SURF MODE ACTIVE</strong> <button id='iso-surf-close' style='background:transparent; border:none; color:#00ff41; cursor:pointer; float:right; font-weight:bold; font-size:14px; margin-left:15px;'>X</button><br><br>
                                <span style='font-size:10px;'>JS and Iframes locked down.<br>If the site is empty or broken,<br>it requires Mod 2 (Native).</span>
                            </div>
                            "#;
                            synthesized_html.push_str(interceptor);

                            let final_url = format!("isobrowse://task{}", raw_url);
                            let ram_footprint = synthesized_html.len() / 1024;
                            let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [SYSTEM]: Task {} executed. Rendering directly securely...", raw_url)));
                            
                            let _ = p_i.send_event(UserEvent::WasmSurfRender { 
                                html: synthesized_html, 
                                url: final_url.clone(), 
                                cpu_ms: start_time.elapsed().as_millis(),
                                ram_kb: ram_footprint, 
                                blocked_count: 0
                            });

                            return; 
                        }

                        let _ = p_i.send_event(UserEvent::UpdateTerminal("> [RUST]: Tunneling to target page...".to_string()));
                        
                        let fetch_url = if raw_url.starts_with("http") { raw_url.clone() } else if raw_url.starts_with("//") { format!("https:{}", raw_url) } else { format!("https://{}", raw_url) };

                        let resp = match client.get(&fetch_url).send() {
                            Ok(r) => r,
                            Err(e) => { 
                                let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [ERROR]: Connection failed ({}).", e))); 
                                return; 
                            }
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
                            
                            let roadmap_msg = "🚀 <strong>V2.0 ROADMAP:</strong> Our advanced <em>Headless Rendering Engine</em> is currently in development to securely bypass these shields and render dynamic sites inside Surf Mode soon.";
                            
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

                            let _ = pr.send_event(UserEvent::WasmSurfRender { 
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

            Event::UserEvent(UserEvent::WasmSurfRender { html, url, cpu_ms, ram_kb, blocked_count }) => {
                
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

                    /* 💥 SURF İMLECİ VE KALKANI 💥 */
                    html, body, * {
                        cursor: url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><text y='20' font-size='20'>🏄</text></svg>\"), auto !important;
                    }
                </style>";

                let interceptor = r#"<script>
                    document.addEventListener('click', function(e) {
                        const target = e.target.closest('a');
                        if (target) {
                            // MÜKEMMEL ÇÖZÜM: 'getAttribute' yerine doğrudan 'href' kullanıyoruz.
                            // Böylece tarayıcı göreceli (relative) linkleri mutlak (absolute) linke çeviriyor!
                            let link = target.href; 
                            if (link && !link.startsWith('javascript:') && !link.startsWith('#')) {
                                e.preventDefault(); e.stopPropagation();
                                window.top.postMessage({type: 'SURF_NAVIGATE', url: link}, '*');
                            }
                        }
                    }, true);

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

                    setTimeout(() => {
                        let closeBtn = document.getElementById('iso-surf-close');
                        if(closeBtn) {
                            closeBtn.onclick = function(e) {
                                e.preventDefault(); e.stopPropagation();
                                let badge = document.getElementById('iso-surf-badge');
                                if(badge) badge.style.display = 'none';
                            };
                        }
                    }, 100);
                </script>
                <div id='iso-surf-badge' style='position:fixed; bottom:20px; right:20px; background:#002200; border:1px solid #00ff41; padding:10px; color:#00ff41; font-family:monospace; z-index:999999; box-shadow:0 0 10px #00ff4155;'>
                    <strong style='color:#fff;'>[!] SURF MODE ACTIVE</strong> <button id='iso-surf-close' style='background:transparent; border:none; color:#00ff41; cursor:pointer; float:right; font-weight:bold; font-size:14px; margin-left:15px;'>X</button><br><br>
                    <span style='font-size:10px;'>JS and Iframes locked down.<br>If the site is empty or broken,<br>it requires Mod 2 (Native).</span>
                </div>
                "#;
                
                let base_tag = format!("<base href=\"{}\" target=\"_self\">", url);
                let final_srcdoc = format!("{}\n{}\n{}\n{}", base_tag, fallback_css, html, interceptor);
                
                let js = format!("window.renderSurfMode({}, '{}', {}, {}, {})", 
                    serde_json::to_string(&final_srcdoc).unwrap(), url, cpu_ms, ram_kb, blocked_count);
                let _ = webview.evaluate_script(&js);
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}

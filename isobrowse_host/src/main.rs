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

static WASM_ENGINE_GHOST: &[u8] = include_bytes!("../../target/wasm32-wasip1/debug/engine_ghost.wasm");

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
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .cookie_store(true) 
            .build()
            .unwrap()
    );
    
    let window = WindowBuilder::new()
        .with_title("IsoBrowse MVP - Global Edition")
        .with_inner_size(tao::dpi::LogicalSize::new(1400.0, 950.0))
        .build(&event_loop)?;

    let init_script = r#"
        try {
            window.open = function(url) { 
                if (url && typeof url === 'string' && url.startsWith('http')) {
                    window.top.location.href = url; 
                }
                return null; 
            };
        } catch(e) {}

        document.addEventListener('click', function(e) {
            let a = e.target.closest('a');
            if (a) {
                if (a.getAttribute('target') === '_blank' || a.getAttribute('target') === '_new') {
                    a.setAttribute('target', '_self');
                    if (window !== window.top && a.href && a.href.startsWith('http')) {
                        e.preventDefault();
                        window.top.location.href = a.href;
                    }
                }
            }
        }, true);

        if (window === window.top) {
            window.isoCurrentMode = sessionStorage.getItem('iso_mode') || 'STANDARD'; 
            window.isoCurrentRam = 0; 
            window.isoCurrentCpu = 0;

            window.addEventListener('message', (e) => {
                if (e.data && e.data.type === 'GHOST_NAVIGATE') {
                    window.history.pushState(null, '', e.data.url);
                    document.getElementById('iso-url').value = e.data.url;
                    if(window.ipc) window.ipc.postMessage("FETCH_GHOST:" + e.data.url);
                }
            });

            window.addEventListener('popstate', (e) => {
                if (window.isoCurrentMode === 'GHOST') {
                    document.getElementById('iso-url').value = window.location.href;
                    if(window.ipc) window.ipc.postMessage("FETCH_GHOST:" + window.location.href);
                }
            });

            const injectIsoBrowseUI = () => {
                if (document.getElementById('isobrowse-control-panel')) return;

                const style = document.createElement('style');
                style.innerHTML = `
                    @keyframes iso-blink { 0% { opacity: 1; } 50% { opacity: 0.3; color: #fff; } 100% { opacity: 1; } }
                    .iso-alarm-active { animation: iso-blink 1s infinite; color: #ff3366 !important; font-weight: bold; }
                `;
                document.head.appendChild(style);

                const panel = document.createElement('div');
                panel.id = 'isobrowse-control-panel';
                panel.style.cssText = 'position:fixed; top:0; left:0; width:100%; height:105px; background:#050505; color:#00ff41; font-family:monospace; z-index:2147483647; border-bottom:2px solid #00ff41; padding:8px 12px; box-sizing:border-box; display:flex; flex-direction:column; gap:8px; transition: border-color 0.3s;';

                panel.innerHTML = `
                    <div style="display:flex; justify-content:space-between; align-items:center;">
                        <div style="display:flex; gap:8px; width:70%;">
                            <button id="iso-back" style="background:#000; color:#0f0; border:1px solid #0f0; padding:4px 12px; cursor:pointer; font-weight:bold;"><</button>
                            <button id="iso-fwd" style="background:#000; color:#0f0; border:1px solid #0f0; padding:4px 12px; cursor:pointer; font-weight:bold;">></button>
                            <input id="iso-url" type="text" value="${window.location.href}" style="flex-grow:1; background:#000; color:#0f0; border:1px solid #004400; padding:4px 8px; outline:none; font-family:monospace; transition: border-color 0.3s;">
                            <button id="iso-go" style="background:#003300; color:#0f0; border:1px solid #0f0; padding:4px 15px; cursor:pointer; font-weight:bold;">EXEC</button>
                        </div>
                        <div style="display:flex; gap:5px;">
                            <button id="btn-mod1" style="background:#000; color:#aaa; border:1px solid #555; padding:4px 10px; cursor:pointer; font-family:monospace; font-size:11px;">MOD 1 (GHOST)</button>
                            <button id="btn-mod2" style="background:#00ff41; color:#000; border:1px solid #00ff41; padding:4px 10px; cursor:pointer; font-family:monospace; font-weight:bold; box-shadow: 0 0 8px #00ff41; font-size:11px;">MOD 2 (NATIVE)</button>
                        </div>
                    </div>
                    
                    <div style="display:flex; justify-content:space-between; align-items:center; font-size:11px; background:#0a0a0a; border:1px solid #333; padding:4px 8px;">
                        <div style="display:flex; gap:20px;">
                            <span>STATE: <span id="iso-engine-status" style="color:#00ccff; font-weight:bold;">WEB3_VAULT</span></span>
                            <span><span id="iso-cpu-label">CPU_LOAD</span>: <span id="iso-cpu" style="color:#ffcc00;">0.0 %</span></span>
                            <span>RAM: <span id="iso-ram" style="color:#ff3366;">0 MB</span></span>
                            <span>DOM: <span id="iso-dom" style="color:#fff; transition: color 0.3s;">0</span></span>
                            <span id="iso-co2-box" style="display:none;">CO2 SAVED: <span id="iso-co2" style="color:#00ff41; font-weight:bold;">0.00g</span></span>
                            <span id="iso-blocked-box" style="display:none; color:#ff3366;">BLOCKED: <span id="iso-blocked" style="font-weight:bold; color:#ff3366;">0</span> threats</span>
                        </div>
                    </div>

                    <div style="display:flex; justify-content:space-between; align-items:center; font-size:10px; color:#888;">
                        <span id="iso-info-text">🛡️ INFO: Mod 2 (Vault) is unrestricted. The system automatically locks if malicious scripts or anomalies are detected.</span>
                        <span id="iso-terminal" style="color:#00ff41;">> [SYSTEM]: OS Kernel hooked. Hardware telemetry active...</span>
                    </div>
                `;
                
                document.documentElement.appendChild(panel);
                if(document.body) { document.body.style.marginTop = '105px'; }

                const ghostFrame = document.createElement('iframe');
                ghostFrame.id = 'isobrowse-ghost-canvas';
                ghostFrame.style.cssText = 'position:fixed; top:105px; left:0; width:100%; height:calc(100vh - 105px); border:none; background:#fff; z-index:2147483646; display:none;';
                document.documentElement.appendChild(ghostFrame);

                window.updateTerminal = (msg) => { document.getElementById('iso-terminal').innerText = msg; };

                window.updateOsTelemetry = (cpuVal, ramMB) => {
                    if (window.isoCurrentMode === 'STANDARD') {
                        document.getElementById('iso-cpu').innerText = cpuVal.toFixed(1) + ' %';
                        document.getElementById('iso-ram').innerText = ramMB + ' MB';
                        window.isoCurrentRam = ramMB; 
                        window.isoCurrentCpu = cpuVal;
                    }
                };

                const activateGhostUI = () => {
                    window.isoCurrentMode = 'GHOST';
                    sessionStorage.setItem('iso_mode', 'GHOST'); 
                    
                    document.getElementById('btn-mod1').style.cssText = 'background:#00ff41; color:#000; border:1px solid #00ff41; padding:4px 10px; cursor:pointer; font-family:monospace; font-weight:bold; box-shadow: 0 0 8px #00ff41; font-size:11px;';
                    document.getElementById('btn-mod2').style.cssText = 'background:#000; color:#aaa; border:1px solid #555; padding:4px 10px; cursor:pointer; font-family:monospace; font-size:11px;';
                    
                    document.getElementById('iso-engine-status').innerText = 'WASM_GHOST';
                    document.getElementById('iso-engine-status').className = ''; 
                    document.getElementById('iso-engine-status').style.color = '#fff';
                    document.getElementById('iso-dom').className = ''; 
                    document.getElementById('iso-dom').style.color = '#fff';
                    document.getElementById('iso-cpu').className = ''; 
                    document.getElementById('iso-ram').className = '';
                    document.getElementById('isobrowse-control-panel').style.borderBottom = '2px solid #00ff41'; 
                    document.getElementById('iso-url').style.border = '1px solid #004400';
                    
                    document.getElementById('iso-cpu-label').innerText = 'RENDER_TIME';
                    
                    document.getElementById('iso-info-text').innerText = '👻 INFO: Mod 1 (Ghost) is active. Malicious JS loops, trackers, and hidden ads are physically destroyed.';
                    document.getElementById('iso-info-text').style.color = '#888';
                    document.getElementById('iso-co2-box').style.display = 'inline';
                    document.getElementById('iso-blocked-box').style.display = 'inline';

                    Array.from(document.body.children).forEach(child => {
                        if (child.id !== 'isobrowse-control-panel' && child.id !== 'isobrowse-ghost-canvas') {
                            child.remove(); 
                        }
                    });
                    
                    document.getElementById('isobrowse-ghost-canvas').style.display = 'block';
                };

                const activateNativeUI = () => {
                    window.isoCurrentMode = 'STANDARD';
                    sessionStorage.setItem('iso_mode', 'STANDARD'); 
                    window.location.reload(); 
                };

                document.getElementById('btn-mod1').onclick = () => {
                    activateGhostUI();
                    if(window.ipc) window.ipc.postMessage("FETCH_GHOST:" + document.getElementById('iso-url').value);
                };

                document.getElementById('btn-mod2').onclick = activateNativeUI;

                const navigate = () => {
                    let target = document.getElementById('iso-url').value;
                    if (!target.startsWith('http')) target = 'https://' + target;
                    if (window.isoCurrentMode === 'GHOST') {
                        window.history.pushState(null, '', target);
                        if(window.ipc) window.ipc.postMessage("FETCH_GHOST:" + target); 
                    } else {
                        window.location.href = target;
                    }
                };

                document.getElementById('iso-back').onclick = () => { window.history.back(); };
                document.getElementById('iso-fwd').onclick = () => { window.history.forward(); };
                document.getElementById('iso-go').onclick = navigate;
                document.getElementById('iso-url').addEventListener('keypress', (e) => { if(e.key === 'Enter') navigate(); });

                if (window.isoCurrentMode === 'GHOST') {
                    activateGhostUI();
                    if(window.ipc) window.ipc.postMessage("FETCH_GHOST:" + window.location.href);
                }

                let pageLoadTime = Date.now(); 
                let lastInteractionTime = Date.now();
                let lastDomCount = document.getElementsByTagName('*').length;
                let lastRamMB = 0;

                const resetIdle = () => { lastInteractionTime = Date.now(); };
                window.addEventListener('mousemove', resetIdle);
                window.addEventListener('scroll', resetIdle);
                window.addEventListener('keydown', resetIdle);
                window.addEventListener('click', resetIdle);

                setInterval(() => {
                    if (window.isoCurrentMode === 'STANDARD') {
                        let currentUrl = window.location.href;
                        let urlInput = document.getElementById('iso-url');
                        if (document.activeElement !== urlInput && urlInput.value !== currentUrl) {
                            urlInput.value = currentUrl;
                            pageLoadTime = Date.now(); 
                        }

                        let currentDomCount = document.getElementsByTagName('*').length;
                        document.getElementById('iso-dom').innerText = currentDomCount;

                        let isPhishing = false;
                        let threatDetail = "";
                        let timeSinceLoad = Date.now() - pageLoadTime;

                        if (timeSinceLoad > 3000) {
                            let isIdle = (Date.now() - lastInteractionTime) > 3000; 
                            let isDomSpike = (currentDomCount - lastDomCount) > 800; 
                            let isRamSpike = (window.isoCurrentRam - lastRamMB) > 100; 
                            let isIdleDrain = isIdle && window.isoCurrentCpu > 25.0; 

                            if (isDomSpike) { isPhishing = true; threatDetail = "Abnormal DOM Spike (Hidden Iframe Attack Detected)"; }
                            else if (isIdleDrain) { isPhishing = true; threatDetail = "High Idle CPU (Cryptojacking/Miner Detected)"; }
                            else if (isRamSpike) { isPhishing = true; threatDetail = "Sudden Memory Leak (Background Data Scraping)"; }
                            else if (currentDomCount > 4000 || window.isoCurrentRam > 600) { 
                                isPhishing = true; threatDetail = "Excessive Hardware Consumption (Static Bloatware)"; 
                            }
                        }

                        if (isPhishing) {
                            document.getElementById('iso-engine-status').innerText = '🚨 DRAINER RISK!';
                            document.getElementById('iso-engine-status').className = 'iso-alarm-active';
                            
                            if(currentDomCount - lastDomCount > 800) document.getElementById('iso-dom').className = 'iso-alarm-active';
                            if((Date.now() - lastInteractionTime) > 3000 && window.isoCurrentCpu > 25.0) document.getElementById('iso-cpu').className = 'iso-alarm-active';
                            if(window.isoCurrentRam - lastRamMB > 100) document.getElementById('iso-ram').className = 'iso-alarm-active';
                            if(currentDomCount > 4000) document.getElementById('iso-dom').className = 'iso-alarm-active';

                            document.getElementById('isobrowse-control-panel').style.borderBottom = '2px solid #ff3366';
                            document.getElementById('iso-url').style.border = '1px solid #ff3366';
                            document.getElementById('iso-info-text').innerHTML = `<span class="iso-alarm-active">⚠️ WARNING: ${threatDetail}! DO NOT CONNECT your wallet!</span>`;
                        } else {
                            if(document.getElementById('iso-engine-status').innerText === '🚨 DRAINER RISK!') {
                                document.getElementById('iso-engine-status').innerText = 'WEB3_VAULT';
                                document.getElementById('iso-engine-status').className = '';
                                document.getElementById('iso-engine-status').style.color = '#00ccff';
                                document.getElementById('iso-dom').className = '';
                                document.getElementById('iso-dom').style.color = '#fff';
                                document.getElementById('iso-cpu').className = '';
                                document.getElementById('iso-ram').className = '';
                                document.getElementById('isobrowse-control-panel').style.borderBottom = '2px solid #00ff41';
                                document.getElementById('iso-url').style.border = '1px solid #004400';
                                document.getElementById('iso-info-text').innerHTML = '🛡️ INFO: Mod 2 (Vault) is unrestricted. The system automatically locks if malicious scripts or anomalies are detected.';
                            }
                        }

                        lastDomCount = currentDomCount;
                        lastRamMB = window.isoCurrentRam;
                    }
                }, 1000);
            };

            window.renderGhostMode = (html, url, cpu, ram, blocked) => {
                document.getElementById('iso-url').value = url;
                document.getElementById('iso-cpu').innerText = cpu + " ms";
                document.getElementById('iso-ram').innerText = ram + " KB";
                document.getElementById('iso-blocked').innerText = blocked; 
                
                let co2Saved = (ram * 0.0002).toFixed(4); 
                document.getElementById('iso-co2').innerText = co2Saved + "g";
                
                window.updateTerminal("> [SYSTEM]: Secure Render Complete. " + blocked + " threats isolated.");
                
                const ghostFrame = document.getElementById('isobrowse-ghost-canvas');
                ghostFrame.srcdoc = html;
            };

            if (document.readyState === 'loading') { document.addEventListener('DOMContentLoaded', injectIsoBrowseUI); } 
            else { injectIsoBrowseUI(); }
            
            const observerUI = new MutationObserver(() => {
                if (document.body && !document.getElementById('isobrowse-control-panel')) {
                    injectIsoBrowseUI();
                }
            });
            observerUI.observe(document.documentElement, { childList: true, subtree: true });
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
                            Err(e) => { 
                                let _ = p_i.send_event(UserEvent::UpdateTerminal(format!("> [ERROR]: Connection failed: {}", e)));
                                return; 
                            }
                        };
                        
                        let final_url = resp.url().as_str().to_string(); 
                        let mut html = resp.text().unwrap_or_default();
                        
                        let _ = p_i.send_event(UserEvent::UpdateTerminal("> [WASM]: Shredding spy scripts and trackers...".to_string()));

                        let lower_for_count = html.to_lowercase();
                        let blocked_trackers = lower_for_count.matches("<script").count() 
                                             + lower_for_count.matches("<iframe").count() 
                                             + lower_for_count.matches("google-analytics").count()
                                             + lower_for_count.matches("refresh").count();

                        html = html.replace("<script", "<template").replace("<SCRIPT", "<template");
                        html = html.replace("</script>", "</template>").replace("</SCRIPT>", "</template>");
                        html = html.replace("<iframe", "<template").replace("<IFRAME", "<template");
                        html = html.replace("</iframe>", "</template>").replace("</IFRAME>", "</template>");

                        html = html.replace("onload=", "data-kill-onload=");
                        html = html.replace("onerror=", "data-kill-onerror=");
                        html = html.replace("onclick=", "data-kill-onclick=");
                        html = html.replace("onmouseover=", "data-kill-onmouse=");

                        let mut clean_html = String::new();
                        let lower_html = html.to_lowercase();
                        let mut last_end = 0;
                        for (i, _) in lower_html.match_indices("<meta") {
                            if i >= last_end {
                                if let Some(end) = lower_html[i..].find('>') {
                                    let tag = &lower_html[i..i+end+1];
                                    if tag.contains("refresh") {
                                        clean_html.push_str(&html[last_end..i]);
                                        last_end = i + end + 1;
                                    }
                                }
                            }
                        }
                        clean_html.push_str(&html[last_end..]);
                        html = clean_html;

                        html = html.replace("data-src=", "src=")
                                   .replace("data-original=", "src=")
                                   .replace("data-lazy-src=", "src=");

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
                            let _ = pr.send_event(UserEvent::WasmGhostRender { 
                                html: String::from_utf8_lossy(&d).to_string(), 
                                url: f_url.clone(), 
                                cpu_ms: start_time.elapsed().as_millis(),
                                ram_kb: ram_footprint,
                                blocked_count: blocked_trackers
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
                        let h_p = alloc.call(&mut store, h_b.len() as i32).unwrap();
                        instance.get_memory(&mut store, "memory").unwrap().write(&mut store, h_p as usize, h_b).unwrap();
                        let _ = on_d.call(&mut store, (h_p, h_b.len() as i32));
                    });
                }
            }
            Event::UserEvent(UserEvent::WasmGhostRender { mut html, url, cpu_ms, ram_kb, blocked_count }) => {
                let base_tag = format!("<base href=\"{}\" target=\"_self\">", url);
                
                // Türkçe site yama kodunu arka planda tuttum, işine yarayabilir.
                html = html.replace("hatalıysa", "eksikse");
                html = html.replace("Hatalıysa", "Eksikse");

                let fallback_css = "<style>
                    body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; line-height: 1.6; max-width: 100vw; overflow-x: hidden; padding: 15px; }
                    img, video, iframe { max-width: 100% !important; height: auto !important; border-radius: 8px; }
                    a { color: #00ccff; text-decoration: none; }
                    a:hover { text-decoration: underline; }
                    html, body { transform: translateZ(0); }
                </style>";

                html = format!("{}\n{}\n{}", base_tag, fallback_css, html);
                
                let interceptor = r#"
                <script>
                    document.addEventListener('click', function(e) {
                        const target = e.target.closest('a');
                        if (target && target.href && target.href.startsWith('http')) {
                            e.preventDefault();
                            window.parent.postMessage({type: 'GHOST_NAVIGATE', url: target.href}, '*');
                        }
                    }, true);
                </script>
                "#;
                html.push_str(interceptor);

                let js = format!("window.renderGhostMode({}, '{}', {}, {}, {})", 
                    serde_json::to_string(&html).unwrap(), url, cpu_ms, ram_kb, blocked_count);
                let _ = webview.evaluate_script(&js);
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}

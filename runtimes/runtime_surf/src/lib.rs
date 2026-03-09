use std::slice;
use std::str;

extern "C" {
    fn send_to_ui(ptr: *const u8, len: usize);
    fn render_html(ptr: *const u8, len: usize);
}

#[no_mangle]
pub extern "C" fn on_data_received(ptr: *mut u8, len: usize) {
    let slice = unsafe { slice::from_raw_parts(ptr, len) };
    let raw_html = str::from_utf8(slice).unwrap_or("");
    
    let script_count = raw_html.to_lowercase().matches("<script").count();
    let iframe_count = raw_html.to_lowercase().matches("<iframe").count();
    
    let ghost_html = raw_html
        .replace("<script", "<template")
        .replace("</script>", "</template>")
        .replace("onclick", "data-blocked-click")
        .replace("target=\"_blank\"", "")
        .replace("target='_blank'", "")
        .replace("<iframe", "<iframe sandbox='' style='opacity: 0.3; pointer-events: none;' ");

    let interceptor_js = r#"
        <script>
            document.addEventListener('click', function(e) {
                const target = e.target.closest('a');
                if (target && target.href) {
                    e.preventDefault();
                    e.stopPropagation();
                    window.parent.postMessage({ type: 'SURF_NAVIGATE', url: target.href }, '*');
                }
            }, true);
            
            document.addEventListener('submit', function(e) {
                e.preventDefault();
                e.stopPropagation();
            }, true);
        </script>
    "#;

    // KUTUCUK (INFO BADGE) BURADAN SİLİNDİ! Sadece saf ve temizlenmiş HTML gönderilecek.
    let final_html = format!("{}{}", ghost_html, interceptor_js);

    let log_msg = format!("SURF_MODE // {} scripts, {} iframes locked down.", script_count, iframe_count);
    unsafe { send_to_ui(log_msg.as_ptr(), log_msg.len()); }

    unsafe { render_html(final_html.as_ptr(), final_html.len()); }
}

#[no_mangle] pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[no_mangle] pub extern "C" fn set_mode(_ptr: i32, _len: i32) {}

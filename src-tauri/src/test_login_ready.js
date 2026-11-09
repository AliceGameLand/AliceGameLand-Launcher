/* global document, window, MutationObserver */
window.addEventListener('DOMContentLoaded', () => {
    const login = document.getElementById('loginOptionsContainer')
    const loading = document.getElementById('loadingContainer')
    const button = document.getElementById('loginOptionMicrosoft')
    function verify() {
        if (!login?.getBoundingClientRect().height || !loading || window.getComputedStyle(loading).display !== 'none'
            || !button || button.disabled) return
        observer.disconnect()
        void window.__TAURI_INTERNALS__.invoke('test_login_ready')
    }
    const observer = new MutationObserver(verify)
    observer.observe(document.body, { attributes: true, subtree: true })
    verify()
})

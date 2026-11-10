/* global document, window, MutationObserver */
window.addEventListener('DOMContentLoaded', () => {
    const login = document.getElementById('loginOptionsContainer')
    const loading = document.getElementById('loadingContainer')
    const button = document.getElementById('loginOptionMicrosoft')
    const invoke = window.__TAURI_INTERNALS__.invoke
    const overlay = document.getElementById('overlayContainer')
    const message = document.getElementById('overlayDesc')
    let verifyCancellation = false
    function verify() {
        if (!login?.getBoundingClientRect().height || !loading || window.getComputedStyle(loading).display !== 'none'
            || !button || button.disabled) return
        if (verifyCancellation && (!overlay?.getBoundingClientRect().height
            || message?.textContent !== 'Microsoft 로그인 창이 닫혔어요.')) return
        observer.disconnect()
        if (verifyCancellation) {
            void invoke('test_login_ready', { cancelled: true })
            return
        }
        void invoke('test_login_ready').then(oauth => {
            if (!oauth) return
            verifyCancellation = true
            observer.observe(document.body, { attributes: true, subtree: true })
            button.click()
        })
    }
    const observer = new MutationObserver(verify)
    observer.observe(document.body, { attributes: true, subtree: true })
    verify()
})

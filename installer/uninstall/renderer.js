/* global document, window */
const api = window.uninstall
const byId = id => document.getElementById(id)
const close = byId('close')
const remove = byId('remove')
const version = new URLSearchParams(window.location.search).get('version')
if(version) byId('version').textContent = 'v' + version

function show(name){
    for(const screen of document.querySelectorAll('.screen')) screen.hidden = screen.id !== name
    byId(name).querySelector('h2').focus()
}

byId('minimize').onclick = () => api.minimize()
close.onclick = byId('cancel').onclick = byId('finish').onclick = () => api.close()
remove.onclick = async () => {
    if(remove.disabled) return
    remove.disabled = close.disabled = true
    byId('error').hidden = true
    show('progress')
    const result = await api.start().catch(() => ({ ok: false }))
    close.disabled = false
    if(result.ok){
        show('complete')
        return
    }
    byId('error').textContent = result.message || '런처를 제거하지 못했어요. 잠시 후 다시 시도해 주세요.'
    byId('error').hidden = false
    remove.textContent = '다시 시도'
    remove.disabled = false
    show('intro')
}

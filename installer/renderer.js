/* global document, window */
const api = window.setup
const byId = id => document.getElementById(id)
let info
let busy = false
let finished = false

function show(name){
    for(const screen of document.querySelectorAll('.screen')) screen.hidden = screen.id !== name
    byId(name).querySelector('h2')?.focus()
}
function controls(locked){
    busy = locked
    byId('close').disabled = locked
    byId('install').disabled = locked
    byId('browse').disabled = locked
}
function error(message){
    show('intro')
    byId('error').textContent = message
    byId('error').hidden = false
    controls(false)
    byId('install').textContent = '다시 시도'
    byId('install').focus()
}
api.info().then(value => {
    info = value
    byId('version').textContent = 'v' + value.version
    byId('destination').value = value.defaults.user
    if(value.updateError){ info = null; error(value.updateError); byId('install').disabled = true; return }
    if(value.updateTarget){
        byId('intro').querySelector('h2').textContent = '업데이트를 설치할까요?'
        byId('intro').querySelector('.description').textContent = '기존 설치 위치에 새 버전을 설치합니다.'
        byId('destination').value = value.updateTarget.destination
        document.querySelector('[name=mode][value='+value.updateTarget.mode+']').checked = true
        byId('scope').hidden = true
        byId('browse').hidden = true
        byId('install').textContent = '업데이트 설치'
    }
}).catch(() => error('설치 정보를 불러오지 못했습니다. 설치파일을 다시 실행해 주세요.'))
byId('browse').onclick = async () => {
    if(busy) return
    try {
        const folder = await api.folder(byId('destination').value)
        if(folder) byId('destination').value = folder
    } catch { error('이 폴더를 사용할 수 없습니다. 다른 설치 폴더를 선택해 주세요.') }
}
for(const radio of document.querySelectorAll('[name=mode]')) radio.onchange = () => {
    if(info) byId('destination').value = info.defaults[radio.value]
}
byId('install').onclick = async () => {
    if(busy || !info) return
    controls(true)
    byId('error').hidden = true
    byId('step-check').classList.remove('done')
    byId('step-check').setAttribute('aria-current', 'step')
    byId('step-install').removeAttribute('aria-current')
    byId('bar').hidden = false
    byId('bar').value = 0
    byId('indeterminate').hidden = true
    show('progress')
    try {
        const result = await api.install({ destination: byId('destination').value, mode: document.querySelector('[name=mode]:checked').value })
        if(!result.ok){ error(result.message); return }
        finished = true
        byId('installed-path').textContent = result.destination
        controls(false)
        show('complete')
    } catch { error('설치를 완료하지 못했습니다. 잠시 후 다시 시도해 주세요.') }
}
api.progress(value => {
    if(value.phase === 'checking'){
        byId('phase').textContent = '설치파일 확인 중'
        byId('bar').value = value.percent ?? 0
        byId('bar').hidden = false
        byId('indeterminate').hidden = true
        byId('percent').textContent = (value.percent ?? 0) + '%'
        byId('progress-note').textContent = '설치 파일을 확인하고 있습니다.'
    } else if(value.phase === 'installing'){
        byId('phase').textContent = '런처 설치 중'
        byId('bar').hidden = true
        byId('indeterminate').hidden = false
        byId('percent').textContent = ''
        byId('step-check').classList.add('done')
        byId('step-check').removeAttribute('aria-current')
        byId('step-install').setAttribute('aria-current', 'step')
        byId('progress-note').textContent = value.mode === 'all' ? 'Windows 권한 요청이 표시되면 승인해 주세요.' : '런처와 Java 파일을 준비하고 있습니다.'
    }
})
byId('launch').onclick = async () => {
    if(!finished) return
    try { await api.launch() } catch {
        byId('launch-error').textContent = '런처를 열지 못했습니다. 설치 폴더에서 실행해 주세요.'
        byId('launch-error').hidden = false
    }
}
byId('finish').onclick = () => api.close()
byId('close').onclick = () => { if(!busy) api.close() }
byId('minimize').onclick = () => api.minimize()

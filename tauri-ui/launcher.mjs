/* global document, window */
import serverArt from './assets/js/serverart.js.mjs'
import serverIcon from './assets/js/servericon.js.mjs'
import artworkTransition from './assets/js/artworktransition.js.mjs'

const byId = id => document.getElementById(id)
const { core, window: windowApi } = window.__TAURI__
const frame = windowApi.getCurrentWindow()
const transitionArtwork = artworkTransition.createArtworkTransition(document.querySelector('.home-art'))
const loginButton = byId('loginOptionMicrosoft')
const homeList = byId('homeServerList')
const setupList = byId('firstServerList')
const overlay = byId('overlayContainer')
const acknowledge = byId('overlayAcknowledge')
let needsServerSelection

for (const [suffix, action] of [['close', 'close'], ['minimize', 'minimize'], ['restoredown', 'toggleMaximize']]) {
    const control = byId('frameButton_' + suffix) ?? byId('frameButtonDarwin_' + suffix)
    control.addEventListener('click', () => frame[action]())
}
for (const area of document.querySelectorAll('.frame-brand, .frame-spacer')) {
    area.setAttribute('data-tauri-drag-region', '')
}

function showView(id) {
    for (const view of ['loginOptionsContainer', 'waitingContainer', 'serverSetupContainer', 'landingContainer']) {
        byId(view).style.display = view === id ? 'grid' : 'none'
    }
    byId('loadingContainer').hidden = true
    document.body.classList.remove('launcher-starting')
}

function showError(title, message, restoreFocus) {
    byId('overlayTitle').textContent = title
    byId('overlayDesc').textContent = typeof message === 'string' ? message : '잠시 후 다시 시도해 주세요.'
    acknowledge.textContent = '확인'
    const main = byId('main')
    function dismiss() {
        overlay.style.display = 'none'
        main.inert = false
        document.removeEventListener('keydown', onKey)
        restoreFocus?.focus()
    }
    function onKey(event) {
        if (event.key === 'Escape') dismiss()
    }
    acknowledge.onclick = dismiss
    document.addEventListener('keydown', onKey)
    main.inert = true
    overlay.style.display = 'grid'
    acknowledge.focus()
}

function setActiveServer(server) {
    byId('homeServerName').textContent = server.name
    const meta = byId('homeServerMeta')
    meta.textContent = 'Minecraft ' + server.minecraftVersion
    meta.hidden = false
    void transitionArtwork(serverArt.getServerBackground(server.id))
    for (const button of homeList.children) {
        button.setAttribute('aria-pressed', String(button.dataset.serverId === server.id))
    }
}

function showAuthenticated(profile) {
    byId('user_text').textContent = profile.name
    showView(needsServerSelection ? 'serverSetupContainer' : 'landingContainer')
    if (needsServerSelection) setupList.firstElementChild?.focus()
}

function makeChoice(server, firstChoice = false) {
    const item = document.createElement('button')
    item.type = 'button'
    item.className = 'home-server'
    item.dataset.serverId = server.id
    item.title = server.name
    const icon = document.createElement('img')
    icon.src = serverIcon.getServerIcon(server)
    icon.alt = ''
    icon.width = 36
    icon.height = 36
    icon.addEventListener('error', () => { icon.src = 'assets/images/SealCircle.png' }, { once: true })
    const name = document.createElement('span')
    name.textContent = server.name
    item.append(icon, name)
    if (firstChoice) {
        const ns = 'http://www.w3.org/2000/svg'
        const arrow = document.createElementNS(ns, 'svg')
        arrow.setAttribute('viewBox', '0 0 20 20')
        arrow.setAttribute('aria-hidden', 'true')
        const path = document.createElementNS(ns, 'path')
        path.setAttribute('d', 'm7.5 4.5 5.5 5.5-5.5 5.5')
        arrow.append(path)
        item.append(arrow)
    }

    item.addEventListener('click', async () => {
        const buttons = firstChoice ? setupList.children : homeList.children
        for (const button of buttons) button.disabled = true
        try {
            await core.invoke('select_server', { serverId: server.id })
            setActiveServer(server)
            if (firstChoice) {
                needsServerSelection = false
                showView('landingContainer')
                homeList.querySelector('[aria-pressed="true"]')?.focus()
            }
        } catch {
            showError('서버 선택을 저장하지 못했어요', '잠시 후 다시 시도해 주세요.', item)
        } finally {
            for (const button of buttons) button.disabled = false
        }
    })
    return item
}

try {
    const snapshot = await core.invoke('launcher_snapshot')
    const remembered = snapshot.servers.find(server => server.id === snapshot.selectedServer)
    const initial = remembered ?? snapshot.servers.find(server => server.mainServer) ?? snapshot.servers[0]
    if (!initial) throw new Error('Empty server distribution')
    needsServerSelection = !remembered
    homeList.replaceChildren(...snapshot.servers.map(server => makeChoice(server)))
    setupList.replaceChildren(...snapshot.servers.map(server => makeChoice(server, true)))
    setActiveServer(initial)
    byId('homeLauncherVersion').textContent = 'v' + snapshot.version

    loginButton.addEventListener('click', async () => {
        loginButton.disabled = true
        showView('waitingContainer')
        try {
            showAuthenticated(await core.invoke('microsoft_sign_in'))
        } catch (error) {
            showView('loginOptionsContainer')
            showError('Microsoft 로그인', error, loginButton)
        } finally {
            loginButton.disabled = false
        }
    })

    try {
        const profile = await core.invoke('account_profile')
        if (profile) showAuthenticated(profile)
        else showView('loginOptionsContainer')
    } catch (error) {
        showView('loginOptionsContainer')
        showError('저장된 계정을 확인하지 못했어요', error, loginButton)
    }
} catch {
    byId('loadingSubtitle').textContent = '런처 정보를 불러오지 못했습니다.'
    document.body.classList.remove('launcher-starting')
}

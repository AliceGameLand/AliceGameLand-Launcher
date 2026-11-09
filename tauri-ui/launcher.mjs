/* global document, window */
import serverArt from './assets/js/serverart.js.mjs'
import serverIcon from './assets/js/servericon.js.mjs'
import artworkTransition from './assets/js/artworktransition.js.mjs'

const byId = id => document.getElementById(id)
const { core, window: windowApi } = window.__TAURI__
const frame = windowApi.getCurrentWindow()
const transitionArtwork = artworkTransition.createArtworkTransition(document.querySelector('.home-art'))

for (const [suffix, action] of [['close', 'close'], ['minimize', 'minimize'], ['restoredown', 'toggleMaximize']]) {
    const control = byId('frameButton_' + suffix) ?? byId('frameButtonDarwin_' + suffix)
    control.addEventListener('click', () => frame[action]())
}
for (const area of document.querySelectorAll('.frame-brand, .frame-spacer')) {
    area.setAttribute('data-tauri-drag-region', '')
}

function selectServer(server, list) {
    byId('homeServerName').textContent = server.name
    const meta = byId('homeServerMeta')
    meta.textContent = 'Minecraft ' + server.minecraftVersion
    meta.hidden = false
    void transitionArtwork(serverArt.getServerBackground(server.id))
    for (const button of list.children) {
        button.setAttribute('aria-pressed', String(button.dataset.serverId === server.id))
    }
}

try {
    const snapshot = await core.invoke('launcher_snapshot')
    const list = byId('homeServerList')
    for (const server of snapshot.servers) {
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
        const name = document.createElement('span')
        name.textContent = server.name
        item.append(icon, name)
        item.addEventListener('click', async () => {
            try {
                await core.invoke('select_server', { serverId: server.id })
                selectServer(server, list)
            } catch {
                const status = byId('homeServerStatus')
                status.textContent = '서버 선택을 저장하지 못했습니다.'
                status.hidden = false
            }
        })
        list.append(item)
    }
    const server = snapshot.servers.find(candidate => candidate.id === snapshot.selectedServer)
        ?? snapshot.servers.find(candidate => candidate.mainServer)
        ?? snapshot.servers[0]
    if (!server) throw new Error('Empty server distribution')
    selectServer(server, list)
    byId('homeLauncherVersion').textContent = 'v' + snapshot.version
    byId('landingContainer').style.display = 'grid'
    byId('loadingContainer').hidden = true
} catch {
    byId('loadingSubtitle').textContent = '런처 정보를 불러오지 못했습니다.'
} finally {
    document.body.classList.remove('launcher-starting')
}

/* global document, window */
import serverArt from './assets/js/serverart.js.mjs'
import serverIcon from './assets/js/servericon.js.mjs'
import artworkTransition from './assets/js/artworktransition.js.mjs'
import { createJavaSettings } from './java-settings.mjs'

const byId = id => document.getElementById(id)
const { core, window: windowApi } = window.__TAURI__
const frame = windowApi.getCurrentWindow()
const transitionArtwork = artworkTransition.createArtworkTransition(document.querySelector('.home-art'))
const loginButton = byId('loginOptionMicrosoft')
const homeList = byId('homeServerList')
const setupList = byId('firstServerList')
const overlay = byId('overlayContainer')
const acknowledge = byId('overlayAcknowledge')
const playButton = byId('launch_button')
const launchDetails = byId('launch_details')
const homeStatus = byId('homeServerStatus')
let needsServerSelection
let activeServer
let activeProfile
let visibleView
let statusRequest = 0
let selectedGameDirectory = null
const javaSettings = createJavaSettings(document)
const settingsTabs = [...document.querySelectorAll('.settingsNavItem')]
const launcherSettings = byId('settingsTabLauncher')
const gameDirectory = launcherSettings.querySelector('[cValue=DataDirectory]')
const chooseGameDirectory = launcherSettings.querySelector('.settingsFileSelButton')
const allowPrerelease = launcherSettings.querySelector('[cValue=AllowPrerelease]')
const gameFields = {
    resWidth: byId('settingsGameWidth'),
    resHeight: byId('settingsGameHeight'),
    fullscreen: document.querySelector('[cValue=Fullscreen]'),
    autoConnect: document.querySelector('[cValue=AutoConnect]'),
    launchDetached: document.querySelector('[cValue=LaunchDetached]')
}
const gameDefaults = { resWidth: 1280, resHeight: 720, fullscreen: false, autoConnect: true, launchDetached: true }

for (const [suffix, action] of [['close', 'close'], ['minimize', 'minimize'], ['restoredown', 'toggleMaximize']]) {
    const control = byId('frameButton_' + suffix) ?? byId('frameButtonDarwin_' + suffix)
    control.addEventListener('click', () => frame[action]())
}
for (const area of document.querySelectorAll('.frame-brand, .frame-spacer')) {
    area.setAttribute('data-tauri-drag-region', '')
}

function showView(id) {
    visibleView = id
    byId('main').style.display = 'block'
    for (const view of ['loginOptionsContainer', 'waitingContainer', 'serverSetupContainer', 'landingContainer', 'settingsContainer']) {
        byId(view).style.display = view === id ? 'grid' : 'none'
    }
    byId('frameHomeActions').hidden = id !== 'landingContainer'
    byId('loadingContainer').hidden = true
    document.body.classList.remove('launcher-starting')
    if (id === 'landingContainer') void refreshServerStatus()
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
    activeServer = server
    byId('homeServerName').textContent = server.name
    const meta = byId('homeServerMeta')
    meta.textContent = 'Minecraft ' + server.minecraftVersion
    meta.hidden = false
    void transitionArtwork(serverArt.getServerBackground(server.id))
    for (const button of homeList.children) {
        button.setAttribute('aria-pressed', String(button.dataset.serverId === server.id))
    }
    homeStatus.dataset.state = 'checking'
    homeStatus.textContent = '상태 확인 중…'
    if (visibleView === 'landingContainer') void refreshServerStatus()
}

async function refreshServerStatus() {
    if (visibleView !== 'landingContainer' || !activeServer) return
    const request = ++statusRequest
    homeStatus.hidden = false
    const players = await core.invoke('server_status', { serverId: activeServer.id }).catch(() => null)
    if (request !== statusRequest) return
    homeStatus.dataset.state = players ? 'online' : 'unknown'
    homeStatus.textContent = players ? `접속 ${players.online}/${players.max}명` : '접속 상태 확인 불가'
}

window.setInterval(refreshServerStatus, 5 * 60 * 1000)

function showAuthenticated(profile) {
    activeProfile = profile
    byId('user_text').textContent = profile.name
    showView(needsServerSelection ? 'serverSetupContainer' : 'landingContainer')
    if (needsServerSelection) setupList.firstElementChild?.focus()
}

function selectSettingsTab(id) {
    for (const tab of settingsTabs) {
        const active = tab.getAttribute('rSc') === id
        tab.toggleAttribute('selected', active)
        tab.setAttribute('aria-current', String(active))
    }
    for (const pane of document.querySelectorAll('.settingsTab')) {
        pane.style.display = pane.id === id ? '' : 'none'
    }
}

function bindProjectPage(link, page) {
    async function open(event) {
        event.preventDefault()
        try {
            await core.invoke('open_project_page', { page })
        } catch (error) {
            showError('링크를 열지 못했어요', error, link)
        }
    }
    link.addEventListener('click', open)
    link.addEventListener('auxclick', event => {
        if (event.button === 1) void open(event)
    })
}

function bindAbout(version) {
    byId('settingsAboutCurrentVersionValue').textContent = version
    const about = byId('settingsTabAbout')
    bindProjectPage(byId('settingsAboutSourceButton'), 'source')
    bindProjectPage(byId('settingsAboutSupportButton'), 'issues')
    bindProjectPage(byId('reportIssueLink'), 'issues')
    bindProjectPage(byId('launchFailureReport'), 'issues')
    bindProjectPage(about.querySelector('.settingsChangelogButton'), 'release')
    const status = byId('mojang_status_text')
    const list = byId('mojangServiceList')
    const details = about.querySelector('.settings-service-status')
    const labels = { red: '장애', yellow: '지연', green: '정상', grey: '확인 불가' }
    let refreshedAt = 0
    details.addEventListener('toggle', async () => {
        if (!details.open || Date.now() - refreshedAt < 60_000) return
        refreshedAt = Date.now()
        status.textContent = '상태 확인 중…'
        status.dataset.state = 'grey'
        list.replaceChildren()
        const services = await core.invoke('authentication_status').catch(() => [])
        const state = Object.keys(labels).find(key => services.some(service => service.state === key)) ?? 'grey'
        status.dataset.state = state
        status.textContent = labels[state]
        list.replaceChildren(...services.map(service => {
            const item = document.createElement('li')
            const name = document.createElement('span')
            const value = document.createElement('span')
            name.textContent = service.name
            value.dataset.state = service.state
            value.textContent = labels[service.state]
            item.append(name, value)
            return item
        }))
    })
}

function showSettingsAccount() {
    const container = byId('settingsCurrentMicrosoftAccounts')
    const row = document.createElement('div')
    row.className = 'settingsAuthAccount'
    const body = document.createElement('div')
    body.className = 'settingsAuthAccountRight'
    const details = document.createElement('div')
    details.className = 'settingsAuthAccountDetails'
    const name = document.createElement('span')
    name.className = 'settingsAuthAccountDetailValue'
    name.textContent = activeProfile.name
    const actions = document.createElement('div')
    actions.className = 'settingsAuthAccountActions'
    const signOut = document.createElement('button')
    signOut.type = 'button'
    signOut.className = 'settingsAuthAccountLogOut'
    signOut.textContent = '로그아웃'
    signOut.addEventListener('click', async () => {
        signOut.disabled = true
        try {
            await core.invoke('microsoft_sign_out')
            activeProfile = null
            showView('loginOptionsContainer')
            loginButton.focus()
        } catch (error) {
            showError('로그아웃하지 못했어요', error, signOut)
        } finally {
            signOut.disabled = false
        }
    })
    details.append(name)
    actions.append(signOut)
    body.append(details, actions)
    row.append(body)
    container.replaceChildren(row)
}

async function openSettings(id, source) {
    source.disabled = true
    try {
        const [saved, java] = await Promise.all([
            core.invoke('game_settings'),
            core.invoke('java_settings', { serverId: activeServer.id })
        ])
        javaSettings.load(activeServer, java)
        for (const [key, field] of Object.entries(gameFields)) {
            const value = saved.game[key] ?? gameDefaults[key]
            if (field.type === 'checkbox') field.checked = value
            else field.value = value
        }
        selectedGameDirectory = null
        gameDirectory.value = saved.launcher.dataDirectory ?? ''
        allowPrerelease.checked = saved.launcher.allowPrerelease ?? false
        showSettingsAccount()
        selectSettingsTab(id)
        showView('settingsContainer')
        settingsTabs.find(tab => tab.getAttribute('rSc') === id)?.focus()
    } catch (error) {
        showError('설정을 열지 못했어요', error, source)
    } finally {
        source.disabled = false
    }
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
    bindAbout(snapshot.version)

    const settingsButton = byId('settingsMediaButton')
    const accountButton = byId('avatarOverlay')
    const settingsDone = byId('settingsNavDone')
    settingsButton.addEventListener('click', () => openSettings('settingsTabMinecraft', settingsButton))
    accountButton.addEventListener('click', () => openSettings('settingsTabAccount', accountButton))
    for (const tab of settingsTabs) {
        const id = tab.getAttribute('rSc')
        tab.disabled = !['settingsTabAccount', 'settingsTabMinecraft', 'settingsTabJava', 'settingsTabLauncher', 'settingsTabAbout'].includes(id)
        tab.addEventListener('click', () => selectSettingsTab(id))
    }
    allowPrerelease.disabled = true
    launcherSettings.querySelector('.settingsFileSelDesc').textContent = '게임 파일은 이 폴더에 저장돼요. 폴더를 바꾸면 기존 파일은 원래 위치에 남아요.'
    chooseGameDirectory.addEventListener('click', async () => {
        chooseGameDirectory.disabled = true
        settingsDone.disabled = true
        try {
            const directory = await core.invoke('choose_game_folder')
            if (directory) {
                selectedGameDirectory = directory
                gameDirectory.value = directory
            }
        } catch (error) {
            showError('게임 폴더를 선택하지 못했어요', error, chooseGameDirectory)
        } finally {
            chooseGameDirectory.disabled = false
            settingsDone.disabled = false
        }
    })
    const changeAccount = byId('settingsAddMicrosoftAccount')
    changeAccount.textContent = '다른 계정으로 로그인'
    changeAccount.addEventListener('click', async event => {
        const button = event.currentTarget
        button.disabled = true
        try {
            activeProfile = await core.invoke('microsoft_sign_in')
            byId('user_text').textContent = activeProfile.name
            showSettingsAccount()
        } catch (error) {
            showError('Microsoft 로그인', error, button)
        } finally {
            button.disabled = false
        }
    })
    byId('settingsTabJava').querySelector('.settingsSwitchServerButton').addEventListener('click', () => settingsDone.click())
    settingsDone.addEventListener('click', async () => {
        const settings = Object.fromEntries(Object.entries(gameFields).map(([key, field]) => [
            key, field.type === 'checkbox' ? field.checked : field.valueAsNumber
        ]))
        settingsDone.disabled = true
        try {
            await core.invoke('save_game_settings', {
                settings,
                java: javaSettings.change(),
                dataDirectory: selectedGameDirectory
            })
            showView('landingContainer')
            settingsButton.focus()
        } catch (error) {
            showError('게임 설정을 저장하지 못했어요', error, settingsDone)
        } finally {
            settingsDone.disabled = false
        }
    })
    const openFolder = byId('openGameFolderButton')
    openFolder.disabled = false
    openFolder.addEventListener('click', async () => {
        try {
            await core.invoke('open_game_folder')
        } catch (error) {
            showError('게임 폴더를 열지 못했어요', error, openFolder)
        }
    })
    playButton.disabled = false
    playButton.addEventListener('click', async () => {
        const server = activeServer
        const progress = new core.Channel()
        progress.onmessage = message => { byId('launch_details_text').textContent = message }
        for (const button of homeList.children) button.disabled = true
        playButton.disabled = true
        byId('launch_content').hidden = true
        byId('launch_progress').removeAttribute('value')
        byId('launch_progress_label').hidden = true
        launchDetails.hidden = false
        try {
            await core.invoke('play_server', { serverId: server.id, progress })
        } catch (error) {
            showError('게임을 시작하지 못했어요', error, playButton)
        } finally {
            launchDetails.hidden = true
            byId('launch_content').hidden = false
            playButton.disabled = false
            for (const button of homeList.children) button.disabled = false
        }
    })
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

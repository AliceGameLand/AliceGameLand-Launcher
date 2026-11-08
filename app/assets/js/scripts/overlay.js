/**
 * Launcher dialogs and their keyboard focus.
 */
const overlayContainer = document.getElementById('overlayContainer')
const overlayMain = document.getElementById('main')
const serverPickerList = document.getElementById('serverSelectListScrollable')
const serverPickerConfirm = document.getElementById('serverSelectConfirm')
const accountPickerList = document.getElementById('accountSelectListScrollable')
let overlayDialog
let overlayDismissable
let overlayReturnFocus

function isOverlayVisible(){
    return overlayDialog != null
}

function toggleOverlay(state = !isOverlayVisible(), dismissable = false, content = 'overlayContent', returnFocus = document.activeElement){
    const frameActions = document.getElementById('frameHomeActions')
    if(state){
        if(!isOverlayVisible()) overlayReturnFocus = returnFocus
        overlayDialog = document.getElementById(content)
        overlayDismissable = dismissable
        for(const panel of overlayContainer.querySelectorAll('[role="dialog"]')){
            panel.style.display = panel === overlayDialog ? 'block' : 'none'
        }
        document.getElementById('overlayDismiss').style.display = dismissable ? 'inline-flex' : 'none'
        overlayMain.inert = true
        frameActions.inert = true
        overlayContainer.style.display = 'grid'
        const initial = Array.from(overlayDialog.querySelectorAll('[aria-pressed="true"], .overlayKeybindEnter:enabled'))
            .find(control => control.getClientRects().length)
        const focusTarget = initial ?? overlayDialog.querySelector('.overlayKeybindEsc') ?? overlayDialog
        focusTarget.focus()
    } else {
        overlayDialog = null
        overlayContainer.style.display = 'none'
        overlayMain.inert = false
        frameActions.inert = false
        overlayReturnFocus?.focus()
        overlayReturnFocus = null
    }
}

document.addEventListener('keydown', event => {
    if(!isOverlayVisible()) return
    if(event.key === 'Escape'){
        event.preventDefault()
        event.stopImmediatePropagation()
        if(overlayDismissable) overlayDialog.querySelector('.overlayKeybindEsc')?.click()
    } else if(event.key === 'Tab'){
        const controls = Array.from(overlayDialog.querySelectorAll('button:enabled, a[href], summary'))
            .filter(control => control.getClientRects().length)
        const index = controls.indexOf(document.activeElement)
        if(index === -1 || (event.shiftKey ? index === 0 : index === controls.length - 1)){
            event.preventDefault()
            const target = (event.shiftKey ? controls.at(-1) : controls[0]) ?? overlayDialog
            target.focus()
        }
    }
}, true)

function setOverlayContent(title, description, acknowledge, dismiss = Lang.queryJS('overlay.dismiss')){
    document.getElementById('overlayTitle').innerHTML = title
    document.getElementById('overlayDesc').innerHTML = description
    document.getElementById('overlayAcknowledge').textContent = acknowledge
    document.getElementById('overlayDismiss').textContent = dismiss
}

function setOverlayHandler(handler){
    document.getElementById('overlayAcknowledge').onclick = handler ?? (() => toggleOverlay(false))
}

function setDismissHandler(handler){
    document.getElementById('overlayDismiss').onclick = handler ?? (() => toggleOverlay(false))
}

function selectDialogEntry(list, entry){
    list.querySelector('[aria-pressed="true"]')?.setAttribute('aria-pressed', 'false')
    entry.setAttribute('aria-pressed', 'true')
}

async function toggleServerSelection(state){
    if(!state){
        toggleOverlay(false)
        return
    }
    if(launchInProgress) return
    const opener = document.activeElement
    const distro = await DistroAPI.getDistribution()
    if(launchInProgress || !distro.servers.length) return
    const selected = ConfigManager.getSelectedServer()

    serverPickerList.replaceChildren(...distro.servers.map(({ rawServer }) => {
        const button = document.createElement('button')
        button.type = 'button'
        button.className = 'serverListing'
        button.dataset.serverId = rawServer.id
        button.setAttribute('aria-pressed', String(rawServer.id === selected || (selected == null && rawServer.id === distro.getMainServer()?.rawServer.id)))

        const icon = document.createElement('img')
        icon.className = 'serverListingImg'
        icon.alt = ''
        icon.src = require('./assets/js/servericon').getServerIcon(rawServer)
        icon.addEventListener('error', () => { icon.src = 'assets/images/SealCircle.png' }, { once: true })

        const details = document.createElement('span')
        details.className = 'serverListingDetails'
        const name = document.createElement('strong')
        name.textContent = rawServer.name
        const description = document.createElement('span')
        description.textContent = rawServer.description || `Minecraft ${rawServer.minecraftVersion}`
        const revision = document.createElement('span')
        revision.textContent = rawServer.mainServer
            ? `${rawServer.version} · ${Lang.queryJS('settings.serverListing.mainServer')}`
            : rawServer.version
        details.append(name, description, revision)
        button.append(icon, details)
        button.onclick = () => selectDialogEntry(serverPickerList, button)
        return button
    }))
    serverPickerConfirm.onclick = async () => {
        const entry = serverPickerList.querySelector('[aria-pressed="true"]') ?? serverPickerList.firstElementChild
        serverPickerConfirm.disabled = true
        try {
            if(entry && await selectLauncherServer(entry.dataset.serverId, distro)) toggleOverlay(false)
        } finally {
            serverPickerConfirm.disabled = false
        }
    }
    toggleOverlay(true, true, 'serverSelectContent', opener)
}

document.getElementById('serverSelectCancel').onclick = () => toggleOverlay(false)

function prepareAccountSelectionList(){
    const accounts = Object.values(ConfigManager.getAuthAccounts())
    const current = ConfigManager.getSelectedAccount()?.uuid ?? accounts[0]?.uuid
    accountPickerList.replaceChildren(...accounts.map(account => {
        const button = document.createElement('button')
        button.type = 'button'
        button.className = 'accountListing'
        button.dataset.uuid = account.uuid
        button.setAttribute('aria-pressed', String(account.uuid === current))
        const icon = document.createElement('img')
        icon.alt = ''
        icon.src = `https://mc-heads.net/head/${encodeURIComponent(account.uuid)}/40`
        const name = document.createElement('span')
        name.textContent = account.displayName
        button.append(icon, name)
        button.onclick = () => selectDialogEntry(accountPickerList, button)
        return button
    }))
}

document.getElementById('accountSelectConfirm').onclick = async () => {
    const entry = accountPickerList.querySelector('[aria-pressed="true"]')
    if(!entry) return
    const account = ConfigManager.setSelectedAccount(entry.dataset.uuid)
    ConfigManager.save()
    updateSelectedAccount(account)
    if(getCurrentView() === VIEWS.settings) await prepareSettings()
    toggleOverlay(false)
    await validateSelectedAccount()
}

document.getElementById('accountSelectCancel').onclick = () => toggleOverlay(true, true)

setOverlayHandler(null)
setDismissHandler(null)

/**
 * Home view and game preparation.
 */
const { MojangRestAPI, getServerStatus } = require('helios-core/mojang')
const { RestResponseStatus, isDisplayableError } = require('helios-core/common')
const { FullRepair, DistributionIndexProcessor, MojangIndexProcessor } = require('helios-core/dl')
const { validateSelectedJvm, ensureJavaDirIsRoot } = require('helios-core/java')
const { ensureDir } = require('fs-extra')

const DiscordWrapper    = require('./assets/js/discordwrapper')
const NeoForgeInstaller = require('./assets/js/neoforgeinstaller')
const ProcessBuilder    = require('./assets/js/processbuilder')

const launchButton      = document.getElementById('launch_button')
const launchContent     = document.getElementById('launch_content')
const launchDetails     = document.getElementById('launch_details')
const launchProgress    = document.getElementById('launch_progress')
const launchPercent     = document.getElementById('launch_progress_label')
const launchDetailsText = document.getElementById('launch_details_text')
const homeServerList    = document.getElementById('homeServerList')
const homeServerName    = document.getElementById('homeServerName')
const homeServerMeta    = document.getElementById('homeServerMeta')
const homeServerStatus  = document.getElementById('homeServerStatus')
const openGameFolderButton = document.getElementById('openGameFolderButton')
const loggerLanding     = LoggerUtil.getLogger('Landing')

let homeDistribution
let statusRequest = 0
let launchInProgress = false
let gameFolderOpening = false
let proc

function refreshLaunchControls(){
    openGameFolderButton.disabled = gameFolderOpening || ConfigManager.getSelectedServer() == null
    launchButton.disabled = launchInProgress || proc != null || ConfigManager.getSelectedServer() == null
    for(const button of homeServerList.querySelectorAll('button')){
        button.disabled = launchInProgress
    }
    for(const button of document.querySelectorAll('.settingsSwitchServerButton')){
        button.disabled = launchInProgress || !homeDistribution?.servers.length
    }
}

function toggleLaunchArea(loading){
    launchInProgress = loading
    launchContent.hidden = loading
    launchDetails.hidden = !loading
    refreshLaunchControls()
    if(!loading) remote.getCurrentWindow().setProgressBar(-1)
}

function setLaunchDetails(details){
    launchDetailsText.textContent = details
}

function setLaunchPercentage(percent){
    launchProgress.value = Math.round(percent)
    launchPercent.textContent = launchProgress.value + '%'
}

function setDownloadPercentage(percent){
    remote.getCurrentWindow().setProgressBar(percent / 100)
    setLaunchPercentage(percent)
}

launchButton.addEventListener('click', async () => {
    if(launchButton.disabled) return
    const serverId = ConfigManager.getSelectedServer()
    setLaunchDetails(Lang.queryJS('landing.launch.pleaseWait'))
    setLaunchPercentage(0)
    toggleLaunchArea(true)

    try {
        const server = (await DistroAPI.getDistribution()).getServerById(serverId)
        if(server == null){
            showLaunchFailure(Lang.queryJS('landing.launch.failureTitle'), Lang.queryJS('landing.launch.serverUnavailable'))
            return
        }
        const jExe = BundledJava.getExecutable()
        if(jExe == null){
            loggerLanding.error('Bundled Java executable is missing.', BundledJava.getRuntimeRoot())
            showLaunchFailure(
                Lang.queryJS('landing.launch.bundledJavaMissingTitle'),
                Lang.queryJS('landing.launch.bundledJavaMissingText')
            )
            return
        }

        const details = await validateSelectedJvm(ensureJavaDirIsRoot(jExe), server.effectiveJavaOptions.supported)
        if(details == null){
            loggerLanding.error('Bundled Java validation failed.', jExe)
            showLaunchFailure(
                Lang.queryJS('landing.launch.bundledJavaInvalidTitle'),
                Lang.queryJS('landing.launch.bundledJavaInvalidText')
            )
            return
        }

        if(ConfigManager.getJavaExecutable(serverId) !== jExe){
            ConfigManager.setJavaExecutable(serverId, jExe)
            ConfigManager.save()
        }
        settingsJavaExecVal.value = jExe
        await dlAsync(serverId)
    } catch(err) {
        loggerLanding.error('Unhandled error during launch.', err)
        showLaunchFailure(Lang.queryJS('landing.launch.failureTitle'), Lang.queryJS('landing.launch.failureText'))
    }
})

async function openLauncherSettings(tabId){
    await prepareSettings()
    switchView(getCurrentView(), VIEWS.settings, 160, 160, () => {
        if(tabId) settingsNavItemListener(document.getElementById(tabId), false)
    }, () => {
        document.querySelector('.settingsNavItem[selected]')?.focus()
    })
}

document.getElementById('settingsMediaButton').onclick = () => openLauncherSettings()
document.getElementById('avatarOverlay').onclick = () => openLauncherSettings('settingsNavAccount')
document.getElementById('launcherUpdateButton').onclick = () => openLauncherSettings('settingsNavUpdate')
document.getElementById('homeLauncherVersion').textContent = 'v' + remote.app.getVersion()

// Open the selected instance, including before its first game download.
openGameFolderButton.onclick = async () => {
    if(openGameFolderButton.disabled) return
    const directory = path.join(ConfigManager.getInstanceDirectory(), ConfigManager.getSelectedServer())
    gameFolderOpening = true
    refreshLaunchControls()
    try {
        await ensureDir(directory)
        const error = await shell.openPath(directory)
        if(error) throw new Error(error)
    } catch(err) {
        loggerLanding.warn('Unable to open the selected game directory.', err)
        setOverlayContent(
            Lang.queryJS('landing.tools.folderErrorTitle'),
            Lang.queryJS('landing.tools.folderErrorMessage'),
            Lang.queryJS('landing.launch.okay')
        )
        setOverlayHandler(null)
        toggleOverlay(true, false, 'overlayContent', openGameFolderButton)
    } finally {
        gameFolderOpening = false
        refreshLaunchControls()
    }
}

function updateSelectedAccount(authUser){
    const avatar = document.getElementById('avatarContainer')
    const uuid = authUser?.uuid ?? ''
    avatar.dataset.accountUuid = uuid
    avatar.style.backgroundImage = ''
    if(uuid){
        void AccountAvatar.getHeadUrl(uuid, 40).then(url => {
            if(avatar.dataset.accountUuid === uuid) avatar.style.backgroundImage = `url('${url}')`
        })
    }
    document.getElementById('user_text').textContent =
        authUser?.displayName ?? Lang.queryJS('landing.selectedAccount.noAccountSelected')
}
updateSelectedAccount(ConfigManager.getSelectedAccount())

function renderHomeServerList(distro){
    const buttons = distro.servers.map(({ rawServer }) => {
        const button = document.createElement('button')
        button.type = 'button'
        button.className = 'home-server'
        button.dataset.serverId = rawServer.id

        const icon = document.createElement('img')
        icon.alt = ''
        icon.width = 36
        icon.height = 36
        const fallbackIcon = 'assets/images/SealCircle.png'
        icon.src = rawServer.icon || fallbackIcon
        icon.addEventListener('error', () => { icon.src = fallbackIcon }, { once: true })

        const label = document.createElement('span')
        label.textContent = rawServer.name
        button.title = rawServer.name
        button.append(icon, label)
        button.onclick = () => selectLauncherServer(rawServer.id, distro)
        return button
    })
    homeServerList.replaceChildren(...buttons)
}

// Both selectors use the same save path. A launch owns its server until preparation finishes.
async function selectLauncherServer(id, distro){
    if(launchInProgress) return false
    const server = distro.getServerById(id)
    if(!server) return false
    await updateSelectedServer(server, distro)
    return true
}

function updateSelectedServer(server, distro){
    const id = server?.rawServer.id ?? null
    const changed = ConfigManager.getSelectedServer() !== id
    const inSettings = getCurrentView() === VIEWS.settings
    if(changed){
        if(inSettings) fullSettingsSave()
        ConfigManager.setSelectedServer(id)
        ConfigManager.save()
    }
    if(homeDistribution !== distro){
        homeDistribution = distro
        renderHomeServerList(distro)
    }
    for(const button of homeServerList.querySelectorAll('button')){
        button.setAttribute('aria-pressed', String(button.dataset.serverId === id))
    }

    homeServerName.textContent = server?.rawServer.name ?? Lang.queryJS('landing.selectedServer.noSelection')
    const loader = server?.modules.find(({ rawModule }) =>
        [Type.NeoForge, Type.Fabric, Type.Forge, Type.ForgeHosted].includes(rawModule.type)
    )?.rawModule.type
    homeServerMeta.textContent = server
        ? [`Minecraft ${server.rawServer.minecraftVersion}`, loader === Type.ForgeHosted ? Type.Forge : loader].filter(Boolean).join(' · ')
        : ''
    homeServerMeta.hidden = !server
    homeServerStatus.hidden = !server
    homeServerStatus.dataset.state = 'checking'
    homeServerStatus.textContent = Lang.queryJS('landing.serverStatus.checking')
    refreshLaunchControls()
    syncLauncherDiscordPresence(distro, server)
    void refreshServerStatus(server)
    if(inSettings && changed) return animateSettingsTabRefresh()
}

async function refreshMojangStatuses(){
    let statuses = MojangRestAPI.getDefaultStatuses()
    try {
        const response = await MojangRestAPI.status()
        if(response.responseStatus === RestResponseStatus.SUCCESS && response.data?.length){
            statuses = response.data
        }
    } catch(err) {
        loggerLanding.warn('Unable to refresh Mojang service status.', err)
    }

    const status = statuses.some(service => service.status === 'red') ? 'red'
        : statuses.some(service => service.status === 'yellow') ? 'yellow'
            : statuses.some(service => service.status === 'green') ? 'green' : 'grey'

    const serviceList = document.getElementById('mojangServiceList')
    const items = statuses.map(service => {
        const item = document.createElement('li')
        const name = document.createElement('span')
        const state = document.createElement('span')
        name.textContent = service.name
        state.dataset.state = service.status
        state.textContent = Lang.queryJS('landing.services.' + (['red', 'yellow', 'green'].includes(service.status) ? service.status : 'grey'))
        item.append(name, state)
        return item
    })
    serviceList.replaceChildren(...items)
    document.getElementById('mojang_status_text').textContent = Lang.queryJS('landing.services.' + status)
}

async function refreshServerStatus(server){
    const request = ++statusRequest
    if(!server) return

    let state = 'unknown'
    let message = Lang.queryJS('landing.serverStatus.unavailable')
    try {
        const { players } = await getServerStatus(47, server.hostname, server.port)
        if(Number.isFinite(players?.online) && Number.isFinite(players?.max)){
            state = 'online'
            message = Lang.queryJS('landing.serverStatus.population', { online: players.online, max: players.max })
        }
    } catch(err) {
        loggerLanding.debug('Server status unavailable.', err)
    }
    if(request !== statusRequest) return
    homeServerStatus.dataset.state = state
    homeServerStatus.textContent = message
}

void refreshMojangStatuses()
setInterval(refreshMojangStatuses, 60 * 60 * 1000)
setInterval(() => {
    void refreshServerStatus(homeDistribution?.getServerById(ConfigManager.getSelectedServer()))
}, 5 * 60 * 1000)

function showLaunchFailure(title, desc){
    toggleLaunchArea(false)
    setOverlayContent(title, desc, Lang.queryJS('landing.launch.okay'))
    setOverlayHandler(null)
    toggleOverlay(true, false, 'overlayContent', getCurrentView() === VIEWS.landing ? launchButton : document.activeElement)
}


// Discord presence follows the selected server until a game starts.
let hasRPC = false
let discordPresenceMode = null
let discordPresenceServerId = null

function syncLauncherDiscordPresence(distro, serv){
    if(proc != null) return

    const genSettings = distro?.rawDistribution.discord
    const servSettings = serv?.rawServer.discord
    if(genSettings == null || servSettings == null){
        if(hasRPC) DiscordWrapper.shutdownRPC()
        hasRPC = false
        discordPresenceMode = null
        discordPresenceServerId = null
        return
    }

    const serverId = serv.rawServer.id
    if(discordPresenceMode === 'launcher' && discordPresenceServerId === serverId) return

    DiscordWrapper.initRPC(genSettings, servSettings, Lang.queryJS('discord.launcher'))
    hasRPC = true
    discordPresenceMode = 'launcher'
    discordPresenceServerId = serverId
}
const GAME_READY_REGEX = /\]: Sound engine started/
const SERVER_CONNECTING_REGEX = /ConnectScreen\/\]: Connecting to /
const GAME_LAUNCH_REGEX = /^\[.+\]: (?:MinecraftForge .+ Initialized|ModLauncher .+ starting: .+|Loading Minecraft .+ with Fabric Loader .+)$/
const MIN_LINGER = 5000

async function dlAsync(serverId) {
    const loggerLaunchSuite = LoggerUtil.getLogger('LaunchSuite')

    setLaunchDetails(Lang.queryJS('landing.dlAsync.loadingServerInfo'))

    let distro

    try {
        distro = await DistroAPI.refreshDistributionOrFallback()
        onDistroRefresh(distro)
    } catch(err) {
        loggerLaunchSuite.error('Unable to refresh distribution index.', err)
        showLaunchFailure(Lang.queryJS('landing.dlAsync.fatalError'), Lang.queryJS('landing.dlAsync.unableToLoadDistributionIndex'))
        return
    }

    const serv = distro.getServerById(serverId)
    if(!serv){
        showLaunchFailure(Lang.queryJS('landing.launch.failureTitle'), Lang.queryJS('landing.launch.serverUnavailable'))
        return
    }

    const authUser = ConfigManager.getSelectedAccount()
    if(!authUser){
        showLaunchFailure(Lang.queryJS('landing.launch.failureTitle'), Lang.queryJS('landing.launch.accountRequired'))
        return
    }

    const fullRepairModule = new FullRepair(
        ConfigManager.getCommonDirectory(),
        ConfigManager.getInstanceDirectory(),
        ConfigManager.getLauncherDirectory(),
        serverId,
        DistroAPI.isDevMode()
    )

    fullRepairModule.spawnReceiver()

    const receiver = fullRepairModule.childProcess
    // The worker's validation and download promises only settle when it sends a reply.
    const workerFailure = new Promise((_, reject) => {
        receiver.once('error', reject)
        receiver.once('close', code => reject(new Error(`File worker exited with code ${code}.`)))
    })

    let failureKey = 'landing.dlAsync.errorDuringFileVerificationTitle'
    try {
        setLaunchDetails(Lang.queryJS('landing.dlAsync.validatingFileIntegrity'))
        const invalidFileCount = await Promise.race([fullRepairModule.verifyFiles(setLaunchPercentage), workerFailure])
        setLaunchPercentage(100)
        if(invalidFileCount > 0){
            failureKey = 'landing.dlAsync.errorDuringFileDownloadTitle'
            setLaunchDetails(Lang.queryJS('landing.dlAsync.downloadingFiles'))
            setLaunchPercentage(0)
            await Promise.race([fullRepairModule.download(setDownloadPercentage), workerFailure])
            setDownloadPercentage(100)
        }
    } catch (err) {
        loggerLaunchSuite.error('Unable to prepare game files.', err)
        const retry = Lang.queryJS('landing.dlAsync.retryFilePreparation')
        showLaunchFailure(Lang.queryJS(failureKey), err.displayable ? `${retry}<br><br>${err.displayable}` : retry)
        return
    } finally {
        if(receiver.connected) fullRepairModule.destroyReceiver()
        remote.getCurrentWindow().setProgressBar(-1)
    }

    setLaunchDetails(Lang.queryJS('landing.dlAsync.preparingToLaunch'))

    try {
        setLaunchDetails(Lang.queryJS('landing.dlAsync.installingNeoForge'))
        await NeoForgeInstaller.ensureInstalled(serv)
    } catch(err) {
        loggerLaunchSuite.error('Failed to install the NeoForge client patch.', err)
        showLaunchFailure(Lang.queryJS('landing.dlAsync.errorDuringLaunchTitle'), err.message)
        return
    }

    const mojangIndexProcessor = new MojangIndexProcessor(
        ConfigManager.getCommonDirectory(),
        serv.rawServer.minecraftVersion)
    const distributionIndexProcessor = new DistributionIndexProcessor(
        ConfigManager.getCommonDirectory(),
        distro,
        serv.rawServer.id
    )

    const modLoaderData = await distributionIndexProcessor.loadModLoaderVersionJson(serv)
    const versionData = await mojangIndexProcessor.getVersionJson()

    loggerLaunchSuite.info(`Sending selected account (${authUser.displayName}) to ProcessBuilder.`)
    let pb = new ProcessBuilder(serv, versionData, modLoaderData, authUser, remote.app.getVersion())
    setLaunchDetails(Lang.queryJS('landing.dlAsync.launchingGame'))

    let launchResetTimer = null
    let launchAreaReset = false
    const onLoadComplete = () => {
        if(launchAreaReset || proc == null){
            return
        }
        launchAreaReset = true
        if(launchResetTimer != null){
            clearTimeout(launchResetTimer)
            launchResetTimer = null
        }
        toggleLaunchArea(false)
        if(hasRPC){
            DiscordWrapper.updateDetails(Lang.queryJS('landing.discord.loading'))
            proc.stdout.on('data', gameStateChange)
        }
        proc.stdout.removeListener('data', tempListener)
        proc.stderr.removeListener('data', gameErrorListener)
    }
    const start = Date.now()

    // Attach a temporary listener to the client output.
    // Will wait for a certain bit of text meaning that
    // the client application has started, and we can hide
    // the progress bar stuff.
    const tempListener = function(data){
        if(GAME_LAUNCH_REGEX.test(data.trim())){
            const diff = Date.now()-start
            if(diff < MIN_LINGER) {
                if(launchResetTimer != null){
                    clearTimeout(launchResetTimer)
                }
                launchResetTimer = setTimeout(onLoadComplete, MIN_LINGER-diff)
            } else {
                onLoadComplete()
            }
        }
    }

    // Listener for Discord RPC.
    const gameStateChange = function(data){
        data = data.trim()
        if(SERVER_CONNECTING_REGEX.test(data)){
            DiscordWrapper.updateDetails(Lang.queryJS('landing.discord.joining'))
        } else if(GAME_READY_REGEX.test(data)){
            DiscordWrapper.updateDetails(Lang.queryJS('landing.discord.joined'))
        }
    }

    const gameErrorListener = function(data){
        data = data.trim()
        if(data.indexOf('Could not find or load main class net.minecraft.launchwrapper.Launch') > -1){
            loggerLaunchSuite.error('Game launch failed, LaunchWrapper was not downloaded properly.')
            showLaunchFailure(Lang.queryJS('landing.dlAsync.errorDuringLaunchTitle'), Lang.queryJS('landing.dlAsync.launchWrapperNotDownloaded'))
        }
    }

    try {
        // Build Minecraft process.
        proc = pb.build()

        // Bind listeners to stdout.
        proc.stdout.on('data', tempListener)
        proc.stderr.on('data', gameErrorListener)

        setLaunchDetails(Lang.queryJS('landing.dlAsync.doneEnjoyServer'))
        launchResetTimer = setTimeout(onLoadComplete, MIN_LINGER)

        proc.once('close', () => {
            if(launchResetTimer != null) clearTimeout(launchResetTimer)
            proc = null
            if(!launchAreaReset) toggleLaunchArea(false)
            else refreshLaunchControls()
            syncLauncherDiscordPresence(homeDistribution, homeDistribution.getServerById(ConfigManager.getSelectedServer()))
        })

        // Init Discord Hook
        if(distro.rawDistribution.discord != null && serv.rawServer.discord != null){
            DiscordWrapper.initRPC(distro.rawDistribution.discord, serv.rawServer.discord)
            hasRPC = true
            discordPresenceMode = 'game'
            discordPresenceServerId = serv.rawServer.id
        }

    } catch(err) {

        loggerLaunchSuite.error('Error during launch', err)
        showLaunchFailure(Lang.queryJS('landing.dlAsync.errorDuringLaunchTitle'), Lang.queryJS('landing.dlAsync.checkConsoleForDetails'))

    }
}

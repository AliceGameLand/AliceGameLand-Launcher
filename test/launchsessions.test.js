const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const vm = require('node:vm')
const { EventEmitter } = require('node:events')

const source = fs.readFileSync(path.join(__dirname, '../app/assets/js/scripts/landing.js'), 'utf8')

function createHarness(){
    const processes = []
    const timers = new Map()
    const resets = []
    const failures = []
    const presence = []
    const servers = new Map(['one', 'two'].map(id => [id, { rawServer: { id, minecraftVersion: '26.2' } }]))
    const distribution = { getServerById: id => servers.get(id), rawDistribution: {} }
    class Repair {
        spawnReceiver(){ this.childProcess = Object.assign(new EventEmitter(), { connected: true }) }
        async verifyFiles(){ return 0 }
        destroyReceiver(){ this.childProcess.connected = false }
    }
    const context = vm.createContext({
        runningGames: new Map(), launchAttempt: 1,
        require: { resolve: () => 'fixture-cache-module' },
        LoggerUtil: { getLogger: () => ({ info(){}, error(){} }) },
        Lang: { queryJS: key => key },
        ConfigManager: {
            getSelectedAccount: () => ({ displayName: 'Test' }),
            getCommonDirectory: () => '', getInstanceDirectory: () => '', getLauncherDirectory: () => ''
        },
        DistroAPI: { refreshDistributionOrFallback: async () => distribution, isDevMode: () => false },
        onDistroRefresh(){}, FullRepair: Repair,
        NeoForgeInstaller: { ensureInstalled: async () => {} },
        MojangIndexProcessor: class { async getVersionJson(){ return {} } },
        DistributionIndexProcessor: class { async loadModLoaderVersionJson(){ return {} } },
        ProcessBuilder: class {
            build(){
                const child = Object.assign(new EventEmitter(), { stdout: new EventEmitter(), stderr: new EventEmitter() })
                processes.push(child)
                return child
            }
        },
        remote: { app: { getVersion: () => 'test' }, getCurrentWindow: () => ({ setProgressBar(){} }) },
        setLaunchDetails(){}, setLaunchPercentage(){}, setDownloadPercentage(){}, setLaunchIndeterminate(){},
        toggleLaunchArea: state => resets.push(state), refreshLaunchControls(){},
        showLaunchFailure: (...args) => failures.push(args),
        syncGameDiscordPresence: () => presence.push(context.runningGames.size),
        hasRPC: false, DiscordWrapper: { updateDetails(){} },
        setTimeout: callback => { const id = {}; timers.set(id, callback); return id },
        clearTimeout: id => timers.delete(id),
        MIN_LINGER: 5000, GAME_LAUNCH_REGEX: /game ready/,
        SERVER_CONNECTING_REGEX: /connecting/, GAME_READY_REGEX: /sound ready/
    })
    vm.runInContext(source.slice(source.indexOf('async function dlAsync(')), context)
    return { context, processes, resets, failures, presence, ready: () => Array.from(timers.values()).at(-1)() }
}

test('Closing an earlier game preserves a later game and its preparation area', async () => {
    const h = createHarness()
    await vm.runInContext('dlAsync("one",1)', h.context)
    h.ready()
    assert.deepEqual(h.resets, [false])
    h.context.launchAttempt = 2
    await vm.runInContext('dlAsync("two",2)', h.context)
    assert.equal(h.context.runningGames.size, 2)
    h.processes[0].emit('close', 0)
    assert.equal(h.context.runningGames.size, 1)
    assert(h.context.runningGames.has(h.processes[1]))
    assert.deepEqual(h.resets, [false])
    h.ready()
    h.processes[1].emit('close', 0)
    assert.equal(h.context.runningGames.size, 0)
    assert.deepEqual(h.presence, [1, 2, 1, 0])
})

test('A later process startup error leaves an earlier game tracked and permits recovery', async () => {
    const h = createHarness()
    await vm.runInContext('dlAsync("one",1)', h.context)
    h.ready()
    h.context.launchAttempt = 2
    await vm.runInContext('dlAsync("two",2)', h.context)
    h.processes[1].emit('error', new Error('Spawn failed'))
    h.processes[1].emit('close', -1)
    assert.equal(h.context.runningGames.size, 1)
    assert(h.context.runningGames.has(h.processes[0]))
    assert.equal(h.failures.length, 1)
    assert.deepEqual(h.resets, [false])
})

test('Running games allow another launch while preparation and missing accounts remain blocked', () => {
    const button = {}, label = {}, serverButton = {}, pickerButton = {}
    const context = vm.createContext({
        openGameFolderButton: {}, gameFolderOpening: false,
        launchButton: button, launchButtonLabel: label, launchInProgress: false,
        runningGames: new Map([[{}, {}]]), homeDistribution: { servers: [{}] },
        ConfigManager: { getSelectedServer: () => 'one', getSelectedAccount: () => ({}), isServerSelectionRequired: () => false },
        Lang: { queryJS: key => key }, homeServerList: { querySelectorAll: () => [serverButton] },
        document: { querySelectorAll: () => [pickerButton], getElementById: () => ({}) }
    })
    vm.runInContext(source.slice(source.indexOf('function refreshLaunchControls('), source.indexOf('function toggleLaunchArea(')), context)
    vm.runInContext('refreshLaunchControls()', context)
    assert.equal(button.disabled, false)
    assert.equal(label.textContent, 'landing.launch.play')
    context.ConfigManager.isServerSelectionRequired = () => true
    vm.runInContext('refreshLaunchControls()', context)
    assert.equal(button.disabled, true)
    context.ConfigManager.isServerSelectionRequired = () => false
    context.launchInProgress = true
    vm.runInContext('refreshLaunchControls()', context)
    assert.equal(button.disabled, true)
    assert.equal(serverButton.disabled, true)
    context.launchInProgress = false
    context.ConfigManager.getSelectedAccount = () => null
    vm.runInContext('refreshLaunchControls()', context)
    assert.equal(button.disabled, true)
})

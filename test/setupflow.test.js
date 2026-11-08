const { test } = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const vm = require('node:vm')
const { EventEmitter } = require('node:events')
const { createHash } = require('node:crypto')
const { promisify } = require('node:util')

// Exercise the actual main process without opening windows or invoking an installer.
async function fixture(t, options = {}){
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'alice-setup-flow-'))
    t.after(() => fs.rmSync(directory, { recursive: true, force: true }))
    fs.mkdirSync(path.join(directory, 'payload'))
    const bytes = Buffer.alloc(150000, 37)
    fs.writeFileSync(path.join(directory, 'payload/launcher.exe'), bytes)
    const handlers = new Map()
    const reports = []
    let window, spawned = null
    class Window extends EventEmitter {
        constructor(config){ super(); this.config = config; window = this; this.webContents = new EventEmitter(); this.webContents.setWindowOpenHandler = () => {}; this.webContents.send = (_channel, value) => reports.push(value) }
        removeMenu(){} loadFile(){} show(){} focus(){} minimize(){} close(){ this.closed = true }
    }
    const app = new EventEmitter()
    app.requestSingleInstanceLock = () => true
    app.whenReady = () => Promise.resolve()
    app.quit = () => {}
    const execute = () => {}
    execute[promisify.custom] = async command => ({ stdout: command === 'powershell.exe' ? JSON.stringify(options.existing ? [{ icon:'D:\\기존 런처\\앨리스의 게임랜드.exe,0', mode:'user' }] : []) : options.running ? '"????.exe","1234","Console","1","30,000 K"' : 'INFO: No tasks are running' })
    const electron = { app, BrowserWindow: Window, ipcMain: { handle: (name, fn) => handlers.set(name, fn), on: (name, fn) => handlers.set(name, fn) }, screen: { getPrimaryDisplay: () => ({ workAreaSize: { width:1280, height:720 } }) }, dialog: {}, shell: { openPath: async () => '' } }
    const childProcess = {
        execFile: execute,
        spawn: (file, args, config) => {
            spawned = { file, args, config }
            const child = new EventEmitter()
            setImmediate(() => {
                if(!options.fail){
                    const target = args.at(-1).slice(3)
                    fs.mkdirSync(target, { recursive: true })
                    fs.writeFileSync(path.join(target, 'QA-Launcher.exe'), 'fixture')
                }
                child.emit('close', options.fail ? 1 : 0)
            })
            return child
        }
    }
    const manifest = { version:'1.0.19', executable:'QA-Launcher.exe', sha256:options.badHash ? 'invalid' : createHash('sha256').update(bytes).digest('hex') }
    const runtimeFs = { ...fs, existsSync: file => options.existing && file === 'D:\\기존 런처\\앨리스의 게임랜드.exe' ? true : fs.existsSync(file) }
    const req = name => name === 'node:fs' ? runtimeFs : name === 'electron' ? electron : name === './payload.json' ? manifest : name === './update-target.cjs' ? require('../installer/update-target.cjs') : name === './paths.cjs' ? require('../installer/paths.cjs') : name === 'node:child_process' ? childProcess : require(name)
    vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../installer/main.cjs'), 'utf8'), { require:req, __dirname:path.join(__dirname,'../installer'), process:{ env:process.env, resourcesPath:directory }, console })
    await new Promise(resolve => setImmediate(resolve))
    const invoke = (name, value) => handlers.get('setup:' + name)({ sender:window.webContents }, value)
    return { invoke, handlers, get window(){ return window }, reports, destination:path.join(directory, '게임 폴더'), get spawned(){ return spawned } }
}
test('Manual reinstall detects the existing location and selects shortcut-preserving update mode', async t => {
    const f = await fixture(t, { existing:true })
    const info = f.invoke('info')
    assert.equal(info.updateTarget.destination, 'D:\\기존 런처')
    assert.equal(info.updateTarget.mode, 'user')
    const { installerArguments } = require('../installer/paths.cjs')
    assert(installerArguments(info.updateTarget.destination, info.updateTarget.mode, true).includes('--updated'))
    assert.equal(f.spawned, null)
})

test('custom setup uses a sandboxed frameless window and rejects foreign IPC', async t => {
    const f = await fixture(t)
    assert.equal(f.window.config.frame, false)
    assert.equal(f.window.config.webPreferences.sandbox, true)
    assert.equal(f.window.config.webPreferences.nodeIntegration, false)
    assert.throws(() => f.handlers.get('setup:info')({ sender:{} }), /Unknown/)
})
test('custom setup verifies payload, serializes writes and launches only its installed file', async t => {
    const f = await fixture(t)
    assert.equal(await f.invoke('launch'), false)
    const pending = f.invoke('install', { destination:f.destination, mode:'user' })
    assert.equal((await f.invoke('install', { destination:f.destination })).ok, false)
    const closeEvent = { prevented:false, preventDefault(){ this.prevented = true } }
    f.window.emit('close', closeEvent)
    assert.equal(closeEvent.prevented, true)
    const result = await pending
    assert.equal(result.ok, true)
    assert.equal(f.spawned.args[0], '/S')
    assert.equal(f.spawned.args.at(-1), '/D=' + result.destination)
    assert.equal(f.spawned.config.windowsVerbatimArguments, true)
    assert.ok(f.reports.some(value => value.phase === 'checking' && value.percent === 100))
    assert.ok(f.reports.some(value => value.phase === 'installing' && value.percent === undefined))
    assert.equal(await f.invoke('launch'), true)
})
test('custom setup leaves running launchers alone and unlocks after a rejected start', async t => {
    const f = await fixture(t, { running:true })
    const result = await f.invoke('install', { destination:f.destination })
    assert.equal(result.ok, false)
    assert.match(result.message, /실행 중인 런처/)
    assert.equal(f.spawned, null)
    f.invoke('close')
    assert.equal(f.window.closed, true)
})
test('custom setup refuses corrupt payloads and permits retry after backend errors', async t => {
    const corrupt = await fixture(t, { badHash:true })
    assert.equal((await corrupt.invoke('install', { destination:corrupt.destination })).ok, false)
    assert.equal(corrupt.spawned, null)
    const failed = await fixture(t, { fail:true })
    for(let attempt = 0; attempt < 2; attempt++){
        assert.equal((await failed.invoke('install', { destination:failed.destination })).ok, false)
        assert.equal(await failed.invoke('launch'), false)
    }
})

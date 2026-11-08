const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const vm = require('node:vm')
const path = require('node:path')

test('Production updates never auto download or install on quit and require separate actions', async () => {
    let handler
    let downloads = 0
    let installs = 0
    const updater = { autoDownload:true, autoInstallOnAppQuit:true, setFeedURL(){}, on(){}, downloadUpdate(){ downloads++; return Promise.resolve() }, quitAndInstall(){ installs++ } }
    const context = vm.createContext({ require:()=>class {}, autoUpdater:updater, isDev:false, process:{ platform:'win32' }, console:{ log(){} }, ipcMain:{ on(_name, fn){ handler = fn } } })
    const source = fs.readFileSync(path.join(__dirname,'../index.js'),'utf8')
    vm.runInContext(source.slice(source.indexOf('function initAutoUpdater'),source.indexOf('// Redirect distribution index')),context)
    const event = { sender:{ send(){} } }
    handler(event,'initAutoUpdater',false)
    assert.equal(updater.autoDownload,false)
    assert.equal(updater.autoInstallOnAppQuit,false)
    assert.equal(downloads,0)
    assert.equal(installs,0)
    handler(event,'downloadUpdate')
    await Promise.resolve()
    assert.equal(downloads,1)
    assert.equal(installs,0)
    handler(event,'installUpdateNow')
    assert.equal(installs,1)
})


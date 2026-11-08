const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const vm = require('node:vm')

function setup(){
    const elements = new Map()
    const element = id => {
        if(!elements.has(id)) elements.set(id, { style: {}, getElementsByClassName: name => [element(name)] })
        return elements.get(id)
    }
    let listener
    const sent = []
    const visible = []
    const context = vm.createContext({
        document: { getElementById: element },
        Lang: { queryJS: key => key },
        remote: { app: { getVersion: () => '1.0.13' } },
        isDev: false, process: { platform: 'win32' },
        isPrerelease: () => false, populateVersionInformation: () => {},
        loggerAutoUpdater: { info: () => {}, error: () => {}, debug: () => {} },
        ipcRenderer: { on: (name, handler) => { listener = handler }, send: (...args) => sent.push(args) },
        setInterval: () => 1, showUpdateUI: (value = true) => visible.push(value)
    })
    const settings = fs.readFileSync(path.join(__dirname, '../app/assets/js/scripts/settings.js'), 'utf8')
    vm.runInContext(settings.slice(settings.indexOf('const settingsTabUpdate '), settings.indexOf('async function prepareSettings')), context)
    const core = fs.readFileSync(path.join(__dirname, '../app/assets/js/scripts/uicore.js'), 'utf8')
    vm.runInContext(core.slice(core.indexOf('let updateCheckListener'), core.indexOf('function changeAllowPrerelease')), context)
    return { context, element, sent, visible, notify: (state, info) => listener(null, state, info) }
}

test('Windows announces an update before download completes and enables install only afterwards', () => {
    const ui = setup()
    const info = { version: '1.0.21', releaseName: 'Update', releaseNotes: '' }
    ui.notify('update-available', info)
    assert.deepEqual(ui.visible, [true])
    assert.equal(ui.element('settingsUpdateActionButton').disabled, false)
    assert.equal(ui.element('settingsUpdateActionButton').innerHTML, 'settings.updates.startDownloadButton')
    assert.deepEqual(ui.sent, [])
    ui.element('settingsUpdateActionButton').onclick()
    assert.equal(ui.element('settingsUpdateActionButton').disabled, true)
    ui.notify('update-downloaded', info)
    assert.equal(ui.element('settingsUpdateActionButton').disabled, false)
    ui.element('settingsUpdateActionButton').onclick()
    assert.deepEqual(ui.sent, [['autoUpdateAction', 'downloadUpdate'], ['autoUpdateAction', 'installUpdateNow']])
})

test('Latest result clears stale release details and allows another check', () => {
    const ui = setup()
    vm.runInContext('prepareUpdateTab()', ui.context)
    assert.equal(ui.element('settingsUpdateTitle').innerHTML, 'settings.updates.checkUpdatesTitle')
    ui.notify('checking-for-update')
    assert.equal(ui.element('settingsUpdateActionButton').disabled, true)
    ui.notify('update-not-available')
    assert.equal(ui.element('settingsUpdateActionButton').innerHTML, 'settings.updates.latestVersionButton')
    assert.equal(ui.element('settingsUpdateActionButton').disabled, false)
    assert.equal(ui.element('settingsChangelogContainer').style.display, 'none')
    ui.element('settingsUpdateActionButton').onclick()
    assert.deepEqual(ui.sent, [['autoUpdateAction', 'checkForUpdate']])
    assert.equal(ui.element('settingsUpdateActionButton').disabled, true)
})

test('Failed check restores the retry action instead of staying disabled', () => {
    const ui = setup()
    ui.notify('checking-for-update')
    ui.notify('realerror')
    assert.equal(ui.element('settingsUpdateTitle').innerHTML, 'settings.updates.checkFailedTitle')
    assert.equal(ui.element('settingsUpdateActionButton').innerHTML, 'settings.updates.retryButton')
    assert.equal(ui.element('settingsUpdateActionButton').disabled, false)
    ui.element('settingsUpdateActionButton').onclick()
    assert.deepEqual(ui.sent, [['autoUpdateAction', 'checkForUpdate']])
})
test('Update download displays actual percent and bytes, survives tab refresh and clears on completion/error', () => {
    const ui = setup()
    const info = { version:'1.0.37', releaseName:'Update', releaseNotes:'' }
    ui.notify('update-available', info)
    ui.element('settingsUpdateActionButton').onclick()
    assert.equal(ui.element('settingsUpdateDownload').hidden, false)
    ui.notify('download-progress', { percent:42.7, transferred:42 * 1048576, total:100 * 1048576 })
    assert.equal(ui.element('settingsUpdateDownloadPercent').textContent, '42%')
    assert.equal(ui.element('settingsUpdateDownloadBar').value, 42.7)
    assert.equal(ui.element('settingsUpdateDownloadSize').textContent, '42.0 MB / 100.0 MB')
    vm.runInContext('populateSettingsUpdateInformation({version:"1.0.37"})', ui.context)
    assert.equal(ui.element('settingsUpdateActionButton').disabled, true)
    ui.notify('update-downloaded', info)
    assert.equal(ui.element('settingsUpdateDownload').hidden, true)
    ui.notify('update-available', info)
    ui.element('settingsUpdateActionButton').onclick()
    ui.notify('realerror')
    assert.equal(ui.element('settingsUpdateDownload').hidden, true)
    assert.equal(ui.element('settingsUpdateActionButton').disabled, false)
})

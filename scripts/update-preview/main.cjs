const { app, BrowserWindow, ipcMain, dialog } = require('electron')
const fs = require('node:fs')
const path = require('node:path')
const os = require('node:os')
const { pathToFileURL } = require('node:url')
const assert = require('node:assert/strict')
const root = path.resolve(__dirname, '../..')
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'alice-update-preview-'))
const verify = process.argv.includes('--verify')
process.env.ELECTRON_IS_DEV = '0'
process.env.ALICE_LOCAL_DISTRIBUTION = '1'
process.env.ALICE_QA_ROOT = root
process.env.APPDATA = profile
app.setPath('appData', profile)
app.setPath('userData', profile)
app.setAppPath(root)
const metadata = require('../../package.json')
app.getVersion = () => metadata.version
const remote = require('@electron/remote/main')
remote.initialize()
const ejs = require('ejs-electron')
const lang = require('../../app/assets/js/langloader')
lang.setupLanguage()
ejs.data('lang', lang.queryEJS)
const distroRoot = path.resolve(root, '../AliceGameLand-Distribution')
const distro = JSON.parse(fs.readFileSync(path.join(distroRoot, 'distribution.json'), 'utf8'))
const icons = ['server-icons/diamond.png','release-assets/server-icon.png','server-icons/burcompany-pvp.png','release-assets/server-icon.png','server-icons/crime-scene.png']
distro.servers.forEach((s,i) => { s.icon = pathToFileURL(path.join(distroRoot, icons[i] || icons[0])).href })
const uuid = '11111111111111111111111111111111'
fs.writeFileSync(path.join(profile, 'distribution_dev.json'), JSON.stringify(distro))
fs.writeFileSync(path.join(profile, 'config.json'), JSON.stringify({ selectedServer:distro.servers[1]?.id || distro.servers[0]?.id, selectedAccount:uuid, authenticationDatabase:{ [uuid]:{ uuid, username:'UpdatePreview', displayName:'UpdatePreview', type:'microsoft', accessToken:'preview', expiresAt:Date.now()+86400000 } }, settings:{ launcher:{ dataDirectory:path.join(profile,'data') } } }))
let win, timer, installed = false
const notify = (name, info) => { if(!win.isDestroyed()) win.webContents.send('autoUpdateNotification', name, info) }
const info = { version:require('semver').inc(metadata.version,'patch'), releaseName:'업데이트 화면 체험 — 테스트 데이터', releaseNotes:'<p>실제 업데이트가 아닌 화면 체험입니다. 다운로드와 재설치는 실행되지 않습니다.</p><p>설치하기를 누르면 체험 완료 안내가 표시됩니다.</p>' }
// Reuse the production renderer notifications; never load electron-updater here.
ipcMain.on('autoUpdateAction', (_event, action) => {
    if(action === 'initAutoUpdater') setTimeout(() => notify('ready'), 500)
    if(action === 'checkForUpdate'){
        clearTimeout(timer)
        notify('checking-for-update')
        timer = setTimeout(() => {
            notify('update-available', info)
        }, 1200)
    }
    if(action === 'downloadUpdate') timer = setTimeout(() => notify('update-downloaded', info), 1800)
    if(action === 'installUpdateNow'){
        installed = true
        notify('update-not-available')
        if(!verify) void dialog.showMessageBox(win, { type:'info', title:'업데이트 체험 완료', message:'설치 버튼까지 정상적으로 연결되었습니다.', detail:'실제 다운로드나 재설치는 실행하지 않았습니다. 업데이트 확인을 누르면 다시 체험할 수 있습니다.' })
    }
})
ipcMain.on('distributionIndexDone', (event,res) => event.sender.send('distributionIndexDone',res))
app.on('window-all-closed', () => app.quit())
app.whenReady().then(async () => {
    win = new BrowserWindow({ width:1280, height:720, frame:false, show:!verify, title:'앨리스의 게임랜드 — 업데이트 체험', webPreferences:{ preload:path.join(__dirname,'preload.cjs'), nodeIntegration:true, contextIsolation:false, offscreen:verify } })
    remote.enable(win.webContents)
    await win.loadURL(pathToFileURL(path.join(root,'app/app.ejs')).href)
    await win.webContents.executeJavaScript(`document.querySelector('.frame-brand span').textContent += ' · 업데이트 체험'`)
    if(!verify) return
    const js = code => win.webContents.executeJavaScript(code)
    const waitFor = async code => { for(let i=0;i<200;i++){ if(await js(code)) return; await new Promise(r=>setTimeout(r,50)) } throw new Error('Preview timed out: '+code) }
    await waitFor(`!document.getElementById('launcherUpdateButton').hidden`)
    const folder = path.join(root,'.impeccable/review/update-preview')
    fs.mkdirSync(folder,{recursive:true})
    fs.writeFileSync(path.join(folder,'alert.png'),(await win.webContents.capturePage()).toPNG())
    const boxes = await js(`[...document.querySelectorAll('#frameButtonDockWin button')].map(b=>({height:b.getBoundingClientRect().height,width:b.getBoundingClientRect().width,radius:getComputedStyle(b).borderRadius}))`)
    assert.ok(boxes.every(b=>b.height>=40 && b.height===boxes[0].height && b.width===38 && b.radius==='0px'))
    win.webContents.debugger.attach('1.3')
    const dom = await win.webContents.debugger.sendCommand('DOM.getDocument')
    for(const action of ['minimize','restoredown','close']){
        const { nodeId } = await win.webContents.debugger.sendCommand('DOM.querySelector',{ nodeId:dom.root.nodeId, selector:'#frameButton_'+action })
        await win.webContents.debugger.sendCommand('CSS.enable')
        await win.webContents.debugger.sendCommand('CSS.forcePseudoState',{ nodeId, forcedPseudoClasses:['hover'] })
        await new Promise(r=>setTimeout(r,200))
        fs.writeFileSync(path.join(folder,action+'-hover.png'),(await win.webContents.capturePage({x:1166,y:0,width:114,height:48})).toPNG())
        await win.webContents.debugger.sendCommand('CSS.forcePseudoState',{ nodeId, forcedPseudoClasses:[] })
    }
    await js(`document.getElementById('launcherUpdateButton').click()`)
    await waitFor(`document.getElementById('settingsTabUpdate').getClientRects().length > 0 && document.getElementById('settingsContainer').getClientRects().length > 0`)
    await waitFor(`document.getElementById('settingsUpdateActionButton').textContent === '다운로드'`)
    await js(`document.getElementById('settingsUpdateActionButton').click()`)
    await waitFor(`document.getElementById('settingsUpdateActionButton').textContent.includes('설치')`)
    await new Promise(r=>setTimeout(r,300))
    fs.writeFileSync(path.join(folder,'ready.png'),(await win.webContents.capturePage()).toPNG())
    await js(`document.getElementById('settingsUpdateActionButton').click()`)
    await new Promise(r=>setTimeout(r,100))
    assert.equal(installed,true)
    assert.equal(await js(`document.getElementById('launcherUpdateButton').hidden`),true)
    console.log('Preview alert -> update tab -> install -> completion verified; frame buttons fill title bar, square corners')
    app.exit(0)
}).catch(error => { console.error(error); app.exit(1) })

const { app, BrowserWindow, ipcMain, dialog, shell, screen } = require('electron')
const fs = require('node:fs')
const path = require('node:path')
const { spawn, execFile } = require('node:child_process')
const { promisify } = require('node:util')
const { createHash } = require('node:crypto')
const { validateDestination, installerArguments } = require('./paths.cjs')
const { findUpdateTarget } = require('./update-target.cjs')

const execute = promisify(execFile)
let window
let installing = false
let installedExecutable
let updateTarget
let updateError
const manifest = require('./payload.json')
const defaults = {
    user: path.join(process.env.LOCALAPPDATA, 'Programs', 'alice-gameland-launcher'),
    all: path.join(process.env.ProgramFiles, 'alice-gameland-launcher')
}

// Only the bundled renderer can initiate an installation or choose a path.
function trusted(event){
    if(event.sender !== window?.webContents) throw new Error('Unknown setup window')
}
function report(phase, detail = {}){
    window?.webContents.send('setup:progress', { phase, ...detail })
}

if(!app.requestSingleInstanceLock()) app.quit()
else {
    app.on('second-instance', () => { window?.show(); window?.focus() })
    app.whenReady().then(async () => {
        if(process.argv?.includes('--updated')){
            try { updateTarget = await findUpdateTarget(execute, fs.existsSync) }
            catch(error){ updateError = error.message }
        }
        const area = screen.getPrimaryDisplay().workAreaSize
        window = new BrowserWindow({
            width: Math.min(720, area.width - 24), height: Math.min(520, area.height - 24),
            minWidth: 640, minHeight: 480, frame: false, resizable: true,
            backgroundColor: '#F8F4EA', title: '앨리스의 게임랜드 설치',
            icon: path.join(__dirname, 'assets/icon.png'), show: false,
            webPreferences: { preload: path.join(__dirname, 'preload.cjs'), contextIsolation: true, nodeIntegration: false, sandbox: true }
        })
        window.removeMenu()
        window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))
        window.webContents.on('will-navigate', event => event.preventDefault())
        window.on('close', event => { if(installing) event.preventDefault() })
        window.once('ready-to-show', () => window.show())
        window.loadFile(path.join(__dirname, 'index.html'))
    })
}
app.on('window-all-closed', () => app.quit())

ipcMain.handle('setup:info', event => {
    trusted(event)
    return { version: manifest.version, defaults, updateTarget, updateError }
})
ipcMain.handle('setup:folder', async (event, current) => {
    trusted(event)
    if(installing) return null
    const result = await dialog.showOpenDialog(window, { title: '설치할 폴더 선택', defaultPath: current, properties: ['openDirectory', 'createDirectory'] })
    return result.canceled ? null : validateDestination(result.filePaths[0])
})
ipcMain.on('setup:minimize', event => { trusted(event); window.minimize() })
ipcMain.on('setup:close', event => { trusted(event); if(!installing) window.close() })
ipcMain.handle('setup:launch', async event => {
    trusted(event)
    if(!installedExecutable) return false
    const error = await shell.openPath(installedExecutable)
    if(error) throw new Error('런처를 열지 못했습니다. 설치 폴더에서 실행해 주세요.')
    window.close()
    return true
})

ipcMain.handle('setup:install', async (event, options) => {
    trusted(event)
    if(installing) return { ok: false, message: '설치가 이미 진행 중입니다.' }
    installing = true
    try {
        if(updateError) throw new Error(updateError)
        const destination = updateTarget?.destination ?? validateDestination(options?.destination)
        const mode = updateTarget?.mode ?? (options?.mode === 'all' ? 'all' : 'user')
        report('checking')
        const processes = await execute('tasklist.exe', ['/FI', 'IMAGENAME eq 앨리스의 게임랜드.exe', '/FO', 'CSV', '/NH'], { windowsHide: true })
        // CSV process rows are recognizable even when tasklist uses the system code page.
        if(/^"[^\r\n]+","\d+",/m.test(processes.stdout)) throw new Error('실행 중인 런처를 닫은 뒤 다시 시도해 주세요. 게임 창은 닫지 않아도 됩니다.')
        const payload = path.join(process.resourcesPath, 'payload', 'launcher.exe')
        const digest = createHash('sha256')
        let received = 0
        const total = fs.statSync(payload).size
        let lastPercent = -1
        for await (const bytes of fs.createReadStream(payload)){
            digest.update(bytes)
            received += bytes.length
            const percent = Math.round(received / total * 100)
            if(percent !== lastPercent){ report('checking', { percent }); lastPercent = percent }
        }
        if(digest.digest('hex') !== manifest.sha256) throw new Error('설치파일을 확인하지 못했습니다. 설치파일을 다시 받아 주세요.')
        report('installing', { destination, mode })
        await new Promise((resolve, reject) => {
            // NSIS requires /D to be last and unquoted, including paths with spaces.
            const child = spawn(payload, installerArguments(destination, mode, Boolean(updateTarget)), { windowsHide: true, windowsVerbatimArguments: true, stdio: 'ignore' })
            child.once('error', reject)
            child.once('close', code => code === 0 ? resolve() : reject(new Error(code === 1223 ? '설치 권한 요청이 취소되었습니다. 다시 시도할 수 있습니다.' : '설치를 완료하지 못했습니다. 설치 위치와 권한을 확인한 뒤 다시 시도해 주세요.')))
        })
        installedExecutable = path.join(destination, manifest.executable)
        if(!fs.existsSync(installedExecutable)) throw new Error('설치된 런처를 찾지 못했습니다. 설치 위치를 확인한 뒤 다시 시도해 주세요.')
        report('complete', { destination })
        return { ok: true, destination }
    } catch(error) {
        installedExecutable = null
        report('error', { message: error.message })
        return { ok: false, message: error.message }
    } finally {
        installing = false
    }
})

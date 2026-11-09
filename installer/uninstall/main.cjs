const { app, BrowserWindow, ipcMain, screen } = require('electron')
const { existsSync } = require('node:fs')
const path = require('node:path')
const { execFile, spawn } = require('node:child_process')
const { promisify } = require('node:util')

const [directory, backend, scope, version] = process.argv.slice(1)
const execute = promisify(execFile)
const powerShell = path.join(process.env.SystemRoot, 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe')
let window
let removing = false

app.setPath('userData', path.join(path.dirname(process.execPath), 'profile'))
app.whenReady().then(() => {
    const { width, height } = screen.getPrimaryDisplay().workAreaSize
    window = new BrowserWindow({
        width: Math.min(720, width), height: Math.min(520, height),
        minWidth: 640, minHeight: 480, frame: false, resizable: false,
        backgroundColor: '#F8F4EA', title: '앨리스의 게임랜드 제거',
        icon: path.join(__dirname, 'assets/SealCircle.png'), show: false,
        webPreferences: { preload: path.join(__dirname, 'preload.cjs'), contextIsolation: true, nodeIntegration: false, sandbox: true }
    })
    window.removeMenu()
    window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))
    window.webContents.on('will-navigate', event => event.preventDefault())
    window.on('close', event => { if(removing) event.preventDefault() })
    window.once('ready-to-show', () => window.show())
    return window.loadFile(path.join(__dirname, 'index.html'), { query: { version } })
}).catch(() => app.exit(1))
app.on('window-all-closed', () => app.quit())

ipcMain.on('remove:minimize', event => { if(event.sender === window?.webContents) window.minimize() })
ipcMain.on('remove:close', event => { if(event.sender === window?.webContents && !removing) window.close() })
ipcMain.handle('remove:start', async event => {
    if(event.sender !== window?.webContents || removing) return { ok: false }
    removing = true
    try {
        const { stdout } = await execute(powerShell, ['-NoProfile', '-NonInteractive', '-Command',
            'Get-CimInstance Win32_Process -ErrorAction Stop | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($env:ALICE_REMOVE_DIRECTORY, [StringComparison]::OrdinalIgnoreCase) } | Select-Object -First 1 -ExpandProperty ProcessId'
        ], { windowsHide: true, env: { ...process.env, ALICE_REMOVE_DIRECTORY: directory + path.sep } })
        if(stdout.trim()) {
            return { ok: false, message: '실행 중인 게임과 런처를 닫은 뒤 다시 시도해 주세요.' }
        }

        await new Promise((resolve, reject) => {
            // The copied NSIS engine can remove its original. _?= must be last and unquoted.
            const child = spawn(backend, ['/S', scope, '--alice-wizard-root="' + directory + '"', '_?=' + directory], {
                cwd: path.dirname(backend), windowsHide: true, windowsVerbatimArguments: true, stdio: 'ignore'
            })
            child.once('error', reject)
            child.once('close', code => code === 0 ? resolve() : reject(new Error('NSIS removal failed')))
        })
        if(existsSync(directory)) {
            return { ok: false, message: '설치 폴더에 파일이 남아 있어요. 실행 중인 게임과 런처를 닫은 뒤 다시 시도해 주세요.' }
        }
        return { ok: true }
    } catch {
        return { ok: false, message: '런처를 제거하지 못했어요. 실행 중인 게임과 런처를 닫은 뒤 다시 시도해 주세요.' }
    } finally {
        removing = false
    }
})

import { execFileSync, spawn } from 'node:child_process'
import { access, copyFile, mkdtemp, readFile, readdir, rm, stat } from 'node:fs/promises'
import { createServer } from 'node:net'
import { homedir, tmpdir } from 'node:os'
import path from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'

const [electronExe, tauriExe, option] = process.argv.slice(2)
const windows = process.platform === 'win32'
const tauriReceipt = !windows || option === '--tauri-receipt'
if ((!windows && process.platform !== 'darwin') || !electronExe || !tauriExe) {
    throw new Error('Usage on Windows or macOS: node scripts/benchmark-launcher.mjs ELECTRON_EXE TAURI_EXE [--tauri-receipt]')
}

const visibleLogin = `(() => {
    const login = document.getElementById('loginOptionsContainer')
    const loading = document.getElementById('loadingContainer')
    const button = document.getElementById('loginOptionMicrosoft')
    return Boolean(login?.getBoundingClientRect().height && loading
        && getComputedStyle(loading).display === 'none' && button && !button.disabled)
})()`

function running(child) {
    return child.pid && child.exitCode === null && child.signalCode === null
}

async function macCrashReport(executable, pid, startedAt) {
    const name = path.basename(executable).normalize()
    const directory = path.join(homedir(), 'Library', 'Logs', 'DiagnosticReports')
    for (let attempt = 0; attempt < 8; attempt++) {
        const entries = await readdir(directory).catch(() => [])
        for (const entry of entries) {
            if (!entry.endsWith('.ips') || !entry.normalize().startsWith(name)) continue
            const file = path.join(directory, entry)
            const info = await stat(file).catch(() => null)
            if (!info || info.mtimeMs < startedAt) continue
            try {
                const text = await readFile(file, 'utf8')
                const report = JSON.parse(text.slice(text.indexOf('\n') + 1))
                if (report.pid !== pid) continue
                const frames = report.threads?.[report.faultingThread]?.frames?.slice(0, 8)
                return {
                    exception: report.exception?.type,
                    reason: report.termination?.namespace,
                    code: report.termination?.code,
                    frames: frames?.map(frame => [report.usedImages?.[frame.imageIndex]?.name, frame.symbol ?? frame.imageOffset])
                }
            } catch { /* The reporter may still be writing the file. */ }
        }
        await sleep(500)
    }
}

async function reservePort() {
    const server = createServer()
    await new Promise((resolve, reject) => {
        server.once('error', reject)
        server.listen(0, '127.0.0.1', resolve)
    })
    const port = server.address().port
    await new Promise(resolve => server.close(resolve))
    return port
}

async function attach(port, pageMatches, child, deadline) {
    while (performance.now() < deadline && running(child)) {
        try {
            const response = await fetch('http://127.0.0.1:' + port + '/json/list', { signal: AbortSignal.timeout(500) })
            const page = (await response.json()).find(item => item.type === 'page' && pageMatches(item.url))
            if (page) {
                const ws = new globalThis.WebSocket(page.webSocketDebuggerUrl)
                await new Promise((resolve, reject) => {
                    ws.addEventListener('open', resolve, { once: true })
                    ws.addEventListener('error', reject, { once: true })
                })
                return ws
            }
        } catch { /* The debugger opens as the native window starts. */ }
        await sleep(25)
    }
    throw new Error('Could not attach to the benchmark window')
}

async function waitForLogin(ws, deadline) {
    let sequence = 0
    while (performance.now() < deadline) {
        const requestId = ++sequence
        const ready = await new Promise((resolve, reject) => {
            const timer = setTimeout(() => reject(new Error('Debugger response timed out')), 3000)
            function receive(event) {
                const result = JSON.parse(event.data)
                if (result.id !== requestId) return
                clearTimeout(timer)
                ws.removeEventListener('message', receive)
                if (result.error) reject(new Error(result.error.message))
                else resolve(result.result?.result?.value === true)
            }
            ws.addEventListener('message', receive)
            ws.send(JSON.stringify({ id: requestId, method: 'Runtime.evaluate',
                params: { expression: visibleLogin, returnByValue: true } }))
        })
        if (ready) return
        await sleep(25)
    }
    throw new Error('The login screen did not finish loading')
}

async function waitForReceipt(profile, child, deadline) {
    const receipt = path.join(profile, 'login-ready')
    while (performance.now() < deadline && running(child)) {
        try {
            await access(receipt)
            return
        } catch (error) {
            if (error.code !== 'ENOENT') throw error
        }
        await sleep(25)
    }
    throw new Error('Tauri did not report that the login screen rendered')
}

function processTree(root) {
    const snapshot = JSON.parse(execFileSync('pwsh.exe', ['-NoProfile', '-Command',
        'Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,WorkingSetSize,UserModeTime,KernelModeTime | ConvertTo-Json -Compress'
    ], { encoding: 'utf8', windowsHide: true }))
    const descendants = new Set([root])
    let previousSize
    do {
        previousSize = descendants.size
        for (const item of snapshot) {
            if (descendants.has(item.ParentProcessId)) descendants.add(item.ProcessId)
        }
    } while (descendants.size !== previousSize)
    return new Map(snapshot.filter(item => descendants.has(item.ProcessId)).map(item => [item.ProcessId, item]))
}

async function measure(executable, mode) {
    const exe = path.resolve(executable)
    const temporary = path.resolve(tmpdir())
    const prefix = 'alice-benchmark-' + mode + '-'
    const profile = await mkdtemp(path.join(temporary, prefix))
    const receipt = mode === 'tauri' && tauriReceipt
    const port = receipt ? null : await reservePort()
    const env = { ...process.env }
    let args = []
    if (mode === 'electron') {
        await copyFile(new URL('../app/assets/distribution.json', import.meta.url), path.join(profile, 'distribution_dev.json'))
        Object.assign(env, { ALICE_LOCAL_DISTRIBUTION: '1', ALICE_LOCAL_PROFILE: profile, ELECTRON_IS_DEV: '1' })
        args = ['--remote-debugging-address=127.0.0.1', '--remote-debugging-port=' + port]
        if (!windows) args.push('--enable-logging=stderr')
    } else {
        Object.assign(env, {
            ALICE_TAURI_TEST_PROFILE: profile,
            WEBVIEW2_USER_DATA_FOLDER: path.join(profile, 'webview')
        })
        if (receipt) env.ALICE_TAURI_VERIFY_LOGIN = '1'
        else env.WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-address=127.0.0.1 --remote-debugging-port=' + port
    }

    const startedAt = Date.now()
    const start = performance.now()
    const deadline = start + 20_000
    const child = spawn(exe, args, { cwd: path.dirname(exe), env, stdio: ['ignore', 'ignore', 'pipe'], windowsHide: true })
    let stderr = ''
    child.stderr?.setEncoding('utf8').on('data', chunk => { stderr = (stderr + chunk).slice(-8192) })
    const spawned = new Promise((resolve, reject) => {
        child.once('spawn', resolve)
        child.once('error', reject)
    })
    const exited = new Promise(resolve => {
        child.once('exit', resolve)
        child.once('error', resolve)
    })
    let ws
    try {
        await spawned
        if (receipt) await waitForReceipt(profile, child, deadline)
        else {
            ws = await attach(port, mode === 'electron' ? url => url.includes('/app.ejs')
                : url => url.startsWith('http://tauri.localhost') || url.startsWith('https://tauri.localhost'), child, deadline)
            await waitForLogin(ws, deadline)
        }
        const readyMs = Math.round(performance.now() - start)
        // WebKit's macOS XPC helpers are outside the launcher's process tree.
        if (!windows) return { readyMs }

        await sleep(2000)
        const first = processTree(child.pid)
        const idleStart = performance.now()
        await sleep(3000)
        const last = processTree(child.pid)
        const idleMs = performance.now() - idleStart
        if (!first.has(child.pid) || !last.has(child.pid)) throw new Error('Launcher process exited early')
        let workingSetBytes = 0
        let cpuTime = 0
        for (const [pid, item] of last) {
            workingSetBytes += Number(item.WorkingSetSize)
            const before = first.get(pid)
            if (before) {
                cpuTime += Number(item.UserModeTime) + Number(item.KernelModeTime)
                    - Number(before.UserModeTime) - Number(before.KernelModeTime)
            }
        }
        return { readyMs, workingSetBytes, idleCpuPercentOfOneCore: 100 * cpuTime / (10_000 * idleMs), processCount: last.size }
    } catch (error) {
        if (stderr) console.error(stderr)
        const exit = child.signalCode ?? child.exitCode
        if (!windows && child.signalCode && child.pid) {
            const report = await macCrashReport(exe, child.pid, startedAt)
            if (report) console.error(JSON.stringify(report))
        }
        throw new Error(`${mode} launcher benchmark failed: ${error.message}${exit === null ? '' : ` (exit ${exit})`}`, { cause: error })
    } finally {
        if (ws?.readyState === globalThis.WebSocket.OPEN) {
            const close = mode === 'electron'
                ? { method: 'Browser.close' }
                : { method: 'Runtime.evaluate', params: { expression: 'window.__TAURI__.window.getCurrentWindow().close()' } }
            ws.send(JSON.stringify({ id: 0, ...close }))
            await Promise.race([exited, sleep(3000)])
        }
        ws?.close()
        if (running(child)) {
            if (windows) {
                try {
                    execFileSync('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], { stdio: 'ignore', windowsHide: true })
                } catch {
                    child.kill()
                }
            } else {
                child.kill()
            }
            await Promise.race([exited, sleep(3000)])
        }
        if (path.dirname(profile) === temporary && path.basename(profile).startsWith(prefix)) {
            await rm(profile, { recursive: true, force: true, maxRetries: 3 }).catch(() => {
                console.error('Could not remove isolated benchmark profile ' + profile)
            })
        }
    }
}

const results = { electron: [], tauri: [] }
for (let trial = 0; trial < 5; trial++) {
    const order = trial % 2 ? [['tauri', tauriExe], ['electron', electronExe]]
        : [['electron', electronExe], ['tauri', tauriExe]]
    for (const [mode, exe] of order) {
        if (mode === 'electron' && results.electronError) continue
        try {
            results[mode].push(await measure(exe, mode))
        } catch (error) {
            if (windows || mode !== 'electron') throw error
            results.electronError = error.message
            console.error(error.message)
        }
    }
}
console.log(JSON.stringify(results, null, 2))

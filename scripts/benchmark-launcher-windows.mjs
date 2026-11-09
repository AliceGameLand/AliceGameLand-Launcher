import { execFileSync, spawn } from 'node:child_process'
import { copyFile, mkdtemp } from 'node:fs/promises'
import { createServer } from 'node:net'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'

const [electronExe, tauriExe] = process.argv.slice(2)
if (process.platform !== 'win32' || !electronExe || !tauriExe) {
    throw new Error('Usage on Windows: node scripts/benchmark-launcher-windows.mjs ELECTRON_EXE TAURI_EXE')
}

const visibleLogin = `(() => {
    const login = document.getElementById('loginOptionsContainer')
    const loading = document.getElementById('loadingContainer')
    const button = document.getElementById('loginOptionMicrosoft')
    return Boolean(login?.getBoundingClientRect().height && loading
        && getComputedStyle(loading).display === 'none' && button && !button.disabled)
})()`

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
    while (performance.now() < deadline && child.exitCode === null) {
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
    const profile = await mkdtemp(path.join(tmpdir(), 'alice-benchmark-' + mode + '-'))
    const port = await reservePort()
    const env = { ...process.env }
    let args = []
    if (mode === 'electron') {
        await copyFile(new URL('../app/assets/distribution.json', import.meta.url), path.join(profile, 'distribution_dev.json'))
        Object.assign(env, { ALICE_LOCAL_DISTRIBUTION: '1', ALICE_LOCAL_PROFILE: profile, ELECTRON_IS_DEV: '1' })
        args = ['--remote-debugging-address=127.0.0.1', '--remote-debugging-port=' + port]
    } else {
        Object.assign(env, {
            ALICE_TAURI_TEST_PROFILE: profile,
            WEBVIEW2_USER_DATA_FOLDER: path.join(profile, 'webview'),
            WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-address=127.0.0.1 --remote-debugging-port=' + port
        })
    }

    const start = performance.now()
    const deadline = start + 20_000
    const child = spawn(exe, args, { cwd: path.dirname(exe), env, stdio: 'ignore' })
    const exited = new Promise(resolve => {
        child.once('exit', resolve)
        child.once('error', resolve)
    })
    let ws
    let sequence = 0
    try {
        ws = await attach(port, mode === 'electron' ? url => url.includes('/app.ejs')
            : url => url.startsWith('http://tauri.localhost') || url.startsWith('https://tauri.localhost'), child, deadline)
        let ready = false
        while (performance.now() < deadline && !ready) {
            const requestId = ++sequence
            ready = await new Promise((resolve, reject) => {
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
            if (!ready) await sleep(25)
        }
        if (!ready) throw new Error('The login screen did not finish loading')
        const readyMs = Math.round(performance.now() - start)

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
    } finally {
        if (ws?.readyState === globalThis.WebSocket.OPEN) {
            const close = mode === 'electron'
                ? { method: 'Browser.close' }
                : { method: 'Runtime.evaluate', params: { expression: 'window.__TAURI__.window.getCurrentWindow().close()' } }
            ws.send(JSON.stringify({ id: ++sequence, ...close }))
            await Promise.race([exited, sleep(3000)])
        }
        ws?.close()
        if (child.exitCode === null) child.kill()
    }
}

const results = { electron: [], tauri: [] }
for (let trial = 0; trial < 5; trial++) {
    const order = trial % 2 ? [['tauri', tauriExe], ['electron', electronExe]]
        : [['electron', electronExe], ['tauri', tauriExe]]
    for (const [mode, exe] of order) results[mode].push(await measure(exe, mode))
}
console.log(JSON.stringify(results, null, 2))

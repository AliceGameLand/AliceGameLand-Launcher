import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { createServer } from 'node:net'
import { setTimeout as wait } from 'node:timers/promises'

const [installer, expected] = process.argv.slice(2)
const listener = createServer()
await new Promise(resolve => listener.listen(0, '127.0.0.1', resolve))
const port = listener.address().port
await new Promise(resolve => listener.close(resolve))
const worker = spawn(installer, ['--updated', '--force-run'], {
    env: { ...process.env, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=' + port },
    windowsHide: true
})
let socket
try {
    for (let attempt = 0; attempt < 120 && !socket; attempt++) {
        const pages = await fetch('http://127.0.0.1:' + port + '/json/list').then(response => response.json()).catch(() => [])
        const page = pages.find(item => item.url.endsWith('/wizard/setup.html'))
        if (page) socket = new WebSocket(page.webSocketDebuggerUrl)
        else await wait(250)
    }
    assert.ok(socket, 'Tauri setup opened')
    await new Promise((resolve, reject) => {
        socket.addEventListener('open', resolve, { once: true })
        socket.addEventListener('error', reject, { once: true })
    })
    let id = 0
    function evaluate(expression) {
        return new Promise((resolve, reject) => {
            const request = ++id
            function receive(event) {
                const reply = JSON.parse(event.data)
                if (reply.id !== request) return
                socket.removeEventListener('message', receive)
                if (reply.error || reply.result.exceptionDetails) reject(new Error(expression))
                else resolve(reply.result.result.value)
            }
            socket.addEventListener('message', receive)
            socket.send(JSON.stringify({ id: request, method: 'Runtime.evaluate', params: { expression, returnByValue: true } }))
        })
    }
    async function until(expression) {
        for (let attempt = 0; attempt < 120; attempt++) {
            if (await evaluate(expression)) return
            const error = await evaluate("document.querySelector('#error:not([hidden])')?.textContent")
            if (error) throw new Error(error)
            await wait(250)
        }
        throw new Error(expression)
    }
    await until("document.querySelector('#destination')?.value === " + JSON.stringify(expected))
    await evaluate("document.querySelector('#install').click()")
    await until("!document.querySelector('#complete').hidden")
    await evaluate("setTimeout(() => document.querySelector('#close').click(), 100)")
} finally {
    socket?.close()
}
if (worker.exitCode === null) await new Promise(resolve => worker.once('exit', resolve))
assert.equal(worker.exitCode, 0)

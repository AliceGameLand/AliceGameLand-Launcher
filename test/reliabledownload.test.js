const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const os = require('node:os')
const path = require('node:path')
const http = require('node:http')
const { directUrl, downloadFile } = require('../app/assets/js/reliabledownload')

test('downloads remain on GitHub and normalize the former owner', () => {
    const a = directUrl('https://github.com/Dot0420/AliceGameLand-Distribution/releases/download/test/a.jar?v=hash')
    assert.equal(a, 'https://github.com/AliceGameLand/AliceGameLand-Distribution/releases/download/test/a.jar?v=hash')
    assert.equal(directUrl('https://maven.fabricmc.net/a.jar'), 'https://maven.fabricmc.net/a.jar')
    assert.equal(directUrl('https://github.com/other/repo/releases/download/test/a.jar'), 'https://github.com/other/repo/releases/download/test/a.jar')
})

test('download retries transient failures and preserves existing files on permanent failure', async t => {
    let calls = 0
    const server = http.createServer((req, res) => {
        calls++
        if(req.url === '/bad' || calls === 1){ res.writeHead(503); res.end('offline') }
        else res.end('valid file')
    })
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
    const folder = await fs.mkdtemp(path.join(os.tmpdir(), 'alice-download-test-'))
    t.after(async () => { server.close(); await fs.rm(folder, { recursive:true, force:true }) })
    const file = path.join(folder, 'file.jar')
    const base = `http://127.0.0.1:${server.address().port}`
    await downloadFile(base + '/ok', file)
    assert.equal(calls, 2)
    assert.equal(await fs.readFile(file, 'utf8'), 'valid file')
    await assert.rejects(downloadFile(base + '/bad', file), /file.jar.*503/)
    assert.equal(await fs.readFile(file, 'utf8'), 'valid file')
    await assert.rejects(fs.access(file + '.alice-download'))
})

import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { createReadStream } from 'node:fs'
import fs from 'node:fs/promises'
import http from 'node:http'
import { createRequire } from 'node:module'
import os from 'node:os'
import path from 'node:path'
import { execFile, spawn, spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { promisify } from 'node:util'
import { setTimeout as delay } from 'node:timers/promises'

assert.equal(process.env.GITHUB_ACTIONS, 'true', 'Run only on an isolated Actions worker')
const require = createRequire(import.meta.url)
const output = path.resolve('dist/custom-setup')
const unpacked = path.join(output, 'win-unpacked')
const release = 'alice-gameland-setup-1.0.42.exe'
const launcherName = '앨리스의 게임랜드.exe'
const key = 'Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\3d6e8bdf-a47b-5327-b17b-ac5678029a9c'
const fixture = await fs.mkdtemp(path.join(os.tmpdir(), 'Alice 업데이트 QA '))
const sha = async file => createHash('sha256').update(await fs.readFile(file)).digest('hex')
const expected = await sha(path.join(unpacked, launcherName))
const { findUpdateTarget } = require('../installer/update-target.cjs')
const userData = path.join(process.env.APPDATA, '.alicegameland', 'qa-user-file.txt')
await fs.mkdir(path.dirname(userData), { recursive:true })
await fs.writeFile(userData, 'keep me')
const registry = hive => spawnSync('reg.exe', ['query', hive + '\\' + key], { windowsHide:true }).status === 0

function run(executable, args, options = {}) {
    const result = spawnSync(executable, args, { cwd:fixture, timeout:180000, windowsHide:true, ...options })
    if(result.status !== 0) throw Error(executable + ': ' + (result.error ?? result.status))
}
assert(!registry('HKCU') && !registry('HKLM'), 'Expected a clean test worker')
run('gh.exe', ['release', 'download', 'v1.0.41', '-R', 'AliceGameLand/AliceGameLand-Launcher', '-p', 'alice-gameland-setup-1.0.41.exe', '-D', fixture], { stdio:'inherit' })
const bytes = await fs.readFile(path.join(fixture, 'alice-gameland-setup-1.0.41.exe'))
const signature = Buffer.from('377abcaf271c0004', 'hex')
const offset = bytes.indexOf(signature)
const size = 32 + Number(bytes.readBigUInt64LE(offset + 12)) + Number(bytes.readBigUInt64LE(offset + 20))
assert(offset >= 0 && offset + size <= bytes.length && size > 100_000_000)
const archive = path.join(fixture, 'outer.7z')
await fs.writeFile(archive, bytes.subarray(offset, offset + size))
run(require('7zip-bin').path7za, ['x', archive, '-o' + fixture, '-y', 'resources/payload/launcher.exe'])
const legacy = path.join(fixture, 'resources/payload/launcher.exe')
assert.equal((await fs.stat(legacy)).size, 246946520)
const feedSize = (await fs.stat(path.join(output, release))).size
const server = http.createServer((request, response) => {
    const file = request.url === '/latest.yml' ? 'latest.yml' : request.url === '/' + release ? release : null
    if(!file) { response.writeHead(404).end(); return }
    response.setHeader('Content-Length', file === release ? feedSize : feedBytes.length)
    createReadStream(path.join(output, file)).pipe(response)
})
const feedBytes = await fs.readFile(path.join(output, 'latest.yml'))
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
const origin = 'http://127.0.0.1:' + server.address().port
const harness = path.join(fixture, 'updater')
await fs.mkdir(path.join(harness, 'resources/app'), { recursive:true })
await fs.writeFile(path.join(harness, 'resources/app/package.json'), JSON.stringify({ name:'alice-legacy-updater-test', version:'1.0.41', main:'index.cjs' }))
await fs.writeFile(path.join(harness, 'resources/app/index.cjs'), `
const { app } = require('electron')
const path = require('node:path')
const old = path.join(process.resourcesPath, 'old-app.asar')
const api = require(path.join(old, 'app/assets/js/githubrelease.js'))
api.getLatestRelease = async () => ({ name:'QA', body:'', assets: [
    { name:'latest.yml' }, { name:${JSON.stringify(release)}, size:${feedSize} }
] })
api.getAssetUrl = async asset => ${JSON.stringify(origin)} + '/' + asset.name
api.readAsset = async asset => Buffer.from(await (await fetch(await api.getAssetUrl(asset))).arrayBuffer())
const Provider = require(path.join(old, 'app/assets/js/updateprovider.js'))
const { NsisUpdater } = require(path.join(old, 'node_modules/electron-updater/out/NsisUpdater.js'))
app.whenReady().then(async () => {
    if(app.getVersion() !== '1.0.41' || !app.isPackaged) throw Error('Expected a packaged v1.0.41')
    const updater = new NsisUpdater({provider:'custom',updateProvider:Provider})
    updater.autoDownload = false
    updater.autoInstallOnAppQuit = false
    updater.disableDifferentialDownload = true
    updater.on('error', () => app.exit(1))
    const result = await updater.checkForUpdates()
    if(result.updateInfo.version !== '1.0.42') throw Error('Unexpected version')
    await updater.downloadUpdate()
    updater.quitAndInstall(true)
}).catch(error => { console.error(error); app.exit(1) })
`)

try {
    for(const [scope, hive] of [['allusers', 'HKLM'], ['currentuser', 'HKCU']]){
        const target = path.join(fixture, scope, 'alice-gameland-launcher')
        const installed = path.join(target, launcherName)
        run(legacy, ['/S', '/' + scope, '/D=' + target], { windowsVerbatimArguments:true })
        assert(registry(hive), 'Legacy installation did not register in ' + hive)
        assert.notEqual(await sha(installed), expected)
        const registered = await findUpdateTarget(promisify(execFile), existsSync)
        assert.equal(registered.mode, scope === 'allusers' ? 'all' : 'user')
        assert.equal(await fs.realpath(registered.destination), await fs.realpath(target))
        for(const file of await fs.readdir(target, {withFileTypes:true})){
            if(file.isFile() && !file.name.startsWith('Uninstall ')) await fs.copyFile(path.join(target, file.name), path.join(harness, file.name))
        }
        await fs.cp(path.join(target, 'locales'), path.join(harness, 'locales'), { recursive:true, force:true })
        for(const file of ['app.asar', 'app-update.yml', 'elevate.exe']){
            await fs.copyFile(path.join(target, 'resources', file), path.join(harness, 'resources', file === 'app.asar' ? 'old-app.asar' : file))
        }
        const updater = spawn(path.join(harness, launcherName), [], { cwd:harness, windowsHide:true, stdio:'inherit' })
        const code = await new Promise((resolve, reject) => {
            const timer = setTimeout(() => { updater.kill(); reject(Error('Updater timed out')) }, 120000)
            updater.once('error', reject)
            updater.once('exit', status => { clearTimeout(timer); resolve(status) })
        })
        assert.equal(code, 0)
        let installedHash
        for(let attempt = 0; attempt < 120; attempt++){
            installedHash = await sha(installed).catch(() => null)
            if(installedHash === expected && !await fs.access(path.join(target, 'resources/runtime')).then(() => true, () => false)) break
            await delay(1000)
        }
        assert.equal(installedHash, expected, 'The real legacy updater did not replace its installation')
        await assert.rejects(fs.access(path.join(target, 'resources/runtime')), { code:'ENOENT' })
        assert.equal(await fs.readFile(userData, 'utf8'), 'keep me')
        assert(registry(hive))
        run(path.join(target, 'Uninstall ' + launcherName), ['/S', '/' + scope])
        assert(!registry(hive), 'Installation still registered after uninstall')
        console.log('UPDATER_1_0_41_PASSED', scope)
    }
} finally {
    server.close()
}

process.resourcesPath = path.join(unpacked, 'resources')
const guard = require(path.resolve('node_modules/helios-core/dist/java/JavaGuard.js'))
guard.getValidatableJavaPaths = async () => []
const java = require('../app/assets/js/javaruntime.js')
const dataDirectory = path.join(fixture, '한글 경로')
const options = { dataDirectory, supported:'>=25 <26', suggestedMajor:25 }
const executable = await java.prepare(options)
assert(await java.describe(executable, options.supported, true))
assert.equal(await java.prepare(options, () => assert.fail('Unexpected redownload')), executable)
console.log('WINDOWS_UNICODE_JAVA_PASSED', path.basename(executable))

import { execFileSync } from 'node:child_process'
import { copyFile, mkdir, mkdtemp, open, readFile, rm, writeFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import path from 'node:path'

const [windows, mac] = process.argv.slice(2)
if (!windows || !mac) {
    throw new Error('Pass the Windows setup EXE and the macOS .app.tar.gz')
}
if (!process.env.TAURI_SIGNING_PRIVATE_KEY && !process.env.TAURI_SIGNING_PRIVATE_KEY_PATH) {
    throw new Error('Set TAURI_SIGNING_PRIVATE_KEY or TAURI_SIGNING_PRIVATE_KEY_PATH')
}

const require = createRequire(import.meta.url)
const { version } = require('../package.json')
if (require('semver').valid(version) !== version) throw new Error('Invalid launcher release version')
const root = path.resolve(import.meta.dirname, '..')
const cli = path.join(root, 'node_modules', '@tauri-apps', 'cli', 'tauri.js')
const packages = [
    { target: 'windows-x86_64', file: windows, magic: [0x4d, 0x5a], name: `alice-gameland-tauri-${version}-win-x64-setup.exe` },
    { target: 'darwin-aarch64', file: mac, magic: [0x1f, 0x8b], name: `alice-gameland-tauri-${version}-mac-arm64.app.tar.gz` }
]
for (const entry of packages) {
    const file = await open(entry.file, 'r')
    try {
        const metadata = await file.stat()
        const header = Buffer.alloc(2)
        await file.read(header, 0, header.length, 0)
        if (!metadata.isFile() || metadata.size > 256 * 1024 ** 2 || !header.equals(Buffer.from(entry.magic))) {
            throw new Error('Invalid update package for ' + entry.target)
        }
    } finally {
        await file.close()
    }
}

const output = path.join(root, 'dist', 'tauri-update')
await mkdir(output, { recursive: true })
const staged = await mkdtemp(path.join(output, version + '-'))
try {
    const platforms = {}
    for (const entry of packages) {
        const destination = path.join(staged, entry.name)
        await copyFile(entry.file, destination)
        // The updater checks the release version embedded in the trusted signature comment.
        execFileSync(process.execPath, [cli, 'signer', 'sign', '--app-version', version, destination], {
            cwd: root, stdio: 'ignore', windowsHide: true
        })
        platforms[entry.target] = {
            url: 'https://github.com/AliceGameLand/AliceGameLand-Launcher/releases/download/v' + version + '/' + entry.name,
            signature: (await readFile(destination + '.sig', 'utf8')).trim()
        }
    }
    const notes = await readFile(path.join(root, 'docs', 'releases', version + '.md'), 'utf8').catch(error => {
        if (error.code !== 'ENOENT') throw error
        return ''
    })
    await writeFile(path.join(staged, 'tauri-update.json'), JSON.stringify({ version, notes: notes.trim(), platforms }, null, 2) + '\n')
    console.log(staged)
} catch (error) {
    if (path.dirname(staged) === output && path.basename(staged).startsWith(version + '-')) {
        await rm(staged, { recursive: true, force: true })
    }
    throw error
}

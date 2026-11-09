import { execFileSync } from 'node:child_process'
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import path from 'node:path'

const input = process.argv[2]
if (!input || !['win32', 'darwin'].includes(process.platform)) {
    throw new Error('Pass the built Windows installer or macOS app bundle')
}
const require = createRequire(import.meta.url)
const { version } = require('../package.json')
const root = path.resolve(import.meta.dirname, '..')
const cli = path.join(root, 'node_modules', '@tauri-apps', 'cli', 'tauri.js')
const source = path.resolve(input)
const parent = path.resolve(tmpdir())
const fixture = await mkdtemp(path.join(parent, 'alice-signed-update-'))
const key = path.join(fixture, 'fixture.key')
const payload = path.join(fixture, process.platform === 'win32' ? 'candidate.exe' : 'candidate.app.tar.gz')
const env = { ...process.env }
delete env.TAURI_SIGNING_PRIVATE_KEY
delete env.TAURI_SIGNING_PRIVATE_KEY_PATH
delete env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD

function signer(...args) {
    execFileSync(process.execPath, [cli, 'signer', ...args], {
        cwd: root, env, windowsHide: true, stdio: 'ignore'
    })
}

try {
    let executable
    let expected
    if (process.platform === 'win32') {
        await copyFile(source, payload)
        executable = path.join(fixture, 'installed', 'launcher.exe')
    } else {
        const name = execFileSync('/usr/libexec/PlistBuddy', ['-c', 'Print :CFBundleExecutable',
            path.join(source, 'Contents', 'Info.plist')], { encoding: 'utf8' }).trim()
        expected = path.join(source, 'Contents', 'MacOS', name)
        executable = path.join(fixture, 'installed', 'Fixture.app', 'Contents', 'MacOS', name)
        execFileSync('/usr/bin/tar', ['-czf', payload, '-C', path.dirname(source), path.basename(source)])
    }
    await mkdir(path.dirname(executable), { recursive: true })
    await writeFile(executable, 'previous launcher fixture')
    signer('generate', '--ci', '--write-keys', key)
    signer('sign', '--private-key-path', key, '--app-version', version, payload)
    const target = process.platform === 'win32' ? 'windows-x86_64' : 'darwin-aarch64'
    const signature = await readFile(payload + '.sig', 'utf8')
    const manifest = {
        version,
        platforms: {
            [target]: {
                url: 'https://github.com/AliceGameLand/AliceGameLand-Launcher/releases/download/v' + version + '/' + path.basename(payload),
                signature: signature.trim()
            }
        }
    }
    await writeFile(path.join(fixture, 'fixture.json'), JSON.stringify({
        executable, expected, payload, manifest,
        publicKey: (await readFile(key + '.pub', 'utf8')).trim()
    }))
    execFileSync('cargo', ['test', '--release', '--locked', '--manifest-path', 'src-tauri/Cargo.toml',
        'signed_update_accepts_only_matching_package_and_version', '--', '--ignored'], {
        cwd: root, stdio: 'inherit', windowsHide: true, env: { ...env, ALICE_TAURI_TEST_UPDATE: fixture }
    })
} finally {
    if (path.dirname(fixture) === parent && path.basename(fixture).startsWith('alice-signed-update-')) {
        await rm(fixture, { recursive: true, force: true })
    }
}

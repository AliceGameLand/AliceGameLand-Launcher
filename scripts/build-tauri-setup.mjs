import { execFileSync } from 'node:child_process'
import { copyFile, mkdir, mkdtemp, rmdir, unlink, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { build, Platform } from 'electron-builder'
import { Arch } from 'builder-util'

if (process.platform !== 'win32') throw new Error('Build the Windows setup on Windows')

const root = path.resolve(import.meta.dirname, '..')
const output = path.join(root, 'dist', 'tauri-setup')
const source = path.resolve(process.argv[2] ?? path.join(root, 'src-tauri', 'target', 'release', 'alice-gameland.exe'))
await mkdir(output, { recursive: true })
const temporary = await mkdtemp(path.join(output, 'package-'))
const stage = path.join(temporary, 'app')
const launcher = path.join(stage, 'AliceGameLand-Tauri.exe')
const bootstrapper = path.join(temporary, 'MicrosoftEdgeWebView2Setup.exe')

async function prepareWebView2() {
    const response = await fetch('https://go.microsoft.com/fwlink/p/?LinkId=2124703', { signal: AbortSignal.timeout(60_000) })
    if (!response.ok) throw new Error('Microsoft WebView2 bootstrapper download failed')
    await writeFile(bootstrapper, Buffer.from(await response.arrayBuffer()))
    const signatureCheck = [
        '$signature = Get-AuthenticodeSignature -LiteralPath $env:ALICE_WEBVIEW2_SETUP',
        "if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch '(?:^|, )O=Microsoft Corporation(?:,|$)') { exit 1 }"
    ].join('; ')
    execFileSync('pwsh.exe', ['-NoProfile', '-NonInteractive', '-Command', signatureCheck], {
        env: { ...process.env, ALICE_WEBVIEW2_SETUP: bootstrapper },
        windowsHide: true
    })
}

try {
    // Use the existing NSIS build tool with a prebuilt Tauri binary. The archive contains only this binary.
    await mkdir(stage)
    await copyFile(source, launcher)
    await prepareWebView2()
    process.env.ALICE_WEBVIEW2_SETUP = bootstrapper
    await build({
        projectDir: root,
        prepackaged: stage,
        targets: Platform.WINDOWS.createTarget(['nsis'], Arch.x64),
        publish: 'never',
        config: {
            extends: null,
            appId: 'kr.dreamcraft.alicegameland.launcher.tauri-preview',
            extraMetadata: { name: 'alice-gameland-tauri-preview' },
            productName: '앨리스의 게임랜드 (Tauri)',
            artifactName: 'alice-gameland-tauri-${version}-setup.exe',
            directories: { output, buildResources: path.join(root, 'build') },
            win: { executableName: 'AliceGameLand-Tauri', icon: 'build/icon.ico', signAndEditExecutable: false },
            nsis: {
                include: 'build/tauri-wizard.nsh',
                guid: '69bd89cc-35ff-5408-9b98-972ba4fb446c',
                oneClick: false,
                perMachine: false,
                allowElevation: true,
                allowToChangeInstallationDirectory: true,
                runAfterFinish: false,
                packElevateHelper: false,
                installerLanguages: ['ko_KR', 'en_US']
            }
        }
    })
} finally {
    await unlink(launcher).catch(error => { if (error.code !== 'ENOENT') throw error })
    delete process.env.ALICE_WEBVIEW2_SETUP
    await unlink(bootstrapper).catch(error => { if (error.code !== 'ENOENT') throw error })
    await rmdir(stage)
    await rmdir(temporary)
}

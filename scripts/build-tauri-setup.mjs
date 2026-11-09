import { copyFile, mkdir, mkdtemp, rmdir, unlink } from 'node:fs/promises'
import path from 'node:path'
import { build, Platform } from 'electron-builder'
import { Arch } from 'builder-util'

if (process.platform !== 'win32') throw new Error('Build the Windows setup on Windows')

const root = path.resolve(import.meta.dirname, '..')
const output = path.join(root, 'dist', 'tauri-setup')
const source = path.resolve(process.argv[2] ?? path.join(root, 'src-tauri', 'target', 'release', 'alice-gameland.exe'))
await mkdir(output, { recursive: true })
const stage = await mkdtemp(path.join(output, 'package-'))
const launcher = path.join(stage, 'AliceGameLand-Tauri.exe')

try {
    // Use the existing NSIS build tool with a prebuilt Tauri binary. The archive contains only this binary.
    await copyFile(source, launcher)
    await build({
        projectDir: root,
        prepackaged: stage,
        targets: Platform.WINDOWS.createTarget(['nsis'], Arch.x64),
        publish: 'never',
        config: {
            extends: null,
            appId: 'kr.dreamcraft.alicegameland.launcher.tauri-preview',
            productName: '앨리스의 게임랜드 Tauri 프리뷰',
            artifactName: 'alice-gameland-tauri-preview-${version}-setup.exe',
            directories: { output, buildResources: path.join(root, 'build') },
            win: { executableName: 'AliceGameLand-Tauri', icon: 'build/icon.ico', signAndEditExecutable: false },
            nsis: {
                include: 'build/tauri-wizard.nsh',
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
    await rmdir(stage)
}

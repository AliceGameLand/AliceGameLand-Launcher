import fs from 'node:fs'
import path from 'node:path'
import { createRequire } from 'node:module'
import { build, Platform } from 'electron-builder'
import { Arch } from 'builder-util'

const require = createRequire(import.meta.url)
const { getSignVendorPath } = require('app-builder-lib/out/codeSign/windowsSignToolManager.js')
const root = path.resolve(import.meta.dirname, '..')
const metadata = require('../package.json')
const stage = path.join(root, 'dist/setup-ui-app')
const output = path.join(root, 'dist/custom-setup')
const artifactName = 'alice-gameland-setup-${version}.${ext}'

const javaEditor = path.join(await getSignVendorPath(), 'rcedit-x64.exe')

fs.mkdirSync(path.join(stage, 'assets'), { recursive: true })
for(const name of ['index.html', 'main.cjs', 'paths.cjs', 'preload.cjs', 'renderer.js', 'style.css', 'update-target.cjs']) {
    fs.copyFileSync(path.join(root, 'installer', name), path.join(stage, name))
}
const assets = {
    'SealCircle.png': 'app/assets/images/SealCircle.png',
    'PretendardVariable.woff2': 'app/assets/fonts/PretendardVariable.woff2',
    'Pretendard-LICENSE.txt': 'app/assets/fonts/Pretendard-LICENSE.txt',
    'icon.png': 'build/icon.png'
}
for(const [name, source] of Object.entries(assets)) {
    fs.copyFileSync(path.join(root, source), path.join(stage, 'assets', name))
}
fs.writeFileSync(path.join(stage, 'package.json'), JSON.stringify({
    name: 'alice-gameland-setup',
    version: metadata.version,
    productName: '앨리스의 게임랜드 설치',
    main: 'main.cjs',
    launcherExecutable: metadata.productName + '.exe',
    private: true
}))

// The installer and the launcher use the same Electron archive.
await build({
    projectDir: root,
    targets: Platform.WINDOWS.createTarget(['nsis'], Arch.x64),
    publish: 'never',
    config: {
        extends: path.join(root, 'electron-builder.yml'),
        directories: { output },
        artifactName,
        extraResources: [
            'libraries',
            { from: javaEditor, to: 'java-launch/rcedit.exe' },
            { from: 'build/java-utf8.manifest', to: 'java-launch/java-utf8.manifest' },
            { from: 'build/rcedit-LICENSE.txt', to: 'java-launch/LICENSE.txt' },
            { from: stage, to: 'setup-wizard', filter: ['package.json', '*.cjs', '*.html', '*.js', '*.css', 'assets/{SealCircle.png,PretendardVariable.woff2,Pretendard-LICENSE.txt,icon.png}'] }
        ],
        nsis: { include: 'build/setup-wizard.nsh', runAfterFinish: false }
    }
})

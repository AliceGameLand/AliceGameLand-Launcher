import fs from 'node:fs'
import path from 'node:path'
import { createHash } from 'node:crypto'
import { createRequire } from 'node:module'
import { build, Platform } from 'electron-builder'
import { executeAppBuilderAsJson } from 'app-builder-lib/out/util/appBuilder.js'

const require = createRequire(import.meta.url)
const root = path.resolve(import.meta.dirname, '..')
const metadata = require('../package.json')
const stage = path.join(root, 'dist/setup-ui-app')

// Keep the existing install/update engine and put the custom UI in front of it.
const outputs = process.argv.includes('--ui-only') ? [path.join(root, 'dist/setup-backend', `alice-gameland-launcher-setup-${metadata.version}.exe`)] : await build({
    projectDir: root,
    targets: Platform.WINDOWS.createTarget(['nsis']),
    publish: 'never',
    config: {
        extends: path.join(root, 'electron-builder.yml'),
        directories: { output: 'dist/setup-backend' },
        nsis: { runAfterFinish: false }
    }
})
const payload = outputs.find(file => file.endsWith('.exe'))
if(!payload) throw new Error('NSIS payload was not built')
fs.mkdirSync(stage, { recursive: true })
for(const name of fs.readdirSync(path.join(root, 'installer'))) fs.copyFileSync(path.join(root, 'installer', name), path.join(stage, name))
fs.mkdirSync(path.join(stage, 'assets'), { recursive: true })
const assets = {
    'SealCircle.png': 'app/assets/images/SealCircle.png',
    'PretendardVariable.woff2': 'app/assets/fonts/PretendardVariable.woff2',
    'Pretendard-LICENSE.txt': 'app/assets/fonts/Pretendard-LICENSE.txt',
    'icon.png': 'build/icon.png'
}
for(const [name, source] of Object.entries(assets)) fs.copyFileSync(path.join(root, source), path.join(stage, 'assets', name))
// Remove the artwork left by older two-column setup builds.
const unusedGarden = path.join(stage, 'assets/garden.jpg')
if(fs.existsSync(unusedGarden)) fs.unlinkSync(unusedGarden)
const digest = createHash('sha256')
for await(const bytes of fs.createReadStream(payload)) digest.update(bytes)
fs.writeFileSync(path.join(stage, 'payload.json'), JSON.stringify({ version: metadata.version, sha256: digest.digest('hex'), executable: metadata.productName + '.exe' }, null, 2))
fs.writeFileSync(path.join(stage, 'asset-origins.json'), JSON.stringify(assets, null, 2))
fs.writeFileSync(path.join(stage, 'package.json'), JSON.stringify({ name: 'alice-gameland-setup', version: metadata.version, productName: '앨리스의 게임랜드 설치', description: '앨리스의 게임랜드 전용 설치 프로그램', author: metadata.author, main: 'main.cjs', private: true }, null, 2))
await build({
    projectDir: stage,
    targets: Platform.WINDOWS.createTarget(['portable']),
    publish: 'never',
    config: {
        extends: null,
        appId: 'kr.dreamcraft.alicegameland.setup',
        productName: '앨리스의 게임랜드 설치',
        electronVersion: require('electron/package.json').version,
        executableName: 'AliceGameLand-Setup',
        directories: { app: '.', output: path.join(root, 'dist/custom-setup'), buildResources: path.join(root, 'build') },
        files: ['**/*'], asar: true, compression: 'maximum',
        win: { icon: path.join(root, 'build/icon.ico') },
        extraResources: [{ from: payload, to: 'payload/launcher.exe' }],
        portable: { artifactName: 'alice-gameland-setup-${version}.exe', requestExecutionLevel: 'user', splashImage: null }
    }
})

// Both manual downloads and the updater open the same custom setup UI.
const setup = path.join(root, 'dist/custom-setup', `alice-gameland-setup-${metadata.version}.exe`)
const updateInfo = await executeAppBuilderAsJson(['blockmap', '--input', setup, '--output', setup + '.blockmap'])
const updateFile = { url:path.basename(setup), sha512:updateInfo.sha512, size:fs.statSync(setup).size }
fs.writeFileSync(path.join(root, 'dist/custom-setup/latest.yml'), require('js-yaml').dump({ version:metadata.version, files:[updateFile], path:updateFile.url, sha512:updateFile.sha512, releaseDate:new Date().toISOString() }))

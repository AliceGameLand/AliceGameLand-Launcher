import { cp, mkdir, readFile, writeFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import path from 'node:path'
import ejs from 'ejs'

const require = createRequire(import.meta.url)
const root = path.resolve(import.meta.dirname, '..')
const app = path.join(root, 'app')
const output = path.join(root, 'dist', 'tauri-ui')
const language = require('../app/assets/js/langloader.js')
const { version } = require('../package.json')

language.setupLanguage()

const releaseDocument = await readFile(path.join(root, 'docs', 'releases', version + '.md'), 'utf8').catch(error => {
    if (error.code !== 'ENOENT') throw error
    return ''
})
const [releaseHeading, ...releaseLines] = releaseDocument.trim().split(/\r?\n/)
const tauriRelease = {
    version,
    title: releaseHeading.replace(/^#\s+/, ''),
    notes: releaseLines.filter(line => line.trim()).map(line => line.replace(/^\s*-\s+/, ''))
}

// Compile the existing templates once. The desktop app serves static assets.
const markup = await ejs.renderFile(path.join(app, 'app.ejs'), {
    lang: language.queryEJS,
    process: { platform: process.platform },
    tauriRelease
})
const html = markup.replace(
    /<script src="\.\/assets\/js\/scripts\/[^"]+"\s*><\/script>/g,
    ''
).replace('</head>', '    <script type="module" src="./launcher.mjs"></script>\n</head>')

await mkdir(output, { recursive: true })
for (const folder of ['css', 'fonts', 'images']) {
    await cp(path.join(app, 'assets', folder), path.join(output, 'assets', folder), { recursive: true })
}
await mkdir(path.join(output, 'assets', 'js'), { recursive: true })
for (const name of ['serverart.js', 'servericon.js', 'artworktransition.js', 'uiclick.js']) {
    const source = path.join(app, 'assets', 'js', name)
    const code = await readFile(source, 'utf8')
    // Keep the canonical Electron modules and expose their exports to the new renderer.
    await writeFile(path.join(output, 'assets', 'js', name + '.mjs'),
        'const module = { exports: {} }; const exports = module.exports;\n' + code + '\nexport default module.exports;\n')
}
await cp(path.join(root, 'tauri-ui', 'launcher.mjs'), path.join(output, 'launcher.mjs'))
await cp(path.join(root, 'tauri-ui', 'java-settings.mjs'), path.join(output, 'java-settings.mjs'))
await writeFile(path.join(output, 'index.html'), html)

if (process.platform === 'win32') {
    const wizard = path.join(output, 'wizard')
    const installer = path.join(root, 'installer')
    await mkdir(wizard, { recursive: true })
    for (const [name, directory] of [['setup', installer], ['remove', path.join(installer, 'uninstall')]]) {
        const page = await readFile(path.join(directory, 'index.html'), 'utf8')
        await writeFile(path.join(wizard, name + '.html'), page
            .replace(/ {4}<meta http-equiv="Content-Security-Policy"[^\n]*\r?\n/, '')
            .replace('assets/SealCircle.png', '../assets/images/SealCircle.png')
            .replace('<header class="titlebar">', '<header class="titlebar" data-tauri-drag-region>')
            .replace('<div class="brand">', '<div class="brand" data-tauri-drag-region>')
            .replace('필요한 Java가 함께 설치됩니다.', 'Java는 첫 게임 실행 때 내려받아요.')
            .replace('<script src="renderer.js"></script>', '<script type="module" src="bridge.mjs"></script>'))
        const renderer = await readFile(path.join(directory, 'renderer.js'), 'utf8')
        await writeFile(path.join(wizard, name + '.mjs'), renderer
            .replace('런처와 Java 파일을 준비하고 있습니다.', '런처를 설치하고 있어요.'))
    }
    const style = await readFile(path.join(installer, 'style.css'), 'utf8')
    await writeFile(path.join(wizard, 'style.css'),
        style.replace('assets/PretendardVariable.woff2', '../assets/fonts/PretendardVariable.woff2'))
    await cp(path.join(root, 'tauri-ui', 'wizard.mjs'), path.join(wizard, 'bridge.mjs'))
}

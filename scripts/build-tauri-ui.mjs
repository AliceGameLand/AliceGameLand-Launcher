import { cp, mkdir, readFile, writeFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import path from 'node:path'
import ejs from 'ejs'

const require = createRequire(import.meta.url)
const root = path.resolve(import.meta.dirname, '..')
const app = path.join(root, 'app')
const output = path.join(root, 'dist', 'tauri-ui')
const language = require('../app/assets/js/langloader.js')

language.setupLanguage()

// Compile the existing templates once. The desktop app serves static assets.
const markup = await ejs.renderFile(path.join(app, 'app.ejs'), {
    lang: language.queryEJS,
    process: { platform: process.platform }
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
for (const name of ['serverart.js', 'servericon.js', 'artworktransition.js']) {
    const source = path.join(app, 'assets', 'js', name)
    const code = await readFile(source, 'utf8')
    // Keep the canonical Electron modules and expose their exports to the new renderer.
    await writeFile(path.join(output, 'assets', 'js', name + '.mjs'),
        'const module = { exports: {} }; const exports = module.exports;\n' + code + '\nexport default module.exports;\n')
}
await cp(path.join(root, 'tauri-ui', 'launcher.mjs'), path.join(output, 'launcher.mjs'))
await writeFile(path.join(output, 'index.html'), html)

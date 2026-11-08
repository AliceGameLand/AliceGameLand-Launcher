const fs = require('fs')
const path = require('path')

// Disable vanilla onboarding without resetting existing player preferences.
exports.disableTutorial = function(gameDirectory, fullscreen){
    const optionsPath = path.join(gameDirectory, 'options.txt')
    const original = fs.existsSync(optionsPath) ? fs.readFileSync(optionsPath, 'utf8') : ''
    const newline = original.includes('\r\n') ? '\r\n' : '\n'
    let updated = /^tutorialStep:[^\r\n]*/m.test(original)
        ? original.replace(/^tutorialStep:[^\r\n]*/gm, 'tutorialStep:none')
        : original + (original.length && !original.endsWith('\n') ? newline : '') + 'tutorialStep:none' + newline
    if(typeof fullscreen === 'boolean'){
        updated = /^fullscreen:[^\r\n]*/m.test(updated)
            ? updated.replace(/^fullscreen:[^\r\n]*/gm, 'fullscreen:' + fullscreen)
            : updated + (updated.length && !updated.endsWith('\n') ? newline : '') + 'fullscreen:' + fullscreen + newline
    }
    if(updated !== original){
        fs.writeFileSync(optionsPath, updated, 'utf8')
    }
}

const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { disableTutorial } = require('../app/assets/js/gameoptions')

test('Tutorial is disabled for new and existing options while preserving other settings', () => {
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'alice-options-'))
    try {
        const file = path.join(directory, 'options.txt')
        for(const before of ['', 'sensitivity:0.7', 'sensitivity:0.7\r\ntutorialStep:punch_tree\r\nguiScale:3\r\n']) {
            fs.writeFileSync(file, before)
            disableTutorial(directory)
            const after = fs.readFileSync(file, 'utf8')
            assert.match(after, /^tutorialStep:none$/m)
            if(before.includes('sensitivity:0.7')) assert.ok(after.includes('sensitivity:0.7'))
            if(before.includes('guiScale:3')) assert.ok(after.includes('guiScale:3\r\n'))
            disableTutorial(directory)
            assert.equal(fs.readFileSync(file, 'utf8'), after)
        }
    } finally {
        fs.rmSync(directory, { recursive: true, force: true })
    }
})

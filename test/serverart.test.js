const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const { DEFAULT_BACKGROUND, getServerBackground } = require('../app/assets/js/serverart')

test('Alice minigames and unknown servers use the main garden', () => {
    for(const id of ['AliceGameLand-26.2', 'new-server', null, undefined, 'toString', '__proto__']){
        assert.equal(getServerBackground(id), DEFAULT_BACKGROUND)
    }
})

test('five themed servers resolve to separate bundled landscape images', () => {
    const ids = ['AliceDiamond-26.2', 'AliceVrMinigames-26.2', 'BurcomPvP-26.3', 'AliceTailTag-26.2', 'AliceCrimeScene-26.2']
    const files = ids.map(getServerBackground)
    assert.equal(new Set(files).size, 5)
    for(const file of files){
        const bytes = fs.readFileSync(path.join(__dirname, '../app', file))
        assert.equal(bytes.subarray(1, 4).toString(), 'PNG')
        assert.ok(Math.abs(bytes.readUInt32BE(16) / bytes.readUInt32BE(20) - 16 / 9) < 0.01)
    }
})

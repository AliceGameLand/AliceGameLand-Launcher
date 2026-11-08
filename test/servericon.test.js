const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const { getServerIcon } = require('../app/assets/js/servericon')

test('all supported server icons are bundled even with missing or broken remote icons', () => {
    const ids = ['AliceDiamond-26.2', 'AliceVrMinigames-26.2', 'BurcomPvP-26.3', 'AliceTailTag-26.2', 'AliceGameLand-26.2', 'AliceCrimeScene-26.2']
    const icons = ids.map(id => getServerIcon({ id, icon: 'https://invalid.example/missing.png' }))
    assert.equal(new Set(icons).size, 6)
    for(const icon of icons){
        const bytes = fs.readFileSync(path.join(__dirname, '../app', icon))
        assert.equal(bytes.subarray(1, 4).toString(), 'PNG')
    }
})

test('unknown servers retain custom icons and absent icons use the mascot', () => {
    assert.equal(getServerIcon({ id: 'custom', icon: 'custom.png' }), 'custom.png')
    for(const id of ['custom', '__proto__', 'toString']){
        assert.equal(getServerIcon({ id }), 'assets/images/SealCircle.png')
    }
})

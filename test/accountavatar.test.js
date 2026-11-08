const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const vm = require('node:vm')

const uuid = 'be27eb1d21f941a8a825fc71d8a3e639'
const texture = 'a'.repeat(64)

function setup(loadProfile){
    const exports = {}
    const requests = []
    const context = vm.createContext({
        exports, Buffer,
        require: name => {
            assert.equal(name, 'got')
            return {
                get: url => {
                    requests.push(url)
                    return { json: loadProfile }
                }
            }
        }
    })
    vm.runInContext(fs.readFileSync(path.join(__dirname, '../app/assets/js/accountavatar.js'), 'utf8'), context)
    return { getHeadUrl: exports.getHeadUrl, requests }
}

test('Account avatars use the official texture and share profile requests across sizes', async () => {
    const avatar = setup(async () => ({
        properties: [{
            name: 'textures',
            value: Buffer.from(JSON.stringify({ textures: { SKIN: { url: 'http://textures.minecraft.net/texture/' + texture } } })).toString('base64')
        }]
    }))
    const urls = await Promise.all([
        avatar.getHeadUrl(uuid, 48),
        avatar.getHeadUrl('be27eb1d-21f9-41a8-a825-fc71d8a3e639', 40)
    ])
    assert.deepEqual(urls, [
        'https://mc-heads.net/head/' + texture + '/48',
        'https://mc-heads.net/head/' + texture + '/40'
    ])
    assert.deepEqual(avatar.requests, ['https://sessionserver.mojang.com/session/minecraft/profile/' + uuid])
})

test('Failed profile requests fall back and can be retried', async () => {
    let attempts = 0
    const avatar = setup(async () => {
        attempts++
        throw new Error('Offline')
    })
    assert.equal(await avatar.getHeadUrl(uuid, 48), 'https://mc-heads.net/head/' + uuid + '/48')
    await avatar.getHeadUrl(uuid, 48)
    assert.equal(attempts, 2)
    assert.equal(await avatar.getHeadUrl('invalid', 48), 'assets/images/icons/profile.svg')
    assert.equal(attempts, 2)
})

test('Late avatar responses cannot replace a newly selected account', async () => {
    const avatar = { style: {}, dataset: {} }
    const username = {}
    const responses = new Map()
    const context = vm.createContext({
        document: { getElementById: id => id === 'avatarContainer' ? avatar : username },
        AccountAvatar: { getHeadUrl: id => new Promise(resolve => responses.set(id, resolve)) },
        Lang: { queryJS: () => 'No account' },
        refreshLaunchControls: () => {}
    })
    const source = fs.readFileSync(path.join(__dirname, '../app/assets/js/scripts/landing.js'), 'utf8')
    vm.runInContext(source.slice(source.indexOf('function updateSelectedAccount('), source.indexOf('updateSelectedAccount(ConfigManager')), context)
    vm.runInContext('updateSelectedAccount({uuid:"first",displayName:"First"}); updateSelectedAccount({uuid:"second",displayName:"Second"})', context)
    responses.get('second')('second.png')
    await Promise.resolve()
    responses.get('first')('first.png')
    await Promise.resolve()
    assert.equal(avatar.style.backgroundImage, 'url(\'second.png\')')
    assert.equal(username.textContent, 'Second')
})

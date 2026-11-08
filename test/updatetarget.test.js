const test = require('node:test')
const assert = require('node:assert/strict')
const { findUpdateTarget } = require('../installer/update-target.cjs')
const { installerArguments } = require('../installer/paths.cjs')

test('Custom updater preserves an existing Korean path and all-user install scope', async () => {
    const target = await findUpdateTarget(async () => ({ stdout:JSON.stringify([{icon:'D:\\기존 런처\\앨리스의 게임랜드.exe,0',mode:'all'}]) }), () => true)
    assert.equal(target.destination,'D:\\기존 런처')
    assert.equal(target.mode,'all')
    assert.deepEqual(installerArguments(target.destination,target.mode,true), ['/S','--updated','/allusers','/D=D:\\기존 런처'])
})
test('Custom updater refuses missing or unrelated installs instead of choosing a new location', async () => {
    await assert.rejects(findUpdateTarget(async () => ({ stdout:'[]' }), () => false))
    await assert.rejects(findUpdateTarget(async () => ({ stdout:JSON.stringify([{icon:'C:\\Other.exe,0',mode:'user'}]) }), () => true))
})

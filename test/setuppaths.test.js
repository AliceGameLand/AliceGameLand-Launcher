const { test } = require('node:test')
const assert = require('node:assert/strict')
const { validateDestination, installerArguments } = require('../installer/paths.cjs')

test('setup keeps Korean and spaces while reserving its own application folder', () => {
    assert.equal(validateDestination('D:\\게임 폴더'), 'D:\\게임 폴더\\alice-gameland-launcher')
    assert.equal(validateDestination('C:\\Users\\A B\\alice-gameland-launcher'), 'C:\\Users\\A B\\alice-gameland-launcher')
})
test('setup rejects ambiguous, reserved and injected destinations', () => {
    for(const value of ['C:\\', 'relative', '\\\\host\\share', 'C:\\A" /S', 'C:\\A\nB', 'C:\\CON', 'C:\\nul.txt', 'C:\\folder.\\app', 'C:\\a:b']) assert.throws(() => validateDestination(value))
})
test('NSIS destination remains last and unquoted for both install scopes', () => {
    assert.deepEqual(installerArguments('D:\\게임 폴더', 'user'), ['/S', '/currentuser', '/D=D:\\게임 폴더\\alice-gameland-launcher'])
    assert.deepEqual(installerArguments('C:\\Program Files', 'all'), ['/S', '/allusers', '/D=C:\\Program Files\\alice-gameland-launcher'])
})

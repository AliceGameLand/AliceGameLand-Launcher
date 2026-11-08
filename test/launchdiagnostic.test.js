const test = require('node:test')
const assert = require('node:assert/strict')
const { createLaunchDiagnostic, redactDiagnostic } = require('../app/assets/js/launchdiagnostic')

test('Launch diagnostics preserve actionable errors and redact credentials and personal data', () => {
    const error = Object.assign(new Error('EACCES C:\\Users\\player\\game.jar user@example.com accessToken=secret refresh_token="refresh secret" Authorization: Bearer private'), { code: 'EACCES' })
    const report = createLaunchDiagnostic({ version: '1.0.24', server: 'garden', stage: 'Checking files', error, homeDirectory: 'C:\\Users\\player' })
    assert.match(report, /Error: Error \(EACCES\)/)
    assert.match(report, /Server: garden/)
    assert.match(report, /\[user\]\\game.jar/)
    assert.doesNotMatch(report, /player|user@example|secret|private|refresh secret/)
})

test('Diagnostics preserve literal error text and limit long token-like values', () => {
    const report = redactDiagnostic('<img src=x onerror=alert(1)> ' + 'x'.repeat(100))
    assert.match(report, /^<img/)
    assert.match(report, /\[redacted\]/)
    assert(redactDiagnostic('error '.repeat(1000)).length <= 2000)
})

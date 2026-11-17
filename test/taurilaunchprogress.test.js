const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const vm = require('node:vm')

test('Launch progress clears the last percentage when the next stage has no known total', () => {
    const source = fs.readFileSync(path.join(__dirname, '../tauri-ui/launcher.mjs'), 'utf8')
    const launchStage = {}
    const launchPercent = {}
    const launchAmount = { id: 'launch_progress_amount' }
    const launchDetails = {
        dataset: {},
        style: { setProperty(key, value) { this[key] = value } }
    }
    const launchProgress = {
        get position() { return this.value === undefined ? -1 : this.value / this.max },
        setAttribute(key, value) { this[key] = value },
        removeAttribute(key) { delete this[key] }
    }
    const context = { launchStage, launchPercent, launchAmount, launchProgress, launchDetails }
    const render = vm.runInNewContext(source.slice(source.indexOf('function formatDownloadSize('),
        source.indexOf('async function refreshServerStatus(')) + '\nrenderLaunchProgress', context)

    render({ phase: 'checking', current: 4, total: 10 })
    assert.equal(launchPercent.textContent, '40%')
    assert.equal(launchAmount.textContent, '4 / 10개')
    assert.equal(launchProgress.value, 4)
    assert.equal(launchProgress.max, 10)
    assert.equal(launchDetails.dataset.fillMotion, 'reset')
    render({ phase: 'checking', current: 8, total: 10 })
    assert.equal(launchDetails.dataset.fillMotion, 'advance')
    assert.equal(launchDetails.style['--launch-fraction'], 0.8)

    render({ phase: 'downloading', message: 'Java 25 다운로드 중', current: 1024, total: 2048 })
    assert.equal(launchStage.textContent, 'Java 25 다운로드 중')
    assert.equal(launchAmount.textContent, '1.0 KiB / 2.0 KiB')
    assert.equal(launchPercent.textContent, '50%')
    assert.equal(launchDetails.dataset.fillMotion, 'reset')
    assert.equal(launchProgress['aria-describedby'], launchAmount.id)

    render({ phase: 'downloading', message: '게임 파일 다운로드 중', current: 512, total: null })
    assert.equal(launchStage.textContent, '게임 파일 다운로드 중')
    assert.equal(launchPercent.hidden, true)
    assert.equal(launchDetails.dataset.fillMotion, undefined)
    assert.equal(launchProgress.value, undefined)
    assert.equal(launchAmount.textContent, '512 B 받음')
    assert.equal(launchAmount.hidden, false)

    render({ phase: 'downloading', current: 0, total: null })
    assert.equal(launchStage.textContent, '파일 다운로드 중')

    render({ phase: 'status', message: '게임 시작 중' })
    assert.equal(launchStage.textContent, '게임 시작 중')
    assert.equal(launchAmount.hidden, true)
    assert.equal(launchProgress['aria-describedby'], undefined)
})

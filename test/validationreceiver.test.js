const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const os = require('node:os')
const vm = require('node:vm')
const { createHash } = require('node:crypto')
const { performance } = require('node:perf_hooks')

test('Repair receiver saves cache before completion, restores full hashing for downloads and avoids warm reads', async t => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'alice-receiver-'))
    t.after(() => fs.rmSync(root, { recursive: true, force: true }))
    const files = Array.from({ length: 16 }, (_, i) => path.join(root, i + '.jar'))
    const bytes = Buffer.alloc(4 * 1024 * 1024, 33)
    const hash = createHash('sha1').update(bytes).digest('hex')
    for(const file of files) fs.writeFileSync(file, bytes)
    let reads = 0
    const original = async (file, algorithm, expected) => {
        reads++
        return createHash(algorithm).update(await fs.promises.readFile(file)).digest('hex') === expected
    }
    const fileUtils = { validateLocalFile: original }
    class Processor {
        async init(){}
        totalStages(){ return 1 }
        async validate(progress){
            for(const file of files) assert(await fileUtils.validateLocalFile(file, 'sha1', hash))
            await progress()
            return { invalid: [] }
        }
    }
    const cacheModule = require.resolve('../app/assets/js/filevalidationcache')
    const context = {
        exports: {}, process: { send: message => {
            if(message.response === 'validateComplete'){
                assert(fs.readdirSync(path.join(root, 'validation-cache')).some(file => file.endsWith('.json')))
            }
        } },
        require: name => {
            if(name === cacheModule) return require(cacheModule)
            if(name.endsWith('/DistributionAPI')) return { DistributionAPI: class {
                async getDistributionLocalLoadOnly(){ return { getServerById: () => ({ rawServer: { minecraftVersion:'26.3' } }) } }
            } }
            if(name.endsWith('/MojangIndexProcessor')) return { MojangIndexProcessor: Processor }
            if(name.endsWith('/DistributionIndexProcessor')) return { DistributionIndexProcessor: Processor }
            if(name.endsWith('/FileUtils')) return fileUtils
            if(name.endsWith('/LoggerUtil')) return { LoggerUtil: { getLogger: () => ({}) } }
            return {}
        }
    }
    vm.runInNewContext(fs.readFileSync(path.join(path.dirname(require.resolve('helios-core/dl')), 'dl/receivers/FullRepairReceiver.js'), 'utf8'), context)
    const message = { action:'validate', launcherDirectory:root, serverId:'server', cacheModule }
    const run = async () => { const start = performance.now(); await new context.exports.FullRepairReceiver().execute(message); return performance.now() - start }
    const cold = await run()
    assert.equal(reads, 16)
    assert.equal(fileUtils.validateLocalFile, original)
    const warm = await run()
    assert.equal(reads, 16)
    assert.equal(fileUtils.validateLocalFile, original)
    t.diagnostic(`64 MiB fixture: cold ${cold.toFixed(1)} ms, warm ${warm.toFixed(1)} ms; warm full-file reads: 0`)
})

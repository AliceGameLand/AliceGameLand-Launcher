const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const fsSync = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { test } = require('node:test')
const { promisify } = require('node:util')
const vm = require('node:vm')

const source = fsSync.readFileSync(path.join(__dirname, '../app/assets/js/javaruntime.js'), 'utf8')

async function fixture(t, options = {}) {
    const dataDirectory = await fs.mkdtemp(path.join(os.tmpdir(), 'alice-java-한글 '))
    t.after(() => fs.rm(dataDirectory, { recursive:true, force:true }))
    const cache = path.join(dataDirectory, 'runtime', 'x64')
    const calls = { probes:[], scans:0, downloads:0 }
    const javaExecFromRoot = root => path.join(root, 'bin', 'javaw.exe')
    const execFile = () => {}
    execFile[promisify.custom] = async (executable, args) => {
        calls.probes.push(executable)
        assert.deepEqual(Array.from(args).slice(-2), ['-XshowSettings:properties', '-version'])
        const info = await fs.readFile(executable, 'utf8')
        const probe = args.find(arg => arg.startsWith('-Dalice.path='))
        return { stderr: info + (probe ? '    alice.path = ' + probe.slice('-Dalice.path='.length) : '') }
    }
    const dependency = name => {
        if(name === 'node:child_process') return { execFile }
        if(name === 'helios-core/java') return {
            javaExecFromRoot,
            getValidatableJavaPaths: async () => { calls.scans++; return options.system ?? [] }
        }
        if(name === './reliabledownload') return {
            downloadFile: async (_url, destination) => {
                calls.downloads++
                if(options.interrupt) throw new Error('socket closed')
                await fs.writeFile(destination, 'corrupt archive')
            }
        }
        return require(name)
    }
    const module = { exports:{} }
    vm.runInNewContext(source, { require:dependency, process, module })
    async function java(root, version = '25.0.5', architecture = 'amd64') {
        const executable = javaExecFromRoot(root)
        await fs.mkdir(path.dirname(executable), { recursive:true })
        await fs.writeFile(executable, '')
        await fs.writeFile(path.join(path.dirname(executable), 'java.exe'),
            '    java.version = ' + version + '\n    java.vendor = Eclipse Adoptium\n    os.arch = ' + architecture + '\n')
        return executable
    }
    return { ...module.exports, cache, calls, java, params:{ dataDirectory, supported:'>=25.x <26', suggestedMajor:25 } }
}

test('Java preparation reuses a configured runtime before looking at cache or system', { skip:process.platform !== 'win32' || process.arch !== 'x64' }, async t => {
    const f = await fixture(t)
    const executable = await f.java(path.join(f.params.dataDirectory, '선택한 Java'), '25.0.4.1')
    assert.equal(await f.prepare({ ...f.params, executable }), executable)
    assert.equal(f.calls.scans, 0)
    assert.equal(f.calls.downloads, 0)
    assert.equal(f.calls.probes.length, 1)
})

test('invalid Java falls back to a shared cache for another server', { skip:process.platform !== 'win32' || process.arch !== 'x64' }, async t => {
    const f = await fixture(t)
    const incompatible = await f.java(path.join(f.params.dataDirectory, 'old'), '25.0.5', 'x86')
    const expected = await f.java(path.join(f.cache, 'jdk-25.0.5+8'))
    assert.equal(await f.prepare({ ...f.params, executable:incompatible }), expected)
    assert.equal(await f.prepare(f.params), expected)
    assert.equal(f.calls.scans, 0)
    assert.equal(f.calls.downloads, 0)
})

test('system Java is used before downloading', { skip:process.platform !== 'win32' || process.arch !== 'x64' }, async t => {
    const system = []
    const f = await fixture(t, { system })
    const root = path.join(f.params.dataDirectory, 'system')
    const executable = await f.java(root)
    system.push(root)
    assert.equal(await f.prepare(f.params), executable)
    assert.equal(f.calls.downloads, 0)
})

test('corrupt or interrupted Java downloads leave no active cache and allow a retry', { skip:process.platform !== 'win32' || process.arch !== 'x64' }, async t => {
    const options = { interrupt:true }
    const f = await fixture(t, options)
    await assert.rejects(f.prepare(f.params), /socket closed/)
    assert.deepEqual(await fs.readdir(f.cache), [])
    options.interrupt = false
    await assert.rejects(f.prepare(f.params), /expected size and SHA-256/)
    assert.deepEqual(await fs.readdir(f.cache), [])
    const executable = await f.java(path.join(f.cache, 'jdk-25.0.5+8'))
    assert.equal(await f.prepare(f.params), executable)
    assert.equal(f.calls.downloads, 2)
})

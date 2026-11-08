const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const os = require('node:os')
const path = require('node:path')
const { createHash } = require('node:crypto')
const { createValidationCache } = require('../app/assets/js/filevalidationcache')

test('Unchanged verified files reuse hashes; changed, missing and different hashes are checked', async t => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), 'alice-hash-'))
    t.after(() => fs.rm(root, { recursive: true, force: true }))
    const target = path.join(root, 'asset.jar')
    const content = Buffer.alloc(4 * 1024 * 1024, 42)
    await fs.writeFile(target, content)
    const hash = createHash('sha1').update(content).digest('hex')
    let reads = 0
    const verify = async (file, algorithm, expected) => {
        reads++
        return expected == null || createHash(algorithm).update(await fs.readFile(file)).digest('hex') === expected
    }
    let cache = await createValidationCache(root, 'server')
    assert(await cache.wrap(verify)(target, 'sha1', hash))
    await cache.save()
    cache = await createValidationCache(root, 'server')
    assert(await cache.wrap(verify)(target, 'sha1', hash))
    assert.equal(reads, 1)
    assert.equal(await cache.wrap(verify)(target, 'sha1', 'wrong'), false)
    await fs.writeFile(target, Buffer.alloc(content.length, 43))
    assert.equal(await cache.wrap(verify)(target, 'sha1', hash), false)
    await fs.unlink(target)
    assert.equal(await cache.wrap(verify)(target, 'sha1', hash), false)
    assert.equal(reads, 3)
})

test('Expired and corrupt caches fall back to hashing; hashless user settings keep their existing rule', async t => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), 'alice-hash-'))
    t.after(() => fs.rm(root, { recursive: true, force: true }))
    const target = path.join(root, 'options.txt')
    await fs.writeFile(target, 'user options')
    const verify = async () => true
    let cache = await createValidationCache(root, 'server')
    assert(await cache.wrap(verify)(target, 'sha1', 'hash'))
    await cache.save()
    const file = path.join(root, 'validation-cache', (await fs.readdir(path.join(root, 'validation-cache')))[0])
    const data = JSON.parse(await fs.readFile(file))
    for(const entry of Object.values(data.entries)) entry.checkedAt = Date.now() - 86400001
    await fs.writeFile(file, JSON.stringify(data))
    let calls = 0
    const count = async () => { calls++; return true }
    cache = await createValidationCache(root, 'server')
    assert(await cache.wrap(count)(target, 'sha1', 'hash'))
    await fs.writeFile(file, '{bad')
    cache = await createValidationCache(root, 'server')
    assert(await cache.wrap(count)(target, 'sha1', 'hash'))
    assert(await cache.wrap(count)(target, 'sha1', null))
    assert.equal(calls, 3)
})

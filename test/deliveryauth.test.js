const test = require('node:test')
const assert = require('node:assert/strict')

test('delivery sends its secret only to GitHub API, never to the asset CDN', async t => {
    const saved = globalThis.caches
    globalThis.caches = { default:{ match:async () => null, put:async () => {} } }
    t.after(() => { globalThis.caches = saved })
    const calls = []
    t.mock.method(globalThis, 'fetch', async (url, options) => {
        calls.push({ url:String(url), options })
        if(String(url).includes('/releases/tags/')) return Response.json({ assets:[{ name:'test.jar', url:'https://api.github.com/repos/AliceGameLand/AliceGameLand-Distribution/releases/assets/123' }] })
        if(String(url).includes('/releases/assets/')) return new Response(null, { status:302, headers:{ location:'https://release-assets.githubusercontent.com/test.jar' } })
        return new Response('file')
    })
    const worker = (await import('../scripts/delivery-worker.mjs')).default
    const pending = []
    const response = await worker.fetch(new Request('https://delivery.example/download/AliceGameLand-Distribution/test/test.jar'), { GITHUB_TOKEN:'test-only-not-a-real-token' }, { waitUntil:p => pending.push(p) })
    assert.equal(response.status, 200)
    assert.equal(await response.text(), 'file')
    assert.equal(calls[0].options.headers.Authorization, 'Bearer test-only-not-a-real-token')
    assert.equal(calls[1].options.headers.Authorization, 'Bearer test-only-not-a-real-token')
    assert.equal(calls[1].options.redirect, 'manual')
    assert.equal(calls[2].options.headers.Authorization, undefined)
    await Promise.all(pending)
})

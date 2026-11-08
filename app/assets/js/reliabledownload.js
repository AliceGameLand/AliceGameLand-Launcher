const got = require('got')
const fs = require('fs-extra')
const path = require('node:path')
const { pipeline } = require('node:stream/promises')
const BASE = 'https://alice-launcher-delivery.gptnr2140.workers.dev'

function mirrorUrl(value){
    const url = new URL(value)
    if(url.protocol !== 'https:') return value
    if(url.hostname === 'api.github.com' && /^\/repos\/AliceGameLand\/(AliceGameLand-Launcher|AliceGameLand-Distribution)\/releases\//.test(url.pathname)){
        return BASE + '/api' + url.pathname
    }
    const match = url.pathname.match(/^\/(?:AliceGameLand|Dot0420)\/(AliceGameLand-Launcher|AliceGameLand-Distribution)\/releases\/download\/([^/]+)\/([^/]+)$/)
    return url.hostname === 'github.com' && match
        ? `${BASE}/download/${match[1]}/${match[2]}/${match[3]}${url.search}`
        : value
}

// Stream into a temporary file; failed attempts cannot replace a valid game file.
async function downloadFile(url, destination, onProgress){
    const temporary = destination + '.alice-download'
    await fs.ensureDir(path.dirname(destination))
    let failure
    const mirror = mirrorUrl(url)
    const attempts = mirror === url ? [url, url, url] : [mirror, url, mirror]
    for(const candidate of attempts){
        try {
            onProgress?.({ transferred: 0 })
            const stream = got.stream(candidate, { timeout: { request: 60000 }, retry: { limit: 0 } })
            if(onProgress) stream.on('downloadProgress', onProgress)
            await pipeline(stream, fs.createWriteStream(temporary))
            await fs.move(temporary, destination, { overwrite: true })
            return
        } catch(error){
            failure = error
            await fs.remove(temporary)
        }
    }
    throw new Error(`Download failed: ${path.basename(destination)} (${new URL(url).hostname}, ${failure?.response?.statusCode || failure?.code || 'network error'})`)
}

module.exports = { mirrorUrl, downloadFile }

const got = require('got')
const fs = require('fs-extra')
const path = require('node:path')
const { pipeline } = require('node:stream/promises')
// Resolve the former repository owner without routing through a third party.
function directUrl(value){
    const url = new URL(value)
    if(url.protocol === 'https:' && url.hostname === 'github.com'){
        url.pathname = url.pathname.replace(/^\/Dot0420\/(AliceGameLand-(?:Launcher|Distribution))\//, '/AliceGameLand/$1/')
        return url.href
    }
    return value
}

// Stream into a temporary file; failed attempts cannot replace a valid game file.
async function downloadFile(url, destination, onProgress){
    const temporary = destination + '.alice-download'
    await fs.ensureDir(path.dirname(destination))
    let failure
    const direct = directUrl(url)
    const attempts = [direct, direct, direct]
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

module.exports = { directUrl, downloadFile }

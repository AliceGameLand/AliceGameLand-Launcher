const got = require('got')
const options = { timeout: { request: 12000 }, retry: { limit: 1 } }
const { mirrorUrl } = require('./reliabledownload')

async function getLatestRelease(repository){
    const url = `https://api.github.com/repos/AliceGameLand/${repository}/releases/latest`
    try { return await got.get(mirrorUrl(url), options).json() }
    catch { return got.get(url, options).json() }
}

async function getAssetUrl(asset){
    if(!/^https:\/\/api\.github\.com\/repos\/AliceGameLand\//.test(asset.url)) throw new Error('Unexpected release asset URL')
    if(asset.browser_download_url){
        const mirrored = mirrorUrl(asset.browser_download_url)
        if(mirrored !== asset.browser_download_url) return mirrored
    }
    const response = await got.get(asset.url, { ...options, followRedirect: false, headers: { accept: 'application/octet-stream' } })
    const location = response.headers.location
    if(!location || new URL(location).hostname !== 'release-assets.githubusercontent.com') throw new Error('Missing release download redirect')
    return location
}

async function readAsset(asset){
    return got.get(await getAssetUrl(asset), options).text()
}

module.exports = { getLatestRelease, getAssetUrl, readAsset }

const got = require('got')
const options = { timeout: { request: 7000 }, retry: { limit: 0 } }

async function getLatestRelease(repository){
    return got.get(`https://api.github.com/repos/AliceGameLand/${repository}/releases/latest`, options).json()
}

async function getAssetUrl(asset){
    if(!/^https:\/\/api\.github\.com\/repos\/AliceGameLand\//.test(asset.url)) throw new Error('Unexpected release asset URL')
    const response = await got.get(asset.url, { ...options, followRedirect: false, headers: { accept: 'application/octet-stream' } })
    const location = response.headers.location
    if(!location || new URL(location).hostname !== 'release-assets.githubusercontent.com') throw new Error('Missing release download redirect')
    return location
}

async function readAsset(asset){
    return got.get(await getAssetUrl(asset), options).text()
}

module.exports = { getLatestRelease, getAssetUrl, readAsset }

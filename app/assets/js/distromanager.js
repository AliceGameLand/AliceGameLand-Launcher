const { DistributionAPI } = require('helios-core/common')

const ConfigManager = require('./configmanager')

// Old WesterosCraft url.
// exports.REMOTE_DISTRO_URL = 'http://mc.westeroscraft.com/WesterosCraftLauncher/distribution.json'
exports.REMOTE_DISTRO_URL = 'https://github.com/AliceGameLand/AliceGameLand-Distribution/releases/latest/download/distribution.json'

const api = new DistributionAPI(
    ConfigManager.getLauncherDirectory(),
    null, // Injected forcefully by the preloader.
    null, // Injected forcefully by the preloader.
    exports.REMOTE_DISTRO_URL,
    process.env.ALICE_LOCAL_DISTRIBUTION === '1'
)

// Keep cached and older remote manifests readable during the display-name change.
for(const method of ['getDistribution', 'refreshDistributionOrFallback']){
    const load = api[method].bind(api)
    api[method] = async (...args) => {
        const distribution = await load(...args)
        for(const server of distribution?.servers ?? []){
            const { rawServer } = server
            if(rawServer.id === 'AliceDiamond-26.2' && rawServer.name === '버컴퍼니 다이아몬드 대결'){
                rawServer.name = '버컴퍼니 다이아 대결'
            }
            if(rawServer.id === 'AliceTailTag-26.2' && rawServer.name === '혜안져스 꼬리잡기'){
                // Keep the instance ID so installed game settings and server visibility carry over.
                rawServer.name = '혜안져스 미니게임2'
            }
        }
        return distribution
    }
}

exports.DistroAPI = api

// Cached manifests render immediately; remote failures must not hold the startup screen.
const { getLatestRelease, readAsset } = require('./githubrelease')
api.pullRemote = async () => {
    try {
        const release = await getLatestRelease('AliceGameLand-Distribution')
        const asset = release.assets.find(asset => asset.name === 'distribution.json')
        const data = JSON.parse(await readAsset(asset))
        if(!Array.isArray(data.servers) || !data.servers.length) throw new Error('Invalid server list')
        return { data }
    } catch {
        return { data: null }
    }
}
const cachedLoad = api.getDistribution.bind(api)
api.getDistribution = async () => {
    if(!api.isDevMode() && api.rawDistribution == null){
        try {
            const fs = require('fs-extra')
            if(!fs.existsSync(api.distroPath)){
                fs.ensureDirSync(ConfigManager.getLauncherDirectory())
                fs.copyFileSync(require('path').join(__dirname, '../distribution.json'), api.distroPath)
            }
            await api.getDistributionLocalLoadOnly()
            void api.refreshDistributionOrFallback().catch(() => {})
        } catch { /* First install uses the bounded remote request. */ }
    }
    return cachedLoad()
}

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
            if(server.rawServer.id === 'AliceDiamond-26.2' && server.rawServer.name === '버컴퍼니 다이아몬드 대결'){
                server.rawServer.name = '버컴퍼니 다이아 대결'
            }
        }
        return distribution
    }
}

exports.DistroAPI = api

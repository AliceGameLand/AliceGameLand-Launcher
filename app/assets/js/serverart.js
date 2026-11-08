const DEFAULT_BACKGROUND = 'assets/images/backgrounds/0.jpg'
const backgrounds = Object.freeze({
    'AliceDiamond-26.2': 'diamond',
    'AliceVrMinigames-26.2': 'minigames',
    'BurcomPvP-26.3': 'pvp',
    'AliceTailTag-26.2': 'tail-tag',
    'AliceCrimeScene-26.2': 'crime-scene'
})

// Unknown servers keep the original garden until their artwork is available.
function getServerBackground(serverId){
    const name = backgrounds[serverId]
    return typeof name === 'string' && Object.hasOwn(backgrounds, serverId)
        ? `assets/images/backgrounds/servers/${name}.png`
        : DEFAULT_BACKGROUND
}

module.exports = { DEFAULT_BACKGROUND, getServerBackground }

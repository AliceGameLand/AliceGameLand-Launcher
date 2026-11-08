const icons = Object.freeze({
    'AliceDiamond-26.2': 'diamond.png',
    'AliceVrMinigames-26.2': 'minigames.png',
    'BurcomPvP-26.3': 'pvp.png',
    'AliceTailTag-26.2': 'tail-tag.png',
    'AliceGameLand-26.2': 'alice.png',
    'AliceCrimeScene-26.2': 'crime-scene.png'
})

// Home and settings must resolve the same bundled icon, including offline use.
exports.getServerIcon = function(server){
    return Object.hasOwn(icons, server.id)
        ? 'assets/images/server-icons/' + icons[server.id]
        : server.icon || 'assets/images/SealCircle.png'
}

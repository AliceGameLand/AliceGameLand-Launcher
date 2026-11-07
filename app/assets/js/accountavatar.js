const got = require('got')

const profiles = new Map()
const profileLifetime = 5 * 60 * 1000

async function resolveTexture(uuid){
    const profile = await got.get(`https://sessionserver.mojang.com/session/minecraft/profile/${uuid}`, {
        timeout: { request: 5000 },
        retry: { limit: 0 }
    }).json()
    const property = profile.properties?.find(value => value.name === 'textures')
    if(!property) return null
    const texture = JSON.parse(Buffer.from(property.value, 'base64').toString('utf8'))
    return /^https?:\/\/textures\.minecraft\.net\/texture\/([a-f0-9]{32,64})$/i.exec(texture.textures?.SKIN?.url)?.[1] ?? null
}

/**
 * Resolve the official texture first to bypass stale UUID-based head renders.
 */
exports.getHeadUrl = async function(uuid, size){
    const id = String(uuid).replaceAll('-', '').toLowerCase()
    if(!/^[a-f0-9]{32}$/.test(id)) return 'assets/images/icons/profile.svg'

    let profile = profiles.get(id)
    if(!profile || profile.expiresAt <= Date.now()){
        profile = { expiresAt: Date.now() + profileLifetime }
        profile.texture = resolveTexture(id).catch(() => {
            if(profiles.get(id) === profile) profiles.delete(id)
            return null
        })
        profiles.set(id, profile)
    }
    const texture = await profile.texture
    return `https://mc-heads.net/head/${texture ?? id}/${size}`
}

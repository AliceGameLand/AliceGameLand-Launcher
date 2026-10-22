const { LoggerUtil } = require('helios-core')

const logger = LoggerUtil.getLogger('DiscordWrapper')

const { Client } = require('discord-rpc-patch')

const Lang = require('./langloader')

let client
let activity
let connected = false
let clientId

function publishActivity(){
    if(!connected || !client || !activity) return
    client.setActivity(activity).catch(error => {
        logger.warn('Unable to update Discord Rich Presence.', error)
    })
}

exports.initRPC = function(genSettings, servSettings, initialDetails = Lang.queryJS('discord.waiting')){
    const nextActivity = {
        details: initialDetails,
        state: Lang.queryJS('discord.state', {shortId: servSettings.shortId}),
        largeImageKey: servSettings.largeImageKey,
        largeImageText: servSettings.largeImageText,
        smallImageKey: genSettings.smallImageKey,
        smallImageText: genSettings.smallImageText,
        startTimestamp: new Date().getTime(),
        instance: false
    }

    // Reuse the connection when moving between launcher and game activity.
    if(client && clientId === genSettings.clientId){
        activity = nextActivity
        publishActivity()
        return
    }

    if(client) exports.shutdownRPC()
    client = new Client({ transport: 'ipc' })
    const rpcClient = client
    clientId = genSettings.clientId
    connected = false
    activity = nextActivity

    rpcClient.on('ready', () => {
        if(client !== rpcClient) return
        connected = true
        logger.info('Discord RPC Connected')
        publishActivity()
    })
    
    rpcClient.login({clientId: genSettings.clientId}).catch(error => {
        if(error.message.includes('ENOENT')) {
            logger.info('Unable to initialize Discord Rich Presence, no client detected.')
        } else {
            logger.info('Unable to initialize Discord Rich Presence: ' + error.message, error)
        }
    })
}

exports.updateDetails = function(details){
    if(!activity) return
    activity.details = details
    publishActivity()
}

exports.shutdownRPC = function(){
    if(!client) return
    const rpcClient = client
    const wasConnected = connected
    client = null
    clientId = null
    activity = null
    connected = false
    Promise.resolve()
        .then(() => wasConnected && rpcClient.clearActivity())
        .catch(error => logger.warn('Unable to clear Discord Rich Presence.', error))
        .finally(() => rpcClient.destroy().catch(error => logger.warn('Unable to close Discord RPC.', error)))
}

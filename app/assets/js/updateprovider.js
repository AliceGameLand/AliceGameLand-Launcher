const { Provider } = require('electron-updater/out/providers/Provider')
const yaml = require('js-yaml')
const { getLatestRelease, getAssetUrl, readAsset } = require('./githubrelease')

class ReleaseApiProvider extends Provider {
    constructor(_options, _updater, runtimeOptions){
        super({ ...runtimeOptions, isUseMultipleRangeRequest: false })
    }

    async getLatestVersion(){
        const release = await getLatestRelease('AliceGameLand-Launcher')
        const feed = release.assets.find(asset => asset.name === 'latest.yml')
        if(!feed) throw new Error('Update feed is missing')
        const info = yaml.load(await readAsset(feed))
        if(!info.version || !Array.isArray(info.files)) throw new Error('Invalid update feed')
        this.files = await Promise.all(info.files.map(async file => {
            const asset = release.assets.find(asset => asset.name === file.url)
            if(!asset || !file.sha512 || asset.size !== file.size) throw new Error('Invalid update asset')
            return { url: new URL(await getAssetUrl(asset)), info: file }
        }))
        return { ...info, releaseName: release.name, releaseNotes: release.body }
    }

    resolveFiles(){
        return this.files
    }
}

module.exports = ReleaseApiProvider

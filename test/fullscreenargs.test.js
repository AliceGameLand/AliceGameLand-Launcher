const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const vm = require('node:vm')

const source = fs.readFileSync(path.join(__dirname, '../app/assets/js/processbuilder.js'), 'utf8')
const start = source.indexOf('    _constructJVMArguments113(mods, tempNativePath){')
const method = source.slice(start, source.indexOf('\n    /**', start))
for(const resolutionRule of [false, true]){
    test(`Fullscreen on/off works with custom resolution rule ${resolutionRule}`, () => {
        let fullscreen = true
        const context = vm.createContext({
            ConfigManager: { getFullscreen: () => fullscreen, getMaxRAM: () => '4G', getMinRAM: () => '2G', getJVMOptions: () => [], getGameWidth: () => 1280, getGameHeight: () => 720 },
            process: { platform: 'win32' }
        })
        vm.runInContext('class ProcessBuilder {' + method + '};this.Builder=ProcessBuilder', context)
        for(const value of [true, false]){
            fullscreen = value
            const builder = new context.Builder()
            builder.server = { rawServer: { id:'fixture' } }
            builder.modManifest = { mainClass:'net.minecraft.client.main.Main', arguments: { game:[] } }
            builder.vanillaManifest = { arguments: { jvm:[], game: resolutionRule ? [{ rules:[{ features:{ has_custom_resolution:true } }], value:['--width','${resolution_width}','--height','${resolution_height}'] }] : [] } }
            builder._processAutoConnectArg = () => {}
            const args = Array.from(builder._constructJVMArguments113([], 'natives'))
            assert.equal(args.filter(arg => arg === '--fullscreen').length, value ? 1 : 0)
            assert(!args.includes('true'))
            if(resolutionRule) assert(args.includes(1280))
        }
    })
}

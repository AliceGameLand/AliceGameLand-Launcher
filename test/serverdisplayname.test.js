const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const vm = require('node:vm')

test('Both cached and refreshed manifests use the shorter name without changing identity or custom names', async () => {
    let name = '버컴퍼니 다이아몬드 대결'
    let minigamesName = '혜안져스 꼬리잡기'
    class DistributionAPI {
        isDevMode(){ return true }
        async getDistribution(){
            return { servers: [
                { rawServer: { id: 'AliceDiamond-26.2', name } },
                { rawServer: { id: 'AliceTailTag-26.2', name: minigamesName } }
            ] }
        }
        async refreshDistributionOrFallback(){ return this.getDistribution() }
    }
    const exports = {}
    vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../app/assets/js/distromanager.js'),'utf8'), {
        exports, process:{env:{}}, require: name => name === 'helios-core/common' ? {DistributionAPI} : {getLauncherDirectory:()=>''}
    })
    for(const method of ['getDistribution','refreshDistributionOrFallback']){
        const [diamond, minigames] = (await exports.DistroAPI[method]()).servers
        const server = diamond.rawServer
        assert.equal(server.name,'버컴퍼니 다이아 대결')
        assert.equal(server.id,'AliceDiamond-26.2')
        assert.equal(minigames.rawServer.name, '혜안져스 미니게임2')
        assert.equal(minigames.rawServer.id, 'AliceTailTag-26.2')
    }
    name = '다음 시즌 이름'
    minigamesName = '다음 미니게임'
    const [diamond, minigames] = (await exports.DistroAPI.getDistribution()).servers
    assert.equal(diamond.rawServer.name, name)
    assert.equal(minigames.rawServer.name, minigamesName)
})

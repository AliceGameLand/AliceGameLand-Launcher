const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const vm = require('node:vm')

test('Both cached and refreshed manifests use the shorter name without changing identity or custom names', async () => {
    let name = '버컴퍼니 다이아몬드 대결'
    class DistributionAPI {
        async getDistribution(){ return { servers:[{ rawServer:{ id:'AliceDiamond-26.2', name } }] } }
        async refreshDistributionOrFallback(){ return this.getDistribution() }
    }
    const exports = {}
    vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../app/assets/js/distromanager.js'),'utf8'), {
        exports, process:{env:{}}, require: name => name === 'helios-core/common' ? {DistributionAPI} : {getLauncherDirectory:()=>''}
    })
    for(const method of ['getDistribution','refreshDistributionOrFallback']){
        const server = (await exports.DistroAPI[method]()).servers[0].rawServer
        assert.equal(server.name,'버컴퍼니 다이아 대결')
        assert.equal(server.id,'AliceDiamond-26.2')
    }
    name = '다음 시즌 이름'
    assert.equal((await exports.DistroAPI.getDistribution()).servers[0].rawServer.name,name)
})

const Module = require('node:module')
const path = require('node:path')
const load = Module._load
// This isolated preview cannot authenticate accounts, download files or launch games.
Module._load = function(request){
    const value = load.apply(this, arguments)
    if(request === 'helios-core/mojang') return { ...value, getServerStatus: async () => ({ players:{ online:12, max:100 } }) }
    if(request.endsWith('/authmanager')) return { ...value, validateSelected:async () => true }
    if(request.endsWith('/processbuilder')) return class { build(){ throw new Error('업데이트 체험에서는 게임을 실행하지 않습니다.') } }
    if(request === 'helios-core/dl') return { ...value, FullRepair:class { constructor(){ throw new Error('업데이트 체험에서는 게임 파일을 내려받지 않습니다.') } } }
    if(request.endsWith('/discordwrapper')) return { initRPC(){}, shutdownRPC(){}, updateDetails(){} }
    return value
}
Module.createRequire(path.join(process.env.ALICE_QA_ROOT, 'app/app.ejs'))('./assets/js/preloader')

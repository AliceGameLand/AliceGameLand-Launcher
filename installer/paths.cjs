const path = require('node:path').win32

function validateDestination(value){
    if(typeof value !== 'string' || /["<>|?*]/.test(value) || [...value].some(character => character.charCodeAt(0) < 32)) throw new Error('설치할 폴더를 다시 선택해 주세요.')
    const directory = path.normalize(value.trim())
    if(!/^[a-z]:\\/i.test(directory) || directory === path.parse(directory).root || directory.includes(':', 2)) throw new Error('드라이브 안의 폴더를 설치 위치로 선택해 주세요.')
    if(directory.split('\\').slice(1).some(part => /[. ]$/.test(part) || /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(part))) throw new Error('Windows에서 사용할 수 없는 폴더 이름입니다.')
    const result = path.basename(directory) === 'alice-gameland-launcher' ? directory : path.join(directory, 'alice-gameland-launcher')
    if(result.length > 200) throw new Error('설치 경로가 너무 깁니다. 더 짧은 경로를 선택해 주세요.')
    return result.replace(/\\$/, '')
}
function installerArguments(destination, mode){
    return ['/S', mode === 'all' ? '/allusers' : '/currentuser', `/D=${validateDestination(destination)}`]
}
module.exports = { validateDestination, installerArguments }

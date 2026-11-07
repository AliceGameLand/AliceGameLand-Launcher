const path = require('node:path').win32
const { validateDestination } = require('./paths.cjs')

// This uninstall key belongs to kr.dreamcraft.alicegameland.launcher.
const query = `[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false); $items=@(); foreach($scope in @('user','all')) { $hive=if($scope -eq 'user'){'HKCU:'}else{'HKLM:'}; $key=$hive+'\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\3d6e8bdf-a47b-5327-b17b-ac5678029a9c'; $r=Get-ItemProperty -LiteralPath $key -ErrorAction SilentlyContinue; if($r.DisplayIcon){$items+=@{icon=$r.DisplayIcon;mode=$scope}} }; ConvertTo-Json -InputObject $items -Compress`

async function findUpdateTarget(execute, exists){
    const result = await execute('powershell.exe', ['-NoProfile','-NonInteractive','-Command',query], { windowsHide:true })
    const entries = JSON.parse(result.stdout.replace(/^\uFEFF/, '').trim())
    for(const entry of entries){
        const executable = entry.icon.replace(/,-?\d+$/, '').replace(/^"|"$/g, '')
        if(path.basename(executable) !== '앨리스의 게임랜드.exe' || !exists(executable)) continue
        return { destination:validateDestination(path.dirname(executable), false), mode:entry.mode }
    }
    throw new Error('기존 런처의 설치 위치를 찾지 못했습니다. 런처를 다시 실행한 뒤 업데이트해 주세요.')
}
module.exports = { findUpdateTarget }

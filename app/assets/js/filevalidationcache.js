const fs = require('node:fs/promises')
const path = require('node:path')
const { createHash } = require('node:crypto')

// Cache only successful hashes, and periodically re-read unchanged files too.
exports.createValidationCache = async function(directory, serverId){
    const file = path.join(directory, 'validation-cache', createHash('sha256').update(serverId).digest('hex') + '.json')
    let previous = {}
    try { previous = JSON.parse(await fs.readFile(file, 'utf8')).entries ?? {} } catch { /* A missing/broken cache requires normal verification. */ }
    const entries = Object.create(null)
    const signature = stat => [stat.size, stat.mtimeNs, stat.ctimeNs, stat.ino, stat.dev].map(String).join(':')
    return {
        wrap: verify => async (target, algorithm, hash) => {
            if(hash == null) return verify(target, algorithm, hash)
            let before
            try { before = await fs.stat(target, { bigint: true }) } catch { return false }
            const key = JSON.stringify([path.resolve(target), algorithm, hash])
            const stamp = signature(before)
            const cached = entries[key] ?? previous[key]
            const age = Date.now() - cached?.checkedAt
            if(before.isFile() && cached?.stamp === stamp && age >= 0 && age < 86400000){
                entries[key] = cached
                return true
            }
            const valid = await verify(target, algorithm, hash)
            if(valid){
                try {
                    if(signature(await fs.stat(target, { bigint: true })) === stamp){
                        entries[key] = { stamp, checkedAt: Date.now() }
                    }
                } catch { /* A changed/deleted file cannot enter the cache. */ }
            }
            return valid
        },
        save: async () => {
            const temporary = file + '.' + process.pid + '.tmp'
            try {
                await fs.mkdir(path.dirname(file), { recursive: true })
                await fs.writeFile(temporary, JSON.stringify({ version: 1, entries }))
                await fs.rename(temporary, file)
            } catch {
                await fs.unlink(temporary).catch(() => {})
            }
        }
    }
}

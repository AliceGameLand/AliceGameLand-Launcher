const { execFile } = require('node:child_process')
const { createHash } = require('node:crypto')
const { createReadStream } = require('node:fs')
const fs = require('node:fs/promises')
const os = require('node:os')
const net = require('node:net')
const path = require('node:path')
const { setTimeout: delay } = require('node:timers/promises')
const { promisify } = require('node:util')
const AdmZip = require('adm-zip')
const semver = require('semver')
const { getValidatableJavaPaths, javaExecFromRoot } = require('helios-core/java')
const { downloadFile } = require('./reliabledownload')

const execute = promisify(execFile)
const version = '25.0.5+8'
const archiveName = 'OpenJDK25U-jdk_x64_windows_hotspot_25.0.5_8.zip'
const url = 'https://github.com/adoptium/temurin25-binaries/releases/download/jdk-25.0.5%2B8/' + archiveName
const size = 141285173
const sha256 = '636bdc13dc649ddc7d5921c5c7d23cca6e44fd19d662b45cc9c02135a6c96c42'
const managedName = 'jdk-' + version
const stagePrefix = '.alice-java-'
const utf8Executable = 'alice-java-utf8.exe'
const unicodeSample = '한글 경로 🦊'

async function describe(executable, supported, requireUnicode = false) {
    if(typeof executable !== 'string' || !path.isAbsolute(executable)) return null
    const command = executable.replace(/javaw\.exe$/i, 'java.exe')
    try {
        await fs.access(executable)
        const args = ['-XshowSettings:properties', '-version']
        if(requireUnicode) args.unshift('-Dstderr.encoding=UTF-8', '-Dalice.path=' + unicodeSample)
        const { stderr } = await execute(command, args, { windowsHide: true, timeout: 10000 })
        if(requireUnicode && !stderr.split(/\r?\n/).some(line => line.trim() === 'alice.path = ' + unicodeSample)) return null
        const actual = /^\s*java\.version\s*=\s*(\S+)/m.exec(stderr)?.[1]
        const vendor = /^\s*java\.vendor\s*=\s*(.+)/m.exec(stderr)?.[1]?.trim()
        const architecture = /^\s*os\.arch\s*=\s*(\S+)/m.exec(stderr)?.[1]
        const parsed = semver.coerce(actual)
        const compatibleArch = process.arch === 'arm64' ? architecture === 'aarch64' : ['amd64', 'x86_64'].includes(architecture)
        if(!compatibleArch || !parsed || !semver.satisfies(parsed, supported)) return null
        return { executable, version: actual, vendor }
    } catch {
        return null
    }
}

async function findCached(directory, supported, requireUnicode) {
    const entries = await fs.readdir(directory, { withFileTypes: true }).catch(error => {
        if(error.code === 'ENOENT') return []
        throw error
    })
    const roots = entries.filter(entry => entry.isDirectory() && !entry.name.startsWith('.')).map(entry => entry.name)
    roots.sort((a, b) => Number(b === managedName) - Number(a === managedName))
    for(const root of roots) {
        if(requireUnicode && root === managedName) {
            const adapted = path.join(directory, root, 'bin', utf8Executable)
            if(await describe(adapted, supported, true)) return adapted
        }
        const executable = javaExecFromRoot(path.join(directory, root))
        if(await describe(executable, supported, requireUnicode)) return executable
    }
    return null
}

// Windows releases the named pipe when its owner exits, including after a crash.
async function lockCache(directory) {
    const canonical = (await fs.realpath(directory)).toLowerCase()
    const name = '\\\\.\\pipe\\alice-java-' + createHash('sha256').update(canonical).digest('hex')
    for(;;) {
        const server = net.createServer(socket => socket.destroy())
        try {
            await new Promise((resolve, reject) => {
                server.once('error', reject)
                server.listen(name, resolve)
            })
            return () => new Promise(resolve => server.close(resolve))
        } catch(error) {
            if(error.code !== 'EADDRINUSE') throw error
            await delay(250)
        }
    }
}

async function adaptManagedJava(root, supported) {
    const bin = path.join(root, 'bin')
    const source = path.join(bin, 'java.exe')
    if(!await fs.access(source).then(() => true, () => false)) return null

    const resources = path.join(process.resourcesPath, 'java-launch')
    const executable = path.join(bin, utf8Executable)
    const staging = path.join(bin, 'alice-java-utf8-staging.exe')
    try {
        // Edit a copy of the verified runtime and keep the vendor executables intact.
        await fs.copyFile(source, staging)
        await execute(path.join(resources, 'rcedit.exe'), [staging, '--application-manifest', path.join(resources, 'java-utf8.manifest')], {
            windowsHide: true, timeout: 10000
        })
        if(!await describe(staging, supported, true)) throw new Error('Java could not preserve Unicode paths in this Windows configuration.')
        await fs.rename(staging, executable)
        return executable
    } finally {
        await fs.rm(staging, { force: true })
    }
}

async function install(directory, supported, requireUnicode, progress) {
    const stage = await fs.mkdtemp(path.join(directory, stagePrefix))
    try {
        const archive = path.join(stage, archiveName)
        progress?.('download', 0)
        await downloadFile(url, archive, ({ transferred }) => {
            progress?.('download', Math.min(100, transferred / size * 100))
        }, { connect: 15000, response: 30000, socket: 30000 })
        progress?.('verify')
        const digest = createHash('sha256')
        for await(const bytes of createReadStream(archive)) digest.update(bytes)
        if((await fs.stat(archive)).size !== size || digest.digest('hex') !== sha256) {
            throw new Error('Temurin archive did not match the expected size and SHA-256.')
        }

        progress?.('extract')
        await new AdmZip(archive).extractAllToAsync(stage, false)
        const extracted = path.join(stage, managedName)
        const original = javaExecFromRoot(extracted)
        const executable = await describe(original, supported, requireUnicode) ? original
            : requireUnicode ? await adaptManagedJava(extracted, supported) : null
        if(!executable) throw new Error('Downloaded Java did not pass the launcher compatibility check.')

        const destination = path.join(directory, managedName)
        await fs.rm(destination, { recursive: true, force: true })
        await fs.rename(extracted, destination)
        return path.join(destination, 'bin', path.basename(executable))
    } finally {
        await fs.rm(stage, { recursive: true, force: true }).catch(() => {})
    }
}

async function prepare({ dataDirectory, executable, supported, suggestedMajor }, progress) {
    const requireUnicode = process.platform === 'win32' && /[^\x20-\x7e]/.test(dataDirectory + os.tmpdir())
    if(await describe(executable, supported, requireUnicode)) return executable
    const directory = path.join(dataDirectory, 'runtime', process.arch)
    const cached = await findCached(directory, supported, requireUnicode)
    if(cached) return cached

    // Reuse Helios discovery and probe candidates without a command shell.
    const roots = await getValidatableJavaPaths(dataDirectory).catch(() => [])
    for(const root of roots) {
        const candidate = javaExecFromRoot(root)
        if(await describe(candidate, supported, requireUnicode)) return candidate
    }

    if(process.platform !== 'win32' || process.arch !== 'x64' || suggestedMajor !== 25 || !semver.satisfies('25.0.5', supported)) {
        throw new Error('No compatible Java runtime is available for this Windows release and server.')
    }
    await fs.mkdir(directory, { recursive: true })
    const unlock = await lockCache(directory)
    try {
        const ready = await findCached(directory, supported, requireUnicode)
        if(ready) return ready
        if(requireUnicode) {
            const adapted = await adaptManagedJava(path.join(directory, managedName), supported)
            if(adapted) return adapted
        }
        for(const entry of await fs.readdir(directory, { withFileTypes: true })) {
            if(entry.isDirectory() && entry.name.startsWith(stagePrefix)) {
                await fs.rm(path.join(directory, entry.name), { recursive: true, force: true })
            }
        }
        return await install(directory, supported, requireUnicode, progress)
    } finally {
        await unlock()
    }
}

module.exports = { describe, prepare }

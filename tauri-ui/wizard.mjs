/* global document, window */
const { core, window: windowApi } = window.__TAURI__
const frame = windowApi.getCurrentWindow()
const controls = { close: () => frame.close(), minimize: () => frame.minimize() }

if (frame.label === 'setup-wizard') {
    const info = await core.invoke('wizard_info')
    let onProgress
    window.setup = {
        ...controls,
        info: () => Promise.resolve(info),
        folder: current => core.invoke('wizard_folder', { current }),
        install: options => {
            const progress = new core.Channel(onProgress)
            return core.invoke('wizard_install', { options, progress })
        },
        launch: () => core.invoke('wizard_launch').then(() => frame.close()),
        progress: callback => { onProgress = callback }
    }
    await import('./setup.mjs')
    if (info.verifyElectronTransition) {
        const button = document.getElementById('install')
        await button.onclick()
        if (!document.getElementById('complete').hidden) await frame.close()
        else throw new Error(document.getElementById('error').textContent)
    }
} else {
    const info = await core.invoke('wizard_info')
    document.getElementById('version').textContent = 'v' + info.version
    window.uninstall = {
        ...controls,
        start: () => core.invoke('wizard_remove')
    }
    await import('./remove.mjs')
    if (info.verifyRemovalWindow === 'remove') {
        await document.getElementById('remove').onclick()
        if (!document.getElementById('complete').hidden) await frame.close()
    } else if (info.verifyRemovalWindow) {
        await frame.close()
    }
}

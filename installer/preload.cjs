const { contextBridge, ipcRenderer } = require('electron')
contextBridge.exposeInMainWorld('setup', {
    info: () => ipcRenderer.invoke('setup:info'),
    folder: current => ipcRenderer.invoke('setup:folder', current),
    install: options => ipcRenderer.invoke('setup:install', options),
    launch: () => ipcRenderer.invoke('setup:launch'),
    close: () => ipcRenderer.send('setup:close'),
    minimize: () => ipcRenderer.send('setup:minimize'),
    progress: callback => ipcRenderer.on('setup:progress', (_event, value) => callback(value))
})

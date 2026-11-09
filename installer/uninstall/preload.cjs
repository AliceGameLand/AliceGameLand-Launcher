const { contextBridge, ipcRenderer } = require('electron')
contextBridge.exposeInMainWorld('uninstall', {
    start: () => ipcRenderer.invoke('remove:start'),
    close: () => ipcRenderer.send('remove:close'),
    minimize: () => ipcRenderer.send('remove:minimize')
})

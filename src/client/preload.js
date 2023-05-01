const { contextBridge, ipcRenderer } = require('electron')

contextBridge.exposeInMainWorld('scarlet', {
    close: () => ipcRenderer.send("close"),
    minimise: () => ipcRenderer.send("minimise"),
    steam_login: () => ipcRenderer.send("steam_login"),
    open_admin_page_in_browser: () => ipcRenderer.send("open_admin_page_in_browser"),
})


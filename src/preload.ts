import {FileDownload} from "./types";

const { contextBridge, ipcRenderer } = require('electron')

/**
 * Communication between [Web] -and-> [Electron]
 */
contextBridge.exposeInMainWorld('scarlet', {
    /**
     * Window Controls
     */
    close: () => ipcRenderer.send("close"),
    minimise: () => ipcRenderer.send("minimise"),

    /**
     * Authentication
     */
    steam_login: () => ipcRenderer.send("steam_login"),
    open_admin_page_in_browser: () => ipcRenderer.send("open_admin_page_in_browser"),

    /**
     * Install Directory
     */
    open_choose_install_dir: (existing_directory: string) => ipcRenderer.send("open_choose_install_dir", existing_directory),
    on_select_install_dir: (callback: (_: any) => any) => ipcRenderer.on('on_select_install_dir', callback),

    /**
     * Rust Bindings
     */
    start_download: (destination_folder: string, files: Array<FileDownload>) => ipcRenderer.invoke("start_download", destination_folder, files),
    stop_download: () => ipcRenderer.invoke('stop_download'),
    get_progress: () => ipcRenderer.invoke('get_progress'),
    ping: () => ipcRenderer.invoke("ping"),

    /**
     * Update Events
     */
    update_available: (callback: (_: any) => any) => ipcRenderer.on('update_available', callback),
    update_downloaded: (callback: (_: any) => any) => ipcRenderer.on('update_downloaded', callback),
})


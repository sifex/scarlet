const { autoUpdater } = require("electron-updater")
const {app, BrowserWindow, shell, ipcMain, dialog} = require('electron')
const fs = require('fs')
const path = require('path')

let WebSocket = require('ws')

const { Deeplink } = require('electron-deeplink');

/**
 * Setup
 */

// Dev
let isDev = () => process.argv[2] === '--dev'

let mainWindow

// URL Handler
const protocol = isDev ? 'scarlet-dev' : 'scarlet';
const deeplink = new Deeplink({ app, mainWindow, protocol, isDev });

// Single Instance
if (!app.requestSingleInstanceLock()) { app.quit() }



/**
 * Main Window
 */


let scarletURI = isDev()
    ? 'http://localhost/'
    : 'https://scarlet.australianarmedforces.org/'

function createWindow() {
    // Create the browser window.
    mainWindow = new BrowserWindow({
        width: 1000,
        height: 600,
        minHeight: 400,
        minWidth: 500,
        icon: __dirname + '/scarlet.ico',
        show: false,
        frame: false,
        transparent: true,
        // frame: false
        webPreferences: {
            sandbox: true,
            preload: path.join(__dirname, 'preload.js')
        }
    })

    mainWindow.loadURL(scarletURI + 'electron/intro/',
        {
            extraHeaders: 'pragma: no-cache\n'
        })

    let websocket = new WebSocket("ws://localhost:2074");
    websocket.onerror = function (evt) {
        const executablePath = fs.existsSync(__dirname + "/agent/Scarlet.exe")
            ? __dirname + "/agent/Scarlet.exe" // Development Version
            : __dirname + "/../../resources/agent/Scarlet.exe";  // Production Version

        // shell.openItem(executablePath); // TODO Uncomment before prod
    };

    mainWindow.once('ready-to-show', () => {
        mainWindow.show()
        isDev() ? mainWindow.openDevTools() : '';
        autoUpdater.checkForUpdatesAndNotify()
    })

    mainWindow.on('closed', function () {
        mainWindow = null
    })

    /**
     * Auto Update
     */
    autoUpdater.on('update-available', () => { mainWindow.webContents.send('update_available'); });
    autoUpdater.on('update-downloaded', () => { mainWindow.webContents.send('update_downloaded'); });
    ipcMain.on('restart_app', () => { autoUpdater.quitAndInstall(); });

    /**
     * Electron Controls
     */
    ipcMain.on('close', () => { mainWindow.close() });
    ipcMain.on('minimise', () => { mainWindow.minimize() });
    ipcMain.on('steam_login', () => {
        shell.openExternal(scarletURI + 'browser/steam/verify')
    });

    ipcMain.on('open_admin_page_in_browser', () => {
        shell.openExternal(scarletURI + 'admin')
    });

    ipcMain.on('quit', () => {
        mainWindow = null
    });

    app.on('open-url', (event, url) => {
        let token = url.replace(protocol + '://', '')

        mainWindow.loadURL(scarletURI + 'electron/steam/verify?token=' + token,
            {
                extraHeaders: 'pragma: no-cache\n'
            })
    })
}

app.whenReady().then(createWindow)

// Quit when all windows are closed.
app.on('window-all-closed', function () {
    app.quit();
})
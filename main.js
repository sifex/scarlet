
const { autoUpdater } = require("electron-updater")
const {app, BrowserWindow, shell, ipcMain} = require('electron')
const fs = require('fs')

let WebSocket = require('ws')
let currentWindow

let isSingleInstance = app.requestSingleInstanceLock()
if (!isSingleInstance) {
    app.quit()
}

function isDev() {
    return process.argv[2] === '--dev';
}

function createWindow() {
    // Create the browser window.
    let mainWindow = new BrowserWindow({
        width: 1000,
        height: 600,
        minHeight: 400,
        minWidth: 500,
        icon: __dirname + '/scarlet.ico',
        show: false,
        frame: false,
        webPreferences: {
            nodeIntegration: true
        }
    })

    mainWindow.loadURL(
        isDev()
            ? 'http://localhost:3000/mods/electron/?username=Omega'
            : `https://staging.scarlet.australianarmedforces.org/key/electron/`,
        {
            extraHeaders: 'pragma: no-cache\n'
        })
    currentWindow = mainWindow

    let websocket = new WebSocket("ws://localhost:2074");
    websocket.onerror = function (evt) {

        const executablePath = fs.existsSync(__dirname + "/agent/Scarlet.exe")
            ? __dirname + "/agent/Scarlet.exe" // Development Version
            : __dirname + "/../../resources/agent/Scarlet.exe";  // Production Version

        shell.openItem(executablePath);
    };

    mainWindow.once('ready-to-show', () => {
        mainWindow.show()
        isDev() ? mainWindow.openDevTools() : '';
        autoUpdater.checkForUpdatesAndNotify()
    })

    mainWindow.on('closed', function () {
        mainWindow = null
    })

    autoUpdater.on('update-available', () => {
        mainWindow.webContents.send('update_available');
    });

    autoUpdater.on('update-downloaded', () => {
        mainWindow.webContents.send('update_downloaded');
    });

    ipcMain.on('restart_app', () => {
        autoUpdater.quitAndInstall();
    });

    ipcMain.on('quit', () => {
        mainWindow = null
    });
}

app.whenReady().then(createWindow)

// Quit when all windows are closed.
app.on('window-all-closed', function () {
    app.quit();
})
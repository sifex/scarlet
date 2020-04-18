
const { autoUpdater } = require("electron-updater")
const {app, BrowserWindow, shell} = require('electron')
const fs = require('fs')

let WebSocket = require('ws')
let currentWindow

let isSingleInstance = app.requestSingleInstanceLock()
if (!isSingleInstance) {
    app.quit()
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

    // mainWindow.loadURL(`https://staging.australianarmedforces.org/mods/electron/?username=Omega`, { extraHeaders: 'pragma: no-cache\n' }) // TODO Remove
    // mainWindow.loadURL(`https://scarlet.australianarmedforces.org/key/electron/`, { extraHeaders: 'pragma: no-cache\n' })
    mainWindow.loadURL('http://localhost:3000/mods/electron/?username=Omega', {extraHeaders: 'pragma: no-cache\n'}) // TODO Remove
    currentWindow = mainWindow

    let websocket = new WebSocket("ws://localhost:2074");
    websocket.onerror = function (evt) {

        const executablePath = fs.existsSync(__dirname + "/../../resources/agent/Scarlet.exe")
            ? __dirname + "/../../resources/agent/Scarlet.exe"  // Production Version
            : __dirname + "/agent/Scarlet.exe"; // Development Version

        shell.openItem(executablePath);
    };

    mainWindow.once('ready-to-show', () => {
        mainWindow.show()
        mainWindow.openDevTools(); // TODO Remove
        autoUpdater.checkForUpdatesAndNotify()
    })

    mainWindow.on('closed', function () {
        mainWindow = null
    })
}

app.whenReady().then(createWindow)

// Quit when all windows are closed.
app.on('window-all-closed', function () {
    app.quit();
})
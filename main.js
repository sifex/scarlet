var electronInstaller = require('electron-winstaller');

const electron = require('electron')
const {shell} = require('electron');
// Module to control application life.
const app = electron.app
var WebSocket = require('ws')
// Module to create native browser window.
const BrowserWindow = electron.BrowserWindow

// Keep a global reference of the window object, if you don't, the window will
// be closed automatically when the JavaScript object is garbage collected.
let mainWindow

function createWindow () {
  // Create the browser window.
    mainWindow = new BrowserWindow({width: 1000, height: 600, icon: __dirname + '/scarlet.ico',
     show: false, frame: false})
    // and load the index.html of the app.
    // mainWindow.loadURL(`https://staging.australianarmedforces.org/mods/electron/?noheader`)
    mainWindow.loadURL(`https://scarlet.australianarmedforces.org/key/electron/`)


	var wsUri = "ws://localhost:1001";

	websocket = new WebSocket(wsUri);
	websocket.onerror = function(evt) {
		var executablePath =  __dirname + "/resources/Scarlet/Scarlet.exe";
		var parameters = [""];

		shell.openItem(executablePath);
	};


    mainWindow.once('ready-to-show', () => {
        mainWindow.show()

        mainWindow.openDevTools();

    })

    // Emitted when the window is closed.
    mainWindow.on('closed', function () {
      // Dereference the window object, usually you would store windows
      // in an array if your app supports multi windows, this is the time
      // when you should delete the corresponding element.
      mainWindow = null
    })



}

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
// Some APIs can only be used after this event occurs.
app.on('ready', createWindow)

// Quit when all windows are closed.
app.on('window-all-closed', function () {
    app.quit();
})

app.on('activate', function () {
})

// In this file you can include the rest of your app's specific main process
// code. You can also put them in separate files and require them here.

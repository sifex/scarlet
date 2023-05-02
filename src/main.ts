import {BrowserWindow, ipcMain, shell, dialog} from 'electron';
import * as path from "path";
import {AppUpdater, autoUpdater} from "electron-updater";
const {hello} = require('./agent.node')


export default class Main {
    static mainWindow: Electron.BrowserWindow;
    static application: Electron.App;
    static browserWindow: typeof BrowserWindow;

    private static isDev = (): boolean => process.argv[2] === '--dev'
    private static scarlet_api_url = () => Main.isDev() ? 'http://localhost/' : 'https://london.australianarmedforces.org/';
    private static protocol = () => Main.isDev() ? 'scarlet-dev' : 'scarlet';


    static main(app: Electron.App, browserWindow: typeof BrowserWindow) {
        console.log(hello())
        if (!app.requestSingleInstanceLock()) {
            app.quit()
        }
        Main.browserWindow = browserWindow
        Main.application = app;

        /**
         * Bits
         */
        Main.registerUrlHandler()
        Main.registerIPCEvents()
        // Main.registerAutoUpdater()

        /**
         * Application Event Listeners
         */
        Main.application.on('window-all-closed', Main.onWindowAllClosed);
        Main.application.on('second-instance', Main.onSecondInstance);
        Main.application.on('open-url', Main.onOpenUrl)
        Main.application.on('ready', Main.onReady);
    }

    /**
     * When the application is ready
     *
     * Loads the main Window, injects preload.js and renders the web page
     *
     * @private
     */
    private static onReady() {
        Main.mainWindow = new Main.browserWindow({
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
                preload: path.join(__dirname, Main.isDev() ? './client/preload.js' : '../client/preload.js')
            }
        })

        /**
         * Load the Scarlet Website
         */
        Main.mainWindow.loadURL(Main.scarlet_api_url() + 'electron/intro/', {extraHeaders: 'pragma: no-cache\n'});
        Main.mainWindow.on('closed', Main.onClose);
        Main.mainWindow.once('ready-to-show', Main.onReadyToShow)
    }

    /**
     * Actions something when all the windows are closed
     *
     * @private
     */
    private static onWindowAllClosed() {
        Main.application.quit();
    }

    /**
     * Opens a URL on the Window
     *
     * @private
     */
    private static onClose() {
        Main.mainWindow = null;
    }

    /**
     * Opens a URL on the Window
     *
     * @private
     */
    private static onReadyToShow() {
        Main.mainWindow.show()
        Main.isDev() ? Main.mainWindow.webContents.openDevTools() : ''
    }

    /**
     * Logs into the Application, requires token
     *
     * @private
     * @param url
     */
    private static login(url: String) {
        let token = url.replace(Main.protocol() + '://', '')

        return Main.mainWindow.loadURL(
            Main.scarlet_api_url() + 'electron/steam/verify?token=' + token,
            {extraHeaders: 'pragma: no-cache\n'}
        )
    }

    /**
     * Register all the URL handlers for opening the application as `scarlet://`
     *
     * @private
     */
    private static registerUrlHandler() {
        if (process.defaultApp) {
            if (process.argv.length >= 2) {
                Main.application.setAsDefaultProtocolClient(
                    Main.protocol(),
                    process.execPath,
                    [path.resolve(process.argv[1])]
                )
            }
        } else {
            Main.application.setAsDefaultProtocolClient(Main.protocol())
        }
    }

    /**
     * Install Auto Updater
     *
     * @private
     */
    private static registerAutoUpdater() {
        autoUpdater.checkForUpdatesAndNotify()

        autoUpdater.on('update-available', () => {
            Main.mainWindow.webContents.send('update_available');
        });
        autoUpdater.on('update-downloaded', () => {
            Main.mainWindow.webContents.send('update_downloaded');
        });
        ipcMain.on('restart_app', () => {
            autoUpdater.quitAndInstall();
        });
    }

    /**
     * Install Inter Process Comms Events
     *
     * This will only handle the [Web] -> [Electron] events,
     * for [Electron] -> [Web] events, checkout `preload.js`
     *
     * @private
     */
    private static registerIPCEvents() {
        ipcMain.on('close', () => Main.mainWindow.close());
        ipcMain.on('minimise', () => Main.mainWindow.minimize());

        ipcMain.on('steam_login', () => {
            shell.openExternal(Main.scarlet_api_url() + 'browser/steam/verify')
        });

        ipcMain.on('open_admin_page_in_browser', () => {
            shell.openExternal(Main.scarlet_api_url() + 'admin')
        });

        ipcMain.on('quit', () => {
            Main.mainWindow = null
        });

        ipcMain.on('open_choose_install_dir', (evt, current_directory: string) => {
            dialog.showOpenDialog(Main.mainWindow, {
                properties: ['openDirectory'],
                defaultPath: current_directory,
                message: 'Select the path to your Arma 3 Folder'
            }).then(result => {
                console.log(result.canceled)
                console.log(result.filePaths)
                if(result.filePaths[0]) {
                    Main.mainWindow.webContents.send('on_select_install_dir', result.filePaths[0])
                }
            }).catch(err => {
                console.log(err)
            })
        });
    }

    /**
     * Handle the instance when a second window appears.
     *
     * In this case we actually want to login as sometimes the user will open Scarlet
     * with the `scarlet://` url handler
     *
     * @param event
     * @param commandLine
     * @param workingDirectory
     * @private
     */
    private static onSecondInstance(event: Electron.Event | Electron.Session, commandLine: any, workingDirectory: unknown) {
        // Someone tried to run a second instance, we should focus our window.
        if (Main.mainWindow) {
            if (Main.mainWindow.isMinimized()) Main.mainWindow.restore()
            Main.mainWindow.focus()
        }

        const url = commandLine.pop().slice(0, -1)

        if (url.startsWith(Main.protocol())) {
            Main.login(url)
        }
    }

    /**
     * Opens a URL on the Window
     *
     * @param event
     * @param url
     * @private
     */
    private static onOpenUrl(event: Event, url: string) {
        return Main.login(url)
    }
}
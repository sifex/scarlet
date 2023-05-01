import {BrowserWindow, ipcMain, shell} from 'electron';
import * as path from "path";
import {AppUpdater, autoUpdater} from "electron-updater";
import IpcMain = Electron.IpcMain;


export default class Main {
    static mainWindow: Electron.BrowserWindow;
    static application: Electron.App;
    static browserWindow: typeof BrowserWindow;

    private static isDev = (): boolean => process.argv[2] === '--dev'
    private static scarlet_api_url = () => Main.isDev() ? 'http://localhost/' : 'https://london.australianarmedforces.org/';
    private static protocol = () => Main.isDev() ? 'scarlet-dev' : 'scarlet';


    static main(app: Electron.App, browserWindow: typeof BrowserWindow) {
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
        Main.registerAutoUpdater()

        /**
         * Application Event Listeners
         */
        Main.application.on('window-all-closed', Main.onWindowAllClosed);
        Main.application.on('second-instance', Main.onSecondInstance);
        Main.application.on('open-url', Main.onOpenUrl)
        Main.application.on('ready', Main.onReady);
    }

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
                preload: path.join(__dirname, 'client/preload.js')
            }
        })
        /**
         * Load the Scarlet Website
         */
        Main.mainWindow.loadURL(Main.scarlet_api_url() + 'electron/intro/', {extraHeaders: 'pragma: no-cache\n'});
        Main.mainWindow.on('closed', Main.onClose);
        Main.mainWindow.once('ready-to-show', Main.onReadyToShow)
    }

    private static onOpenUrl(event: Event, url: string) {
        return Main.login(url)
    }


    private static onWindowAllClosed() {
        if (process.platform !== 'darwin') {
            Main.application.quit();
        }
    }

    private static onClose() {
        Main.mainWindow = null;
    }

    private static onReadyToShow() {
        Main.mainWindow.show()
        Main.isDev() ? Main.mainWindow.webContents.openDevTools() : ''
        autoUpdater.checkForUpdatesAndNotify()
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
    }

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
}
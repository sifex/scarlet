import {BrowserWindow, ipcMain, shell, dialog, App} from 'electron';
import * as path from "path";
import {autoUpdater} from "electron-updater";
import {fetchAndConvertXML, getKeywordArguments} from './utils';
import {FileDownload} from './types';

const {
    ping,
    get_progress,
    start_download,
    stop_download
}: {
    ping: () => void,
    get_progress: () => Promise<any>,
    start_download: (destination_path: string, files: Array<FileDownload>) => Promise<any>,
    stop_download: () => void
} = require('./agent.node');

export default class Main {
    static mainWindow: Electron.BrowserWindow;
    static application: Electron.App;
    static browserWindow: typeof BrowserWindow;

    private static isDev = (): boolean => getKeywordArguments()['dev'] as boolean;
    private static scarlet_api_url = 'https://staging.scarlet.australianarmedforces.org/';
    private static protocol = 'scarlet';

    /**
     * Main entry point of the application
     * @param app Electron.App instance
     * @param browserWindow BrowserWindow constructor
     */
    static main(app: App, browserWindow: typeof BrowserWindow): void {
        /**
         * Set the Scarlet API URL, based on Args or based on dev flag
         */
        if(getKeywordArguments()['api-url'] as string) {
            Main.scarlet_api_url = getKeywordArguments()['api-url'] as string;
        }

        if (!app.requestSingleInstanceLock()) {
            app.quit();
        }
        Main.browserWindow = browserWindow;
        Main.application = app;

        Main.registerUrlHandler();
        Main.registerIPCEvents();
        Main.registerAutoUpdater();

        Main.application.on('window-all-closed', Main.onWindowAllClosed);
        Main.application.on('second-instance', Main.onSecondInstance);
        Main.application.on('open-url', Main.onOpenUrl);
        Main.application.on('ready', Main.onReady);
    }

    /**
     * Initializes the main window when the application is ready
     */
    private static onReady(): void {
        Main.mainWindow = new Main.browserWindow({
            width: 1000,
            height: 600,
            minHeight: 400,
            minWidth: 500,
            icon: __dirname + '/scarlet.ico',
            show: false,
            frame: false,
            transparent: true,
            webPreferences: {
                sandbox: false,
                preload: path.join(__dirname, 'preload.js')
            }
        });

        Main.mainWindow.loadURL(Main.scarlet_api_url + 'electron/intro/', {extraHeaders: 'pragma: no-cache\n'});
        Main.mainWindow.on('closed', Main.onClose);
        Main.mainWindow.once('ready-to-show', Main.onReadyToShow);
    }

    /**
     * Quits the application when all windows are closed
     */
    private static onWindowAllClosed(): void {
        Main.application.quit();
    }

    /**
     * Handles the closing of the main window
     */
    private static onClose(): void {
        Main.mainWindow = null;
    }

    /**
     * Shows the main window when it's ready
     */
    private static onReadyToShow(): void {
        Main.mainWindow.show();
        Main.isDev() ? Main.mainWindow.webContents.openDevTools() : null;
    }

    /**
     * Logs into the application using a token
     * @param url The URL containing the token
     */
    private static login(url: string): Promise<void> {

        console.log('Logging in with URL:', url);

        // If the query string specifies a new scarlet_api_url, update it and go to that instead.
        const parsedURL = new URL(url);


        if (parsedURL.searchParams.has('scarlet_api_url')) {
            Main.scarlet_api_url = decodeURIComponent(parsedURL.searchParams.get('scarlet_api_url'));
        }

        // let token = url.replace(Main.protocol + '://', '');
        const token = parsedURL.searchParams.get('token');

        console.log(token)

        return Main.mainWindow.loadURL(
            Main.scarlet_api_url + 'electron/steam/verify?token=' + token,
            {extraHeaders: 'pragma: no-cache\n'}
        );
    }

    /**
     * Registers the URL handler for the application
     */
    private static registerUrlHandler(): void {
        if (process.defaultApp) {
            if (process.argv.length >= 2) {
                Main.application.setAsDefaultProtocolClient(
                    Main.protocol,
                    process.execPath,
                    [path.resolve(process.argv[1])]
                );
            }
        } else {
            Main.application.setAsDefaultProtocolClient(Main.protocol);
        }
    }

    /**
     * Registers the auto-updater for the application
     */
    private static registerAutoUpdater(): void {
        autoUpdater.checkForUpdatesAndNotify().catch(console.error);

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
     * Registers IPC events for communication between main and renderer processes
     */
    private static registerIPCEvents(): void {
        ipcMain.on('close', () => Main.mainWindow.close());
        ipcMain.on('minimise', () => Main.mainWindow.minimize());
        ipcMain.on('steam_login', () => {
            shell.openExternal(Main.scarlet_api_url + 'browser/steam/verify');
        });
        ipcMain.on('open_admin_page_in_browser', () => {
            shell.openExternal(Main.scarlet_api_url + 'admin');
        });
        ipcMain.on('quit', () => {
            Main.mainWindow = null;
        });
        ipcMain.on('open_choose_install_dir', (evt, current_directory: string) => {
            dialog.showOpenDialog(Main.mainWindow, {
                properties: ['openDirectory'],
                defaultPath: current_directory ?? '',
                message: 'Select the path to your Arma 3 Folder'
            }).then(result => {
                if (result.filePaths[0]) {
                    Main.mainWindow.webContents.send('on_select_install_dir', result.filePaths[0]);
                }
            }).catch(err => {
                console.error(err);
            });
        });

        ipcMain.handle('ping', ping);

        ipcMain.handle('stop_download', stop_download);
        ipcMain.handle('get_progress', get_progress);

        ipcMain.handle('start_download', async (
            evt,
            destination_folder: string,
            files: Array<FileDownload>
        ) => {
            return start_download(
                destination_folder,
                files
            );
        })
    }

    /**
     * Handles the second instance of the application
     * @param event The event object
     * @param commandLine The command line arguments
     * @param workingDirectory The working directory
     */
    private static onSecondInstance(event: Electron.Event | Electron.Session, commandLine: string[], workingDirectory: string): void {
        if (Main.mainWindow) {
            if (Main.mainWindow.isMinimized()) Main.mainWindow.restore();
            Main.mainWindow.focus();
        }

        const url = commandLine.pop().slice(0, -1);

        if (url.startsWith(Main.protocol)) {
            Main.login(url);
        }
    }

    /**
     * Handles opening URLs
     * @param event The event object
     * @param url The URL to open
     */
    private static onOpenUrl(event: Event, url: string): Promise<void> {
        return Main.login(url);
    }
}
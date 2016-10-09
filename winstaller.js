var electronInstaller = require('electron-winstaller');

resultPromise = electronInstaller.createWindowsInstaller({
    appDirectory: 'scarlet-win32-ia32/',
    outputDirectory: 'tmp/build/installer64',
    authors: 'Scarlet',
    exe: 'Scarlet.exe'
  });

resultPromise.then(() => console.log("It worked!"), (e) => console.log(`No dice: ${e.message}`));

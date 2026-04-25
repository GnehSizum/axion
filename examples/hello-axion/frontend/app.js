window.addEventListener('DOMContentLoaded', async () => {
  const status = document.getElementById('bridge-status');
  const details = document.getElementById('bridge-details');
  if (!status) return;

  if (!window.__AXION__) {
    status.textContent = 'Axion bootstrap was not injected.';
    if (details) details.textContent = 'Bridge unavailable';
    return;
  }

  try {
    const lifecycleEvents = [];
    const recordLifecycleEvent = (name) => (payload) => {
      lifecycleEvents.push({ name, payload });
      if (details) {
        details.textContent = JSON.stringify({ lifecycleEvents }, null, 2);
      }
    };
    [
      'window.created',
      'window.resized',
      'window.close_requested',
      'window.closed',
      'window.redraw_failed',
    ].forEach((name) => window.__AXION__.listen(name, recordLifecycleEvent(name)));

    const pluginReady = new Promise((resolve) => {
      window.__AXION__.listen('demo.ready', resolve);
    });
    const [ping, appInfo, appVersion, appEcho, windowInfo, greeting, pluginEvent] = await Promise.all([
      window.__AXION__.invoke('app.ping', { from: 'hello-axion' }),
      window.__AXION__.invoke('app.info', null),
      window.__AXION__.invoke('app.version', null),
      window.__AXION__.invoke('app.echo', { from: 'hello-axion', async: true }),
      window.__AXION__.invoke('window.info', null),
      window.__AXION__.invoke('demo.greet', { from: 'hello-axion-plugin-demo' }),
      pluginReady,
    ]);
    const fsWrite = await window.__AXION__.invoke('fs.write_text', {
      path: 'notes/hello.txt',
      contents: 'hello-axion wrote this through the Axion bridge',
    });
    const fsRead = await window.__AXION__.invoke('fs.read_text', { path: 'notes/hello.txt' });
    const dialogOpen = await window.__AXION__.invoke('dialog.open', {
      title: 'Select files for the Axion preview',
      multiple: true,
      filters: [
        { name: 'Text', extensions: ['txt', 'md'] },
        { name: 'Images', extensions: ['png', 'jpg'] },
      ],
    });
    const dialogSave = await window.__AXION__.invoke('dialog.save', {
      title: 'Choose a save path for the Axion preview',
      defaultPath: 'notes/export.txt',
    });
    const hostLog = window.__AXION__.events.includes('app.log')
      ? await window.__AXION__.emit('app.log', {
          message: 'hello-axion frontend is ready',
          windowId: windowInfo.id,
        })
      : false;

    status.textContent = `Axion bridge ready: ${ping.message} from ${ping.appName}; plugin=${greeting.appName}`;
    if (details) {
      details.textContent = JSON.stringify(
        {
          appInfo,
          appVersion,
          appEcho,
          windowInfo,
          greeting,
          pluginEvent,
          fsWrite,
          fsRead,
          dialogOpen,
          dialogSave,
          hostLog,
          lifecycleEvents,
        },
        null,
        2,
      );
    }
  } catch (error) {
    status.textContent = `Axion invoke failed: ${error instanceof Error ? error.message : String(error)}`;
    if (details) {
      details.textContent = status.textContent;
    }
  }
});
